//! Scalar replacement of nonescaping local enums. Each local gets its own
//! tag and payload slots, so copying an enum snapshots its value even when
//! another local in the copy group is subsequently reassigned.
use crate::mir::*;
use lang_runtime::desc::{Kind, TypeDesc};
use rustc_hash::{FxHashMap, FxHashSet};

type Field = (i32, Repr);

fn root(parents: &[usize], mut n: usize) -> usize {
    while parents[n] != n {
        n = parents[n];
    }
    n
}

fn fields(d: &TypeDesc) -> Option<Vec<Field>> {
    let Kind::Enum { variants, .. } = &d.kind else { return None };
    let mut out = vec![];
    for v in variants {
        for f in &v.fields {
            let key = (f.offset as i32, f.repr);
            if !out.contains(&key) {
                out.push(key);
            }
        }
    }
    (out.len() <= 8).then_some(out)
}

pub fn scalarize(program: &mut Program) -> usize {
    if program.live {
        return 0;
    }
    program.funcs.iter_mut().map(|f| scalarize_func(f, &program.origins.nullary)).sum()
}

fn scalarize_func(f: &mut Func, nullary: &FxHashMap<u64, (u64, u32)>) -> usize {
    let mut parents: Vec<_> = (0..f.locals.len()).collect();
    for b in &f.blocks {
        for s in &b.stmts {
            if let Stmt::Assign(a, Rvalue::Use(Operand::Local(b))) = s
                && f.locals[*a].repr == Repr::Ptr
                && f.locals[*b].repr == Repr::Ptr
            {
                let (a, b) = (root(&parents, a.index()), root(&parents, b.index()));
                parents[a] = b;
            }
        }
    }
    let group = |l: Local| root(&parents, l.index());
    let mut shapes: FxHashMap<usize, (u64, Vec<Field>)> = FxHashMap::default();
    let mut bad = FxHashSet::default();
    for b in &f.blocks {
        for s in &b.stmts {
            if let Stmt::Assign(l, Rvalue::Alloc { desc, .. }) = s
                && let Some(fs) = fields(unsafe { &*(*desc as *const TypeDesc) })
                && let Some((old, _)) = shapes.insert(group(*l), (*desc, fs))
                && old != *desc
            {
                bad.insert(group(*l));
            }
        }
    }
    if shapes.is_empty() {
        return 0;
    }
    for l in f.params.iter().chain([&f.env]) {
        bad.insert(group(*l));
    }
    let candidate = |o: &Operand| match o {
        Operand::Local(l) if shapes.contains_key(&group(*l)) => Some(group(*l)),
        _ => None,
    };
    let constant = |g: usize, o: &Operand| match o {
        Operand::Const(Const::Addr(a)) => {
            nullary.get(a).filter(|(d, _)| *d == shapes[&g].0).map(|(_, tag)| *tag)
        }
        _ => None,
    };
    // Require the complete, adjacent constructor emitted by lowering.
    let mut constructors = FxHashMap::default();
    let mut initializers = FxHashSet::default();
    for (bi, b) in f.blocks.iter().enumerate() {
        for (i, s) in b.stmts.iter().enumerate() {
            if let Stmt::Assign(l, Rvalue::Alloc { desc, .. }) = s
                && shapes.contains_key(&group(*l))
            {
                let d = unsafe { &*(*desc as *const TypeDesc) };
                let tag = match b.stmts.get(i + 1) {
                    Some(Stmt::Store {
                        base: Operand::Local(base),
                        offset: 0,
                        repr: Repr::I32,
                        value: Operand::Const(Const::Int(tag, Repr::I32)),
                    }) if base == l => Some(*tag as u32),
                    _ => None,
                };
                let valid = tag.and_then(|t| d.variant(t)).is_some_and(|v| {
                    v.fields.iter().enumerate().all(|(j, field)| {
                        matches!(b.stmts.get(i + 2 + j),
                        Some(Stmt::Store { base: Operand::Local(base), offset, repr, .. })
                            if base == l && *offset == field.offset as i32 && *repr == field.repr)
                    })
                });
                if valid && *desc == shapes[&group(*l)].0 {
                    let tag = tag.unwrap();
                    constructors.insert((bi, i), tag);
                    for j in 1..=1 + d.variant(tag).unwrap().fields.len() {
                        initializers.insert((bi, i + j));
                    }
                } else {
                    bad.insert(group(*l));
                }
            }
        }
    }
    for (bi, b) in f.blocks.iter().enumerate() {
        for (i, s) in b.stmts.iter().enumerate() {
            match s {
                Stmt::Assign(dst, rv) => {
                    if let Some(g) = candidate(&Operand::Local(*dst)) {
                        let valid = match rv {
                            Rvalue::Alloc { .. } => constructors.contains_key(&(bi, i)),
                            Rvalue::Use(o) => candidate(o) == Some(g) || constant(g, o).is_some(),
                            _ => false,
                        };
                        if !valid {
                            bad.insert(g);
                        }
                    }
                    for o in crate::verify::rvalue_operands(rv) {
                        if let Some(g) = candidate(o) {
                            let valid = match rv {
                                Rvalue::Use(_) => candidate(&Operand::Local(*dst)) == Some(g),
                                Rvalue::Tag(_) => true,
                                Rvalue::Load { offset, repr, .. } => shapes[&g].1.contains(&(*offset, *repr)),
                                _ => false,
                            };
                            if !valid {
                                bad.insert(g);
                            }
                        }
                    }
                }
                Stmt::Store { base, value, .. } => {
                    if let Some(g) = candidate(base)
                        && !initializers.contains(&(bi, i))
                    {
                        bad.insert(g);
                    }
                    if let Some(g) = candidate(value) {
                        bad.insert(g);
                    }
                }
                Stmt::GlobalStore { value, .. } => {
                    if let Some(g) = candidate(value) {
                        bad.insert(g);
                    }
                }
                _ => {}
            }
        }
        let o = match &b.term {
            Some(Term::Return(o) | Term::Branch(o, ..) | Term::Switch(o, ..)) => Some(o),
            _ => None,
        };
        if let Some(g) = o.and_then(candidate) {
            bad.insert(g);
        }
    }
    let eligible = |l: Local| shapes.contains_key(&group(l)) && !bad.contains(&group(l));
    let mut slots = FxHashMap::default();
    let locals: Vec<_> = f.locals.indices().filter(|l| eligible(*l)).collect();
    for l in locals {
        let tag = f.locals.push(LocalDecl { repr: Repr::I32, name: None });
        let fs: FxHashMap<_, _> = shapes[&group(l)]
            .1
            .iter()
            .map(|key| (*key, f.locals.push(LocalDecl { repr: key.1, name: None })))
            .collect();
        slots.insert(l, (tag, fs));
    }
    let initialize = |dst: Local, tag: u32, out: &mut Vec<Stmt>| {
        out.push(Stmt::Assign(dst, Rvalue::Use(zero(Repr::Ptr))));
        out.push(Stmt::Assign(slots[&dst].0, Rvalue::Use(Operand::Const(Const::Int(tag as i64, Repr::I32)))));
        // Inactive pointer fields must not keep the previous variant alive.
        for (&(_, repr), &l) in &slots[&dst].1 {
            out.push(Stmt::Assign(l, Rvalue::Use(zero(repr))));
        }
    };
    let mut removed = 0;
    for (bi, b) in f.blocks.iter_mut().enumerate() {
        let mut out = vec![];
        for (i, s) in std::mem::take(&mut b.stmts).into_iter().enumerate() {
            match s {
                Stmt::Assign(dst, Rvalue::Alloc { .. }) if eligible(dst) => {
                    initialize(dst, constructors[&(bi, i)], &mut out);
                    removed += 1;
                }
                Stmt::Assign(dst, Rvalue::Use(o)) if eligible(dst) => {
                    if let Operand::Local(src) = o {
                        out.push(Stmt::Assign(dst, Rvalue::Use(zero(Repr::Ptr))));
                        out.push(Stmt::Assign(slots[&dst].0, Rvalue::Use(Operand::Local(slots[&src].0))));
                        for (key, l) in &slots[&dst].1 {
                            out.push(Stmt::Assign(*l, Rvalue::Use(Operand::Local(slots[&src].1[key]))));
                        }
                    } else {
                        initialize(dst, constant(group(dst), &o).unwrap(), &mut out);
                    }
                }
                Stmt::Store { base: Operand::Local(base), offset, repr, value } if eligible(base) => {
                    if offset != 0 {
                        out.push(Stmt::Assign(slots[&base].1[&(offset, repr)], Rvalue::Use(value)));
                    }
                }
                Stmt::Assign(dst, Rvalue::Tag(Operand::Local(base))) if eligible(base) => {
                    out.push(Stmt::Assign(dst, Rvalue::Use(Operand::Local(slots[&base].0))));
                }
                Stmt::Assign(dst, Rvalue::Load { base: Operand::Local(base), offset, repr })
                    if eligible(base) =>
                {
                    out.push(Stmt::Assign(dst, Rvalue::Use(Operand::Local(slots[&base].1[&(offset, repr)]))));
                }
                _ => out.push(s),
            }
        }
        b.stmts = out;
    }
    removed
}

fn zero(repr: Repr) -> Operand {
    Operand::Const(match repr {
        Repr::F32 => Const::F32(0.0),
        Repr::F64 => Const::F64(0.0),
        _ => Const::Int(0, repr),
    })
}
