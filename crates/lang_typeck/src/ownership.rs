//! Affine ownership and borrows.
//!
//! Affine values (list builders, buffers, batch leases, `once` closures,
//! closures that consume an affine capture, and aggregates or collections
//! containing them) have one owning use. Passing one by value, storing it,
//! returning it, or capturing it moves it; later uses of the binding are
//! rejected. Taking an affine element or field out of an aggregate consumes
//! the aggregate. Parameters and receivers declared `&T`/`&mut T` borrow
//! instead, and a borrowed binding cannot give its referent away.
//!
//! Borrows bound to a name (`let r = &x` / `&mut x`) are tracked until the
//! name's last use: while one lives, its owner cannot be moved, assigned, or
//! mutably borrowed, and an exclusive borrow excludes every other access.
//! Borrows cannot outlive their owner (returned or escaping their block), and
//! `&mut` needs an assignable place. Branches merge conservatively and loop
//! bodies are checked as if run twice.

use lang_base::{Diagnostic, Span};
use lang_hir::{DefId, DefKind, ExprId, ExprKind, InterpPart, LocalId, PatId, PatKind, Stmt, UnOp};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::infer::{Infer, Res, Target};
use crate::ty::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Move,
    Borrow,
    BorrowMut,
}

type State = FxHashMap<LocalId, Span>;

/// A borrow bound to a local: `holder` borrows `owner`.
#[derive(Clone)]
struct Loan {
    holder: LocalId,
    owner: LocalId,
    mutable: bool,
    start: usize,
    span: Span,
}

/// How a callee takes each parameter.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Param {
    Owned { repeatable_fn: bool },
    Shared,
    Exclusive,
}

struct Own<'i, 'g, 'a> {
    inf: &'i mut Infer<'g, 'a>,
    affine: FxHashMap<LocalId, bool>,
    reported: FxHashSet<Span>,
    /// Locals bound to closures that consume an affine capture.
    consuming: FxHashSet<LocalId>,
    consuming_lambdas: FxHashMap<ExprId, LocalId>,
    /// Source position of each local's last use (borrow liveness).
    last_use: FxHashMap<LocalId, usize>,
    loans: Vec<Loan>,
    params: FxHashSet<LocalId>,
    /// The function's result type (a borrowed result passes references on).
    ret_ty: Option<Ty>,
    /// Borrows carried by a binding's value (`let w = { r: &b }`).
    carriers: FxHashMap<LocalId, Vec<(LocalId, Span)>>,
    /// Generic pass: type variables of the definition count as affine, and
    /// the types of locals used non-affinely are collected.
    generic: bool,
    offending: Vec<Ty>,
    /// Locals read anywhere (assignments are not reads).
    reads: FxHashSet<LocalId>,
    /// Captures each closure borrows rather than moves.
    lambda_borrows: FxHashMap<ExprId, Vec<LocalId>>,
    /// Locals bound directly to a closure expression.
    local_lambdas: FxHashMap<LocalId, ExprId>,
    /// `Result` values bound or assigned and not read since.
    unread: FxHashMap<LocalId, Span>,
}

pub fn check_def(inf: &mut Infer<'_, '_>, d: DefId) {
    let hir = inf.g.hir;
    let (body, params) = match &hir.defs[d].kind {
        DefKind::Fn(f) if !hir.modules[hir.defs[d].module].is_std => {
            (f.body, f.params.iter().map(|p| p.pat).collect())
        }
        DefKind::Global(g) if !hir.modules[hir.defs[d].module].is_std => (Some(g.value), vec![]),
        _ => (None, vec![]),
    };
    let Some(body) = body else { return };
    let ret_ty = match inf.g.scheme(d).map(|s| s.ty) {
        Some(t) => match &*t {
            TyKind::Fn(f) => Some(f.ret.clone()),
            _ => None,
        },
        None => None,
    };
    let param_pats: Vec<PatId> = params.clone();
    let mut o = Own {
        inf,
        affine: FxHashMap::default(),
        reported: FxHashSet::default(),
        consuming: FxHashSet::default(),
        consuming_lambdas: FxHashMap::default(),
        last_use: FxHashMap::default(),
        loans: vec![],
        params: FxHashSet::default(),
        ret_ty: ret_ty.clone(),
        carriers: FxHashMap::default(),
        generic: false,
        offending: vec![],
        reads: FxHashSet::default(),
        lambda_borrows: FxHashMap::default(),
        local_lambdas: FxHashMap::default(),
        unread: FxHashMap::default(),
    };
    for p in params.clone() {
        let mut ls = vec![];
        pat_locals(hir, p, &mut ls);
        o.params.extend(ls);
    }
    o.collect_uses(body);
    let mut st = State::default();
    let m = o.mode_into(o.ret_ty.clone());
    o.expr(body, m, &mut st);
    o.check_escape(body, true);
    o.unused_tasks(body);
    o.check_consuming_result(body);
    let _ = param_pats;
}

