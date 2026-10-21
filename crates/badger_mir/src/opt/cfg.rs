//! Control-flow cleanup: guard branches with constant conditions and
//! duplicated targets, merge chains of switches on one value, thread empty
//! gotos, merge single-predecessor successors, and clear unreachable
//! blocks. Block 0 stays the entry.

use std::collections::VecDeque;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::mir::*;

/// Blocks reachable from the entry.
pub fn reachable(f: &Func) -> Vec<bool> {
    let n = f.blocks.len();
    let mut reach = vec![false; n];
    let mut todo = VecDeque::new();
    if n > 0 {
        reach[0] = true;
        todo.push_back(BlockId::new(0));
    }
    while let Some(b) = todo.pop_front() {
        if let Some(t) = &f.blocks[b].term {
            for s in t.successors() {
                if s.index() < n && !reach[s.index()] {
                    reach[s.index()] = true;
                    todo.push_back(s);
                }
            }
        }
    }
    reach
}

/// Blocks dominated by each block, as a membership test `dominates(def,
/// use)`. The entry's dominance over block 0 holds by definition.
pub struct Dominators {
    sets: Vec<Vec<u64>>,
    reach: Vec<bool>,
}

impl Dominators {
    pub fn new(f: &Func) -> Dominators {
        let n = f.blocks.len();
        let reach = reachable(f);
        let words = n.div_ceil(64).max(1);
        let mut sets = vec![vec![0u64; words]; n];
        for b in 1..n {
            if reach[b] {
                sets[b].fill(u64::MAX);
                let extra = words * 64 - n;
                if extra > 0 {
                    sets[b][words - 1] &= u64::MAX >> extra;
                }
            }
        }
        if n > 0 {
            sets[0][0] |= 1;
        }
        let mut preds: Vec<Vec<usize>> = vec![vec![]; n];
        for (b, block) in f.blocks.iter_enumerated() {
            if let Some(t) = &block.term {
                for s in t.successors() {
                    if s.index() < n {
                        preds[s.index()].push(b.index());
                    }
                }
            }
        }
        let mut changed = true;
        while changed {
            changed = false;
            for b in 1..n {
                if !reach[b] {
                    continue;
                }
                let ps: Vec<usize> = preds[b].iter().copied().filter(|p| reach[*p]).collect();
                let Some(&first) = ps.first() else { continue };
                let mut next = sets[first].clone();
                for p in &ps[1..] {
                    for (w, v) in next.iter_mut().zip(&sets[*p]) {
                        *w &= *v;
                    }
                }
                next[b / 64] |= 1 << (b % 64);
                if next != sets[b] {
                    sets[b] = next;
                    changed = true;
                }
            }
        }
        Dominators { sets, reach }
    }

    /// Whether the definition at `(def_block, def_stmt)` reaches the use at
    /// `(use_block, use_stmt)` on every path.
    pub fn dominates(
        &self,
        def_block: BlockId,
        def_stmt: usize,
        use_block: BlockId,
        use_stmt: usize,
    ) -> bool {
        let (db, ub) = (def_block.index(), use_block.index());
        if !self.reach.get(db).copied().unwrap_or(false) || !self.reach.get(ub).copied().unwrap_or(false) {
            return false;
        }
        if db == ub {
            return def_stmt < use_stmt;
        }
        self.sets[ub][db / 64] & (1 << (db % 64)) != 0
    }

    pub fn reachable(&self) -> &[bool] {
        &self.reach
    }
}

/// Locals with exactly one constant definition, and that value.
pub fn constant_locals(f: &Func) -> FxHashMap<Local, i64> {
    let mut defs: FxHashMap<Local, Vec<i64>> = FxHashMap::default();
    let mut multi: FxHashSet<Local> = FxHashSet::default();
    for block in &f.blocks {
        for s in &block.stmts {
            if let Stmt::Assign(l, rv) = s {
                match rv {
                    Rvalue::Use(Operand::Const(Const::Int(v, _))) if !multi.contains(l) => {
                        defs.entry(*l).or_default().push(*v);
                    }
                    _ => {
                        multi.insert(*l);
                    }
                }
            }
        }
    }
    defs.into_iter()
        .filter(|(l, vs)| vs.len() == 1 && !multi.contains(l) && !f.params.contains(l) && *l != f.env)
        .map(|(l, vs)| (l, vs[0]))
        .collect()
}

