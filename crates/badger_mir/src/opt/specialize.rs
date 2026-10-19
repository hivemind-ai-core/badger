//! Closure parameter specialization.
//!
//! A helper called directly with a closure allocated in the caller (a
//! single allocation whose code word is a lambda's address, possibly
//! reached through single-definition copies) gets a cloned body in which
//! calls through that closure become `Callee::Lambda` calls: a direct call
//! to the lambda that still passes the closure object as its environment.
//! The clone keeps the closure parameter, so capture layout, rooting, and
//! liveness are unchanged while the indirect dispatch (and its dynamic
//! signature) disappears. Inlining and scalar replacement then see an
//! ordinary direct call and may remove the environment.
//!
//! Unknown, escaping, and reassigned callbacks keep the original body. A
//! clone is shared per (callee, argument position, lambda).

use rustc_hash::FxHashMap;

use crate::mir::*;

const MAX_CLONES: usize = 12;

fn clone_func(f: &Func, name: String) -> Func {
    Func {
        name,
        kind: f.kind,
        env: f.env,
        params: f.params.clone(),
        ret: f.ret,
        locals: f.locals.clone(),
        blocks: f.blocks.iter().map(|b| Block { stmts: b.stmts.clone(), term: b.term.clone() }).collect(),
        needs_adapter: f.needs_adapter,
        live_key: None,
        live_sig: String::new(),
        is_global: false,
    }
}

/// Locals that certainly hold a closure of a known lambda: an allocation
/// whose code word is set to the lambda's address, and single-definition
/// copies of it.
fn known_closures(f: &Func, kinds: &[FuncKind]) -> FxHashMap<Local, FuncId> {
    let mut defs: FxHashMap<Local, (usize, Option<Rvalue>)> = FxHashMap::default();
    for b in f.blocks.iter() {
        for s in &b.stmts {
            if let Stmt::Assign(l, rv) = s {
                let e = defs.entry(*l).or_insert((0, None));
                e.0 += 1;
                e.1 = Some(rv.clone());
            }
        }
    }
    let single = |l: &Local| -> Option<&Rvalue> {
        match defs.get(l) {
            Some((1, rv)) if !f.params.contains(l) && *l != f.env => rv.as_ref(),
            _ => None,
        }
    };
    let mut known: FxHashMap<Local, FuncId> = FxHashMap::default();
    for b in f.blocks.iter() {
        for s in &b.stmts {
            if let Stmt::Store {
                base: Operand::Local(c),
                offset: 0,
                repr: Repr::I64,
                value: Operand::Local(t),
            } = s
                && matches!(single(c), Some(Rvalue::Alloc { .. }))
                && let Some(Rvalue::FuncAddr(l)) = single(t)
                && kinds.get(l.index()) == Some(&FuncKind::Lambda)
            {
                known.insert(*c, *l);
            }
        }
    }
    loop {
        let mut grew = false;
        for b in f.blocks.iter() {
            for s in &b.stmts {
                if let Stmt::Assign(l, Rvalue::Use(Operand::Local(src))) = s
                    && !known.contains_key(l)
                    && single(l).is_some()
                    && let Some(t) = known.get(src).copied()
                {
                    known.insert(*l, t);
                    grew = true;
                }
            }
        }
        if !grew {
            break;
        }
    }
    known
}

