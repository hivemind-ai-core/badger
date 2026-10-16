//! Static concurrency checks: warnings for task and channel patterns that
//! deadlock on every run.
//!
//! - A task that receives from a channel until it closes, joined before
//!   that channel's sender is closed in the same block (E0450): the join
//!   waits for the task, the task waits for the close, and the close comes
//!   after the join.

use lang_base::Diagnostic;
use lang_hir::{DefId, DefKind, ExprId, ExprKind, Hir, LocalId, PatKind, Stmt};
use rustc_hash::FxHashMap;

use crate::infer::Infer;
use crate::program::walk_exprs;

pub fn check_def(inf: &mut Infer<'_, '_>, d: DefId) {
    let hir = inf.g.hir;
    if hir.modules[hir.defs[d].module].is_std {
        return;
    }
    let body = match &hir.defs[d].kind {
        DefKind::Fn(f) => f.body,
        _ => None,
    };
    let Some(body) = body else { return };
    let mut blocks: Vec<ExprId> = vec![];
    walk_exprs(hir, body, &mut |e| {
        if matches!(hir.exprs[e].kind, ExprKind::Block { .. }) {
            blocks.push(e);
        }
    });
    for b in blocks {
        join_before_close(inf, b);
    }
}

/// The std type a receiver expression has (`Task`, `Sender`, ...).
fn type_name(inf: &mut Infer<'_, '_>, e: ExprId) -> Option<String> {
    let t = inf.exprs.get(&e).cloned()?;
    let d = inf.head_def(&t)?;
    inf.lang_name(d)
}

/// `x.<label>` on a local: that local.
fn field_of_local(hir: &Hir, e: ExprId, label: &str, inf: &Infer<'_, '_>) -> Option<LocalId> {
    match &hir.exprs[e].kind {
        ExprKind::Field { base, label: l, .. } if inf.sym(*l) == label => match &hir.exprs[*base].kind {
            ExprKind::Local(x) => Some(*x),
            _ => None,
        },
        _ => None,
    }
}

/// A closure body whose loop receives from `c.receiver` and leaves only at
/// the channel's end (`.none => break`): the channel local `c`.
fn receives_until_closed(inf: &mut Infer<'_, '_>, lambda_body: ExprId) -> Option<LocalId> {
    let hir = inf.g.hir;
    let mut found: Option<LocalId> = None;
    let mut other_exit = false;
    let mut loops: Vec<ExprId> = vec![];
    walk_exprs(hir, lambda_body, &mut |e| {
        if matches!(hir.exprs[e].kind, ExprKind::Loop { .. } | ExprKind::While { .. }) {
            loops.push(e);
        }
    });
    for l in loops {
        let (ExprKind::Loop { body } | ExprKind::While { body, .. }) = &hir.exprs[l].kind else { continue };
        let mut breaks = 0usize;
        let mut end_breaks = 0usize;
        let mut chan: Option<LocalId> = None;
        let mut matches: Vec<ExprId> = vec![];
        walk_exprs(hir, *body, &mut |e| match &hir.exprs[e].kind {
            ExprKind::Break { target, .. } if *target == l => breaks += 1,
            ExprKind::Return { .. } => breaks += 1,
            ExprKind::Match { .. } => matches.push(e),
            _ => {}
        });
        for m in matches {
            let ExprKind::Match { scrutinee, arms } = &hir.exprs[m].kind else { continue };
            let ExprKind::MethodCall { recv, method, .. } = &hir.exprs[*scrutinee].kind else { continue };
            if inf.sym(*method) != "recv" || type_name(inf, *recv).as_deref() != Some("Receiver") {
                continue;
            }
            let Some(c) = field_of_local(hir, *recv, "receiver", inf) else { continue };
            for a in arms {
                let is_none = matches!(&hir.pats[a.pat].kind,
                    PatKind::Variant { name, .. } if inf.sym(*name) == "none");
                if !is_none {
                    continue;
                }
                let mut exits = false;
                walk_exprs(hir, a.body, &mut |e| {
                    if matches!(&hir.exprs[e].kind, ExprKind::Break { target, .. } if *target == l) {
                        exits = true;
                    }
                });
                if exits {
                    chan = Some(c);
                    end_breaks += 1;
                }
            }
        }
        if let Some(c) = chan {
            if breaks == end_breaks {
                found = Some(c);
            } else {
                other_exit = true;
            }
        }
    }
    if other_exit { None } else { found }
}

