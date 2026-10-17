//! Static concurrency checks: warnings for task and channel patterns that
//! deadlock on every run.
//!
//! - A task that receives from a channel until it closes, joined before
//!   that channel's sender is closed in the same block (E0450): the join
//!   waits for the task, the task waits for the close, and the close comes
//!   after the join.
//! - A task that waits to receive from a channel created in the block,
//!   joined before anything is sent on it (E0451).
//! - Sends on a channel created in the block, more than its capacity,
//!   before any other task could receive from it (E0452).

use badger_base::Diagnostic;
use badger_hir::{DefId, DefKind, ExprId, ExprKind, Hir, Lit, LocalId, PatKind, Stmt};
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
    // channels created fresh in this function
    let mut fresh: FxHashMap<LocalId, u128> = FxHashMap::default();
    for &b in &blocks {
        if let ExprKind::Block { stmts, .. } = &hir.exprs[b].kind {
            for s in stmts {
                if let Some((c, cap)) = created_channel(inf, s) {
                    fresh.insert(c, cap);
                }
            }
        }
    }
    for b in blocks {
        join_before_close(inf, b);
        join_before_send(inf, body, b, &fresh);
        sends_without_receiver(inf, b);
    }
}

/// The expressions in `e` that read local `c`.
fn uses_of(hir: &Hir, e: ExprId, c: LocalId) -> Vec<ExprId> {
    let mut out = vec![];
    walk_exprs(hir, e, &mut |x| {
        if matches!(hir.exprs[x].kind, ExprKind::Local(l) if l == c) {
            out.push(x);
        }
    });
    out
}

/// A block's statements as expressions (a `let`'s initializer, an
/// assignment's value), then its tail.
fn block_exprs(hir: &Hir, block: ExprId) -> Vec<ExprId> {
    let ExprKind::Block { stmts, tail } = &hir.exprs[block].kind else { return vec![] };
    stmts
        .iter()
        .map(|s| match s {
            Stmt::Let { init, .. } => *init,
            Stmt::Assign { value, .. } => *value,
            Stmt::Expr { expr, .. } => *expr,
        })
        .chain(*tail)
        .collect()
}

/// `let c = channel.create<T>({ capacity: N, .. })?`: the local and its
/// literal capacity.
fn created_channel(inf: &mut Infer<'_, '_>, s: &Stmt) -> Option<(LocalId, u128)> {
    let hir = inf.g.hir;
    let Stmt::Let { pat, init, .. } = s else { return None };
    let PatKind::Bind(c) = hir.pats[*pat].kind else { return None };
    // `channel.create(..)?`, possibly through `.map_err(..)`
    let mut e = *init;
    loop {
        e = match &hir.exprs[e].kind {
            ExprKind::Try(x) => *x,
            ExprKind::MethodCall { recv, method, .. } if inf.sym(*method) == "map_err" => *recv,
            _ => break,
        };
    }
    let ExprKind::Call { callee, args } = &hir.exprs[e].kind else { return None };
    let ExprKind::Def { def, .. } = &hir.exprs[*callee].kind else { return None };
    if hir.try_lang("channel.create") != Some(*def) {
        return None;
    }
    let ExprKind::Record { fields, .. } = &hir.exprs[*args.first()?].kind else { return None };
    let cap = fields.iter().find(|f| inf.sym(f.0) == "capacity")?;
    match &hir.exprs[cap.2].kind {
        ExprKind::Lit(Lit::Int { value, .. }) => Some((c, *value)),
        _ => None,
    }
}

/// Whether `e` mentions local `c` anywhere (closures included).
fn mentions(hir: &Hir, e: ExprId, c: LocalId) -> bool {
    let mut found = false;
    walk_exprs(hir, e, &mut |x| match &hir.exprs[x].kind {
        ExprKind::Local(l) if *l == c => found = true,
        ExprKind::Lambda(l) if l.captures.contains(&c) => found = true,
        _ => {}
    });
    found
}

/// Statement `e` is exactly `c.<side>.<method>(..)` (possibly under `?`),
/// with arguments that do not mention `c`.
fn channel_op(inf: &mut Infer<'_, '_>, e: ExprId, side: &str, method: &str, ty: &str) -> Option<LocalId> {
    let hir = inf.g.hir;
    let e = match &hir.exprs[e].kind {
        ExprKind::Try(x) => *x,
        _ => e,
    };
    let e = match &hir.exprs[e].kind {
        ExprKind::Match { scrutinee, .. } => *scrutinee,
        _ => e,
    };
    let ExprKind::MethodCall { recv, method: m, args, .. } = &hir.exprs[e].kind else { return None };
    if inf.sym(*m) != method || type_name(inf, *recv).as_deref() != Some(ty) {
        return None;
    }
    let c = field_of_local(hir, *recv, side, inf)?;
    if args.iter().any(|a| mentions(hir, *a, c)) {
        return None;
    }
    Some(c)
}

