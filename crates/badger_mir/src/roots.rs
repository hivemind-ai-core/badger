//! Which pointer locals need a GC root slot.
//!
//! The collector runs only at safe points: allocations, runtime and
//! function calls (including entering the runtime, where another worker
//! may collect), string concatenation, and polls. A pointer local needs a
//! root slot only if its value is live across one of them, or is an
//! argument of a call (runtime functions read their arguments as raw
//! words). A pointer converted to a word (`ToWord`) stays rooted for the
//! call: the word copy is not traced. Root slots are packed afresh at each
//! safe point, so dead pointers disappear and disjoint lifetimes reuse
//! storage. A function without roots needs no shadow frame at all.

use crate::mir::*;

/// Safe points: statements during which a collection may happen.
pub fn is_safepoint(s: &Stmt) -> bool {
    match s {
        Stmt::Poll => true,
        Stmt::Assign(_, rv) => matches!(rv, Rvalue::Call { .. } | Rvalue::Alloc { .. } | Rvalue::Concat(_)),
        Stmt::Store { .. } | Stmt::GlobalStore { .. } | Stmt::Loc { .. } => false,
    }
}

#[derive(Clone)]
struct Bits(Vec<u64>);

impl Bits {
    fn new(n: usize) -> Bits {
        Bits(vec![0; n.div_ceil(64)])
    }
    fn set(&mut self, i: usize) {
        self.0[i / 64] |= 1 << (i % 64);
    }
    fn clear(&mut self, i: usize) {
        self.0[i / 64] &= !(1 << (i % 64));
    }
    fn get(&self, i: usize) -> bool {
        self.0[i / 64] & (1 << (i % 64)) != 0
    }
    fn union(&mut self, o: &Bits) -> bool {
        let mut changed = false;
        for (a, b) in self.0.iter_mut().zip(&o.0) {
            let n = *a | *b;
            changed |= n != *a;
            *a = n;
        }
        changed
    }
}

fn successors(t: &Option<Term>) -> Vec<BlockId> {
    match t {
        Some(Term::Goto(x)) => vec![*x],
        Some(Term::Branch(_, a, b)) => vec![*a, *b],
        Some(Term::Switch(_, cases, d)) => cases.iter().map(|c| c.1).chain([*d]).collect(),
        _ => vec![],
    }
}

fn term_uses(t: &Option<Term>) -> Vec<&Operand> {
    match t {
        Some(Term::Branch(c, ..)) | Some(Term::Switch(c, ..)) | Some(Term::Return(c)) => vec![c],
        _ => vec![],
    }
}

fn stmt_uses(s: &Stmt) -> Vec<&Operand> {
    match s {
        Stmt::Assign(_, rv) => crate::verify::rvalue_operands(rv),
        Stmt::Store { base, value, .. } => vec![base, value],
        Stmt::GlobalStore { value, .. } => vec![value],
        Stmt::Poll | Stmt::Loc { .. } => vec![],
    }
}

fn local_of(o: &Operand) -> Option<usize> {
    match o {
        Operand::Local(l) => Some(l.index()),
        _ => None,
    }
}

/// Backward liveness transfer through block `b`.
fn transfer(f: &Func, b: BlockId, live: &mut Bits) {
    let block = &f.blocks[b];
    for o in term_uses(&block.term) {
        if let Some(i) = local_of(o) {
            live.set(i);
        }
    }
    for s in block.stmts.iter().rev() {
        if let Stmt::Assign(l, _) = s {
            live.clear(l.index());
        }
        let uses = stmt_uses(s);
        for o in uses {
            if let Some(i) = local_of(o) {
                live.set(i);
            }
        }
    }
}

