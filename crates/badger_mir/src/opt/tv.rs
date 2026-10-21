//! Translation validation of the MIR passes, in the style of Alive2 (Lopes
//! et al., PLDI 2021): a pass's output is checked against its input, one
//! function at a time, by asking Z3 whether the two can be told apart.
//!
//! # The model
//!
//! A function's meaning is what code generation makes of it ([`smt`] states
//! each operation as the Cranelift sequence `badger_codegen` emits): the
//! locals are a map from place to bitvector (every local starts at zero, the
//! parameters at shared inputs), the integer operations are exact, a checked
//! operation and a division or shift report their defect -- kind and source
//! location -- under the condition the generated code tests, and control
//! flow is a path condition, with the values merged where paths join.
//! Everything else is uninterpreted: memory and every effect (a call, an
//! allocation, a store, a poll) thread an opaque *world*, an effect is a
//! function of the world and its arguments, and a load, a tag, or a float
//! operation is a function of its operands (and the world it reads). An
//! uninterpreted operation is equal on both sides only when it is applied to
//! equal arguments, so the abstraction can fail a proof but never make one.
//!
//! Two functions are equivalent when, from equal inputs and an equal world,
//! they end the same way: both return equal values in equal worlds, or both
//! report the same defect at the same location in equal worlds, or both
//! reach `Unreachable` in equal worlds.
//!
//! # Loops
//!
//! A function with a loop is cut at its loop headers (the targets of the
//! back edges of either side's depth-first order), so each *region* between
//! cuts is acyclic and is executed symbolically. At a cut the two sides are
//! related by an invariant: a conjunction of candidate facts (a local equal
//! on both sides, a source copy equal to the target's original, a
//! single-definition constant, a boolean's domain) from which every
//! candidate not preserved by every region is removed until the rest are
//! inductive (Houdini, Flanagan and Leino, FME 2001). Each region is then
//! proved under its invariant to reach the same exit as the other side with
//! the invariant of that exit established -- a simulation, so the proof
//! covers every iteration. The cuts correspond by block index, which the
//! passes in reach keep; where they do not, the proof fails.
//!
//! # Verdicts
//!
//! [`Verdict::Equivalent`] is the solver's *unsatisfiable* on every claim, a
//! proof. [`Verdict::Refuted`] is a satisfying assignment: the two functions
//! differ on it *in the model*, which is a real miscompile when the region
//! has no uninterpreted operation and no loop invariant the proof needed,
//! and otherwise the reason the proof failed. [`Verdict::NotProven`] is
//! everything else: the function's shape changed (its parameters, locals,
//! or loop structure), a query did not finish within its budget, or the
//! budget for the function ran out. Nothing outside the model is ever
//! called equivalent.
//!
//! The check never changes the program: it reads the input and output of a
//! pass and reports.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use rustc_hash::{FxHashMap, FxHashSet};
use z3::ast::{Ast, BV, Bool, Dynamic};
use z3::{FuncDecl, Params, SatResult, Solver, Sort};

use crate::mir::*;

use super::smt;

/// What the validator concluded about one function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Every claim is unsatisfiable in negation: the two functions agree on
    /// every input.
    Equivalent,
    /// The solver found an assignment on which the two differ in the model;
    /// the string names the region and the inputs.
    Refuted(String),
    /// Neither proved nor refuted; the string is the reason.
    NotProven(String),
}

impl Verdict {
    /// The verdict's class, for a report: `equivalent`, `refuted`, or `not proven`.
    pub fn word(&self) -> &'static str {
        match self {
            Verdict::Equivalent => "equivalent",
            Verdict::Refuted(_) => "refuted",
            Verdict::NotProven(_) => "not proven",
        }
    }
}

/// Per-query solver budget.
const QUERY_TIMEOUT: Duration = Duration::from_secs(2);

/// Budget for one function; past it the function is [`Verdict::NotProven`].
pub const FUNCTION_BUDGET: Duration = Duration::from_secs(10);

/// Whether two functions are the same program: equal signature, locals'
/// representations, blocks and terminators. A pass that leaves a function
/// so is not asked about it.
pub fn same_func(a: &Func, b: &Func) -> bool {
    a.params == b.params
        && a.env == b.env
        && a.ret == b.ret
        && a.locals.len() == b.locals.len()
        && a.locals.iter().zip(&b.locals).all(|(x, y)| x.repr == y.repr)
        && a.blocks.len() == b.blocks.len()
        && a.blocks.iter().zip(&b.blocks).all(|(x, y)| x.stmts == y.stmts && x.term == y.term)
}

/// Decide whether `tgt` preserves the meaning of `src`, within
/// [`FUNCTION_BUDGET`].
pub fn validate(src: &Func, tgt: &Func) -> Verdict {
    validate_within(src, tgt, FUNCTION_BUDGET)
}

/// [`validate`] with an explicit budget for the function.
pub fn validate_within(src: &Func, tgt: &Func, budget: Duration) -> Verdict {
    if let Some(reason) = shape_change(src, tgt) {
        return Verdict::NotProven(reason);
    }
    let deadline = Instant::now() + budget;
    match Validation::new(src, tgt, deadline).and_then(|mut v| v.run()) {
        Ok(()) => Verdict::Equivalent,
        Err(v) => v,
    }
}

/// The part of a function the validator relates by identity and does not
/// prove: a pass in reach keeps the signature and the locals.
fn shape_change(src: &Func, tgt: &Func) -> Option<String> {
    if src.params != tgt.params || src.env != tgt.env || src.ret != tgt.ret || src.kind != tgt.kind {
        return Some("the signature changed".into());
    }
    if src.locals.len() != tgt.locals.len() {
        return Some(format!("the locals changed ({} to {})", src.locals.len(), tgt.locals.len()));
    }
    if src.locals.iter().zip(&tgt.locals).any(|(a, b)| a.repr != b.repr) {
        return Some("a local's representation changed".into());
    }
    if src.blocks.is_empty() || tgt.blocks.is_empty() {
        return Some("a function has no blocks".into());
    }
    None
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
enum Side {
    Src,
    Tgt,
}

impl Side {
    fn tag(self) -> char {
        match self {
            Side::Src => 's',
            Side::Tgt => 't',
        }
    }
}

/// Where a region begins: the function's entry, or a cut at a loop header.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Origin {
    Start,
    Cut(BlockId),
}

/// How a region ends.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum ExitKind {
    Return,
    /// A reported defect: its kind and source location.
    Defect(u8, u64),
    Unreachable,
    /// Control reaches a cut.
    Cut(BlockId),
}

/// The values on one path: the locals assigned since the region began (the
/// rest hold the region's initial values) and the world.
#[derive(Clone)]
struct State {
    vals: FxHashMap<Local, BV>,
    world: Dynamic,
}

struct Exit {
    guard: Bool,
    kind: ExitKind,
    state: State,
    /// The returned value, coerced to the return representation.
    value: Option<BV>,
}

/// The uninterpreted functions and the world's sort, shared by both sides so
/// that an operation means the same on each.
struct Encoder {
    world: Sort,
    decls: RefCell<FxHashMap<String, Rc<FuncDecl>>>,
}

impl Encoder {
    fn new() -> Encoder {
        // a multiplication by a power of two is rewritten as the shift it
        // equals, so `x * 8` and `x << 3` are one term to the solver rather
        // than two multipliers to bit-blast
        z3::set_global_param("rewriter.mul2concat", "true");
        Encoder { world: Sort::uninterpreted("World".into()), decls: RefCell::new(FxHashMap::default()) }
    }

    fn decl(&self, name: &str, domain: &[Sort], range: &Sort) -> Rc<FuncDecl> {
        self.decls
            .borrow_mut()
            .entry(name.to_string())
            .or_insert_with(|| {
                let domain: Vec<&Sort> = domain.iter().collect();
                Rc::new(FuncDecl::new(name, &domain, range))
            })
            .clone()
    }

    /// An uninterpreted value of `bits` named `name` over `args`.
    fn value(&self, name: &str, args: &[&dyn Ast], domain: &[Sort], bits: u32) -> BV {
        self.decl(name, domain, &Sort::bitvector(bits)).apply(args).as_bv().expect("bitvector range")
    }

    fn world_at(&self, origin: Origin) -> Dynamic {
        let name = match origin {
            Origin::Start => "w.start".to_string(),
            Origin::Cut(h) => format!("w.h{}", h.index()),
        };
        Dynamic::new_const(name, &self.world)
    }
}

/// Live-in locals per block, as bitsets.
struct Liveness {
    words: usize,
    live_in: Vec<Vec<u64>>,
}

