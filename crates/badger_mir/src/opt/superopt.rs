//! Superoptimization for straight-line integer, boolean, and comparison
//! regions.
//!
//! Two kinds of rewrite are applied:
//!
//! - Algebraic identities whose equivalence holds for every bit pattern at
//!   the operation's width: multiplication by a power of two is a left
//!   shift, unsigned division and remainder by a power of two are a right
//!   shift and a mask, and a left-then-right shift pair is a mask of the low
//!   bits. These are exact for the wrapping operations MIR emits (checked
//!   operations are never rewritten, and the constant shift amounts are in
//!   range, so no defect can appear or disappear).
//! - Boolean identities (`x == 1` is `x` when every definition of `x`
//!   produces 0/1, double negation, and comparison polarity) and operand
//!   canonicalization for commutative operations, which exposes more common
//!   subexpressions to [`super::cse`].
//!
//! Every boolean rewrite is additionally checked exhaustively over all
//! assignments of its free boolean variables before it is applied, so a
//! mistaken rule cannot change behaviour. Expressions with more than 16
//! free bits are never rewritten by a boolean rule.
//!
//! The rules themselves are stated, and their claims proved, in [`smt`]: the
//! match site names the rule it matched, and when the check is enabled
//! (`BADGER_MIR_SMT=1`) a rule the solver does not prove is not applied. With
//! the check off the optimiser never calls the solver.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::mir::*;

use super::simplify::bool_locals;
use super::smt::{self, Rule};

/// A rewrite candidate: the rule that justifies it and the value it rewrites
/// to. The rule's preconditions -- a shift amount in range, an operand's
/// boolean domain -- are the ones the match site established when it built
/// the candidate; they are stated with the rule in [`smt`], which the solver
/// discharges, and where the rule needs it the site re-checks them with
/// [`verify`].
struct Candidate {
    rule: Rule,
    rv: Rvalue,
}

/// `BADGER_MIR_BREAK=superopt` (debug builds only): apply the multiplication
/// rewrite with the wrong shift, the deliberately broken rewrite the
/// translation-validation gate exists to catch. A release build ignores the
/// variable, so a shipped compiler can never take this path; the gate's own
/// test sets it to prove the gate fails a miscompile. See
/// `crates/badger_cli/tests/mir_validation.rs`.
fn broken() -> bool {
    cfg!(debug_assertions) && std::env::var_os("BADGER_MIR_BREAK").is_some_and(|v| v == "superopt")
}

pub fn superopt(prog: &mut Program) -> usize {
    if smt::enabled() {
        smt::run();
        smt::report();
    }
    prog.funcs.iter_mut().map(superopt_func).sum()
}

fn superopt_func(f: &mut Func) -> usize {
    let bools = bool_locals(f);
    let mut defs: FxHashMap<Local, Rvalue> = FxHashMap::default();
    let mut multi: FxHashSet<Local> = FxHashSet::default();
    for b in f.blocks.iter() {
        for s in &b.stmts {
            if let Stmt::Assign(l, rv) = s {
                if defs.contains_key(l) || multi.contains(l) {
                    multi.insert(*l);
                } else {
                    defs.insert(*l, rv.clone());
                }
            }
        }
    }
    for l in multi {
        defs.remove(&l);
    }
    for p in &f.params {
        defs.remove(p);
    }
    defs.remove(&f.env);
    let def_at = def_positions(f);
    let mut changed = 0;
    for b in 0..f.blocks.len() {
        for i in 0..f.blocks[b].stmts.len() {
            let Stmt::Assign(dest, rv) = &f.blocks[b].stmts[i] else { continue };
            let repr = f.locals[*dest].repr;
            let mut candidate = canonicalize(rv);
            if let Some(c) = rewrite(rv, &bools, f, &defs, &def_at, b, i) {
                let ok = smt::accepted(c.rule)
                    && (!smt::needs_exhaustive(c.rule) || verify(rv, &c.rv, &defs, &bools, f).is_some());
                if ok {
                    candidate = canonicalize(&c.rv);
                }
            }
            if candidate != *rv {
                f.blocks[b].stmts[i] = Stmt::Assign(*dest, candidate);
                changed += 1;
            }
            let _ = repr;
        }
    }
    changed
}

