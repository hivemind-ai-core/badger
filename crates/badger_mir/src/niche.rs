//! Nullable-pointer representation for nonescaping local options.
//!
//! A group of pointer locals connected by copies can use zero for `none`
//! and the payload pointer for `some` when every producer is a recognized
//! constructor and every consumer is a copy, tag test or payload load.
//! Function boundaries, stored values and all other observations retain
//! the boxed ABI. Pointer locals remain pointer-typed for GC rooting.

use crate::mir::*;
use badger_runtime::desc::{Kind, TypeDesc};
use rustc_hash::{FxHashMap, FxHashSet};

fn find(parent: &[usize], mut i: usize) -> usize {
    while parent[i] != i {
        i = parent[i];
    }
    i
}

/// Payload offset when zero is unavailable to every valid payload value.
fn payload_offset(desc: u64) -> Option<i32> {
    let d = unsafe { &*(desc as *const TypeDesc) };
    let Kind::Enum { variants, .. } = &d.kind else { return None };
    if d.name != "Option"
        || variants.len() != 2
        || variants[0].name != "none"
        || !variants[0].fields.is_empty()
        || variants[1].name != "some"
        || variants[1].fields.len() != 1
    {
        return None;
    }
    let field = &variants[1].fields[0];
    // Dynamic values may be zero. Reserve nested options for a future
    // representation-aware analysis; their inner option consumes a niche.
    if field.repr != Repr::Ptr
        || matches!(field.desc.kind, Kind::Dynamic)
        || matches!(&field.desc.kind, Kind::Enum { .. } if field.desc.name == "Option")
    {
        return None;
    }
    Some(field.offset as i32)
}

pub fn elide_local_options(program: &mut Program) -> usize {
    if program.live {
        return 0;
    }
    let mut count = 0;
    for f in &mut program.funcs {
        count += elide(f, &program.origins.nullary);
    }
    count
}