impl Liveness {
    fn new(f: &Func) -> Liveness {
        let n = f.blocks.len();
        let words = f.locals.len().div_ceil(64).max(1);
        let mut live_in = vec![vec![0u64; words]; n];
        let set = |bits: &mut [u64], l: Local| bits[l.index() / 64] |= 1 << (l.index() % 64);
        let clear = |bits: &mut [u64], l: Local| bits[l.index() / 64] &= !(1 << (l.index() % 64));
        let mut changed = true;
        while changed {
            changed = false;
            for b in (0..n).rev() {
                let block = &f.blocks[BlockId::from_usize(b)];
                let mut cur = vec![0u64; words];
                if let Some(t) = &block.term {
                    for s in t.successors() {
                        if s.index() < n {
                            for (c, x) in cur.iter_mut().zip(&live_in[s.index()]) {
                                *c |= *x;
                            }
                        }
                    }
                    if let Term::Branch(Operand::Local(l), ..)
                    | Term::Switch(Operand::Local(l), ..)
                    | Term::Return(Operand::Local(l)) = t
                    {
                        set(&mut cur, *l);
                    }
                }
                for s in block.stmts.iter().rev() {
                    if let Stmt::Assign(l, _) = s {
                        clear(&mut cur, *l);
                    }
                    for o in stmt_operands(s) {
                        if let Operand::Local(l) = o {
                            set(&mut cur, l);
                        }
                    }
                }
                if cur != live_in[b] {
                    live_in[b] = cur;
                    changed = true;
                }
            }
        }
        Liveness { words, live_in }
    }

    fn contains(&self, b: BlockId, l: Local) -> bool {
        self.live_in[b.index()][l.index() / 64] & (1 << (l.index() % 64)) != 0
    }

    fn locals(&self, b: BlockId) -> impl Iterator<Item = Local> + '_ {
        (0..self.words * 64)
            .filter(move |i| self.live_in[b.index()][i / 64] & (1 << (i % 64)) != 0)
            .map(Local::from_usize)
    }
}

fn stmt_operands(s: &Stmt) -> Vec<Operand> {
    match s {
        Stmt::Assign(_, rv) => rv.operands(),
        Stmt::Store { base, value, .. } => vec![*base, *value],
        Stmt::GlobalStore { value, .. } => vec![*value],
        Stmt::Poll | Stmt::Loc { .. } => vec![],
    }
}

/// The back edges of a depth-first walk from the entry. Every cycle has
/// one; their targets are the loop headers.
fn back_edges(f: &Func) -> FxHashSet<(BlockId, BlockId)> {
    let n = f.blocks.len();
    // 0 unvisited, 1 on the stack, 2 done
    let mut mark = vec![0u8; n];
    let mut back = FxHashSet::default();
    let succ = |b: usize| -> Vec<BlockId> {
        f.blocks[BlockId::from_usize(b)]
            .term
            .as_ref()
            .map(|t| t.successors())
            .unwrap_or_default()
            .into_iter()
            .filter(|s| s.index() < n)
            .collect()
    };
    let mut stack: Vec<(usize, Vec<BlockId>)> = vec![(0, succ(0))];
    mark[0] = 1;
    while let Some((b, rest)) = stack.last_mut() {
        if let Some(s) = rest.pop() {
            match mark[s.index()] {
                0 => {
                    mark[s.index()] = 1;
                    let next = succ(s.index());
                    stack.push((s.index(), next));
                }
                1 => {
                    back.insert((BlockId::from_usize(*b), s));
                }
                _ => {}
            }
        } else {
            mark[*b] = 2;
            stack.pop();
        }
    }
    back
}

/// One side's symbolic execution.
struct Exec<'a> {
    enc: &'a Encoder,
    f: &'a Func,
    side: Side,
    live: Liveness,
    inputs: FxHashSet<Local>,
    /// The back edges into this side's loop headers that are not loop
    /// headers on the other side: a region leaves by them (the loop is cut
    /// at its back edge rather than at its header, so a loop the other side
    /// does not have must never iterate for the proof to go through).
    back: FxHashSet<(BlockId, BlockId)>,
    /// Each cut's invariant's equalities as a substitution: a local the
    /// invariant holds equal to another, on either side, or to a constant
    /// starts from the representative of its class rather than from its own
    /// unknown, so equal values are one term on both sides.
    shared: Rc<RefCell<FxHashMap<BlockId, Classes>>>,
}

