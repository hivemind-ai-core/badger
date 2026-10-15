//! Deferred constraints: protocol obligations, method and associated
//! function resolution, operators, row updates, iteration/indexing, and `?`
//! error conversions.

use lang_base::{Diagnostic, Span, Symbol};
use lang_hir::{BinOp, DefId, ExprId};

use crate::env::*;
use crate::infer::*;
use crate::ty::*;

#[derive(Debug, Clone)]
pub enum Constraint {
    Impl {
        proto: DefId,
        args: Vec<Ty>,
        ty: Ty,
        span: Span,
        expr: Option<ExprId>,
    },
    Display {
        ty: Ty,
        span: Span,
        expr: Option<ExprId>,
    },
    Method {
        recv: Ty,
        name: Symbol,
        fn_ty: Ty,
        explicit: Vec<Ty>,
        span: Span,
        expr: Option<ExprId>,
    },
    Assoc {
        self_ty: Ty,
        name: Symbol,
        fn_ty: Ty,
        explicit: Vec<Ty>,
        span: Span,
        expr: Option<ExprId>,
    },
    Arith {
        op: BinOp,
        lhs: Ty,
        rhs: Ty,
        out: Ty,
        span: Span,
        expr: Option<ExprId>,
    },
    Compare {
        ty: Ty,
        ordered: bool,
        span: Span,
        expr: Option<ExprId>,
    },
    Numeric {
        ty: Ty,
        span: Span,
        signed: bool,
    },
    IntOnly {
        ty: Ty,
        span: Span,
    },
    IntLit {
        ty: Ty,
        value: u128,
        span: Span,
    },
    FloatLit {
        ty: Ty,
        span: Span,
    },
    RowRemove {
        out: Ty,
        base: Ty,
        labels: Vec<Symbol>,
        span: Span,
    },
    ErrConv {
        from: Ty,
        to: Ty,
        span: Span,
        expr: Option<ExprId>,
    },
    Iter {
        coll: Ty,
        item: Ty,
        span: Span,
        expr: Option<ExprId>,
    },
    Index {
        coll: Ty,
        index: Ty,
        out: Ty,
        span: Span,
        expr: Option<ExprId>,
    },
    MustUse {
        ty: Ty,
        span: Span,
    },
    /// `out = right ∪ (left \ right)` for closed record rows.
    Merge {
        left: Ty,
        right: Ty,
        out: Ty,
        span: Span,
    },
}

impl Constraint {
    /// Instantiate a scheme constraint at a use site.
    pub fn subst(&self, args: &[Ty], span: Span) -> Constraint {
        let s = |t: &Ty| subst_gen(t, args);
        let sv = |ts: &[Ty]| ts.iter().map(|t| subst_gen(t, args)).collect::<Vec<_>>();
        match self {
            Constraint::Impl { proto, args: a, ty, .. } => {
                Constraint::Impl { proto: *proto, args: sv(a), ty: s(ty), span, expr: None }
            }
            Constraint::Display { ty, .. } => Constraint::Display { ty: s(ty), span, expr: None },
            Constraint::Method { recv, name, fn_ty, explicit, .. } => Constraint::Method {
                recv: s(recv),
                name: *name,
                fn_ty: s(fn_ty),
                explicit: sv(explicit),
                span,
                expr: None,
            },
            Constraint::Assoc { self_ty, name, fn_ty, explicit, .. } => Constraint::Assoc {
                self_ty: s(self_ty),
                name: *name,
                fn_ty: s(fn_ty),
                explicit: sv(explicit),
                span,
                expr: None,
            },
            Constraint::Arith { op, lhs, rhs, out, .. } => {
                Constraint::Arith { op: *op, lhs: s(lhs), rhs: s(rhs), out: s(out), span, expr: None }
            }
            Constraint::Compare { ty, ordered, .. } => {
                Constraint::Compare { ty: s(ty), ordered: *ordered, span, expr: None }
            }
            Constraint::Numeric { ty, signed, .. } => {
                Constraint::Numeric { ty: s(ty), span, signed: *signed }
            }
            Constraint::IntOnly { ty, .. } => Constraint::IntOnly { ty: s(ty), span },
            Constraint::IntLit { ty, value, .. } => Constraint::IntLit { ty: s(ty), value: *value, span },
            Constraint::FloatLit { ty, .. } => Constraint::FloatLit { ty: s(ty), span },
            Constraint::RowRemove { out, base, labels, .. } => {
                Constraint::RowRemove { out: s(out), base: s(base), labels: labels.clone(), span }
            }
            Constraint::ErrConv { from, to, .. } => {
                Constraint::ErrConv { from: s(from), to: s(to), span, expr: None }
            }
            Constraint::Iter { coll, item, .. } => {
                Constraint::Iter { coll: s(coll), item: s(item), span, expr: None }
            }
            Constraint::Index { coll, index, out, .. } => {
                Constraint::Index { coll: s(coll), index: s(index), out: s(out), span, expr: None }
            }
            Constraint::MustUse { ty, .. } => Constraint::MustUse { ty: s(ty), span },
            Constraint::Merge { left, right, out, .. } => {
                Constraint::Merge { left: s(left), right: s(right), out: s(out), span }
            }
        }
    }

    /// All types mentioned by this constraint.
    pub fn types(&self) -> Vec<Ty> {
        match self {
            Constraint::Impl { args, ty, .. } => {
                let mut v = args.clone();
                v.push(ty.clone());
                v
            }
            Constraint::Display { ty, .. }
            | Constraint::Compare { ty, .. }
            | Constraint::Numeric { ty, .. }
            | Constraint::IntOnly { ty, .. }
            | Constraint::IntLit { ty, .. }
            | Constraint::FloatLit { ty, .. }
            | Constraint::MustUse { ty, .. } => vec![ty.clone()],
            Constraint::Method { recv, fn_ty, explicit, .. } => {
                let mut v = vec![recv.clone(), fn_ty.clone()];
                v.extend(explicit.iter().cloned());
                v
            }
            Constraint::Assoc { self_ty, fn_ty, explicit, .. } => {
                let mut v = vec![self_ty.clone(), fn_ty.clone()];
                v.extend(explicit.iter().cloned());
                v
            }
            Constraint::Arith { lhs, rhs, out, .. } => vec![lhs.clone(), rhs.clone(), out.clone()],
            Constraint::RowRemove { out, base, .. } => vec![out.clone(), base.clone()],
            Constraint::ErrConv { from, to, .. } => vec![from.clone(), to.clone()],
            Constraint::Iter { coll, item, .. } => vec![coll.clone(), item.clone()],
            Constraint::Index { coll, index, out, .. } => vec![coll.clone(), index.clone(), out.clone()],
            Constraint::Merge { left, right, out, .. } => vec![left.clone(), right.clone(), out.clone()],
        }
    }

    pub fn map_types(&self, f: &mut impl FnMut(&Ty) -> Ty) -> Constraint {
        let mut c = self.clone();
        match &mut c {
            Constraint::Impl { args, ty, .. } => {
                *args = args.iter().map(&mut *f).collect();
                *ty = f(ty);
            }
            Constraint::Display { ty, .. }
            | Constraint::Compare { ty, .. }
            | Constraint::Numeric { ty, .. }
            | Constraint::IntOnly { ty, .. }
            | Constraint::IntLit { ty, .. }
            | Constraint::FloatLit { ty, .. }
            | Constraint::MustUse { ty, .. } => *ty = f(ty),
            Constraint::Method { recv, fn_ty, explicit, .. } => {
                *recv = f(recv);
                *fn_ty = f(fn_ty);
                *explicit = explicit.iter().map(&mut *f).collect();
            }
            Constraint::Assoc { self_ty, fn_ty, explicit, .. } => {
                *self_ty = f(self_ty);
                *fn_ty = f(fn_ty);
                *explicit = explicit.iter().map(&mut *f).collect();
            }
            Constraint::Arith { lhs, rhs, out, .. } => {
                *lhs = f(lhs);
                *rhs = f(rhs);
                *out = f(out);
            }
            Constraint::RowRemove { out, base, .. } => {
                *out = f(out);
                *base = f(base);
            }
            Constraint::ErrConv { from, to, .. } => {
                *from = f(from);
                *to = f(to);
            }
            Constraint::Iter { coll, item, .. } => {
                *coll = f(coll);
                *item = f(item);
            }
            Constraint::Index { coll, index, out, .. } => {
                *coll = f(coll);
                *index = f(index);
                *out = f(out);
            }
            Constraint::Merge { left, right, out, .. } => {
                *left = f(left);
                *right = f(right);
                *out = f(out);
            }
        }
        c
    }
}

pub enum Step {
    Solved,
    Pending,
}

pub enum Lookup {
    Found(Target, Ty),
    NotFound,
    Unknown,
}

enum ImplSearch {
    Found(Option<Target>),
    None,
    Pending,
}

impl Infer<'_, '_> {
    /// Run the solver until no constraint makes progress (no defaulting).
    pub fn solve_some(&mut self) {
        self.solve(false);
    }

    pub fn solve(&mut self, final_: bool) {
        if final_ {
            // settle what can be inferred, then default literals first so
            // later final steps see their types
            self.solve(false);
            let (lits, others): (Vec<_>, Vec<_>) = std::mem::take(&mut self.pending)
                .into_iter()
                .partition(|c| matches!(c, Constraint::IntLit { .. } | Constraint::FloatLit { .. }));
            self.pending = others;
            self.converge_from_targets();
            for c in lits {
                if let Step::Pending = self.step(&c, true) {
                    self.pending.push(c);
                }
            }
            self.solve(false);
        }
        loop {
            let mut progress = false;
            // projections whose base type was unknown when first unified
            let deferred = std::mem::take(&mut self.icx.deferred);
            let nd = deferred.len();
            for (a, b, span) in deferred {
                self.icx.span_hint = span;
                if let Err(e) = self.icx.unify(&a, &b) {
                    self.report_unify(e, span, &a, &b);
                }
            }
            if self.icx.deferred.len() < nd {
                progress = true;
            }
            let pending = std::mem::take(&mut self.pending);
            let n = pending.len();
            for c in pending {
                match self.step(&c, final_) {
                    Step::Solved => progress = true,
                    Step::Pending => self.pending.push(c),
                }
            }
            if !progress || self.pending.is_empty() || self.pending.len() > n * 4 + 64 {
                break;
            }
        }
    }