fn int_const(v: i64) -> Operand {
    Operand::Const(Const::Int(v, Repr::I64))
}

fn const_of(o: &Operand) -> Option<i64> {
    match o {
        Operand::Const(Const::Int(v, _)) => Some(*v),
        _ => None,
    }
}

/// A nonzero power of two in the operation's bit width.
fn power_of_two(v: u64) -> Option<u8> {
    (v != 0 && v.is_power_of_two()).then(|| v.trailing_zeros() as u8)
}

/// Rules for one rvalue.
/// The rvalue defining `l` earlier in `block`, when it has exactly one
/// definition in the whole function and it is that one.
fn earlier_def(
    defs: &FxHashMap<Local, Rvalue>,
    f: &Func,
    block: usize,
    before: usize,
    l: Local,
) -> Option<Rvalue> {
    let rv = defs.get(&l)?.clone();
    for (b, blk) in f.blocks.iter_enumerated() {
        for (i, s) in blk.stmts.iter().enumerate() {
            if let Stmt::Assign(x, _) = s
                && *x == l
                && !(b.index() == block && i < before)
            {
                return None;
            }
        }
    }
    Some(rv)
}

fn def_positions(f: &Func) -> FxHashMap<Local, (usize, usize)> {
    let mut defs: FxHashMap<Local, (usize, usize)> = FxHashMap::default();
    let mut multi: FxHashSet<Local> = FxHashSet::default();
    for (b, blk) in f.blocks.iter_enumerated() {
        for (i, s) in blk.stmts.iter().enumerate() {
            if let Stmt::Assign(l, _) = s {
                if defs.contains_key(l) || multi.contains(l) {
                    multi.insert(*l);
                } else {
                    defs.insert(*l, (b.index(), i));
                }
            }
        }
    }
    for l in multi {
        defs.remove(&l);
    }
    for p in &f.params {
        defs.remove(p);
    }
    defs.remove(&f.env);
    defs
}

/// A value reused from an earlier definition is only stable when every
/// local it reads was last defined before that definition (a parameter or
/// the environment never changes).
fn stable_operands(
    rv: &Rvalue,
    def_at: &FxHashMap<Local, (usize, usize)>,
    f: &Func,
    block: usize,
    at: usize,
) -> bool {
    rv.operands().iter().all(|o| match o {
        Operand::Local(l) => {
            f.params.contains(l) || *l == f.env || def_at.get(l).is_some_and(|(b, i)| *b == block && *i < at)
        }
        _ => true,
    })
}

