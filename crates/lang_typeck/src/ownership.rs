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
    /// Where the holder was given another value (the loan ends there).
    until: usize,
    /// The block whose statements began the loan: only an assignment
    /// there (not one on some paths) ends it.
    block: Option<ExprId>,
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
    /// Blocks being walked, innermost last.
    blocks: Vec<ExprId>,
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
        blocks: vec![],
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
        o.mark_once_holders(&ls);
        o.params.extend(ls);
    }
    // postconditions run after the body, reading what it leaves
    let posts: Vec<ExprId> = match &hir.defs[d].kind {
        DefKind::Fn(f) => f.ensures.iter().map(|p| p.cond).collect(),
        _ => vec![],
    };
    o.collect_uses(body);
    for p in &posts {
        o.collect_uses(*p);
    }
    let mut st = State::default();
    let m = o.mode_into(o.ret_ty.clone());
    o.expr(body, m, &mut st);
    for p in &posts {
        o.expr(*p, Mode::Borrow, &mut st);
    }
    // the function returns `result` after its postconditions: they may
    // read it, not consume it
    if let DefKind::Fn(f) = &hir.defs[d].kind
        && let Some(r) = f.result
        && let Some(sp) = st.get(&r).copied()
    {
        o.report(
            Diagnostic::error("a postcondition consumes `result`, which the function then returns")
                .with_code("E0404")
                .with_label(sp, "`result` is moved here")
                .with_note("a postcondition can read the returned value but not take it; borrow it instead"),
            sp,
        );
    }
    o.check_escape(body, true);
    o.unused_tasks(body);
    o.check_consuming_result(body);
    // a var's last assigned `Result` that nothing reads afterwards
    let mut left: Vec<(LocalId, Span)> = o.unread.iter().map(|(l, s)| (*l, *s)).collect();
    left.sort_by_key(|x| x.1.range().start);
    for (l, span) in left {
        if hir.locals[l].mutable && o.reads.contains(&l) && !o.name(l).starts_with('_') {
            let name = o.name(l);
            o.report(
                Diagnostic::error(format!("the `Result` last assigned to `{name}` is never inspected"))
                    .with_code("E0337")
                    .with_label(span, "this `Result` is ignored")
                    .with_note("handle it with `?` or `match` before the variable goes out of use"),
                span,
            );
        }
    }
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
            // locals computed from a parameter's value (`for g in gs`,
            // `let`, `match`, `var` assignments, closures over it) stand for
            // that parameter, when their type can carry a closure or other
            // confined state (plain data computed from it cannot)
            let mut derived: FxHashMap<LocalId, Vec<usize>> = FxHashMap::default();
            let param_of = |l: LocalId, derived: &FxHashMap<LocalId, Vec<usize>>| -> Vec<usize> {
                match params.iter().position(|p| *p == Some(l)) {
                    Some(i) => vec![i],
                    None => derived.get(&l).cloned().unwrap_or_default(),
                }
            };
            let mentions = |e: ExprId, derived: &FxHashMap<LocalId, Vec<usize>>| -> Vec<usize> {
                let mut out: Vec<usize> = vec![];
                crate::program::walk_exprs(hir, e, &mut |x| {
                    let ls: Vec<LocalId> = match &hir.exprs[x].kind {
                        ExprKind::Local(l) => vec![*l],
                        ExprKind::Lambda(lam) => lam.captures.clone(),
                        _ => vec![],
                    };
                    for l in ls {
                        for i in param_of(l, derived) {
                            if !out.contains(&i) {
                                out.push(i);
                            }
                        }
                    }
                });
                out
            };
            loop {
                let mut grew = false;
                let mut binds: Vec<(Vec<LocalId>, ExprId)> = vec![];
                crate::program::walk_exprs(hir, body, &mut |e| match &hir.exprs[e].kind {
                    ExprKind::For { pat, iter, .. } => {
                        let mut ls = vec![];
                        pat_locals(hir, *pat, &mut ls);
                        binds.push((ls, *iter));
                    }
                    ExprKind::Match { scrutinee, arms } => {
                        for a in arms {
                            let mut ls = vec![];
                            pat_locals(hir, a.pat, &mut ls);
                            binds.push((ls, *scrutinee));
                        }
                    }
                    ExprKind::Block { stmts, .. } => {
                        for s in stmts {
                            match s {
                                Stmt::Let { pat, init, .. } => {
                                    let mut ls = vec![];
                                    pat_locals(hir, *pat, &mut ls);
                                    binds.push((ls, *init));
                                }
                                Stmt::Assign { local, value, .. } => binds.push((vec![*local], *value)),
                                Stmt::Expr { .. } => {}
                            }
                        }
                    }
                    _ => {}
                });
                for (ls, src) in binds {
                    let from = mentions(src, &derived);
                    if from.is_empty() {
                        continue;
                    }
                    for l in ls {
                        let carries = inf.locals.get(&l).cloned().is_some_and(|t| {
                            let t = inf.icx.resolve(&t);
                            carries_code(inf.g, &t, &mut vec![])
                        });
                        if !carries {
                            continue;
                        }
                        let e = derived.entry(l).or_default();
                        for i in &from {
                            if !e.contains(i) {
                                e.push(*i);
                                grew = true;
                            }
                        }
                    }
                }
                if !grew {
                    break;
                }
            }
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
                    for pi in mentions(a, &derived) {
                        if !found.contains(&pi) {
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

/// Whether a value of type `t` can carry code or confined state: a
/// function, `dyn` value, type parameter, cell, or a structure holding one.
fn carries_code(g: &crate::program::Global<'_>, t: &Ty, seen: &mut Vec<DefId>) -> bool {
    match &**t {
        TyKind::Fn(_) | TyKind::Dyn(..) | TyKind::Skolem(_) | TyKind::Var(_) | TyKind::Gen(_) => true,
        TyKind::Con(d, args) => {
            if *d == g.env.b.cell || args.iter().any(|a| carries_code(g, a, seen)) {
                return true;
            }
            if seen.contains(d) {
                return false;
            }
            seen.push(*d);
            let fields: Vec<Ty> = if let Some(e) = g.env.enums.get(d) {
                (0..e.variants.len())
                    .flat_map(|v| g.env.variant_fields(*d, v as u32, args).into_iter().map(|f| f.1))
                    .collect()
            } else if g.env.structs.contains_key(d) {
                g.env.struct_fields(*d, args).into_iter().map(|f| f.1).collect()
            } else {
                vec![]
            };
            let r = fields.iter().any(|f| carries_code(g, f, seen));
            seen.pop();
            r
        }
        TyKind::Tuple(ts) => ts.iter().any(|x| carries_code(g, x, seen)),
        TyKind::Record(r) => row_fields(r).0.iter().any(|(_, x)| carries_code(g, x, seen)),
        TyKind::Ref(_, x) => carries_code(g, x, seen),
        _ => false,
    }
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
    // affine values captured by closures that user functions hand to
    // tasks were moved there: later uses are errors
    let mut seen: FxHashSet<(LocalId, Span)> = FxHashSet::default();
    for (def, i, later) in std::mem::take(&mut inf.task_moves) {
        if !tp.get(&def).is_some_and(|v| v.contains(&i)) {
            continue;
        }
        let callee = inf.g.interner.resolve(hir.defs[def].name).to_string();
        for (l, span) in later {
            if !seen.insert((l, span)) {
                continue;
            }
            let name = inf.g.interner.resolve(hir.locals[l].name).to_string();
            inf.push_diag(
                Diagnostic::error(format!("use of moved value `{name}`"))
                    .with_code("E0404")
                    .with_label(span, format!("`{name}` moves into a task here: `{callee}` hands this closure to one"))
                    .with_note("a closure running in another task takes its affine captures; use the value before, or not at all after"),
            );
        }
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
    // initializers of local bindings (a closure may call another closure
    // bound earlier)
    let mut inits: FxHashMap<LocalId, ExprId> = FxHashMap::default();
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
            if let ExprKind::Block { stmts, .. } = &hir.exprs[e].kind {
                for st in stmts {
                    if let Stmt::Let { pat, init, .. } = st
                        && let PatKind::Bind(l) = &hir.pats[*pat].kind
                    {
                        inits.insert(*l, *init);
                    }
                }
            }
        });
    }
    // the state a value captures: through closures (and closures they
    // capture), aggregates, and calls that may return what they are given
    fn captured_state(
        inf: &mut Infer<'_, '_>,
        e: ExprId,
        returning: &FxHashMap<DefId, String>,
        inits: &FxHashMap<LocalId, ExprId>,
        depth: usize,
    ) -> Option<String> {
        if depth > 8 {
            return None;
        }
        let hir = inf.g.hir;
        let ty_state = |inf: &mut Infer<'_, '_>, e: ExprId| -> Option<String> {
            let t = inf.exprs.get(&e).cloned()?;
            let t = inf.icx.resolve(&t);
            crate::program::stored_state(inf.g, &t, &mut vec![])
        };
        match &hir.exprs[e].kind {
            ExprKind::Lambda(l) => {
                for c in l.captures.clone() {
                    if let Some(t) = inf.locals.get(&c).cloned() {
                        let t = inf.icx.resolve(&t);
                        if let Some(s) = crate::program::stored_state(inf.g, &t, &mut vec![]) {
                            return Some(s);
                        }
                    }
                    if let Some(&init) = inits.get(&c)
                        && let Some(s) = captured_state(inf, init, returning, inits, depth + 1)
                    {
                        return Some(s);
                    }
                }
                None
            }
            ExprKind::Local(l) => {
                let init = inits.get(l).copied()?;
                captured_state(inf, init, returning, inits, depth + 1)
            }
            ExprKind::Call { callee, args } => {
                if let Some(Res::Callee { def, .. }) = inf.res.get(callee)
                    && let Some(s) = returning.get(def)
                {
                    return Some(s.clone());
                }
                // a call returning a function may return (a closure over)
                // what it was given
                let returns_fn = inf.exprs.get(&e).cloned().is_some_and(|t| {
                    let t = inf.icx.resolve(&t);
                    ty_any(&t, &mut |k| matches!(k, TyKind::Fn(_)))
                });
                if !returns_fn {
                    return None;
                }
                for a in args.clone() {
                    if let Some(s) =
                        ty_state(inf, a).or_else(|| captured_state(inf, a, returning, inits, depth + 1))
                    {
                        return Some(s);
                    }
                }
                None
            }
            ExprKind::Record { fields, .. } => {
                let xs: Vec<ExprId> = fields.iter().map(|f| f.2).collect();
                xs.into_iter().find_map(|x| captured_state(inf, x, returning, inits, depth + 1))
            }
            ExprKind::Tuple(xs) | ExprKind::List(xs) => {
                xs.clone().into_iter().find_map(|x| captured_state(inf, x, returning, inits, depth + 1))
            }
            ExprKind::Map(kv) => {
                kv.clone().into_iter().find_map(|(_, v)| captured_state(inf, v, returning, inits, depth + 1))
            }
            ExprKind::Block { tail: Some(t), .. } => captured_state(inf, *t, returning, inits, depth + 1),
            ExprKind::If { then, els, .. } => captured_state(inf, *then, returning, inits, depth + 1)
                .or_else(|| els.and_then(|x| captured_state(inf, x, returning, inits, depth + 1))),
            ExprKind::Match { arms, .. } => {
                let bodies: Vec<ExprId> = arms.iter().map(|a| a.body).collect();
                bodies.into_iter().find_map(|b| captured_state(inf, b, returning, inits, depth + 1))
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
            if let Some(s) = captured_state(inf, body, &returning, &inits, 0) {
                returning.insert(id, s);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    // `dyn` values hide their type: a stateful value coerced to `dyn` (in
    // the initializer, or in a function returning it) is state too
    let stateful_dyn =
        |inf: &mut Infer<'_, '_>, body: ExprId, via: &FxHashMap<DefId, String>| -> Option<String> {
            let mut found = None;
            crate::program::walk_exprs(hir, body, &mut |x| {
                if found.is_some() {
                    return;
                }
                if matches!(inf.coercions.get(&x), Some(crate::Coercion::Dyn { .. }))
                    && let Some(t) = inf.exprs.get(&x).cloned()
                {
                    let t = inf.icx.resolve(&t);
                    found = crate::program::stored_state(inf.g, &t, &mut vec![]);
                }
                if found.is_none()
                    && let ExprKind::Call { callee, .. } = &hir.exprs[x].kind
                    && let Some(Res::Callee { def, .. }) = inf.res.get(callee)
                {
                    found = via.get(def).cloned();
                }
            });
            found
        };
    let returns_dyn = |inf: &mut Infer<'_, '_>, d: DefId| -> bool {
        inf.g.scheme(d).is_some_and(|s| match &*s.ty {
            TyKind::Fn(f) => ty_any(&f.ret, &mut |k| matches!(k, TyKind::Dyn(..))),
            _ => false,
        })
    };
    let mut dyn_state: FxHashMap<DefId, String> = FxHashMap::default();
    loop {
        let mut grew = false;
        for (id, def) in hir.defs.iter() {
            if hir.modules[def.module].is_std || dyn_state.contains_key(&id) {
                continue;
            }
            let DefKind::Fn(f) = &def.kind else { continue };
            let Some(body) = f.body else { continue };
            if !returns_dyn(inf, id) {
                continue;
            }
            if let Some(s) = stateful_dyn(inf, body, &dyn_state) {
                dyn_state.insert(id, s);
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    let mut reports = vec![];
    for (id, def) in hir.defs.iter() {
        if hir.modules[def.module].is_std {
            continue;
        }
        let DefKind::Global(g) = &def.kind else { continue };
        if let Some(s) = captured_state(inf, g.value, &returning, &inits, 0) {
            reports.push((def.span, inf.g.interner.resolve(def.name).to_string(), s));
            continue;
        }
        let holds_dyn =
            inf.g.scheme(id).is_some_and(|s| ty_any(&s.ty, &mut |k| matches!(k, TyKind::Dyn(..))));
        if holds_dyn && let Some(s) = stateful_dyn(inf, g.value, &dyn_state) {
            reports.push((def.span, inf.g.interner.resolve(def.name).to_string(), s));
        }
    }
    for (span, name, s) in reports {
        inf.push_diag(
            Diagnostic::error(format!("module-level value `{name}` holds a closure or `dyn` value over a `{s}`"))
                .with_code("E0138")
                .with_label(span, "module values are pure immutable data shared by every task")
                .with_note("create mutable state and acquire resources in functions or system initialization, and pass them where they are needed"),
        );
    }
}

/// Calls recorded before their generic callee was analysed.
pub fn check_deferred_dups(inf: &mut Infer<'_, '_>) {
    let hir = inf.g.hir;
    // analyse generic bodies again until the duplication flags settle: a
    // generic function calling a protocol method or another generic
    // function analysed after it learns what they duplicate
    let fns: Vec<DefId> = hir
        .defs
        .iter()
        .filter(|(_, d)| matches!(&d.kind, DefKind::Fn(f) if f.body.is_some()))
        .map(|(id, _)| id)
        .collect();
    for _ in 0..4 {
        let before = inf.g.dup_params.borrow().clone();
        // a protocol method duplicates `Self` when an implementation does
        for imp in &inf.g.env.impls {
            let (Some((proto, _)), crate::env::ImplMethods::Defs(methods)) = (&imp.protocol, &imp.methods)
            else {
                continue;
            };
            let DefKind::Protocol(pd) = &hir.defs[*proto].kind else { continue };
            for (name, m) in methods {
                if inf.g.dup_params.borrow().get(m).is_some_and(|v| !v.is_empty())
                    && let Some(&pm) = pd.methods.iter().find(|pm| hir.defs[**pm].name == *name)
                {
                    inf.g.dup_params.borrow_mut().insert(pm, vec![0]);
                }
            }
        }
        for &d in &fns {
            if let Some(s) = inf.g.scheme(d) {
                generic_pass(inf, d, &s);
            }
        }
        if *inf.g.dup_params.borrow() == before {
            break;
        }
    }
    inf.g.dups_final.set(true);
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
        blocks: vec![],
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
    if scheme.n == 0 || trusted_std(inf, d, TRUSTED_SINGLE_USE) {
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
        blocks: vec![],
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
        // a local from outside a loop that the loop uses is used again on
        // the next iteration: it lives to the loop's end
        if matches!(hir.exprs[e].kind, ExprKind::While { .. } | ExprKind::For { .. } | ExprKind::Loop { .. })
        {
            let (start, end) = (span.range().start, span.range().end);
            let mut inner = vec![];
            crate::program::walk_exprs(hir, e, &mut |x| match &hir.exprs[x].kind {
                ExprKind::Local(l) => inner.push(*l),
                ExprKind::Lambda(lam) => inner.extend(lam.captures.iter().copied()),
                _ => {}
            });
            for l in inner {
                if hir.locals[l].span.range().start < start {
                    self.bump(l, end);
                }
            }
        }
    }

    /// Whether a value of this type holds a `Result` (directly, through an
    /// option, collection, tuple, record, or a captured task's `Exit`); a
    /// task handle itself is checked as a task.
    fn holds_result(&mut self, t: &Ty) -> bool {
        let t = self.inf.icx.resolve(t);
        let result = self.inf.g.env.b.result;
        let task = self.inf.g.hir.try_lang("task.Task");
        if matches!(&*t, TyKind::Con(d, _) if Some(*d) == task) {
            return false;
        }
        let mut found = false;
        let mut stack = vec![t];
        let mut seen: Vec<DefId> = vec![];
        while let Some(x) = stack.pop() {
            match &*x {
                TyKind::Con(d, _) if *d == result => found = true,
                TyKind::Con(d, args) if Some(*d) != task => {
                    stack.extend(args.iter().cloned());
                    // a user struct or enum holding one in a field or payload
                    let user = !self.inf.g.hir.modules[self.inf.g.hir.defs[*d].module].is_std;
                    if user && !seen.contains(d) {
                        seen.push(*d);
                        let env = &self.inf.g.env;
                        if let Some(e) = env.enums.get(d) {
                            for v in 0..e.variants.len() {
                                stack.extend(env.variant_fields(*d, v as u32, args).into_iter().map(|f| f.1));
                            }
                        } else if env.structs.contains_key(d) {
                            stack.extend(env.struct_fields(*d, args).into_iter().map(|f| f.1));
                        }
                    }
                }
                TyKind::Tuple(ts) => stack.extend(ts.iter().cloned()),
                TyKind::Record(r) => stack.extend(row_fields(r).0.into_iter().map(|f| f.1)),
                _ => {}
            }
        }
        found
    }

    /// A named fallible task that is never used again loses its error.
    fn unused_tasks(&mut self, e: ExprId) {
        let hir = self.inf.g.hir;
        // a `match` arm binding a `Result` it never reads ignores it
        if let ExprKind::Match { arms, .. } = &hir.exprs[e].kind {
            for a in arms.clone() {
                let mut ls = vec![];
                pat_locals(hir, a.pat, &mut ls);
                for l in ls {
                    let unread = !self.reads.contains(&l)
                        && !self.name(l).starts_with('_')
                        && self.local_ty(l).is_some_and(|t| self.holds_result(&t));
                    if unread {
                        let span = hir.locals[l].span;
                        let name = self.name(l);
                        self.report(
                            Diagnostic::error(format!("`{name}` holds a `Result` that is never inspected"))
                                .with_code("E0337")
                                .with_label(span, "this `Result` is ignored")
                                .with_note("handle it, or bind it to `_` to discard explicitly"),
                            span,
                        );
                    }
                }
            }
        }
        if let ExprKind::Block { stmts, .. } = &hir.exprs[e].kind {
            for s in stmts {
                let Stmt::Let { pat, .. } = s else { continue };
                // `Result`s bound by destructuring and never read
                if !matches!(hir.pats[*pat].kind, PatKind::Bind(_)) {
                    let mut ls = vec![];
                    pat_locals(hir, *pat, &mut ls);
                    for l in ls {
                        let unread_result = !self.reads.contains(&l)
                            && !self.name(l).starts_with('_')
                            && self.local_ty(l).is_some_and(|t| self.holds_result(&t));
                        if unread_result {
                            let span = hir.locals[l].span;
                            let name = self.name(l);
                            self.report(
                                Diagnostic::error(format!(
                                    "`{name}` holds a `Result` that is never inspected"
                                ))
                                .with_code("E0337")
                                .with_label(span, "this `Result` is ignored")
                                .with_note(
                                    "handle it with `?` or `match`, or bind it to `_` to discard explicitly",
                                ),
                                span,
                            );
                        }
                    }
                    continue;
                }
                let PatKind::Bind(l) = &hir.pats[*pat].kind else { continue };
                // a bound `Result` (or a value holding one) never read is ignored
                if !self.reads.contains(l)
                    && !self.name(*l).starts_with('_')
                    && let Some(t) = self.local_ty(*l)
                    && self.holds_result(&t)
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
                    && self.holds_fallible_task(&t)
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
                let r = self.result_of_task(&args[1]);
                if self.holds_result(&r) {
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

    /// A task's result type, looking through a captured task's `Exit<T>`.
    fn result_of_task(&mut self, t: &Ty) -> Ty {
        let r = self.inf.icx.resolve(t);
        if let TyKind::Con(xd, xa) = &*r
            && Some(*xd) == self.inf.g.hir.try_lang("task.Exit")
            && let Some(inner) = xa.first()
        {
            return self.inf.icx.resolve(inner);
        }
        r
    }

    /// Whether a type holds a task whose result is a `Result`.
    fn holds_fallible_task(&mut self, t: &Ty) -> bool {
        let t = self.inf.icx.resolve(t);
        let hir = self.inf.g.hir;
        match &*t {
            TyKind::Con(d, args) if Some(*d) == hir.try_lang("task.Task") && args.len() == 2 => {
                let r = self.result_of_task(&args[1]);
                self.holds_result(&r)
            }
            TyKind::Con(_, args) | TyKind::Tuple(args) => {
                args.clone().iter().any(|a| self.holds_fallible_task(a))
            }
            TyKind::Record(r) => row_fields(r).0.iter().any(|(_, x)| self.holds_fallible_task(x)),
            _ => false,
        }
    }

    /// An `if`/`else` or `match` statement assigning a variable in every
    /// branch ends the loans the variable held before it.
    fn end_loans_assigned_in_all_branches(&mut self, e: ExprId) {
        let hir = self.inf.g.hir;
        let branches: Vec<ExprId> = match &hir.exprs[e].kind {
            ExprKind::If { then, els: Some(els), .. } => vec![*then, *els],
            ExprKind::Match { arms, .. } if !arms.is_empty() => arms.iter().map(|a| a.body).collect(),
            _ => return,
        };
        fn assigned(hir: &lang_hir::Hir, b: ExprId, out: &mut Vec<LocalId>) {
            match &hir.exprs[b].kind {
                ExprKind::Block { stmts, .. } => {
                    for s in stmts {
                        match s {
                            Stmt::Assign { local, .. } => out.push(*local),
                            Stmt::Expr { expr, .. } => {
                                // a nested `if`/`match` assigning in all its branches
                                let inner: Vec<ExprId> = match &hir.exprs[*expr].kind {
                                    ExprKind::If { then, els: Some(els), .. } => vec![*then, *els],
                                    ExprKind::Match { arms, .. } if !arms.is_empty() => {
                                        arms.iter().map(|a| a.body).collect()
                                    }
                                    _ => vec![],
                                };
                                let mut sets = inner.iter().map(|x| {
                                    let mut v = vec![];
                                    assigned(hir, *x, &mut v);
                                    v
                                });
                                if let Some(first) = sets.next() {
                                    let rest: Vec<Vec<LocalId>> = sets.collect();
                                    out.extend(
                                        first.into_iter().filter(|l| rest.iter().all(|r| r.contains(l))),
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                }
                ExprKind::If { then, els: Some(els), .. } => {
                    let (mut a, mut b2) = (vec![], vec![]);
                    assigned(hir, *then, &mut a);
                    assigned(hir, *els, &mut b2);
                    out.extend(a.into_iter().filter(|l| b2.contains(l)));
                }
                _ => {}
            }
        }
        let mut sets = branches.iter().map(|b| {
            let mut v = vec![];
            assigned(hir, *b, &mut v);
            v
        });
        let Some(first) = sets.next() else { return };
        let rest: Vec<Vec<LocalId>> = sets.collect();
        let all: Vec<LocalId> = first.into_iter().filter(|l| rest.iter().all(|r| r.contains(l))).collect();
        let span = hir.exprs[e].span.range();
        let here = self.blocks.last().copied();
        for ln in self.loans.iter_mut() {
            if all.contains(&ln.holder) && ln.start <= span.start && ln.block == here {
                ln.until = ln.until.min(span.end);
            }
        }
    }

    /// Loans made in a loop's first pass (from `n` on) whose holder lives
    /// outside the loop are still held when the next iteration starts.
    fn loans_carried_around(&mut self, lp: ExprId, n: usize) {
        let hir = self.inf.g.hir;
        let start = hir.exprs[lp].span.range().start;
        for ln in &mut self.loans[n..] {
            if hir.locals[ln.holder].span.range().start < start {
                ln.start = ln.start.min(start);
            }
        }
    }

    fn bump(&mut self, l: LocalId, pos: usize) {
        let e = self.last_use.entry(l).or_insert(0);
        *e = (*e).max(pos);
    }

    fn is_affine_ty(&mut self, t: &Ty) -> bool {
        self.affine_in(t, &mut vec![])
    }

    /// An affine part anywhere inside: a type that keeps growing as it
    /// recurs counts as affine rather than as freely duplicable.
    fn affine_in(&mut self, t: &Ty, seen: &mut Vec<Ty>) -> bool {
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
                    return args.iter().any(|a| self.affine_in(a, seen));
                }
                match expand(seen, &t) {
                    Expand::Cycle => return false,
                    Expand::Deep => return args.iter().any(|a| self.affine_in(a, seen)),
                    Expand::Go => {}
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
                let r = fields.iter().any(|f| self.affine_in(f, seen));
                seen.pop();
                r
            }
            TyKind::Fn(f) => f.once,
            TyKind::Skolem(s) if self.generic => {
                !self.inf.icx.skolems.get(*s as usize).is_some_and(|k| k.region)
            }
            TyKind::Var(_) | TyKind::Gen(_) if self.generic => true,
            TyKind::Tuple(ts) => ts.iter().any(|x| self.affine_in(x, seen)),
            TyKind::Record(r) => row_fields(r).0.iter().any(|(_, x)| self.affine_in(x, seen)),
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
            Some(t) => self.is_affine_ty(&t),
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
            Some(r) => !matches!(&*r, TyKind::Ref(..)) && self.is_affine_ty(&r),
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
                    self.is_affine_ty(&inner)
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
                        | "channel.ReplySender"
                        | "Sender"
                        | "BatchSender"
                        | "ReplySender"
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
            _ => self.holds_fn(&ret),
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
            if self.local_ty(b).is_some_and(|t| self.holds_fn(&t)) {
                self.consuming.insert(b);
                self.affine.insert(b, true);
            }
        }
    }

    /// Whether a type holds a function (directly, in fields, payloads, or
    /// elements).
    fn holds_fn(&mut self, t: &Ty) -> bool {
        self.fn_in(t, &mut vec![])
    }

    fn fn_in(&mut self, t: &Ty, seen: &mut Vec<Ty>) -> bool {
        let t = self.inf.icx.resolve(t);
        match &*t {
            TyKind::Fn(_) => true,
            TyKind::Con(d, args) => {
                match expand(seen, &t) {
                    Expand::Cycle => return false,
                    Expand::Deep => return args.iter().any(|a| self.fn_in(a, seen)),
                    Expand::Go => {}
                }
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
                let r = fields.iter().any(|f| self.fn_in(f, seen));
                seen.pop();
                r
            }
            TyKind::Tuple(ts) => ts.clone().iter().any(|x| self.fn_in(x, seen)),
            TyKind::Record(r) => row_fields(r).0.iter().any(|(_, x)| self.fn_in(x, seen)),
            _ => false,
        }
    }

    /// A closure passed where the callee requires `Transfer` (it runs in
    /// another task: `spawn`, `capture`, `timeout`) takes its affine
    /// captures by move, never by borrow.
    fn moves_into_task(&mut self, def: Option<DefId>, i: usize, arg: ExprId, span: Span, st: &mut State) {
        let Some(def) = def else { return };
        let hir = self.inf.g.hir;
        // the closure itself, or a local bound to it
        let lam = match &hir.exprs[arg].kind {
            ExprKind::Lambda(_) => arg,
            ExprKind::Local(l) => match self.local_lambdas.get(l).copied() {
                Some(lam) => lam,
                None => return,
            },
            _ => return,
        };
        let Some(scheme) = self.inf.g.scheme(def) else { return };
        let TyKind::Fn(f) = &*scheme.ty else { return };
        let Some(pt) = f.params.get(i) else { return };
        let transfer = hir.try_lang("Transfer");
        let required = scheme.constraints.iter().any(|c| {
            matches!(c, crate::solve::Constraint::Impl { proto, ty, .. } if Some(*proto) == transfer && ty == pt)
        });
        let captured: Vec<LocalId> = self
            .lambda_borrows
            .get(&lam)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|c| self.is_affine(*c))
            .collect();
        if !required {
            // a user function may still hand it to a task: decided once
            // every body is checked (`check_task_params`)
            if !captured.is_empty() && !hir.modules[hir.defs[def].module].is_std {
                let after = span.range().end;
                let later: Vec<(LocalId, Span)> = captured
                    .iter()
                    .filter(|c| self.last_use.get(c).copied().unwrap_or(0) > after)
                    .map(|c| (*c, span))
                    .collect();
                if !later.is_empty() {
                    self.inf.task_moves.push((def, i, later));
                }
            }
            return;
        }
        // captured values of a type parameter must be transferable where the
        // function is instantiated
        if let ExprKind::Lambda(l) = &hir.exprs[lam].kind {
            for c in l.captures.clone() {
                if let Some(t) = self.local_ty(c) {
                    let skolems = self.inf.icx.skolems.clone();
                    let mut found = vec![];
                    ty_any(&t, &mut |k| {
                        if let TyKind::Skolem(s) = k
                            && !skolems.get(*s as usize).is_some_and(|i| i.region)
                        {
                            found.push(*s);
                        }
                        false
                    });
                    for s in found {
                        self.inf.implied_caps.push((s, "Transfer"));
                    }
                }
            }
        }
        for c in captured {
            self.use_local(c, span, Mode::Move, st);
        }
        self.lambda_borrows.remove(&lam);
    }

    /// Whether an expression is a borrow (`&T`, `&mut T`) of an affine value.
    fn ref_to_affine(&mut self, e: ExprId) -> bool {
        let Some(t) = self.inf.exprs.get(&e).cloned() else { return false };
        let t = self.inf.icx.resolve(&t);
        match &*t {
            TyKind::Ref(_, inner) => {
                let inner = inner.clone();
                self.is_affine_ty(&inner)
            }
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
    fn holds_consuming(&mut self, e: ExprId) -> bool {
        let hir = self.inf.g.hir;
        if self.consuming_lambdas.contains_key(&e) {
            return true;
        }
        match &hir.exprs[e].kind {
            // a consuming holder, or a value typed with a `once fn` inside
            ExprKind::Local(l) => {
                self.consuming.contains(l)
                    || self
                        .local_ty(*l)
                        .is_some_and(|t| crate::ty::ty_any(&t, &mut |k| matches!(k, TyKind::Fn(f) if f.once)))
            }
            ExprKind::Lambda(_) => false,
            k => children(k).into_iter().any(|c| self.holds_consuming(c)),
        }
    }

    /// Mark a parameter or a local holding a `once fn` inside an aggregate
    /// (`List<once fn ...>`) as single-use.
    fn mark_once_holders(&mut self, locals: &[LocalId]) {
        for &l in locals {
            let holds_once = self.local_ty(l).is_some_and(|t| {
                !matches!(&*t, TyKind::Fn(_))
                    && crate::ty::ty_any(&t, &mut |k| matches!(k, TyKind::Fn(f) if f.once))
            });
            if holds_once {
                self.consuming.insert(l);
                self.affine.insert(l, true);
            }
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

    /// A call through a local alias of a generic function: its type
    /// arguments follow from the arguments at the scheme's parameters.
    fn check_alias_dup(&mut self, def: DefId, args: &[ExprId], span: Span) {
        let Some(scheme) = self.inf.g.scheme(def) else { return };
        let TyKind::Fn(f) = &*scheme.ty else { return };
        let mut inst: Vec<Ty> = vec![crate::ty::error_ty(); scheme.n as usize];
        for (p, a) in f.params.iter().zip(args) {
            if let TyKind::Gen(k) = &**p
                && let Some(t) = self.inf.exprs.get(a).cloned()
                && let Some(slot) = inst.get_mut(*k as usize)
            {
                *slot = self.inf.icx.resolve(&t);
            }
        }
        self.check_dup(def, &inst, span);
    }

    /// A generic function that may duplicate a type parameter cannot take
    /// an affine type for it.
    fn check_dup(&mut self, def: DefId, args: &[Ty], span: Span) {
        // calls are checked once every generic function (and protocol
        // method, through its implementations) is analysed
        if !self.generic && !self.inf.g.dups_final.get() {
            let args: Vec<Ty> = args.iter().map(|a| self.inf.icx.resolve(a)).collect();
            self.inf.g.dup_deferred.borrow_mut().push((def, args, span));
            return;
        }
        let Some(dup) = self.inf.g.dup_params.borrow().get(&def).cloned() else { return };
        for k in dup {
            let Some(a) = args.get(k as usize) else { continue };
            let a = self.inf.icx.resolve(a);
            if self.is_affine_ty(&a) {
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
                    && pos < ln.until
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
            // a call result holding a reference or a function (a closure the
            // callee may return) carries its arguments' borrows
            ExprKind::Call { callee, args }
                if holds_ref
                    || matches!(hir.exprs[*callee].kind, ExprKind::Ctor { .. })
                    || self.inf.exprs.get(&e).cloned().is_some_and(|t| self.holds_fn(&t)) =>
            {
                for a in args.clone() {
                    self.carried_borrows(a, out);
                }
            }
            // a view (a sequence or closure) read from an affine value by a
            // method (`batch.items()`, `acc.writer()`) borrows it, and a
            // returned function may hold the arguments' borrows
            ExprKind::MethodCall { recv, args, .. } => {
                let (recv, args) = (*recv, args.clone());
                let view = self.inf.exprs.get(&e).cloned().is_some_and(|t| self.holds_fn(&t));
                if view
                    && let Some(root) = self.place_root(recv)
                    && self.is_affine(root)
                {
                    out.push((root, hir.exprs[e].span));
                }
                if view || holds_ref {
                    self.carried_borrows(recv, out);
                    for a in args {
                        self.carried_borrows(a, out);
                    }
                }
            }
            ExprKind::Block { tail: Some(t), .. } => self.carried_borrows(*t, out),
            // a borrow chosen by a branch, or a loop's `break` value
            ExprKind::If { then, els, .. } => {
                let (then, els) = (*then, *els);
                self.carried_borrows(then, out);
                if let Some(x) = els {
                    self.carried_borrows(x, out);
                }
            }
            ExprKind::Match { arms, .. } => {
                for b in arms.iter().map(|a| a.body).collect::<Vec<_>>() {
                    self.carried_borrows(b, out);
                }
            }
            ExprKind::Loop { body } => {
                let mut breaks = vec![];
                crate::program::walk_exprs(hir, *body, &mut |x| {
                    if let ExprKind::Break { value: Some(v), target } = &hir.exprs[x].kind
                        && *target == e
                    {
                        breaks.push(*v);
                    }
                });
                for v in breaks {
                    self.carried_borrows(v, out);
                }
            }
            // a closure borrowing its captures (or copying captured borrows)
            ExprKind::Lambda(_) => {
                let hir_l = match &hir.exprs[e].kind {
                    ExprKind::Lambda(l) => l.captures.clone(),
                    _ => vec![],
                };
                let mut caps = self.lambda_borrows.get(&e).cloned().unwrap_or_default();
                // a captured closure (or other value) holding borrows passes
                // them on, as a captured reference does
                for c in hir_l {
                    if !caps.contains(&c)
                        && (self.carriers.contains_key(&c) || self.loans.iter().any(|ln| ln.holder == c))
                    {
                        caps.push(c);
                    }
                }
                for c in caps {
                    let is_ref = self.local_ty(c).is_some_and(|t| matches!(&*t, TyKind::Ref(..)))
                        || self.carriers.contains_key(&c)
                        || self.loans.iter().any(|ln| ln.holder == c);
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
                    match &hir.exprs[lam].kind {
                        // a local alias of a generic function (`let d = dup`)
                        ExprKind::Def { def, .. } => {
                            let def = *def;
                            self.check_alias_dup(def, args, span);
                        }
                        _ => self.check_lambda_dup(lam, args, span),
                    }
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
                let callee_def = match &hir.exprs[*callee].kind {
                    ExprKind::Def { def, .. } => Some(*def),
                    _ => None,
                };
                for (i, a) in args.iter().enumerate() {
                    let p = modes.get(i).copied();
                    if p == Some(Param::Exclusive) {
                        self.check_exclusive_access(*a);
                    }
                    self.check_explicit_borrow(*a, p);
                    d |= self.expr(*a, Self::mode_of(p), st);
                    self.check_repeatable(*a, p);
                    self.moves_into_task(callee_def, i, *a, span, st);
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
                // calling a consuming closure stored in a field, or reading one
                // out of its holder (`m.get(k)`), uses up the holder
                let reads_fn = self.inf.exprs.get(&e).cloned().is_some_and(|t| self.holds_fn(&t));
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
                // (also through a borrow of such a collection: that is a move
                // out of borrowed, reported by the receiver's use)
                let recv_affine = self.expr_affine(*recv) || self.ref_to_affine(*recv);
                if rm == Mode::Borrow && self.expr_affine(e) && recv_affine {
                    rm = Mode::Move;
                }
                // so do element-visiting methods of collections and sequences
                // of affine items (only observers keep the collection)
                if rm == Mode::Borrow
                    && recv_affine
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
                let callee_def = match self.inf.res.get(&e) {
                    Some(Res::Method(Target::Fn { def, .. })) => Some(*def),
                    _ => None,
                };
                for (i, a) in args.iter().enumerate() {
                    let p = modes.get(i + 1).copied();
                    d |= self.expr(*a, Self::mode_of(p), st);
                    self.check_repeatable(*a, p);
                    self.moves_into_task(callee_def, i + 1, *a, span, st);
                }
                // after the arguments: closures among them know their borrows
                self.check_stored_borrows(e, *recv, args);
                // a consuming closure stored into a builder, buffer, or channel
                // makes the container (and what it becomes) single-use
                if let ExprKind::MethodCall { method, .. } = &hir.exprs[e].kind
                    && matches!(
                        self.inf.sym(*method),
                        "push" | "append" | "insert" | "set" | "send" | "try_send" | "try_append"
                    )
                    && let Some(root) = self.place_root(*recv)
                    && args.iter().any(|a| self.holds_consuming(*a))
                {
                    self.consuming.insert(root);
                    self.affine.insert(root, true);
                }
                if let Some(root) = consumed_root
                    && rm != Mode::Move
                {
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
                // a match whose arms bind nothing (wildcards, literals,
                // variant tags) only reads its scrutinee
                let binds = arms.iter().any(|a| {
                    let mut ls = vec![];
                    pat_locals(hir, a.pat, &mut ls);
                    !ls.is_empty()
                });
                let how = if binds { Mode::Move } else { Mode::Borrow };
                if self.expr(*scrutinee, how, st) {
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
                for pass in 0..2 {
                    let n = self.loans.len();
                    self.expr(*cond, Mode::Move, st);
                    let mut s = st.clone();
                    self.expr(*body, Mode::Move, &mut s);
                    Self::merge(st, &s);
                    if pass == 0 {
                        self.loans_carried_around(e, n);
                    }
                }
                false
            }
            ExprKind::For { iter, body, pat } => {
                self.expr(*iter, Mode::Move, st);
                let mut bound = vec![];
                pat_locals(hir, *pat, &mut bound);
                for pass in 0..2 {
                    let n = self.loans.len();
                    let mut s = st.clone();
                    // the loop variable is bound afresh each iteration
                    for b in &bound {
                        s.remove(b);
                    }
                    self.expr(*body, Mode::Move, &mut s);
                    Self::merge(st, &s);
                    if pass == 0 {
                        self.loans_carried_around(e, n);
                    }
                }
                false
            }
            ExprKind::Loop { body } => {
                for pass in 0..2 {
                    let n = self.loans.len();
                    let mut s = st.clone();
                    self.expr(*body, Mode::Move, &mut s);
                    Self::merge(st, &s);
                    if pass == 0 {
                        self.loans_carried_around(e, n);
                    }
                }
                false
            }
            ExprKind::Block { stmts, tail } => {
                self.blocks.push(e);
                for s in stmts {
                    let d = match s {
                        Stmt::Let { pat, init, span: lspan, .. } => {
                            // binding a reference copies it
                            let init_ty = self.inf.exprs.get(init).cloned();
                            let m = self.mode_into(init_ty);
                            let d = self.expr(*init, m, st);
                            // destructuring a borrow cannot take affine parts out of it
                            if !matches!(hir.pats[*pat].kind, PatKind::Bind(_) | PatKind::Discard)
                                && (self.ref_to_affine(*init) || m == Mode::Borrow && self.expr_affine(*init))
                            {
                                let mut ls = vec![];
                                pat_locals(hir, *pat, &mut ls);
                                if let Some(l) = ls.into_iter().find(|l| self.is_affine(*l)) {
                                    let span = hir.locals[l].span;
                                    let name = self.name(l);
                                    self.report(
                                        Diagnostic::error(format!("cannot move `{name}` out of a borrowed value"))
                                            .with_code("E0404")
                                            .with_label(span, "this takes ownership of a part of a value that is only borrowed")
                                            .with_note("borrow the part instead (`let b = &x.b`), or take the value by value"),
                                        span,
                                    );
                                }
                            }
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
                                && matches!(
                                    &hir.exprs[*init].kind,
                                    ExprKind::Lambda(_) | ExprKind::Def { .. }
                                )
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
                                        until: usize::MAX,
                                        block: self.blocks.last().copied(),
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
                                        // a view (a sequence or closure) read from an affine
                                        // value (`batch.items()`) borrows it while it lives
                                        if let ExprKind::MethodCall { recv, .. } = &hir.exprs[*init].kind
                                            && let Some(root) = self.place_root(*recv)
                                            && self.is_affine(root)
                                            && self
                                                .inf
                                                .exprs
                                                .get(init)
                                                .cloned()
                                                .is_some_and(|t| self.holds_fn(&t))
                                        {
                                            extra.push((root, false, hir.exprs[*init].span));
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
                                            until: usize::MAX,
                                            block: self.blocks.last().copied(),
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
                            // the value replaces what the variable held: its
                            // old loans do not conflict with it (unless the
                            // value reads the variable)
                            let mut reads_self = false;
                            crate::program::walk_exprs(hir, *value, &mut |x| {
                                if matches!(hir.exprs[x].kind, ExprKind::Local(l) if l == *local) {
                                    reads_self = true;
                                }
                            });
                            let at = aspan.range().start;
                            let saved: Vec<(usize, usize)> = if reads_self {
                                vec![]
                            } else {
                                self.loans
                                    .iter_mut()
                                    .enumerate()
                                    .filter(|(_, ln)| ln.holder == *local && ln.until > at)
                                    .map(|(i, ln)| {
                                        let u = ln.until;
                                        ln.until = at;
                                        (i, u)
                                    })
                                    .collect()
                            };
                            let d = self.expr(*value, Mode::Move, st);
                            for (i, u) in saved {
                                self.loans[i].until = u;
                            }
                            self.check_loans(*local, *aspan, Mode::BorrowMut);
                            st.remove(local);
                            self.overwrite_result(*local, *aspan);
                            // a consuming closure assigned to a variable makes it single-use
                            if self.holds_consuming(*value) {
                                self.consuming.insert(*local);
                                self.affine.insert(*local, true);
                            }
                            // the variable's earlier borrows end here, when
                            // every path from them passes this assignment
                            let at = aspan.range().start;
                            let here = self.blocks.last().copied();
                            for ln in self
                                .loans
                                .iter_mut()
                                .filter(|ln| ln.holder == *local && ln.start <= at && ln.block == here)
                            {
                                ln.until = ln.until.min(at);
                            }
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
                                            until: usize::MAX,
                                            block: self.blocks.last().copied(),
                                        });
                                    }
                                }
                                self.carriers.entry(*local).or_default().extend(carried);
                            }
                            d
                        }
                        Stmt::Expr { expr, .. } => {
                            let d = self.expr(*expr, Mode::Borrow, st);
                            self.end_loans_assigned_in_all_branches(*expr);
                            d
                        }
                    };
                    if d {
                        self.blocks.pop();
                        return true;
                    }
                }
                let r = match tail {
                    Some(t) => self.expr(*t, mode, st),
                    None => false,
                };
                self.blocks.pop();
                r
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

/// Standard definitions trusted to use each value of their type parameters
/// once although the analysis cannot see it: an explicit, named boundary,
/// not an exemption for all standard code. `channel.select_keep` gives the
/// runtime its operations, which sends exactly one value, and returns the
/// others (a partition of the list it reads twice).
const TRUSTED_SINGLE_USE: &[(&str, &str)] = &[("channel", "select_keep")];

/// Whether `d` is one of the named standard definitions in `list`.
fn trusted_std(inf: &Infer<'_, '_>, d: DefId, list: &[(&str, &str)]) -> bool {
    let hir = inf.g.hir;
    let def = &hir.defs[d];
    let m = &hir.modules[def.module];
    if !m.is_std {
        return false;
    }
    let module = m.path.last().map(|s| inf.sym(*s).to_string()).unwrap_or_default();
    let name = inf.sym(def.name).to_string();
    list.iter().any(|(mm, n)| *mm == module && *n == name)
}