/// Parameters of each function whose argument may run in another task: the
/// callee's scheme requires `Transfer` of it (`spawn`, `capture`,
/// `timeout`), or a user function hands the parameter (directly or captured
/// by a closure) to such a parameter.
fn transfer_params(inf: &mut Infer<'_, '_>) -> FxHashMap<DefId, Vec<usize>> {
    let hir = inf.g.hir;
    let transfer = hir.try_lang("Transfer");
    let mut out: FxHashMap<DefId, Vec<usize>> = FxHashMap::default();
    // declared requirements
    let defs: Vec<DefId> = hir.defs.iter().map(|(id, _)| id).collect();
    for d in &defs {
        let Some(s) = inf.g.scheme(*d) else { continue };
        let TyKind::Fn(f) = &*s.ty else { continue };
        let mut idx = vec![];
        for c in &s.constraints {
            if let crate::solve::Constraint::Impl { proto, ty, .. } = c
                && Some(*proto) == transfer
            {
                for (i, p) in f.params.iter().enumerate() {
                    if p == ty && !idx.contains(&i) {
                        idx.push(i);
                    }
                }
            }
        }
        if !idx.is_empty() {
            out.insert(*d, idx);
        }
    }
    // user functions forwarding a function-typed parameter to one of those
    loop {
        let mut changed = false;
        for (id, def) in hir.defs.iter() {
            if hir.modules[def.module].is_std {
                continue;
            }
            let DefKind::Fn(f) = &def.kind else { continue };
            let Some(body) = f.body else { continue };
            let params: Vec<Option<LocalId>> = f
                .params
                .iter()
                .map(|p| match &hir.pats[p.pat].kind {
                    PatKind::Bind(l) => Some(*l),
                    _ => None,
                })
                .collect();
            let mut found: Vec<usize> = vec![];
            crate::program::walk_exprs(hir, body, &mut |e| {
                let (callee_def, args): (Option<DefId>, Vec<ExprId>) = match &hir.exprs[e].kind {
                    ExprKind::Call { callee, args } => match inf.res.get(callee) {
                        Some(Res::Callee { def, .. }) => (Some(*def), args.clone()),
                        _ => (None, vec![]),
                    },
                    ExprKind::MethodCall { recv, args, .. } => match inf.res.get(&e) {
                        Some(Res::Method(Target::Fn { def, .. })) => {
                            (Some(*def), std::iter::once(*recv).chain(args.iter().copied()).collect())
                        }
                        _ => (None, vec![]),
                    },
                    _ => (None, vec![]),
                };
                let Some(cd) = callee_def else { return };
                let Some(tp) = out.get(&cd) else { return };
                for &i in tp {
                    let Some(&a) = args.get(i) else { continue };
                    let reached: Vec<LocalId> = match &hir.exprs[a].kind {
                        ExprKind::Local(l) => vec![*l],
                        ExprKind::Lambda(lam) => lam.captures.clone(),
                        _ => vec![],
                    };
                    for l in reached {
                        if let Some(pi) = params.iter().position(|p| *p == Some(l))
                            && !found.contains(&pi)
                        {
                            found.push(pi);
                        }
                    }
                }
            });
            if !found.is_empty() {
                let e = out.entry(id).or_default();
                for i in found {
                    if !e.contains(&i) {
                        e.push(i);
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    out
}

/// A closure carrying task-confined state cannot be passed where it may run
/// in another task (through any function that spawns with it).
pub fn check_task_params(inf: &mut Infer<'_, '_>) {
    let hir = inf.g.hir;
    let tp = transfer_params(inf);
    let mut reports: Vec<(Span, String)> = vec![];
    for (_, def) in hir.defs.iter() {
        if hir.modules[def.module].is_std {
            continue;
        }
        let body = match &def.kind {
            DefKind::Fn(f) => f.body,
            DefKind::Global(g) => Some(g.value),
            _ => None,
        };
        let Some(body) = body else { continue };
        crate::program::walk_exprs(hir, body, &mut |e| {
            let (cd, args): (Option<DefId>, Vec<ExprId>) = match &hir.exprs[e].kind {
                ExprKind::Call { callee, args } => match inf.res.get(callee) {
                    Some(Res::Callee { def, .. }) => (Some(*def), args.clone()),
                    _ => (None, vec![]),
                },
                _ => (None, vec![]),
            };
            let Some(cd) = cd else { return };
            // direct std task functions are checked by their own constraints
            if hir.modules[hir.defs[cd].module].is_std {
                return;
            }
            let Some(idx) = tp.get(&cd) else { return };
            for &i in idx {
                let Some(&a) = args.get(i) else { continue };
                let Some(t) = inf.exprs.get(&a).cloned() else { continue };
                let t = inf.icx.resolve(&t);
                let confined = crate::ty::ty_any(&t, &mut |k| match k {
                    TyKind::Fn(f) => {
                        row_fields(&f.eff).0.iter().any(|(l, _)| inf.g.interner.resolve(*l) == "%confined")
                    }
                    _ => false,
                });
                if confined {
                    let name = inf.g.interner.resolve(hir.defs[cd].name).to_string();
                    reports.push((hir.exprs[a].span, name));
                }
            }
        });
    }
    for (span, name) in reports {
        inf.push_diag(
            Diagnostic::error(format!("`{name}` may run this closure in another task"))
                .with_code("E0402")
                .with_label(span, "this closure captures task-confined state (a `Cell` or a transaction)")
                .with_note("the function hands this argument to a spawned task; capture shareable state such as an `Atom` instead"),
        );
    }
}

/// Module values are pure immutable data: an initializer may not produce a
/// closure that captures mutable state or an acquired resource created
/// while initializing (in the initializer, or inside a function it calls
/// that returns such a closure).
pub fn check_module_closures(inf: &mut Infer<'_, '_>) {
    let hir = inf.g.hir;
    // the state a closure-valued expression captures, if any
    fn captured_state(
        inf: &mut Infer<'_, '_>,
        e: ExprId,
        returning: &FxHashMap<DefId, String>,
    ) -> Option<String> {
        let hir = inf.g.hir;
        match &hir.exprs[e].kind {
            ExprKind::Lambda(l) => {
                for c in &l.captures {
                    if let Some(t) = inf.locals.get(c).cloned() {
                        let t = inf.icx.resolve(&t);
                        if let Some(s) = crate::program::stored_state(inf.g, &t, &mut vec![]) {
                            return Some(s);
                        }
                    }
                }
                None
            }
            ExprKind::Call { callee, .. } => match inf.res.get(callee) {
                Some(Res::Callee { def, .. }) => returning.get(def).cloned(),
                _ => None,
            },
            ExprKind::Block { tail: Some(t), .. } => captured_state(inf, *t, returning),
            ExprKind::If { then, els, .. } => captured_state(inf, *then, returning)
                .or_else(|| els.and_then(|x| captured_state(inf, x, returning))),
            ExprKind::Match { arms, .. } => {
                let bodies: Vec<ExprId> = arms.iter().map(|a| a.body).collect();
                bodies.into_iter().find_map(|b| captured_state(inf, b, returning))
            }
            _ => None,
        }
    }
    // functions whose result is a closure over state they create
    let mut returning: FxHashMap<DefId, String> = FxHashMap::default();
    loop {
        let mut changed = false;
        for (id, def) in hir.defs.iter() {
            if hir.modules[def.module].is_std || returning.contains_key(&id) {
                continue;
            }
            let DefKind::Fn(f) = &def.kind else { continue };
            let Some(body) = f.body else { continue };
            if let Some(s) = captured_state(inf, body, &returning) {
                returning.insert(id, s);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut reports = vec![];
    for (_, def) in hir.defs.iter() {
        if hir.modules[def.module].is_std {
            continue;
        }
        let DefKind::Global(g) = &def.kind else { continue };
        if let Some(s) = captured_state(inf, g.value, &returning) {
            reports.push((def.span, inf.g.interner.resolve(def.name).to_string(), s));
        }
    }
    for (span, name, s) in reports {
        inf.push_diag(
            Diagnostic::error(format!("module-level value `{name}` holds a closure over a `{s}`"))
                .with_code("E0138")
                .with_label(span, "module values are pure immutable data shared by every task")
                .with_note("create mutable state and acquire resources in functions or system initialization, and pass them where they are needed"),
        );
    }
}

/// Calls recorded before their generic callee was analysed.
pub fn check_deferred_dups(inf: &mut Infer<'_, '_>) {
    let calls = std::mem::take(&mut *inf.g.dup_deferred.borrow_mut());
    if calls.is_empty() {
        return;
    }
    let mut o = Own {
        inf,
        affine: FxHashMap::default(),
        reported: FxHashSet::default(),
        consuming: FxHashSet::default(),
        consuming_lambdas: FxHashMap::default(),
        last_use: FxHashMap::default(),
        loans: vec![],
        params: FxHashSet::default(),
        ret_ty: None,
        carriers: FxHashMap::default(),
        generic: false,
        offending: vec![],
        reads: FxHashSet::default(),
        lambda_borrows: FxHashMap::default(),
        local_lambdas: FxHashMap::default(),
        unread: FxHashMap::default(),
    };
    for (def, args, span) in calls {
        o.inf.g.dup_done.borrow_mut().insert(def);
        o.check_dup(def, &args, span);
    }
}

/// Which scheme parameters of generic `d` its body may duplicate: the body
/// is re-checked with its type variables treated as affine, and the
/// variables of every value it would then use twice are recorded.
pub fn generic_pass(inf: &mut Infer<'_, '_>, d: DefId, scheme: &crate::program::Scheme) {
    let hir = inf.g.hir;
    inf.g.dup_done.borrow_mut().insert(d);
    if scheme.n == 0 || hir.modules[hir.defs[d].module].is_std {
        return;
    }
    let DefKind::Fn(f) = &hir.defs[d].kind else { return };
    let Some(body) = f.body else { return };
    let params: Vec<PatId> = f.params.iter().map(|p| p.pat).collect();
    let params = &params[..];
    let ret_ty = match &*scheme.ty {
        TyKind::Fn(f) => Some(f.ret.clone()),
        _ => None,
    };
    // diagnostics the ordinary pass already gave are not re-flagged
    let reported: FxHashSet<Span> = inf.diags.iter().flat_map(|d| d.labels.iter().map(|l| l.span)).collect();
    let TyKind::Fn(sf) = &*scheme.ty else { return };
    // scheme parameter index of each type variable in the body's view
    let mut binding: Vec<(u32, Ty)> = vec![];
    for (sp, p) in sf.params.iter().zip(params) {
        if let Some(t) = inf.pats.get(p).cloned() {
            let t = inf.icx.resolve(&t);
            match_gens(sp, &t, &mut binding);
        }
    }
    // `T: Share` parameters may be duplicated freely
    let share = inf.g.hir.try_lang("Share");
    let shared: Vec<u32> = scheme
        .constraints
        .iter()
        .filter_map(|c| match c {
            crate::solve::Constraint::Impl { proto, ty, .. } if Some(*proto) == share => match &**ty {
                TyKind::Gen(k) => Some(*k),
                _ => None,
            },
            _ => None,
        })
        .collect();
    let saved = std::mem::take(&mut inf.diags);
    let mut o = Own {
        inf,
        affine: FxHashMap::default(),
        reported,
        consuming: FxHashSet::default(),
        consuming_lambdas: FxHashMap::default(),
        last_use: FxHashMap::default(),
        loans: vec![],
        params: FxHashSet::default(),
        ret_ty,
        carriers: FxHashMap::default(),
        generic: true,
        offending: vec![],
        reads: FxHashSet::default(),
        lambda_borrows: FxHashMap::default(),
        local_lambdas: FxHashMap::default(),
        unread: FxHashMap::default(),
    };
    for p in params {
        let mut ls = vec![];
        pat_locals(o.inf.g.hir, *p, &mut ls);
        o.params.extend(ls);
    }
    o.collect_uses(body);
    let mut st = State::default();
    let m = o.mode_into(o.ret_ty.clone());
    o.expr(body, m, &mut st);
    o.check_consuming_result(body);
    let offending = std::mem::take(&mut o.offending);
    let flagged = !o.inf.diags.is_empty();
    o.inf.diags = saved;
    if !flagged {
        return;
    }
    let mut vars: Vec<Ty> = vec![];
    for t in &offending {
        let t = o.inf.icx.resolve(t);
        ty_any(&t, &mut |k| {
            if matches!(k, TyKind::Skolem(_) | TyKind::Var(_)) {
                vars.push(mk(k.clone()));
            }
            false
        });
    }
    let mut dup: Vec<u32> = binding
        .iter()
        .filter(|(_, t)| {
            let t = o.inf.icx.resolve(t);
            offending.is_empty() || vars.iter().any(|v| same_var_root(&mut o.inf.icx, v, &t))
        })
        .map(|(k, _)| *k)
        .filter(|k| !shared.contains(k))
        .collect();
    dup.sort();
    dup.dedup();
    if !dup.is_empty() {
        o.inf.g.dup_params.borrow_mut().insert(d, dup);
    }
}

fn same_var(a: &Ty, b: &Ty) -> bool {
    match (&**a, &**b) {
        (TyKind::Skolem(x), TyKind::Skolem(y)) => x == y,
        (TyKind::Var(x), TyKind::Var(y)) => x == y,
        _ => false,
    }
}

fn same_var_root(icx: &mut InferCtxt, a: &Ty, b: &Ty) -> bool {
    match (&**a, &**b) {
        (TyKind::Var(x), TyKind::Var(y)) => icx.root(*x) == icx.root(*y),
        _ => same_var(a, b),
    }
}

/// Pair each `Gen(k)` of a scheme type with the type at the same position.
fn match_gens(s: &Ty, t: &Ty, out: &mut Vec<(u32, Ty)>) {
    match (&**s, &**t) {
        (TyKind::Gen(k), _) => out.push((*k, t.clone())),
        (TyKind::Con(a, xs), TyKind::Con(b, ys)) | (TyKind::Dyn(a, xs), TyKind::Dyn(b, ys)) if a == b => {
            xs.iter().zip(ys).for_each(|(x, y)| match_gens(x, y, out))
        }
        (TyKind::Tuple(xs), TyKind::Tuple(ys)) => xs.iter().zip(ys).for_each(|(x, y)| match_gens(x, y, out)),
        (TyKind::Fn(f), TyKind::Fn(g)) => {
            f.params.iter().zip(&g.params).for_each(|(x, y)| match_gens(x, y, out));
            match_gens(&f.ret, &g.ret, out);
        }
        (TyKind::Ref(_, x), TyKind::Ref(_, y)) | (TyKind::Record(x), TyKind::Record(y)) => {
            match_gens(x, y, out)
        }
        (TyKind::RowExt(l, x, r), _) => {
            let (fs, tail) = row_fields(t);
            if let Some((_, y)) = fs.iter().find(|f| f.0 == *l) {
                match_gens(x, y, out);
            }
            let _ = tail;
            match_gens(r, t, out);
        }
        _ => {}
    }
}

fn pat_locals(hir: &lang_hir::Hir, p: PatId, out: &mut Vec<LocalId>) {
    match &hir.pats[p].kind {
        PatKind::Bind(l) => out.push(*l),
        PatKind::Tuple(ps) => ps.iter().for_each(|x| pat_locals(hir, *x, out)),
        PatKind::Record { fields, .. } => fields.iter().for_each(|(_, x)| pat_locals(hir, *x, out)),
        PatKind::Variant { args, .. } => args.iter().for_each(|x| pat_locals(hir, *x, out)),
        _ => {}
    }
}

/// Direct sub-expressions of an expression.
fn children(kind: &ExprKind) -> Vec<ExprId> {
    let mut v = vec![];
    match kind {
        ExprKind::Interp(parts) => {
            for p in parts {
                if let InterpPart::Expr(x) = p {
                    v.push(*x);
                }
            }
        }
        ExprKind::Field { base, .. } => v.push(*base),
        ExprKind::Index { base, index } => v.extend([*base, *index]),
        ExprKind::Call { callee, args } => {
            v.push(*callee);
            v.extend(args.iter().copied());
        }
        ExprKind::MethodCall { recv, args, .. } => {
            v.push(*recv);
            v.extend(args.iter().copied());
        }
        ExprKind::Try(x) | ExprKind::Unsafe(x) | ExprKind::Loop { body: x } => v.push(*x),
        ExprKind::Unary { operand, .. } => v.push(*operand),
        ExprKind::Binary { lhs, rhs, .. } => v.extend([*lhs, *rhs]),
        ExprKind::Tuple(xs) | ExprKind::List(xs) | ExprKind::Set(xs) => v.extend(xs.iter().copied()),
        ExprKind::Map(kv) => kv.iter().for_each(|(k, x)| v.extend([*k, *x])),
        ExprKind::Record { spread, fields } => {
            v.extend(spread.iter().copied());
            v.extend(fields.iter().map(|f| f.2));
        }
        ExprKind::Lambda(l) => v.push(l.body),
        ExprKind::If { cond, then, els } => {
            v.extend([*cond, *then]);
            v.extend(els.iter().copied());
        }
        ExprKind::Match { scrutinee, arms } => {
            v.push(*scrutinee);
            for a in arms {
                v.extend(a.guard.iter().copied());
                v.push(a.body);
            }
        }
        ExprKind::While { cond, body } => v.extend([*cond, *body]),
        ExprKind::For { iter, body, .. } => v.extend([*iter, *body]),
        ExprKind::Block { stmts, tail } => {
            for s in stmts {
                match s {
                    Stmt::Let { init, .. } => v.push(*init),
                    Stmt::Assign { value, .. } => v.push(*value),
                    Stmt::Expr { expr, .. } => v.push(*expr),
                }
            }
            v.extend(tail.iter().copied());
        }
        ExprKind::Return { value, .. } | ExprKind::Break { value, .. } => v.extend(value.iter().copied()),
        _ => {}
    }
    v
}

impl Own<'_, '_, '_> {
    /// Record the last source position at which each local is used.
    fn collect_uses(&mut self, e: ExprId) {
        let hir = self.inf.g.hir;
        let span = hir.exprs[e].span;
        let pos = span.range().end;
        match &hir.exprs[e].kind {
            ExprKind::Local(l) => {
                self.bump(*l, pos);
                self.reads.insert(*l);
            }
            ExprKind::Lambda(l) => {
                for c in &l.captures {
                    self.bump(*c, pos);
                    self.reads.insert(*c);
                }
            }
            ExprKind::Block { stmts, .. } => {
                for s in stmts {
                    if let Stmt::Assign { local, span, .. } = s {
                        self.bump(*local, span.range().end);
                    }
                }
            }
            _ => {}
        }
        for c in children(&hir.exprs[e].kind) {
            self.collect_uses(c);
        }
    }

    /// A named fallible task that is never used again loses its error.
    fn unused_tasks(&mut self, e: ExprId) {
        let hir = self.inf.g.hir;
        if let ExprKind::Block { stmts, .. } = &hir.exprs[e].kind {
            for s in stmts {
                let Stmt::Let { pat, .. } = s else { continue };
                let PatKind::Bind(l) = &hir.pats[*pat].kind else { continue };
                // a bound `Result` that is never read is ignored
                if !self.reads.contains(l)
                    && let Some(t) = self.local_ty(*l)
                    && matches!(&*t, TyKind::Con(d, _) if *d == self.inf.g.env.b.result)
                {
                    let span = hir.pats[*pat].span;
                    let name = self.name(*l);
                    self.report(
                        Diagnostic::error(format!("`{name}` holds a `Result` that is never inspected"))
                            .with_code("E0337")
                            .with_label(span, "this `Result` is ignored")
                            .with_note(
                                "handle it with `?` or `match`, or bind it to `_` to discard explicitly",
                            ),
                        span,
                    );
                    continue;
                }
                if self.last_use.contains_key(l) {
                    continue;
                }
                let Some(t) = self.local_ty(*l) else { continue };
                // fallible tasks inside a collection or aggregate
                if !matches!(&*t, TyKind::Con(d, _) if Some(*d) == hir.try_lang("task.Task"))
                    && self.holds_fallible_task(&t, 0)
                {
                    let span = hir.pats[*pat].span;
                    let name = self.name(*l);
                    self.report(
                        Diagnostic::error(format!("`{name}` holds tasks whose `Result`s are never inspected"))
                            .with_code("E0337")
                            .with_label(span, "these tasks' errors would be lost")
                            .with_note("join each task and handle its `Result`, or name the binding `_` to discard explicitly"),
                        span,
                    );
                    continue;
                }
                let TyKind::Con(d, args) = &*t else { continue };
                if Some(*d) != hir.try_lang("task.Task") || args.len() != 2 {
                    continue;
                }
                let r = self.inf.icx.resolve(&args[1]);
                if matches!(&*r, TyKind::Con(rd, _) if *rd == self.inf.g.env.b.result) {
                    let span = hir.pats[*pat].span;
                    let name = self.name(*l);
                    self.report(
                        Diagnostic::error(format!(
                            "task `{name}` returns a `Result` that is never inspected"
                        ))
                        .with_code("E0337")
                        .with_label(span, "this task's error would be lost")
                        .with_note("join it and handle the `Result`, or name it `_` to discard explicitly"),
                        span,
                    );
                }
            }
        }
        for c in children(&hir.exprs[e].kind) {
            self.unused_tasks(c);
        }
    }

    /// Whether a type holds a task whose result is a `Result`.
    fn holds_fallible_task(&mut self, t: &Ty, depth: usize) -> bool {
        if depth > 6 {
            return false;
        }
        let t = self.inf.icx.resolve(t);
        let hir = self.inf.g.hir;
        match &*t {
            TyKind::Con(d, args) if Some(*d) == hir.try_lang("task.Task") && args.len() == 2 => {
                let r = self.inf.icx.resolve(&args[1]);
                matches!(&*r, TyKind::Con(rd, _) if *rd == self.inf.g.env.b.result)
            }
            TyKind::Con(_, args) | TyKind::Tuple(args) => {
                args.clone().iter().any(|a| self.holds_fallible_task(a, depth + 1))
            }
            TyKind::Record(r) => row_fields(r).0.iter().any(|(_, x)| self.holds_fallible_task(x, depth + 1)),
            _ => false,
        }
    }

    fn bump(&mut self, l: LocalId, pos: usize) {
        let e = self.last_use.entry(l).or_insert(0);
        *e = (*e).max(pos);
    }

    fn is_affine_ty(&mut self, t: &Ty, depth: usize) -> bool {
        if depth > 8 {
            return false;
        }
        let t = self.inf.icx.resolve(t);
        match &*t {
            TyKind::Con(d, args) => {
                let name = self.inf.lang_name(*d);
                if matches!(
                    name.as_deref(),
                    Some("ListBuilder" | "Buffer" | "Batch" | "MapBuilder" | "SetBuilder" | "FixedBuffer")
                ) {
                    return true;
                }
                // a sequence of affine items is a one-shot iterator
                if name.as_deref() == Some("Seq") {
                    return args.iter().any(|a| self.is_affine_ty(a, depth + 1));
                }
                let fields: Vec<Ty> = if let Some(e) = self.inf.g.env.enums.get(d) {
                    (0..e.variants.len())
                        .flat_map(|v| {
                            self.inf.g.env.variant_fields(*d, v as u32, args).into_iter().map(|f| f.1)
                        })
                        .collect()
                } else if self.inf.g.env.structs.contains_key(d) {
                    self.inf.g.env.struct_fields(*d, args).into_iter().map(|f| f.1).collect()
                } else if matches!(name.as_deref(), Some("List" | "Array" | "Map" | "Set" | "Task")) {
                    // runtime collections and task results hold their
                    // elements: affine elements make them affine
                    args.clone()
                } else {
                    // handles (channel endpoints, cells, atoms, pools) refer to
                    // shared runtime objects: they are not affine themselves
                    vec![]
                };
                fields
                    .iter()
                    .any(|f| !matches!(&**f, TyKind::Con(x, _) if x == d) && self.is_affine_ty(f, depth + 1))
            }
            TyKind::Fn(f) => f.once,
            TyKind::Skolem(s) if self.generic => {
                !self.inf.icx.skolems.get(*s as usize).is_some_and(|k| k.region)
            }
            TyKind::Var(_) | TyKind::Gen(_) if self.generic => true,
            TyKind::Tuple(ts) => ts.iter().any(|x| self.is_affine_ty(x, depth + 1)),
            TyKind::Record(r) => row_fields(r).0.iter().any(|(_, x)| self.is_affine_ty(x, depth + 1)),
            TyKind::Ref(..) => false,
            _ => false,
        }
    }

    /// Whether an expression's type is a `Seq`.
    fn seq_typed(&mut self, e: ExprId) -> bool {
        let Some(t) = self.inf.exprs.get(&e).cloned() else { return false };
        let t = self.inf.icx.resolve(&t);
        matches!(&*t, TyKind::Con(d, _) if self.inf.lang_name(*d).as_deref() == Some("Seq"))
    }

    /// Whether any subexpression (outside closures) has an affine type.
    fn has_affine_part(&mut self, e: ExprId) -> bool {
        let hir = self.inf.g.hir;
        if matches!(hir.exprs[e].kind, ExprKind::Lambda(_)) {
            return false;
        }
        if self.expr_affine(e) {
            return true;
        }
        children(&hir.exprs[e].kind).into_iter().any(|c| self.has_affine_part(c))
    }

    /// A list, array, or sequence (of affine items, when `expr_affine`).
    fn elementwise_receiver(&mut self, e: ExprId) -> bool {
        let Some(t) = self.inf.exprs.get(&e).cloned() else { return false };
        let t = self.inf.icx.resolve(&t);
        let t = match &*t {
            TyKind::Ref(_, x) => x.clone(),
            _ => t,
        };
        match &*t {
            TyKind::Con(d, _) => matches!(self.inf.lang_name(*d).as_deref(), Some("List" | "Array" | "Seq")),
            _ => false,
        }
    }

    fn expr_affine(&mut self, e: ExprId) -> bool {
        match self.inf.exprs.get(&e).cloned() {
            Some(t) => self.is_affine_ty(&t, 0),
            None => false,
        }
    }

    fn local_ty(&mut self, l: LocalId) -> Option<Ty> {
        let t = self.inf.locals.get(&l).cloned()?;
        Some(self.inf.icx.resolve(&t))
    }

    fn is_affine(&mut self, l: LocalId) -> bool {
        if self.consuming.contains(&l) {
            return true;
        }
        if let Some(b) = self.affine.get(&l) {
            return *b;
        }
        let b = match self.local_ty(l) {
            // a binding of borrowed type does not own its referent
            Some(r) => !matches!(&*r, TyKind::Ref(..)) && self.is_affine_ty(&r, 0),
            None => false,
        };
        self.affine.insert(l, b);
        b
    }

    /// A binding of borrowed type whose referent is affine.
    fn borrows_affine(&mut self, l: LocalId) -> bool {
        match self.local_ty(l) {
            Some(r) => match &*r {
                TyKind::Ref(_, inner) => {
                    let inner = inner.clone();
                    self.is_affine_ty(&inner, 0)
                }
                _ => false,
            },
            None => false,
        }
    }

    /// A borrow stored into a cell, atom, builder, buffer, or channel could
    /// outlive the borrowed value (the container may live longer and be read
    /// after the owner moves): rejected.
    fn check_stored_borrows(&mut self, e: ExprId, recv: ExprId, args: &[ExprId]) {
        let hir = self.inf.g.hir;
        let ExprKind::MethodCall { method, .. } = &hir.exprs[e].kind else { return };
        if !matches!(
            self.inf.sym(*method),
            "set" | "update" | "swap" | "push" | "append" | "try_append" | "send" | "try_send" | "insert"
        ) {
            return;
        }
        let Some(rt) = self.inf.exprs.get(&recv).cloned() else { return };
        let rt = self.inf.icx.resolve(&rt);
        let rt = match &*rt {
            TyKind::Ref(_, x) => x.clone(),
            _ => rt,
        };
        let container = match &*rt {
            TyKind::Con(d, _) => self.inf.lang_name(*d).filter(|n| {
                matches!(
                    n.as_str(),
                    "Cell"
                        | "Atom"
                        | "ListBuilder"
                        | "MapBuilder"
                        | "SetBuilder"
                        | "Buffer"
                        | "FixedBuffer"
                        | "channel.Sender"
                        | "channel.BatchSender"
                        | "Sender"
                        | "BatchSender"
                )
            }),
            _ => None,
        };
        let Some(container) = container else { return };
        for a in args {
            let mut carried = vec![];
            self.carried_borrows(*a, &mut carried);
            if let ExprKind::Local(l) = &hir.exprs[*a].kind {
                for ln in self.loans.iter().filter(|ln| ln.holder == *l) {
                    carried.push((ln.owner, ln.span));
                }
            }
            for (owner, _) in carried {
                let name = self.name(owner);
                let span = hir.exprs[*a].span;
                let shown = container.rsplit('.').next().unwrap_or(&container).to_string();
                self.report(
                    Diagnostic::error(format!("a borrow of `{name}` is stored in a `{shown}`"))
                        .with_code("E0407")
                        .with_label(
                            span,
                            format!("the `{shown}` can be read after `{name}` moves or is dropped"),
                        )
                        .with_note("store the value itself (or an index or key) rather than a reference"),
                    span,
                );
            }
        }
    }

    /// Whether an expression takes an exclusive borrow (outside closures).
    fn mentions_mut_borrow(&self, e: ExprId) -> bool {
        let hir = self.inf.g.hir;
        match &hir.exprs[e].kind {
            ExprKind::Unary { op: UnOp::RefMut, .. } => true,
            ExprKind::Lambda(_) => false,
            k => children(k).into_iter().any(|c| self.mentions_mut_borrow(c)),
        }
    }

    /// A function returning a closure that consumes what it captured must
    /// say so in its result type (`once fn`): callers could otherwise call
    /// it repeatedly.
    fn check_consuming_result(&mut self, body: ExprId) {
        let Some(ret) = self.ret_ty.clone() else { return };
        let ret = self.inf.icx.resolve(&ret);
        let repeatable = match &*ret {
            TyKind::Fn(f) => !f.once,
            _ => self.holds_fn(&ret, 0),
        };
        if !repeatable {
            return;
        }
        let mut exits = vec![];
        self.result_exprs(body, &mut exits);
        for x in exits {
            if self.holds_consuming(x) {
                let span = self.inf.g.hir.exprs[x].span;
                let shown = self.inf.show(&ret);
                self.report(
                    Diagnostic::error(
                        "a closure that consumes a captured value is returned as a repeatable function",
                    )
                    .with_code("E0404")
                    .with_label(span, format!("callers of this `{shown}` could call it more than once"))
                    .with_note(
                        "declare the result `once fn(...)`, or borrow the capture instead of consuming it",
                    ),
                    span,
                );
            }
        }
    }

    /// Expressions whose value a function body returns.
    fn result_exprs(&self, e: ExprId, out: &mut Vec<ExprId>) {
        let hir = self.inf.g.hir;
        match &hir.exprs[e].kind {
            ExprKind::Block { tail: Some(t), .. } => self.result_exprs(*t, out),
            ExprKind::If { then, els: Some(x), .. } => {
                self.result_exprs(*then, out);
                self.result_exprs(*x, out);
            }
            ExprKind::Match { arms, .. } => arms.iter().for_each(|a| self.result_exprs(a.body, out)),
            _ => out.push(e),
        }
        self.return_values(e, out);
    }

    fn return_values(&self, e: ExprId, out: &mut Vec<ExprId>) {
        let hir = self.inf.g.hir;
        match &hir.exprs[e].kind {
            ExprKind::Lambda(_) => {}
            ExprKind::Return { value: Some(v), .. } => out.push(*v),
            k => {
                for c in children(k) {
                    self.return_values(c, out);
                }
            }
        }
    }

    /// Mark the function-holding locals a pattern binds from a value that
    /// holds a consuming closure as single-use.
    fn mark_consuming_bindings(&mut self, pat: PatId) {
        let mut bound = vec![];
        pat_locals(self.inf.g.hir, pat, &mut bound);
        for b in bound {
            if self.local_ty(b).is_some_and(|t| self.holds_fn(&t, 0)) {
                self.consuming.insert(b);
                self.affine.insert(b, true);
            }
        }
    }

    /// Whether a type holds a function (directly, in fields, payloads, or
    /// elements).
    fn holds_fn(&mut self, t: &Ty, depth: usize) -> bool {
        if depth > 6 {
            return false;
        }
        let t = self.inf.icx.resolve(t);
        match &*t {
            TyKind::Fn(_) => true,
            TyKind::Con(d, args) => {
                let fields: Vec<Ty> = if let Some(e) = self.inf.g.env.enums.get(d) {
                    (0..e.variants.len())
                        .flat_map(|v| {
                            self.inf.g.env.variant_fields(*d, v as u32, args).into_iter().map(|f| f.1)
                        })
                        .collect()
                } else if self.inf.g.env.structs.contains_key(d) {
                    self.inf.g.env.struct_fields(*d, args).into_iter().map(|f| f.1).collect()
                } else {
                    args.clone()
                };
                fields
                    .iter()
                    .any(|f| !matches!(&**f, TyKind::Con(x, _) if x == d) && self.holds_fn(f, depth + 1))
            }
            TyKind::Tuple(ts) => ts.clone().iter().any(|x| self.holds_fn(x, depth + 1)),
            TyKind::Record(r) => row_fields(r).0.iter().any(|(_, x)| self.holds_fn(x, depth + 1)),
            _ => false,
        }
    }

    /// The local a place expression (`r.f`, `fs[0]`) is rooted in.
    fn place_root(&self, mut e: ExprId) -> Option<LocalId> {
        let hir = self.inf.g.hir;
        loop {
            match &hir.exprs[e].kind {
                ExprKind::Local(l) => return Some(*l),
                ExprKind::Field { base, .. } | ExprKind::Index { base, .. } => e = *base,
                _ => return None,
            }
        }
    }

    /// Whether a value contains a consuming closure (outside other closures'
    /// bodies): directly, in an aggregate, or as an argument of a call that
    /// may return it.
    fn holds_consuming(&self, e: ExprId) -> bool {
        let hir = self.inf.g.hir;
        if self.consuming_lambdas.contains_key(&e) {
            return true;
        }
        match &hir.exprs[e].kind {
            ExprKind::Local(l) => self.consuming.contains(l),
            ExprKind::Lambda(_) => false,
            k => children(k).into_iter().any(|c| self.holds_consuming(c)),
        }
    }

    /// Whether a closure's body would use one of its parameters more than
    /// once if the parameter were affine; reported when an affine argument
    /// is passed for it.
    fn check_lambda_dup(&mut self, lam: ExprId, args: &[ExprId], span: Span) {
        let hir = self.inf.g.hir;
        let ExprKind::Lambda(l) = &hir.exprs[lam].kind else { return };
        let affine_args: Vec<usize> = (0..args.len()).filter(|i| self.expr_affine(args[*i])).collect();
        if affine_args.is_empty() || self.generic {
            return;
        }
        let mut params: Vec<LocalId> = vec![];
        for &i in &affine_args {
            if let Some(p) = l.params.get(i) {
                pat_locals(hir, p.pat, &mut params);
            }
        }
        // re-walk the body with the parameters' types treated as affine
        let saved = std::mem::take(&mut self.inf.diags);
        let (was_generic, saved_reported, saved_affine) =
            (self.generic, std::mem::take(&mut self.reported), std::mem::take(&mut self.affine));
        self.generic = true;
        let mut inner = State::default();
        self.expr(l.body, Mode::Move, &mut inner);
        self.offending.clear();
        let dup = !self.inf.diags.is_empty();
        self.generic = was_generic;
        self.reported = saved_reported;
        self.affine = saved_affine;
        self.inf.diags = saved;
        let _ = params;
        if dup {
            let shown =
                self.inf.show(&self.inf.exprs.get(&args[affine_args[0]]).cloned().unwrap_or_else(error_ty));
            self.report(
                Diagnostic::error(format!(
                    "this closure may use its argument of type `{shown}` more than once"
                ))
                .with_code("E0404")
                .with_label(span, format!("`{shown}` is affine: it has one owning use"))
                .with_note("the closure copies or reuses this parameter; pass a shareable value, or borrow"),
                span,
            );
        }
    }

    /// A generic function that may duplicate a type parameter cannot take
    /// an affine type for it.
    fn check_dup(&mut self, def: DefId, args: &[Ty], span: Span) {
        let hir = self.inf.g.hir;
        // a user function analysed later: check this call once it is
        if !self.generic
            && !self.inf.g.dup_done.borrow().contains(&def)
            && !hir.modules[hir.defs[def].module].is_std
            && matches!(&hir.defs[def].kind, DefKind::Fn(f) if f.body.is_some())
        {
            let args: Vec<Ty> = args.iter().map(|a| self.inf.icx.resolve(a)).collect();
            self.inf.g.dup_deferred.borrow_mut().push((def, args, span));
            return;
        }
        let Some(dup) = self.inf.g.dup_params.borrow().get(&def).cloned() else { return };
        for k in dup {
            let Some(a) = args.get(k as usize) else { continue };
            let a = self.inf.icx.resolve(a);
            if self.is_affine_ty(&a, 0) {
                if self.generic {
                    self.offending.push(a.clone());
                }
                let hir = self.inf.g.hir;
                let name = self.inf.sym(hir.defs[def].name).to_string();
                let shown = self.inf.show(&a);
                self.report(
                    Diagnostic::error(format!("`{name}` may use its argument of type `{shown}` more than once"))
                        .with_code("E0404")
                        .with_label(span, format!("`{shown}` is affine: it has one owning use"))
                        .with_related(name.clone(), hir.defs[def].span)
                        .with_note("this generic function copies or reuses a value of this type parameter; pass a shareable type, or borrow"),
                    span,
                );
            }
        }
    }

    fn name(&self, l: LocalId) -> String {
        self.inf.sym(self.inf.g.hir.locals[l].name).to_string()
    }

    fn report(&mut self, d: Diagnostic, span: Span) {
        if self.reported.insert(span) {
            self.inf.push_diag(d);
        }
    }

    /// Assigning over a `Result` that was never read loses it.
    fn overwrite_result(&mut self, l: LocalId, span: Span) {
        if self.generic {
            return;
        }
        let is_result = self
            .local_ty(l)
            .is_some_and(|t| matches!(&*t, TyKind::Con(d, _) if *d == self.inf.g.env.b.result));
        if !is_result {
            return;
        }
        if let Some(prev) = self.unread.insert(l, span) {
            let name = self.name(l);
            self.report(
                Diagnostic::error(format!("the `Result` in `{name}` is overwritten without being inspected"))
                    .with_code("E0337")
                    .with_label(span, "this assignment discards the previous `Result`")
                    .with_label(prev, "this `Result` is never read")
                    .with_note("handle the earlier `Result` before replacing it"),
                span,
            );
        }
    }

    fn use_local(&mut self, l: LocalId, span: Span, mode: Mode, st: &mut State) {
        self.unread.remove(&l);
        self.check_loans(l, span, mode);
        if mode == Mode::Move && self.borrows_affine(l) {
            if self.generic
                && let Some(t) = self.local_ty(l)
            {
                self.offending.push(t);
            }
            let name = self.name(l);
            self.report(
                Diagnostic::error(format!("cannot move out of borrowed `{name}`"))
                    .with_code("E0404")
                    .with_label(span, "this takes ownership of a value that is only borrowed")
                    .with_note(
                        "borrowed parameters do not acquire ownership; take the parameter by value instead",
                    ),
                span,
            );
            return;
        }
        if !self.is_affine(l) {
            return;
        }
        if let Some(moved) = st.get(&l).copied() {
            if self.generic
                && let Some(t) = self.local_ty(l)
            {
                self.offending.push(t);
            }
            let name = self.name(l);
            self.report(
                Diagnostic::error(format!("use of moved value `{name}`"))
                    .with_code("E0404")
                    .with_label(span, "value used here after move")
                    .with_label(moved, "value moved here")
                    .with_note("affine values (builders, buffers, leases, consuming closures) have one owning use; borrow with `&` to keep using it"),
                span,
            );
            return;
        }
        if mode == Mode::Move {
            st.insert(l, span);
        }
    }

    /// Accesses of a place while a named borrow of it is still live.
    fn check_loans(&mut self, l: LocalId, span: Span, mode: Mode) {
        let pos = span.range().start;
        let live: Vec<Loan> = self
            .loans
            .iter()
            .filter(|ln| {
                ln.owner == l
                    && ln.holder != l
                    && ln.start < pos
                    && pos < self.last_use.get(&ln.holder).copied().unwrap_or(0)
            })
            .cloned()
            .collect();
        for ln in live {
            if ln.mutable || mode != Mode::Borrow {
                let (owner, holder) = (self.name(l), self.name(ln.holder));
                let what = match mode {
                    Mode::Move => "moved",
                    Mode::BorrowMut => "mutated or mutably borrowed",
                    Mode::Borrow => "used",
                };
                let kind = if ln.mutable { "exclusively borrowed" } else { "borrowed" };
                self.report(
                    Diagnostic::error(format!("`{owner}` is {what} while {kind} by `{holder}`"))
                        .with_code("E0405")
                        .with_label(span, format!("`{owner}` {what} here"))
                        .with_label(ln.span, format!("`{holder}` borrows `{owner}` here and is used later"))
                        .with_note("an exclusive borrow excludes other access; a shared borrow excludes moves and mutation"),
                    span,
                );
            }
        }
    }

    /// How a callee takes its parameters (from its scheme).
    fn param_modes(&mut self, def: DefId) -> Vec<Param> {
        match self.inf.g.scheme(def).map(|s| s.ty) {
            Some(t) => match &*t {
                TyKind::Fn(f) => f
                    .params
                    .iter()
                    .map(|p| match &**p {
                        TyKind::Ref(true, _) => Param::Exclusive,
                        TyKind::Ref(false, _) => Param::Shared,
                        TyKind::Fn(pf) => Param::Owned { repeatable_fn: !pf.once },
                        _ => Param::Owned { repeatable_fn: false },
                    })
                    .collect(),
                _ => vec![],
            },
            None => vec![],
        }
    }

    fn mode_of(p: Option<Param>) -> Mode {
        match p {
            Some(Param::Shared) => Mode::Borrow,
            Some(Param::Exclusive) => Mode::BorrowMut,
            _ => Mode::Move,
        }
    }

    /// A value passed where exclusive access is needed (`&mut self`, a
    /// `&mut T` parameter) must not be a shared borrow.
    fn check_exclusive_access(&mut self, e: ExprId) {
        let hir = self.inf.g.hir;
        let shared = match &hir.exprs[e].kind {
            ExprKind::Unary { op: UnOp::Ref, .. } => true,
            ExprKind::Local(l) => matches!(self.local_ty(*l).as_deref(), Some(TyKind::Ref(false, _))),
            // a field or element reached through a shared borrow
            ExprKind::Field { .. } | ExprKind::Index { .. } => {
                let mut x = e;
                let mut through_shared = false;
                while let ExprKind::Field { base, .. } | ExprKind::Index { base, .. } = &hir.exprs[x].kind {
                    x = *base;
                    let bt = self.inf.exprs.get(&x).cloned().map(|t| self.inf.icx.resolve(&t));
                    if matches!(bt.as_deref(), Some(TyKind::Ref(false, _))) {
                        through_shared = true;
                        break;
                    }
                }
                through_shared
            }
            _ => false,
        };
        if shared {
            let span = hir.exprs[e].span;
            self.report(
                Diagnostic::error("exclusive access through a shared borrow")
                    .with_code("E0408")
                    .with_label(span, "this is a shared `&` borrow, but mutable access is needed")
                    .with_note("`&T` borrows read access; lend `&mut` to permit mutation"),
                span,
            );
        }
    }

    /// Within one call, an exclusive borrow of a place excludes every other
    /// borrow or use of it among the arguments.
    fn check_call_borrows(&mut self, args: &[ExprId]) {
        let hir = self.inf.g.hir;
        let mut seen: Vec<(LocalId, bool, Span)> = vec![];
        for a in args {
            let sp0 = hir.exprs[*a].span;
            let entries: Vec<(LocalId, bool, Span)> = match &hir.exprs[*a].kind {
                ExprKind::Unary { op, .. } if matches!(op, UnOp::Ref | UnOp::RefMut) => {
                    self.borrowed_local(*a).map(|(l, sp)| (l, *op == UnOp::RefMut, sp)).into_iter().collect()
                }
                // a holder of a borrow stands for the borrowed place
                ExprKind::Local(l) => match self.loans.iter().rev().find(|ln| ln.holder == *l) {
                    Some(ln) => vec![(ln.owner, ln.mutable, sp0)],
                    None => vec![(*l, false, sp0)],
                },
                // a borrow returned by a call stands for what it borrows
                ExprKind::Call { .. } | ExprKind::MethodCall { .. } => {
                    let holds_ref = self
                        .inf
                        .exprs
                        .get(a)
                        .cloned()
                        .is_some_and(|t| matches!(&*self.inf.icx.resolve(&t), TyKind::Ref(..)));
                    let mut carried = vec![];
                    if holds_ref {
                        self.carried_call_borrows(*a, &mut carried);
                    }
                    carried
                }
                // a closure argument uses what it captures
                ExprKind::Lambda(lx) => lx.captures.iter().map(|c| (*c, false, sp0)).collect(),
                _ => vec![],
            };
            for (l, exclusive, sp) in entries {
                if let Some((_, _, first)) =
                    seen.iter().find(|(m, x, _)| *m == l && (*x || exclusive)).copied()
                {
                    let name = self.name(l);
                    self.report(
                        Diagnostic::error(format!(
                            "`{name}` is borrowed exclusively and used again in the same call"
                        ))
                        .with_code("E0405")
                        .with_label(sp, "this use conflicts")
                        .with_label(first, "with this one")
                        .with_note(
                            "an exclusive borrow excludes every other access to the place while it lives",
                        ),
                        sp,
                    );
                }
                seen.push((l, exclusive, sp));
            }
        }
    }

    /// Owners borrowed by a call's arguments, with the borrow's mutability.
    fn carried_call_borrows(&mut self, e: ExprId, out: &mut Vec<(LocalId, bool, Span)>) {
        let hir = self.inf.g.hir;
        let args: Vec<ExprId> = match &hir.exprs[e].kind {
            ExprKind::Call { args, .. } => args.clone(),
            ExprKind::MethodCall { recv, args, .. } => {
                std::iter::once(*recv).chain(args.iter().copied()).collect()
            }
            _ => vec![],
        };
        for a in args {
            match &hir.exprs[a].kind {
                ExprKind::Unary { op, .. } if matches!(op, UnOp::Ref | UnOp::RefMut) => {
                    if let Some((l, sp)) = self.borrowed_local(a) {
                        out.push((l, *op == UnOp::RefMut, sp));
                    }
                }
                ExprKind::Local(l) => {
                    if let Some(ln) = self.loans.iter().rev().find(|ln| ln.holder == *l) {
                        out.push((ln.owner, ln.mutable, hir.exprs[a].span));
                    }
                }
                _ => {}
            }
        }
    }

    /// Ordinary function arguments borrow explicitly: an owned affine value
    /// passed where the function takes `&T` / `&mut T` must be written
    /// `&x` / `&mut x` (methods borrow their receiver automatically).
    fn check_explicit_borrow(&mut self, arg: ExprId, p: Option<Param>) {
        let want = match p {
            Some(Param::Shared) => "&",
            Some(Param::Exclusive) => "&mut ",
            _ => return,
        };
        let hir = self.inf.g.hir;
        if matches!(hir.exprs[arg].kind, ExprKind::Unary { op: UnOp::Ref | UnOp::RefMut, .. }) {
            return;
        }
        let arg_ty = self.inf.exprs.get(&arg).cloned().map(|t| self.inf.icx.resolve(&t));
        if arg_ty.as_ref().is_some_and(|t| matches!(&**t, TyKind::Ref(..))) || !self.expr_affine(arg) {
            return;
        }
        let span = hir.exprs[arg].span;
        self.report(
            Diagnostic::error("an owned value is passed where the function borrows it")
                .with_code("E0412")
                .with_label(span, format!("write `{want}...` to lend this value"))
                .with_note("ordinary function arguments borrow explicitly; only method receivers are borrowed automatically"),
            span,
        );
    }

    /// An argument passed where it may be called repeatedly must not
    /// consume its captures.
    fn check_repeatable(&mut self, arg: ExprId, p: Option<Param>) {
        if p != Some(Param::Owned { repeatable_fn: true }) {
            return;
        }
        let hir = self.inf.g.hir;
        let consumed = match &hir.exprs[arg].kind {
            ExprKind::Lambda(_) => self.consuming_lambdas.get(&arg).copied(),
            ExprKind::Local(l) if self.consuming.contains(l) => Some(*l),
            // a value typed `once fn` (it may consume what it captured)
            ExprKind::Local(l)
                if self.local_ty(*l).is_some_and(|t| matches!(&*t, TyKind::Fn(f) if f.once)) =>
            {
                Some(*l)
            }
            _ => None,
        };
        if let Some(c) = consumed {
            let name = self.name(c);
            let span = hir.exprs[arg].span;
            self.report(
                Diagnostic::error("a closure that consumes a captured value may be called more than once here")
                    .with_code("E0404")
                    .with_label(span, format!("this closure gives away `{name}` when called"))
                    .with_note("such a closure can be called once: pass it where a `once fn` is expected, or borrow the capture"),
                span,
            );
        }
    }

    fn merge(into: &mut State, from: &State) {
        for (k, v) in from {
            into.entry(*k).or_insert(*v);
        }
    }

    /// `&mut` needs an assignable place: a `var` binding, or an owning
    /// mutable container (builder, buffer) reached through a binding.
    fn check_mut_place(&mut self, operand: ExprId, span: Span) {
        let hir = self.inf.g.hir;
        let mut cur = operand;
        loop {
            match &hir.exprs[cur].kind {
                ExprKind::Field { base, .. } => cur = *base,
                ExprKind::Local(l) => {
                    let l = *l;
                    // a shared borrow grants read access only
                    if matches!(self.local_ty(l).as_deref(), Some(TyKind::Ref(false, _))) {
                        let name = self.name(l);
                        self.report(
                            Diagnostic::error(format!("cannot mutate through `{name}`, a shared borrow"))
                                .with_code("E0408")
                                .with_label(span, "`&T` borrows read access")
                                .with_note("take `&mut T` to mutate a borrowed value"),
                            span,
                        );
                        return;
                    }
                    let ok = hir.locals[l].mutable || self.is_affine(l) || self.borrows_affine(l) || {
                        matches!(self.local_ty(l).as_deref(), Some(TyKind::Ref(true, _)))
                    };
                    if !ok {
                        let name = self.name(l);
                        self.report(
                            Diagnostic::error(format!("cannot mutably borrow `{name}`"))
                                .with_code("E0408")
                                .with_label(span, "`let` bindings are not assignable places")
                                .with_note("declare it with `var` to permit mutation through `&mut`"),
                            span,
                        );
                    }
                    return;
                }
                _ => {
                    self.report(
                        Diagnostic::error("cannot mutably borrow a temporary value")
                            .with_code("E0408")
                            .with_label(span, "`&mut` needs an assignable place"),
                        span,
                    );
                    return;
                }
            }
        }
    }

    /// The local a borrow expression refers to (`&x`, or a holder of one).
    fn borrowed_local(&self, e: ExprId) -> Option<(LocalId, Span)> {
        let hir = self.inf.g.hir;
        match &hir.exprs[e].kind {
            ExprKind::Unary { op: UnOp::Ref | UnOp::RefMut, operand } => {
                let mut cur = *operand;
                while let ExprKind::Field { base, .. } = &hir.exprs[cur].kind {
                    cur = *base;
                }
                match &hir.exprs[cur].kind {
                    ExprKind::Local(l) => Some((*l, hir.exprs[e].span)),
                    _ => None,
                }
            }
            ExprKind::Local(r) => {
                self.loans.iter().rev().find(|ln| ln.holder == *r).map(|ln| (ln.owner, hir.exprs[e].span))
            }
            ExprKind::Block { tail: Some(t), .. } => self.borrowed_local(*t),
            _ => None,
        }
    }

    /// How a value flows into a destination of type `target`: copying a
    /// reference into a borrowed destination does not take ownership.
    fn mode_into(&mut self, target: Option<Ty>) -> Mode {
        match target.map(|t| self.inf.icx.resolve(&t)) {
            Some(t) if matches!(&*t, TyKind::Ref(..)) => Mode::Borrow,
            _ => Mode::Move,
        }
    }

    /// Borrows carried by a value: the borrow itself, or borrows placed inside
    /// records, tuples, lists, or constructor/function results whose type
    /// holds a reference.
    fn carried_borrows(&mut self, e: ExprId, out: &mut Vec<(LocalId, Span)>) {
        if let Some(b) = self.borrowed_local(e) {
            out.push(b);
            return;
        }
        let hir = self.inf.g.hir;
        if let ExprKind::Local(l) = &hir.exprs[e].kind
            && let Some(cs) = self.carriers.get(l)
        {
            out.extend(cs.iter().copied());
            return;
        }
        let holds_ref = self.inf.exprs.get(&e).cloned().is_some_and(|t| {
            let t = self.inf.icx.resolve(&t);
            crate::ty::ty_any(&t, &mut |k| matches!(k, TyKind::Ref(..)))
        });
        match &hir.exprs[e].kind {
            ExprKind::Tuple(xs) | ExprKind::List(xs) => {
                for x in xs.clone() {
                    self.carried_borrows(x, out);
                }
            }
            ExprKind::Record { fields, .. } => {
                for f in fields.clone() {
                    self.carried_borrows(f.2, out);
                }
            }
            ExprKind::Map(kv) => {
                for (k, v) in kv.clone() {
                    self.carried_borrows(k, out);
                    self.carried_borrows(v, out);
                }
            }
            ExprKind::Call { callee, args }
                if holds_ref || matches!(hir.exprs[*callee].kind, ExprKind::Ctor { .. }) =>
            {
                for a in args.clone() {
                    self.carried_borrows(a, out);
                }
            }
            ExprKind::Block { tail: Some(t), .. } => self.carried_borrows(*t, out),
            // a closure borrowing its captures (or copying captured borrows)
            ExprKind::Lambda(_) => {
                for c in self.lambda_borrows.get(&e).cloned().unwrap_or_default() {
                    let is_ref = self.local_ty(c).is_some_and(|t| matches!(&*t, TyKind::Ref(..)));
                    if is_ref {
                        let mut copied: Vec<(LocalId, Span)> =
                            self.carriers.get(&c).cloned().unwrap_or_default();
                        copied.extend(
                            self.loans.iter().filter(|ln| ln.holder == c).map(|ln| (ln.owner, ln.span)),
                        );
                        out.extend(copied);
                    } else if self.is_affine(c) {
                        out.push((c, hir.exprs[e].span));
                    }
                }
            }
            _ => {}
        }
    }

    /// Borrows cannot outlive their owner: a block's result may not borrow
    /// a binding declared in that block, and a function's result may not
    /// borrow its own locals.
    fn check_escape(&mut self, e: ExprId, fn_body: bool) {
        let hir = self.inf.g.hir;
        match &hir.exprs[e].kind {
            ExprKind::Block { stmts, tail } => {
                let mut declared = vec![];
                for s in stmts {
                    if let Stmt::Let { pat, .. } = s {
                        pat_locals(hir, *pat, &mut declared);
                    }
                }
                // assigning a borrow of a block-local binding to an outer one
                for st in stmts {
                    if let Stmt::Assign { local, value, .. } = st
                        && !declared.contains(local)
                    {
                        let mut carried = vec![];
                        self.carried_borrows(*value, &mut carried);
                        for (owner, span) in carried {
                            if declared.contains(&owner) {
                                self.escape_error(owner, span);
                            }
                        }
                    }
                }
                if let Some(t) = tail {
                    let mut carried = vec![];
                    self.carried_borrows(*t, &mut carried);
                    for (owner, span) in carried {
                        if declared.contains(&owner) || (fn_body && !self.params.contains(&owner)) {
                            self.escape_error(owner, span);
                        }
                    }
                }
            }
            ExprKind::Return { value: Some(v), .. } => {
                let mut carried = vec![];
                self.carried_borrows(*v, &mut carried);
                for (owner, span) in carried {
                    if !self.params.contains(&owner) && hir.locals[owner].owner.is_some() {
                        self.escape_error(owner, span);
                    }
                }
            }
            ExprKind::Lambda(_) => return,
            _ => {}
        }
        for c in children(&hir.exprs[e].kind) {
            self.check_escape(c, false);
        }
    }

    fn escape_error(&mut self, owner: LocalId, span: Span) {
        let name = self.name(owner);
        self.report(
            Diagnostic::error(format!("borrow of `{name}` outlives it"))
                .with_code("E0407")
                .with_label(
                    span,
                    format!("`{name}` is dropped at the end of its scope, but this borrow escapes"),
                )
                .with_note("return the value itself, or borrow something owned by the caller"),
            span,
        );
    }

    /// Returns true if the expression diverges.
    fn expr(&mut self, e: ExprId, mode: Mode, st: &mut State) -> bool {
        let hir = self.inf.g.hir;
        let span = hir.exprs[e].span;
        match &hir.exprs[e].kind {
            ExprKind::Local(l) => {
                self.use_local(*l, span, mode, st);
                false
            }
            ExprKind::Def { .. } => {
                if let Some(Res::Callee { def, args }) = self.inf.res.get(&e).cloned() {
                    self.check_dup(def, &args, span);
                }
                false
            }
            ExprKind::Lit(_) | ExprKind::Ctor { .. } | ExprKind::AssocFn { .. } | ExprKind::Error => false,
            ExprKind::Interp(parts) => {
                for p in parts {
                    if let InterpPart::Expr(x) = p {
                        self.expr(*x, Mode::Borrow, st);
                    }
                }
                false
            }
            ExprKind::Field { base, .. } => {
                // taking an affine field out consumes the aggregate
                let m = if mode == Mode::Move && self.expr_affine(e) {
                    Mode::Move
                } else if mode == Mode::BorrowMut {
                    Mode::BorrowMut
                } else {
                    Mode::Borrow
                };
                self.expr(*base, m, st)
            }
            ExprKind::Index { base, index } => {
                let m = if mode == Mode::Move && self.expr_affine(e) { Mode::Move } else { Mode::Borrow };
                self.expr(*base, m, st) | self.expr(*index, Mode::Move, st)
            }
            ExprKind::Call { callee, args } => {
                let modes = match &hir.exprs[*callee].kind {
                    ExprKind::Def { def, .. } => self.param_modes(*def),
                    _ => vec![],
                };
                // a `once` or consuming closure is used up by its call
                let cm = match &hir.exprs[*callee].kind {
                    ExprKind::Local(l) if self.is_affine(*l) => Mode::Move,
                    _ => Mode::Borrow,
                };
                self.check_call_borrows(args);
                // a local generic closure that uses a parameter twice cannot
                // take an affine argument for it
                if let ExprKind::Local(l) = &hir.exprs[*callee].kind
                    && let Some(lam) = self.local_lambdas.get(l).copied()
                {
                    self.check_lambda_dup(lam, args, span);
                }
                let mut d = self.expr(*callee, cm, st);
                // calling a consuming closure held in a field or element
                // (`(r.f)()`, `fs[0]()`) uses up its holder
                if !matches!(hir.exprs[*callee].kind, ExprKind::Local(_))
                    && let Some(root) = self.place_root(*callee)
                    && self.consuming.contains(&root)
                {
                    self.use_local(root, span, Mode::Move, st);
                }
                for (i, a) in args.iter().enumerate() {
                    let p = modes.get(i).copied();
                    if p == Some(Param::Exclusive) {
                        self.check_exclusive_access(*a);
                    }
                    self.check_explicit_borrow(*a, p);
                    d |= self.expr(*a, Self::mode_of(p), st);
                    self.check_repeatable(*a, p);
                }
                d
            }
            ExprKind::MethodCall { recv, args, .. } => {
                let modes = match self.inf.res.get(&e).cloned() {
                    Some(Res::Method(Target::Fn { def, args: targs })) => {
                        self.check_dup(def, &targs, span);
                        self.param_modes(def)
                    }
                    _ => vec![Param::Shared],
                };
                self.check_stored_borrows(e, *recv, args);
                // calling a consuming closure stored in a field, or reading one
                // out of its holder (`m.get(k)`), uses up the holder
                let reads_fn = self.inf.exprs.get(&e).cloned().is_some_and(|t| self.holds_fn(&t, 0));
                let consumed_root = match self.inf.res.get(&e) {
                    Some(Res::Method(Target::Field(_))) => {
                        self.place_root(*recv).filter(|r| self.consuming.contains(r) && !st.contains_key(r))
                    }
                    _ if reads_fn => {
                        self.place_root(*recv).filter(|r| self.consuming.contains(r) && !st.contains_key(r))
                    }
                    _ => None,
                };
                let mut rm = Self::mode_of(Some(modes.first().copied().unwrap_or(Param::Shared)));
                if rm == Mode::BorrowMut {
                    self.check_exclusive_access(*recv);
                }
                // reading an affine element through a shared borrow would
                // duplicate it: the collection is consumed instead
                if rm == Mode::Borrow && self.expr_affine(e) && self.expr_affine(*recv) {
                    rm = Mode::Move;
                }
                // so do element-visiting methods of collections and sequences
                // of affine items (only observers keep the collection)
                if rm == Mode::Borrow
                    && self.expr_affine(*recv)
                    && let ExprKind::MethodCall { method, .. } = &hir.exprs[e].kind
                    && !matches!(
                        self.inf.sym(*method),
                        "len"
                            | "is_empty"
                            | "page_count"
                            | "element_address"
                            | "capacity"
                            | "storage_address"
                            | "is_full"
                    )
                    && self.elementwise_receiver(*recv)
                {
                    rm = Mode::Move;
                }
                let mut d = self.expr(*recv, rm, st);
                for (i, a) in args.iter().enumerate() {
                    let p = modes.get(i + 1).copied();
                    d |= self.expr(*a, Self::mode_of(p), st);
                    self.check_repeatable(*a, p);
                }
                if let Some(root) = consumed_root {
                    self.use_local(root, span, Mode::Move, st);
                }
                d
            }
            ExprKind::Try(x) => self.expr(*x, Mode::Move, st),
            ExprKind::Unary { op, operand } => {
                let m = match op {
                    UnOp::Ref => Mode::Borrow,
                    UnOp::RefMut => {
                        self.check_mut_place(*operand, span);
                        Mode::BorrowMut
                    }
                    _ => Mode::Move,
                };
                self.expr(*operand, m, st)
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let m = if op.is_comparison() { Mode::Borrow } else { Mode::Move };
                let a = self.expr(*lhs, m, st);
                if matches!(op, lang_hir::BinOp::And | lang_hir::BinOp::Or) {
                    let mut s2 = st.clone();
                    self.expr(*rhs, m, &mut s2);
                    Self::merge(st, &s2);
                    return a;
                }
                a | self.expr(*rhs, m, st)
            }
            ExprKind::Tuple(xs) | ExprKind::List(xs) | ExprKind::Set(xs) => {
                let mut d = false;
                for x in xs {
                    d |= self.expr(*x, Mode::Move, st);
                }
                d
            }
            ExprKind::Map(kv) => {
                let mut d = false;
                for (k, v) in kv {
                    d |= self.expr(*k, Mode::Move, st);
                    d |= self.expr(*v, Mode::Move, st);
                }
                d
            }
            ExprKind::Record { spread, fields } => {
                let mut d = false;
                if let Some(s) = spread {
                    d |= self.expr(*s, Mode::Move, st);
                }
                for (_, _, x) in fields {
                    // a field of borrowed type holds a copy of the reference
                    let ft = self.inf.exprs.get(x).cloned();
                    let m = match ft.map(|t| self.inf.icx.resolve(&t)) {
                        Some(t) if matches!(&*t, TyKind::Ref(..)) => Mode::Borrow,
                        _ => Mode::Move,
                    };
                    d |= self.expr(*x, m, st);
                }
                d
            }
            ExprKind::Lambda(l) => {
                // the body is checked on its own (captures start available);
                // consuming an affine capture makes the closure single-use
                let mut inner = State::default();
                self.expr(l.body, Mode::Move, &mut inner);
                // capture modes: a captured borrow is copied; a value the
                // body consumes, or one not used after the closure, moves
                // in; otherwise the closure borrows it (and holds that loan)
                let end = span.range().end;
                let mut borrowed = vec![];
                for c in &l.captures {
                    let is_ref = self.local_ty(*c).is_some_and(|t| matches!(&*t, TyKind::Ref(..)));
                    let consumed = inner.contains_key(c);
                    let used_after = self.last_use.get(c).is_some_and(|p| *p > end);
                    let mode = if is_ref || (!consumed && used_after) { Mode::Borrow } else { Mode::Move };
                    self.use_local(*c, span, mode, st);
                    if mode == Mode::Borrow {
                        borrowed.push(*c);
                    }
                }
                if !borrowed.is_empty() {
                    self.lambda_borrows.insert(e, borrowed);
                }
                if let Some(c) = l.captures.iter().find(|c| inner.contains_key(c) && self.is_affine(**c)) {
                    self.consuming_lambdas.insert(e, *c);
                }
                false
            }
            ExprKind::If { cond, then, els } => {
                if self.expr(*cond, Mode::Move, st) {
                    return true;
                }
                let mut a = st.clone();
                let da = self.expr(*then, mode, &mut a);
                let mut b = st.clone();
                let db = match els {
                    Some(x) => self.expr(*x, mode, &mut b),
                    None => false,
                };
                *st = match (da, db) {
                    (true, true) => return true,
                    (true, false) => b,
                    (false, true) => a,
                    (false, false) => {
                        Self::merge(&mut a, &b);
                        a
                    }
                };
                false
            }
            ExprKind::Match { scrutinee, arms } => {
                if self.expr(*scrutinee, Mode::Move, st) {
                    return true;
                }
                // closures taken out of a value holding a consuming closure
                if self.holds_consuming(*scrutinee) {
                    for a in arms {
                        self.mark_consuming_bindings(a.pat);
                    }
                }
                let base = st.clone();
                let mut out: Option<State> = None;
                for a in arms {
                    let mut s = base.clone();
                    // the arm's bindings are fresh each time it is entered
                    let mut bound = vec![];
                    pat_locals(hir, a.pat, &mut bound);
                    for b in bound {
                        s.remove(&b);
                    }
                    if let Some(g) = a.guard {
                        self.expr(g, Mode::Borrow, &mut s);
                    }
                    if !self.expr(a.body, mode, &mut s) {
                        match &mut out {
                            Some(o) => Self::merge(o, &s),
                            None => out = Some(s),
                        }
                    }
                }
                match out {
                    Some(o) => {
                        *st = o;
                        false
                    }
                    None => !arms.is_empty(),
                }
            }
            ExprKind::While { cond, body } => {
                for _ in 0..2 {
                    self.expr(*cond, Mode::Move, st);
                    let mut s = st.clone();
                    self.expr(*body, Mode::Move, &mut s);
                    Self::merge(st, &s);
                }
                false
            }
            ExprKind::For { iter, body, pat } => {
                self.expr(*iter, Mode::Move, st);
                let mut bound = vec![];
                pat_locals(hir, *pat, &mut bound);
                for _ in 0..2 {
                    let mut s = st.clone();
                    // the loop variable is bound afresh each iteration
                    for b in &bound {
                        s.remove(b);
                    }
                    self.expr(*body, Mode::Move, &mut s);
                    Self::merge(st, &s);
                }
                false
            }
            ExprKind::Loop { body } => {
                for _ in 0..2 {
                    let mut s = st.clone();
                    self.expr(*body, Mode::Move, &mut s);
                    Self::merge(st, &s);
                }
                false
            }
            ExprKind::Block { stmts, tail } => {
                for s in stmts {
                    let d = match s {
                        Stmt::Let { pat, init, span: lspan, .. } => {
                            // binding a reference copies it
                            let init_ty = self.inf.exprs.get(init).cloned();
                            let m = self.mode_into(init_ty);
                            let d = self.expr(*init, m, st);
                            // a sequence derived from an affine source is one-shot
                            if let PatKind::Bind(holder) = &hir.pats[*pat].kind
                                && self.seq_typed(*init)
                                && self.has_affine_part(*init)
                            {
                                self.consuming.insert(*holder);
                                self.affine.insert(*holder, true);
                            }
                            // a consuming closure kept inside an aggregate (or
                            // returned through a call) makes the holder single-use
                            if self.holds_consuming(*init) {
                                self.mark_consuming_bindings(*pat);
                            }
                            if let PatKind::Bind(holder) = &hir.pats[*pat].kind
                                && let ExprKind::Lambda(_) = &hir.exprs[*init].kind
                            {
                                self.local_lambdas.insert(*holder, *init);
                            }
                            // borrows the bound value carries
                            if let PatKind::Bind(holder) = &hir.pats[*pat].kind {
                                let mut carried = vec![];
                                self.carried_borrows(*init, &mut carried);
                                if !carried.is_empty() {
                                    self.carriers.insert(*holder, carried);
                                }
                            }
                            // a fresh binding (e.g. in each loop iteration) is
                            // initialized again
                            let mut bound = vec![];
                            pat_locals(hir, *pat, &mut bound);
                            for b in bound {
                                st.remove(&b);
                            }
                            // a bound `Result` must be read before it is replaced
                            if let PatKind::Bind(holder) = &hir.pats[*pat].kind
                                && self.local_ty(*holder).is_some_and(
                                    |t| matches!(&*t, TyKind::Con(d, _) if *d == self.inf.g.env.b.result),
                                )
                            {
                                self.unread.insert(*holder, hir.exprs[*init].span);
                            }
                            if let PatKind::Bind(holder) = &hir.pats[*pat].kind {
                                // a named borrow lives until the name's last use
                                if let ExprKind::Unary { op: op @ (UnOp::Ref | UnOp::RefMut), .. } =
                                    &hir.exprs[*init].kind
                                    && let Some((owner, _)) = self.borrowed_local(*init)
                                {
                                    self.loans.push(Loan {
                                        holder: *holder,
                                        owner,
                                        mutable: *op == UnOp::RefMut,
                                        start: hir.exprs[*init].span.range().end,
                                        span: *lspan,
                                    });
                                }
                                // a copy of a borrow, a borrow returned by a call,
                                // or an aggregate holding borrows keeps the
                                // borrowed places borrowed while it lives
                                let mut extra: Vec<(LocalId, bool, Span)> = vec![];
                                match &hir.exprs[*init].kind {
                                    ExprKind::Local(src) => {
                                        for ln in self.loans.iter().filter(|ln| ln.holder == *src) {
                                            extra.push((ln.owner, ln.mutable, ln.span));
                                        }
                                    }
                                    ExprKind::Call { .. } | ExprKind::MethodCall { .. } => {
                                        let holds_ref = self.inf.exprs.get(init).cloned().is_some_and(|t| {
                                            let t = self.inf.icx.resolve(&t);
                                            crate::ty::ty_any(&t, &mut |k| matches!(k, TyKind::Ref(..)))
                                        });
                                        if holds_ref {
                                            self.carried_call_borrows(*init, &mut extra);
                                        }
                                    }
                                    _ => {}
                                }
                                if let Some(cs) = self.carriers.get(holder).cloned() {
                                    for (owner, sp) in cs {
                                        let mutable = matches!(
                                            &hir.exprs
                                                .iter()
                                                .find(|(_, x)| x.span == sp)
                                                .map(|(_, x)| &x.kind),
                                            Some(ExprKind::Unary { op: UnOp::RefMut, .. })
                                        );
                                        extra.push((owner, mutable, sp));
                                    }
                                }
                                for (owner, mutable, _) in extra {
                                    if owner != *holder {
                                        self.loans.push(Loan {
                                            holder: *holder,
                                            owner,
                                            mutable,
                                            start: hir.exprs[*init].span.range().end,
                                            span: *lspan,
                                        });
                                    }
                                }
                                if let Some(c) = self.consuming_lambdas.get(init).copied() {
                                    let _ = c;
                                    self.consuming.insert(*holder);
                                    self.affine.insert(*holder, true);
                                }
                            }
                            d
                        }
                        Stmt::Assign { local, value, span: aspan } => {
                            let d = self.expr(*value, Mode::Move, st);
                            self.check_loans(*local, *aspan, Mode::BorrowMut);
                            st.remove(local);
                            self.overwrite_result(*local, *aspan);
                            // an assigned value carrying borrows keeps them
                            // borrowed while the variable is used
                            let mut carried = vec![];
                            self.carried_borrows(*value, &mut carried);
                            if !carried.is_empty() {
                                let mutable = self.mentions_mut_borrow(*value);
                                for (owner, sp) in &carried {
                                    if *owner != *local {
                                        self.loans.push(Loan {
                                            holder: *local,
                                            owner: *owner,
                                            mutable,
                                            start: hir.exprs[*value].span.range().end,
                                            span: *sp,
                                        });
                                    }
                                }
                                self.carriers.entry(*local).or_default().extend(carried);
                            }
                            d
                        }
                        Stmt::Expr { expr, .. } => self.expr(*expr, Mode::Borrow, st),
                    };
                    if d {
                        return true;
                    }
                }
                match tail {
                    Some(t) => self.expr(*t, mode, st),
                    None => false,
                }
            }
            ExprKind::Return { value, .. } | ExprKind::Break { value, .. } => {
                if let Some(v) = value {
                    let m = if matches!(&hir.exprs[e].kind, ExprKind::Return { .. }) {
                        self.mode_into(self.ret_ty.clone())
                    } else {
                        Mode::Move
                    };
                    self.expr(*v, m, st);
                }
                true
            }
            ExprKind::Continue { .. } => true,
            ExprKind::Unsafe(b) => self.expr(*b, mode, st),
        }
    }
}