fn rewrite(
    rv: &Rvalue,
    bools: &FxHashSet<Local>,
    f: &Func,
    defs: &FxHashMap<Local, Rvalue>,
    def_at: &FxHashMap<Local, (usize, usize)>,
    block: usize,
    at: usize,
) -> Option<Candidate> {
    let is_bool = |o: &Operand| match o {
        Operand::Local(l) => bools.contains(l),
        Operand::Const(Const::Int(0 | 1, _)) => true,
        _ => false,
    };
    match rv {
        Rvalue::Bin { op: BinOp::Mul, a, b, ty: NumTy::I(bits, signed), loc: 0 } => {
            let (x, c) = match (const_of(a), const_of(b)) {
                (None, Some(c)) => (*a, c),
                (Some(c), None) => (*b, c),
                _ => return None,
            };
            let k = power_of_two((c as u64) & mask(*bits))?;
            if k >= *bits {
                return None;
            }
            let shift = u32::from(k) + u32::from(broken());
            if shift >= u32::from(*bits) {
                return None;
            }
            Some(Candidate {
                rule: Rule::MulPow2,
                rv: Rvalue::Bin {
                    op: BinOp::Shl,
                    a: x,
                    b: int_const(shift as i64),
                    ty: NumTy::I(*bits, *signed),
                    loc: 0,
                },
            })
        }
        Rvalue::Bin { op, a, b, ty: NumTy::I(bits, false), loc } if matches!(op, BinOp::Div | BinOp::Rem) => {
            let c = (const_of(b)? as u64) & mask(*bits);
            let k = power_of_two(c)?;
            if k >= *bits {
                return None;
            }
            Some(Candidate {
                rule: if *op == BinOp::Div { Rule::UDivPow2 } else { Rule::URemPow2 },
                rv: if *op == BinOp::Div {
                    Rvalue::Bin {
                        op: BinOp::Shr,
                        a: *a,
                        b: int_const(k as i64),
                        ty: NumTy::I(*bits, false),
                        loc: 0,
                    }
                } else {
                    Rvalue::Bin {
                        op: BinOp::And,
                        a: *a,
                        b: int_const(((c - 1) & mask(*bits)) as i64),
                        ty: NumTy::I(*bits, false),
                        loc: 0,
                    }
                },
            })
        }
        Rvalue::Bin { op: BinOp::Shr, a: Operand::Local(inner), b, ty: NumTy::I(bits, false), loc: 0 } => {
            let Rvalue::Bin { op: BinOp::Shl, a: x, b: b2, ty: inner_ty, .. } =
                earlier_def(defs, f, block, at, *inner)?
            else {
                return None;
            };
            if *b != b2
                || inner_ty != NumTy::I(*bits, false)
                || !stable_operands(&Rvalue::Use(x), def_at, f, block, at)
            {
                return None;
            }
            let k = const_of(b)? as u8;
            if k == 0 || k >= *bits {
                return None;
            }
            // (x << k) >> k keeps x's low `bits - k` bits; the high bits the
            // left shift dropped stay zero.
            Some(Candidate {
                rule: Rule::ShrShlMask,
                rv: Rvalue::Bin {
                    op: BinOp::And,
                    a: x,
                    b: int_const(mask(*bits - k) as i64),
                    ty: NumTy::I(*bits, false),
                    loc: 0,
                },
            })
        }
        Rvalue::Cmp { op: CmpOp::Eq, a, b, ty: NumTy::I(_, _) }
        | Rvalue::Cmp { op: CmpOp::Ne, a, b, ty: NumTy::I(_, _) } => {
            let op = match rv {
                Rvalue::Cmp { op, .. } => *op,
                _ => return None,
            };
            let (x, c) = match (is_bool(a), const_of(b)) {
                (true, Some(c)) => (a, c),
                (true, None) if const_of(a).is_some() => (b, const_of(a)?),
                _ => return None,
            };
            if c != 0 && c != 1 {
                return None;
            }
            let keep = match (op, c) {
                (CmpOp::Eq, 1) | (CmpOp::Ne, 0) => true,
                (CmpOp::Eq, 0) | (CmpOp::Ne, 1) => false,
                _ => return None,
            };
            Some(Candidate {
                rule: if keep { Rule::BoolCmpKeep } else { Rule::BoolCmpFlip },
                rv: if keep { Rvalue::Use(*x) } else { Rvalue::Not(*x) },
            })
        }
        Rvalue::Not(Operand::Local(inner)) => {
            match earlier_def(defs, f, block, at, *inner)? {
                Rvalue::Not(x) if is_bool(&x) && stable_operands(&Rvalue::Use(x), def_at, f, block, at) => {
                    Some(Candidate { rule: Rule::DoubleNeg, rv: Rvalue::Use(x) })
                }
                // !(a == b) is (a != b), and _not_ of an integer ordering is
                // its opposite ordering. Floating ordering is excluded (NaN);
                // floating equality and inequality are exact opposites.
                Rvalue::Cmp { op, a, b, ty } => {
                    let invertible = matches!(ty, NumTy::I(..)) || matches!(op, CmpOp::Eq | CmpOp::Ne);
                    if !invertible || !stable_operands(&Rvalue::Cmp { op, a, b, ty }, def_at, f, block, at) {
                        return None;
                    }
                    let op = match op {
                        CmpOp::Eq => CmpOp::Ne,
                        CmpOp::Ne => CmpOp::Eq,
                        CmpOp::Lt => CmpOp::Ge,
                        CmpOp::Le => CmpOp::Gt,
                        CmpOp::Gt => CmpOp::Le,
                        CmpOp::Ge => CmpOp::Lt,
                    };
                    Some(Candidate { rule: Rule::CmpInvert, rv: Rvalue::Cmp { op, a, b, ty } })
                }
                _ => None,
            }
        }
        Rvalue::Bin { op, a, b, ty, loc: 0 } => {
            let NumTy::I(bits, signed) = *ty else { return None };
            // (x + c) - c == x and (x - c) + c == x, for wrapping
            // arithmetic; checked forms are excluded by `loc: 0`.
            let other = match op {
                BinOp::Sub => BinOp::Add,
                BinOp::Add => BinOp::Sub,
                _ => {
                    // x + x == x << 1
                    if *op == BinOp::Add && a == b {
                        return Some(Candidate {
                            rule: Rule::AddSelfShl,
                            rv: Rvalue::Bin {
                                op: BinOp::Shl,
                                a: *a,
                                b: int_const(1),
                                ty: NumTy::I(bits, signed),
                                loc: 0,
                            },
                        });
                    }
                    // (x op c) op c collapses to (x op c) for & and |
                    if matches!(op, BinOp::And | BinOp::Or)
                        && let Operand::Local(inner) = a
                        && let Some(Rvalue::Bin { op: inner_op, a: x, b: c1, ty: it, loc: 0 }) =
                            earlier_def(defs, f, block, at, *inner)
                        && inner_op == *op
                        && it == *ty
                        && stable_operands(&Rvalue::Use(x), def_at, f, block, at)
                        && let Some(c2) = const_of(b)
                        && ((const_of(&c1).unwrap_or(0) as u64) & mask(bits)) == ((c2 as u64) & mask(bits))
                    {
                        return Some(Candidate {
                            rule: Rule::MaskIdempotent,
                            rv: Rvalue::Bin { op: inner_op, a: x, b: c1, ty: *ty, loc: 0 },
                        });
                    }
                    return None;
                }
            };
            // find constant on either side and the inner (x op c) form
            let (local_side, c) = match (a, b) {
                (Operand::Local(l), Operand::Const(Const::Int(c, _))) => (*l, *c),
                (Operand::Const(Const::Int(c, _)), Operand::Local(l)) => (*l, *c),
                _ => return None,
            };
            let Rvalue::Bin { op: inner_op, a: x, b: c1, ty: it, loc: 0 } =
                earlier_def(defs, f, block, at, local_side)?
            else {
                return None;
            };
            if inner_op != other
                || it != *ty
                || ((const_of(&c1)? as u64) & mask(bits)) != ((c as u64) & mask(bits))
                || !stable_operands(&Rvalue::Use(x), def_at, f, block, at)
            {
                return None;
            }
            Some(Candidate { rule: Rule::AddSubCancel, rv: Rvalue::Use(x) })
        }
        _ => None,
    }
}