impl<'a> Exec<'a> {
    fn new(
        enc: &'a Encoder,
        f: &'a Func,
        side: Side,
        shared: Rc<RefCell<FxHashMap<BlockId, Classes>>>,
    ) -> Exec<'a> {
        let inputs = f.params.iter().copied().chain([f.env]).collect();
        Exec { enc, f, side, live: Liveness::new(f), inputs, back: FxHashSet::default(), shared }
    }

    fn bits(&self, l: Local) -> u32 {
        smt::repr_bits(self.f.locals[l].repr)
    }

    /// A local's value where a region begins: an input shared by both sides
    /// or zero at the entry (code generation zeroes every local); at a cut,
    /// the representative of its class under the cut's invariant -- a
    /// constant, or the unknown of the class's first member.
    fn initial(&self, origin: Origin, l: Local) -> BV {
        let bits = self.bits(l);
        match origin {
            Origin::Start if self.inputs.contains(&l) => BV::new_const(format!("in.{}", l.index()), bits),
            Origin::Start => BV::from_u64(0, bits),
            Origin::Cut(h) => {
                let rep = self.shared.borrow().get(&h).and_then(|c| c.get(&(self.side, l)).copied());
                match rep {
                    Some(Rep::Const(c)) => BV::from_u64(c, bits),
                    Some(Rep::Var(side, r)) => {
                        BV::new_const(format!("h{}.{}.{}", h.index(), side.tag(), r.index()), bits)
                    }
                    None => BV::new_const(format!("h{}.{}.{}", h.index(), self.side.tag(), l.index()), bits),
                }
            }
        }
    }

    fn read(&self, origin: Origin, st: &State, l: Local) -> BV {
        st.vals.get(&l).cloned().unwrap_or_else(|| self.initial(origin, l))
    }

    fn operand(&self, origin: Origin, st: &State, o: &Operand) -> BV {
        match o {
            Operand::Local(l) => self.read(origin, st, *l),
            Operand::Const(Const::Int(v, r)) => BV::from_u64(*v as u64, smt::repr_bits(*r)),
            Operand::Const(Const::F32(x)) => BV::from_u64(u64::from(x.to_bits()), 32),
            Operand::Const(Const::F64(x)) => BV::from_u64(x.to_bits(), 64),
            Operand::Const(Const::Addr(a)) => BV::from_u64(*a, 64),
        }
    }

    /// The constant a local holds wherever it is read, when its single
    /// definition is a pure operation whose operands are such constants
    /// (to `depth` definitions deep) and the encoding simplifies it to a
    /// numeral that cannot report a defect: the facts a constant folding
    /// pass relies on, derived with the same semantics the proof uses.
    fn constant(&self, defs: &FxHashMap<Local, Rvalue>, l: Local, depth: u32) -> Option<u64> {
        let rv = defs.get(&l)?;
        let pure = matches!(
            rv,
            Rvalue::Use(_)
                | Rvalue::Bin { .. }
                | Rvalue::Neg { .. }
                | Rvalue::Not(_)
                | Rvalue::Cmp { .. }
                | Rvalue::Convert { .. }
                | Rvalue::Fits { .. }
                | Rvalue::Overflows { .. }
                | Rvalue::FromWord { .. }
                | Rvalue::ToWord(_)
        );
        if !pure || depth == 0 {
            return None;
        }
        let mut st = State { vals: FxHashMap::default(), world: self.enc.world_at(Origin::Start) };
        for o in rv.operands() {
            if let Operand::Local(x) = o {
                let c = self.constant(defs, x, depth - 1)?;
                st.vals.insert(x, BV::from_u64(c, self.bits(x)));
            }
        }
        let mut defects = Vec::new();
        let bits = self.bits(l);
        let v = smt::coerce(&self.rvalue(Origin::Start, rv, bits, &mut st, &mut defects), bits).simplify();
        let safe = defects.iter().all(|(c, _, _)| c.simplify().as_bool() == Some(false));
        if safe { v.as_u64() } else { None }
    }

    /// Apply an effect: the new world is a function of the old one and the
    /// arguments, and the result, when there is one, a function of the new
    /// world.
    fn effect(&self, key: &str, args: &[BV], st: &mut State, ret: Option<u32>) -> Option<BV> {
        let widths: Vec<String> = args.iter().map(|a| a.get_size().to_string()).collect();
        let name = format!("eff.{key}.{}", widths.join("."));
        let mut domain = vec![self.enc.world.clone()];
        domain.extend(args.iter().map(|a| Sort::bitvector(a.get_size())));
        let mut call: Vec<&dyn Ast> = vec![&st.world];
        call.extend(args.iter().map(|a| a as &dyn Ast));
        let world = self.enc.decl(&name, &domain, &self.enc.world).apply(&call);
        st.world = world;
        ret.map(|bits| {
            self.enc.value(&format!("res.{name}.{bits}"), &[&st.world], &[self.enc.world.clone()], bits)
        })
    }

    /// A pure read of the world: a function of the world and the arguments.
    fn read_world(&self, key: &str, args: &[BV], st: &State, bits: u32) -> BV {
        let widths: Vec<String> = args.iter().map(|a| a.get_size().to_string()).collect();
        let name = format!("rd.{key}.{}.{bits}", widths.join("."));
        let mut domain = vec![self.enc.world.clone()];
        domain.extend(args.iter().map(|a| Sort::bitvector(a.get_size())));
        let mut call: Vec<&dyn Ast> = vec![&st.world];
        call.extend(args.iter().map(|a| a as &dyn Ast));
        self.enc.value(&name, &call, &domain, bits)
    }

    /// An uninterpreted pure operation (a float operation, an address).
    fn pure_op(&self, key: &str, args: &[BV], bits: u32) -> BV {
        let widths: Vec<String> = args.iter().map(|a| a.get_size().to_string()).collect();
        let name = format!("op.{key}.{}.{bits}", widths.join("."));
        let domain: Vec<Sort> = args.iter().map(|a| Sort::bitvector(a.get_size())).collect();
        let call: Vec<&dyn Ast> = args.iter().map(|a| a as &dyn Ast).collect();
        self.enc.value(&name, &call, &domain, bits)
    }

    /// An uninterpreted pure predicate (a float comparison), as an 8-bit flag.
    fn pure_pred(&self, key: &str, args: &[BV]) -> BV {
        let widths: Vec<String> = args.iter().map(|a| a.get_size().to_string()).collect();
        let name = format!("pred.{key}.{}", widths.join("."));
        let domain: Vec<Sort> = args.iter().map(|a| Sort::bitvector(a.get_size())).collect();
        let domain: Vec<&Sort> = domain.iter().collect();
        let call: Vec<&dyn Ast> = args.iter().map(|a| a as &dyn Ast).collect();
        let decl = {
            let mut decls = self.enc.decls.borrow_mut();
            decls
                .entry(name.clone())
                .or_insert_with(|| Rc::new(FuncDecl::new(name.as_str(), &domain, &Sort::bool())))
                .clone()
        };
        smt::flag(&decl.apply(&call).as_bool().expect("boolean range"))
    }

    /// The value of an rvalue, with the defects it may report appended to
    /// `defects` as (condition, kind, location).
    fn rvalue(
        &self,
        origin: Origin,
        rv: &Rvalue,
        dest_bits: u32,
        st: &mut State,
        defects: &mut Vec<(Bool, u8, u64)>,
    ) -> BV {
        let op = |st: &State, o: &Operand| self.operand(origin, st, o);
        match rv {
            Rvalue::Use(a) => op(st, a),
            Rvalue::Bin { op: bin, a, b, ty, loc } => match ty {
                NumTy::I(bits, signed) => {
                    let w = u32::from(*bits);
                    let (x, y) = (smt::coerce(&op(st, a), w), smt::coerce(&op(st, b), w));
                    let (r, ds) = smt::int_bin(*bin, &x, &y, w, *signed, *loc != 0);
                    defects.extend(ds.into_iter().map(|(c, k)| (c, k, *loc)));
                    r
                }
                NumTy::F(bits) => {
                    let w = u32::from(*bits);
                    let (x, y) = (smt::coerce(&op(st, a), w), smt::coerce(&op(st, b), w));
                    self.pure_op(&format!("f{bin:?}"), &[x, y], w)
                }
            },
            Rvalue::Neg { a, ty, loc } => match ty {
                NumTy::I(bits, signed) => {
                    let w = u32::from(*bits);
                    let x = smt::coerce(&op(st, a), w);
                    let (r, ds) = smt::int_bin(BinOp::Sub, &BV::from_u64(0, w), &x, w, *signed, *loc != 0);
                    defects.extend(ds.into_iter().map(|(c, k)| (c, k, *loc)));
                    r
                }
                NumTy::F(bits) => self.pure_op("fneg", &[op(st, a)], u32::from(*bits)),
            },
            Rvalue::Not(a) => {
                let x = smt::coerce(&op(st, a), 8);
                smt::flag(&smt::same(&x, &BV::from_u64(0, 8)))
            }
            Rvalue::Cmp { op: cmp, a, b, ty } => match ty {
                NumTy::I(bits, signed) => {
                    let w = u32::from(*bits);
                    let (x, y) = (smt::coerce(&op(st, a), w), smt::coerce(&op(st, b), w));
                    smt::flag(&smt::int_cmp(*cmp, &x, &y, *signed))
                }
                NumTy::F(bits) => {
                    // `fcmp`'s ordered comparisons mirror (`a > b` is `b < a`,
                    // both false on a NaN), equality is symmetric, and `!=`
                    // is the negation of `==`: stated here, so a mirrored or
                    // swapped comparison means the same in the model
                    let w = u32::from(*bits);
                    let (x, y) = (smt::coerce(&op(st, a), w), smt::coerce(&op(st, b), w));
                    match cmp {
                        CmpOp::Lt | CmpOp::Le => self.pure_pred(&format!("f{cmp:?}"), &[x, y]),
                        CmpOp::Gt => self.pure_pred("fLt", &[y, x]),
                        CmpOp::Ge => self.pure_pred("fLe", &[y, x]),
                        CmpOp::Eq | CmpOp::Ne => {
                            // symmetric by construction, `q(x, y) & q(y, x)`,
                            // so it does not depend on the operands' order
                            let there = self.pure_pred("fEq", &[x.clone(), y.clone()]);
                            let eq = there.bvand(self.pure_pred("fEq", &[y, x]));
                            if *cmp == CmpOp::Eq {
                                eq
                            } else {
                                smt::flag(&smt::same(&eq, &BV::from_u64(0, 8)))
                            }
                        }
                    }
                }
            },
            Rvalue::Convert { a, from, to, checked, loc } => match (from, to) {
                (NumTy::I(fb, fs), NumTy::I(tb, ts)) => {
                    let (fw, tw) = (u32::from(*fb), u32::from(*tb));
                    let x = smt::coerce(&op(st, a), fw);
                    let (r, bad) = smt::int_convert(&x, fw, *fs, tw, *ts);
                    if *checked && (tb < fb || fs != ts) {
                        defects.push((bad, smt::DEFECT_CONVERSION, *loc));
                    }
                    r
                }
                (NumTy::F(fb), NumTy::F(tb)) if fb == tb => smt::coerce(&op(st, a), u32::from(*fb)),
                _ => {
                    let (fw, tw) = (num_bits(*from), num_bits(*to));
                    let x = smt::coerce(&op(st, a), fw);
                    self.pure_op(&format!("cvt.{}.{}", num_name(*from), num_name(*to)), &[x], tw)
                }
            },
            Rvalue::Fits { a, from, to } => match (from, to) {
                (NumTy::I(fb, fs), NumTy::I(tb, ts)) => {
                    let (fw, tw) = (u32::from(*fb), u32::from(*tb));
                    let x = smt::coerce(&op(st, a), fw);
                    let (_, bad) = smt::int_convert(&x, fw, *fs, tw, *ts);
                    smt::flag(&bad.not())
                }
                _ => BV::from_u64(1, 8),
            },
            Rvalue::Overflows { op: bin, a, b, ty } => match ty {
                NumTy::I(bits, signed) => {
                    let w = u32::from(*bits);
                    let (x, y) = (smt::coerce(&op(st, a), w), smt::coerce(&op(st, b), w));
                    smt::flag(&smt::int_overflows(*bin, &x, &y, w, *signed))
                }
                NumTy::F(_) => BV::from_u64(0, 8),
            },
            Rvalue::Load { base, offset, repr } => {
                let p = smt::coerce(&op(st, base), 64);
                self.read_world(&format!("load.{offset}.{repr:?}"), &[p], st, smt::repr_bits(*repr))
            }
            Rvalue::FromWord { a, to } => smt::coerce(&smt::coerce(&op(st, a), 64), smt::repr_bits(*to)),
            Rvalue::ToWord(a) => smt::coerce(&op(st, a), 64),
            Rvalue::Tag(a) => {
                let p = smt::coerce(&op(st, a), 64);
                self.read_world("tag", &[p], st, 32)
            }
            Rvalue::Alloc { desc, size } => {
                self.effect(&format!("alloc.{desc}.{size}"), &[], st, Some(64)).expect("a result")
            }
            Rvalue::Call { callee, args, ret } => {
                let mut values: Vec<BV> = args.iter().map(|a| op(st, a)).collect();
                let key = match callee {
                    Callee::Direct(id) => format!("call.d{}", id.index()),
                    Callee::Closure(c) => {
                        values.push(op(st, c));
                        "call.closure".to_string()
                    }
                    Callee::Lambda(id, c) => {
                        values.push(op(st, c));
                        format!("call.l{}", id.index())
                    }
                    Callee::Runtime(name) => format!("call.r.{name}"),
                    Callee::Intrinsic { name, targs, ret } => format!("call.i.{name}.{targs}.{ret}"),
                    Callee::Foreign { name, params, ret, contract } => {
                        format!("call.f.{name}.{contract}.{params:?}.{ret:?}")
                    }
                };
                let key = format!("{key}.{ret:?}");
                self.effect(&key, &values, st, Some(smt::repr_bits(*ret))).expect("a result")
            }
            Rvalue::Concat(parts) => {
                let values: Vec<BV> = parts.iter().map(|a| op(st, a)).collect();
                self.effect("concat", &values, st, Some(64)).expect("a result")
            }
            Rvalue::GlobalLoad(slot) => {
                let w = self.read_world(&format!("global.{slot}"), &[], st, 64);
                smt::coerce(&w, dest_bits)
            }
            Rvalue::GlobalAddr(slot) => self.pure_op(&format!("gaddr.{slot}"), &[], 64),
            Rvalue::FuncAddr(id) => self.pure_op(&format!("faddr.{}", id.index()), &[], 64),
            Rvalue::AdapterAddr(id) => self.pure_op(&format!("aaddr.{}", id.index()), &[], 64),
        }
    }

    /// Execute the region that begins at `start` (from `origin`'s values) up
    /// to its exits: returns, defects, `Unreachable`, the edges into a cut,
    /// and this side's other back edges. `None` when the region is not
    /// acyclic (a cycle avoids every cut).
    fn region(&self, start: BlockId, origin: Origin, cuts: &FxHashSet<BlockId>) -> Option<Vec<Exit>> {
        let n = self.f.blocks.len();
        let succ = |b: BlockId| -> Vec<BlockId> {
            self.f.blocks[b].term.as_ref().map(|t| t.successors()).unwrap_or_default()
        };
        // the region's blocks, and the number of region edges into each
        let mut inside = vec![false; n];
        let mut preds = vec![0usize; n];
        let mut todo = vec![start];
        inside[start.index()] = true;
        while let Some(b) = todo.pop() {
            for s in succ(b) {
                if s.index() >= n || cuts.contains(&s) || self.back.contains(&(b, s)) {
                    continue;
                }
                preds[s.index()] += 1;
                if !inside[s.index()] {
                    inside[s.index()] = true;
                    todo.push(s);
                }
            }
        }
        let mut incoming: FxHashMap<BlockId, Vec<(Bool, State)>> = FxHashMap::default();
        incoming.insert(
            start,
            vec![(
                Bool::from_bool(true),
                State { vals: FxHashMap::default(), world: self.enc.world_at(origin) },
            )],
        );
        let mut ready = vec![start];
        let mut done = 0;
        let mut exits = Vec::new();
        while let Some(b) = ready.pop() {
            done += 1;
            let arrivals = incoming.remove(&b).unwrap_or_default();
            let Some((mut guard, mut st)) = self.merge(origin, b, arrivals) else { continue };
            for s in &self.f.blocks[b].stmts {
                self.stmt(origin, s, &mut guard, &mut st, &mut exits);
            }
            let mut edges: Vec<(Bool, BlockId)> = Vec::new();
            match self.f.blocks[b].term.as_ref().unwrap_or(&Term::Unreachable) {
                Term::Goto(t) => edges.push((guard, *t)),
                Term::Branch(c, t, e) => {
                    let v = self.operand(origin, &st, c);
                    let zero = BV::from_u64(0, v.get_size());
                    let taken = smt::same(&v, &zero).not();
                    edges.push((Bool::and(&[guard.clone(), taken.clone()]), *t));
                    edges.push((Bool::and(&[guard, taken.not()]), *e));
                }
                Term::Switch(c, cases, default) => {
                    let v = self.operand(origin, &st, c);
                    let mut rest = guard;
                    for (k, t) in cases {
                        let hit = smt::same(&v, &BV::from_u64(*k as u64, v.get_size()));
                        edges.push((Bool::and(&[rest.clone(), hit.clone()]), *t));
                        rest = Bool::and(&[rest, hit.not()]);
                    }
                    edges.push((rest, *default));
                }
                Term::Return(o) => {
                    let v = smt::coerce(&self.operand(origin, &st, o), smt::repr_bits(self.f.ret));
                    exits.push(Exit { guard, kind: ExitKind::Return, state: st.clone(), value: Some(v) });
                }
                Term::Unreachable => {
                    exits.push(Exit { guard, kind: ExitKind::Unreachable, state: st.clone(), value: None });
                }
            }
            for (g, t) in edges {
                if t.index() >= n || cuts.contains(&t) || self.back.contains(&(b, t)) {
                    exits.push(Exit { guard: g, kind: ExitKind::Cut(t), state: st.clone(), value: None });
                    continue;
                }
                incoming.entry(t).or_default().push((g, st.clone()));
                preds[t.index()] -= 1;
                if preds[t.index()] == 0 {
                    ready.push(t);
                }
            }
        }
        let size = inside.iter().filter(|x| **x).count();
        (done == size).then_some(exits)
    }

    /// The state where paths join: the disjunction of their guards, and each
    /// live local and the world selected by the guard of the path it came
    /// from.
    fn merge(&self, origin: Origin, b: BlockId, mut arrivals: Vec<(Bool, State)>) -> Option<(Bool, State)> {
        if arrivals.len() <= 1 {
            return arrivals.pop();
        }
        let guards: Vec<Bool> = arrivals.iter().map(|(g, _)| g.clone()).collect();
        let guard = Bool::or(&guards);
        let pick = |values: Vec<Dynamic>| -> Dynamic {
            let mut it = values.into_iter().rev();
            let mut v = it.next().expect("a value per arrival");
            for (g, x) in guards.iter().rev().skip(1).zip(it) {
                v = g.ite(&x, &v);
            }
            v
        };
        let world = pick(arrivals.iter().map(|(_, s)| s.world.clone()).collect());
        let mut keys: Vec<Local> = arrivals
            .iter()
            .flat_map(|(_, s)| s.vals.keys().copied())
            .filter(|l| self.live.contains(b, *l))
            .collect();
        keys.sort_unstable();
        keys.dedup();
        let mut vals = FxHashMap::default();
        for l in keys {
            let values: Vec<BV> = arrivals.iter().map(|(_, s)| self.read(origin, s, l)).collect();
            let v = if values.iter().all(|v| *v == values[0]) {
                values[0].clone()
            } else {
                let mut it = values.into_iter().rev();
                let mut v = it.next().expect("a value per arrival");
                for (g, x) in guards.iter().rev().skip(1).zip(it) {
                    v = g.ite(&x, &v);
                }
                v
            };
            vals.insert(l, v);
        }
        arrivals.clear();
        Some((guard, State { vals, world }))
    }

    fn stmt(&self, origin: Origin, s: &Stmt, guard: &mut Bool, st: &mut State, exits: &mut Vec<Exit>) {
        let mut defects = Vec::new();
        match s {
            Stmt::Assign(l, rv) => {
                let bits = self.bits(*l);
                let v = self.rvalue(origin, rv, bits, st, &mut defects);
                // the defects are tested before the result is stored
                self.defects(guard, st, defects, exits);
                st.vals.insert(*l, smt::coerce(&v, bits));
            }
            Stmt::Store { base, offset, repr, value } => {
                let p = smt::coerce(&self.operand(origin, st, base), 64);
                let v = smt::coerce(&self.operand(origin, st, value), smt::repr_bits(*repr));
                self.effect(&format!("store.{offset}.{repr:?}"), &[p, v], st, None);
            }
            Stmt::GlobalStore { slot, value } => {
                let v = smt::coerce(&self.operand(origin, st, value), 64);
                self.effect(&format!("gstore.{slot}"), &[v], st, None);
            }
            Stmt::Poll => {
                self.effect("poll", &[], st, None);
            }
            Stmt::Loc { .. } => {}
        }
    }

    /// Split the path at each defect: the defect is an exit under its
    /// condition, and the path continues under its negation.
    fn defects(&self, guard: &mut Bool, st: &State, defects: Vec<(Bool, u8, u64)>, exits: &mut Vec<Exit>) {
        for (cond, kind, loc) in defects {
            let at = Bool::and(&[guard.clone(), cond.clone()]);
            let state = State { vals: FxHashMap::default(), world: st.world.clone() };
            exits.push(Exit { guard: at, kind: ExitKind::Defect(kind, loc), state, value: None });
            *guard = Bool::and(&[guard.clone(), cond.not()]);
        }
    }
}