    /// Finish constraints whose variables are private to the given types'
    /// bodies: they cannot be refined by any later use, so literals default
    /// and ambiguities are decided now instead of leaking into a scheme.
    pub fn solve_internal(&mut self, visible: &[Ty]) {
        let mut seen: rustc_hash::FxHashSet<TyVid> = Default::default();
        // module values still being inferred are visible to every function
        let globals: Vec<Ty> = self.g.pending_schemes.borrow().values().map(|s| s.ty.clone()).collect();
        for t in visible.iter().chain(globals.iter()) {
            let mut fv = vec![];
            let r = strip_effects(&self.icx.resolve(t));
            self.icx.free_vars(&r, &mut fv);
            seen.extend(fv.into_iter().map(|v| self.icx.root(v)));
        }
        let vars_of = |inf: &mut Self, c: &Constraint| -> Vec<TyVid> {
            let mut out = vec![];
            for t in c.types() {
                let mut fv = vec![];
                let r = strip_effects(&inf.icx.resolve(&t));
                inf.icx.free_vars(&r, &mut fv);
                out.extend(fv.into_iter().map(|v| inf.icx.root(v)));
            }
            out
        };
        let cvars: Vec<Vec<TyVid>> = self.pending.clone().iter().map(|c| vars_of(self, c)).collect();
        // variables connected to visible ones through pending constraints
        loop {
            let mut grew = false;
            for vs in &cvars {
                if vs.iter().any(|v| seen.contains(v)) {
                    for v in vs {
                        grew |= seen.insert(*v);
                    }
                }
            }
            if !grew {
                break;
            }
        }
        let pending = std::mem::take(&mut self.pending);
        let (internal, rest): (Vec<_>, Vec<_>) = pending
            .into_iter()
            .zip(cvars)
            .partition(|(_, vs)| !vs.is_empty() && vs.iter().all(|v| !seen.contains(v)));
        self.pending = rest.into_iter().map(|(c, _)| c).collect();
        let outer = std::mem::replace(&mut self.pending, internal.into_iter().map(|(c, _)| c).collect());
        self.solve(true);
        let left = std::mem::replace(&mut self.pending, outer);
        self.pending.extend(left);
    }

    /// Default unresolved error-conversion targets when exactly one source
    /// error type flows into them (`?` in private fallible functions).
    pub fn default_err_convs(&mut self) {
        let convs: Vec<(Ty, Ty, Span)> = self
            .pending
            .iter()
            .filter_map(|c| match c {
                Constraint::ErrConv { from, to, span, .. } => Some((from.clone(), to.clone(), *span)),
                _ => None,
            })
            .collect();
        let mut done: Vec<TyVid> = vec![];
        for (_, to, span) in &convs {
            let t = self.icx.shallow(to);
            let TyKind::Var(v) = &*t else { continue };
            let v = self.icx.root(*v);
            if done.contains(&v) {
                continue;
            }
            done.push(v);
            let mut sources: Vec<Ty> = vec![];
            for (f, t2, _) in &convs {
                let t2 = self.icx.shallow(t2);
                if matches!(&*t2, TyKind::Var(w) if self.icx.root(*w) == v) {
                    let f = self.icx.resolve(f);
                    if !matches!(&*f, TyKind::Var(_)) && !sources.contains(&f) {
                        sources.push(f);
                    }
                }
            }
            match sources.len() {
                0 => {}
                1 => {
                    let _ = self.icx.unify(&t, &sources[0]);
                }
                _ => {
                    let names: Vec<String> = sources.iter().map(|s| self.show(s)).collect();
                    let d = Diagnostic::error("cannot infer a single error type for `?`")
                        .with_code("E0325")
                        .with_label(*span, format!("errors of types {} flow here", names.join(", ")))
                        .with_note("annotate the function's result type, e.g. `-> Result<T, AppError>`, or map errors explicitly");
                    self.push_diag(d);
                    let _ = self.icx.unify(&t, &error_ty());
                }
            }
        }
        self.solve_some();
    }

    /// An undetermined target of several `From` sources (as in `try_join`):
    /// choose a source every other source converts into.
    fn converge_from_targets(&mut self) {
        let from = self.b().from;
        let mut groups: Vec<(TyVid, Ty, Vec<Ty>)> = vec![];
        for c in self.pending.clone() {
            if let Constraint::Impl { proto, args, ty, .. } = &c
                && *proto == from
                && args.len() == 1
                && let TyKind::Var(v) = &*self.deref(ty)
            {
                let root = self.icx.root(*v);
                match groups.iter_mut().find(|g| g.0 == root) {
                    Some(g) => g.2.push(args[0].clone()),
                    None => groups.push((root, ty.clone(), vec![args[0].clone()])),
                }
            }
        }
        for (_, target, sources) in groups {
            if sources.len() < 2 {
                continue;
            }
            for cand in &sources {
                let cand = self.deref(cand);
                if matches!(&*cand, TyKind::Var(_)) {
                    continue;
                }
                let others: Vec<Ty> = sources.iter().map(|x| self.deref(x)).filter(|x| *x != cand).collect();
                if others.iter().all(|o| {
                    matches!(self.find_impl(from, std::slice::from_ref(o), &cand, true), ImplSearch::Found(_))
                }) {
                    let _ = self.icx.unify(&target, &cand);
                    break;
                }
            }
        }
    }

    /// Whether a numeric literal or arithmetic constraint still mentions `v`.
    fn numeric_pending(&mut self, v: TyVid) -> bool {
        let root = self.icx.root(v);
        let pending = self.pending.clone();
        pending.iter().any(|c| {
            let tys: Vec<Ty> = match c {
                Constraint::IntLit { ty, .. }
                | Constraint::FloatLit { ty, .. }
                | Constraint::Numeric { ty, .. } => vec![ty.clone()],
                Constraint::Arith { lhs, rhs, out, .. } => vec![lhs.clone(), rhs.clone(), out.clone()],
                _ => vec![],
            };
            tys.iter().any(|t| matches!(&*self.deref(t), TyKind::Var(w) if self.icx.root(*w) == root))
        })
    }