/// Pointer locals that need a root slot, in local order.
pub fn safepoint_roots(f: &Func) -> Vec<Vec<Vec<Local>>> {
    let n = f.locals.len();
    let mut ptr = Bits::new(n);
    for (l, d) in f.locals.iter_enumerated() {
        if d.repr == Repr::Ptr {
            ptr.set(l.index());
        }
    }
    // A local whose only producers are null never needs tracing. Parameters
    // and environments may have an incoming non-null value.
    let mut non_null = Bits::new(n);
    non_null.set(f.env.index());
    for p in &f.params {
        non_null.set(p.index());
    }
    for block in &f.blocks {
        for s in &block.stmts {
            if let Stmt::Assign(l, rv) = s
                && !matches!(rv, Rvalue::Use(Operand::Const(Const::Addr(0) | Const::Int(0, Repr::Ptr))))
            {
                non_null.set(l.index());
            }
        }
    }
    for i in 0..n {
        if !non_null.get(i) {
            ptr.clear(i);
        }
    }
    let mut need = Bits::new(n);
    // pointers converted to words stay rooted for the whole call
    for block in &f.blocks {
        for s in &block.stmts {
            if let Stmt::Assign(_, Rvalue::ToWord(o)) = s
                && let Some(i) = local_of(o)
                && ptr.get(i)
            {
                need.set(i);
            }
        }
    }
    // live-in sets to a fixed point
    let nb = f.blocks.len();
    let mut live_in: Vec<Bits> = (0..nb).map(|_| Bits::new(n)).collect();
    let mut changed = true;
    while changed {
        changed = false;
        for b in (0..nb).rev() {
            let bid = BlockId::from_usize(b);
            let mut live = Bits::new(n);
            for s in successors(&f.blocks[bid].term) {
                if s.index() < nb {
                    live.union(&live_in[s.index()]);
                }
            }
            transfer(f, bid, &mut live);
            changed |= live_in[b].union(&live);
        }
    }
    let mut points = Vec::with_capacity(nb);
    for b in 0..nb {
        let block = &f.blocks[BlockId::from_usize(b)];
        let mut live = Bits::new(n);
        for next in successors(&block.term) {
            live.union(&live_in[next.index()]);
        }
        for o in term_uses(&block.term) {
            if let Some(i) = local_of(o) {
                live.set(i);
            }
        }
        let mut roots = vec![vec![]; block.stmts.len()];
        for (index, s) in block.stmts.iter().enumerate().rev() {
            if let Stmt::Assign(dst, _) = s {
                live.clear(dst.index());
            }
            for o in stmt_uses(s) {
                if let Some(i) = local_of(o) {
                    live.set(i);
                }
            }
            if is_safepoint(s) {
                roots[index] = f
                    .locals
                    .indices()
                    .filter(|l| ptr.get(l.index()) && (need.get(l.index()) || live.get(l.index())))
                    .collect();
            }
        }
        points.push(roots);
    }
    points
}

/// Union of the roots used at any safe point (diagnostics and metrics).
pub fn rooted_locals(f: &Func) -> Vec<Local> {
    let mut need = Bits::new(f.locals.len());
    for local in safepoint_roots(f).into_iter().flatten().flatten() {
        need.set(local.index());
    }
    f.locals.indices().filter(|l| need.get(l.index())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roots_expire_between_safe_points_but_word_sources_stay_pinned() {
        let mut f = Func::new("roots".into(), FuncKind::Normal, Repr::I64);
        let a = f.locals.push(LocalDecl { repr: Repr::Ptr, name: None });
        let b = f.locals.push(LocalDecl { repr: Repr::Ptr, name: None });
        let zero = f.locals.push(LocalDecl { repr: Repr::Ptr, name: None });
        let word = f.locals.push(LocalDecl { repr: Repr::I64, name: None });
        f.params = vec![a, b];
        f.blocks.clear();
        f.blocks.push(Block {
            stmts: vec![
                Stmt::Assign(zero, Rvalue::Use(Operand::Const(Const::Addr(0)))),
                Stmt::Poll,
                Stmt::Assign(word, Rvalue::Load { base: Operand::Local(a), offset: 0, repr: Repr::I64 }),
                Stmt::Poll,
                Stmt::Assign(word, Rvalue::ToWord(Operand::Local(b))),
                Stmt::Poll,
                Stmt::Assign(word, Rvalue::ToWord(Operand::Local(zero))),
            ],
            term: Some(Term::Return(Operand::Const(Const::Int(0, Repr::I64)))),
        });
        let points = safepoint_roots(&f);
        assert_eq!(points[0][1], vec![a, b]);
        assert_eq!(points[0][3], vec![b]);
        assert_eq!(points[0][5], vec![b]);
        assert_eq!(rooted_locals(&f), vec![a, b]);
    }
}