fn elide(f: &mut Func, nullary: &FxHashMap<u64, (u64, u32)>) -> usize {
    let mut parent: Vec<usize> = (0..f.locals.len()).collect();
    for block in &f.blocks {
        for s in &block.stmts {
            if let Stmt::Assign(dst, Rvalue::Use(Operand::Local(src))) = s
                && f.locals[*dst].repr == Repr::Ptr
                && f.locals[*src].repr == Repr::Ptr
            {
                let (a, b) = (find(&parent, dst.index()), find(&parent, src.index()));
                parent[a] = b;
            }
        }
    }
    let group = |l: Local| find(&parent, l.index());
    let mut candidates: FxHashMap<usize, (u64, i32)> = FxHashMap::default();
    let mut bad = FxHashSet::default();
    for block in &f.blocks {
        for s in &block.stmts {
            if let Stmt::Assign(dst, Rvalue::Alloc { desc, .. }) = s
                && let Some(offset) = payload_offset(*desc)
            {
                let shape = (*desc, offset);
                if let Some(previous) = candidates.insert(group(*dst), shape)
                    && previous != shape
                {
                    bad.insert(group(*dst));
                }
            }
        }
    }
    if candidates.is_empty() {
        return 0;
    }
    for l in f.params.iter().chain([&f.env]) {
        bad.insert(group(*l));
    }
    let mut sites = FxHashMap::default();
    let mut stores = FxHashSet::default();
    for (bi, block) in f.blocks.iter().enumerate() {
        for (i, s) in block.stmts.iter().enumerate() {
            if let Stmt::Assign(dst, Rvalue::Alloc { desc, .. }) = s
                && let Some(&(expected, offset)) = candidates.get(&group(*dst))
            {
                let tag = matches!(block.stmts.get(i + 1), Some(Stmt::Store {
                    base: Operand::Local(base), offset: 0, repr: Repr::I32,
                    value: Operand::Const(Const::Int(1, Repr::I32)),
                }) if base == dst);
                let payload = match block.stmts.get(i + 2) {
                    Some(Stmt::Store { base: Operand::Local(base), offset: off, repr: Repr::Ptr, value })
                        if base == dst && *off == offset =>
                    {
                        Some(*value)
                    }
                    _ => None,
                };
                if *desc == expected
                    && tag
                    && let Some(payload) = payload
                {
                    sites.insert((bi, i), payload);
                    stores.insert((bi, i + 1));
                    stores.insert((bi, i + 2));
                } else {
                    bad.insert(group(*dst));
                }
            }
        }
    }
    let option_group = |o: &Operand| match o {
        Operand::Local(l) if candidates.contains_key(&group(*l)) => Some(group(*l)),
        _ => None,
    };
    let is_none = |g: usize, o: &Operand| match o {
        Operand::Const(Const::Addr(a)) => nullary.get(a) == Some(&(candidates[&g].0, 0)),
        _ => false,
    };
    for (bi, block) in f.blocks.iter().enumerate() {
        for (i, s) in block.stmts.iter().enumerate() {
            match s {
                Stmt::Assign(dst, rv) => {
                    if let Some(g) = option_group(&Operand::Local(*dst)) {
                        let valid = match rv {
                            Rvalue::Alloc { .. } => sites.contains_key(&(bi, i)),
                            Rvalue::Use(o) => option_group(o) == Some(g) || is_none(g, o),
                            _ => false,
                        };
                        if !valid {
                            bad.insert(g);
                        }
                    }
                    for op in crate::verify::rvalue_operands(rv) {
                        if let Some(g) = option_group(op) {
                            let valid = match rv {
                                Rvalue::Use(_) => option_group(&Operand::Local(*dst)) == Some(g),
                                Rvalue::Tag(_) => true,
                                Rvalue::Load { offset, repr, .. } => {
                                    *offset == candidates[&g].1 && *repr == Repr::Ptr
                                }
                                _ => false,
                            };
                            if !valid {
                                bad.insert(g);
                            }
                        }
                    }
                }
                Stmt::Store { base, value, .. } => {
                    if let Some(g) = option_group(base)
                        && !stores.contains(&(bi, i))
                    {
                        bad.insert(g);
                    }
                    if let Some(g) = option_group(value) {
                        bad.insert(g);
                    }
                }
                Stmt::GlobalStore { value, .. } => {
                    if let Some(g) = option_group(value) {
                        bad.insert(g);
                    }
                }
                _ => {}
            }
        }
        let op = match &block.term {
            Some(Term::Return(o) | Term::Branch(o, ..) | Term::Switch(o, ..)) => Some(o),
            _ => None,
        };
        if let Some(g) = op.and_then(option_group) {
            bad.insert(g);
        }
    }
    let eligible = |l: Local| candidates.contains_key(&group(l)) && !bad.contains(&group(l));
    let mut removed = 0;
    for (bi, block) in f.blocks.iter_mut().enumerate() {
        let mut out = Vec::with_capacity(block.stmts.len());
        for (i, s) in std::mem::take(&mut block.stmts).into_iter().enumerate() {
            match s {
                Stmt::Assign(dst, Rvalue::Alloc { .. }) if eligible(dst) => {
                    out.push(Stmt::Assign(dst, Rvalue::Use(sites[&(bi, i)])));
                    removed += 1;
                }
                Stmt::Store { base: Operand::Local(base), .. } if eligible(base) => {}
                Stmt::Assign(dst, Rvalue::Use(o)) if eligible(dst) && is_none(group(dst), &o) => {
                    out.push(Stmt::Assign(dst, Rvalue::Use(Operand::Const(Const::Int(0, Repr::Ptr)))));
                }
                Stmt::Assign(dst, Rvalue::Tag(Operand::Local(src))) if eligible(src) => {
                    let test = f.locals.push(LocalDecl { repr: Repr::I8, name: None });
                    out.push(Stmt::Assign(
                        test,
                        Rvalue::Cmp {
                            op: CmpOp::Ne,
                            a: Operand::Local(src),
                            b: Operand::Const(Const::Int(0, Repr::Ptr)),
                            ty: NumTy::I(64, false),
                        },
                    ));
                    out.push(Stmt::Assign(
                        dst,
                        Rvalue::Convert {
                            a: Operand::Local(test),
                            from: NumTy::I(8, false),
                            to: NumTy::I(32, false),
                            checked: false,
                            loc: 0,
                        },
                    ));
                }
                Stmt::Assign(dst, Rvalue::Load { base: Operand::Local(src), .. }) if eligible(src) => {
                    out.push(Stmt::Assign(dst, Rvalue::Use(Operand::Local(src))));
                }
                _ => out.push(s),
            }
        }
        block.stmts = out;
    }
    removed
}
