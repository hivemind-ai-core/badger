//! Performance warnings: loops that rebuild a value from itself, so the
//! work grows with the square of the iterations.
//!
//! - A `String` variable rebuilt from itself in a loop (`s = $"{s}.."`,
//!   `s = text.concat(s, ..)`) (E0453): each step copies everything built
//!   so far. Collecting the parts and joining them once is linear.
//! - A `List` variable concatenated with a list literal in a loop
//!   (`xs = list.concat(xs, [x])`, `xs = xs.concat([x])`) (E0454): each step
//!   copies the whole list, where `list.append` adds in place.
//!
//! Only a variable declared outside the loop counts (one declared inside
//! starts over each iteration), only assignments the loop itself runs
//! (not ones in a closure it builds), and not in a loop over a short
//! literal range (`for i in 0..9`), whose cost cannot grow.

use badger_base::Diagnostic;
use badger_hir::{BinOp, DefId, DefKind, ExprId, ExprKind, Hir, InterpPart, Lit, LocalId, Stmt};
use rustc_hash::FxHashSet;

use crate::infer::Infer;
use crate::program::walk_exprs;
use crate::ty::TyKind;

pub fn check_def(inf: &mut Infer<'_, '_>, d: DefId) {
    let hir = inf.g.hir;
    if hir.modules[hir.defs[d].module].is_std {
        return;
    }
    let Some(body) = (match &hir.defs[d].kind {
        DefKind::Fn(f) => f.body,
        _ => None,
    }) else {
        return;
    };
    let mut loops: Vec<ExprId> = vec![];
    walk_exprs(hir, body, &mut |e| {
        if matches!(hir.exprs[e].kind, ExprKind::For { .. } | ExprKind::While { .. } | ExprKind::Loop { .. })
        {
            loops.push(e);
        }
    });
    // an assignment in nested loops is reported once
    let mut reported: FxHashSet<ExprId> = FxHashSet::default();
    for l in loops {
        let (ExprKind::For { body, .. } | ExprKind::While { body, .. } | ExprKind::Loop { body }) =
            &hir.exprs[l].kind
        else {
            continue;
        };
        if short_range(hir, l) {
            continue;
        }
        let start = hir.exprs[l].span.range().start;
        let mut assigns: Vec<(LocalId, ExprId, badger_base::Span)> = vec![];
        walk_in_loop(hir, *body, &mut |e| {
            if let ExprKind::Block { stmts, .. } = &hir.exprs[e].kind {
                for s in stmts {
                    if let Stmt::Assign { local, value, span } = s {
                        assigns.push((*local, *value, *span));
                    }
                }
            }
        });
        for (local, value, span) in assigns {
            if hir.locals[local].span.range().start >= start || !reported.insert(value) {
                continue;
            }
            let name = inf.g.interner.resolve(hir.locals[local].name).to_string();
            if string_rebuilt(inf, local, value) {
                inf.push_diag(
                    Diagnostic::warning(format!(
                        "`{name}` is rebuilt from itself on every iteration: building a string this way takes time quadratic in its length"
                    ))
                    .with_code("E0453")
                    .with_label(span, "this copies everything built so far")
                    .with_note("collect the parts in a list and `text.join` them once after the loop"),
                );
            } else if list_concatenated(inf, local, value) {
                inf.push_diag(
                    Diagnostic::warning(format!(
                        "`{name}` is copied on every iteration: concatenating a list literal in a loop takes time quadratic in the list's length"
                    ))
                    .with_code("E0454")
                    .with_label(span, "this copies the whole list")
                    .with_note(format!(
                        "add items with `list.append({name}, x)`, or collect them with `list.builder()`"
                    )),
                );
            }
        }
    }
}

/// `for _ in A..B` (or `A..=B`) over integer literals, at most 100 steps.
fn short_range(hir: &Hir, l: ExprId) -> bool {
    let ExprKind::For { iter, .. } = &hir.exprs[l].kind else { return false };
    let ExprKind::Binary { op: op @ (BinOp::Range | BinOp::RangeIncl), lhs, rhs } = &hir.exprs[*iter].kind
    else {
        return false;
    };
    let lit = |e: ExprId| match hir.exprs[e].kind {
        ExprKind::Lit(Lit::Int { value, .. }) => i64::try_from(value).ok(),
        _ => None,
    };
    match (lit(*lhs), lit(*rhs)) {
        (Some(a), Some(b)) => b.saturating_sub(a) + i64::from(*op == BinOp::RangeIncl) <= 100,
        _ => false,
    }
}

/// Visit `e` and what it runs, but not the bodies of closures it builds.
fn walk_in_loop(hir: &Hir, e: ExprId, f: &mut impl FnMut(ExprId)) {
    f(e);
    if matches!(hir.exprs[e].kind, ExprKind::Lambda(_)) {
        return;
    }
    hir.exprs[e].kind.for_each_child(|c| walk_in_loop(hir, c, f));
}

fn is_local(hir: &Hir, e: ExprId, l: LocalId) -> bool {
    matches!(hir.exprs[e].kind, ExprKind::Local(x) if x == l)
}

/// The std type a local has (`String`, `List`, ...).
fn local_type(inf: &mut Infer<'_, '_>, l: LocalId) -> Option<String> {
    let t = inf.locals.get(&l).cloned()?;
    let t = inf.icx.resolve(&t);
    match &*t {
        TyKind::Con(d, _) => inf.lang_name(*d),
        _ => None,
    }
}

/// A call of std function `name` (`text.concat`): its arguments.
fn std_call<'h>(hir: &'h Hir, e: ExprId, name: &str) -> Option<&'h [ExprId]> {
    let ExprKind::Call { callee, args } = &hir.exprs[e].kind else { return None };
    match &hir.exprs[*callee].kind {
        ExprKind::Def { def, .. } if hir.try_lang(name) == Some(*def) => Some(args),
        _ => None,
    }
}

/// `s = $"{s}.."` or `s = text.concat(s, ..)` (either side), for a string `s`.
fn string_rebuilt(inf: &mut Infer<'_, '_>, s: LocalId, value: ExprId) -> bool {
    let hir = inf.g.hir;
    let rebuilt = match &hir.exprs[value].kind {
        ExprKind::Interp(parts) => {
            parts.iter().any(|p| matches!(p, InterpPart::Expr(x) if is_local(hir, *x, s)))
        }
        _ => {
            std_call(hir, value, "text.concat").is_some_and(|args| args.iter().any(|a| is_local(hir, *a, s)))
        }
    };
    rebuilt && local_type(inf, s).as_deref() == Some("String")
}

/// `xs = list.concat(xs, [..])` (either order) or `xs = xs.concat([..])`,
/// for a list `xs`.
fn list_concatenated(inf: &mut Infer<'_, '_>, xs: LocalId, value: ExprId) -> bool {
    let hir = inf.g.hir;
    let literal = |e: ExprId| matches!(hir.exprs[e].kind, ExprKind::List(_));
    let concatenated = match &hir.exprs[value].kind {
        ExprKind::MethodCall { recv, method, args, .. } => {
            inf.sym(*method) == "concat" && is_local(hir, *recv, xs) && args.len() == 1 && literal(args[0])
        }
        _ => std_call(hir, value, "list.concat").is_some_and(|args| {
            args.len() == 2
                && ((is_local(hir, args[0], xs) && literal(args[1]))
                    || (literal(args[0]) && is_local(hir, args[1], xs)))
        }),
    };
    concatenated && local_type(inf, xs).as_deref() == Some("List")
}
