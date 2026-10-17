//! MIR invariants, checked after lowering (and after any later pass):
//! every reachable block is terminated and jumps within the function,
//! operands name existing locals, values land in locals of their own
//! representation, and a managed pointer never moves into a scalar local
//! (where the collector could not see it; a word converted to a pointer
//! type goes the other way and is allowed).

use crate::mir::*;

/// Violations of the invariants, one message each.
pub fn verify(prog: &Program) -> Vec<String> {
    let mut out = vec![];
    for (fid, f) in prog.funcs.iter_enumerated() {
        verify_func(prog, fid, f, &mut out);
    }
    out
}

fn verify_func(prog: &Program, _fid: FuncId, f: &Func, out: &mut Vec<String>) {
    let n_locals = f.locals.len();
    let n_blocks = f.blocks.len();
    let mut err = |msg: String| out.push(format!("MIR {}: {msg}", f.name));
    let local_ok = |l: Local| l.index() < n_locals;
    let repr_of = |o: &Operand| -> Option<Repr> {
        match o {
            Operand::Local(l) if l.index() < n_locals => Some(f.locals[*l].repr),
            _ => None,
        }
    };
    // reachable blocks from the entry
    let mut reach = vec![false; n_blocks];
    let mut stack = vec![BlockId::from_usize(0)];
    while let Some(b) = stack.pop() {
        if b.index() >= n_blocks || reach[b.index()] {
            continue;
        }
        reach[b.index()] = true;
        match &f.blocks[b].term {
            Some(Term::Goto(t)) => stack.push(*t),
            Some(Term::Branch(_, a, c)) => stack.extend([*a, *c]),
            Some(Term::Switch(_, cases, d)) => {
                stack.extend(cases.iter().map(|c| c.1));
                stack.push(*d);
            }
            _ => {}
        }
    }
    for (b, block) in f.blocks.iter_enumerated() {
        if !reach[b.index()] {
            continue;
        }
        let check_op = |o: &Operand, err: &mut dyn FnMut(String)| {
            if let Operand::Local(l) = o
                && !local_ok(*l)
            {
                err(format!("block {} uses undeclared local {}", b.index(), l.index()));
            }
        };
        for s in &block.stmts {
            match s {
                Stmt::Assign(l, rv) => {
                    if !local_ok(*l) {
                        err(format!("block {} assigns undeclared local {}", b.index(), l.index()));
                        continue;
                    }
                    let dest = f.locals[*l].repr;
                    for o in rvalue_operands(rv) {
                        check_op(o, &mut err);
                    }
                    match rv {
                        Rvalue::Call { callee, args, ret } => {
                            if *ret != dest {
                                err(format!(
                                    "block {}: a call returning {ret:?} lands in {dest:?} local {}",
                                    b.index(),
                                    l.index()
                                ));
                            }
                            // (a split receive returns its result's tag only)
                            if let Callee::Intrinsic { name, ret: rd, .. } = callee
                                && !name.ends_with("_split")
                                && *rd != 0
                                && unsafe { &*(*rd as *const badger_runtime::TypeDesc) }.is_ptr()
                                && dest != Repr::Ptr
                            {
                                err(format!(
                                    "block {}: managed result of `{name}` in unrooted {dest:?} local",
                                    b.index()
                                ));
                            }
                            if let Callee::Direct(t) = callee
                                && let Some(g) = prog.funcs.get(*t)
                            {
                                if g.ret != *ret {
                                    err(format!(
                                        "block {}: call of `{}` expects {:?}, callee returns {:?}",
                                        b.index(),
                                        g.name,
                                        ret,
                                        g.ret
                                    ));
                                }
                                if g.params.len() != args.len() {
                                    err(format!(
                                        "block {}: `{}` takes {} arguments, given {}",
                                        b.index(),
                                        g.name,
                                        g.params.len(),
                                        args.len()
                                    ));
                                }
                            }
                        }
                        Rvalue::Load { repr, .. } if *repr != dest => {
                            err(format!(
                                "block {}: a {repr:?} load lands in {dest:?} local {}",
                                b.index(),
                                l.index()
                            ));
                        }
                        Rvalue::Use(o) => {
                            if repr_of(o) == Some(Repr::Ptr) && dest != Repr::Ptr {
                                let src = match o {
                                    Operand::Local(s) => {
                                        f.locals[*s].name.clone().unwrap_or_else(|| format!("{}", s.index()))
                                    }
                                    _ => String::new(),
                                };
                                err(format!(
                                    "block {}: pointer local {src} moves into unrooted {dest:?} local {}",
                                    b.index(),
                                    f.locals[*l].name.clone().unwrap_or_else(|| format!("{}", l.index()))
                                ));
                            }
                        }
                        Rvalue::Alloc { .. } if dest != Repr::Ptr => {
                            err(format!(
                                "block {}: allocation lands in unrooted {dest:?} local {}",
                                b.index(),
                                l.index()
                            ));
                        }
                        _ => {}
                    }
                }
                Stmt::Store { base, value, .. } => {
                    check_op(base, &mut err);
                    check_op(value, &mut err);
                }
                Stmt::GlobalStore { value, .. } => check_op(value, &mut err),
                Stmt::Poll | Stmt::Loc { .. } => {}
            }
        }
        match &block.term {
            None => err(format!("reachable block {} has no terminator", b.index())),
            Some(t) => {
                let targets: Vec<BlockId> = match t {
                    Term::Goto(x) => vec![*x],
                    Term::Branch(c, x, y) => {
                        check_op(c, &mut err);
                        vec![*x, *y]
                    }
                    Term::Switch(c, cases, d) => {
                        check_op(c, &mut err);
                        cases.iter().map(|c| c.1).chain([*d]).collect()
                    }
                    Term::Return(o) => {
                        check_op(o, &mut err);
                        if let Some(r) = repr_of(o)
                            && r != f.ret
                            && !(matches!(r, Repr::I64 | Repr::Ptr) && matches!(f.ret, Repr::I64 | Repr::Ptr))
                        {
                            err(format!(
                                "block {} returns a {r:?} local from a {:?} function",
                                b.index(),
                                f.ret
                            ));
                        }
                        vec![]
                    }
                    Term::Unreachable => vec![],
                };
                for x in targets {
                    if x.index() >= n_blocks {
                        err(format!("block {} jumps to missing block {}", b.index(), x.index()));
                    }
                }
            }
        }
    }
}

pub(crate) fn rvalue_operands(rv: &Rvalue) -> Vec<&Operand> {
    match rv {
        Rvalue::Use(a) | Rvalue::Not(a) | Rvalue::ToWord(a) | Rvalue::Tag(a) => vec![a],
        Rvalue::Neg { a, .. }
        | Rvalue::Convert { a, .. }
        | Rvalue::Fits { a, .. }
        | Rvalue::FromWord { a, .. } => {
            vec![a]
        }
        Rvalue::Bin { a, b, .. } | Rvalue::Cmp { a, b, .. } | Rvalue::Overflows { a, b, .. } => vec![a, b],
        Rvalue::Load { base, .. } => vec![base],
        Rvalue::Call { callee, args, .. } => {
            let mut v: Vec<&Operand> = args.iter().collect();
            if let Callee::Closure(c) = callee {
                v.push(c);
            }
            v
        }
        Rvalue::Concat(xs) => xs.iter().collect(),
        Rvalue::Alloc { .. }
        | Rvalue::GlobalLoad(_)
        | Rvalue::GlobalAddr(_)
        | Rvalue::FuncAddr(_)
        | Rvalue::AdapterAddr(_) => vec![],
    }
}