/// Fold constant and equal-target branches and switches.
pub fn fold_terms(f: &mut Func) -> usize {
    let consts = constant_locals(f);
    let pick = |o: Operand| match o {
        Operand::Const(Const::Int(v, _)) => Some(v),
        Operand::Local(l) => consts.get(&l).copied(),
        _ => None,
    };
    let mut changed = 0;
    for block in f.blocks.iter_mut() {
        let Some(t) = block.term.take() else { continue };
        block.term = Some(match t {
            Term::Branch(c, a, b) => match pick(c) {
                Some(v) => {
                    changed += 1;
                    Term::Goto(if v != 0 { a } else { b })
                }
                None if a == b => {
                    changed += 1;
                    Term::Goto(a)
                }
                None => Term::Branch(c, a, b),
            },
            Term::Switch(c, cases, d) => match pick(c) {
                Some(v) => {
                    changed += 1;
                    Term::Goto(cases.iter().find(|(k, _)| *k == v).map(|(_, t)| *t).unwrap_or(d))
                }
                None if cases.iter().all(|(_, t)| *t == d) => {
                    changed += 1;
                    Term::Goto(d)
                }
                None => Term::Switch(c, cases, d),
            },
            other => other,
        });
    }
    changed
}

/// Thread trivially empty gotos and merge a block into its unique successor
/// when that successor has one predecessor.
pub fn thread_and_merge(f: &mut Func) -> usize {
    let mut changed = 0;
    loop {
        let mut step = 0;
        for b in 0..f.blocks.len() {
            let Some(Term::Goto(t)) = f.blocks[b].term else { continue };
            if f.blocks[b].stmts.iter().any(|s| !matches!(s, Stmt::Loc { .. })) {
                continue;
            }
            // only an empty intermediate block can be skipped: threading
            // through a block with statements would drop its work
            if f.blocks[t].stmts.iter().any(|s| !matches!(s, Stmt::Loc { .. })) {
                continue;
            }
            let Some(Term::Goto(u)) = f.blocks[t].term else { continue };
            if u.index() == b || u == t || t.index() == 0 {
                continue;
            }
            f.blocks[b].term = Some(Term::Goto(u));
            step += 1;
        }
        changed += step;
        if step == 0 {
            break;
        }
    }
    loop {
        let reach = reachable(f);
        let mut preds = vec![0usize; f.blocks.len()];
        for block in &f.blocks {
            if let Some(t) = &block.term {
                for s in t.successors() {
                    preds[s.index()] += 1;
                }
            }
        }
        let mut merged = false;
        for b in 0..f.blocks.len() {
            let Some(Term::Goto(t)) = f.blocks[b].term else { continue };
            let ti = t.index();
            if ti == b || ti == 0 || !reach[b] || !reach[ti] || preds[ti] != 1 {
                continue;
            }
            let stmts = std::mem::take(&mut f.blocks[ti].stmts);
            let term = f.blocks[ti].term.take();
            f.blocks[b].stmts.extend(stmts);
            f.blocks[b].term = term;
            merged = true;
            changed += 1;
            break;
        }
        if !merged {
            break;
        }
    }
    let reach = reachable(f);
    for (b, block) in f.blocks.iter_mut_enumerated() {
        if !reach[b.index()] && (block.term != Some(Term::Unreachable) || !block.stmts.is_empty()) {
            block.stmts.clear();
            block.term = Some(Term::Unreachable);
            changed += 1;
        }
    }
    changed
}

/// Merge a chain of switches on one value. A `match` lowers each arm's test
/// to its own switch whose default is the next test; when that default
/// block is reached only from the switch and holds nothing but a
/// recomputation of the switched value (a `Tag`, `FromWord`, `ToWord`, or
/// copy of operands unchanged since) before switching on it again, its
/// cases join the first switch. One switch lets the backend emit a jump
/// table instead of a chain of compares.
pub fn merge_switch_chains(f: &mut Func) -> usize {
    let mut total = 0;
    loop {
        let merged = merge_switch_pairs(f);
        if merged == 0 {
            return total;
        }
        total += merged;
    }
}

