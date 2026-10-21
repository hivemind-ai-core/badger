//! Control-flow cleanup: guard branches with constant conditions and
//! duplicated targets, thread empty gotos, merge single-predecessor
//! successors, and clear unreachable blocks. Block 0 stays the entry.

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

/// Reachable blocks in reverse postorder of a depth-first walk from the entry.
fn reverse_postorder(f: &Func, reach: &[bool]) -> Vec<usize> {
    let n = f.blocks.len();
    let mut seen = vec![false; n];
    let mut post = Vec::with_capacity(n);
    if n == 0 {
        return post;
    }
    let succ =
        |b: usize| f.blocks[BlockId::from_usize(b)].term.as_ref().map(Term::successors).unwrap_or_default();
    let mut stack: Vec<(usize, Vec<BlockId>, usize)> = vec![(0, succ(0), 0)];
    seen[0] = true;
    while let Some((b, ss, i)) = stack.last_mut() {
        if *i < ss.len() {
            let s = ss[*i].index();
            *i += 1;
            if s < n && reach[s] && !seen[s] {
                seen[s] = true;
                stack.push((s, succ(s), 0));
            }
        } else {
            post.push(*b);
            stack.pop();
        }
    }
    post.reverse();
    post
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
            if !reach[b.index()] {
                continue;
            }
            if let Some(t) = &block.term {
                for s in t.successors() {
                    if s.index() < n {
                        preds[s.index()].push(b.index());
                    }
                }
            }
        }
        // the maximal fixpoint does not depend on the visiting order;
        // reverse postorder reaches it in a few sweeps
        let order = reverse_postorder(f, &reach);
        let mut next = vec![0u64; words];
        let mut changed = true;
        while changed {
            changed = false;
            for &b in &order {
                if b == 0 {
                    continue;
                }
                let Some((&first, rest)) = preds[b].split_first() else { continue };
                next.copy_from_slice(&sets[first]);
                for p in rest {
                    for (w, v) in next.iter_mut().zip(&sets[*p]) {
                        *w &= *v;
                    }
                }
                next[b / 64] |= 1 << (b % 64);
                if next != sets[b] {
                    sets[b].copy_from_slice(&next);
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
    // Merging `t` into its only predecessor `b` moves `t`'s edges to `b`
    // (no other count changes) and leaves `t` unreachable, so one forward
    // sweep that keeps merging into `b` makes the same merges, in the same
    // order, as restarting the scan after each one.
    let mut reach = reachable(f);
    let mut preds = vec![0usize; f.blocks.len()];
    for block in &f.blocks {
        if let Some(t) = &block.term {
            for s in t.successors() {
                preds[s.index()] += 1;
            }
        }
    }
    for b in 0..f.blocks.len() {
        while let Some(Term::Goto(t)) = f.blocks[b].term {
            let ti = t.index();
            if ti == b || ti == 0 || !reach[b] || !reach[ti] || preds[ti] != 1 {
                break;
            }
            let stmts = std::mem::take(&mut f.blocks[ti].stmts);
            let term = f.blocks[ti].term.take();
            f.blocks[b].stmts.extend(stmts);
            f.blocks[b].term = term;
            preds[ti] = 0;
            reach[ti] = false;
            changed += 1;
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

/// Full CFG cleanup (returns changes).
pub fn cleanup(f: &mut Func) -> usize {
    fold_terms(f) + thread_and_merge(f)
}
