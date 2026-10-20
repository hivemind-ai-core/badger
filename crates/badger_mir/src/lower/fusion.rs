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
/// The positional stages share one shape: a counter that admits some items
/// and drops the rest. `take` admits while the count lasts, `skip` drops
/// the first `n`, and `step_by` passes one then drops `n - 1`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CursorMode {
    Take,
    Skip,
    StepBy,
}
enum Stage {
    Map(ExprId),
    Filter(ExprId),
    /// `take`/`skip`/`step_by`: the counter's argument.
    Cursor(CursorMode, ExprId),
    FilterMap(ExprId),
    /// `f` and the initial accumulator.
    Scan(ExprId, ExprId),
    /// The `(Int, T)` produced.
    Enumerate(Ty),
    /// The element type (for structural equality).
    Dedup(Ty),
}
enum Active {
    Map(Operand, Repr),
    Filter(Operand),
    /// The argument and the counter it spends.
    Cursor { mode: CursorMode, arg: Local, left: Local },
    /// The callback and the `Option<U>` it returns.
    FilterMap(Operand, Ty),
    /// The callback, the running accumulator, and its representation.
    Scan(Operand, Local, Repr),
    /// The counter and the `(Int, T)` produced.
    Enumerate(Local, Ty),
    /// Previous item, a `seen` flag, and the element descriptor.
    Dedup(Local, Local, Operand),
}
#[derive(Clone)]
enum PreparedStage {
    Map(Operand, Repr),
    Filter(Operand),
    Cursor(CursorMode, Operand),
    FilterMap(Operand, Ty),
    Scan(Operand, Operand, Repr),
    Enumerate(Ty),
    Dedup(Ty, Repr, Operand),
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
/// The terminal accumulators share one shape: a running value updated per
/// item. `count` adds one, `sum` adds the item, and `product` multiplies.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Combine {
    Add,
    Count,
    Mul,
}
enum Terminal {
    Fold(ExprId, ExprId),
    Accumulate(Combine),
    Collect,
    /// `any`/`all`: the predicate decides, and the loop stops at the first
    /// item that does.
    Any(ExprId),
    All(ExprId),
    /// The first item the predicate accepts, the first `some` the callback
    /// gives, the first item, and the position of the first accepted item.
    /// Each stops at the item it returns.
    Find(ExprId),
    FindMap(ExprId),
    First,
    Position(ExprId),
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

