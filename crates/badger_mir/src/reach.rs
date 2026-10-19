//! Remove functions left unreferenced after inlining. Runtime callbacks,
//! static closures (including relocation origins), display hooks, and tests
//! are roots alongside the program entry. Live dispatch keeps stable IDs.
use crate::mir::*;

pub fn prune(prog: &mut Program) -> usize {
    if prog.live {
        return 0;
    }
    let mut live = vec![false; prog.funcs.len()];
    let mut todo: Vec<_> = prog
        .entry
        .into_iter()
        .chain(prog.error_report)
        .chain(prog.tests.iter().map(|(_, f)| *f))
        .chain(prog.display_hooks.iter().map(|(_, f)| *f))
        .chain(prog.static_closures.iter().map(|(_, f)| *f))
        .chain(prog.origins.closures.values().map(|(_, _, f)| *f))
        .collect();
    while let Some(id) = todo.pop() {
        if std::mem::replace(&mut live[id.index()], true) {
            continue;
        }
        for block in &prog.funcs[id].blocks {
            for stmt in &block.stmts {
                if let Stmt::Assign(
                    _,
                    Rvalue::FuncAddr(f)
                    | Rvalue::AdapterAddr(f)
                    | Rvalue::Call { callee: Callee::Direct(f) | Callee::Lambda(f, _), .. },
                ) = stmt
                {
                    todo.push(*f);
                }
            }
        }
    }
    let removed = live.iter().filter(|x| !**x).count();
    if removed == 0 {
        return 0;
    }
    let mut next = 0;
    let map: Vec<_> = live
        .iter()
        .map(|keep| {
            keep.then(|| {
                let id = FuncId::from_usize(next);
                next += 1;
                id
            })
        })
        .collect();
    let remap = |id: &mut FuncId| {
        *id = map[id.index()].expect("reachable function reference");
    };
    for (id, mut f) in std::mem::take(&mut prog.funcs).into_iter_enumerated() {
        if !live[id.index()] {
            continue;
        }
        for block in &mut f.blocks {
            for stmt in &mut block.stmts {
                if let Stmt::Assign(
                    _,
                    Rvalue::FuncAddr(f)
                    | Rvalue::AdapterAddr(f)
                    | Rvalue::Call { callee: Callee::Direct(f) | Callee::Lambda(f, _), .. },
                ) = stmt
                {
                    remap(f);
                }
            }
        }
        prog.funcs.push(f);
    }
    if let Some(id) = &mut prog.entry {
        remap(id);
    }
    if let Some(id) = &mut prog.error_report {
        remap(id);
    }
    for (_, id) in &mut prog.tests {
        remap(id);
    }
    for (_, id) in &mut prog.display_hooks {
        remap(id);
    }
    for (_, id) in &mut prog.static_closures {
        remap(id);
    }
    for (_, _, id) in prog.origins.closures.values_mut() {
        remap(id);
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callbacks_and_relocation_origins_survive_renumbering() {
        let mut p = Program::default();
        for name in ["dead", "entry", "callee", "callback", "origin"] {
            let mut f = Func::new(name.into(), FuncKind::Normal, Repr::I64);
            let dst = f.locals.push(LocalDecl { repr: Repr::I64, name: None });
            f.blocks.clear();
            f.blocks.push(Block {
                stmts: if name == "entry" {
                    vec![Stmt::Assign(
                        dst,
                        Rvalue::Call {
                            callee: Callee::Direct(FuncId::from_usize(2)),
                            args: vec![],
                            ret: Repr::I64,
                        },
                    )]
                } else {
                    vec![]
                },
                term: Some(Term::Return(Operand::Const(Const::Int(0, Repr::I64)))),
            });
            p.funcs.push(f);
        }
        p.entry = Some(FuncId::from_usize(1));
        p.static_closures.push((1, FuncId::from_usize(3)));
        p.origins.closures.insert(2, (0, 16, FuncId::from_usize(4)));
        assert_eq!(prune(&mut p), 1);
        assert_eq!(p.funcs.len(), 4);
        assert_eq!(p.entry.unwrap().index(), 0);
        assert_eq!(p.static_closures[0].1.index(), 2);
        assert_eq!(p.origins.closures[&2].2.index(), 3);
        assert!(matches!(&p.funcs[FuncId::from_usize(0)].blocks[BlockId::from_usize(0)].stmts[0],
            Stmt::Assign(_, Rvalue::Call { callee: Callee::Direct(f), .. }) if f.index() == 1));
        assert_eq!(prune(&mut p), 0);
        p.live = true;
        p.funcs.push(Func::new("live slot".into(), FuncKind::Normal, Repr::I64));
        assert_eq!(prune(&mut p), 0);
    }
}
