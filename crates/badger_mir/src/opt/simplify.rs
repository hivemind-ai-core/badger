//! Local simplification: constant folding, arithmetic identities, copy and
//! constant propagation over dominated uses, dead-definition elimination,
//! and control-flow cleanup.
//!
//! A checked operation (nonzero source location) is folded only when its
//! operands prove it cannot fail, and is never deleted as dead code, so a
//! defect keeps its location and behaviour.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::mir::*;

use super::cfg::{self, Dominators};

const ROUNDS: usize = 6;

pub fn simplify(prog: &mut Program) -> usize {
    let mut n = 0;
    for f in prog.funcs.iter_mut() {
        n += simplify_func(f);
    }
    n
}

fn simplify_func(f: &mut Func) -> usize {
    let mut total = 0;
    for _ in 0..ROUNDS {
        let mut changed = fold(f);
        changed += propagate(f);
        changed += dead_assignments(f);
        changed += cfg::cleanup(f);
        total += changed;
        if changed == 0 {
            break;
        }
    }
    total
}

/// Fold constant operations and identities.
fn fold(f: &mut Func) -> usize {
    let bools = bool_locals(f);
    let mut changed = 0;
    for block in f.blocks.iter_mut() {
        for s in block.stmts.iter_mut() {
            let Stmt::Assign(dest, rv) = s else { continue };
            let repr = f.locals[*dest].repr;
            let Some(next) = fold_rvalue(rv, repr, &bools) else { continue };
            if next != *rv {
                *rv = next;
                changed += 1;
            }
        }
    }
    changed
}

fn mask(bits: u8) -> u64 {
    if bits >= 64 { u64::MAX } else { (1u64 << bits) - 1 }
}

fn integer(o: &Operand) -> Option<(i64, u8, bool)> {
    match o {
        Operand::Const(Const::Int(v, Repr::I8)) => Some((*v, 8, true)),
        Operand::Const(Const::Int(v, Repr::I16)) => Some((*v, 16, true)),
        Operand::Const(Const::Int(v, Repr::I32)) => Some((*v, 32, true)),
        Operand::Const(Const::Int(v, Repr::I64 | Repr::Ptr)) => Some((*v, 64, true)),
        _ => None,
    }
}

