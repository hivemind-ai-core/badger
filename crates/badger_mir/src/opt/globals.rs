//! Fold module-level values whose initializer is a constant.
//!
//! A module-value getter initializes its slot lazily, once, under the
//! runtime's `global_init` / `global_done` protocol. When the initializer
//! only stores a constant, reading the value is that constant: the lazy
//! initialization has no observable effect (no call, allocation, check, or
//! other statement runs), so a direct call of the getter becomes the
//! constant. Simplifying a caller that is itself a getter can turn its
//! initializer into a constant (`let scale = limit * 4`), so the pass
//! repeats until no new getter folds. Getters left without callers are
//! removed by reachability pruning. Live programs keep their getters (the
//! pipeline does not run on them).

use rustc_hash::FxHashMap;

use crate::mir::*;

pub fn fold_constant_globals(prog: &mut Program) -> usize {
    let mut consts: FxHashMap<FuncId, Const> = FxHashMap::default();
    let mut total = 0;
    loop {
        let mut grew = false;
        for (id, f) in prog.funcs.iter_enumerated() {
            if !consts.contains_key(&id)
                && let Some(c) = constant_initializer(f)
            {
                consts.insert(id, c);
                grew = true;
            }
        }
        if !grew {
            break;
        }
        for f in prog.funcs.iter_mut() {
            let mut hits = 0;
            for block in f.blocks.iter_mut() {
                for s in block.stmts.iter_mut() {
                    let Stmt::Assign(_, Rvalue::Call { callee: Callee::Direct(g), args, .. }) = s else {
                        continue;
                    };
                    if !args.is_empty() {
                        continue;
                    }
                    let Some(c) = consts.get(g) else { continue };
                    let Stmt::Assign(dest, _) = *s else { unreachable!() };
                    *s = Stmt::Assign(dest, Rvalue::Use(Operand::Const(*c)));
                    hits += 1;
                }
            }
            if hits > 0 {
                // fold what the constant feeds (a getter's own initializer
                // among it) before looking for newly constant getters
                super::simplify::simplify_func(f);
                total += hits;
            }
        }
    }
    total
}

/// The constant a getter's initializer stores, when that is all it does.
///
/// The whole function must have the lowered getter shape (library getters
/// carry no `is_global` flag, so the shape is the test): the ready-flag
/// test, the `global_init` slow path, one `GlobalStore` of a constant, the
/// `global_done` publication, and a return of that slot. Any other
/// statement (a call, a checked operation, an allocation) keeps the lazy
/// initialization.
fn constant_initializer(f: &Func) -> Option<Const> {
    if f.kind != FuncKind::Normal || !f.params.is_empty() {
        return None;
    }
    let mut stored: Option<(u32, Const)> = None;
    let (mut init, mut done) = (0, 0);
    let mut loads: Vec<(Local, u32)> = vec![];
    for block in &f.blocks {
        for s in &block.stmts {
            match s {
                Stmt::Loc { .. } | Stmt::Poll => {}
                Stmt::GlobalStore { slot, value: Operand::Const(c) } if stored.is_none() => {
                    stored = Some((*slot, *c))
                }
                Stmt::Assign(l, rv) => match rv {
                    Rvalue::GlobalLoad(slot) => loads.push((*l, *slot)),
                    Rvalue::GlobalAddr(_) | Rvalue::Cmp { .. } => {}
                    Rvalue::Call { callee: Callee::Runtime("badger_rt_global_init"), .. } => init += 1,
                    Rvalue::Call { callee: Callee::Runtime("badger_rt_global_done"), .. } => done += 1,
                    _ => return None,
                },
                _ => return None,
            }
        }
    }
    let (slot, c) = stored?;
    if init != 1 || done != 1 {
        return None;
    }
    // every return reads the stored slot
    for block in &f.blocks {
        if let Some(Term::Return(o)) = &block.term {
            let Operand::Local(l) = o else { return None };
            if !loads.iter().any(|(x, s)| x == l && *s == slot)
                || loads.iter().filter(|(x, _)| x == l).count() != 1
            {
                return None;
            }
        }
    }
    // the constant must already have the getter's representation (an
    // assignment would otherwise coerce it differently than the slot load)
    let fits = match c {
        Const::Int(_, r) => r == f.ret,
        Const::F32(_) => f.ret == Repr::F32,
        Const::F64(_) => f.ret == Repr::F64,
        Const::Addr(_) => matches!(f.ret, Repr::Ptr | Repr::I64),
    };
    fits.then_some(c)
}
