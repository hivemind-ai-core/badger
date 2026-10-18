//! Reuse a fresh scalar map only while no earlier version can be observed.
//!
//! Copy/update-connected locals form one candidate. Every producer must be
//! its single fresh empty origin, a copy, or a recognized insert. Every insert
//! must kill all old aliases. Scalar reads are safe during construction; any
//! other observation (including a cursor) freezes it against later updates.
//! Transform the whole candidate or none, so a persistent fork never leaves
//! shared descendants behind for a later transient edit.
use badger_runtime::desc::{Kind, TypeDesc};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::mir::*;

const EMPTY: &str = "badger_rt_i_map_empty";
const INSERT: &str = "badger_rt_i_map_insert";
const PRIVATE: &str = "badger_rt_i_map_insert_private";

fn find(parent: &[usize], mut index: usize) -> usize {
    while parent[index] != index {
        index = parent[index];
    }
    index
}

fn scalar_map(desc: u64) -> bool {
    if desc == 0 {
        return false;
    }
    // Like the other MIR passes, this operates on resolved descriptors in a
    // checked source program, not raw addresses from an untrusted file.
    matches!(&unsafe { &*(desc as *const TypeDesc) }.kind,
        Kind::Map { key, val } if !key.is_ptr() && !val.is_ptr())
}

fn reads(stmt: &Stmt) -> Vec<Operand> {
    match stmt {
        Stmt::Assign(_, rv) => rv.operands(),
        Stmt::Store { base, value, .. } => vec![*base, *value],
        Stmt::GlobalStore { value, .. } => vec![*value],
        Stmt::Poll | Stmt::Loc { .. } => vec![],
    }
}

fn term_read(term: &Option<Term>) -> Option<Operand> {
    match term {
        Some(Term::Branch(value, ..) | Term::Switch(value, ..) | Term::Return(value)) => Some(*value),
        _ => None,
    }
}

fn successors(block: &Block) -> Vec<BlockId> {
    block.term.as_ref().map(Term::successors).unwrap_or_default()
}

type Live = FxHashSet<Local>;

