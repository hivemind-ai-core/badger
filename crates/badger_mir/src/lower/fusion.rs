//! Lower visible, finite standard-library sequence pipelines as one loop.
//! Resolve definitions, not spellings. Only range/list sources are accepted:
//! unknown and scoped iterators retain their normal close/unwind protocol.
//! Construction arguments are evaluated once, in source order, before any
//! iteration. Filters and take limits retain their position in the pipeline.
use super::*;

enum Source {
    Bounds(ExprId, ExprId),
    Range(ExprId, bool),
    List(ExprId, Ty, bool),
}
enum Stage {
    Map(ExprId),
    Filter(ExprId),
    Take(ExprId),
}
enum Active {
    Map(Operand, Repr),
    Filter(Operand),
    Take(Local),
}
enum Terminal {
    Fold(ExprId, ExprId),
    Count,
    For(PatId, ExprId),
}
pub(super) struct Plan {
    source: Source,
    stages: Vec<Stage>,
    terminal: Terminal,
}

impl FnLower<'_, '_, '_> {
    fn sequence_call(&self, e: ExprId) -> Option<(String, Vec<ExprId>)> {
        let (def, args) = match &self.hir().exprs[e].kind {
            ExprKind::Call { callee, args } => {
                let ExprKind::Def { def, .. } = &self.hir().exprs[*callee].kind else {
                    return None;
                };
                (*def, args.clone())
            }
            ExprKind::MethodCall { recv, args, .. } => {
                let Res::Method(Target::Fn { def, .. }) = self.inst.res.get(&e)? else {
                    return None;
                };
                (*def, std::iter::once(*recv).chain(args.iter().copied()).collect())
            }
            _ => return None,
        };
        let module = &self.hir().modules[self.hir().defs[def].module];
        module.is_std.then(|| (self.m.def_path(def), args))
    }

    fn sequence_plan(&self, e: ExprId, depth: usize) -> Option<(Source, Vec<Stage>)> {
        if depth > 16 {
            return None;
        }
        match self.inst.coercions.get(&e) {
            Some(badger_typeck::Coercion::RangeToSeq) => return Some((Source::Range(e, true), vec![])),
            Some(badger_typeck::Coercion::ToSeq) if self.m.lay.is_std(&self.ty(e), "List") => {
                let elem = self.m.lay.args(&self.ty(e)).first()?.clone();
                return Some((Source::List(e, elem, true), vec![]));
            }
            _ => {}
        }
        let (name, args) = self.sequence_call(e)?;
        match (name.as_str(), args.as_slice()) {
            ("std.seq.range", [lo, hi]) => Some((Source::Bounds(*lo, *hi), vec![])),
            ("std.seq.from_range", [r]) => Some((Source::Range(*r, false), vec![])),
            ("std.list.of" | "std.list.seq", [list]) => {
                let elem = self.m.lay.args(&self.ty(*list)).first()?.clone();
                Some((Source::List(*list, elem, false), vec![]))
            }
            ("std.seq.map" | "std.seq.filter" | "std.seq.take", [inner, arg]) => {
                let (source, mut stages) = self.sequence_plan(*inner, depth + 1)?;
                stages.push(match name.as_str() {
                    "std.seq.map" => Stage::Map(*arg),
                    "std.seq.filter" => Stage::Filter(*arg),
                    _ => Stage::Take(*arg),
                });
                Some((source, stages))
            }
            _ => None,
        }
    }

    pub(super) fn fused_terminal(&self, e: ExprId) -> Option<Plan> {
        let (name, args) = self.sequence_call(e)?;
        let (iter, terminal) = match (name.as_str(), args.as_slice()) {
            ("std.seq.fold", [iter, initial, f]) => (*iter, Terminal::Fold(*initial, *f)),
            ("std.seq.count", [iter]) => (*iter, Terminal::Count),
            _ => return None,
        };
        let (source, stages) = self.sequence_plan(iter, 0)?;
        Some(Plan { source, stages, terminal })
    }

    pub(super) fn fused_for(&self, iter: ExprId, pat: PatId, body: ExprId) -> Option<Plan> {
        let (source, stages) = self.sequence_plan(iter, 0)?;
        Some(Plan { source, stages, terminal: Terminal::For(pat, body) })
    }

    pub(super) fn lower_fused(&mut self, e: ExprId, plan: Plan) -> Option<Operand> {
        let mut list_source = None;
        let (start, end, inclusive) = match plan.source {
            Source::Bounds(lo, hi) => (self.lower(lo)?, self.lower(hi)?, int(0)),
            Source::Range(expr, coerced) => {
                let r = if coerced { self.lower_inner(expr)? } else { self.lower(expr)? };
                let mut field =
                    |offset| self.assign(Repr::I64, Rvalue::Load { base: r, offset, repr: Repr::I64 });
                (field(0), field(8), field(16))
            }
            Source::List(expr, elem, coerced) => {
                let l = if coerced { self.lower_inner(expr)? } else { self.lower(expr)? };
                let n = self.rt("badger_rt_list_len", vec![l], Repr::I64);
                list_source = Some((l, elem));
                (int(0), n, int(0))
            }
        };
        let mut stages = vec![];
        for stage in plan.stages {
            stages.push(match stage {
                Stage::Map(expr) => {
                    let TyKind::Fn(ft) = &*self.ty(expr) else { unreachable!() };
                    let ret = self.repr(&ft.ret);
                    Active::Map(self.lower(expr)?, ret)
                }
                Stage::Filter(expr) => Active::Filter(self.lower(expr)?),
                Stage::Take(expr) => {
                    let n = self.lower(expr)?;
                    Active::Take(self.copy_to_local(n, Repr::I64, None))
                }
            });
        }
        let ret = match plan.terminal {
            Terminal::For(..) => Repr::I8,
            _ => self.repr(&self.ty(e)),
        };
        let (acc, fold) = match plan.terminal {
            Terminal::Fold(initial, f) => {
                let initial = self.lower(initial)?;
                let acc = self.copy_to_local(initial, ret, None);
                (Some(acc), Some(self.lower(f)?))
            }
            Terminal::Count => (Some(self.copy_to_local(int(0), Repr::I64, None)), None),
            Terminal::For(..) => (None, None),
        };
        let i = self.copy_to_local(start, Repr::I64, None);
        let (head, body, step, exit) = (self.block(), self.block(), self.block(), self.block());
        let cmp =
            |s: &mut Self, op, a, b| s.assign(Repr::I8, Rvalue::Cmp { op, a, b, ty: NumTy::I(64, true) });
        let bin = |s: &mut Self, op, a, b| {
            s.assign(Repr::I64, Rvalue::Bin { op, a, b, ty: NumTy::I(64, false), loc: 0 })
        };
        self.term(Term::Goto(head));
        self.switch(head);
        self.emit(Stmt::Poll);
        // An exhausted downstream take stops before requesting another
        // upstream item, including before upstream map/filter side effects.
        for stage in &stages {
            if let Active::Take(left) = stage {
                let empty = cmp(self, CmpOp::Le, Operand::Local(*left), int(0));
                let more = self.block();
                self.term(Term::Branch(empty, exit, more));
                self.switch(more);
            }
        }
        let less = cmp(self, CmpOp::Lt, Operand::Local(i), end);
        let equal = cmp(self, CmpOp::Eq, Operand::Local(i), end);
        let incl = self.assign(
            Repr::I8,
            Rvalue::Convert {
                a: inclusive,
                from: NumTy::I(64, false),
                to: NumTy::I(8, false),
                checked: false,
                loc: 0,
            },
        );
        let at_end = self.assign(
            Repr::I8,
            Rvalue::Bin { op: BinOp::And, a: equal, b: incl, ty: NumTy::I(8, false), loc: 0 },
        );
        let more = self.assign(
            Repr::I8,
            Rvalue::Bin { op: BinOp::Or, a: less, b: at_end, ty: NumTy::I(8, false), loc: 0 },
        );
        self.term(Term::Branch(more, body, exit));
        self.switch(body);
        let mut value = if let Some((list, elem)) = list_source {
            let loc = self.m.loc(self.span(e));
            let word = self.rt(
                "badger_rt_list_get",
                vec![list, Operand::Local(i), Operand::Const(Const::Addr(loc))],
                Repr::I64,
            );
            self.from_word(word, &elem)
        } else {
            Operand::Local(i)
        };
        for stage in stages {
            match stage {
                Active::Map(f, ret) => value = self.call(Callee::Closure(f), vec![value], ret),
                Active::Filter(f) => {
                    let keep = self.call(Callee::Closure(f), vec![value], Repr::I8);
                    let accepted = self.block();
                    self.term(Term::Branch(keep, accepted, step));
                    self.switch(accepted);
                }
                Active::Take(left) => {
                    let n = bin(self, BinOp::Sub, Operand::Local(left), int(1));
                    self.emit(Stmt::Assign(left, Rvalue::Use(n)));
                }
            }
        }
        match plan.terminal {
            Terminal::For(pat, expr) => {
                let ty = self.pat_ty(pat);
                self.bind_irrefutable(pat, value, &ty);
                self.loops.push(LoopCtx { expr: e, cont: step, brk: exit, result: None, close: None });
                if self.lower(expr).is_some() {
                    self.term(Term::Goto(step));
                }
                self.loops.pop();
            }
            _ => {
                let acc = acc.unwrap();
                let next = if let Some(f) = fold {
                    self.call(Callee::Closure(f), vec![Operand::Local(acc), value], ret)
                } else {
                    let loc = self.m.loc(self.span(e));
                    self.assign(
                        Repr::I64,
                        Rvalue::Bin {
                            op: BinOp::Add,
                            a: Operand::Local(acc),
                            b: int(1),
                            ty: NumTy::I(64, true),
                            loc,
                        },
                    )
                };
                self.emit(Stmt::Assign(acc, Rvalue::Use(next)));
                self.term(Term::Goto(step));
            }
        }
        self.switch(step);
        // Inclusive Int::MAX must stop without stepping past the endpoint.
        let last = cmp(self, CmpOp::Eq, Operand::Local(i), end);
        let advance = self.block();
        self.term(Term::Branch(last, exit, advance));
        self.switch(advance);
        let next = bin(self, BinOp::Add, Operand::Local(i), int(1));
        self.emit(Stmt::Assign(i, Rvalue::Use(next)));
        self.term(Term::Goto(head));
        self.switch(exit);
        Some(acc.map_or_else(unit_op, Operand::Local))
    }
}