fn mask(bits: u8) -> u64 {
    if bits >= 64 { u64::MAX } else { (1u64 << bits) - 1 }
}

/// Canonical operand order for commutative operations and mirrored
/// comparisons, so identical expressions compare equal.
fn canonicalize(rv: &Rvalue) -> Rvalue {
    let mut rv = rv.clone();
    match &mut rv {
        Rvalue::Bin { op: op @ (BinOp::Add | BinOp::Mul | BinOp::And | BinOp::Or | BinOp::Xor), a, b, ty, .. } => {
            // `a + b` and `b + a` agree for the integers; for the floats they
            // do not. x86-64 propagates the *first* NaN operand's payload, and
            // the payload is observable: floats are hashed by their bits and
            // the binary codec writes them bit for bit. The translation
            // validator (reports/2026-10-21-mir-translation-validation-smt.md)
            // refutes four functions of the corpus that this swap changed.
            let commutative = match ty {
                NumTy::I(..) => true,
                NumTy::F(..) => !matches!(op, BinOp::Add | BinOp::Mul),
            };
            if commutative && key(b) < key(a) {
                std::mem::swap(a, b);
            }
        }
        Rvalue::Cmp { op, a, b, .. } => {
            if matches!(op, CmpOp::Eq | CmpOp::Ne) {
                if key(b) < key(a) {
                    std::mem::swap(a, b);
                }
            } else {
                let mirrored = match op {
                    CmpOp::Gt => Some(CmpOp::Lt),
                    CmpOp::Ge => Some(CmpOp::Le),
                    _ => None,
                };
                if let Some(m) = mirrored {
                    *op = m;
                    std::mem::swap(a, b);
                }
            }
        }
        _ => {}
    }
    rv
}