    fn step(&mut self, c: &Constraint, final_: bool) -> Step {
        match c {
            Constraint::IntLit { ty, value, span } => {
                let t = self.deref(ty);
                match &*t {
                    TyKind::Var(_) => {
                        if final_ {
                            let i = self.int_ty();
                            let _ = self.icx.unify(&t, &i);
                            self.check_int_range(&i, *value, *span);
                            return Step::Solved;
                        }
                        Step::Pending
                    }
                    TyKind::Con(d, _) if self.b().is_numeric(*d) => {
                        self.check_int_range(&t, *value, *span);
                        Step::Solved
                    }
                    TyKind::Error => Step::Solved,
                    _ => {
                        let s = self.show(&t);
                        self.push_diag(
                            Diagnostic::error(format!("integer literal used as `{s}`"))
                                .with_code("E0326")
                                .with_label(*span, "expected a numeric type")
                                .with_shapes(s.clone(), "integer literal"),
                        );
                        Step::Solved
                    }
                }
            }
            Constraint::FloatLit { ty, span } => {
                let t = self.deref(ty);
                match &*t {
                    TyKind::Var(_) => {
                        if final_ {
                            let f = self.prim(self.b().f64_);
                            let _ = self.icx.unify(&t, &f);
                            return Step::Solved;
                        }
                        Step::Pending
                    }
                    TyKind::Con(d, _) if self.b().is_float(*d) => Step::Solved,
                    TyKind::Error => Step::Solved,
                    _ => {
                        let s = self.show(&t);
                        self.push_diag(
                            Diagnostic::error(format!("decimal literal used as `{s}`"))
                                .with_code("E0326")
                                .with_label(*span, "expected F32 or F64")
                                .with_shapes(s.clone(), "decimal literal"),
                        );
                        Step::Solved
                    }
                }
            }
            Constraint::Numeric { ty, span, signed } => {
                let t = self.deref(ty);
                match &*t {
                    TyKind::Var(v) => {
                        if final_ {
                            // a literal fixes the type (its default is `Int`);
                            // an operator alone does not choose one
                            let v = self.icx.root(*v);
                            if !self.literal_fixes(v) {
                                self.error(
                                    "E0332",
                                    *span,
                                    "cannot infer the numeric type of this operation",
                                    "add a type annotation",
                                );
                            }
                            let i = self.int_ty();
                            let _ = self.icx.unify(&t, &i);
                            return Step::Solved;
                        }
                        Step::Pending
                    }
                    TyKind::Con(d, _) if self.b().is_numeric(*d) => {
                        if *signed && self.is_unsigned(*d) {
                            let s = self.show(&t);
                            self.error(
                                "E0327",
                                *span,
                                format!("cannot negate unsigned `{s}`"),
                                "unsigned value",
                            );
                        }
                        Step::Solved
                    }
                    TyKind::Con(d, _) if *signed && *d == self.b().duration => {
                        // durations are nonnegative literal units
                        self.push_diag(
                            Diagnostic::error("cannot negate a `Duration`")
                                .with_code("E0327")
                                .with_label(*span, "durations are not negative")
                                .with_note("use explicit signed arithmetic, e.g. on `time.as_nanos(d)`"),
                        );
                        Step::Solved
                    }
                    TyKind::Error => Step::Solved,
                    _ => {
                        let s = self.show(&t);
                        self.error(
                            "E0328",
                            *span,
                            format!("`{s}` is not a numeric type"),
                            "numeric operation",
                        );
                        Step::Solved
                    }
                }
            }
            Constraint::IntOnly { ty, span } => {
                let t = self.deref(ty);
                match &*t {
                    TyKind::Var(_) if !final_ => Step::Pending,
                    TyKind::Var(_) => {
                        let i = self.int_ty();
                        let _ = self.icx.unify(&t, &i);
                        Step::Solved
                    }
                    TyKind::Con(d, _) if self.b().is_int(*d) => Step::Solved,
                    TyKind::Error => Step::Solved,
                    _ => {
                        let s = self.show(&t);
                        self.error(
                            "E0328",
                            *span,
                            format!("`{s}` is not an integer type"),
                            "integer operation",
                        );
                        Step::Solved
                    }
                }
            }
            Constraint::Arith { op, lhs, rhs, out, span, expr } => {
                self.step_arith(*op, lhs, rhs, out, *span, *expr, final_)
            }
            Constraint::Compare { ty, ordered, span, expr } => {
                let t = self.deref(ty);
                if let TyKind::Var(_) = &*t {
                    if final_ {
                        return Step::Solved;
                    }
                    return Step::Pending;
                }
                if *ordered {
                    let ok = match &*t {
                        TyKind::Con(d, _) => {
                            self.b().is_numeric(*d)
                                || [
                                    self.b().char_,
                                    self.b().string,
                                    self.b().duration,
                                    self.b().instant,
                                    self.b().bool_,
                                ]
                                .contains(d)
                        }
                        TyKind::Skolem(_) | TyKind::Error => true,
                        _ => false,
                    };
                    if !ok {
                        let s = self.show(&t);
                        self.error(
                            "E0329",
                            *span,
                            format!("values of type `{s}` cannot be ordered"),
                            "comparison",
                        );
                    }
                } else {
                    match self.eq_ok(&t, final_) {
                        Some(true) => {}
                        Some(false) => {
                            let s = self.show(&t);
                            self.error(
                                "E0330",
                                *span,
                                format!("values of type `{s}` cannot be compared for equality"),
                                "comparison",
                            );
                        }
                        None => return Step::Pending,
                    }
                }
                if let Some(e) = expr {
                    self.record_res(*e, Res::Compare(t));
                }
                Step::Solved
            }
            Constraint::Display { ty, span, expr } => {
                let t = self.deref(ty);
                match &*t {
                    TyKind::Var(v) => {
                        // a number still being inferred (e.g. a module value's
                        // literal arithmetic) is not text
                        if final_ && !self.numeric_pending(*v) {
                            let s = self.string_ty();
                            let _ = self.icx.unify(&t, &s);
                            return self.step(c, false);
                        }
                        Step::Pending
                    }
                    TyKind::Error => Step::Solved,
                    _ => {
                        let display = self.b().display;
                        match self.find_impl(display, &[], &t, final_) {
                            ImplSearch::Found(_) => {
                                let name = self.g.interner.intern("display");
                                if let Lookup::Found(target, _) = self.lookup_method(&t, name, &[], *span)
                                    && let Some(e) = expr
                                {
                                    self.displays.insert(*e, target);
                                }
                                Step::Solved
                            }
                            ImplSearch::Pending => Step::Pending,
                            ImplSearch::None => {
                                let s = self.show(&t);
                                self.push_diag(
                                    Diagnostic::error(format!("`{s}` does not implement `Display`"))
                                        .with_code("E0331")
                                        .with_label(
                                            *span,
                                            "interpolated values need a `Display` implementation",
                                        ),
                                );
                                Step::Solved
                            }
                        }
                    }
                }
            }
            Constraint::Impl { proto, args, ty, span, .. } => {
                let t = self.deref(ty);
                // an undetermined conversion target defaults to its source
                if final_ && *proto == self.b().from && args.len() == 1 && matches!(&*t, TyKind::Var(_)) {
                    let chosen = args[0].clone();
                    let _ = self.icx.unify(&t, &chosen);
                    return Step::Solved;
                }
                match self.find_impl(*proto, args, &t, final_) {
                    ImplSearch::Found(_) => Step::Solved,
                    ImplSearch::Pending => {
                        if final_ {
                            return Step::Solved;
                        }
                        Step::Pending
                    }
                    ImplSearch::None => {
                        self.impl_error(*proto, args, &t, *span);
                        Step::Solved
                    }
                }
            }
            Constraint::Method { recv, name, fn_ty, explicit, span, expr } => {
                let r = self.deref(recv);
                if let TyKind::Record(_) = &*r {
                    let ft = self.field(r.clone(), *name, *span, expr.unwrap_or_else(dummy_expr));
                    let fty = self.icx.shallow(fn_ty);
                    if let TyKind::Fn(f) = &*fty {
                        let rest = mk(TyKind::Fn(FnTy {
                            params: f.params[1..].to_vec(),
                            ret: f.ret.clone(),
                            eff: f.eff.clone(),
                            once: f.once,
                        }));
                        self.expect(*span, &ft, &rest);
                    }
                    if let Some(e) = expr {
                        self.record_res(*e, Res::Method(Target::Field(*name)));
                    }
                    return Step::Solved;
                }
                match self.lookup_method_typed(&r, *name, explicit, *span, Some(fn_ty), final_) {
                    Lookup::Unknown => {
                        if final_ {
                            let m = self.sym(*name).to_string();
                            self.error(
                                "E0332",
                                *span,
                                format!("cannot infer the receiver type of `.{m}()`"),
                                "add a type annotation",
                            );
                            return Step::Solved;
                        }
                        Step::Pending
                    }
                    Lookup::NotFound => {
                        let (m, t) = (self.sym(*name).to_string(), self.show(&r));
                        let rel = self.head_def(&r);
                        self.error_rel(
                            "E0312",
                            *span,
                            format!("no method `{m}` on `{t}`"),
                            "method not found",
                            rel,
                        );
                        Step::Solved
                    }
                    Lookup::Found(target, mty) => {
                        // receiver parameter is auto-borrowed: unify loosely
                        let mty = self.icx.shallow(&mty);
                        let fty = self.icx.shallow(fn_ty);
                        if let (TyKind::Fn(m), TyKind::Fn(f)) = (&*mty, &*fty) {
                            if m.params.len() != f.params.len() {
                                let rel = match &target {
                                    Target::Fn { def, .. } => Some(*def),
                                    _ => None,
                                };
                                self.error_rel(
                                    "E0309",
                                    *span,
                                    format!(
                                        "method takes {} argument(s) but {} were supplied",
                                        m.params.len() - 1,
                                        f.params.len() - 1
                                    ),
                                    "wrong number of arguments",
                                    rel,
                                );
                            } else {
                                for (a, b) in m.params.iter().zip(&f.params) {
                                    self.expect(*span, a, b);
                                }
                                self.expect(*span, &m.ret, &f.ret);
                                let meff = m.eff.clone();
                                let _ = self.icx.unify_rows(&meff, &f.eff);
                            }
                        }
                        if let Some(e) = expr {
                            self.record_res(*e, Res::Method(target));
                        }
                        Step::Solved
                    }
                }
            }
            Constraint::Assoc { self_ty, name, fn_ty, explicit, span, expr } => {
                self.step_assoc(self_ty, *name, fn_ty, explicit, *span, *expr, final_)
            }
            Constraint::RowRemove { out, base, labels, span } => {
                let (fields, tail) = self.icx.flatten_row(base);
                let all_known = labels.iter().all(|l| fields.iter().any(|f| f.0 == *l));
                let tail_open = matches!(&*tail, TyKind::Var(_));
                if !all_known && tail_open && !final_ {
                    return Step::Pending;
                }
                let rest: Vec<(Symbol, Ty)> = fields.into_iter().filter(|f| !labels.contains(&f.0)).collect();
                let r = row(rest, tail);
                if let Err(e) = self.icx.unify(out, &r) {
                    let (o, rr) = (mk(TyKind::Record(out.clone())), mk(TyKind::Record(r.clone())));
                    self.report_unify(e, *span, &o, &rr);
                }
                Step::Solved
            }
            Constraint::ErrConv { from, to, span, expr } => {
                let f = self.icx.shallow(from);
                let t = self.icx.shallow(to);
                let fv = matches!(&*f, TyKind::Var(_));
                let tv = matches!(&*t, TyKind::Var(_));
                if fv || tv {
                    if final_ {
                        let _ = self.icx.unify(&f, &t);
                        if let Some(e) = expr {
                            self.record_res(*e, Res::Try(None));
                        }
                        return Step::Solved;
                    }
                    return Step::Pending;
                }
                if self.icx.try_unify(&f, &t) {
                    if let Some(e) = expr {
                        self.record_res(*e, Res::Try(None));
                    }
                    return Step::Solved;
                }
                let from_proto = self.b().from;
                match self.find_impl(from_proto, std::slice::from_ref(&f), &t, final_) {
                    ImplSearch::Found(target) => {
                        if let Some(e) = expr {
                            self.record_res(*e, Res::Try(target));
                        }
                        Step::Solved
                    }
                    ImplSearch::Pending if !final_ => Step::Pending,
                    _ => {
                        let (fs, ts) = (self.show(&f), self.show(&t));
                        self.push_diag(
                            Diagnostic::error(format!("`?` cannot convert error `{fs}` into `{ts}`"))
                                .with_code("E0333")
                                .with_label(*span, "error type mismatch")
                                .with_note(format!(
                                    "add a variant with a `{fs}` payload to `{ts}` and derive `From`, or map the error explicitly"
                                )),
                        );
                        Step::Solved
                    }
                }
            }
            Constraint::Iter { coll, item, span, expr } => {
                let t = self.deref(coll);
                let (kind, it) = match &*t {
                    TyKind::Var(_) => return Step::Pending,
                    TyKind::Error => return Step::Solved,
                    TyKind::Con(d, args) => {
                        let name = self.lang_name(*d);
                        match name.as_deref() {
                            Some("Range") => (IterKind::Range, args[0].clone()),
                            Some("List") => (IterKind::List, args[0].clone()),
                            Some("Array") => (IterKind::Array, args[0].clone()),
                            Some("Buffer") => (IterKind::Buffer, args[0].clone()),
                            Some("Seq") => (IterKind::Seq, args[0].clone()),
                            Some("Stream") => (IterKind::Stream, args[0].clone()),
                            Some("Set") => (IterKind::Set, args[0].clone()),
                            Some("Map") => {
                                (IterKind::Map, mk(TyKind::Tuple(vec![args[0].clone(), args[1].clone()])))
                            }
                            _ => {
                                let s = self.show(&t);
                                self.error("E0334", *span, format!("`{s}` is not iterable"), "used in `for`");
                                return Step::Solved;
                            }
                        }
                    }
                    _ => {
                        let s = self.show(&t);
                        self.error("E0334", *span, format!("`{s}` is not iterable"), "used in `for`");
                        return Step::Solved;
                    }
                };
                self.expect(*span, item, &it);
                if let Some(e) = expr {
                    self.record_res(*e, Res::Iter(kind));
                }
                Step::Solved
            }
            Constraint::Index { coll, index, out, span, expr } => {
                let t = self.deref(coll);
                match &*t {
                    TyKind::Var(_) => Step::Pending,
                    TyKind::Error => Step::Solved,
                    TyKind::Record(_) => {
                        self.push_diag(
                            Diagnostic::error("a record cannot be indexed by a runtime value")
                                .with_code("E0335")
                                .with_label(*span, "heterogeneous fields have no single result type")
                                .with_note("use a literal symbol label `record[:label]`, or convert to `Data` or a `Map` explicitly"),
                        );
                        Step::Solved
                    }
                    TyKind::Con(d, args) => {
                        let name = self.lang_name(*d);
                        if name.as_deref() == Some("Bytes") {
                            let i = self.int_ty();
                            self.expect(*span, &i, index);
                            let u = self.prim(self.b().u8_);
                            self.expect(*span, out, &u);
                            if let Some(e) = expr {
                                self.record_res(*e, Res::Index(IndexKind::Bytes));
                            }
                            return Step::Solved;
                        }
                        let kind = match name.as_deref() {
                            Some("List") => IndexKind::List,
                            Some("Array") => IndexKind::Array,
                            Some("Buffer") => IndexKind::Buffer,
                            Some("Map") => IndexKind::Map,
                            _ => {
                                let s = self.show(&t);
                                self.error("E0336", *span, format!("`{s}` cannot be indexed"), "indexing");
                                return Step::Solved;
                            }
                        };
                        if let IndexKind::Map = kind {
                            self.expect(*span, &args[0], index);
                            self.expect(*span, out, &args[1]);
                        } else {
                            let i = self.int_ty();
                            self.expect(*span, &i, index);
                            self.expect(*span, out, &args[0]);
                        }
                        if let Some(e) = expr {
                            self.record_res(*e, Res::Index(kind));
                        }
                        Step::Solved
                    }
                    _ => {
                        let s = self.show(&t);
                        self.error("E0336", *span, format!("`{s}` cannot be indexed"), "indexing");
                        Step::Solved
                    }
                }
            }
            Constraint::Merge { left, right, out, span } => {
                let (l, r) = (self.deref(left), self.deref(right));
                let (TyKind::Record(lr), TyKind::Record(rr)) = (&*l, &*r) else {
                    if matches!(&*l, TyKind::Var(_)) || matches!(&*r, TyKind::Var(_)) {
                        return Step::Pending;
                    }
                    self.error("E0343", *span, "`records.merge` combines two records", "expected records");
                    return Step::Solved;
                };
                let (lf, lt) = self.icx.flatten_row(&lr.clone());
                let (rf, rt) = self.icx.flatten_row(&rr.clone());
                let closed = |t: &Ty| matches!(&**t, TyKind::RowEmpty);
                if !closed(&lt) || !closed(&rt) {
                    if final_ {
                        self.error(
                            "E0343",
                            *span,
                            "`records.merge` needs records with known fields",
                            "annotate the record types",
                        );
                        return Step::Solved;
                    }
                    return Step::Pending;
                }
                let mut fields: Vec<(Symbol, Ty)> =
                    lf.into_iter().filter(|(l, _)| !rf.iter().any(|(m, _)| m == l)).collect();
                fields.extend(rf);
                let merged = record(fields, row_empty());
                self.expect(*span, out, &merged);
                Step::Solved
            }
            Constraint::MustUse { ty, span } => {
                let t = self.deref(ty);
                match &*t {
                    TyKind::Var(_) if !final_ => Step::Pending,
                    TyKind::Con(d, _) if *d == self.b().result => {
                        self.push_diag(
                            Diagnostic::error("unused `Result`")
                                .with_code("E0337")
                                .with_label(*span, "this `Result` is ignored")
                                .with_note("handle it with `?` or `match`, or discard it explicitly with `let _ = ...`"),
                        );
                        Step::Solved
                    }
                    TyKind::Con(d, args)
                        if Some(*d) == self.g.hir.try_lang("task.Task") && args.len() == 2 =>
                    {
                        let mut r = self.deref(&args[1]);
                        // a captured task's `Exit<T>` still carries `T`
                        if let TyKind::Con(xd, xa) = &*r.clone()
                            && Some(*xd) == self.g.hir.try_lang("task.Exit")
                            && let Some(inner) = xa.first()
                        {
                            r = self.deref(inner);
                        }
                        match &*r {
                            TyKind::Var(_) if !final_ => Step::Pending,
                            TyKind::Con(rd, _) if *rd == self.b().result => {
                                self.push_diag(
                                    Diagnostic::error("unused task with a `Result`")
                                        .with_code("E0337")
                                        .with_label(*span, "this task's result is never inspected")
                                        .with_note("join the task and handle its `Result`, or discard it explicitly with `let _ = ...`"),
                                );
                                Step::Solved
                            }
                            _ => Step::Solved,
                        }
                    }
                    _ => Step::Solved,
                }
            }
        }
    }