fn fold_rvalue(rv: &Rvalue, dest: Repr, bools: &FxHashSet<Local>) -> Option<Rvalue> {
    let int_const = |v: i64| Some(Rvalue::Use(Operand::Const(Const::Int(v, dest))));
    let bool_const = |v: bool| Some(Rvalue::Use(Operand::Const(Const::Int(v as i64, Repr::I8))));
    match rv {
        Rvalue::Bin { op, a, b, ty, loc } => {
            let NumTy::I(bits, signed) = *ty else { return None };
            if let (Some(x), Some(y)) = (integer(a), integer(b)) {
                let (x, y) = (x.0, y.0);
                let m = mask(bits);
                let (ux, uy) = ((x as u64) & m, (y as u64) & m);
                let bit_op = match op {
                    BinOp::And => Some(ux & uy),
                    BinOp::Or => Some(ux | uy),
                    BinOp::Xor => Some(ux ^ uy),
                    _ => None,
                };
                if let Some(v) = bit_op {
                    return int_const((v & m) as i64);
                }
                let signed_ext = |v: i64| -> i128 {
                    if signed && bits < 64 {
                        let shift = 64 - bits as u32;
                        ((v << shift >> shift) as i64) as i128
                    } else {
                        v as i128
                    }
                };
                let (lo, hi) = if signed {
                    (-(1i128 << (bits as u32 - 1)), (1i128 << (bits as u32 - 1)) - 1)
                } else {
                    (0, (1i128 << bits as u32) - 1)
                };
                let checked = *loc != 0;
                match op {
                    BinOp::Add | BinOp::Sub | BinOp::Mul => {
                        if signed {
                            let (x, y) = (signed_ext(x), signed_ext(y));
                            let wide = match op {
                                BinOp::Add => x + y,
                                BinOp::Sub => x - y,
                                _ => x * y,
                            };
                            if checked && (wide < lo || wide > hi) {
                                return None;
                            }
                            return int_const((wide as i64) & m as i64);
                        }
                        let (x, y) = (ux as i128, uy as i128);
                        let wide = match op {
                            BinOp::Add => x + y,
                            BinOp::Sub => x - y,
                            _ => x * y,
                        };
                        if checked && (wide < 0 || wide > hi) {
                            return None;
                        }
                        return int_const(((wide as u64) & m) as i64);
                    }
                    BinOp::Div | BinOp::Rem => {
                        if uy == 0 {
                            return None;
                        }
                        let x = signed_ext(x);
                        let y = signed_ext(y);
                        if signed && x == -(1i128 << (bits as u32 - 1)) && y == -1 {
                            return None;
                        }
                        let r = match op {
                            BinOp::Div => {
                                if signed {
                                    x / y
                                } else {
                                    (ux as i128) / (uy as i128)
                                }
                            }
                            _ => {
                                if signed {
                                    x % y
                                } else {
                                    (ux as i128) % (uy as i128)
                                }
                            }
                        };
                        return int_const((r as i64) & m as i64);
                    }
                    BinOp::Shl | BinOp::Shr => {
                        if uy >= bits as u64 {
                            return None;
                        }
                        let r = match (op, signed) {
                            (BinOp::Shl, _) => (ux << uy) & m,
                            (_, true) => {
                                let v = signed_ext(x) as i64;
                                let shift = 64 - bits as u32;
                                (((v << shift >> shift) >> uy) as u64) & m
                            }
                            _ => ux >> uy,
                        };
                        return int_const(r as i64);
                    }
                    BinOp::And | BinOp::Or | BinOp::Xor => unreachable!("folded above"),
                }
            }
            // identities
            let zero = |o: &Operand| matches!(o, Operand::Const(Const::Int(0, _)));
            let one = |o: &Operand| matches!(o, Operand::Const(Const::Int(1, _)));
            match (op, a, b) {
                (BinOp::Add | BinOp::Or | BinOp::Xor | BinOp::Shl | BinOp::Shr, _, x) if zero(x) => {
                    Some(Rvalue::Use(*a))
                }
                (BinOp::Add | BinOp::Or | BinOp::Xor, x, _) if zero(x) => Some(Rvalue::Use(*b)),
                (BinOp::Sub | BinOp::Shr | BinOp::Shl, _, _) if a == b && *op == BinOp::Sub => int_const(0),
                (BinOp::Mul, _, x) if one(x) => Some(Rvalue::Use(*a)),
                (BinOp::Mul, x, _) if one(x) => Some(Rvalue::Use(*b)),
                (BinOp::Mul, _, x) | (BinOp::And, _, x) if zero(x) => int_const(0),
                (BinOp::Mul, x, _) | (BinOp::And, x, _) if zero(x) => int_const(0),
                (BinOp::And, x, y) | (BinOp::Or, x, y) if x == y => Some(Rvalue::Use(*a)),
                (BinOp::And, Operand::Local(x), Operand::Const(Const::Int(1, _))) if bools.contains(x) => {
                    Some(Rvalue::Use(*a))
                }
                (BinOp::And, Operand::Const(Const::Int(1, _)), Operand::Local(x)) if bools.contains(x) => {
                    Some(Rvalue::Use(*b))
                }
                _ => None,
            }
        }
        Rvalue::Neg { a, ty, loc } => {
            let NumTy::I(bits, signed) = *ty else { return None };
            if let Some((v, _, _)) = integer(a) {
                let m = mask(bits);
                if signed && bits < 64 {
                    let shift = 64 - bits as u32;
                    let v = (v << shift >> shift) as i128;
                    let neg = -v;
                    let lo = -(1i128 << (bits as u32 - 1));
                    if *loc != 0 && neg < lo {
                        return None;
                    }
                    return int_const((neg as i64) & m as i64);
                }
                return int_const(((v as u64).wrapping_neg() & m) as i64);
            }
            None
        }
        Rvalue::Not(a) => match a {
            Operand::Const(Const::Int(v, _)) => bool_const(*v == 0),
            Operand::Local(l) if bools.contains(l) => None,
            _ => None,
        },
        Rvalue::Cmp { op, a, b, ty } => {
            let NumTy::I(bits, signed) = *ty else { return None };
            if let (Some(x), Some(y)) = (integer(a), integer(b)) {
                let (x, y) = (x.0, y.0);
                let ord = if signed {
                    sign_extend(x as u64, bits).cmp(&sign_extend(y as u64, bits))
                } else {
                    ((x as u64) & mask(bits)).cmp(&((y as u64) & mask(bits)))
                };
                let r = match op {
                    CmpOp::Eq => ord == std::cmp::Ordering::Equal,
                    CmpOp::Ne => ord != std::cmp::Ordering::Equal,
                    CmpOp::Lt => ord == std::cmp::Ordering::Less,
                    CmpOp::Le => ord != std::cmp::Ordering::Greater,
                    CmpOp::Gt => ord == std::cmp::Ordering::Greater,
                    CmpOp::Ge => ord != std::cmp::Ordering::Less,
                };
                return bool_const(r);
            }
            if a == b {
                let r = matches!(op, CmpOp::Eq | CmpOp::Le | CmpOp::Ge);
                return bool_const(r);
            }
            None
        }
        Rvalue::Fits { a, from, to } => {
            if from == to {
                return bool_const(true);
            }
            if let Some((v, _, _)) = integer(a) {
                let (_, bad) = convert_const(v, *from, *to);
                return bool_const(!bad);
            }
            None
        }
        Rvalue::Overflows { op, a, b, ty } => {
            if let (Some((x, _, _)), Some((y, _, _))) = (integer(a), integer(b)) {
                if let Some(r) = overflow_const(*op, x, y, *ty) {
                    return bool_const(r);
                }
            }
            None
        }
        Rvalue::Convert { a, from, to, checked, .. } => {
            if from == to {
                return Some(Rvalue::Use(*a));
            }
            if !matches!((from, to), (NumTy::I(..), NumTy::I(..))) {
                return None;
            }
            if let Some((v, _, _)) = integer(a) {
                let (r, bad) = convert_const(v, *from, *to);
                let lossy = lossy_conversion(*from, *to);
                if *checked && lossy && bad {
                    return None;
                }
                return Some(Rvalue::Use(Operand::Const(Const::Int(r, dest))));
            }
            None
        }
        _ => None,
    }
}