fn key(o: &Operand) -> (u8, u64) {
    match o {
        Operand::Const(Const::Int(v, _)) => (0, *v as u64),
        Operand::Local(l) => (1, l.raw() as u64),
        _ => (2, 0),
    }
}

// ---------------------------------------------------------------- checking

/// The observable result of evaluating an expression: its bits, and
/// whether evaluation traps (a checked defect).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Outcome {
    value: i64,
    trap: bool,
}

fn repr_width(r: Repr) -> u8 {
    match r {
        Repr::I8 => 8,
        Repr::I16 => 16,
        Repr::I32 => 32,
        Repr::I64 | Repr::Ptr => 64,
        Repr::F32 => 32,
        Repr::F64 => 64,
    }
}

/// Free variables of an expression, with the width of their domain: one bit
/// for proven booleans, the local's representation otherwise.
fn free_vars(
    rv: &Rvalue,
    defs: &FxHashMap<Local, Rvalue>,
    bools: &FxHashSet<Local>,
    f: &Func,
    out: &mut Vec<(Local, u8)>,
    seen: &mut FxHashSet<Local>,
    depth: usize,
) {
    if depth > 16 {
        return;
    }
    for o in rv.operands() {
        let Operand::Local(l) = o else { continue };
        if bools.contains(&l) {
            // a proven boolean is an independent one-bit variable; expanding
            // its definition would hide the boolean domain and widen the
            // search far past the exhaustive budget
            if !out.iter().any(|(x, _)| *x == l) {
                out.push((l, 1));
            }
            continue;
        }
        match defs.get(&l) {
            Some(inner) if !seen.contains(&l) => {
                seen.insert(l);
                free_vars(inner, defs, bools, f, out, seen, depth + 1);
            }
            _ => {
                if out.iter().any(|(x, _)| *x == l) {
                    continue;
                }
                let w = repr_width(f.locals[l].repr);
                // a proven boolean is one bit even when stored in I8
                out.push((l, if bools.contains(&l) { 1 } else { w }));
            }
        }
    }
}

/// Exhaustively compare two expressions over at most 2^16 assignments;
/// `None` when the domain is too large or an expression reads memory.
fn verify(
    a: &Rvalue,
    b: &Rvalue,
    defs: &FxHashMap<Local, Rvalue>,
    bools: &FxHashSet<Local>,
    f: &Func,
) -> Option<Outcome> {
    let mut vars = vec![];
    let mut seen = FxHashSet::default();
    free_vars(a, defs, bools, f, &mut vars, &mut seen, 0);
    free_vars(b, defs, bools, f, &mut vars, &mut seen, 0);
    let bits: u32 = vars.iter().map(|(_, w)| *w as u32).sum();
    if bits > 16 {
        return None;
    }
    let total = 1u32 << bits;
    let mut env: FxHashMap<Local, i64> = FxHashMap::default();
    for n in 0..total {
        let mut shift = 0;
        for (l, w) in &vars {
            let v = (n >> shift) & ((1u32 << w) - 1);
            env.insert(*l, v as i64);
            shift += *w;
        }
        let x = eval_rv(a, defs, &env, f, 0)?;
        let y = eval_rv(b, defs, &env, f, 0)?;
        if x != y {
            return None;
        }
    }
    Some(Outcome { value: 0, trap: false })
}