/// Replace calls through `closure` in `body` with direct lambda calls that
/// pass the closure as the environment.
fn closure_aliases(body: &Func, closure: Local) -> rustc_hash::FxHashSet<Local> {
    let mut aliases: rustc_hash::FxHashSet<Local> = rustc_hash::FxHashSet::default();
    aliases.insert(closure);
    loop {
        let mut grew = false;
        for b in body.blocks.iter() {
            for s in &b.stmts {
                if let Stmt::Assign(l, Rvalue::Use(Operand::Local(src))) = s
                    && aliases.contains(src)
                    && !aliases.contains(l)
                    && !body.params.contains(l)
                    && *l != body.env
                {
                    // only a single definition can be an alias
                    let defs = body
                        .blocks
                        .iter()
                        .flat_map(|b| b.stmts.iter())
                        .filter(|s| matches!(s, Stmt::Assign(x, _) if x == l))
                        .count();
                    if defs == 1 {
                        aliases.insert(*l);
                        grew = true;
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }
    aliases
}

fn rewrite_calls(body: &mut Func, closure: Local, lambda: FuncId, lambda_uses_env: bool) -> usize {
    let aliases = closure_aliases(body, closure);
    let mut n = 0;
    for b in body.blocks.iter_mut() {
        for s in b.stmts.iter_mut() {
            if let Stmt::Assign(_, Rvalue::Call { callee, ret, .. }) = s
                && let Callee::Closure(Operand::Local(l)) = callee
                && aliases.contains(l)
                && *ret == body.ret
            {
                // The lambda's environment is only needed when its body
                // reads captures; a non-capturing lambda gets a null
                // environment, letting the caller's closure die.
                let env = if lambda_uses_env {
                    Operand::Local(*l)
                } else {
                    Operand::Const(Const::Int(0, Repr::Ptr))
                };
                *callee = Callee::Lambda(lambda, env);
                n += 1;
            }
        }
    }
    if n > 0 && !lambda_uses_env {
        // Drop alias copies whose only role was feeding the rewritten
        // calls, so the closure parameter itself can become unused.
        loop {
            let mut uses: rustc_hash::FxHashMap<Local, usize> = rustc_hash::FxHashMap::default();
            for b in body.blocks.iter() {
                for s in &b.stmts {
                    for o in stmt_operands(s) {
                        if let Operand::Local(l) = o {
                            *uses.entry(l).or_default() += 1;
                        }
                    }
                }
                if let Some(t) = &b.term {
                    if let Term::Branch(o, _, _) | Term::Switch(o, _, _) | Term::Return(o) = t
                        && let Operand::Local(l) = o
                    {
                        *uses.entry(*l).or_default() += 1;
                    }
                }
            }
            let mut changed = false;
            for b in body.blocks.iter_mut() {
                b.stmts.retain(|s| {
                    if let Stmt::Assign(l, Rvalue::Use(Operand::Local(src))) = s
                        && aliases.contains(l)
                        && aliases.contains(src)
                        && uses.get(l).copied().unwrap_or(0) == 0
                    {
                        changed = true;
                        return false;
                    }
                    true
                });
            }
            if !changed {
                break;
            }
        }
    }
    n
}

fn stmt_operands(s: &Stmt) -> Vec<Operand> {
    match s {
        Stmt::Assign(_, rv) => rv.operands(),
        Stmt::Store { base, value, .. } => vec![*base, *value],
        Stmt::GlobalStore { value, .. } => vec![*value],
        Stmt::Poll | Stmt::Loc { .. } => vec![],
    }
}

/// Whether a function uses its environment local at all.
fn uses_env(f: &Func) -> bool {
    let env = Operand::Local(f.env);
    f.blocks.iter().any(|b| {
        b.stmts.iter().any(|s| stmt_operands(s).contains(&env))
            || b.term.as_ref().is_some_and(|t| match t {
                Term::Branch(o, _, _) | Term::Switch(o, _, _) | Term::Return(o) => *o == env,
                _ => false,
            })
    })
}

/// Whether any operand in the body still reads `l`.
fn local_used(f: &Func, l: Local) -> bool {
    let target = Operand::Local(l);
    f.blocks.iter().any(|b| {
        b.stmts.iter().any(|s| stmt_operands(s).contains(&target))
            || b.term.as_ref().is_some_and(|t| match t {
                Term::Branch(o, _, _) | Term::Switch(o, _, _) | Term::Return(o) => *o == target,
                _ => false,
            })
    })
}

pub fn specialize(prog: &mut Program) -> usize {
    if prog.live {
        return 0;
    }
    let kinds: Vec<FuncKind> = prog.funcs.iter().map(|f| f.kind).collect();
    let mut planted: FxHashMap<(FuncId, usize, FuncId), (FuncId, bool)> = FxHashMap::default();
    let originals = prog.funcs.len();
    for caller in 0..originals {
        let cid = FuncId::from_usize(caller);
        let known = known_closures(&prog.funcs[cid], &kinds);
        if known.is_empty() {
            continue;
        }
        let mut sites = vec![];
        for (b, block) in prog.funcs[cid].blocks.iter_enumerated() {
            for (i, s) in block.stmts.iter().enumerate() {
                let Stmt::Assign(_, Rvalue::Call { callee: Callee::Direct(t), args, .. }) = s else {
                    continue;
                };
                for (k, a) in args.iter().enumerate() {
                    if let Operand::Local(l) = a
                        && let Some(lambda) = known.get(l).copied()
                    {
                        sites.push((b, i, *t, k, lambda));
                    }
                }
            }
        }
        for (b, i, target, index, lambda) in sites {
            let key = (target, index, lambda);
            let (clone, dropped) = match planted.get(&key) {
                Some(&(id, dropped)) => (id, dropped),
                None => {
                    if planted.len() >= MAX_CLONES {
                        continue;
                    }
                    let callee = &prog.funcs[target];
                    if target == cid || callee.is_global || callee.kind != FuncKind::Normal {
                        continue;
                    }
                    let Some(&param) = callee.params.get(index) else { continue };
                    if callee.locals[param].repr != Repr::Ptr {
                        continue;
                    }
                    let mut body = clone_func(callee, format!("{}.spec{index}", callee.name));
                    let lambda_uses_env = uses_env(&prog.funcs[lambda]);
                    if rewrite_calls(&mut body, param, lambda, lambda_uses_env) == 0 {
                        continue;
                    }
                    // Drop the closure parameter when nothing reads it and
                    // the lambda ignores its environment: the caller's
                    // closure object then has no non-store uses and scalar
                    // replacement removes it.
                    let drop_param = !lambda_uses_env && !local_used(&body, param);
                    if drop_param {
                        body.params.retain(|p| *p != param);
                    }
                    let id = prog.funcs.push(body);
                    planted.insert(key, (id, drop_param));
                    (id, drop_param)
                }
            };
            if let Stmt::Assign(_, Rvalue::Call { callee, args, .. }) =
                &mut prog.funcs[cid].blocks[b].stmts[i]
            {
                *callee = Callee::Direct(clone);
                if dropped && index < args.len() {
                    args.remove(index);
                }
            }
        }
    }
    planted.len()
}