/// One round of `merge_switch_chains`: each block takes part in at most one
/// merge.
fn merge_switch_pairs(f: &mut Func) -> usize {
    let n = f.blocks.len();
    let mut defs: FxHashMap<Local, usize> = FxHashMap::default();
    for block in &f.blocks {
        for s in &block.stmts {
            if let Stmt::Assign(l, _) = s {
                *defs.entry(*l).or_default() += 1;
            }
        }
    }
    let single = |l: Local| defs.get(&l) == Some(&1) && !f.params.contains(&l) && l != f.env;
    let mut preds = vec![0usize; n];
    for block in &f.blocks {
        if let Some(t) = &block.term {
            for s in t.successors() {
                preds[s.index()] += 1;
            }
        }
    }
    let mut plans: Vec<(usize, usize)> = vec![];
    let mut used = vec![false; n];
    for p in 0..n {
        let Some(Term::Switch(Operand::Local(t1), _, d)) = &f.blocks[p].term else { continue };
        let (t1, d) = (*t1, d.index());
        if d == p || d == 0 || preds[d] != 1 || used[p] || used[d] {
            continue;
        }
        let Some(Term::Switch(o2, _, d2)) = &f.blocks[d].term else { continue };
        if d2.index() == d {
            continue;
        }
        let body: Vec<&Stmt> = f.blocks[d].stmts.iter().filter(|s| !matches!(s, Stmt::Loc { .. })).collect();
        let same = match (o2, body.as_slice()) {
            // the default switches on the very same local
            (Operand::Local(t2), []) => *t2 == t1,
            // or recomputes it from the same operands
            (Operand::Local(t2), [Stmt::Assign(l, rv2)]) if l == t2 && single(*t2) => {
                recomputes(&f.blocks[p].stmts, t1, rv2) && f.locals[*t2].repr == f.locals[t1].repr
            }
            _ => false,
        };
        if same {
            plans.push((p, d));
            used[p] = true;
            used[d] = true;
        }
    }
    for &(p, d) in &plans {
        let stmts = std::mem::take(&mut f.blocks[d].stmts);
        let Some(Term::Switch(_, cases2, d2)) = f.blocks[d].term.take() else { unreachable!() };
        f.blocks[d].term = Some(Term::Unreachable);
        // the recomputation is pure and its operands are those of the
        // first switch's value, so it can run on every path out of `p`
        f.blocks[p].stmts.extend(stmts.into_iter().filter(|s| !matches!(s, Stmt::Loc { .. })));
        let Some(Term::Switch(_, cases, d)) = &mut f.blocks[p].term else { unreachable!() };
        for (k, t) in cases2 {
            // a value the first switch already handles never reaches `d`
            if !cases.iter().any(|(c, _)| *c == k) {
                cases.push((k, t));
            }
        }
        *d = d2;
    }
    plans.len()
}

/// Whether `rv` recomputes the value `t1` holds at the end of `stmts`:
/// `t1` is defined there by the same pure rvalue, and neither `t1` nor the
/// rvalue's operands (nor memory, for a `Tag`) change after that.
fn recomputes(stmts: &[Stmt], t1: Local, rv: &Rvalue) -> bool {
    if !matches!(rv, Rvalue::Tag(_) | Rvalue::FromWord { .. } | Rvalue::ToWord(_) | Rvalue::Use(_)) {
        return false;
    }
    let Some(at) = stmts.iter().rposition(|s| matches!(s, Stmt::Assign(l, _) if *l == t1)) else {
        return false;
    };
    let Stmt::Assign(_, rv1) = &stmts[at] else { unreachable!() };
    if rv1 != rv {
        return false;
    }
    let operands = rv.operands();
    if operands.contains(&Operand::Local(t1)) {
        return false;
    }
    stmts[at + 1..].iter().all(|s| match s {
        Stmt::Assign(l, later) => {
            !operands.contains(&Operand::Local(*l))
                && !(matches!(rv, Rvalue::Tag(_))
                    && matches!(later, Rvalue::Call { .. } | Rvalue::Alloc { .. } | Rvalue::Concat(_)))
        }
        Stmt::Loc { .. } => true,
        Stmt::Store { .. } | Stmt::GlobalStore { .. } | Stmt::Poll => !matches!(rv, Rvalue::Tag(_)),
    })
}

/// Full CFG cleanup (returns changes).
pub fn cleanup(f: &mut Func) -> usize {
    fold_terms(f) + merge_switch_chains(f) + thread_and_merge(f)
}