/// Semantic liveness projected onto candidate locals, after each statement.
/// Unlike GC root liveness, a word conversion is an observation, never an alias.
fn live_after(f: &Func, tracked: impl Fn(Local) -> bool) -> Vec<Vec<Live>> {
    let add = |value: Operand, live: &mut Live| {
        if let Operand::Local(local) = value
            && tracked(local)
        {
            live.insert(local);
        }
    };
    let transfer = |stmt: &Stmt, live: &mut Live| {
        if let Stmt::Assign(dst, _) = stmt {
            live.remove(dst);
        }
        for value in reads(stmt) {
            add(value, live);
        }
    };
    let mut incoming = vec![Live::default(); f.blocks.len()];
    loop {
        let mut changed = false;
        for (bid, block) in f.blocks.iter_enumerated().rev() {
            let mut live = Live::default();
            for next in successors(block) {
                live.extend(incoming[next.index()].iter().copied());
            }
            if let Some(value) = term_read(&block.term) {
                add(value, &mut live);
            }
            for stmt in block.stmts.iter().rev() {
                transfer(stmt, &mut live);
            }
            if live != incoming[bid.index()] {
                incoming[bid.index()] = live;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    f.blocks
        .iter()
        .map(|block| {
            let mut live = Live::default();
            for next in successors(block) {
                live.extend(incoming[next.index()].iter().copied());
            }
            if let Some(value) = term_read(&block.term) {
                add(value, &mut live);
            }
            let mut after = vec![Live::default(); block.stmts.len()];
            for (index, stmt) in block.stmts.iter().enumerate().rev() {
                after[index] = live.clone();
                transfer(stmt, &mut live);
            }
            after
        })
        .collect()
}

/// Any update reachable from each block's entry (including loop backedges).
fn future_updates(f: &Func, sites: &[(BlockId, usize, Local)]) -> Vec<bool> {
    let mut future = vec![false; f.blocks.len()];
    for (block, ..) in sites {
        future[block.index()] = true;
    }
    loop {
        let mut changed = false;
        for (bid, block) in f.blocks.iter_enumerated().rev() {
            if !future[bid.index()] && successors(block).iter().any(|next| future[next.index()]) {
                future[bid.index()] = true;
                changed = true;
            }
        }
        if !changed {
            return future;
        }
    }
}

pub fn reuse_private_maps(program: &mut Program) -> usize {
    if program.live {
        return 0;
    }
    program.funcs.iter_mut().map(reuse).sum()
}

fn reuse(f: &mut Func) -> usize {
    let mut parent: Vec<usize> = (0..f.locals.len()).collect();
    for block in &f.blocks {
        for stmt in &block.stmts {
            let pair = match stmt {
                Stmt::Assign(dst, Rvalue::Use(Operand::Local(src))) => Some((*dst, *src)),
                Stmt::Assign(dst, Rvalue::Call { callee: Callee::Intrinsic { name, .. }, args, .. })
                    if name == INSERT =>
                {
                    match args.first() {
                        Some(Operand::Local(src)) => Some((*dst, *src)),
                        _ => None,
                    }
                }
                _ => None,
            };
            if let Some((dst, src)) = pair
                && f.locals[dst].repr == Repr::Ptr
                && f.locals[src].repr == Repr::Ptr
            {
                let (a, b) = (find(&parent, dst.index()), find(&parent, src.index()));
                parent[a] = b;
            }
        }
    }
    let group: Vec<_> = (0..parent.len()).map(|i| find(&parent, i)).collect();
    let mut origins: FxHashMap<usize, (u64, usize)> = FxHashMap::default();
    for block in &f.blocks {
        for stmt in &block.stmts {
            if let Stmt::Assign(dst, Rvalue::Call { callee: Callee::Intrinsic { name, ret, .. }, .. }) = stmt
                && name == EMPTY
                && scalar_map(*ret)
            {
                let origin = origins.entry(group[dst.index()]).or_insert((*ret, 0));
                origin.1 += 1;
            }
        }
    }
    if origins.is_empty() {
        return 0;
    }
    let mut bad: FxHashSet<usize> =
        origins.iter().filter_map(|(&g, &(_, n))| (n != 1).then_some(g)).collect();
    for local in f.params.iter().chain([&f.env]) {
        bad.insert(group[local.index()]);
    }
    let mut sites: FxHashMap<usize, Vec<(BlockId, usize, Local)>> = FxHashMap::default();
    for (bid, block) in f.blocks.iter_enumerated() {
        for (index, stmt) in block.stmts.iter().enumerate() {
            let Stmt::Assign(dst, rv) = stmt else { continue };
            let g = group[dst.index()];
            let Some(&(desc, _)) = origins.get(&g) else { continue };
            let valid = match rv {
                Rvalue::Use(Operand::Local(src)) => group[src.index()] == g,
                Rvalue::Call { callee: Callee::Intrinsic { name, ret, .. }, args, ret: repr }
                    if *ret == desc && *repr == Repr::Ptr =>
                {
                    if name == EMPTY && args.is_empty() {
                        true
                    } else if name == INSERT
                        && args.len() == 3
                        && matches!(args[0], Operand::Local(src) if group[src.index()] == g)
                    {
                        sites.entry(g).or_default().push((bid, index, *dst));
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            };
            if !valid {
                bad.insert(g);
            }
        }
    }
    if sites.is_empty() {
        return 0;
    }
    let after = live_after(f, |local| origins.contains_key(&group[local.index()]));
    for (&g, updates) in &sites {
        if bad.contains(&g) {
            continue;
        }
        let future = future_updates(f, updates);
        for (bid, block) in f.blocks.iter_enumerated() {
            let after_block = successors(block).iter().any(|next| future[next.index()]);
            for (index, stmt) in block.stmts.iter().enumerate() {
                let has_future = after_block || updates.iter().any(|&(b, i, _)| b == bid && i > index);
                let in_group =
                    |operand: &Operand| matches!(operand, Operand::Local(l) if group[l.index()] == g);
                let allowed = match stmt {
                    Stmt::Assign(dst, Rvalue::Use(Operand::Local(src))) => {
                        group[dst.index()] == g && group[src.index()] == g
                    }
                    Stmt::Assign(_, Rvalue::Call { callee, args, .. }) => {
                        let safe = match callee {
                            Callee::Intrinsic { name, .. } => matches!(
                                name.as_str(),
                                INSERT
                                    | "badger_rt_i_map_get"
                                    | "badger_rt_i_map_get_split"
                                    | "badger_rt_i_map_len"
                                    | "badger_rt_i_map_contains"
                            ),
                            Callee::Runtime("badger_rt_map_index") => true,
                            _ => false,
                        };
                        safe && args.first().is_some_and(&in_group) && !args[1..].iter().any(&in_group)
                    }
                    _ => false,
                };
                if has_future && !allowed && reads(stmt).iter().any(in_group) {
                    bad.insert(g);
                }
            }
            if after_block
                && term_read(&block.term)
                    .as_ref()
                    .is_some_and(|value| matches!(value, Operand::Local(l) if group[l.index()] == g))
            {
                bad.insert(g);
            }
        }
        for &(bid, index, dst) in updates {
            if after[bid.index()][index].iter().any(|&local| local != dst && group[local.index()] == g) {
                bad.insert(g);
            }
        }
    }
    let mut changed = 0;
    for (g, updates) in sites {
        if bad.contains(&g) {
            continue;
        }
        for (bid, index, _) in updates {
            let Stmt::Assign(_, Rvalue::Call { callee: Callee::Intrinsic { name, .. }, .. }) =
                &mut f.blocks[bid].stmts[index]
            else {
                unreachable!()
            };
            *name = PRIVATE.into();
            changed += 1;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desc(pointer_value: bool) -> u64 {
        let key = TypeDesc::new("Int", Kind::Int { bits: 64, signed: true }).leak();
        let val = if pointer_value { TypeDesc::new("String", Kind::String).leak() } else { key };
        TypeDesc::new("Map", Kind::Map { key, val }).leak() as *const TypeDesc as u64
    }
    fn word(value: i64) -> Operand {
        Operand::Const(Const::Int(value, Repr::I64))
    }
    fn local(f: &mut Func, repr: Repr) -> Local {
        f.locals.push(LocalDecl { repr, name: None })
    }
    fn call(f: &mut Func, b: BlockId, name: &str, desc: u64, repr: Repr, args: Vec<Operand>) -> Local {
        let dst = local(f, repr);
        f.blocks[b].stmts.push(Stmt::Assign(
            dst,
            Rvalue::Call {
                callee: Callee::Intrinsic { name: name.into(), targs: 0, ret: desc },
                args,
                ret: repr,
            },
        ));
        dst
    }
    fn fresh(f: &mut Func, desc: u64) -> Local {
        call(f, BlockId::from(0), EMPTY, desc, Repr::Ptr, vec![])
    }
    fn insert(f: &mut Func, b: BlockId, desc: u64, src: Local) -> Local {
        call(f, b, INSERT, desc, Repr::Ptr, vec![Operand::Local(src), word(1), word(42)])
    }
    fn len(f: &mut Func, b: BlockId, src: Local) {
        call(f, b, "badger_rt_i_map_len", 0, Repr::I64, vec![Operand::Local(src)]);
    }
    fn copy(f: &mut Func, b: BlockId, src: Local) -> Local {
        let dst = local(f, Repr::Ptr);
        f.blocks[b].stmts.push(Stmt::Assign(dst, Rvalue::Use(Operand::Local(src))));
        dst
    }
    fn cursor(f: &mut Func, b: BlockId, src: Local) {
        let dst = local(f, Repr::Ptr);
        f.blocks[b].stmts.push(Stmt::Assign(
            dst,
            Rvalue::Call {
                callee: Callee::Runtime("badger_rt_map_cursor"),
                args: vec![Operand::Local(src)],
                ret: Repr::Ptr,
            },
        ));
    }
    fn count(f: &Func, target: &str) -> usize {
        f.blocks
            .iter()
            .flat_map(|b| &b.stmts)
            .filter(|stmt| {
                matches!(stmt,
            Stmt::Assign(_, Rvalue::Call { callee: Callee::Intrinsic { name, .. }, .. }) if name == target)
            })
            .count()
    }

    #[test]
    fn a_chain_can_be_read_then_frozen_or_returned() {
        let d = desc(false);
        let b = BlockId::from(0);
        let mut f = Func::new("chain".into(), FuncKind::Normal, Repr::Ptr);
        let start = fresh(&mut f, d);
        let first = insert(&mut f, b, d, start);
        len(&mut f, b, first);
        let alias = copy(&mut f, b, first);
        let last = insert(&mut f, b, d, alias);
        cursor(&mut f, b, last);
        f.blocks[b].term = Some(Term::Return(Operand::Local(last)));
        assert_eq!(reuse(&mut f), 2);
        assert_eq!(count(&f, PRIVATE), 2);
        assert_eq!(count(&f, INSERT), 0);
    }

    #[test]
    fn any_live_old_alias_or_fork_rejects_every_update() {
        for fork in [false, true] {
            let d = desc(false);
            let b = BlockId::from(0);
            let mut f = Func::new("aliases".into(), FuncKind::Normal, Repr::I64);
            let start = fresh(&mut f, d);
            let populated = insert(&mut f, b, d, start);
            let alias = copy(&mut f, b, populated);
            let first = insert(&mut f, b, d, populated);
            if fork {
                let second = insert(&mut f, b, d, populated);
                len(&mut f, b, second);
            }
            len(&mut f, b, alias);
            len(&mut f, b, first);
            assert_eq!(reuse(&mut f), 0);
            assert_eq!(count(&f, PRIVATE), 0);
        }
    }

    #[test]
    fn hidden_publications_and_cursors_before_updates_escape() {
        for case in ["store", "capture", "cursor", "call", "word"] {
            let d = desc(false);
            let b = BlockId::from(0);
            let mut f = Func::new(case.into(), FuncKind::Normal, Repr::I64);
            let start = fresh(&mut f, d);
            match case {
                "store" => {
                    f.blocks[b].stmts.push(Stmt::GlobalStore { slot: 0, value: Operand::Local(start) })
                }
                "capture" => f.blocks[b].stmts.push(Stmt::Store {
                    base: Operand::Const(Const::Addr(1)),
                    offset: 0,
                    repr: Repr::Ptr,
                    value: Operand::Local(start),
                }),
                "cursor" => cursor(&mut f, b, start),
                "call" => {
                    call(&mut f, b, "observer", 0, Repr::I64, vec![Operand::Local(start)]);
                }
                "word" => {
                    let dst = local(&mut f, Repr::I64);
                    f.blocks[b].stmts.push(Stmt::Assign(dst, Rvalue::ToWord(Operand::Local(start))));
                }
                _ => unreachable!(),
            }
            let last = insert(&mut f, b, d, start);
            len(&mut f, b, last);
            assert_eq!(reuse(&mut f), 0, "{case}");
        }
    }

    #[test]
    fn unknown_origins_inputs_and_pointer_maps_stay_persistent() {
        for case in ["parameter", "environment", "unknown", "two-origins", "pointer"] {
            let d = desc(case == "pointer");
            let b = BlockId::from(0);
            let mut f = Func::new(case.into(), FuncKind::Normal, Repr::I64);
            let start = fresh(&mut f, d);
            match case {
                "parameter" | "environment" => {
                    let source = if case == "environment" {
                        f.env
                    } else {
                        let p = local(&mut f, Repr::Ptr);
                        f.params.push(p);
                        p
                    };
                    f.blocks[b].stmts.push(Stmt::Assign(start, Rvalue::Use(Operand::Local(source))));
                }
                "unknown" => {
                    let other = call(&mut f, b, "factory", d, Repr::Ptr, vec![]);
                    f.blocks[b].stmts.push(Stmt::Assign(start, Rvalue::Use(Operand::Local(other))));
                }
                "two-origins" => {
                    let other = fresh(&mut f, d);
                    f.blocks[b].stmts.push(Stmt::Assign(start, Rvalue::Use(Operand::Local(other))));
                }
                "pointer" => (),
                _ => unreachable!(),
            }
            let last = insert(&mut f, b, d, start);
            len(&mut f, b, last);
            assert_eq!(reuse(&mut f), 0, "{case}");
        }
    }

    #[test]
    fn loop_backedges_preserve_old_aliases_and_cursor_lifetimes() {
        for case in ["private", "old-alias", "old-cursor"] {
            let d = desc(false);
            let entry = BlockId::from(0);
            let mut f = Func::new(case.into(), FuncKind::Normal, Repr::I64);
            let again = local(&mut f, Repr::I8);
            f.params.push(again);
            let start = fresh(&mut f, d);
            let current = copy(&mut f, entry, start);
            let alias = copy(&mut f, entry, current);
            if case == "old-cursor" {
                cursor(&mut f, entry, current);
            }
            let body = f.blocks.push(Block::default());
            let exit = f.blocks.push(Block::default());
            f.blocks[entry].term = Some(Term::Goto(body));
            len(&mut f, body, current);
            let next = insert(&mut f, body, d, current);
            f.blocks[body].stmts.push(Stmt::Assign(current, Rvalue::Use(Operand::Local(next))));
            f.blocks[body].term = Some(Term::Branch(Operand::Local(again), body, exit));
            if case == "old-alias" {
                len(&mut f, exit, alias);
            }
            cursor(&mut f, exit, current);
            f.blocks[exit].term = Some(Term::Return(word(0)));
            assert_eq!(reuse(&mut f), usize::from(case == "private"), "{case}");
        }
    }

    #[test]
    fn independent_groups_and_live_dispatch_have_separate_boundaries() {
        let d = desc(false);
        let b = BlockId::from(0);
        let mut f = Func::new("groups".into(), FuncKind::Normal, Repr::I64);
        let a = fresh(&mut f, d);
        let last_a = insert(&mut f, b, d, a);
        let c = fresh(&mut f, d);
        f.blocks[b].stmts.push(Stmt::GlobalStore { slot: 0, value: Operand::Local(c) });
        let last_c = insert(&mut f, b, d, c);
        len(&mut f, b, last_a);
        len(&mut f, b, last_c);
        let mut p = Program { live: true, ..Default::default() };
        p.funcs.push(f);
        assert_eq!(reuse_private_maps(&mut p), 0);
        p.live = false;
        assert_eq!(reuse_private_maps(&mut p), 1);
        assert_eq!(count(&p.funcs[FuncId::from(0)], PRIVATE), 1);
        assert_eq!(count(&p.funcs[FuncId::from(0)], INSERT), 1);
    }
}