fn num_bits(t: NumTy) -> u32 {
    match t {
        NumTy::I(b, _) | NumTy::F(b) => u32::from(b),
    }
}

fn num_name(t: NumTy) -> String {
    match t {
        NumTy::I(b, true) => format!("i{b}"),
        NumTy::I(b, false) => format!("u{b}"),
        NumTy::F(b) => format!("f{b}"),
    }
}

/// A fact relating the two sides at a cut.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Fact {
    /// The local holds the same value on both sides.
    Same(Local),
    /// The source's copy holds the value of the target's original: a copy
    /// the pass propagated.
    Copy(Local, Local),
    /// A local holds its single constant definition on one side (the
    /// constant's bits at the local's width).
    Const(Side, Local, u64),
    /// A local holds 0 or 1 on one side.
    Boolean(Side, Local),
}

/// A class representative: a constant, or the unknown of a (side, local).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Rep {
    Const(u64),
    Var(Side, Local),
}

/// The members of each equality class of an invariant and their
/// representatives; a member absent from the map is its own class.
type Classes = FxHashMap<(Side, Local), Rep>;

/// The classes of the equalities among `facts` (a union-find over the
/// sides' locals): each class's representative is its constant when it has
/// exactly one, and otherwise its least member. A class with two different
/// constants is a contradiction the facts still state; its members keep a
/// variable, so the contradiction is the solver's to see.
fn classes(facts: &[Fact]) -> Classes {
    let mut parent: FxHashMap<(Side, Local), (Side, Local)> = FxHashMap::default();
    fn find(parent: &mut FxHashMap<(Side, Local), (Side, Local)>, x: (Side, Local)) -> (Side, Local) {
        let mut root = x;
        while let Some(p) = parent.get(&root).copied().filter(|p| *p != root) {
            root = p;
        }
        let mut cur = x;
        while let Some(p) = parent.get(&cur).copied().filter(|p| *p != cur) {
            parent.insert(cur, root);
            cur = p;
        }
        root
    }
    let union = |parent: &mut FxHashMap<_, _>, a, b| {
        let (ra, rb) = (find(parent, a), find(parent, b));
        let (lo, hi) = if ra <= rb { (ra, rb) } else { (rb, ra) };
        parent.insert(lo, lo);
        parent.insert(hi, lo);
    };
    let mut constants: Vec<((Side, Local), u64)> = Vec::new();
    for fact in facts {
        match *fact {
            Fact::Same(l) => union(&mut parent, (Side::Src, l), (Side::Tgt, l)),
            Fact::Copy(y, x) => union(&mut parent, (Side::Src, y), (Side::Tgt, x)),
            Fact::Const(side, l, c) => {
                parent.entry((side, l)).or_insert((side, l));
                constants.push(((side, l), c));
            }
            Fact::Boolean(..) => {}
        }
    }
    let mut value: FxHashMap<(Side, Local), Option<u64>> = FxHashMap::default();
    for (member, c) in constants {
        let root = find(&mut parent, member);
        let slot = value.entry(root).or_insert(Some(c));
        if *slot != Some(c) {
            *slot = None;
        }
    }
    let members: Vec<(Side, Local)> = parent.keys().copied().collect();
    members
        .into_iter()
        .map(|m| {
            let root = find(&mut parent, m);
            let rep = match value.get(&root) {
                Some(Some(c)) => Rep::Const(*c),
                _ => Rep::Var(root.0, root.1),
            };
            (m, rep)
        })
        .collect()
}