fn lossy_conversion(from: NumTy, to: NumTy) -> bool {
    let NumTy::I(fb, fs) = from else { return false };
    let NumTy::I(tb, ts) = to else { return false };
    tb < fb || fs != ts
}

fn sign_bit(v: u64, bits: u8) -> bool {
    if bits == 0 {
        false
    } else if bits >= 64 {
        (v as i64) < 0
    } else {
        v & (1u64 << (bits - 1)) != 0
    }
}

fn sign_extend(v: u64, bits: u8) -> i64 {
    if bits >= 64 {
        v as i64
    } else if bits == 0 {
        0
    } else {
        let shift = 64 - bits as u32;
        ((v << shift) as i64) >> shift
    }
}

fn convert_const(v: i64, from: NumTy, to: NumTy) -> (i64, bool) {
    let (NumTy::I(fb, fs), NumTy::I(tb, ts)) = (from, to) else { return (v, false) };
    let fmask = mask(fb);
    let tmask = mask(tb);
    let x = (v as u64) & fmask;
    // Mirror `int_convert`: extend by the source's signedness when
    // widening, reduce when narrowing, and round-trip through the source
    // width with the destination's signedness.
    let r = if tb < fb {
        x & tmask
    } else if tb > fb {
        (if fs { sign_extend(x, fb) as u64 } else { x }) & tmask
    } else {
        x
    };
    let back = if tb < fb {
        (if ts { sign_extend(r, tb) as u64 } else { r }) & fmask
    } else if tb > fb {
        r & fmask
    } else {
        r
    };
    let lossy = back != x;
    let neg_src = fs && sign_bit(x, fb);
    let neg_dst = ts && sign_bit(r, tb);
    (r as i64, lossy || neg_src != neg_dst)
}

fn overflow_const(op: BinOp, x: i64, y: i64, ty: NumTy) -> Option<bool> {
    let NumTy::I(bits, signed) = ty else { return None };
    if bits == 0 || bits > 64 {
        return None;
    }
    let m = mask(bits);
    match op {
        // codegen: only +, -, * use the arithmetic overflow flag; every
        // other operation reports a zero divisor (and signed `div` also
        // reports MIN / -1)
        BinOp::Add | BinOp::Sub | BinOp::Mul => {
            let ext = |v: i64| -> i128 {
                if signed && bits < 64 {
                    let shift = 64 - bits as u32;
                    ((v << shift >> shift) as i64) as i128
                } else {
                    v as i128
                }
            };
            let (lo, hi) = if signed {
                (-(1i128 << (bits as u32 - 1)), (1i128 << (bits as u32 - 1)) - 1)
            } else {
                (0, (1i128 << bits as u32) - 1)
            };
            let x = if signed { ext(x) } else { ((x as u64) & m) as i128 };
            let y = if signed { ext(y) } else { ((y as u64) & m) as i128 };
            let wide = match op {
                BinOp::Add => x + y,
                BinOp::Sub => x - y,
                _ => x * y,
            };
            Some(wide < lo || wide > hi)
        }
        _ => {
            let min_div = signed && op == BinOp::Div && ((x as u64) & m) == (1u64 << (bits - 1)) && y == -1;
            let zero_divisor = ((y as u64) & m) == 0;
            Some(zero_divisor || min_div)
        }
    }
}

