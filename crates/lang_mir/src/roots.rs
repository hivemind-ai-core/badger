//! Which pointer locals need a GC root slot.
//!
//! The collector runs only at safe points: allocations, runtime and
//! function calls (including entering the runtime, where another worker
//! may collect), string concatenation, and polls. A pointer local needs a
//! root slot only if its value is live across one of them, or is an
//! argument of a call (runtime functions read their arguments as raw
//! words). A pointer converted to a word (`ToWord`) always keeps its slot:
//! the word copy is not traced. A function none of whose pointer locals
//! need a slot needs no shadow frame at all.

use crate::mir::*;

/// Safe points: statements during which a collection may happen.
fn is_safepoint(s: &Stmt) -> bool {
    match s {
        Stmt::Poll => true,
        Stmt::Assign(_, rv) => matches!(rv, Rvalue::Call { .. } | Rvalue::Alloc { .. } | Rvalue::Concat(_)),
        Stmt::Store { .. } | Stmt::GlobalStore { .. } | Stmt::Loc { .. } => false,
    }
}

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
    fn union_masked(&mut self, o: &Bits, mask: &Bits) {
        for ((a, b), m) in self.0.iter_mut().zip(&o.0).zip(&mask.0) {
            *a |= b & m;
        }
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

/// Backward transfer through block `b`'s statements; `need` (if given)
/// collects pointer locals live across a safe point.
fn transfer(f: &Func, b: BlockId, live: &mut Bits, ptr: &Bits, mut need: Option<&mut Bits>) {
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
        if is_safepoint(s)
            && let Some(need) = need.as_deref_mut()
        {
            need.union_masked(live, ptr);
            for o in &uses {
                if let Some(i) = local_of(o)
                    && ptr.get(i)
                {
                    need.set(i);
                }
            }
        }
        for o in uses {
            if let Some(i) = local_of(o) {
                live.set(i);
            }
        }
    }
}

/// Pointer locals that need a root slot, in local order.
pub fn rooted_locals(f: &Func) -> Vec<Local> {
    let n = f.locals.len();
    let mut ptr = Bits::new(n);
    for (l, d) in f.locals.iter_enumerated() {
        if d.repr == Repr::Ptr {
            ptr.set(l.index());
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
            transfer(f, bid, &mut live, &ptr, None);
            changed |= live_in[b].union(&live);
        }
    }
    for b in 0..nb {
        let bid = BlockId::from_usize(b);
        let mut live = Bits::new(n);
        for s in successors(&f.blocks[bid].term) {
            if s.index() < nb {
                live.union(&live_in[s.index()]);
            }
        }
        transfer(f, bid, &mut live, &ptr, Some(&mut need));
    }
    f.locals.indices().filter(|l| need.get(l.index())).collect()
}