fn eval_operand(
    o: &Operand,
    defs: &FxHashMap<Local, Rvalue>,
    env: &FxHashMap<Local, i64>,
    f: &Func,
    depth: usize,
) -> Option<Outcome> {
    match o {
        Operand::Const(Const::Int(v, _)) => Some(Outcome { value: *v, trap: false }),
        Operand::Local(l) => match env.get(l) {
            Some(v) => Some(Outcome { value: *v, trap: false }),
            None => {
                let rv = defs.get(l)?;
                eval_rv(rv, defs, env, f, depth + 1)
            }
        },
        _ => None,
    }
}

fn eval_rv(
    rv: &Rvalue,
    defs: &FxHashMap<Local, Rvalue>,
    env: &FxHashMap<Local, i64>,
    f: &Func,
    depth: usize,
) -> Option<Outcome> {
    if depth > 32 {
        return None;
    }
    let ok = |v: i64| Some(Outcome { value: v, trap: false });
    match rv {
        Rvalue::Use(o) => eval_operand(o, defs, env, f, depth),
        Rvalue::Not(o) => {
            let x = eval_operand(o, defs, env, f, depth)?;
            ok((x.value == 0) as i64)
        }
        Rvalue::Cmp { op, a, b, ty } => {
            let x = eval_operand(a, defs, env, f, depth)?;
            let y = eval_operand(b, defs, env, f, depth)?;
            if x.trap || y.trap {
                return Some(Outcome { value: 0, trap: true });
            }
            let NumTy::I(bits, signed) = *ty else { return None };
            let (x, y) = (trunc(x.value, bits), trunc(y.value, bits));
            let ord = if signed { sign(x, bits).cmp(&sign(y, bits)) } else { x.cmp(&y) };
            let r = match op {
                CmpOp::Eq => ord == std::cmp::Ordering::Equal,
                CmpOp::Ne => ord != std::cmp::Ordering::Equal,
                CmpOp::Lt => ord == std::cmp::Ordering::Less,
                CmpOp::Le => ord != std::cmp::Ordering::Greater,
                CmpOp::Gt => ord == std::cmp::Ordering::Greater,
                CmpOp::Ge => ord != std::cmp::Ordering::Less,
            };
            ok(r as i64)
        }
        Rvalue::Fits { a, from, to } => {
            let x = eval_operand(a, defs, env, f, depth)?;
            if x.trap {
                return Some(Outcome { value: 0, trap: true });
            }
            let (_, bad) = convert(x.value, *from, *to);
            ok(!bad as i64)
        }
        Rvalue::Convert { a, from, to, checked, .. } => {
            let x = eval_operand(a, defs, env, f, depth)?;
            if x.trap {
                return Some(Outcome { value: 0, trap: true });
            }
            let (v, bad) = convert(x.value, *from, *to);
            let lossy = lossy_conversion(*from, *to);
            if *checked && lossy && bad {
                return Some(Outcome { value: 0, trap: true });
            }
            ok(v)
        }
        Rvalue::Bin { op, a, b, ty, loc } => {
            let x = eval_operand(a, defs, env, f, depth)?;
            let y = eval_operand(b, defs, env, f, depth)?;
            if x.trap || y.trap {
                return Some(Outcome { value: 0, trap: true });
            }
            eval_bin(*op, x.value, y.value, *ty, *loc != 0)
        }
        Rvalue::Neg { a, ty, loc } => {
            let x = eval_operand(a, defs, env, f, depth)?;
            if x.trap {
                return Some(Outcome { value: 0, trap: true });
            }
            eval_bin(BinOp::Sub, 0, x.value, *ty, *loc != 0)
        }
        _ => None,
    }
}

