//! Inlining of small direct callees (optimized builds without live
//! dispatch), followed by scalar replacement of the callers, so a record or
//! tuple built only to be passed to a small helper is never allocated.
//!
//! A callee is inlined when it is an ordinary function (not a lambda, a
//! display hook, or a module-value getter), does not call itself, and is
//! small (`MAX_CALLEE` statements, not counting copies); each call's arguments and the callee's
//! result must have exactly the representations of its parameters and
//! return (no conversion is implied by the call). Inlining runs in rounds
//! over the callees' bodies as they were before the round, and stops
//! growing a caller past `MAX_CALLER` statements. Source positions and
//! checks inside the callee are kept, so debug lines and defects are
//! unchanged; a callee's polls are kept when it loops (except its entry
//! poll, which only calls need), and dropped when its body has no cycle (its
//! work is bounded, between the caller's own polls), so an inlined helper
//! adds no budget bookkeeping to its caller's loop.
//!
//! Calls through a closure whose lambda is known (the closure object was
//! allocated in the caller, possibly reached through plain copies, with that
//! lambda's address as its code) are inlined the same way, with the object
//! as the lambda's environment; if nothing else uses the object, scalar
//! replacement then removes it. A closure that is a *static* (a function
//! passed as a value, whose environment is null) is inlined too — the
//! program records which function its object calls, so no allocation by the
//! caller is needed. A callee given such a closure as an argument
//! may be larger (`MAX_HIGHER_ORDER`), so `map`, `filter`, and `fold` become
//! loops around the lambda's body.

use crate::mir::*;
use rustc_hash::FxHashMap;

const MAX_CALLEE: usize = 40;
const MAX_HIGHER_ORDER: usize = 96;
const MAX_CALLER: usize = 4000;
const ROUNDS: usize = 2;

/// Statements that do work: inlining runs before simplification, so plain
/// copies and conversions to the same type (which simplification removes)
/// do not count against a callee.
fn size(f: &Func) -> usize {
    let free = |s: &Stmt| match s {
        Stmt::Loc { .. } | Stmt::Assign(_, Rvalue::Use(_)) => true,
        Stmt::Assign(_, Rvalue::Convert { from, to, checked: false, .. }) => from == to,
        _ => false,
    };
    f.blocks
        .iter()
        .filter(|b| !(b.stmts.is_empty() && matches!(b.term, Some(Term::Unreachable))))
        .map(|b| b.stmts.iter().filter(|s| !free(s)).count() + 1)
        .sum()
}

fn calls_itself(f: &Func, me: FuncId) -> bool {
    f.blocks.iter().any(|b| {
        b.stmts.iter().any(|s| {
            matches!(
                s,
                Stmt::Assign(_, Rvalue::Call { callee: Callee::Direct(t) | Callee::Lambda(t, _), .. })
                    if *t == me
            ) || matches!(s, Stmt::Assign(_, Rvalue::FuncAddr(t)) if *t == me)
        })
    })
}

fn inlinable(prog: &Program, id: FuncId) -> bool {
    let f = &prog.funcs[id];
    matches!(f.kind, FuncKind::Normal | FuncKind::Lambda)
        && !f.is_global
        && size(f) <= MAX_HIGHER_ORDER
        && !calls_itself(f, id)
}