/// `s.spawn(fn() { ... })` / `s.capture(...)` on a task scope: the closure body.
fn spawned_body(inf: &mut Infer<'_, '_>, e: ExprId) -> Option<ExprId> {
    let hir = inf.g.hir;
    let ExprKind::MethodCall { recv, method, args, .. } = &hir.exprs[e].kind else { return None };
    if !matches!(inf.sym(*method), "spawn" | "capture")
        || type_name(inf, *recv).as_deref() != Some("TaskScope")
    {
        return None;
    }
    let ExprKind::Lambda(l) = &hir.exprs[*args.first()?].kind else { return None };
    Some(l.body)
}

/// A join of a task local in statement `e` (`t.join()`, possibly under `?`
/// or `let _ =`).
fn joined_task(inf: &mut Infer<'_, '_>, e: ExprId) -> Option<LocalId> {
    let hir = inf.g.hir;
    let e = match &hir.exprs[e].kind {
        ExprKind::Try(x) => *x,
        _ => e,
    };
    let ExprKind::MethodCall { recv, method, .. } = &hir.exprs[e].kind else { return None };
    if inf.sym(*method) != "join" || type_name(inf, *recv).as_deref() != Some("Task") {
        return None;
    }
    match &hir.exprs[*recv].kind {
        ExprKind::Local(t) => Some(*t),
        _ => None,
    }
}

/// Whether `e` (any part of it) closes `c.sender`.
fn closes(inf: &mut Infer<'_, '_>, e: ExprId, c: LocalId) -> bool {
    let hir = inf.g.hir;
    let mut calls: Vec<ExprId> = vec![];
    walk_exprs(hir, e, &mut |x| {
        if let ExprKind::MethodCall { method, .. } = &hir.exprs[x].kind
            && inf.sym(*method) == "close"
        {
            calls.push(x);
        }
    });
    calls.into_iter().any(|x| {
        let ExprKind::MethodCall { recv, .. } = &hir.exprs[x].kind else { return false };
        field_of_local(hir, *recv, "sender", inf) == Some(c)
            && type_name(inf, *recv).as_deref() == Some("Sender")
    })
}

fn join_before_close(inf: &mut Infer<'_, '_>, block: ExprId) {
    let hir = inf.g.hir;
    let ExprKind::Block { stmts, tail } = &hir.exprs[block].kind else { return };
    let exprs: Vec<ExprId> = stmts
        .iter()
        .map(|s| match s {
            Stmt::Let { init, .. } => *init,
            Stmt::Assign { value, .. } => *value,
            Stmt::Expr { expr, .. } => *expr,
        })
        .chain(*tail)
        .collect();
    // task locals spawned here whose closure receives until a channel closes
    let mut consumers: FxHashMap<LocalId, LocalId> = FxHashMap::default();
    for (i, s) in stmts.iter().enumerate() {
        if let Stmt::Let { pat, init, .. } = s
            && let PatKind::Bind(t) = hir.pats[*pat].kind
            && let Some(body) = spawned_body(inf, *init)
            && let Some(c) = receives_until_closed(inf, body)
        {
            consumers.insert(t, c);
        }
        let Some(t) = joined_task(inf, exprs[i]) else { continue };
        let Some(&c) = consumers.get(&t) else { continue };
        let closed_before = exprs[..i].iter().any(|&x| closes(inf, x, c));
        let closed_after = exprs[i + 1..].iter().any(|&x| closes(inf, x, c));
        if !closed_before && closed_after {
            let task = inf.g.interner.resolve(hir.locals[t].name).to_string();
            let chan = inf.g.interner.resolve(hir.locals[c].name).to_string();
            let span = hir.exprs[exprs[i]].span;
            inf.push_diag(
                Diagnostic::warning(format!(
                    "`{task}` receives from `{chan}` until it is closed, but `{chan}.sender.close()` comes after this join"
                ))
                .with_code("E0450")
                .with_label(span, "this join waits forever")
                .with_note(format!("close `{chan}.sender` before joining `{task}`")),
            );
        }
    }
}