/// Locals whose every definition produces a boolean (0 or 1).
pub(crate) fn bool_locals(f: &Func) -> FxHashSet<Local> {
    let mut defs: FxHashMap<Local, Vec<&Rvalue>> = FxHashMap::default();
    for block in &f.blocks {
        for s in &block.stmts {
            if let Stmt::Assign(l, rv) = s {
                defs.entry(*l).or_default().push(rv);
            }
        }
    }
    let mut bools: FxHashSet<Local> = FxHashSet::default();
    loop {
        let mut grew = false;
        for (l, rvs) in &defs {
            if bools.contains(l) {
                continue;
            }
            let is_bool = |o: &Operand| match o {
                Operand::Const(Const::Int(0 | 1, _)) => true,
                Operand::Local(x) => bools.contains(x),
                _ => false,
            };
            let all = !rvs.is_empty()
                && rvs.iter().all(|rv| match rv {
                    Rvalue::Cmp { .. } | Rvalue::Not(_) | Rvalue::Fits { .. } | Rvalue::Overflows { .. } => {
                        true
                    }
                    Rvalue::Use(o) => is_bool(o),
                    Rvalue::Bin { op: BinOp::And | BinOp::Or | BinOp::Xor, a, b, .. } => {
                        is_bool(a) && is_bool(b)
                    }
                    _ => false,
                });
            if all {
                bools.insert(*l);
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    bools
}

/// Reads of a statement's rvalue/operands.
fn reads(s: &Stmt) -> Vec<Operand> {
    match s {
        Stmt::Assign(_, rv) => rv.operands(),
        Stmt::Store { base, value, .. } => vec![*base, *value],
        Stmt::GlobalStore { value, .. } => vec![*value],
        Stmt::Poll | Stmt::Loc { .. } => vec![],
    }
}

fn operand_repr(f: &Func, o: &Operand) -> Option<Repr> {
    match o {
        Operand::Local(l) => Some(f.locals[*l].repr),
        Operand::Const(Const::Int(_, r)) => Some(*r),
        Operand::Const(Const::F32(_)) => Some(Repr::F32),
        Operand::Const(Const::F64(_)) => Some(Repr::F64),
        Operand::Const(Const::Addr(_)) => None,
    }
}

fn rewrite_operands(rv: &mut Rvalue, f: &mut impl FnMut(&mut Operand)) {
    match rv {
        Rvalue::Use(a) | Rvalue::Not(a) | Rvalue::ToWord(a) | Rvalue::Tag(a) => f(a),
        Rvalue::Neg { a, .. }
        | Rvalue::Convert { a, .. }
        | Rvalue::Fits { a, .. }
        | Rvalue::FromWord { a, .. } => f(a),
        Rvalue::Bin { a, b, .. } | Rvalue::Cmp { a, b, .. } | Rvalue::Overflows { a, b, .. } => {
            f(a);
            f(b);
        }
        Rvalue::Load { base, .. } => f(base),
        Rvalue::Call { callee, args, .. } => {
            for a in args {
                f(a);
            }
            if let Callee::Closure(c) | Callee::Lambda(_, c) = callee {
                f(c);
            }
        }
        Rvalue::Concat(xs) => {
            for x in xs {
                f(x);
            }
        }
        Rvalue::Alloc { .. }
        | Rvalue::GlobalLoad(_)
        | Rvalue::GlobalAddr(_)
        | Rvalue::FuncAddr(_)
        | Rvalue::AdapterAddr(_) => {}
    }
}

/// Propagate single-definition constants and stable copies into dominated
/// uses. Values do not change between the definition and the use, so the
/// substitutions preserve semantics.
fn propagate(f: &mut Func) -> usize {
    let mut defs: FxHashMap<Local, (BlockId, usize, Rvalue)> = FxHashMap::default();
    let mut multi: FxHashSet<Local> = FxHashSet::default();
    for (b, block) in f.blocks.iter_enumerated() {
        for (i, s) in block.stmts.iter().enumerate() {
            if let Stmt::Assign(l, rv) = s {
                if defs.contains_key(l) || multi.contains(l) {
                    multi.insert(*l);
                } else {
                    defs.insert(*l, (b, i, rv.clone()));
                }
            }
        }
    }
    // only single-definition locals may be propagated: a later
    // reassignment on another path would invalidate the earlier value
    for l in &multi {
        defs.remove(l);
    }
    let assigned: FxHashSet<Local> = defs.keys().copied().chain(multi.iter().copied()).collect();
    let params: FxHashSet<Local> = f.params.iter().copied().chain(std::iter::once(f.env)).collect();
    let dom = Dominators::new(f);
    let mut repl: FxHashMap<Local, Operand> = FxHashMap::default();
    for (l, (db, ds, rv)) in &defs {
        if params.contains(l) {
            continue;
        }
        match rv {
            Rvalue::Use(c @ Operand::Const(_)) => {
                // The defining assignment coerced the constant to the
                // local's representation; substituting it elsewhere is only
                // safe when that is the constant's own representation (or
                // it is a pointer-sized address).
                let same = match c {
                    Operand::Const(Const::Addr(_)) => {
                        matches!(f.locals[*l].repr, Repr::I64 | Repr::Ptr)
                    }
                    other => operand_repr(f, other) == Some(f.locals[*l].repr),
                };
                if same {
                    repl.insert(*l, *c);
                }
            }
            Rvalue::Use(Operand::Local(src)) if src != l && f.locals[*src].repr == f.locals[*l].repr => {
                if let Some((sb, ss, _)) = defs.get(src) {
                    if !multi.contains(src) {
                        if dom.dominates(*sb, *ss, *db, *ds) {
                            repl.insert(*l, Operand::Local(*src));
                        }
                    }
                } else if params.contains(src) && !assigned.contains(src) {
                    repl.insert(*l, Operand::Local(*src));
                }
            }
            _ => {}
        }
    }
    if repl.is_empty() {
        return 0;
    }
    // Resolve an operand's replacement, following copies to their source.
    let resolve = |o: Operand, b: BlockId, at: usize| -> Option<Operand> {
        let mut cur = o;
        let mut last: Option<Operand> = None;
        for _ in 0..8 {
            let Operand::Local(l) = cur else { break };
            let Some((db, ds, _)) = defs.get(&l) else { break };
            let Some(&r) = repl.get(&l) else { break };
            if r == Operand::Local(l) || !dom.dominates(*db, *ds, b, at) {
                break;
            }
            cur = r;
            last = Some(r);
        }
        last
    };
    let mut changed = 0;
    for b in 0..f.blocks.len() {
        let bid = BlockId::from_usize(b);
        let n = f.blocks[b].stmts.len();
        for i in 0..n {
            let mut hits = 0;
            let stmt = &mut f.blocks[b].stmts[i];
            match stmt {
                Stmt::Assign(_, rv) => rewrite_operands(rv, &mut |o| {
                    if let Some(r) = resolve(*o, bid, i) {
                        *o = r;
                        hits += 1;
                    }
                }),
                Stmt::Store { base, value, .. } => {
                    for o in [base, value] {
                        if let Some(r) = resolve(*o, bid, i) {
                            *o = r;
                            hits += 1;
                        }
                    }
                }
                Stmt::GlobalStore { value, .. } => {
                    if let Some(r) = resolve(*value, bid, i) {
                        *value = r;
                        hits += 1;
                    }
                }
                _ => {}
            }
            changed += hits;
        }
        let at = f.blocks[b].stmts.len();
        if let Some(t) = &mut f.blocks[b].term {
            let mut fix = |o: &mut Operand| {
                if let Some(r) = resolve(*o, bid, at) {
                    *o = r;
                    changed += 1;
                }
            };
            match t {
                Term::Branch(o, _, _) | Term::Switch(o, _, _) | Term::Return(o) => fix(o),
                _ => {}
            }
        }
    }
    changed
}

/// Whether evaluating `rv` can be removed when its result is unused: no
/// side effect, no allocation, and no possible defect.
fn removable(rv: &Rvalue) -> bool {
    match rv {
        Rvalue::Use(_)
        | Rvalue::Not(_)
        | Rvalue::Cmp { .. }
        | Rvalue::Fits { .. }
        | Rvalue::Overflows { .. }
        | Rvalue::FromWord { .. }
        | Rvalue::ToWord(_)
        | Rvalue::FuncAddr(_)
        | Rvalue::AdapterAddr(_)
        | Rvalue::GlobalAddr(_)
        | Rvalue::Tag(_)
        | Rvalue::Load { .. }
        | Rvalue::GlobalLoad(_) => true,
        Rvalue::Neg { ty, loc, .. } => match ty {
            NumTy::F(_) => true,
            NumTy::I(_, signed) => !(*signed && *loc != 0),
        },
        Rvalue::Bin { op, ty, loc, .. } => match ty {
            NumTy::F(_) => true,
            NumTy::I(..) => match op {
                BinOp::Add | BinOp::Sub | BinOp::Mul => *loc == 0,
                BinOp::And | BinOp::Or | BinOp::Xor => true,
                BinOp::Div | BinOp::Rem | BinOp::Shl | BinOp::Shr => false,
            },
        },
        Rvalue::Convert { from, to, checked, .. } => !(*checked && lossy_conversion(*from, *to)),
        Rvalue::Alloc { .. } | Rvalue::Call { .. } | Rvalue::Concat(_) => false,
    }
}

/// Remove assignments whose destination is dead and whose evaluation is
/// side-effect and defect free.
fn dead_assignments(f: &mut Func) -> usize {
    let n = f.blocks.len();
    let mut live_in: Vec<FxHashSet<Local>> = vec![FxHashSet::default(); n];
    let mut live_out: Vec<FxHashSet<Local>> = vec![FxHashSet::default(); n];
    let mut changed = true;
    while changed {
        changed = false;
        for b in (0..n).rev() {
            let mut out: FxHashSet<Local> = FxHashSet::default();
            if let Some(t) = &f.blocks[b].term {
                for s in t.successors() {
                    if s.index() < n {
                        out.extend(live_in[s.index()].iter().copied());
                    }
                }
            }
            let mut cur = out.clone();
            if let Some(t) = &f.blocks[b].term {
                match t {
                    Term::Branch(o, _, _) | Term::Switch(o, _, _) | Term::Return(o) => {
                        if let Operand::Local(l) = o {
                            cur.insert(*l);
                        }
                    }
                    _ => {}
                }
            }
            for s in f.blocks[b].stmts.iter().rev() {
                if let Stmt::Assign(l, _) = s {
                    cur.remove(l);
                }
                for o in reads(s) {
                    if let Operand::Local(l) = o {
                        cur.insert(l);
                    }
                }
            }
            if out != live_out[b] || cur != live_in[b] {
                live_out[b] = out;
                live_in[b] = cur;
                changed = true;
            }
        }
    }
    let mut removed = 0;
    for b in 0..n {
        let mut cur = live_out[b].clone();
        // the terminator reads its operands: seed them before the scan
        if let Some(t) = &f.blocks[b].term {
            match t {
                Term::Branch(o, _, _) | Term::Switch(o, _, _) | Term::Return(o) => {
                    if let Operand::Local(l) = o {
                        cur.insert(*l);
                    }
                }
                _ => {}
            }
        }
        let mut dead = vec![];
        for (i, s) in f.blocks[b].stmts.iter().enumerate().rev() {
            if let Stmt::Assign(l, rv) = s
                && !cur.contains(l)
                && removable(rv)
            {
                dead.push(i);
            }
            if let Stmt::Assign(l, _) = s {
                cur.remove(l);
            }
            for o in reads(s) {
                if let Operand::Local(l) = o {
                    cur.insert(l);
                }
            }
        }
        // collected from the last statement backwards: remove in that same
        // (descending) order so earlier indices stay valid
        for i in dead {
            f.blocks[b].stmts.remove(i);
            removed += 1;
        }
    }
    // self copies carry no information
    for block in f.blocks.iter_mut() {
        let before = block.stmts.len();
        block.stmts.retain(|s| !matches!(s, Stmt::Assign(l, Rvalue::Use(Operand::Local(src))) if l == src));
        removed += before - block.stmts.len();
    }
    removed
}