/// One region on both sides.
struct RegionPair {
    origin: Origin,
    src: Vec<Exit>,
    tgt: Vec<Exit>,
}

struct Validation<'a> {
    enc: &'a Encoder,
    src: Exec<'a>,
    tgt: Exec<'a>,
    cuts: FxHashSet<BlockId>,
    facts: FxHashMap<BlockId, Vec<Fact>>,
    regions: Vec<RegionPair>,
    /// Per region and the cut it reaches: the facts it was proved to
    /// re-establish there, and the invariant at its origin it was proved
    /// under. A later check with the same origin invariant and a subset of
    /// those facts is already decided.
    established: FxHashMap<(usize, BlockId), (Vec<Fact>, FxHashSet<Fact>)>,
    deadline: Instant,
}

/// The encoder outlives every validation; one per thread, since Z3's
/// context is thread-local.
fn encoder() -> &'static Encoder {
    thread_local! {
        static ENCODER: &'static Encoder = Box::leak(Box::new(Encoder::new()));
    }
    ENCODER.with(|e| *e)
}

impl<'a> Validation<'a> {
    fn new(src: &'a Func, tgt: &'a Func, deadline: Instant) -> Result<Validation<'a>, Verdict> {
        let enc = encoder();
        // a header of both sides is a cut; a header of one side only is cut
        // at that side's back edges into it
        let (back_s, back_t) = (back_edges(src), back_edges(tgt));
        let headers = |back: &FxHashSet<(BlockId, BlockId)>| -> FxHashSet<BlockId> {
            back.iter().map(|(_, h)| *h).collect()
        };
        let (hs, ht) = (headers(&back_s), headers(&back_t));
        let cuts: FxHashSet<BlockId> = hs.intersection(&ht).copied().collect();
        let shared = Rc::new(RefCell::new(FxHashMap::default()));
        let mut src = Exec::new(enc, src, Side::Src, shared.clone());
        let mut tgt = Exec::new(enc, tgt, Side::Tgt, shared);
        src.back = back_s.into_iter().filter(|(_, h)| !cuts.contains(h)).collect();
        tgt.back = back_t.into_iter().filter(|(_, h)| !cuts.contains(h)).collect();
        let analyses = [Analysis::new(src.f), Analysis::new(tgt.f)];
        let facts = cuts.iter().map(|h| (*h, candidates(&src, &tgt, &analyses, *h))).collect();
        Ok(Validation {
            enc,
            src,
            tgt,
            cuts,
            facts,
            regions: vec![],
            established: FxHashMap::default(),
            deadline,
        })
    }

    /// Search the invariant, then prove every region under it. The regions
    /// are executed with the equalities of the invariant as shared
    /// variables, so whenever the search drops one they are executed again,
    /// until the search ends on the equalities they were executed with.
    fn run(&mut self) -> Result<(), Verdict> {
        let _ = self.enc;
        *self.src.shared.borrow_mut() = self.equalities();
        self.explore()?;
        loop {
            self.houdini()?;
            let equal = self.equalities();
            let changed: Vec<usize> = (0..self.regions.len())
                .filter(|i| match self.regions[*i].origin {
                    Origin::Start => false,
                    Origin::Cut(h) => self.src.shared.borrow().get(&h) != equal.get(&h),
                })
                .collect();
            if changed.is_empty() {
                break;
            }
            *self.src.shared.borrow_mut() = equal;
            for i in changed {
                let origin = self.regions[i].origin;
                self.regions[i] = self.execute(origin)?;
                self.established.retain(|(region, _), _| *region != i);
            }
        }
        for i in 0..self.regions.len() {
            self.prove(i)?;
        }
        Ok(())
    }

    /// Execute one region on both sides.
    fn execute(&self, origin: Origin) -> Result<RegionPair, Verdict> {
        if Instant::now() > self.deadline {
            return Err(Verdict::NotProven("the function's budget ran out".into()));
        }
        let start = match origin {
            Origin::Start => BlockId::new(0),
            Origin::Cut(h) => h,
        };
        let not_acyclic = || Verdict::NotProven("a region is not acyclic".into());
        let src = self.src.region(start, origin, &self.cuts).ok_or_else(not_acyclic)?;
        let tgt = self.tgt.region(start, origin, &self.cuts).ok_or_else(not_acyclic)?;
        Ok(RegionPair { origin, src, tgt })
    }

    /// The locals each cut's invariant currently holds equal on both sides.
    fn equalities(&self) -> FxHashMap<BlockId, Classes> {
        self.facts.iter().map(|(h, facts)| (*h, classes(facts))).collect()
    }

    /// Execute every region the two sides reach together, from the entry.
    fn explore(&mut self) -> Result<(), Verdict> {
        let mut todo = vec![Origin::Start];
        let mut seen: FxHashSet<Origin> = FxHashSet::default();
        seen.insert(Origin::Start);
        while let Some(origin) = todo.pop() {
            let RegionPair { src, tgt, .. } = self.execute(origin)?;
            let reached = |exits: &[Exit]| -> FxHashSet<BlockId> {
                exits
                    .iter()
                    .filter_map(|e| match e.kind {
                        ExitKind::Cut(h) => Some(h),
                        _ => None,
                    })
                    .collect()
            };
            let (a, b) = (reached(&src), reached(&tgt));
            for h in a.intersection(&b) {
                if seen.insert(Origin::Cut(*h)) {
                    todo.push(Origin::Cut(*h));
                }
            }
            self.regions.push(RegionPair { origin, src, tgt });
        }
        Ok(())
    }

    /// A fact's statement over one side's and the other's values.
    fn fact(&self, fact: &Fact, s: &dyn Fn(Local) -> BV, t: &dyn Fn(Local) -> BV) -> Bool {
        let side = |sd: Side, l: Local| if sd == Side::Src { s(l) } else { t(l) };
        match *fact {
            Fact::Same(l) => smt::same(&s(l), &t(l)),
            Fact::Copy(y, x) => smt::same(&s(y), &t(x)),
            Fact::Const(sd, l, c) => {
                let v = side(sd, l);
                smt::same(&v, &BV::from_u64(c, v.get_size()))
            }
            Fact::Boolean(sd, l) => {
                let v = side(sd, l);
                v.bvule(BV::from_u64(1, v.get_size()))
            }
        }
    }

    /// The invariant assumed where a region begins.
    fn assumption(&self, origin: Origin) -> Bool {
        let Origin::Cut(h) = origin else { return Bool::from_bool(true) };
        let s = |l| self.src.initial(origin, l);
        let t = |l| self.tgt.initial(origin, l);
        let facts: Vec<Bool> = self.facts[&h].iter().map(|f| self.fact(f, &s, &t)).collect();
        Bool::and(&facts)
    }

    /// The guard under which a side leaves a region by `kind`, and the
    /// selected state and value.
    fn select<'e>(exits: &'e [Exit], kind: ExitKind) -> (Bool, Vec<&'e Exit>) {
        let chosen: Vec<&Exit> = exits.iter().filter(|e| e.kind == kind).collect();
        let guard = Bool::or(&chosen.iter().map(|e| e.guard.clone()).collect::<Vec<_>>());
        (guard, chosen)
    }

    /// One exit kind's value on one side: the value of the path that took it.
    fn chosen<T: Ast + Clone>(chosen: &[&Exit], get: impl Fn(&Exit) -> T) -> Option<T> {
        let mut it = chosen.iter().rev();
        let mut v = get(it.next()?);
        for e in it {
            v = e.guard.ite(&get(e), &v);
        }
        Some(v)
    }

    /// Decide one query on a fresh solver: Z3 preprocesses a query it sees
    /// whole (equalities solved, then bit-blasted) far better than one
    /// pushed onto an incremental solver. A satisfiable answer comes with
    /// its model.
    fn check(&self, query: &Bool) -> Result<Option<z3::Model>, Verdict> {
        let left = self.deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(Verdict::NotProven("the function's budget ran out".into()));
        }
        let solver = Solver::new();
        let mut params = Params::new();
        params.set_u32("timeout", left.min(QUERY_TIMEOUT).as_millis().max(1) as u32);
        solver.set_params(&params);
        solver.assert(query);
        match solver.check() {
            SatResult::Unsat => Ok(None),
            SatResult::Sat => solver
                .get_model()
                .map(Some)
                .ok_or_else(|| Verdict::NotProven("the solver gave no model".into())),
            SatResult::Unknown => Err(Verdict::NotProven("a query did not finish within its budget".into())),
        }
    }

    /// Remove every fact some region does not re-establish at the cut it
    /// reaches, until the remaining facts are inductive.
    fn houdini(&mut self) -> Result<(), Verdict> {
        loop {
            let mut dropped = false;
            for i in 0..self.regions.len() {
                let region = &self.regions[i];
                let assume = self.assumption(region.origin);
                let mut cuts: Vec<BlockId> = region
                    .src
                    .iter()
                    .filter_map(|e| match e.kind {
                        ExitKind::Cut(h) => Some(h),
                        _ => None,
                    })
                    .collect();
                cuts.sort_unstable();
                cuts.dedup();
                let assumed = match region.origin {
                    Origin::Start => vec![],
                    Origin::Cut(o) => self.facts[&o].clone(),
                };
                for h in cuts {
                    let kind = ExitKind::Cut(h);
                    let (gs, es) = Self::select(&region.src, kind);
                    let (gt, et) = Self::select(&region.tgt, kind);
                    if et.is_empty() {
                        continue;
                    }
                    if let Some((under, proven)) = self.established.get(&(i, h))
                        && *under == assumed
                        && self.facts[&h].iter().all(|f| proven.contains(f))
                    {
                        continue;
                    }
                    loop {
                        let facts = self.facts[&h].clone();
                        if facts.is_empty() {
                            break;
                        }
                        let stated: Vec<Bool> = {
                            let s = self.cut_values(&self.src, region.origin, &es);
                            let t = self.cut_values(&self.tgt, region.origin, &et);
                            facts.iter().map(|f| self.fact(f, &s, &t)).collect()
                        };
                        let query =
                            Bool::and(&[assume.clone(), gs.clone(), gt.clone(), Bool::and(&stated).not()]);
                        match self.check(&query)? {
                            None => {
                                self.established
                                    .insert((i, h), (assumed.clone(), facts.into_iter().collect()));
                                break;
                            }
                            Some(model) => {
                                let keep: Vec<Fact> = facts
                                    .iter()
                                    .zip(&stated)
                                    .filter(|(_, b)| {
                                        model.eval(*b, true).and_then(|v| v.as_bool()) == Some(true)
                                    })
                                    .map(|(f, _)| *f)
                                    .collect();
                                if keep.len() == facts.len() {
                                    return Err(Verdict::NotProven("the invariant search stalled".into()));
                                }
                                self.facts.insert(h, keep);
                                dropped = true;
                            }
                        }
                    }
                }
            }
            if !dropped {
                return Ok(());
            }
        }
    }

    /// A side's values at a cut exit: each local selected by the guard of
    /// the path that took it.
    fn cut_values<'e>(
        &'e self,
        exec: &'e Exec<'a>,
        origin: Origin,
        chosen: &'e [&'e Exit],
    ) -> impl Fn(Local) -> BV + 'e {
        move |l: Local| Self::chosen(chosen, |e| exec.read(origin, &e.state, l)).expect("a chosen exit")
    }

    /// The claim for one region: the two sides take the same exit, and at
    /// it the worlds, the returned values, and the cut's facts agree. A
    /// refutation from the entry is a difference in the model; one from a
    /// cut may start from a state no run reaches, so it is only a failure
    /// to prove.
    fn prove(&self, i: usize) -> Result<(), Verdict> {
        let region = &self.regions[i];
        let assume = self.assumption(region.origin);
        let mut kinds: Vec<ExitKind> = region.src.iter().chain(&region.tgt).map(|e| e.kind).collect();
        // in the order the exits occur, never by a location's address: the
        // order the terms are built in is the order the solver sees them in
        let mut seen = FxHashSet::default();
        kinds.retain(|k| seen.insert(*k));
        // each claim, why it fails, and whether the failure is observable:
        // reaching a cut is not, so a claim about one is only a failure to
        // prove
        let mut claims: Vec<(String, bool, Bool)> = Vec::new();
        let mut returned: Option<(BV, BV)> = None;
        for kind in kinds {
            let observable = !matches!(kind, ExitKind::Cut(_));
            let (gs, es) = Self::select(&region.src, kind);
            let (gt, et) = Self::select(&region.tgt, kind);
            claims.push((format!("only one side reaches {}", exit_name(kind)), observable, gs.iff(&gt)));
            if es.is_empty() || et.is_empty() {
                continue;
            }
            let both = Bool::and(&[gs, gt]);
            let ws = Self::chosen(&es, |e| e.state.world.clone()).expect("a chosen exit");
            let wt = Self::chosen(&et, |e| e.state.world.clone()).expect("a chosen exit");
            let worlds = <Dynamic as Ast>::eq(&ws, wt);
            claims.push((
                format!("the effects differ before {}", exit_name(kind)),
                observable,
                both.implies(&worlds),
            ));
            if kind == ExitKind::Return {
                let vs =
                    Self::chosen(&es, |e| e.value.clone().expect("a returned value")).expect("a chosen exit");
                let vt =
                    Self::chosen(&et, |e| e.value.clone().expect("a returned value")).expect("a chosen exit");
                returned = Some((vs.clone(), vt.clone()));
                claims.push(("the returned values differ".into(), true, both.implies(smt::same(&vs, &vt))));
            }
            if let ExitKind::Cut(h) = kind {
                let s_vals = self.cut_values(&self.src, region.origin, &es);
                let t_vals = self.cut_values(&self.tgt, region.origin, &et);
                for f in &self.facts[&h] {
                    let fact = self.fact(f, &s_vals, &t_vals);
                    claims.push((format!("{f:?} fails at {}", exit_name(kind)), false, both.implies(&fact)));
                }
            }
        }
        let all: Vec<Bool> = claims.iter().map(|(_, _, c)| c.clone()).collect();
        let query = Bool::and(&[assume, Bool::and(&all).not()]);
        let Some(model) = self.check(&query)? else { return Ok(()) };
        let (failed, observable) = claims
            .iter()
            .find(|(_, _, c)| model.eval(c, true).and_then(|v| v.as_bool()) == Some(false))
            .map(|(why, observable, _)| (why.clone(), *observable))
            .unwrap_or_else(|| ("the claim fails".into(), false));
        let value = |v: &BV| model.eval(v, true).and_then(|v| v.as_u64());
        let failed = match &returned {
            Some((vs, vt)) if failed == "the returned values differ" => match (value(vs), value(vt)) {
                (Some(a), Some(b)) => format!("the source returns {a}, the target {b}"),
                _ => failed,
            },
            _ => failed,
        };
        match region.origin {
            Origin::Start if observable => Err(Verdict::Refuted(format!("{failed}{}", self.inputs(&model)))),
            Origin::Start => Err(Verdict::NotProven(format!("the loops do not correspond: {failed}"))),
            Origin::Cut(h) => Err(Verdict::NotProven(format!(
                "the invariant at the loop header bb{} does not prove it: {failed}",
                h.index()
            ))),
        }
    }

    /// The parameters of a refuting assignment.
    fn inputs(&self, model: &z3::Model) -> String {
        let mut parts = vec![];
        for p in &self.src.f.params {
            let v = self.src.initial(Origin::Start, *p);
            if let Some(x) = model.eval(&v, true).and_then(|v| v.as_u64()) {
                let name = self.src.f.locals[*p].name.clone().unwrap_or_else(|| format!("_{}", p.index()));
                parts.push(format!("{name} = {x}"));
            }
        }
        if parts.is_empty() { String::new() } else { format!(" when {}", parts.join(", ")) }
    }
}

