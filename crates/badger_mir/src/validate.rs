//! Translation validation: a debug gate that compares two MIR programs
//! structurally, bit for bit, and reports the first difference.
//!
//! The gate `BADGER_MIR_VALIDATE=1` turns on (see [`crate::opt`]) runs the
//! optimizer twice from the same lowered program and calls [`compare`] on
//! the two results: a pass that depends on hidden state, on iteration
//! order, or on anything but its input makes the two runs diverge, and the
//! first divergence is reported. The program handed to code generation is
//! the first run's, so the check never changes the compiler's output.
//!
//! The comparison walks every semantic part of the program: the entry and
//! each function's name, kind, parameters, return representation, locals,
//! blocks, statements in order, and terminator; every operand, constant,
//! callee, and representation. The first difference is reported with its
//! function, block, and statement index and both values.
//!
//! What it deliberately does *not* compare, and why:
//!
//! - a **local's debug name** — a pass may rename a local for the dump
//!   without changing the program;
//! - **`Program::warnings`, `entry_error`, `debug_files`, `origins`,
//!   `live`, `workers`, and `natives`** — lowering-side metadata the pass
//!   pipeline does not produce.
//!
//! Two runs at the *same* level must agree on everything else, so the
//! comparison is exact everywhere it looks; there is no tolerant matching.
//!
//! Comparing the *optimized* program with the *unoptimized* one is a
//! different question and is not what [`compare`] answers: the release
//! passes restructure the program by design (inlining, dead-code removal,
//! representation changes, renumbering), so no bit-for-bit comparison of
//! the two is a sound gate. The behavioural parity gate that covers that
//! question lives in the CLI (`BADGER_MIR_VALIDATE=1` runs the program
//! under both optimizers and compares the results).

use crate::mir::*;
use crate::opt::{self, Level};

/// The first difference between two programs.
#[derive(Debug)]
pub struct Difference {
    /// Index of the function in the program's table.
    pub function: usize,
    /// The function's name, for a report that does not depend on the index.
    pub name: String,
    /// The block index, when the difference is inside a function.
    pub block: Option<usize>,
    /// The statement index within the block, when the difference is a
    /// statement.
    pub statement: Option<usize>,
    /// What the first program holds.
    pub left: String,
    /// What the second program holds.
    pub right: String,
}

impl std::fmt::Display for Difference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "fn {} `{}`", self.function, self.name)?;
        if let Some(b) = self.block {
            write!(f, ", block {b}")?;
        }
        if let Some(s) = self.statement {
            write!(f, ", statement {s}")?;
        }
        write!(f, ": first run `{}`, second run `{}`", self.left, self.right)
    }
}

fn at(function: usize, name: &str, block: Option<usize>, statement: Option<usize>, left: String, right: String) -> Difference {
    Difference { function, name: name.to_string(), block, statement, left, right }
}

/// The first structural difference between `left` and `right`, or `Ok(())`
/// when the two programs are bit for bit the same.
pub fn compare(left: &Program, right: &Program) -> Result<(), Difference> {
    let program = |l: String, r: String| at(0, "<program>", None, None, l, r);
    if left.entry != right.entry {
        return Err(program(format!("{:?}", left.entry), format!("{:?}", right.entry)));
    }
    if left.entry_ret.map(|d| d as *const _) != right.entry_ret.map(|d| d as *const _) {
        return Err(program(format!("{:?}", left.entry_ret), format!("{:?}", right.entry_ret)));
    }
    if left.entry_args.map(|d| d as *const _) != right.entry_args.map(|d| d as *const _) {
        return Err(program(format!("{:?}", left.entry_args), format!("{:?}", right.entry_args)));
    }
    if left.error_report != right.error_report {
        return Err(program(format!("{:?}", left.error_report), format!("{:?}", right.error_report)));
    }
    if left.n_globals != right.n_globals || left.ptr_globals != right.ptr_globals {
        return Err(program(
            format!("{} globals, ptr {:?}", left.n_globals, left.ptr_globals),
            format!("{} globals, ptr {:?}", right.n_globals, right.ptr_globals),
        ));
    }
    if left.static_closures != right.static_closures {
        return Err(program(format!("{:?}", left.static_closures), format!("{:?}", right.static_closures)));
    }
    if left.display_hooks.len() != right.display_hooks.len()
        || left.display_hooks.iter().zip(&right.display_hooks).any(|(a, b)| a.0 as *const _ != b.0 as *const _ || a.1 != b.1)
    {
        return Err(program(format!("{:?}", left.display_hooks), format!("{:?}", right.display_hooks)));
    }
    if left.tests != right.tests {
        return Err(program(format!("{:?}", left.tests), format!("{:?}", right.tests)));
    }
    if left.test_errors.len() != right.test_errors.len()
        || left.test_errors.iter().zip(&right.test_errors).any(|(a, b)| a.0 != b.0 || a.1 as *const _ != b.1 as *const _)
    {
        return Err(program(format!("{:?}", left.test_errors), format!("{:?}", right.test_errors)));
    }
    if left.funcs.len() != right.funcs.len() {
        return Err(program(format!("{} functions", left.funcs.len()), format!("{} functions", right.funcs.len())));
    }
    for (id, (l, r)) in left.funcs.iter().zip(&right.funcs).enumerate() {
        compare_func(id, l, r)?;
    }
    Ok(())
}