    /// The fusion template of a called standard-library definition: from its
    /// `@fusion(...)` annotation, or the legacy path table (kept while the
    /// annotations are rolled out) so nothing regresses.
    fn fusion_template(&self, name: &str) -> Option<FusionTemplate> {
        if let Some(t) = self.m.fusion.get(name) {
            return Some(t.clone());
        }
        Some(match name {
            "std.seq.range" => FusionTemplate::new("range", &[]),
            "std.seq.from_range" => FusionTemplate::new("from_range", &[]),
            "std.list.of" | "std.list.seq" => FusionTemplate::new("list", &[]),
            "std.list.map" | "std.list.filter" => FusionTemplate::new("call", &[]),
            "std.seq.map" | "std.seq.filter" | "std.seq.filter_map" => FusionTemplate::new("call", &[]),
            "std.seq.take" => FusionTemplate::new("cursor", &[("mode", "take")]),
            "std.seq.skip" => FusionTemplate::new("cursor", &[("mode", "skip")]),
            "std.seq.step_by" => FusionTemplate::new("cursor", &[("mode", "step_by")]),
            "std.seq.scan" => FusionTemplate::new("scan", &[]),
            "std.seq.enumerate" => FusionTemplate::new("enumerate", &[]),
            "std.seq.dedup" => FusionTemplate::new("dedup", &[]),
            "std.seq.fold" => FusionTemplate::new("fold", &[]),
            "std.seq.count" => FusionTemplate::new("accumulate", &[("combine", "count")]),
            "std.seq.sum" => FusionTemplate::new("accumulate", &[("combine", "add")]),
            "std.seq.product" => FusionTemplate::new("accumulate", &[("combine", "mul")]),
            "std.seq.collect" | "std.list.collect" => FusionTemplate::new("collect", &[]),
            "std.seq.any" => FusionTemplate::new("any", &[]),
            "std.seq.all" => FusionTemplate::new("all", &[]),
            "std.seq.find" => FusionTemplate::new("find", &[]),
            "std.seq.find_map" => FusionTemplate::new("find_map", &[]),
            "std.seq.first" => FusionTemplate::new("first", &[]),
            "std.seq.position" => FusionTemplate::new("position", &[]),
            _ => return None,
        })
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
        let template = self.fusion_template(&name)?;
        match (template.kind.as_str(), args.as_slice()) {
            ("range", [lo, hi]) => Some((Source::Bounds(*lo, *hi), vec![])),
            ("from_range", [r]) => Some((Source::Range(*r, false), vec![])),
            ("list", [list]) => {
                let elem = self.m.lay.args(&self.ty(*list)).first()?.clone();
                Some((Source::List(*list, elem, false), vec![]))
            }
            ("cursor", [inner, arg]) => {
                let (source, mut stages) = self.sequence_plan(*inner, depth + 1)?;
                // `mode` is checked where the annotation is read; a marking
                // without one does not fuse.
                let mode = match template.param("mode")? {
                    "take" => CursorMode::Take,
                    "skip" => CursorMode::Skip,
                    "step_by" => CursorMode::StepBy,
                    _ => return None,
                };
                stages.push(Stage::Cursor(mode, *arg));
                Some((source, stages))
            }
            ("call", [inner, arg]) => {
                // the callback's shape decides the stage; a `Seq` result
                // keeps this the sequence pipeline (a `List.map` chain is
                // the eager engine's)
                let t = self.ty(e);
                if !self.m.lay.is_std(&t, "Seq") {
                    return None;
                }
                let (source, mut stages) = self.sequence_plan(*inner, depth + 1)?;
                stages.push(if self.callback_returns_bool(*arg) {
                    Stage::Filter(*arg)
                } else if self.callback_returns_option(*arg) {
                    Stage::FilterMap(*arg)
                } else {
                    Stage::Map(*arg)
                });
                Some((source, stages))
            }
            ("scan", [inner, initial, f]) => {
                let (source, mut stages) = self.sequence_plan(*inner, depth + 1)?;
                stages.push(Stage::Scan(*initial, *f));
                Some((source, stages))
            }
            ("enumerate", [inner]) => {
                let (source, mut stages) = self.sequence_plan(*inner, depth + 1)?;
                let t = self.ty(e);
                let elem = self.m.lay.args(&t).first()?.clone();
                stages.push(Stage::Enumerate(elem));
                Some((source, stages))
            }
            ("dedup", [inner]) => {
                let (source, mut stages) = self.sequence_plan(*inner, depth + 1)?;
                let t = self.ty(e);
                let elem = self.m.lay.args(&t).first()?.clone();
                stages.push(Stage::Dedup(elem));
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
        let template = self.fusion_template(&name)?;
        match (template.kind.as_str(), args.as_slice()) {
            ("call" | "cursor", [inner, _]) => self.sequence_local_use(*inner, depth + 1),
            ("scan", [inner, _, _]) => self.sequence_local_use(*inner, depth + 1),
            ("enumerate", [inner]) => self.sequence_local_use(*inner, depth + 1),
            ("dedup", [inner]) => self.sequence_local_use(*inner, depth + 1),
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
                _ => self.sequence_call(e).and_then(|(name, args)| {
                    let template = self.fusion_template(&name)?;
                    match (template.kind.as_str(), args.as_slice()) {
                        ("fold", [iter, _, _]) | ("accumulate" | "collect", [iter]) => Some(*iter),
                        _ => None,
                    }
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
        let template = self.fusion_template(&name)?;
        let (iter, terminal) = match (template.kind.as_str(), args.as_slice()) {
            ("fold", [iter, initial, f]) => (*iter, Terminal::Fold(*initial, *f)),
            ("accumulate", [iter]) => (
                *iter,
                Terminal::Accumulate(match template.param("combine")? {
                    "add" => Combine::Add,
                    "count" => Combine::Count,
                    "mul" => Combine::Mul,
                    _ => return None,
                }),
            ),
            ("collect", [iter]) => (*iter, Terminal::Collect),
            ("any", [iter, test]) => (*iter, Terminal::Any(*test)),
            ("all", [iter, test]) => (*iter, Terminal::All(*test)),
            ("find", [iter, matches]) => (*iter, Terminal::Find(*matches)),
            ("find_map", [iter, f]) => (*iter, Terminal::FindMap(*f)),
            ("first", [iter]) => (*iter, Terminal::First),
            ("position", [iter, matches]) => (*iter, Terminal::Position(*matches)),
            _ => return None,
        };
        let (source, stages) = self.sequence_plan(iter, 0)?;
        Some(Plan { source, stages, terminal })
    }

    pub(super) fn fused_for(&self, iter: ExprId, pat: PatId, body: ExprId) -> Option<Plan> {
        let (source, stages) = self.sequence_plan(iter, 0)?;
        Some(Plan { source, stages, terminal: Terminal::For(pat, body) })
    }

    /// A source's construction-time bounds: the first index, the exclusive
    /// end, the inclusive flag, and the backing list when there is one. The
    /// construction arguments are evaluated once, here, in source order.
    #[allow(clippy::type_complexity)]
    fn lower_source(
        &mut self,
        source: &Source,
    ) -> Option<(Operand, Operand, Operand, Option<(Operand, Ty)>)> {
        Some(match source {
            Source::Bounds(lo, hi) => (self.lower(*lo)?, self.lower(*hi)?, int(0), None),
            Source::Range(expr, coerced) => {
                let r = if *coerced { self.lower_inner(*expr)? } else { self.lower(*expr)? };
                let mut field =
                    |offset| self.assign(Repr::I64, Rvalue::Load { base: r, offset, repr: Repr::I64 });
                (field(0), field(8), field(16), None)
            }
            Source::List(expr, elem, coerced) => {
                let l = if *coerced { self.lower_inner(*expr)? } else { self.lower(*expr)? };
                let n = self.rt("badger_rt_list_len", vec![l], Repr::I64);
                (int(0), n, int(0), Some((l, elem.clone())))
            }
            Source::Stored(_) => return None,
        })
    }

    pub(super) fn prepare_sequence(&mut self, plan: SequencePlan) -> Option<Prepared> {
        let SequencePlan(source, stages) = plan;
        let mut prepared = if let Source::Stored(local) = source {
            self.stored_sequences[&local].clone()
        } else {
            let (start, end, inclusive, list) = self.lower_source(&source)?;
            Prepared { start, end, inclusive, list, stages: vec![] }
        };
        for stage in stages {
            prepared.stages.push(match stage {
                Stage::Map(expr) => {
                    let TyKind::Fn(ft) = &*self.ty(expr) else { unreachable!() };
                    let ret = self.repr(&ft.ret);
                    PreparedStage::Map(self.lower(expr)?, ret)
                }
                Stage::Filter(expr) => PreparedStage::Filter(self.lower(expr)?),
                Stage::Cursor(mode, expr) => PreparedStage::Cursor(mode, self.lower(expr)?),
                Stage::FilterMap(expr) => {
                    let TyKind::Fn(ft) = &*self.ty(expr) else { unreachable!() };
                    // the callback returns `Option<U>`
                    PreparedStage::FilterMap(self.lower(expr)?, ft.ret.clone())
                }
                Stage::Scan(initial, f) => {
                    let TyKind::Fn(ft) = &*self.ty(f) else { unreachable!() };
                    // `f: fn(A, T) -> A`: the accumulator's representation
                    let ret = self.repr(&ft.params[0]);
                    PreparedStage::Scan(self.lower(f)?, self.lower(initial)?, ret)
                }
                Stage::Enumerate(ty) => PreparedStage::Enumerate(ty),
                Stage::Dedup(ty) => {
                    let repr = self.repr(&ty);
                    let desc = self.desc_op(&ty);
                    PreparedStage::Dedup(ty, repr, desc)
                }
            });
        }
        Some(prepared)
    }

    /// Whether `count` can be answered from the source's span alone. Only the
    /// 1:1 stages keep it, and skipping their callbacks is unobservable only
    /// when each is pure and infallible: a defect or an effect must still
    /// happen. (`scan`'s initial is evaluated at construction, so it is not
    /// eligible here even though it is 1:1.)
    fn count_is_known(&self, plan: &Plan) -> bool {
        !matches!(plan.source, Source::Stored(_))
            && plan.stages.iter().all(|stage| match stage {
                Stage::Enumerate(_) => true,
                Stage::Map(cb) => self.callback_pure_infallible(*cb),
                _ => false,
            })
    }

    /// A `Bool` operand: the `Int`-literal conversion a lowered `Bool`
    /// constant uses.
    fn bool_op(&mut self, value: bool) -> Operand {
        self.assign(
            Repr::I8,
            Rvalue::Convert {
                a: int(i64::from(value)),
                from: NumTy::I(64, false),
                to: NumTy::I(8, false),
                checked: false,
                loc: 0,
            },
        )
    }

    /// `max(0, (end - start) + inclusive)`: how many items the source yields.
    /// A start past the end is an empty range, which the subtraction alone
    /// would report as negative.
    fn clamped_span(&mut self, start: Operand, end: Operand, inclusive: Operand) -> Operand {
        let bin = |s: &mut Self, op, a, b| {
            s.assign(Repr::I64, Rvalue::Bin { op, a, b, ty: NumTy::I(64, false), loc: 0 })
        };
        let span = bin(self, BinOp::Sub, end, start);
        let raw = bin(self, BinOp::Add, span, inclusive);
        let out = self.copy_to_local(raw, Repr::I64, None);
        let negative = self.assign(
            Repr::I8,
            Rvalue::Cmp { op: CmpOp::Lt, a: raw, b: int(0), ty: NumTy::I(64, true) },
        );
        let empty = self.block();
        let done = self.block();
        self.term(Term::Branch(negative, empty, done));
        self.switch(empty);
        self.emit(Stmt::Assign(out, Rvalue::Use(int(0))));
        self.term(Term::Goto(done));
        self.switch(done);
        Operand::Local(out)
    }

    pub(super) fn lower_fused(&mut self, e: ExprId, plan: Plan) -> Option<Operand> {
        if matches!(plan.terminal, Terminal::Accumulate(Combine::Count))
            && self.count_is_known(&plan)
        {
            let (start, end, inclusive, _) = self.lower_source(&plan.source)?;
            return Some(self.clamped_span(start, end, inclusive));
        }
        let Prepared { start, end, inclusive, list: list_source, stages } =
            self.prepare_sequence(SequencePlan(plan.source, plan.stages))?;
        let stages: Vec<_> = stages
            .into_iter()
            .map(|stage| match stage {
                PreparedStage::Map(f, ret) => Active::Map(f, ret),
                PreparedStage::Filter(f) => Active::Filter(f),
                PreparedStage::Cursor(mode, n) => {
                    let arg = self.copy_to_local(n, Repr::I64, None);
                    // `step_by` starts having passed nothing and is primed by
                    // the last stage below; the others spend a count from the
                    // start.
                    let left = match mode {
                        CursorMode::StepBy => self.copy_to_local(int(0), Repr::I64, None),
                        _ => self.copy_to_local(Operand::Local(arg), Repr::I64, None),
                    };
                    Active::Cursor { mode, arg, left }
                }
                PreparedStage::FilterMap(f, opt) => Active::FilterMap(f, opt),
                PreparedStage::Scan(f, initial, ret) => {
                    Active::Scan(f, self.copy_to_local(initial, ret, None), ret)
                }
                PreparedStage::Enumerate(ty) => {
                    Active::Enumerate(self.copy_to_local(int(0), Repr::I64, None), ty)
                }
                PreparedStage::Dedup(_ty, repr, desc) => {
                    // `prev` is read only after `seen` is set, so a zero
                    // placeholder is safe
                    let prev = self.copy_to_local(Operand::Const(Const::Int(0, repr)), repr, None);
                    let seen = self.copy_to_local(int(0), Repr::I64, None);
                    Active::Dedup(prev, seen, desc)
                }
            })
            .collect();
        let ret = match plan.terminal {
            Terminal::For(..) => Repr::I8,
            Terminal::Collect => Repr::Ptr,
            _ => self.repr(&self.ty(e)),
        };
        let (acc, fold) = match plan.terminal {
            Terminal::Fold(initial, f) => {
                let initial = self.lower(initial)?;
                let acc = self.copy_to_local(initial, ret, None);
                (Some(acc), Some(self.lower(f)?))
            }
            // `count` and `sum` start at 0, `product` at 1.
            Terminal::Accumulate(combine) => {
                let start = if combine == Combine::Mul { int(1) } else { int(0) };
                (Some(self.copy_to_local(start, Repr::I64, None)), None)
            }
            // `any` keeps false and `all` keeps true: the answer when no item
            // decides. The loop overwrites it and exits at the first decider.
            Terminal::Any(_) | Terminal::All(_) => {
                let init = self.bool_op(matches!(plan.terminal, Terminal::All(_)));
                (Some(self.copy_to_local(init, Repr::I8, None)), None)
            }
            // The searches keep `none`: the answer when nothing is found.
            Terminal::Find(_) | Terminal::FindMap(_) | Terminal::First | Terminal::Position(_) => {
                let opt = self.ty(e);
                let none = self.alloc_enum(&opt, 0, vec![]);
                (Some(self.copy_to_local(none, Repr::Ptr, None)), None)
            }
            Terminal::Collect | Terminal::For(..) => (None, None),
        };
        // The search predicate is construction-time, like a fold's callback.
        let search = match plan.terminal {
            Terminal::Any(cb)
            | Terminal::All(cb)
            | Terminal::Find(cb)
            | Terminal::FindMap(cb)
            | Terminal::Position(cb) => Some(self.lower(cb)?),
            _ => None,
        };
        // `position` counts the items that reach the terminal, not the
        // source's index (an earlier filter would drop some).
        let pos = match plan.terminal {
            Terminal::Position(_) => Some(self.copy_to_local(int(0), Repr::I64, None)),
            _ => None,
        };
        // `collect` builds its list in the fused loop. When the source count
        // is known and no filter can drop items, pages are sized up front.
        let collect = if matches!(plan.terminal, Terminal::Collect) {
            let list_ty = self.ty(e);
            let elem = self.m.lay.args(&list_ty).first()?.clone();
            let no_filter = !stages.iter().any(|s| matches!(s, Active::Filter(_)));
            let span = self.span(e);
            let b = if no_filter {
                let span_diff = self.assign(
                    Repr::I64,
                    Rvalue::Bin { op: BinOp::Sub, a: end, b: start, ty: NumTy::I(64, false), loc: 0 },
                );
                let upper = self.assign(
                    Repr::I64,
                    Rvalue::Bin {
                        op: BinOp::Add,
                        a: span_diff,
                        b: inclusive,
                        ty: NumTy::I(64, false),
                        loc: 0,
                    },
                );
                self.intrinsic("list_builder_capacity", vec![elem.clone()], vec![upper], &list_ty, span)?
            } else {
                self.intrinsic("list_builder", vec![elem.clone()], vec![], &list_ty, span)?
            };
            Some((self.copy_to_local(b, Repr::Ptr, None), elem))
        } else {
            None
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
            if let Active::Cursor { mode: CursorMode::Take, left, .. } = stage {
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
                Active::Cursor { mode, arg, left } => match mode {
                    // `take` admits while the count lasts
                    CursorMode::Take => {
                        let dec = bin(self, BinOp::Sub, Operand::Local(left), int(1));
                        self.emit(Stmt::Assign(left, Rvalue::Use(dec)));
                    }
                    // `skip` and `step_by` drop while the skip lasts
                    CursorMode::Skip | CursorMode::StepBy => {
                        let positive = cmp(self, CmpOp::Gt, Operand::Local(left), int(0));
                        let skipping = self.block();
                        let passing = self.block();
                        self.term(Term::Branch(positive, skipping, passing));
                        self.switch(skipping);
                        let dec = bin(self, BinOp::Sub, Operand::Local(left), int(1));
                        self.emit(Stmt::Assign(left, Rvalue::Use(dec)));
                        self.term(Term::Goto(step));
                        self.switch(passing);
                        if mode == CursorMode::StepBy {
                            // one passed: drop the rest of the step
                            let reset = bin(self, BinOp::Sub, Operand::Local(arg), int(1));
                            self.emit(Stmt::Assign(left, Rvalue::Use(reset)));
                        }
                    }
                },
                Active::FilterMap(f, opt_ty) => {
                    // `f(x)` is `Option<U>`: pass on `some`, skip `none`
                    let opt = self.call(Callee::Closure(f), vec![value], Repr::Ptr);
                    let ol = self.copy_to_local(opt, Repr::Ptr, None);
                    let tag = self.assign(Repr::I32, Rvalue::Tag(Operand::Local(ol)));
                    let some = self.block();
                    self.term(Term::Switch(tag, vec![(1, some)], step));
                    self.switch(some);
                    let (x, _r) = self.variant_field(&opt_ty, 1, 0, Operand::Local(ol));
                    value = x;
                }
                Active::Scan(f, state, ret) => {
                    // the running accumulator becomes the item
                    let next = self.call(Callee::Closure(f), vec![Operand::Local(state), value], ret);
                    self.emit(Stmt::Assign(state, Rvalue::Use(next)));
                    value = Operand::Local(state);
                }
                Active::Enumerate(idx, ty) => {
                    // `(index, item)`: index counts items that reach here.
                    // Snapshot it before incrementing (the local is mutated).
                    let idxv = Operand::Local(self.copy_to_local(Operand::Local(idx), Repr::I64, None));
                    let inc = bin(self, BinOp::Add, Operand::Local(idx), int(1));
                    self.emit(Stmt::Assign(idx, Rvalue::Use(inc)));
                    let layout = self.m.lay.record(&ty);
                    let desc = self.m.lay.desc(&ty);
                    let obj = self.assign(
                        Repr::Ptr,
                        Rvalue::Alloc { desc: desc as *const _ as u64, size: layout.size },
                    );
                    self.emit(Stmt::Store {
                        base: obj,
                        offset: layout.fields[0].offset,
                        repr: layout.fields[0].repr,
                        value: idxv,
                    });
                    self.emit(Stmt::Store {
                        base: obj,
                        offset: layout.fields[1].offset,
                        repr: layout.fields[1].repr,
                        value,
                    });
                    value = obj;
                }
                Active::Dedup(prev, seen, desc) => {
                    // collapse consecutive equal items: keep the first, drop
                    // later equal ones (structural equality)
                    let first = self.block();
                    let rest = self.block();
                    let unseen = cmp(self, CmpOp::Eq, Operand::Local(seen), int(0));
                    self.term(Term::Branch(unseen, first, rest));
                    let passing = self.block();
                    self.switch(first);
                    self.emit(Stmt::Assign(seen, Rvalue::Use(int(1))));
                    self.emit(Stmt::Assign(prev, Rvalue::Use(value)));
                    self.term(Term::Goto(passing));
                    self.switch(rest);
                    let eq = self.rt("badger_rt_eq", vec![desc, Operand::Local(prev), value], Repr::I8);
                    let eqb = self.copy_to_local(eq, Repr::I8, None);
                    let changed = self.block();
                    self.term(Term::Branch(Operand::Local(eqb), step, changed));
                    self.switch(changed);
                    self.emit(Stmt::Assign(prev, Rvalue::Use(value)));
                    self.term(Term::Goto(passing));
                    self.switch(passing);
                }
            }
        }
        match plan.terminal {
            Terminal::Any(_) | Terminal::All(_) => {
                let acc = acc.unwrap();
                let decided = self.call(Callee::Closure(search?), vec![value], Repr::I8);
                // `any` stops on a match, `all` on a miss.
                let hit = if matches!(plan.terminal, Terminal::Any(_)) {
                    decided
                } else {
                    self.assign(Repr::I8, Rvalue::Not(decided))
                };
                let done = self.block();
                self.term(Term::Branch(hit, done, step));
                self.switch(done);
                let outcome = self.bool_op(matches!(plan.terminal, Terminal::Any(_)));
                self.emit(Stmt::Assign(acc, Rvalue::Use(outcome)));
                self.term(Term::Goto(exit));
            }
            Terminal::FindMap(_) => {
                // the first `some`: the callback's own `Option` is the result
                let acc = acc.unwrap();
                let opt = self.call(Callee::Closure(search?), vec![value], Repr::Ptr);
                let ol = self.copy_to_local(opt, Repr::Ptr, None);
                let tag = self.assign(Repr::I32, Rvalue::Tag(Operand::Local(ol)));
                let done = self.block();
                self.term(Term::Switch(tag, vec![(1, done)], step));
                self.switch(done);
                self.emit(Stmt::Assign(acc, Rvalue::Use(Operand::Local(ol))));
                self.term(Term::Goto(exit));
            }
            Terminal::Find(_) | Terminal::First | Terminal::Position(_) => {
                let acc = acc.unwrap();
                // the position is taken before it is advanced, so it is this
                // item's; the counter advances even when the item is refused.
                let payload = match plan.terminal {
                    Terminal::Position(_) => {
                        // snapshot before advancing: the local is mutated
                        let idx = Operand::Local(self.copy_to_local(
                            Operand::Local(pos.unwrap()),
                            Repr::I64,
                            None,
                        ));
                        let inc = self.assign(
                            Repr::I64,
                            Rvalue::Bin {
                                op: BinOp::Add,
                                a: Operand::Local(pos.unwrap()),
                                b: int(1),
                                ty: NumTy::I(64, false),
                                loc: 0,
                            },
                        );
                        self.emit(Stmt::Assign(pos.unwrap(), Rvalue::Use(inc)));
                        idx
                    }
                    _ => value,
                };
                let done = self.block();
                match plan.terminal {
                    // `first` takes whatever reaches it
                    Terminal::First => self.term(Term::Goto(done)),
                    _ => {
                        let accepted = self.call(Callee::Closure(search?), vec![value], Repr::I8);
                        self.term(Term::Branch(accepted, done, step));
                    }
                }
                self.switch(done);
                let opt = self.ty(e);
                let found = self.alloc_enum(&opt, 1, vec![payload]);
                self.emit(Stmt::Assign(acc, Rvalue::Use(found)));
                self.term(Term::Goto(exit));
            }
            Terminal::For(pat, expr) => {
                let ty = self.pat_ty(pat);
                self.bind_irrefutable(pat, value, &ty);
                self.loops.push(LoopCtx { expr: e, cont: step, brk: exit, result: None, close: None });
                if self.lower(expr).is_some() {
                    self.term(Term::Goto(step));
                }
                self.loops.pop();
            }
            Terminal::Collect => {
                let (builder, elem) = collect.as_ref()?;
                let unit = badger_typeck::ty::unit();
                self.intrinsic(
                    "list_builder_append",
                    vec![elem.clone()],
                    vec![Operand::Local(*builder), value],
                    &unit,
                    self.span(e),
                )?;
                self.term(Term::Goto(step));
            }
            Terminal::Accumulate(combine) => {
                let acc = acc.unwrap();
                let loc = self.m.loc(self.span(e));
                let (op, rhs) = match combine {
                    Combine::Add => (BinOp::Add, value),
                    Combine::Count => (BinOp::Add, int(1)),
                    Combine::Mul => (BinOp::Mul, value),
                };
                let next = self.assign(
                    Repr::I64,
                    Rvalue::Bin { op, a: Operand::Local(acc), b: rhs, ty: NumTy::I(64, true), loc },
                );
                self.emit(Stmt::Assign(acc, Rvalue::Use(next)));
                self.term(Term::Goto(step));
            }
            _ => {
                // what remains is a user `fold`'s callback
                let acc = acc.unwrap();
                let f = fold.unwrap_or_else(|| unreachable!());
                let next = self.call(Callee::Closure(f), vec![Operand::Local(acc), value], ret);
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
        if let Some((builder, elem)) = collect {
            let list_ty = self.ty(e);
            return self.intrinsic(
                "list_finish",
                vec![elem],
                vec![Operand::Local(builder)],
                &list_ty,
                self.span(e),
            );
        }
        Some(acc.map_or_else(unit_op, Operand::Local))
    }
}