fn exit_name(kind: ExitKind) -> String {
    match kind {
        ExitKind::Return => "the return".into(),
        ExitKind::Defect(k, loc) => format!("the defect {k} at {loc:#x}"),
        ExitKind::Unreachable => "`Unreachable`".into(),
        ExitKind::Cut(h) => format!("bb{}", h.index()),
    }
}

/// The candidate facts at a cut: every local both sides read equal on both
/// sides, every
/// source copy chain (through copies and identity conversions) equal to its
/// original in the target, every
/// single-definition constant, and every boolean's domain.
fn candidates(src: &Exec, tgt: &Exec, analyses: &[Analysis; 2], h: BlockId) -> Vec<Fact> {
    let mut facts: Vec<Fact> = Vec::new();
    // a local only one side reads is related by a copy or a constant, if at
    // all: the pass replaced its uses
    facts.extend(src.live.locals(h).filter(|l| tgt.live.contains(h, *l)).map(Fact::Same));
    for y in src.live.locals(h) {
        let copies = chain(src.f, &analyses[0].defs, y);
        facts.extend(copies.into_iter().filter(|x| tgt.live.contains(h, *x)).map(|x| Fact::Copy(y, x)));
    }
    for ((exec, side), Analysis { defs, bools, constants }) in
        [(src, Side::Src), (tgt, Side::Tgt)].into_iter().zip(analyses)
    {
        for l in exec.live.locals(h) {
            let c = *constants.borrow_mut().entry(l).or_insert_with(|| exec.constant(defs, l, 8));
            if let Some(c) = c {
                facts.push(Fact::Const(side, l, c));
            }
            if bools.contains(&l) {
                facts.push(Fact::Boolean(side, l));
            }
        }
    }
    // ordered by the locals, never by a constant (an address differs from
    // run to run), so the terms are built in the same order every run
    facts.sort_unstable_by_key(|f| match *f {
        Fact::Same(l) => (0, Side::Src, l, l, 0),
        Fact::Copy(y, x) => (1, Side::Src, y, x, 0),
        Fact::Const(side, l, c) => (2, side, l, l, c),
        Fact::Boolean(side, l) => (3, side, l, l, 0),
    });
    facts.dedup();
    facts
}