fn compare_func(index: usize, l: &Func, r: &Func) -> Result<(), Difference> {
    let d = |block: Option<usize>, statement: Option<usize>, left: String, right: String| {
        at(index, if l.name.is_empty() { &r.name } else { &l.name }, block, statement, left, right)
    };
    if l.name != r.name {
        return Err(d(None, None, l.name.clone(), r.name.clone()));
    }
    if l.kind != r.kind {
        return Err(d(None, None, format!("{:?}", l.kind), format!("{:?}", r.kind)));
    }
    if l.env != r.env || l.params != r.params {
        return Err(d(
            None,
            None,
            format!("env {:?} params {:?}", l.env, l.params),
            format!("env {:?} params {:?}", r.env, r.params),
        ));
    }
    if l.ret != r.ret {
        return Err(d(None, None, format!("{:?}", l.ret), format!("{:?}", r.ret)));
    }
    if l.needs_adapter != r.needs_adapter || l.is_global != r.is_global {
        return Err(d(
            None,
            None,
            format!("adapter {} global {}", l.needs_adapter, l.is_global),
            format!("adapter {} global {}", r.needs_adapter, r.is_global),
        ));
    }
    if l.live_key != r.live_key || l.live_sig != r.live_sig {
        return Err(d(None, None, format!("{:?} {:?}", l.live_key, l.live_sig), format!("{:?} {:?}", r.live_key, r.live_sig)));
    }
    if l.locals.len() != r.locals.len() {
        return Err(d(None, None, format!("{} locals", l.locals.len()), format!("{} locals", r.locals.len())));
    }
    for (i, (a, b)) in l.locals.iter().zip(&r.locals).enumerate() {
        // The local's name is debug metadata the dump shows; a pass that
        // renames it does not change the program, so it is not compared.
        if a.repr != b.repr {
            return Err(d(None, None, format!("local {i} {:?}", a.repr), format!("local {i} {:?}", b.repr)));
        }
    }
    if l.blocks.len() != r.blocks.len() {
        return Err(d(None, None, format!("{} blocks", l.blocks.len()), format!("{} blocks", r.blocks.len())));
    }
    for (b, (lb, rb)) in l.blocks.iter().zip(&r.blocks).enumerate() {
        if lb.stmts.len() != rb.stmts.len() {
            return Err(d(Some(b), None, format!("{} statements", lb.stmts.len()), format!("{} statements", rb.stmts.len())));
        }
        for (s, (ls, rs)) in lb.stmts.iter().zip(&rb.stmts).enumerate() {
            if ls != rs {
                return Err(d(Some(b), Some(s), format!("{ls:?}"), format!("{rs:?}")));
            }
        }
        if lb.term != rb.term {
            return Err(d(Some(b), None, format!("{:?}", lb.term), format!("{:?}", rb.term)));
        }
    }
    Ok(())
}

/// Run the optimizer at `level` twice from `prog` and compare the two
/// results bit for bit; `Ok(())` when the pipeline is a function of its
/// input. The first difference is an error naming its place and both
/// values.
pub fn check(prog: &Program, level: Level) -> Result<(), String> {
    let mut left = prog.clone();
    opt::optimize(&mut left, level);
    let mut right = prog.clone();
    opt::optimize(&mut right, level);
    compare(&left, &right).map_err(|d| d.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_stmt(rv: Rvalue) -> Program {
        let mut p = Program::default();
        let mut f = Func::new("main".into(), FuncKind::Normal, Repr::I64);
        let dst = f.locals.push(LocalDecl { repr: Repr::I64, name: None });
        f.blocks[BlockId::new(0)].stmts.push(Stmt::Assign(dst, rv));
        f.blocks[BlockId::new(0)].term = Some(Term::Return(Operand::Local(dst)));
        p.funcs.push(f);
        p.entry = Some(FuncId::new(0));
        p
    }

    #[test]
    fn identical_programs_compare_equal() {
        let p = one_stmt(Rvalue::Use(Operand::Const(Const::Int(1, Repr::I64))));
        assert!(compare(&p, &p.clone()).is_ok());
    }

    #[test]
    fn first_difference_names_the_place_and_both_values() {
        let a = one_stmt(Rvalue::Bin {
            op: BinOp::Mul,
            a: Operand::Const(Const::Int(3, Repr::I64)),
            b: Operand::Const(Const::Int(2, Repr::I64)),
            ty: NumTy::I(64, true),
            loc: 0,
        });
        let b = one_stmt(Rvalue::Bin {
            op: BinOp::Shl,
            a: Operand::Const(Const::Int(3, Repr::I64)),
            b: Operand::Const(Const::Int(1, Repr::I64)),
            ty: NumTy::I(64, true),
            loc: 0,
        });
        let d = compare(&a, &b).unwrap_err();
        assert_eq!((d.function, d.block, d.statement), (0, Some(0), Some(0)));
        assert!(d.left.contains("Mul") && d.right.contains("Shl"), "{d}");
    }

    #[test]
    fn a_renamed_local_and_lowering_metadata_are_not_differences() {
        let mut a = one_stmt(Rvalue::Use(Operand::Const(Const::Int(1, Repr::I64))));
        a.funcs[FuncId::new(0)].locals[Local::new(1)].name = Some("x".into());
        let mut b = a.clone();
        b.funcs[FuncId::new(0)].locals[Local::new(1)].name = Some("renamed".into());
        b.debug_files.push("another/file.bdg".into());
        b.warnings.push("a lowering warning".into());
        assert!(compare(&a, &b).is_ok());
    }
}
