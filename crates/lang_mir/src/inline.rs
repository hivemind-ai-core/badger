//! Inlining of small direct callees (optimized builds without live
//! dispatch), followed by scalar replacement of the callers, so a record or
//! tuple built only to be passed to a small helper is never allocated.
//!
//! A callee is inlined when it is an ordinary function (not a lambda, a
//! display hook, or a module-value getter), does not call itself, and is
//! small (`MAX_CALLEE` statements); each call's arguments and the callee's
//! result must have exactly the representations of its parameters and
//! return (no conversion is implied by the call). Inlining runs in rounds
//! over the callees' bodies as they were before the round, and stops
//! growing a caller past `MAX_CALLER` statements. Polls, source positions,
//! and checks inside the callee are kept, so scheduling bounds, debug lines,
//! and defects are unchanged.

use crate::mir::*;

const MAX_CALLEE: usize = 40;
const MAX_CALLER: usize = 4000;
const ROUNDS: usize = 2;

fn size(f: &Func) -> usize {
    f.blocks.iter().map(|b| b.stmts.len() + 1).sum()
}

fn calls_itself(f: &Func, me: FuncId) -> bool {
    f.blocks.iter().any(|b| {
        b.stmts.iter().any(|s| {
            matches!(s, Stmt::Assign(_, Rvalue::Call { callee: Callee::Direct(t), .. }) if *t == me)
                || matches!(s, Stmt::Assign(_, Rvalue::FuncAddr(t)) if *t == me)
        })
    })
}

fn inlinable(prog: &Program, id: FuncId) -> bool {
    let f = &prog.funcs[id];
    f.kind == FuncKind::Normal && !f.is_global && size(f) <= MAX_CALLEE && !calls_itself(f, id)
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
/// assigns `dst`.
fn inline_at(f: &mut Func, bi: BlockId, at: usize, callee: &Func) {
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
    for cb in callee.blocks.iter() {
        let mut nb = Block { stmts: cb.stmts.iter().map(|s| remap_stmt(s, &lmap)).collect(), term: None };
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
    // bind the parameters (a direct call's environment is null) and enter
    let block = &mut f.blocks[bi];
    block.stmts.push(Stmt::Assign(lmap[callee.env.index()], Rvalue::Use(Operand::Const(Const::Addr(0)))));
    for (a, p) in args.iter().zip(&callee.params) {
        block.stmts.push(Stmt::Assign(lmap[p.index()], Rvalue::Use(*a)));
    }
    block.term = Some(Term::Goto(bmap(BlockId::new(0))));
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
            if let Callee::Closure(c) = callee {
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
        let mut round = 0;
        for id in prog.funcs.indices() {
            let f = &mut prog.funcs[id];
            let mut changed = false;
            let mut bi = 0;
            while bi < f.blocks.len() && size(f) < MAX_CALLER {
                let b = BlockId::from_usize(bi);
                let site = f.blocks[b].stmts.iter().enumerate().find_map(|(i, s)| match s {
                    Stmt::Assign(dst, Rvalue::Call { callee: Callee::Direct(t), args, ret })
                        if *t != id
                            && *ret == f.locals[*dst].repr
                            && bodies[t.index()].as_ref().is_some_and(|c| fits(f, args, *dst, c)) =>
                    {
                        Some((i, *t))
                    }
                    _ => None,
                });
                match site {
                    Some((i, t)) => {
                        inline_at(f, b, i, bodies[t.index()].as_ref().unwrap());
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