fn trunc(v: i64, bits: u8) -> u64 {
    (v as u64) & mask(bits)
}

fn sign(v: u64, bits: u8) -> i64 {
    if bits >= 64 {
        v as i64
    } else {
        let shift = 64 - bits as u32;
        ((v << shift) as i64) >> shift
    }
}

fn eval_bin(op: BinOp, a: i64, b: i64, ty: NumTy, checked: bool) -> Option<Outcome> {
    let NumTy::I(bits, signed) = ty else { return None };
    let m = mask(bits);
    let trap = Outcome { value: 0, trap: true };
    let (ua, ub) = ((a as u64) & m, (b as u64) & m);
    let ok = |v: u64| Some(Outcome { value: (v & m) as i64, trap: false });
    match op {
        BinOp::And => ok(ua & ub),
        BinOp::Or => ok(ua | ub),
        BinOp::Xor => ok(ua ^ ub),
        BinOp::Shl => {
            if ub >= bits as u64 {
                return Some(trap);
            }
            ok(ua << ub)
        }
        BinOp::Shr => {
            if ub >= bits as u64 {
                return Some(trap);
            }
            if signed { ok((sign(ua, bits) >> ub) as u64) } else { ok(ua >> ub) }
        }
        BinOp::Div | BinOp::Rem => {
            if ub == 0 {
                return Some(trap);
            }
            if signed {
                let (x, y) = (sign(ua, bits) as i128, sign(ub, bits) as i128);
                if x == -(1i128 << (bits as u32 - 1)) && y == -1 {
                    if op == BinOp::Div {
                        return Some(trap);
                    }
                    return ok(0);
                }
                let r = if op == BinOp::Div { x / y } else { x % y };
                ok(r as u64)
            } else {
                let (x, y) = (ua as u128, ub as u128);
                ok(if op == BinOp::Div { (x / y) as u64 } else { (x % y) as u64 })
            }
        }
        BinOp::Add | BinOp::Sub | BinOp::Mul => {
            let (x, y) = if signed {
                (sign(ua, bits) as i128, sign(ub, bits) as i128)
            } else {
                (ua as i128, ub as i128)
            };
            let wide = match op {
                BinOp::Add => x + y,
                BinOp::Sub => x - y,
                _ => x * y,
            };
            if checked {
                let (lo, hi) = if signed {
                    (-(1i128 << (bits as u32 - 1)), (1i128 << (bits as u32 - 1)) - 1)
                } else {
                    (0, (1i128 << bits as u32) - 1)
                };
                if wide < lo || wide > hi {
                    return Some(trap);
                }
            }
            ok(wide as u64)
        }
    }
}

fn lossy_conversion(from: NumTy, to: NumTy) -> bool {
    let NumTy::I(fb, fs) = from else { return false };
    let NumTy::I(tb, ts) = to else { return false };
    tb < fb || fs != ts
}

fn convert(v: i64, from: NumTy, to: NumTy) -> (i64, bool) {
    let (NumTy::I(fb, fs), NumTy::I(tb, ts)) = (from, to) else { return (v, false) };
    let fmask = mask(fb);
    let tmask = mask(tb);
    let x = (v as u64) & fmask;
    let sign_bit = |v: u64, bits: u8| {
        if bits == 0 {
            false
        } else if bits >= 64 {
            (v as i64) < 0
        } else {
            v & (1u64 << (bits - 1)) != 0
        }
    };
    let sign_extend = |v: u64, bits: u8| -> i64 {
        if bits >= 64 {
            v as i64
        } else if bits == 0 {
            0
        } else {
            let shift = 64 - bits as u32;
            ((v << shift) as i64) >> shift
        }
    };
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