/// The single-definition chain from `y` through copies and conversions to
/// the type converted from, all of `y`'s representation: the locals it
/// passes, the originals `y` copies.
fn chain(f: &Func, defs: &FxHashMap<Local, Rvalue>, y: Local) -> Vec<Local> {
    let mut locals = Vec::new();
    let mut cur = y;
    for _ in 0..8 {
        let x = match defs.get(&cur) {
            Some(Rvalue::Use(Operand::Local(x))) => *x,
            Some(Rvalue::Convert { a: Operand::Local(x), from, to, .. }) if from == to => *x,
            _ => break,
        };
        if x == y || f.locals[x].repr != f.locals[y].repr {
            break;
        }
        locals.push(x);
        cur = x;
    }
    locals
}

/// What the candidate facts are drawn from, per side: the single
/// definitions, the boolean locals, and the constants found so far.
struct Analysis {
    defs: FxHashMap<Local, Rvalue>,
    bools: FxHashSet<Local>,
    constants: RefCell<FxHashMap<Local, Option<u64>>>,
}

impl Analysis {
    fn new(f: &Func) -> Analysis {
        Analysis {
            defs: single_definitions(f),
            bools: super::simplify::bool_locals(f),
            constants: RefCell::new(FxHashMap::default()),
        }
    }
}

/// The rvalue of every local assigned exactly once (not a parameter) in
/// the blocks reachable from the entry: an assignment no run reaches does
/// not count.
fn single_definitions(f: &Func) -> FxHashMap<Local, Rvalue> {
    let reachable = super::cfg::reachable(f);
    let mut defs: FxHashMap<Local, Rvalue> = FxHashMap::default();
    let mut multi: FxHashSet<Local> = FxHashSet::default();
    for (b, block) in f.blocks.iter_enumerated() {
        if !reachable[b.index()] {
            continue;
        }
        for s in &block.stmts {
            if let Stmt::Assign(l, rv) = s
                && defs.insert(*l, rv.clone()).is_some()
            {
                multi.insert(*l);
            }
        }
    }
    for l in multi.iter().chain(&f.params).chain([&f.env]) {
        defs.remove(l);
    }
    defs
}

/// Which passes the check validates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// No check (the default).
    Off,
    /// `BADGER_MIR_TV=1`: the passes in reach of the model; a function
    /// another pass changed is counted, not validated, a refutation fails
    /// the compilation, and the summary is printed when a function is not
    /// proven.
    InReach,
    /// `BADGER_MIR_TV=all`: every pass, for the survey of what the model
    /// decides; nothing fails the compilation, and the summary is always
    /// printed.
    All,
}

impl Mode {
    /// The mode `BADGER_MIR_TV` selects. Read once.
    pub fn get() -> Mode {
        static MODE: std::sync::LazyLock<Mode> =
            std::sync::LazyLock::new(|| match std::env::var("BADGER_MIR_TV").as_deref() {
                Ok("1") => Mode::InReach,
                Ok("all") => Mode::All,
                _ => Mode::Off,
            });
        *MODE
    }

    /// Whether the check validates the functions `pass` changes.
    pub fn validates(self, pass: &str) -> bool {
        match self {
            Mode::Off => false,
            Mode::InReach => in_reach(pass),
            Mode::All => true,
        }
    }
}

/// The passes the model covers: they rewrite the operations it states
/// exactly and keep each function's signature, locals, and loop structure
/// (the simplification and common-subexpression passes, and the
/// superoptimiser). The others change what the model leaves uninterpreted
/// -- calls (inlining, specialisation), objects (scalar replacement, enum
/// and niche layout), runtime entry points (the map and receive passes) --
/// or the function table (reachability).
pub fn in_reach(pass: &str) -> bool {
    let base = pass.trim_end_matches(|c: char| c.is_ascii_digit());
    matches!(base, "simplify" | "cse" | "superopt")
}

/// One validated function: the pass, the function, the verdict, how long
/// the validator took, and the function's statement count before the pass.
pub struct Record {
    pub pass: &'static str,
    pub function: String,
    pub verdict: Verdict,
    pub elapsed: Duration,
    pub statements: usize,
}

/// Check every function `pass` changed between `before` and `after`: validate
/// it when `validate` is set, and otherwise report it as outside the model. A
/// function the pass appended to the table, and every function of a table
/// the pass renumbered, is reported as not proven: its meaning is the call
/// sites', which a per-function check does not relate.
pub fn check_pass(pass: &'static str, before: &Program, after: &Program, validate: bool) -> Vec<Record> {
    let kept = after.funcs.len() >= before.funcs.len()
        && before.funcs.iter().zip(&after.funcs).all(|(a, b)| a.name == b.name);
    let mut out = Vec::new();
    for (id, f) in after.funcs.iter_enumerated() {
        let old = before.funcs.get(id);
        if kept && old.is_some_and(|old| same_func(old, f)) {
            continue;
        }
        let statements = old.map_or(0, |old| old.blocks.iter().map(|b| b.stmts.len()).sum());
        let start = Instant::now();
        let verdict = match old {
            Some(old) if old.name == f.name && same_func(old, f) => continue,
            _ if !validate => Verdict::NotProven(OUTSIDE.into()),
            Some(old) if kept => self::validate(old, f),
            Some(_) => Verdict::NotProven("the pass renumbered the function table".into()),
            None if kept => Verdict::NotProven("the pass added the function".into()),
            None => Verdict::NotProven("the pass renumbered the function table".into()),
        };
        out.push(Record { pass, function: f.name.clone(), verdict, elapsed: start.elapsed(), statements });
    }
    out
}

/// The reason given for a function a pass outside the model changed.
const OUTSIDE: &str = "the pass is outside the model";