    fn is_unsigned(&self, d: DefId) -> bool {
        self.b().ints[4..].contains(&d)
    }

    fn check_int_range(&mut self, t: &Ty, value: u128, span: Span) {
        let TyKind::Con(d, _) = &**t else { return };
        let Some(i) = self.b().ints.iter().position(|x| x == d) else { return };
        let max: u128 = match i {
            0 => i8::MAX as u128,
            1 => i16::MAX as u128,
            2 => i32::MAX as u128,
            3 => i64::MAX as u128,
            4 => u8::MAX as u128,
            5 => u16::MAX as u128,
            6 => u32::MAX as u128,
            _ => u64::MAX as u128,
        };
        // negative literals are encoded in two's complement (top bit set)
        let neg = value >> 127 == 1;
        let shown = if neg { format!("-{}", (value as i128).wrapping_neg()) } else { value.to_string() };
        let ok = if neg {
            // a signed minimum has magnitude max + 1; unsigned types have none
            i < 4 && (value as i128).wrapping_neg() as u128 <= max + 1
        } else {
            value <= max
        };
        if !ok {
            let s = self.show(t);
            self.error("E0338", span, format!("literal `{shown}` does not fit in `{s}`"), "out of range");
        }
    }

    pub fn lang_name(&self, d: DefId) -> Option<String> {
        let def = &self.g.hir.defs[d];
        let m = &self.g.hir.modules[def.module];
        if !m.is_std {
            return None;
        }
        Some(self.sym(def.name).to_string())
    }