/// Whether `e` may leave its block early (`return`, `break`, `?`).
fn may_exit(hir: &Hir, e: ExprId) -> bool {
    let mut found = false;
    walk_exprs(hir, e, &mut |x| {
        if matches!(hir.exprs[x].kind, ExprKind::Return { .. } | ExprKind::Break { .. } | ExprKind::Try(_)) {
            found = true;
        }
    });
    found
}

/// A spawned closure whose body, before it could finish, waits on
/// `c.receiver.recv()` for a channel `c` it captures.
fn first_waits_on(inf: &mut Infer<'_, '_>, body: ExprId, c: LocalId) -> bool {
    let hir = inf.g.hir;
    for x in block_exprs(hir, body) {
        if channel_op(inf, x, "receiver", "recv", "Receiver") == Some(c) {
            return true;
        }
        if mentions(hir, x, c) || may_exit(hir, x) {
            return false;
        }
    }
    false
}

/// `let t = s.spawn(fn() { .. c.receiver.recv() .. })` for a channel `c`
/// created fresh in this function, then `t.join()` before anything is sent
/// on `c` (every other use of `c` follows the join): the task waits for a
/// message that is only sent once it has finished.
fn join_before_send(
    inf: &mut Infer<'_, '_>,
    fn_body: ExprId,
    block: ExprId,
    fresh: &FxHashMap<LocalId, u128>,
) {
    let hir = inf.g.hir;
    let ExprKind::Block { stmts, .. } = &hir.exprs[block].kind else { return };
    let exprs = block_exprs(hir, block);
    for (k, s) in stmts.iter().enumerate() {
        let Stmt::Let { pat, init, .. } = s else { continue };
        let PatKind::Bind(t) = hir.pats[*pat].kind else { continue };
        let Some(body) = spawned_body(inf, *init) else { continue };
        let ExprKind::Lambda(l) = &hir.exprs[match &hir.exprs[*init].kind {
            ExprKind::MethodCall { args, .. } => args[0],
            _ => continue,
        }]
        .kind
        else {
            continue;
        };
        let Some(&c) = l.captures.iter().find(|c| fresh.contains_key(c)) else { continue };
        if !first_waits_on(inf, body, c) {
            continue;
        }
        let Some(j) = (k + 1..exprs.len()).find(|&j| joined_task(inf, exprs[j]) == Some(t)) else { continue };
        // every use of `c` is in the task or after the join, and nothing
        // between can leave the block
        let allowed: Vec<ExprId> = std::iter::once(*init)
            .chain(exprs[j + 1..].iter().copied())
            .flat_map(|x| uses_of(hir, x, c))
            .collect();
        if uses_of(hir, fn_body, c).iter().any(|u| !allowed.contains(u))
            || exprs[k + 1..j].iter().any(|&x| may_exit(hir, x))
        {
            continue;
        }
        let sent_after =
            exprs[j + 1..].iter().any(|&x| channel_op(inf, x, "sender", "send", "Sender") == Some(c));
        if sent_after {
            let task = inf.g.interner.resolve(hir.locals[t].name).to_string();
            let chan = inf.g.interner.resolve(hir.locals[c].name).to_string();
            inf.push_diag(
                Diagnostic::warning(format!(
                    "`{task}` waits to receive from `{chan}`, but the first send on `{chan}` comes after this join"
                ))
                .with_code("E0451")
                .with_label(hir.exprs[exprs[j]].span, "this join waits forever")
                .with_note(format!("send on `{chan}.sender` before joining `{task}`")),
            );
        }
    }
}

/// More sends on a channel created in this block than its capacity, before
/// anything else could receive from it: the send past capacity waits
/// forever.
fn sends_without_receiver(inf: &mut Infer<'_, '_>, block: ExprId) {
    let hir = inf.g.hir;
    let ExprKind::Block { stmts, .. } = &hir.exprs[block].kind else { return };
    let exprs = block_exprs(hir, block);
    for (i, s) in stmts.iter().enumerate() {
        let Some((c, cap)) = created_channel(inf, s) else { continue };
        let mut sent: u128 = 0;
        for &x in &exprs[i + 1..] {
            if channel_op(inf, x, "sender", "send", "Sender") == Some(c) {
                sent += 1;
                if sent > cap {
                    let chan = inf.g.interner.resolve(hir.locals[c].name).to_string();
                    inf.push_diag(
                        Diagnostic::warning(format!("this send on `{chan}` waits forever"))
                            .with_code("E0452")
                            .with_label(hir.exprs[x].span, format!("`{chan}` holds {cap} message(s) and no other task can receive from it yet"))
                            .with_note(format!("start the receiving task before sending more than {cap}, or use `try_send` to test for room")),
                    );
                    break;
                }
                continue;
            }
            if mentions(hir, x, c) {
                break;
            }
        }
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
