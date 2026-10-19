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
    Stored(LocalId),
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
#[derive(Clone)]
enum PreparedStage {
    Map(Operand, Repr),
    Filter(Operand),
    Take(Operand),
}
/// Construction-time snapshots. Traversals create their own mutable position
/// and take counters; every retained pointer remains a typed MIR operand.
#[derive(Clone)]
pub(super) struct Prepared {
    start: Operand,
    end: Operand,
    inclusive: Operand,
    list: Option<(Operand, Ty)>,
    stages: Vec<PreparedStage>,
}
pub(super) struct SequencePlan(Source, Vec<Stage>);
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
        if let ExprKind::Local(local) = self.hir().exprs[e].kind
            && self.stored_sequences.contains_key(&local)
        {
            return Some((Source::Stored(local), vec![]));
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

    /// Find the local at the base of a supported terminal's inline adapters.
    /// Returning the exact expression (not just its local) lets the use audit
    /// reject even one escaping reference to an otherwise eligible binding.
    fn sequence_local_use(&self, e: ExprId, depth: usize) -> Option<ExprId> {
        if depth > 16 {
            return None;
        }
        if matches!(self.hir().exprs[e].kind, ExprKind::Local(_)) {
            return Some(e);
        }
        let (name, args) = self.sequence_call(e)?;
        match (name.as_str(), args.as_slice()) {
            ("std.seq.map" | "std.seq.filter" | "std.seq.take", [inner, _]) => {
                self.sequence_local_use(*inner, depth + 1)
            }
            _ => None,
        }
    }

    fn eligible_sequence_locals(&self) -> rustc_hash::FxHashSet<LocalId> {
        use rustc_hash::FxHashSet;
        let hir = self.hir();
        let body = match self.owner {
            Owner::Fn(def) => match &hir.defs[def].kind {
                DefKind::Fn(f) => f.body,
                _ => None,
            },
            Owner::Global(def) => match &hir.defs[def].kind {
                DefKind::Global(g) => Some(g.value),
                _ => None,
            },
            Owner::Lambda(e) => match &hir.exprs[e].kind {
                ExprKind::Lambda(l) => Some(l.body),
                _ => None,
            },
        };
        let Some(body) = body else { return FxHashSet::default() };
        let mut exprs = vec![];
        badger_typeck::program::walk_exprs(hir, body, &mut |e| exprs.push(e));
        let mut allowed = FxHashSet::default();
        for &e in &exprs {
            let iter = match &hir.exprs[e].kind {
                ExprKind::For { iter, .. }
                    if matches!(self.inst.res.get(&e), Some(Res::Iter(IterKind::Seq))) =>
                {
                    Some(*iter)
                }
                _ => self.sequence_call(e).and_then(|(name, args)| match (name.as_str(), args.as_slice()) {
                    ("std.seq.fold", [iter, _, _]) | ("std.seq.count", [iter]) => Some(*iter),
                    _ => None,
                }),
            };
            if let Some(iter) = iter
                && let Some(reference) = self.sequence_local_use(iter, 0)
            {
                allowed.insert(reference);
            }
        }
        let mut candidates = FxHashSet::default();
        let mut rejected = FxHashSet::default();
        for e in exprs {
            match &hir.exprs[e].kind {
                ExprKind::Local(local) => {
                    if allowed.contains(&e) {
                        candidates.insert(*local);
                    } else {
                        rejected.insert(*local);
                    }
                }
                ExprKind::Lambda(l) => rejected.extend(l.captures.iter().copied()),
                _ => {}
            }
        }
        candidates.retain(|local| {
            let l = &hir.locals[*local];
            l.owner == Some(self.owner)
                && !l.mutable
                && !rejected.contains(local)
                && !self.inst.poly_locals.contains_key(local)
        });
        candidates
    }

    pub(super) fn stored_sequence_plan(
        &mut self,
        pat: PatId,
        init: ExprId,
    ) -> Option<(LocalId, SequencePlan)> {
        if !self.m.fuse_sequences {
            return None;
        }
        let PatKind::Bind(local) = self.hir().pats[pat].kind else { return None };
        if !self.m.lay.is_std(&self.pat_ty(pat), "Seq") {
            return None;
        }
        if self.sequence_locals.is_none() {
            self.sequence_locals = Some(self.eligible_sequence_locals());
        }
        if !self.sequence_locals.as_ref()?.contains(&local) {
            return None;
        }
        let (source, stages) = self.sequence_plan(init, 0)?;
        Some((local, SequencePlan(source, stages)))
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

    pub(super) fn prepare_sequence(&mut self, plan: SequencePlan) -> Option<Prepared> {
        let SequencePlan(source, stages) = plan;
        let mut list_source = None;
        let mut prepared = if let Source::Stored(local) = source {
            self.stored_sequences[&local].clone()
        } else {
            let (start, end, inclusive) = match source {
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
                Source::Stored(_) => unreachable!(),
            };
            Prepared { start, end, inclusive, list: list_source, stages: vec![] }
        };
        for stage in stages {
            prepared.stages.push(match stage {
                Stage::Map(expr) => {
                    let TyKind::Fn(ft) = &*self.ty(expr) else { unreachable!() };
                    let ret = self.repr(&ft.ret);
                    PreparedStage::Map(self.lower(expr)?, ret)
                }
                Stage::Filter(expr) => PreparedStage::Filter(self.lower(expr)?),
                Stage::Take(expr) => PreparedStage::Take(self.lower(expr)?),
            });
        }
        Some(prepared)
    }

    pub(super) fn lower_fused(&mut self, e: ExprId, plan: Plan) -> Option<Operand> {
        let Prepared { start, end, inclusive, list: list_source, stages } =
            self.prepare_sequence(SequencePlan(plan.source, plan.stages))?;
        let stages: Vec<_> = stages
            .into_iter()
            .map(|stage| match stage {
                PreparedStage::Map(f, ret) => Active::Map(f, ret),
                PreparedStage::Filter(f) => Active::Filter(f),
                PreparedStage::Take(n) => Active::Take(self.copy_to_local(n, Repr::I64, None)),
            })
            .collect();
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