    #[allow(clippy::too_many_arguments)]
    fn step_arith(
        &mut self,
        op: BinOp,
        lhs: &Ty,
        rhs: &Ty,
        out: &Ty,
        span: Span,
        expr: Option<ExprId>,
        final_: bool,
    ) -> Step {
        let _ = expr;
        let l = self.deref(lhs);
        let r = self.deref(rhs);
        let b = self.b().clone();
        let int_only =
            matches!(op, BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr | BinOp::Rem);
        let is_num = |t: &Ty| matches!(&**t, TyKind::Con(d, _) if b.is_numeric(*d));
        match (&*l, &*r) {
            (TyKind::Error, _) | (_, TyKind::Error) => Step::Solved,
            (TyKind::Con(d, _), _) if *d == b.duration || *d == b.instant => {
                let dur = self.prim(b.duration);
                let ins = self.prim(b.instant);
                let int = self.int_ty();
                let res = match (*d == b.instant, op) {
                    (true, BinOp::Sub) => {
                        // Instant - Instant = Duration; Instant - Duration = Instant
                        if let TyKind::Var(_) = &*r
                            && !final_
                        {
                            return Step::Pending;
                        }
                        let rr = self.deref(&r);
                        if matches!(&*rr, TyKind::Con(x, _) if *x == b.duration) {
                            Some(ins.clone())
                        } else {
                            self.expect(span, &ins, &rr);
                            Some(dur.clone())
                        }
                    }
                    (true, BinOp::Add) => {
                        self.expect(span, &dur, &r);
                        Some(ins.clone())
                    }
                    (false, BinOp::Add | BinOp::Sub) => {
                        self.expect(span, &dur, &r);
                        Some(dur.clone())
                    }
                    (false, BinOp::Mul | BinOp::Div) => {
                        self.expect(span, &int, &r);
                        Some(dur.clone())
                    }
                    _ => None,
                };
                match res {
                    Some(t) => self.expect(span, out, &t),
                    None => {
                        let s = self.show(&l);
                        self.error(
                            "E0339",
                            span,
                            format!("operator `{}` is not defined for `{s}`", op.as_str()),
                            "operator",
                        );
                    }
                }
                Step::Solved
            }
            (TyKind::Con(..), _) if is_num(&l) => {
                if int_only && !matches!(&*l, TyKind::Con(d, _) if b.is_int(*d)) {
                    let s = self.show(&l);
                    self.error(
                        "E0339",
                        span,
                        format!("operator `{}` requires integers, found `{s}`", op.as_str()),
                        "operator",
                    );
                }
                self.expect(span, &l, &r);
                self.expect(span, out, &l);
                Step::Solved
            }
            (TyKind::Var(_), TyKind::Con(..)) if is_num(&r) => {
                self.expect(span, &l, &r);
                self.expect(span, out, &r);
                Step::Solved
            }
            // a known numeric result fixes homogeneous operands (literals
            // take the expected type of a call's result)
            (TyKind::Var(_), TyKind::Var(_)) if is_num(&self.deref(out)) => {
                let o = self.deref(out);
                self.expect(span, &o, &l);
                self.expect(span, &o, &r);
                Step::Solved
            }
            (TyKind::Var(lv), TyKind::Var(_) | TyKind::Con(..)) => {
                if final_ {
                    // a literal fixes the type (its default is `Int`); an
                    // operator alone does not choose one
                    let lv = self.icx.root(*lv);
                    let rv = match &*r {
                        TyKind::Var(w) => Some(self.icx.root(*w)),
                        _ => None,
                    };
                    let ov = match &*self.deref(out) {
                        TyKind::Var(w) => Some(self.icx.root(*w)),
                        _ => None,
                    };
                    let fixed = self.literal_fixes(lv)
                        || rv.is_some_and(|v| self.literal_fixes(v))
                        || ov.is_some_and(|v| self.literal_fixes(v));
                    if !fixed {
                        self.error(
                            "E0332",
                            span,
                            format!("cannot infer the numeric type of `{}`", op.as_str()),
                            "add a type annotation",
                        );
                    }
                    let i = self.int_ty();
                    let _ = self.icx.unify(&l, &i);
                    return self.step_arith(op, lhs, rhs, out, span, expr, false);
                }
                Step::Pending
            }
            _ => {
                let s = self.show(&l);
                self.error(
                    "E0339",
                    span,
                    format!("operator `{}` is not defined for `{s}`", op.as_str()),
                    "operator",
                );
                Step::Solved
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn step_assoc(
        &mut self,
        self_ty: &Ty,
        name: Symbol,
        fn_ty: &Ty,
        explicit: &[Ty],
        span: Span,
        expr: Option<ExprId>,
        final_: bool,
    ) -> Step {
        let st = self.deref(self_ty);
        let head = match &*st {
            TyKind::Var(_) => {
                return if final_ {
                    self.error("E0332", span, "cannot infer the type of this associated call", "annotate");
                    Step::Solved
                } else {
                    Step::Pending
                };
            }
            TyKind::Con(d, _) => Some(*d),
            TyKind::Skolem(_) => None,
            TyKind::Error => return Step::Solved,
            _ => None,
        };
        let mut cands: Vec<(Target, Ty)> = vec![];
        if let Some(head) = head {
            let idxs = self.g.env.impls_by_head.get(&head).cloned().unwrap_or_default();
            for i in idxs {
                let imp = self.g.env.impls[i].clone();
                if let Some(c) = self.impl_member(&imp, &st, name, explicit, span) {
                    cands.push(c);
                }
            }
        } else if let TyKind::Skolem(_) = &*st {
            for (proto, pargs, subj) in self.givens.clone() {
                if self.icx.try_unify(&subj, &st)
                    && let Some(c) = self.protocol_member(proto, &pargs, &st, name, explicit)
                {
                    cands.push(c);
                }
            }
        }
        // reflexive `From<T> for T`
        if self.sym(name) == "from" && !matches!(&*st, TyKind::Var(_)) {
            let eff = self.icx.fresh_row();
            let id_ty = mk(TyKind::Fn(FnTy { params: vec![st.clone()], ret: st.clone(), eff, once: false }));
            cands.push((Target::Derived { imp: DerivedImpl::Identity, self_ty: st.clone() }, id_ty));
        }
        // filter by the call-site function type
        let fty = self.icx.shallow(fn_ty);
        let matching: Vec<(Target, Ty)> = if let TyKind::Var(_) = &*fty {
            cands
        } else {
            let mut m = vec![];
            for (t, ty) in cands {
                let snap = self.icx.snapshot();
                let ok = self.icx.unify(&ty, &fty).is_ok();
                self.icx.rollback(snap);
                if ok {
                    m.push((t, ty));
                }
            }
            m
        };
        match matching.len() {
            0 => {
                let (n, t) = (self.sym(name).to_string(), self.show(&st));
                let ft = self.show(&fty);
                self.error(
                    "E0312",
                    span,
                    format!("no associated function `{n}` on `{t}` matching `{ft}`"),
                    "not found",
                );
                Step::Solved
            }
            1 => {
                let (target, ty) = matching.into_iter().next().unwrap();
                self.expect(span, fn_ty, &ty);
                if let Some(e) = expr {
                    self.record_res(e, Res::Method(target));
                }
                Step::Solved
            }
            _ if !final_ && matches!(&*fty, TyKind::Var(_)) => Step::Pending,
            _ if !final_ => Step::Pending,
            _ => {
                let n = self.sym(name).to_string();
                self.error(
                    "E0340",
                    span,
                    format!("ambiguous associated function `{n}`"),
                    "several implementations apply",
                );
                Step::Solved
            }
        }
    }

    // ------------------------------------------------------------ impls

    /// Does `ty` support structural equality? `None` = not yet known.
    /// Ordering support (`<`): numbers (floats partially), characters, text,
    /// durations, instants.
    fn ord_ok(&mut self, t: &Ty, final_: bool) -> Option<bool> {
        let t = self.deref(t);
        match &*t {
            TyKind::Var(_) => {
                if final_ {
                    Some(false)
                } else {
                    None
                }
            }
            TyKind::Error | TyKind::Skolem(_) | TyKind::Gen(_) => Some(true),
            TyKind::Con(d, _) => {
                let name = self.lang_name(*d).unwrap_or_default();
                Some(
                    self.b().is_numeric(*d)
                        || matches!(
                            name.as_str(),
                            "Char" | "String" | "Duration" | "Instant" | "Timestamp" | "Symbol"
                        ),
                )
            }
            _ => Some(false),
        }
    }

    /// No region-parameterized constructor (a scoped resource) inside `t`.
    fn unscoped(&mut self, t: &Ty, final_: bool, depth: usize) -> Option<bool> {
        if depth > 8 {
            return Some(true);
        }
        let t = self.deref(t);
        match &*t {
            TyKind::Var(_) => {
                if final_ {
                    Some(true)
                } else {
                    None
                }
            }
            TyKind::Con(d, args) => {
                if self.g.env.last_param_is_region(self.g.hir, *d) {
                    return Some(false);
                }
                let mut parts: Vec<Ty> = args.clone();
                if self.g.env.structs.contains_key(d) {
                    parts.extend(self.g.env.struct_fields(*d, args).into_iter().map(|f| f.1));
                } else if let Some(e) = self.g.env.enums.get(d) {
                    let n = e.variants.len();
                    for v in 0..n {
                        parts.extend(self.g.env.variant_fields(*d, v as u32, args).into_iter().map(|f| f.1));
                    }
                }
                let mut pending = false;
                for p in parts {
                    match self.unscoped(&p, final_, depth + 1) {
                        Some(false) => return Some(false),
                        None => pending = true,
                        Some(true) => {}
                    }
                }
                if pending { None } else { Some(true) }
            }
            TyKind::Tuple(ts) => {
                for p in ts.clone() {
                    if self.unscoped(&p, final_, depth + 1) == Some(false) {
                        return Some(false);
                    }
                }
                Some(true)
            }
            TyKind::Record(r) => {
                for (_, p) in row_fields(r).0 {
                    if self.unscoped(&p, final_, depth + 1) == Some(false) {
                        return Some(false);
                    }
                }
                Some(true)
            }
            // a closure or `dyn` value may hold a scoped resource invisibly;
            // a `dyn Error` cause is itself an (unscoped) error value
            TyKind::Fn(_) => Some(false),
            TyKind::Dyn(p, _) => Some(self.lang_name(*p).as_deref() == Some("Error")),
            _ => Some(true),
        }
    }

    /// Structural equality support (`==`): `PartialEq`.
    fn eq_ok(&mut self, t: &Ty, final_: bool) -> Option<bool> {
        self.eq_check(t, final_, false)
    }

    fn eq_check(&mut self, t: &Ty, final_: bool, strict: bool) -> Option<bool> {
        let t = self.deref(t);
        match &*t {
            TyKind::Var(_) => {
                if final_ {
                    Some(true)
                } else {
                    None
                }
            }
            TyKind::Error | TyKind::Skolem(_) | TyKind::Gen(_) => Some(true),
            TyKind::Fn(_) => Some(false),
            // floats have IEEE (partial) equality only: no total `Eq`
            TyKind::Con(d, _) if strict && self.b().floats.contains(d) => Some(false),
            TyKind::Tuple(ts) => {
                for x in ts.clone() {
                    match self.eq_check(&x, final_, strict) {
                        Some(true) => {}
                        other => return other,
                    }
                }
                Some(true)
            }
            TyKind::Record(r) => {
                let (fs, _) = self.icx.flatten_row(&r.clone());
                for (_, x) in fs {
                    match self.eq_check(&x, final_, strict) {
                        Some(true) => {}
                        other => return other,
                    }
                }
                Some(true)
            }
            TyKind::Con(d, args) => {
                if *d == self.b().cell || Some(*d) == self.g.hir.try_lang("Atom") {
                    return Some(false);
                }
                if self.g.env.enums.contains_key(d) || self.g.env.structs.contains_key(d) {
                    if self.visiting.contains(d) {
                        return Some(true);
                    }
                    // fields must support equality
                    let fields: Vec<Ty> = if let Some(e) = self.g.env.enums.get(d) {
                        (0..e.variants.len())
                            .flat_map(|v| {
                                self.g.env.variant_fields(*d, v as u32, args).into_iter().map(|f| f.1)
                            })
                            .collect()
                    } else {
                        self.g.env.struct_fields(*d, args).into_iter().map(|f| f.1).collect()
                    };
                    self.visiting.push(*d);
                    let mut result = Some(true);
                    for x in fields {
                        match self.eq_check(&x, final_, strict) {
                            Some(true) => {}
                            other => {
                                result = other;
                                break;
                            }
                        }
                    }
                    self.visiting.pop();
                    return result;
                }
                let name = self.lang_name(*d).unwrap_or_default();
                let data = [
                    "Bool",
                    "I8",
                    "I16",
                    "I32",
                    "I64",
                    "U8",
                    "U16",
                    "U32",
                    "U64",
                    "F32",
                    "F64",
                    "Char",
                    "Symbol",
                    "String",
                    "Bytes",
                    "Duration",
                    "Instant",
                    "Timestamp",
                    "List",
                    "Map",
                    "Set",
                    "Array",
                    "Range",
                ];
                if !data.contains(&name.as_str()) {
                    return Some(false);
                }
                for x in args.clone() {
                    match self.eq_check(&x, final_, strict) {
                        Some(true) => {}
                        other => return other,
                    }
                }
                Some(true)
            }
            _ => Some(true),
        }
    }

    /// Structural capabilities: Hash/Codec (data-shaped) and Transfer.
    fn structural(&mut self, which: &str, t: &Ty, final_: bool) -> Option<bool> {
        let t = self.deref(t);
        match &*t {
            TyKind::Var(_) => {
                if final_ {
                    Some(true)
                } else {
                    None
                }
            }
            TyKind::Error | TyKind::Gen(_) => Some(true),
            TyKind::Skolem(s) => {
                // a generic parameter satisfies a capability through a given bound
                let s = *s;
                let found = self.givens.iter().any(|(p, _, subj)| {
                    matches!(&**subj, TyKind::Skolem(x) if *x == s)
                        && self.lang_name(*p).as_deref() == Some(which)
                });
                // a region brand is shareable with child tasks, never encodable
                let region = self.icx.skolems[s as usize].region;
                Some(found || which == "Transfer" || (region && which != "Codec"))
            }
            TyKind::Fn(f) => {
                if which != "Transfer" && which != "Share" {
                    return Some(false);
                }
                // a `once` closure has one owning call: it is not shareable
                if which == "Share" && f.once {
                    return Some(false);
                }
                let (labels, tail) = self.icx.flatten_row(&f.eff.clone());
                if labels.iter().any(|(l, _)| self.sym(*l) == "%confined") {
                    return Some(false);
                }
                // capture marks may still arrive while the closure is inferred
                if matches!(&*tail, TyKind::Var(_)) && !final_ {
                    return None;
                }
                Some(true)
            }
            TyKind::Tuple(ts) => {
                for x in ts.clone() {
                    match self.structural(which, &x, final_) {
                        Some(true) => {}
                        other => return other,
                    }
                }
                Some(true)
            }
            TyKind::Record(r) => {
                let (fs, _) = self.icx.flatten_row(&r.clone());
                for (_, x) in fs {
                    match self.structural(which, &x, final_) {
                        Some(true) => {}
                        other => return other,
                    }
                }
                Some(true)
            }
            TyKind::Con(d, args) => {
                let d = *d;
                let name = self.lang_name(d).unwrap_or_default();
                if which == "Transfer" || which == "Share" {
                    if d == self.b().cell || Some(d) == self.g.hir.try_lang("sqlite.Tx") {
                        return Some(false);
                    }
                    // affine values move rather than share
                    if which == "Share" && matches!(name.as_str(), "ListBuilder" | "Buffer" | "Batch") {
                        return Some(false);
                    }
                } else {
                    let data = [
                        "Bool",
                        "I8",
                        "I16",
                        "I32",
                        "I64",
                        "U8",
                        "U16",
                        "U32",
                        "U64",
                        "F32",
                        "F64",
                        "Char",
                        "Symbol",
                        "String",
                        "Bytes",
                        "Duration",
                        "Instant",
                        "Timestamp",
                        "List",
                        "Map",
                        "Set",
                        "Array",
                        "Option",
                        "Result",
                        "Data",
                    ];
                    let user = !self.g.hir.modules[self.g.hir.defs[d].module].is_std;
                    if !data.contains(&name.as_str()) && !user {
                        return Some(false);
                    }
                    // a branded type cannot escape its region through data
                    if which == "Codec"
                        && args.iter().any(|a| {
                            self.icx.skolems.iter().enumerate().any(|(i, k)| {
                                k.region
                                    && ty_any(a, &mut |x| matches!(x, TyKind::Skolem(j) if *j as usize == i))
                            })
                        })
                    {
                        return Some(false);
                    }
                    // encoding exposes, and decoding constructs, every field:
                    // private fields keep a struct's codec inside its module
                    if which == "Codec"
                        && let Some(si) = self.g.env.structs.get(&d)
                        && si.module_differs(self.module)
                        && si.fields.iter().any(|f| !f.2)
                    {
                        return Some(false);
                    }
                    if which == "Hash" && (name == "F32" || name == "F64") {
                        return Some(false);
                    }
                }
                if self.visiting.contains(&d) {
                    return Some(true);
                }
                let fields: Vec<Ty> = if let Some(e) = self.g.env.enums.get(&d) {
                    (0..e.variants.len())
                        .flat_map(|v| self.g.env.variant_fields(d, v as u32, args).into_iter().map(|f| f.1))
                        .collect()
                } else if self.g.env.structs.contains_key(&d) {
                    self.g.env.struct_fields(d, args).into_iter().map(|f| f.1).collect()
                } else {
                    args.clone()
                };
                self.visiting.push(d);
                let mut result = Some(true);
                for x in fields {
                    match self.structural(which, &x, final_) {
                        Some(true) => {}
                        other => {
                            result = other;
                            break;
                        }
                    }
                }
                self.visiting.pop();
                result
            }
            _ => Some(true),
        }
    }

    fn find_impl(&mut self, proto: DefId, args: &[Ty], t: &Ty, final_: bool) -> ImplSearch {
        let t = self.deref(t);
        let pname = self.lang_name(proto).unwrap_or_default();
        let builtin = |r: Option<bool>| match r {
            Some(true) => ImplSearch::Found(None),
            Some(false) => ImplSearch::None,
            None => ImplSearch::Pending,
        };
        // the polymorphic-recursion fallback's `Dynamic` values satisfy the
        // capabilities the runtime dispatches through their descriptors (the
        // generic code they come from was checked against the real types)
        if let TyKind::Con(d, _) = &*t
            && self.lang_name(*d).as_deref() == Some("Dynamic")
            && matches!(pname.as_str(), "Eq" | "PartialEq" | "PartialOrd" | "Hash" | "Transfer" | "Share")
        {
            return ImplSearch::Found(None);
        }
        match pname.as_str() {
            "Eq" => return builtin(self.eq_check(&t, final_, true)),
            "PartialEq" => return builtin(self.eq_ok(&t, final_)),
            "PartialOrd" => return builtin(self.ord_ok(&t, final_)),
            "Hash" | "Codec" | "Transfer" | "Share" => {
                // user-declared impls on nominal types also count
                let r = self.structural(&pname.clone(), &t, final_);
                return builtin(r);
            }
            "Extends" | "Projects" => return builtin(self.extends(&t, &args[0], final_)),
            "Unscoped" => return builtin(self.unscoped(&t, final_, 0)),
            // a row variable stands for record fields, never a value type
            "Row" => {
                return match &*t {
                    TyKind::Var(_) if !final_ => ImplSearch::Pending,
                    TyKind::Var(_) | TyKind::RowEmpty | TyKind::RowExt(..) | TyKind::Error => {
                        ImplSearch::Found(None)
                    }
                    TyKind::Skolem(_) => ImplSearch::Found(None),
                    _ => ImplSearch::None,
                };
            }
            "Integer" => {
                return match &*t {
                    TyKind::Var(_) if !final_ => ImplSearch::Pending,
                    TyKind::Con(d, _) if self.b().is_int(*d) => ImplSearch::Found(None),
                    TyKind::Skolem(_) => builtin(Some(
                        self.givens.iter().any(|(p, _, _)| self.lang_name(*p).as_deref() == Some("Integer")),
                    )),
                    TyKind::Var(_) => ImplSearch::Found(None),
                    _ => ImplSearch::None,
                };
            }
            // reflexive conversion
            "From"
                if !matches!(&*t, TyKind::Var(_)) && args.len() == 1 && self.icx.try_unify(&args[0], &t) =>
            {
                return ImplSearch::Found(Some(Target::Derived {
                    imp: DerivedImpl::Identity,
                    self_ty: t.clone(),
                }));
            }
            _ => {}
        }
        match &*t {
            TyKind::Var(_) => ImplSearch::Pending,
            TyKind::Error => ImplSearch::Found(None),
            TyKind::Dyn(p, _) if *p == proto || self.protocol_extends(*p, proto) => ImplSearch::Found(None),
            TyKind::Proj(b, _) if !final_ && matches!(&*self.icx.resolve(b), TyKind::Var(_)) => {
                ImplSearch::Pending
            }
            // rigid: generic parameters and their associated types satisfy
            // protocols through the `where` clauses in scope
            TyKind::Skolem(_) | TyKind::Proj(..) => {
                for (p, pargs, subj) in self.givens.clone() {
                    if p == proto && self.icx.try_unify(&subj, &t) {
                        let mut ok = pargs.len() == args.len();
                        for (a, b) in pargs.iter().zip(args) {
                            ok &= self.icx.try_unify(a, b);
                        }
                        if ok {
                            return ImplSearch::Found(None);
                        }
                    }
                }
                // a protocol is implied by any given that extends it
                for (p, _, subj) in self.givens.clone() {
                    if self.icx.try_unify(&subj, &t)
                        && self.g.env.protocols.get(&p).is_some_and(|i| i.supers.contains(&proto))
                    {
                        return ImplSearch::Found(None);
                    }
                }
                ImplSearch::None
            }
            TyKind::Con(..) | TyKind::Record(_) | TyKind::Tuple(_) => {
                // implementations for the head constructor, plus blanket ones
                let mut idxs = match &*t {
                    TyKind::Con(head, _) => self.g.env.impls_by_head.get(head).cloned().unwrap_or_default(),
                    _ => vec![],
                };
                idxs.extend(self.g.env.structural_impls.iter().copied());
                let mut found: Vec<(usize, Vec<Ty>)> = vec![];
                for i in idxs {
                    let imp = self.g.env.impls[i].clone();
                    let Some((p, pargs)) = &imp.protocol else { continue };
                    if *p != proto {
                        continue;
                    }
                    let snap = self.icx.snapshot();
                    let vars: Vec<Ty> = (0..imp.n_generics).map(|_| self.icx.fresh_matching()).collect();
                    let mut ok = self.icx.unify(&subst_gen(&imp.self_ty, &vars), &t).is_ok();
                    for (a, b) in pargs.iter().zip(args) {
                        ok &= self.icx.unify(&subst_gen(a, &vars), b).is_ok();
                    }
                    self.icx.rollback(snap);
                    if ok {
                        found.push((i, vars));
                    }
                }
                match found.len() {
                    0 => ImplSearch::None,
                    1 => {
                        let (i, _) = found.pop().unwrap();
                        let imp = self.g.env.impls[i].clone();
                        let vars: Vec<Ty> = (0..imp.n_generics).map(|_| self.icx.fresh_matching()).collect();
                        let _ = self.icx.unify(&subst_gen(&imp.self_ty, &vars), &t);
                        if let Some((_, pargs)) = &imp.protocol {
                            for (a, b) in pargs.iter().zip(args) {
                                let _ = self.icx.unify(&subst_gen(a, &vars), b);
                            }
                        }
                        for (p, pa, subj) in &imp.preds {
                            self.pending.push(Constraint::Impl {
                                proto: *p,
                                args: pa.iter().map(|x| subst_gen(x, &vars)).collect(),
                                ty: subst_gen(subj, &vars),
                                span: imp.span,
                                expr: None,
                            });
                        }
                        let target = match &imp.methods {
                            ImplMethods::Derived(d) => {
                                Some(Target::Derived { imp: d.clone(), self_ty: t.clone() })
                            }
                            ImplMethods::Defs(ms) => {
                                // single-method protocols (From) resolve to that method
                                let pinfo = &self.g.env.protocols[&proto];
                                if pinfo.methods.len() == 1 {
                                    let (mname, _) = pinfo.methods.iter().next().unwrap();
                                    ms.get(mname).map(|m| {
                                        let scheme = self.g.scheme(*m).unwrap();
                                        let n = scheme.n as usize;
                                        let mut margs: Vec<Ty> = vars.clone();
                                        while margs.len() < n {
                                            margs.push(self.icx.fresh());
                                        }
                                        Target::Fn { def: *m, args: margs }
                                    })
                                } else {
                                    None
                                }
                            }
                        };
                        ImplSearch::Found(target)
                    }
                    _ => {
                        if final_ {
                            ImplSearch::Found(None)
                        } else {
                            ImplSearch::Pending
                        }
                    }
                }
            }
            _ => ImplSearch::None,
        }
    }

    /// `S: Extends<T>`: every field of record `T` is present in record `S`
    /// with the same type (a typed projection target).
    fn extends(&mut self, s: &Ty, t: &Ty, final_: bool) -> Option<bool> {
        let s = self.deref(s);
        let t = self.deref(t);
        let (TyKind::Record(sr), TyKind::Record(tr)) = (&*s, &*t) else {
            return match (&*s, &*t) {
                (TyKind::Var(_), _) | (_, TyKind::Var(_)) => {
                    if final_ {
                        Some(true)
                    } else {
                        None
                    }
                }
                (TyKind::Skolem(_), _) | (_, TyKind::Skolem(_)) => Some(true),
                _ => Some(self.icx.try_unify(&s, &t)),
            };
        };
        let (sf, stail) = self.icx.flatten_row(&sr.clone());
        let (tf, ttail) = self.icx.flatten_row(&tr.clone());
        let mut missing: Vec<(Symbol, Ty)> = vec![];
        for (l, ty) in &tf {
            match sf.iter().find(|f| f.0 == *l) {
                Some((_, sty)) => {
                    if !self.icx.try_unify(ty, sty) {
                        return Some(false);
                    }
                }
                None => {
                    if matches!(&*stail, TyKind::Var(_)) {
                        if !final_ {
                            return None;
                        }
                        // nothing else determines the open rest: it carries
                        // the established fields (e.g. a stage that always stops)
                        missing.push((*l, ty.clone()));
                        continue;
                    }
                    return Some(false);
                }
            }
        }
        if !missing.is_empty() {
            let rest = self.icx.fresh_row();
            if !self.icx.try_unify(&stail, &row(missing, rest)) {
                return Some(false);
            }
            return Some(true);
        }
        match &*ttail {
            TyKind::Var(_) if !final_ => {
                // the target's remaining fields are not yet known
                if matches!(&*stail, TyKind::RowEmpty) {
                    let rest: Vec<(Symbol, Ty)> =
                        sf.iter().filter(|f| !tf.iter().any(|g| g.0 == f.0)).cloned().collect();
                    let _ = rest;
                }
                None
            }
            TyKind::Var(_) => {
                let _ = self.icx.unify(&ttail, &row_empty());
                Some(true)
            }
            _ => Some(true),
        }
    }

    /// Whether a pending integer or float literal constraint determines `v`.
    fn literal_fixes(&mut self, v: TyVid) -> bool {
        let tys: Vec<Ty> = self
            .pending
            .iter()
            .filter_map(|c| match c {
                Constraint::IntLit { ty, .. } | Constraint::FloatLit { ty, .. } => Some(ty.clone()),
                _ => None,
            })
            .collect();
        tys.iter().any(|t| matches!(&*self.icx.shallow(t), TyKind::Var(w) if self.icx.root(*w) == v))
    }

    fn impl_error(&mut self, proto: DefId, args: &[Ty], t: &Ty, span: Span) {
        let pname =
            self.lang_name(proto).unwrap_or_else(|| self.sym(self.g.hir.defs[proto].name).to_string());
        let ts = self.show(t);
        let d = match pname.as_str() {
            "Transfer" => Diagnostic::error(format!("`{ts}` cannot be transferred to another task"))
                .with_code("E0402")
                .with_label(span, "task-confined value crosses a task boundary")
                .with_note("`Cell`, transactions, and closures capturing them are confined to their owning task"),
            "Share" => Diagnostic::error(format!("`{ts}` does not implement `Share`"))
                .with_code("E0341")
                .with_label(span, "this value would be copied or shared")
                .with_note("shareable values can be copied and used by several tasks; affine values (buffers, builders, leases, `once` closures) and task-confined cells cannot (a `dyn` value, plain `select` sends, and module-wide sharing require `Share`)"),
            "Unscoped" => Diagnostic::error(format!("`{ts}` may carry a scoped resource"))
                .with_code("E0411")
                .with_label(span, "this value may outlive the scope its resource belongs to")
                .with_note("provider errors must not hold pools, transactions, region-bound handles, or closures (which may capture them); report the failure with plain data"),
            "Codec" => Diagnostic::error(format!("`{ts}` cannot be encoded as data"))
                .with_code("E0403")
                .with_label(span, "requires a data-shaped type")
                .with_note("resources, functions, and scoped handles have no `Data` encoding and cannot escape through it"),
            "Projects" => {
                let target = self.show(&args[0]);
                Diagnostic::error(format!("cannot project `{target}` out of `{ts}`"))
                    .with_code("E0301")
                    .with_label(span, format!("`{ts}` lacks a field of `{target}` or has it at another type"))
                    .with_note("a projection copies every field of the target type; supply the missing fields or project a smaller type")
            }
            "Extends" => {
                let target = self.show(&args[0]);
                Diagnostic::error("a later stage changes or removes an established context field")
                    .with_code("E0703")
                    .with_label(span, format!("`{ts}` does not preserve the fields of `{target}` at their types"))
                    .with_note("enter steps may add fields and replace values, but may not change the type of a field visible to an earlier stage")
            }
            _ => {
                let a = if args.is_empty() {
                    String::new()
                } else {
                    let v: Vec<String> = args.iter().map(|x| self.show(x)).collect();
                    format!("<{}>", v.join(", "))
                };
                Diagnostic::error(format!("`{ts}` does not implement `{pname}{a}`"))
                    .with_code("E0341")
                    .with_label(span, "required here")
            }
        };
        self.push_diag(d);
    }

    /// Resolve `recv.name(...)` for a known receiver head.
    pub fn lookup_method(&mut self, recv: &Ty, name: Symbol, explicit: &[Ty], span: Span) -> Lookup {
        self.lookup_method_typed(recv, name, explicit, span, None, true)
    }

    /// Method lookup; when several protocol implementations (e.g.
    /// `Conv<Int>` and `Conv<String>`) provide the method, the call's
    /// function type `fn_ty` selects among them.
    pub fn lookup_method_typed(
        &mut self,
        recv: &Ty,
        name: Symbol,
        explicit: &[Ty],
        span: Span,
        fn_ty: Option<&Ty>,
        final_: bool,
    ) -> Lookup {
        let r = self.deref(recv);
        match &*r {
            TyKind::Var(_) => Lookup::Unknown,
            // nominal types by head; structural ones (`()`, tuples, records)
            // through the structural and blanket implementations
            TyKind::Con(..) | TyKind::Tuple(_) | TyKind::Record(_) => {
                let mut idxs = match &*r {
                    TyKind::Con(head, _) => self.g.env.impls_by_head.get(head).cloned().unwrap_or_default(),
                    _ => vec![],
                };
                // blanket implementations (`impl<T> P for T where ...`)
                idxs.extend(self.g.env.structural_impls.iter().copied());
                // inherent methods first
                for &i in &idxs {
                    let imp = self.g.env.impls[i].clone();
                    if imp.protocol.is_some() {
                        continue;
                    }
                    if let Some((target, ty)) = self.impl_member(&imp, &r, name, explicit, span) {
                        return Lookup::Found(target, ty);
                    }
                }
                // protocol methods: every implementation that fits the call
                let mut fits = vec![];
                for &i in &idxs {
                    let imp = self.g.env.impls[i].clone();
                    if imp.protocol.is_none() {
                        continue;
                    }
                    let (snap, np) = (self.icx.snapshot(), self.pending.len());
                    let ok = match self.impl_member(&imp, &r, name, explicit, span) {
                        Some((_, ty)) => match fn_ty {
                            Some(f) => self.fits_call(&ty, f),
                            None => true,
                        },
                        None => false,
                    };
                    self.icx.rollback(snap);
                    self.pending.truncate(np);
                    if ok {
                        fits.push(i);
                    }
                }
                if fits.len() > 1 && fn_ty.is_some() && !final_ {
                    return Lookup::Unknown;
                }
                if fits.len() > 1 && fn_ty.is_some() {
                    let m = self.sym(name).to_string();
                    self.error(
                        "E0340",
                        span,
                        format!("ambiguous method `{m}`"),
                        "several implementations apply; annotate the expected type",
                    );
                    return Lookup::Found(Target::Field(name), error_ty());
                }
                match fits.first() {
                    Some(&i) => {
                        let imp = self.g.env.impls[i].clone();
                        match self.impl_member(&imp, &r, name, explicit, span) {
                            Some((target, ty)) => Lookup::Found(target, ty),
                            None => Lookup::NotFound,
                        }
                    }
                    None => Lookup::NotFound,
                }
            }
            TyKind::Skolem(_) => {
                for (proto, pargs, subj) in self.givens.clone() {
                    if !self.icx.try_unify(&subj, &r) {
                        continue;
                    }
                    let mut protos = vec![proto];
                    protos.extend(
                        self.g.env.protocols.get(&proto).map(|p| p.supers.clone()).unwrap_or_default(),
                    );
                    for p in protos {
                        let pa = if p == proto { pargs.clone() } else { vec![] };
                        if let Some(c) = self.protocol_member(p, &pa, &r, name, explicit) {
                            return Lookup::Found(c.0, c.1);
                        }
                    }
                }
                Lookup::NotFound
            }
            TyKind::Dyn(p, pargs) => {
                let (p, pargs) = (*p, pargs.clone());
                // the dictionary also carries supertrait methods
                let mut stack = vec![(p, pargs)];
                let mut seen = vec![];
                while let Some((q, qa)) = stack.pop() {
                    if seen.contains(&q) {
                        continue;
                    }
                    seen.push(q);
                    if let Some((_, ty)) = self.protocol_member(q, &qa, &r, name, explicit) {
                        return Lookup::Found(Target::Dyn { protocol: p, method: name }, ty);
                    }
                    let supers = self.g.env.protocols.get(&q).map(|i| i.supers.clone()).unwrap_or_default();
                    stack.extend(supers.into_iter().map(|s| (s, vec![])));
                }
                Lookup::NotFound
            }
            TyKind::Error => Lookup::Found(Target::Field(name), error_ty()),
            _ => Lookup::NotFound,
        }
    }

    /// Does protocol `p` have `q` among its (transitive) supertraits?
    fn protocol_extends(&self, p: DefId, q: DefId) -> bool {
        let mut stack = vec![p];
        let mut seen = vec![];
        while let Some(x) = stack.pop() {
            if seen.contains(&x) {
                continue;
            }
            seen.push(x);
            let supers = self.g.env.protocols.get(&x).map(|i| i.supers.clone()).unwrap_or_default();
            if supers.contains(&q) {
                return true;
            }
            stack.extend(supers);
        }
        false
    }

    /// Could a method of type `m` serve a call typed `f` (receiver first)?
    fn fits_call(&mut self, m: &Ty, f: &Ty) -> bool {
        let (m, f) = (self.icx.shallow(m), self.icx.shallow(f));
        let (TyKind::Fn(m), TyKind::Fn(f)) = (&*m, &*f) else { return true };
        if m.params.len() != f.params.len() {
            return false;
        }
        for (a, b) in m.params.iter().zip(&f.params).skip(1) {
            if !self.icx.try_unify(a, b) {
                return false;
            }
        }
        self.icx.try_unify(&m.ret, &f.ret)
    }

    /// Member `name` of impl `imp` applied to self type `st`.
    fn impl_member(
        &mut self,
        imp: &ImplInfo,
        st: &Ty,
        name: Symbol,
        explicit: &[Ty],
        _span: Span,
    ) -> Option<(Target, Ty)> {
        match &imp.methods {
            ImplMethods::Defs(ms) if !ms.contains_key(&name) => {
                // the protocol's default body, with `Self` = the implementing type
                let (proto, pargs) = imp.protocol.clone()?;
                let pm = *self.g.env.protocols.get(&proto)?.methods.get(&name)?;
                let has_body =
                    matches!(&self.g.hir.defs[pm].kind, lang_hir::DefKind::Fn(f) if f.body.is_some());
                if !has_body {
                    return None;
                }
                let snap = self.icx.snapshot();
                let vars: Vec<Ty> = (0..imp.n_generics).map(|_| self.icx.fresh_matching()).collect();
                if self.icx.unify(&subst_gen(&imp.self_ty, &vars), st).is_err() {
                    self.icx.rollback(snap);
                    return None;
                }
                self.icx.commit(snap);
                let pargs: Vec<Ty> = pargs.iter().map(|a| subst_gen(a, &vars)).collect();
                self.protocol_member(proto, &pargs, st, name, explicit)
            }
            ImplMethods::Defs(ms) => {
                let m = *ms.get(&name)?;
                let scheme = self.g.scheme(m)?;
                let snap = self.icx.snapshot();
                let vars: Vec<Ty> = (0..imp.n_generics).map(|_| self.icx.fresh_matching()).collect();
                if self.icx.unify(&subst_gen(&imp.self_ty, &vars), st).is_err() {
                    self.icx.rollback(snap);
                    return None;
                }
                self.icx.commit(snap);
                // method scheme: impl generics first, then the method's own
                let mut margs = vars;
                for (k, e) in explicit.iter().enumerate() {
                    let _ = k;
                    margs.push(e.clone());
                }
                while (margs.len() as u32) < scheme.n {
                    margs.push(self.icx.fresh());
                }
                let ty = subst_gen(&scheme.ty, &margs);
                for c in &scheme.constraints {
                    self.pending.push(c.subst(&margs, _span));
                }
                // the implementation applies only where its bounds hold
                for (p, pargs, subj) in &imp.preds {
                    let n = imp.n_generics as usize;
                    self.pending.push(Constraint::Impl {
                        proto: *p,
                        args: pargs.iter().map(|a| subst_gen(a, &margs[..n])).collect(),
                        ty: subst_gen(subj, &margs[..n]),
                        span: _span,
                        expr: None,
                    });
                }
                Some((Target::Fn { def: m, args: margs }, open_effects(&mut self.icx, &ty)))
            }
            ImplMethods::Derived(d) => {
                let (proto, pargs) = imp.protocol.clone()?;
                let pinfo = self.g.env.protocols.get(&proto)?;
                let pm = *pinfo.methods.get(&name)?;
                let snap = self.icx.snapshot();
                let vars: Vec<Ty> = (0..imp.n_generics).map(|_| self.icx.fresh_matching()).collect();
                if self.icx.unify(&subst_gen(&imp.self_ty, &vars), st).is_err() {
                    self.icx.rollback(snap);
                    return None;
                }
                self.icx.commit(snap);
                let pargs: Vec<Ty> = pargs.iter().map(|a| subst_gen(a, &vars)).collect();
                let scheme = self.g.scheme(pm)?;
                let mut margs = vec![st.clone()];
                margs.extend(pargs);
                while (margs.len() as u32) < scheme.n {
                    margs.push(self.icx.fresh());
                }
                let ty = subst_gen(&scheme.ty, &margs);
                let ty = open_effects(&mut self.icx, &ty);
                // derived `Error`: the cause is the wrapped error; other
                // methods use the protocol's default bodies
                if proto == self.b().error && self.sym(name) == "cause" {
                    return Some((Target::Derived { imp: DerivedImpl::ErrorCause, self_ty: st.clone() }, ty));
                }
                if matches!(d, DerivedImpl::Marker)
                    && matches!(&self.g.hir.defs[pm].kind, lang_hir::DefKind::Fn(f) if f.body.is_some())
                {
                    return Some((Target::Fn { def: pm, args: margs }, ty));
                }
                Some((Target::Derived { imp: d.clone(), self_ty: st.clone() }, ty))
            }
        }
    }

    /// Protocol method `name` with `Self = st` (generic receivers).
    fn protocol_member(
        &mut self,
        proto: DefId,
        pargs: &[Ty],
        st: &Ty,
        name: Symbol,
        explicit: &[Ty],
    ) -> Option<(Target, Ty)> {
        let pinfo = self.g.env.protocols.get(&proto)?;
        let pm = *pinfo.methods.get(&name)?;
        let scheme = self.g.scheme(pm)?;
        let mut margs = vec![st.clone()];
        margs.extend(pargs.iter().cloned());
        let nproto = 1 + pinfo.n_generics as usize;
        while margs.len() < nproto {
            margs.push(self.icx.fresh());
        }
        margs.extend(explicit.iter().cloned());
        while (margs.len() as u32) < scheme.n {
            margs.push(self.icx.fresh());
        }
        let ty = subst_gen(&scheme.ty, &margs);
        Some((Target::Fn { def: pm, args: margs }, open_effects(&mut self.icx, &ty)))
    }
}

fn dummy_expr() -> ExprId {
    la_arena::Idx::from_raw(u32::MAX.into())
}

/// A type with function effect rows erased: effects never decide which
/// implementation or literal type a constraint selects.
fn strip_effects(t: &Ty) -> Ty {
    crate::ty::ty_map(t, &mut |x| match &**x {
        TyKind::Fn(f) => Some(mk(TyKind::Fn(FnTy {
            params: f.params.iter().map(strip_effects).collect(),
            ret: strip_effects(&f.ret),
            eff: crate::ty::row_empty(),
            once: f.once,
        }))),
        _ => None,
    })
}
