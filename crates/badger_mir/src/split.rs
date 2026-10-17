//! Split results of channel receives (optimized builds, after inlining): a
//! receive whose `Option`/`TryRecv` result is only examined (its tag read
//! and its payload loaded, through plain copies) calls the receive's split
//! form, which returns the tag and leaves the payload with the running
//! task, taken right after (`badger_rt_take_aux`); the result object is never
//! allocated. A result that is stored, passed, returned, or compared keeps
//! the ordinary call.
//!
//! A pointer payload is taken as a pointer, so it is rooted from the take
//! on; the task's slot is traced until then (no safe point lies between the
//! split call and the take).

use crate::mir::*;
use rustc_hash::{FxHashMap, FxHashSet};

/// Receives with a split form, and that form.
const SPLIT: &[(&str, &str)] = &[
    ("badger_rt_i_chan_recv", "badger_rt_i_chan_recv_split"),
    ("badger_rt_i_chan_try_recv", "badger_rt_i_chan_try_recv_split"),
    ("badger_rt_i_batch_recv_one", "badger_rt_i_batch_recv_one_split"),
    ("badger_rt_i_batch_try_recv", "badger_rt_i_batch_try_recv_split"),
];

fn operands(rv: &Rvalue) -> Vec<Operand> {
    match rv {
        Rvalue::Use(a) | Rvalue::Not(a) | Rvalue::ToWord(a) | Rvalue::Tag(a) => vec![*a],
        Rvalue::Neg { a, .. }
        | Rvalue::Convert { a, .. }
        | Rvalue::Fits { a, .. }
        | Rvalue::FromWord { a, .. } => {
            vec![*a]
        }
        Rvalue::Bin { a, b, .. } | Rvalue::Cmp { a, b, .. } | Rvalue::Overflows { a, b, .. } => vec![*a, *b],
        Rvalue::Load { base, .. } => vec![*base],
        Rvalue::Call { callee, args, .. } => {
            let mut v = args.clone();
            if let Callee::Closure(c) = callee {
                v.push(*c);
            }
            v
        }
        Rvalue::Concat(xs) => xs.clone(),
        Rvalue::Alloc { .. }
        | Rvalue::GlobalLoad(_)
        | Rvalue::GlobalAddr(_)
        | Rvalue::FuncAddr(_)
        | Rvalue::AdapterAddr(_) => vec![],
    }
}

/// Split the eligible receives of `prog`; returns how many were split.
pub fn split_receives(prog: &mut Program) -> usize {
    if prog.live {
        return 0;
    }
    let mut n = 0;
    for f in prog.funcs.iter_mut() {
        n += split_func(f);
    }
    n
}

