//! Common-subexpression elimination within a block.
//!
//! Two identical pure computations with the same operands produce the same
//! value, so the second reads the first's destination. Entries that read
//! memory (loads and tags) are dropped at every operation that may write
//! memory or have another observable effect (stores, calls, allocations,
//! concatenation, polls); the rest only depend on their operands, and
//! entries that read a local are dropped when that local is reassigned.
//!
//! A checked operation identical to an earlier one in the block (checked
//! the same way) shares its result whatever its source location: the
//! earlier one runs first, so if the check fails it reports first, and if
//! it passes the later one cannot fail.

use rustc_hash::FxHashMap;

use crate::mir::*;

/// An operand's identity. Integer constants are normalized to the bits
/// codegen actually materializes for their representation, so two constants
/// with the same value and representation (or another spelling of the same
/// bits) are interchangeable, and values of different representations are
/// not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum OpKey {
    Local(u32),
    Int(u64, u8),
    Addr(u64),
}

/// One expression, keyed by its operation and its operands' identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    Bin(u8, OpKey, OpKey, u8, bool),
    Neg(OpKey, u8, bool),
    Not(OpKey),
    Cmp(u8, OpKey, OpKey, u8),
    Convert(OpKey, u8, u8, bool),
    Fits(OpKey, u8, u8),
    Overflows(u8, OpKey, OpKey, u8),
    Tag(OpKey),
    FromWord(OpKey, u8),
    ToWord(OpKey),
    Load(OpKey, i32, u8),
}

fn op_code(op: BinOp) -> u8 {
    match op {
        BinOp::Add => 0,
        BinOp::Sub => 1,
        BinOp::Mul => 2,
        BinOp::Div => 3,
        BinOp::Rem => 4,
        BinOp::And => 5,
        BinOp::Or => 6,
        BinOp::Xor => 7,
        BinOp::Shl => 8,
        BinOp::Shr => 9,
    }
}

fn cmp_code(op: CmpOp) -> u8 {
    match op {
        CmpOp::Eq => 0,
        CmpOp::Ne => 1,
        CmpOp::Lt => 2,
        CmpOp::Le => 3,
        CmpOp::Gt => 4,
        CmpOp::Ge => 5,
    }
}

fn num_code(t: NumTy) -> u8 {
    match t {
        NumTy::I(bits, signed) => bits | if signed { 0x80 } else { 0 },
        NumTy::F(bits) => bits | 0x40,
    }
}

fn repr_code(r: Repr) -> u8 {
    match r {
        Repr::I8 => 0,
        Repr::I16 => 1,
        Repr::I32 => 2,
        Repr::I64 => 3,
        Repr::F32 => 4,
        Repr::F64 => 5,
        Repr::Ptr => 6,
    }
}

fn op_key(o: &Operand) -> Option<OpKey> {
    match o {
        Operand::Local(l) => Some(OpKey::Local(l.raw())),
        Operand::Const(Const::Int(v, r)) => {
            let bits = match r {
                Repr::I8 => 8,
                Repr::I16 => 16,
                Repr::I32 => 32,
                Repr::I64 | Repr::Ptr => 64,
                _ => return None,
            };
            let m = if bits >= 64 { u64::MAX } else { (1u64 << bits) - 1 };
            Some(OpKey::Int((*v as u64) & m, repr_code(*r)))
        }
        Operand::Const(Const::Addr(a)) => Some(OpKey::Addr(*a)),
        _ => None,
    }
}

fn key(rv: &Rvalue) -> Option<Key> {
    Some(match rv {
        // a checked operation (nonzero location) may only share the result
        // of another checked one: a wrapping result skips the check
        Rvalue::Bin { op, a, b, ty, loc } => {
            Key::Bin(op_code(*op), op_key(a)?, op_key(b)?, num_code(*ty), *loc != 0)
        }
        Rvalue::Neg { a, ty, loc } => Key::Neg(op_key(a)?, num_code(*ty), *loc != 0),
        Rvalue::Not(a) => Key::Not(op_key(a)?),
        Rvalue::Cmp { op, a, b, ty } => Key::Cmp(cmp_code(*op), op_key(a)?, op_key(b)?, num_code(*ty)),
        Rvalue::Convert { a, from, to, checked, .. } => {
            Key::Convert(op_key(a)?, num_code(*from), num_code(*to), *checked)
        }
        Rvalue::Fits { a, from, to } => Key::Fits(op_key(a)?, num_code(*from), num_code(*to)),
        Rvalue::Overflows { op, a, b, ty } => {
            Key::Overflows(op_code(*op), op_key(a)?, op_key(b)?, num_code(*ty))
        }
        Rvalue::Tag(a) => Key::Tag(op_key(a)?),
        Rvalue::FromWord { a, to } => Key::FromWord(op_key(a)?, repr_code(*to)),
        Rvalue::ToWord(a) => Key::ToWord(op_key(a)?),
        Rvalue::Load { base, offset, repr } => Key::Load(op_key(base)?, *offset, repr_code(*repr)),
        _ => return None,
    })
}

/// Whether a computation reads managed memory (which an effect may change).
fn reads_memory(k: &Key) -> bool {
    matches!(k, Key::Load(..) | Key::Tag(_))
}

fn reads_local(rv: &Rvalue, l: Local) -> bool {
    rv.operands().contains(&Operand::Local(l))
}

pub fn cse(prog: &mut Program) -> usize {
    prog.funcs.iter_mut().map(cse_func).sum()
}

fn cse_func(f: &mut Func) -> usize {
    let mut changed = 0;
    for b in 0..f.blocks.len() {
        // key → (destination, the value it currently holds)
        let mut table: FxHashMap<Key, (Local, Rvalue)> = FxHashMap::default();
        let mut i = 0;
        while i < f.blocks[b].stmts.len() {
            let (dest, rv) = match &f.blocks[b].stmts[i] {
                Stmt::Assign(d, rv) => (*d, rv.clone()),
                Stmt::Store { .. } | Stmt::GlobalStore { .. } | Stmt::Poll => {
                    table.retain(|k, _| !reads_memory(k));
                    i += 1;
                    continue;
                }
                Stmt::Loc { .. } => {
                    i += 1;
                    continue;
                }
            };
            let effect = matches!(
                rv,
                Rvalue::Call { .. }
                    | Rvalue::Alloc { .. }
                    | Rvalue::Concat(_)
                    | Rvalue::GlobalAddr(_)
                    | Rvalue::GlobalLoad(_)
            );
            let k = key(&rv);
            let mut replacement: Option<Local> = None;
            if let Some(k) = k
                && let Some((prev, _)) = table.get(&k)
                && f.locals[*prev].repr == f.locals[dest].repr
                && *prev != dest
            {
                replacement = Some(*prev);
                f.blocks[b].stmts[i] = Stmt::Assign(dest, Rvalue::Use(Operand::Local(*prev)));
                changed += 1;
            }
            // Overwriting a local invalidates expressions that read it, and
            // any expression whose result was that local.
            table.retain(|_, (d, expr)| *d != dest && !reads_local(expr, dest));
            if effect {
                table.retain(|k, _| !reads_memory(k));
            }
            if let Some(k) = k {
                let stored = match replacement {
                    Some(prev) => Rvalue::Use(Operand::Local(prev)),
                    None => rv,
                };
                table.insert(k, (dest, stored));
            }
            i += 1;
        }
    }
    changed
}
