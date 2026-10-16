//! Scalar replacement of aggregates: a record allocated in a function that
//! never leaves it (only its fields are written and read, possibly through
//! plain copies of the reference) is not heap-allocated; each field becomes
//! a local. Records that escape — passed, returned, stored, captured,
//! compared, or displayed — keep their heap object.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::mir::{Const, Func, Local, Operand, Repr, Rvalue, Stmt, Term};

fn operands(rv: &Rvalue) -> Vec<Operand> {
    match rv {
        Rvalue::Use(a) | Rvalue::Not(a) | Rvalue::ToWord(a) | Rvalue::Tag(a) => vec![*a],
        Rvalue::Neg { a, .. }
        | Rvalue::Convert { a, .. }
        | Rvalue::Fits { a, .. }
        | Rvalue::FromWord { a, .. } => vec![*a],
        Rvalue::Bin { a, b, .. } | Rvalue::Cmp { a, b, .. } | Rvalue::Overflows { a, b, .. } => vec![*a, *b],
        Rvalue::Load { base, .. } => vec![*base],
        Rvalue::Call { callee, args, .. } => {
            let mut v = args.clone();
            if let crate::mir::Callee::Closure(c) = callee {
                v.push(*c);
            }
            v
        }
        Rvalue::Concat(xs) => xs.clone(),
        Rvalue::Alloc { .. }
        | Rvalue::GlobalLoad(_)
        | Rvalue::GlobalAddr(_)
        | Rvalue::FuncAddr(_)
        | Rvalue::AdapterAddr(_) => vec![],
    }
}

pub fn scalar_replace(f: &mut Func) {
    // definitions per local
    let mut defs: FxHashMap<Local, usize> = FxHashMap::default();
    for b in f.blocks.iter() {
        for s in &b.stmts {
            if let Stmt::Assign(l, _) = s {
                *defs.entry(*l).or_default() += 1;
            }
        }
    }
    let single = |l: &Local| defs.get(l) == Some(&1) && !f.params.contains(l) && *l != f.env;
    // allocation roots and their aliases (single-definition copies)
    let mut group: FxHashMap<Local, Local> = FxHashMap::default();
    for b in f.blocks.iter() {
        for s in &b.stmts {
            // any object used only through field loads and stores (records,
            // tuples, ranges); tag reads, calls, and copies elsewhere escape
            if let Stmt::Assign(l, Rvalue::Alloc { .. }) = s
                && single(l)
            {
                group.insert(*l, *l);
            }
        }
    }
    loop {
        let mut grew = false;
        for b in f.blocks.iter() {
            for s in &b.stmts {
                if let Stmt::Assign(l, Rvalue::Use(Operand::Local(src))) = s
                    && single(l)
                    && !group.contains_key(l)
                    && let Some(root) = group.get(src).copied()
                {
                    group.insert(*l, root);
                    grew = true;
                }
            }
        }
        if !grew {
            break;
        }
    }
    if group.is_empty() {
        return;
    }
    // any other use makes the whole group escape
    let mut escaped: FxHashSet<Local> = FxHashSet::default();
    let mut fields: FxHashMap<(Local, i32), Repr> = FxHashMap::default();
    let mut conflict: FxHashSet<Local> = FxHashSet::default();
    let note = |root: Local,
                off: i32,
                r: Repr,
                fields: &mut FxHashMap<(Local, i32), Repr>,
                conflict: &mut FxHashSet<Local>| {
        if let Some(prev) = fields.insert((root, off), r)
            && prev != r
        {
            conflict.insert(root);
        }
    };
    let esc = |op: &Operand, escaped: &mut FxHashSet<Local>| {
        if let Operand::Local(l) = op
            && let Some(r) = group.get(l)
        {
            escaped.insert(*r);
        }
    };
    for b in f.blocks.iter() {
        for s in &b.stmts {
            match s {
                Stmt::Assign(_, Rvalue::Use(Operand::Local(src))) if group.contains_key(src) => {
                    // an alias copy (checked above) or a copy into a
                    // multiply-assigned local (escape)
                }
                Stmt::Assign(_, Rvalue::Load { base: Operand::Local(m), offset, repr })
                    if group.contains_key(m) =>
                {
                    note(group[m], *offset, *repr, &mut fields, &mut conflict);
                }
                Stmt::Assign(_, rv) => {
                    for op in operands(rv) {
                        esc(&op, &mut escaped);
                    }
                }
                Stmt::Store { base: Operand::Local(m), offset, repr, value } if group.contains_key(m) => {
                    note(group[m], *offset, *repr, &mut fields, &mut conflict);
                    esc(value, &mut escaped);
                }
                Stmt::Store { base, value, .. } => {
                    esc(base, &mut escaped);
                    esc(value, &mut escaped);
                }
                Stmt::GlobalStore { value, .. } => esc(value, &mut escaped),
                Stmt::Poll | Stmt::Loc { .. } => {}
            }
        }
        match &b.term {
            Some(Term::Branch(c, _, _)) | Some(Term::Switch(c, _, _)) | Some(Term::Return(c)) => {
                esc(c, &mut escaped)
            }
            _ => {}
        }
    }
    // copies into locals that are assigned more than once escape
    for b in f.blocks.iter() {
        for s in &b.stmts {
            if let Stmt::Assign(l, Rvalue::Use(Operand::Local(src))) = s
                && let Some(r) = group.get(src)
                && !group.contains_key(l)
            {
                escaped.insert(*r);
            }
        }
    }
    let keep: FxHashSet<Local> =
        group.values().copied().filter(|r| !escaped.contains(r) && !conflict.contains(r)).collect();
    if keep.is_empty() {
        return;
    }
    // one local per field
    let mut field_local: FxHashMap<(Local, i32), Local> = FxHashMap::default();
    let mut keys: Vec<((Local, i32), Repr)> =
        fields.into_iter().filter(|((r, _), _)| keep.contains(r)).collect();
    keys.sort_by_key(|((r, o), _)| (r.index(), *o));
    for (k, repr) in keys {
        let l = f.locals.push(crate::mir::LocalDecl { repr, name: None });
        field_local.insert(k, l);
    }
    let null = Operand::Const(Const::Int(0, Repr::Ptr));
    for b in f.blocks.iter_mut() {
        for s in b.stmts.iter_mut() {
            let replacement = match s {
                Stmt::Assign(l, Rvalue::Alloc { .. }) if group.get(l).is_some_and(|r| keep.contains(r)) => {
                    Some(Stmt::Assign(*l, Rvalue::Use(null)))
                }
                Stmt::Assign(l, Rvalue::Use(Operand::Local(src)))
                    if group.get(src).is_some_and(|r| keep.contains(r)) && group.contains_key(l) =>
                {
                    Some(Stmt::Assign(*l, Rvalue::Use(null)))
                }
                Stmt::Assign(l, Rvalue::Load { base: Operand::Local(m), offset, .. })
                    if group.get(m).is_some_and(|r| keep.contains(r)) =>
                {
                    let fl = field_local[&(group[m], *offset)];
                    Some(Stmt::Assign(*l, Rvalue::Use(Operand::Local(fl))))
                }
                Stmt::Store { base: Operand::Local(m), offset, value, .. }
                    if group.get(m).is_some_and(|r| keep.contains(r)) =>
                {
                    let fl = field_local[&(group[m], *offset)];
                    Some(Stmt::Assign(fl, Rvalue::Use(*value)))
                }
                _ => None,
            };
            if let Some(r) = replacement {
                *s = r;
            }
        }
    }
}