fn split_func(f: &mut Func) -> usize {
    let mut defs: FxHashMap<Local, usize> = FxHashMap::default();
    for b in f.blocks.iter() {
        for s in &b.stmts {
            if let Stmt::Assign(l, _) = s {
                *defs.entry(*l).or_default() += 1;
            }
        }
    }
    let single = |l: Local| defs.get(&l) == Some(&1) && !f.params.contains(&l) && l != f.env;
    // candidate results: (block, statement, result local, split name)
    let mut sites = vec![];
    for (bi, b) in f.blocks.iter_enumerated() {
        for (i, s) in b.stmts.iter().enumerate() {
            if let Stmt::Assign(
                x,
                Rvalue::Call { callee: Callee::Intrinsic { name, .. }, ret: Repr::Ptr, .. },
            ) = s
                && let Some((_, to)) = SPLIT.iter().find(|(from, _)| from == name)
                && single(*x)
            {
                sites.push((bi, i, *x, *to));
            }
        }
    }
    let mut done = 0;
    for (bi, i, x, to) in sites {
        // the result and its single-definition copies
        let mut group: FxHashSet<Local> = FxHashSet::default();
        group.insert(x);
        loop {
            let mut grew = false;
            for b in f.blocks.iter() {
                for s in &b.stmts {
                    if let Stmt::Assign(l, Rvalue::Use(Operand::Local(src))) = s
                        && group.contains(src)
                        && !group.contains(l)
                        && single(*l)
                    {
                        group.insert(*l);
                        grew = true;
                    }
                }
            }
            if !grew {
                break;
            }
        }
        // every use: a copy into the group, a tag read, or a payload load
        // at one offset
        let in_group = |o: &Operand| matches!(o, Operand::Local(l) if group.contains(l));
        let mut ok = true;
        let mut payload: Option<(i32, Repr)> = None;
        for b in f.blocks.iter() {
            for s in &b.stmts {
                match s {
                    Stmt::Assign(l, Rvalue::Use(Operand::Local(src))) if group.contains(src) => {
                        ok &= group.contains(l);
                    }
                    Stmt::Assign(_, Rvalue::Tag(a)) if in_group(a) => {}
                    Stmt::Assign(_, Rvalue::Load { base, offset, repr }) if in_group(base) => {
                        if *offset == 0 {
                            ok = false;
                        } else {
                            match payload {
                                None => payload = Some((*offset, *repr)),
                                Some(p) => ok &= p == (*offset, *repr),
                            }
                        }
                    }
                    Stmt::Assign(_, rv) => ok &= !operands(rv).iter().any(&in_group),
                    Stmt::Store { base, value, .. } => ok &= !in_group(base) && !in_group(value),
                    Stmt::GlobalStore { value, .. } => ok &= !in_group(value),
                    Stmt::Poll | Stmt::Loc { .. } => {}
                }
            }
            match &b.term {
                Some(Term::Branch(c, _, _)) | Some(Term::Switch(c, _, _)) | Some(Term::Return(c)) => {
                    ok &= !in_group(c)
                }
                _ => {}
            }
        }
        if !ok {
            continue;
        }
        // rewrite: the tag from the split call, the payload from the task
        let tag = f.locals.push(LocalDecl { repr: Repr::I64, name: None });
        let take_repr = match payload {
            Some((_, Repr::Ptr)) => Repr::Ptr,
            _ => Repr::I64,
        };
        let aux = f.locals.push(LocalDecl { repr: take_repr, name: None });
        let Stmt::Assign(_, Rvalue::Call { callee: Callee::Intrinsic { targs, ret, .. }, args, .. }) =
            f.blocks[bi].stmts[i].clone()
        else {
            continue;
        };
        let call = Stmt::Assign(
            tag,
            Rvalue::Call {
                callee: Callee::Intrinsic { name: to.to_string(), targs, ret },
                args,
                ret: Repr::I64,
            },
        );
        let take = Stmt::Assign(
            aux,
            Rvalue::Call { callee: Callee::Runtime("badger_rt_take_aux"), args: vec![], ret: take_repr },
        );
        let null = Operand::Const(Const::Int(0, Repr::Ptr));
        f.blocks[bi].stmts[i] = Stmt::Assign(x, Rvalue::Use(null));
        f.blocks[bi].stmts.insert(i, take);
        f.blocks[bi].stmts.insert(i, call);
        let locals = f.locals.clone();
        for b in f.blocks.iter_mut() {
            for s in b.stmts.iter_mut() {
                let new = match s {
                    Stmt::Assign(l, Rvalue::Use(Operand::Local(src)))
                        if group.contains(src) && group.contains(l) =>
                    {
                        Some(Stmt::Assign(*l, Rvalue::Use(null)))
                    }
                    Stmt::Assign(d, Rvalue::Tag(a)) if in_group(a) => Some(Stmt::Assign(
                        *d,
                        Rvalue::FromWord { a: Operand::Local(tag), to: locals[*d].repr },
                    )),
                    Stmt::Assign(d, Rvalue::Load { base, repr, .. }) if in_group(base) => {
                        Some(if *repr == take_repr {
                            Stmt::Assign(*d, Rvalue::Use(Operand::Local(aux)))
                        } else {
                            Stmt::Assign(*d, Rvalue::FromWord { a: Operand::Local(aux), to: *repr })
                        })
                    }
                    _ => None,
                };
                if let Some(n) = new {
                    *s = n;
                }
            }
        }
        done += 1;
    }
    done
}