/// Print a summary per pass to stderr, and append one line per record to
/// the file `BADGER_MIR_TV_LOG` names (tab-separated: pass, function,
/// verdict, microseconds, statements, reason).
pub fn report(records: &[Record], mir_bytes: usize, elapsed: Duration) {
    let mut passes: Vec<&'static str> = records.iter().map(|r| r.pass).collect();
    passes.dedup();
    let mut seen = FxHashSet::default();
    passes.retain(|p| seen.insert(*p));
    let outside = |r: &Record| matches!(&r.verdict, Verdict::NotProven(why) if why == OUTSIDE);
    eprintln!(
        "BADGER_MIR_TV: {} changed functions, {} validated, over {} KB of MIR in {:.1} ms",
        records.len(),
        records.iter().filter(|r| !outside(r)).count(),
        mir_bytes / 1024,
        elapsed.as_secs_f64() * 1e3,
    );
    for pass in passes {
        let of: Vec<&Record> = records.iter().filter(|r| r.pass == pass).collect();
        let count = |w: &str| of.iter().filter(|r| r.verdict.word() == w).count();
        let ms: f64 = of.iter().map(|r| r.elapsed.as_secs_f64() * 1e3).sum();
        if of.iter().all(|r| outside(r)) {
            eprintln!("  {pass}: {} changed, outside the model", of.len());
            continue;
        }
        eprintln!(
            "  {pass}: {} changed, {} equivalent, {} refuted, {} not proven, {ms:.1} ms",
            of.len(),
            count("equivalent"),
            count("refuted"),
            count("not proven"),
        );
        for r in of.iter().filter(|r| matches!(r.verdict, Verdict::Refuted(_))).take(3) {
            if let Verdict::Refuted(why) = &r.verdict {
                eprintln!("      refuted {}: {why}", r.function);
            }
        }
    }
    if let Some(path) = std::env::var_os("BADGER_MIR_TV_LOG") {
        use std::io::Write;
        let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) else { return };
        for r in records {
            let reason = match &r.verdict {
                Verdict::Equivalent => String::new(),
                Verdict::Refuted(s) | Verdict::NotProven(s) => s.replace(['\t', '\n'], " "),
            };
            let _ = writeln!(
                file,
                "{}\t{}\t{}\t{}\t{}\t{}",
                r.pass,
                r.function.replace(['\t', '\n'], " "),
                r.verdict.word(),
                r.elapsed.as_micros(),
                r.statements,
                reason,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int(v: i64) -> Operand {
        Operand::Const(Const::Int(v, Repr::I64))
    }

    fn local(n: usize) -> Operand {
        Operand::Local(Local::new(n))
    }

    fn add(a: Operand, b: Operand, loc: u64) -> Rvalue {
        Rvalue::Bin { op: BinOp::Add, a, b, ty: NumTy::I(64, true), loc }
    }

    /// A function of one `i64` parameter (local 1) with `extra` more `i64`
    /// locals and the given blocks.
    fn func(extra: usize, blocks: Vec<(Vec<Stmt>, Term)>) -> Func {
        let mut f = Func::new("f".into(), FuncKind::Normal, Repr::I64);
        for _ in 0..=extra {
            f.locals.push(LocalDecl { repr: Repr::I64, name: None });
        }
        f.params = vec![Local::new(1)];
        f.blocks.clear();
        for (stmts, term) in blocks {
            f.blocks.push(Block { stmts, term: Some(term) });
        }
        f
    }

    fn assign(l: usize, rv: Rvalue) -> Stmt {
        Stmt::Assign(Local::new(l), rv)
    }

    /// A call the model leaves uninterpreted: an effect on the world.
    fn call(name: &'static str, args: Vec<Operand>) -> Rvalue {
        Rvalue::Call { callee: Callee::Runtime(name), args, ret: Repr::I64 }
    }

    #[test]
    fn a_folded_constant_is_equivalent() {
        let src = func(1, vec![(vec![assign(2, add(int(3), int(4), 0))], Term::Return(local(2)))]);
        let tgt = func(1, vec![(vec![], Term::Return(int(7)))]);
        assert_eq!(validate(&src, &tgt), Verdict::Equivalent);
    }

    #[test]
    fn a_wrong_fold_is_refuted_with_the_two_values() {
        let src = func(1, vec![(vec![assign(2, add(local(1), int(4), 0))], Term::Return(local(2)))]);
        let tgt = func(1, vec![(vec![assign(2, add(local(1), int(5), 0))], Term::Return(local(2)))]);
        let Verdict::Refuted(why) = validate(&src, &tgt) else { panic!("not refuted") };
        assert!(why.contains("the source returns") && why.contains("the target"), "{why}");
    }

    /// Dropping an overflow check removes a defect the source reports.
    #[test]
    fn a_dropped_overflow_check_is_refuted() {
        let src = func(1, vec![(vec![assign(2, add(local(1), int(1), 7))], Term::Return(local(2)))]);
        let tgt = func(1, vec![(vec![assign(2, add(local(1), int(1), 0))], Term::Return(local(2)))]);
        let Verdict::Refuted(why) = validate(&src, &tgt) else { panic!("not refuted") };
        assert!(why.contains("defect"), "{why}");
    }

    /// Two effects in the other order are two different worlds.
    #[test]
    fn reordered_effects_are_refuted() {
        let effects = |a, b| vec![assign(2, call(a, vec![local(1)])), assign(3, call(b, vec![local(1)]))];
        let src = func(2, vec![(effects("first", "second"), Term::Return(int(0)))]);
        let tgt = func(2, vec![(effects("second", "first"), Term::Return(int(0)))]);
        let Verdict::Refuted(why) = validate(&src, &tgt) else { panic!("not refuted") };
        assert!(why.contains("effects"), "{why}");
    }

    /// A copy propagated into a loop and a constant folded into its
    /// condition: the proof needs the invariant at the header (the copy
    /// equals its original, the bound is its constant).
    #[test]
    fn a_loop_after_propagation_is_proven() {
        // src: n = 10; c = p; i = 0; loop { if i < n { i = i + c } else { return i } }
        let src = func(
            5,
            vec![
                (
                    vec![
                        assign(2, Rvalue::Use(int(10))),
                        assign(3, Rvalue::Use(local(1))),
                        assign(4, Rvalue::Use(int(0))),
                    ],
                    Term::Goto(BlockId::new(1)),
                ),
                (
                    vec![assign(
                        5,
                        Rvalue::Cmp { op: CmpOp::Lt, a: local(4), b: local(2), ty: NumTy::I(64, true) },
                    )],
                    Term::Branch(local(5), BlockId::new(2), BlockId::new(3)),
                ),
                (vec![assign(4, add(local(4), local(3), 0))], Term::Goto(BlockId::new(1))),
                (vec![], Term::Return(local(4))),
            ],
        );
        // tgt: the copy and the bound propagated, their definitions removed
        let tgt = func(
            5,
            vec![
                (vec![assign(4, Rvalue::Use(int(0)))], Term::Goto(BlockId::new(1))),
                (
                    vec![assign(
                        5,
                        Rvalue::Cmp { op: CmpOp::Lt, a: local(4), b: int(10), ty: NumTy::I(64, true) },
                    )],
                    Term::Branch(local(5), BlockId::new(2), BlockId::new(3)),
                ),
                (vec![assign(4, add(local(4), local(1), 0))], Term::Goto(BlockId::new(1))),
                (vec![], Term::Return(local(4))),
            ],
        );
        assert_eq!(validate(&src, &tgt), Verdict::Equivalent);
        // and the same loop adding one more each iteration is not proven
        let mut wrong = tgt.clone();
        wrong.blocks[BlockId::new(2)].stmts =
            vec![assign(4, add(local(4), local(1), 0)), assign(4, add(local(4), int(1), 0))];
        assert!(matches!(validate(&src, &wrong), Verdict::NotProven(_)), "{:?}", validate(&src, &wrong));
    }

    /// The validator relates locals by identity: a function whose locals
    /// changed is outside what it decides, and is never called equivalent.
    #[test]
    fn a_changed_function_shape_is_not_proven() {
        let src = func(1, vec![(vec![], Term::Return(local(1)))]);
        let tgt = func(2, vec![(vec![], Term::Return(local(1)))]);
        assert!(matches!(validate(&src, &tgt), Verdict::NotProven(_)));
    }

    /// IEEE equality is symmetric and the model states it so, whatever the
    /// order the operands reach the solver in; float multiplication is
    /// left uninterpreted, so swapping its operands -- which changes the
    /// NaN payload of two NaNs -- is a difference.
    #[test]
    fn swapped_float_equality_is_proven_and_a_swapped_float_product_is_not() {
        let cmp = |a, b| Rvalue::Cmp { op: CmpOp::Eq, a, b, ty: NumTy::F(64) };
        let src = func(
            2,
            vec![(vec![assign(2, cmp(local(1), Operand::Const(Const::F64(2.5))))], Term::Return(local(2)))],
        );
        let tgt = func(
            2,
            vec![(vec![assign(2, cmp(Operand::Const(Const::F64(2.5)), local(1)))], Term::Return(local(2)))],
        );
        assert_eq!(validate(&src, &tgt), Verdict::Equivalent);
        let mul = |a, b| Rvalue::Bin { op: BinOp::Mul, a, b, ty: NumTy::F(64), loc: 0 };
        let src = func(
            2,
            vec![(vec![assign(2, mul(local(1), Operand::Const(Const::F64(3.0))))], Term::Return(local(2)))],
        );
        let tgt = func(
            2,
            vec![(vec![assign(2, mul(Operand::Const(Const::F64(3.0)), local(1)))], Term::Return(local(2)))],
        );
        assert!(matches!(validate(&src, &tgt), Verdict::Refuted(_)));
    }

    #[test]
    fn the_passes_in_reach_are_the_ones_the_model_covers() {
        for pass in ["simplify", "simplify2", "simplify4", "cse", "cse3", "superopt"] {
            assert!(in_reach(pass), "{pass}");
        }
        for pass in
            ["inline", "inline2", "specialize", "sra", "niche", "enums", "split", "maps-private", "reach"]
        {
            assert!(!in_reach(pass), "{pass}");
        }
    }
}
