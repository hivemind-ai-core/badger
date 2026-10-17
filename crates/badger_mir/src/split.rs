//! Split results of channel receives and Map/List lookups (optimized builds,
//! after inlining): an `Option`/`TryRecv` result only examined (its tag read
//! and its payload loaded, through plain copies) calls the intrinsic's split
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

/// Intrinsics with a split form, and that form.
const SPLIT: &[(&str, &str)] = &[
    ("badger_rt_i_map_get", "badger_rt_i_map_get_split"),
    ("badger_rt_i_list_get", "badger_rt_i_list_get_split"),
    ("badger_rt_i_chan_recv", "badger_rt_i_chan_recv_split"),
    ("badger_rt_i_chan_try_recv", "badger_rt_i_chan_try_recv_split"),
    ("badger_rt_i_batch_recv_one", "badger_rt_i_batch_recv_one_split"),
    ("badger_rt_i_batch_try_recv", "badger_rt_i_batch_try_recv_split"),
];

/// Split eligible receives and lookups of `prog`; returns how many were split.
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
    // Rewriting inserts statements. Work backward so later sites in the
    // same block retain their original positions (and argument lists).
    for (bi, i, x, to) in sites.into_iter().rev() {
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
                    Stmt::Assign(_, rv) => ok &= !rv.operands().iter().any(&in_group),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn local(f: &mut Func, repr: Repr) -> Local {
        f.locals.push(LocalDecl { repr, name: None })
    }
    fn call(f: &mut Func, name: &str, ret: Repr, args: Vec<Operand>) -> Local {
        let out = local(f, ret);
        f.blocks[BlockId::from(0)].stmts.push(Stmt::Assign(
            out,
            Rvalue::Call { callee: Callee::Intrinsic { name: name.into(), targs: 0, ret: 0 }, args, ret },
        ));
        out
    }
    fn tag(f: &mut Func, value: Local) {
        let out = local(f, Repr::I32);
        f.blocks[BlockId::from(0)].stmts.push(Stmt::Assign(out, Rvalue::Tag(Operand::Local(value))));
    }
    fn word(n: i64) -> Operand {
        Operand::Const(Const::Int(n, Repr::I64))
    }

    #[test]
    fn multiple_sites_keep_their_arguments_and_unrelated_calls() {
        let mut f = Func::new("lookups".into(), FuncKind::Normal, Repr::I64);
        let x = call(&mut f, "badger_rt_i_map_get", Repr::Ptr, vec![word(101), word(7)]);
        tag(&mut f, x);
        call(&mut f, "badger_rt_i_map_len", Repr::I64, vec![word(202)]);
        let padding = local(&mut f, Repr::I64);
        f.blocks[BlockId::from(0)].stmts.push(Stmt::Assign(padding, Rvalue::Use(word(0))));
        let y = call(&mut f, "badger_rt_i_map_get", Repr::Ptr, vec![word(303), word(9)]);
        tag(&mut f, y);
        assert_eq!(split_func(&mut f), 2);
        let calls: Vec<_> = f.blocks[BlockId::from(0)]
            .stmts
            .iter()
            .filter_map(|s| match s {
                Stmt::Assign(_, Rvalue::Call { callee: Callee::Intrinsic { name, .. }, args, .. }) => {
                    Some((name.as_str(), args))
                }
                _ => None,
            })
            .collect();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].0, "badger_rt_i_map_get_split");
        assert!(matches!(
            calls[0].1.as_slice(),
            [Operand::Const(Const::Int(101, _)), Operand::Const(Const::Int(7, _))]
        ));
        assert_eq!(calls[1].0, "badger_rt_i_map_len");
        assert!(matches!(calls[1].1.as_slice(), [Operand::Const(Const::Int(202, _))]));
        assert_eq!(calls[2].0, "badger_rt_i_map_get_split");
        assert!(matches!(
            calls[2].1.as_slice(),
            [Operand::Const(Const::Int(303, _)), Operand::Const(Const::Int(9, _))]
        ));
    }

    #[test]
    fn pointer_payloads_remain_roots_and_packed_words_keep_their_repr() {
        for repr in [Repr::Ptr, Repr::I8, Repr::I16, Repr::I32, Repr::I64, Repr::F32, Repr::F64] {
            let mut f = Func::new("payload".into(), FuncKind::Normal, repr);
            let x = call(&mut f, "badger_rt_i_list_get", Repr::Ptr, vec![word(101), word(0)]);
            tag(&mut f, x);
            let value = local(&mut f, repr);
            let block = &mut f.blocks[BlockId::from(0)];
            block.stmts.push(Stmt::Assign(value, Rvalue::Load { base: Operand::Local(x), offset: 8, repr }));
            block.stmts.push(Stmt::Poll);
            block.term = Some(Term::Return(Operand::Local(value)));
            assert_eq!(split_func(&mut f), 1);
            let block = &f.blocks[BlockId::from(0)];
            let take_repr = if repr == Repr::Ptr { Repr::Ptr } else { Repr::I64 };
            assert!(matches!(&block.stmts[1], Stmt::Assign(_, Rvalue::Call {
                callee: Callee::Runtime("badger_rt_take_aux"), ret, ..
            }) if *ret == take_repr));
            assert_eq!(f.locals[value].repr, repr);
            assert!(!block.stmts.iter().any(|s| matches!(s,
                Stmt::Assign(_, Rvalue::Load { base: Operand::Local(base), .. }) if *base == x)));
            let roots = crate::roots::safepoint_roots(&f);
            let poll = block.stmts.iter().position(|s| matches!(s, Stmt::Poll)).unwrap();
            assert_eq!(roots[0][poll].contains(&value), repr == Repr::Ptr);
        }
    }

    #[test]
    fn escaped_reassigned_and_live_results_keep_the_boxed_abi() {
        for case in ["return", "store", "argument", "reassigned", "live"] {
            let mut f = Func::new(case.into(), FuncKind::Normal, Repr::Ptr);
            let x = call(&mut f, "badger_rt_i_map_get", Repr::Ptr, vec![word(101), word(7)]);
            tag(&mut f, x);
            match case {
                "return" => f.blocks[BlockId::from(0)].term = Some(Term::Return(Operand::Local(x))),
                "store" => f.blocks[BlockId::from(0)]
                    .stmts
                    .push(Stmt::GlobalStore { slot: 0, value: Operand::Local(x) }),
                "argument" => {
                    call(&mut f, "observer", Repr::Ptr, vec![Operand::Local(x)]);
                }
                "reassigned" => f.blocks[BlockId::from(0)].stmts.push(Stmt::Assign(x, Rvalue::Use(word(0)))),
                "live" => (),
                _ => unreachable!(),
            }
            if case == "live" {
                let mut p = Program { live: true, ..Default::default() };
                p.funcs.push(f);
                assert_eq!(split_receives(&mut p), 0);
                f = p.funcs.into_iter().next().unwrap();
            } else {
                assert_eq!(split_func(&mut f), 0, "{case}");
            }
            assert!(matches!(&f.blocks[BlockId::from(0)].stmts[0], Stmt::Assign(_, Rvalue::Call {
                callee: Callee::Intrinsic { name, .. }, ret: Repr::Ptr, ..
            }) if name == "badger_rt_i_map_get"));
        }
    }
}