/// Locals that certainly hold a closure of a known lambda: a single
/// allocation whose code word is the lambda's address, a *static* closure (a
/// function passed by name, whose object the program records), and any local
/// whose every definition is one of those or a copy of one.
fn known_closures(
    kinds: &[FuncKind],
    f: &Func,
    statics: &FxHashMap<u64, (u64, u32, FuncId)>,
) -> FxHashMap<Local, FuncId> {
    // every known closure starts from a lambda's address or a static closure
    let seeded = f.blocks.iter().any(|b| {
        b.stmts.iter().any(|s| match s {
            Stmt::Assign(_, Rvalue::FuncAddr(l)) => kinds[l.index()] == FuncKind::Lambda,
            Stmt::Assign(_, Rvalue::Use(Operand::Const(Const::Addr(w)))) => statics.contains_key(w),
            _ => false,
        })
    });
    if !seeded {
        return FxHashMap::default();
    }
    // every definition of every local, so a local set in more than one branch
    // is seen as a whole
    let mut defs: FxHashMap<Local, Vec<&Rvalue>> = FxHashMap::default();
    for b in f.blocks.iter() {
        for s in &b.stmts {
            if let Stmt::Assign(l, rv) = s {
                defs.entry(*l).or_default().push(rv);
            }
        }
    }
    let single = |l: &Local| -> Option<&Rvalue> {
        match defs.get(l) {
            Some(rvs) if rvs.len() == 1 && !f.params.contains(l) && *l != f.env => Some(rvs[0]),
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
                && kinds[l.index()] == FuncKind::Lambda
            {
                known.insert(*c, *l);
            }
        }
    }
    // A local whose *every* definition is the same known callable: a static
    // closure (a function passed by name, whose object the program records),
    // or a copy of a local that holds one. A declaration always gives a
    // value, so a `var` that is only ever set to that callable — through any
    // branch — holds it at every use.
    loop {
        let mut grew = false;
        for (l, rvs) in &defs {
            if known.contains_key(l) || f.params.contains(l) || *l == f.env {
                continue;
            }
            let mut target: Option<FuncId> = None;
            let mut agree = true;
            for rv in rvs {
                let t = match rv {
                    Rvalue::Use(Operand::Const(Const::Addr(w))) => statics.get(w).map(|&(_, _, t)| t),
                    Rvalue::Use(Operand::Local(src)) => known.get(src).copied(),
                    _ => None,
                };
                match (t, target) {
                    (Some(t), None) => target = Some(t),
                    (Some(t), Some(seen)) if t == seen => {}
                    _ => {
                        agree = false;
                        break;
                    }
                }
            }
            if agree
                && let Some(t) = target
            {
                known.insert(*l, t);
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    known
}

fn op_repr(f: &Func, o: &Operand) -> Option<Repr> {
    match o {
        Operand::Local(l) => Some(f.locals[*l].repr),
        Operand::Const(Const::Int(_, r)) => Some(*r),
        Operand::Const(Const::F32(_)) => Some(Repr::F32),
        Operand::Const(Const::F64(_)) => Some(Repr::F64),
        Operand::Const(Const::Addr(_)) => None,
    }
}

/// Representations line up: every argument is its parameter's
/// representation, and every returned value is the call's.
fn fits(caller: &Func, args: &[Operand], dst: Local, callee: &Func) -> bool {
    if args.len() != callee.params.len() || callee.ret != caller.locals[dst].repr {
        return false;
    }
    let params_ok = args.iter().zip(&callee.params).all(|(a, p)| {
        let pr = callee.locals[*p].repr;
        match op_repr(caller, a) {
            Some(r) => r == pr,
            None => matches!(pr, Repr::I64 | Repr::Ptr),
        }
    });
    let returns_ok = callee.blocks.iter().all(|b| match &b.term {
        Some(Term::Return(o)) => match op_repr(callee, o) {
            Some(r) => r == callee.ret,
            None => matches!(callee.ret, Repr::I64 | Repr::Ptr),
        },
        _ => true,
    });
    params_ok && returns_ok
}

/// Inline `callee` at statement `at` of block `bi` of `f`, whose call
/// assigns `dst`; `env` is the callee's environment.
fn inline_at(f: &mut Func, bi: BlockId, at: usize, callee: &Func, env: Operand) {
    let block = &mut f.blocks[bi];
    let Stmt::Assign(dst, Rvalue::Call { args, .. }) = block.stmts[at].clone() else { return };
    let tail: Vec<Stmt> = block.stmts.split_off(at + 1);
    block.stmts.pop();
    let term = block.term.take();
    // the continuation: what followed the call
    let cont = f.blocks.push(Block { stmts: tail, term });
    // the callee's locals and blocks, renumbered
    let lmap: Vec<Local> = callee.locals.iter().map(|d| f.locals.push(d.clone())).collect();
    let base = f.blocks.len();
    let bmap = |b: BlockId| BlockId::from_usize(base + b.index());
    let lop = |o: &Operand| match o {
        Operand::Local(l) => Operand::Local(lmap[l.index()]),
        c => *c,
    };
    let bounded = !has_cycle(callee);
    // the entry poll exists for calls (recursion polls on the way down); an
    // inlined body is not called, and its loops poll at their heads
    let entry = BlockId::new(0);
    let reentered =
        callee.blocks.iter().any(|b| b.term.as_ref().is_some_and(|t| t.successors().contains(&entry)));
    for (cbi, cb) in callee.blocks.iter_enumerated() {
        let drop_polls = bounded || (cbi == entry && !reentered);
        let stmts = cb.stmts.iter().filter(|s| !(drop_polls && matches!(s, Stmt::Poll)));
        let mut nb = Block { stmts: stmts.map(|s| remap_stmt(s, &lmap)).collect(), term: None };
        nb.term = cb.term.as_ref().map(|t| match t {
            Term::Goto(x) => Term::Goto(bmap(*x)),
            Term::Branch(c, x, y) => Term::Branch(lop(c), bmap(*x), bmap(*y)),
            Term::Switch(c, cases, d) => {
                Term::Switch(lop(c), cases.iter().map(|(k, x)| (*k, bmap(*x))).collect(), bmap(*d))
            }
            Term::Return(v) => {
                nb.stmts.push(Stmt::Assign(dst, Rvalue::Use(lop(v))));
                Term::Goto(cont)
            }
            Term::Unreachable => Term::Unreachable,
        });
        f.blocks.push(nb);
    }
    // bind the environment and the parameters, and enter
    let block = &mut f.blocks[bi];
    block.stmts.push(Stmt::Assign(lmap[callee.env.index()], Rvalue::Use(env)));
    for (a, p) in args.iter().zip(&callee.params) {
        block.stmts.push(Stmt::Assign(lmap[p.index()], Rvalue::Use(*a)));
    }
    block.term = Some(Term::Goto(bmap(BlockId::new(0))));
}

/// Whether `f`'s control flow has a cycle (a loop) among the blocks
/// reachable from its entry.
fn has_cycle(f: &Func) -> bool {
    // 0: unvisited, 1: on the current path, 2: done
    let mut state = vec![0u8; f.blocks.len()];
    let mut stack: Vec<(BlockId, usize)> = vec![(BlockId::new(0), 0)];
    state[0] = 1;
    while let Some((b, i)) = stack.pop() {
        let succ: Vec<BlockId> = f.blocks[b].term.as_ref().map(Term::successors).unwrap_or_default();
        if i < succ.len() {
            stack.push((b, i + 1));
            let n = succ[i];
            match state[n.index()] {
                1 => return true,
                0 => {
                    state[n.index()] = 1;
                    stack.push((n, 0));
                }
                _ => {}
            }
        } else {
            state[b.index()] = 2;
        }
    }
    false
}

fn remap_stmt(s: &Stmt, lmap: &[Local]) -> Stmt {
    let mut s = s.clone();
    let l = |x: &mut Local| *x = lmap[x.index()];
    let o = |x: &mut Operand| {
        if let Operand::Local(v) = x {
            *v = lmap[v.index()];
        }
    };
    match &mut s {
        Stmt::Assign(d, rv) => {
            l(d);
            remap_rvalue(rv, &o);
        }
        Stmt::Store { base, value, .. } => {
            o(base);
            o(value);
        }
        Stmt::GlobalStore { value, .. } => o(value),
        Stmt::Poll | Stmt::Loc { .. } => {}
    }
    s
}

fn remap_rvalue(rv: &mut Rvalue, o: &dyn Fn(&mut Operand)) {
    match rv {
        Rvalue::Use(a) | Rvalue::Not(a) | Rvalue::ToWord(a) | Rvalue::Tag(a) => o(a),
        Rvalue::Neg { a, .. }
        | Rvalue::Convert { a, .. }
        | Rvalue::Fits { a, .. }
        | Rvalue::FromWord { a, .. } => o(a),
        Rvalue::Bin { a, b, .. } | Rvalue::Cmp { a, b, .. } | Rvalue::Overflows { a, b, .. } => {
            o(a);
            o(b);
        }
        Rvalue::Load { base, .. } => o(base),
        Rvalue::Call { callee, args, .. } => {
            args.iter_mut().for_each(o);
            if let Callee::Closure(c) | Callee::Lambda(_, c) = callee {
                o(c);
            }
        }
        Rvalue::Concat(xs) => xs.iter_mut().for_each(o),
        Rvalue::Alloc { .. }
        | Rvalue::GlobalLoad(_)
        | Rvalue::GlobalAddr(_)
        | Rvalue::FuncAddr(_)
        | Rvalue::AdapterAddr(_) => {}
    }
}

/// Inline small direct callees throughout `prog`; returns how many calls
/// were inlined.
pub fn inline(prog: &mut Program) -> usize {
    if prog.live {
        return 0;
    }
    let mut total = 0;
    for _ in 0..ROUNDS {
        let ok: Vec<bool> = prog.funcs.indices().map(|id| inlinable(prog, id)).collect();
        // callee bodies as they are at the start of the round
        let bodies: Vec<Option<Func>> =
            prog.funcs.iter_enumerated().map(|(id, f)| ok[id.index()].then(|| clone_func(f))).collect();
        let kinds: Vec<FuncKind> = prog.funcs.iter().map(|f| f.kind).collect();
        let sizes: Vec<usize> = bodies.iter().map(|b| b.as_ref().map_or(usize::MAX, size)).collect();
        let mut round = 0;
        for id in prog.funcs.indices() {
            let f = &mut prog.funcs[id];
            let mut changed = false;
            let mut bi = 0;
            let mut known = known_closures(&kinds, f, &prog.origins.closures);
            let mut cur = size(f);
            while bi < f.blocks.len() && cur < MAX_CALLER {
                let b = BlockId::from_usize(bi);
                let body = |t: FuncId, args: &[Operand], dst: Local, limit: usize| {
                    sizes[t.index()] <= limit
                        && bodies[t.index()].as_ref().is_some_and(|c| fits(f, args, dst, c))
                };
                let site = f.blocks[b].stmts.iter().enumerate().find_map(|(i, s)| match s {
                    Stmt::Assign(dst, Rvalue::Call { callee, args, ret }) if *ret == f.locals[*dst].repr => {
                        match callee {
                            Callee::Direct(t) if *t != id => {
                                // more room when it is given a known closure
                                let higher = args
                                    .iter()
                                    .any(|a| matches!(a, Operand::Local(l) if known.contains_key(l)));
                                let limit = if higher { MAX_HIGHER_ORDER } else { MAX_CALLEE };
                                body(*t, args, *dst, limit).then_some((i, *t, Operand::Const(Const::Addr(0))))
                            }
                            Callee::Closure(c @ Operand::Local(l))
                            | Callee::Lambda(_, c @ Operand::Local(l)) => known
                                .get(l)
                                .filter(|t| **t != id && body(**t, args, *dst, MAX_CALLEE))
                                .map(|t| (i, *t, *c)),
                            // A function value is a static closure (a null
                            // environment with the function's code): the
                            // program records which function that is, so the
                            // call can be seen through without the caller
                            // having allocated it.
                            Callee::Closure(Operand::Const(Const::Addr(w)))
                            | Callee::Lambda(_, Operand::Const(Const::Addr(w))) => {
                                match prog.origins.closures.get(w) {
                                    Some(&(_, _, t)) if t != id => body(t, args, *dst, MAX_CALLEE)
                                        .then_some((i, t, Operand::Const(Const::Addr(*w)))),
                                    _ => None,
                                }
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                });
                match site {
                    Some((i, t, env)) => {
                        inline_at(f, b, i, bodies[t.index()].as_ref().unwrap(), env);
                        known = known_closures(&kinds, f, &prog.origins.closures);
                        cur = size(f);
                        changed = true;
                        round += 1;
                        // the rest of this block moved to a new block: keep
                        // scanning from here
                    }
                    None => bi += 1,
                }
            }
            if changed {
                crate::sra::scalar_replace(f);
            }
        }
        total += round;
        if round == 0 {
            break;
        }
    }
    total
}

fn clone_func(f: &Func) -> Func {
    let mut g = Func::new(f.name.clone(), f.kind, f.ret);
    g.env = f.env;
    g.params = f.params.clone();
    g.locals = f.locals.clone();
    g.blocks = f.blocks.iter().map(|b| Block { stmts: b.stmts.clone(), term: b.term.clone() }).collect();
    g.is_global = f.is_global;
    g
}
