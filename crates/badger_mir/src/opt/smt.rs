//! SMT check for the superoptimiser's algebraic rules.
//!
//! Every algebraic rule [`super::superopt`] applies is stated here as a claim
//! `premises ⇒ src ≡ dst` over bitvectors, in the style of Alive / Alive2
//! (Lopes et al., PLDI 2015 / 2021): the premises are the conditions the
//! match site establishes (an operand's boolean domain, a nonzero shift
//! amount), `src` is the value the rule matches and `dst` the value it
//! rewrites to. The check asks the solver for a counterexample -- the
//! satisfiability of `premises ∧ src ≠ dst` -- so an *unsatisfiable* answer
//! is the proof that the rewrite preserves every bit, and a satisfiable one
//! is a concrete counterexample. [`Verdict::Unknown`] is neither.
//!
//! The encoding is over the values the rule talks about, not over MIR: each
//! free variable is a bitvector, the operations are the bitvector operations
//! with the signedness the rule uses, and a comparison is the 1-bit value
//! MIR's `Rvalue::Cmp` produces. The rules are stated at the four widths the
//! optimiser's integer types have (8, 16, 32, 64 bits); a rule is accepted
//! only when the solver proves it at every one of them. A shift rule is
//! checked at each shift amount the optimiser can use -- a concrete constant,
//! never a variable -- because the optimiser only ever applies a shift rule
//! at one, and because Z3 does not decide the 64-bit variable-divisor
//! division identity within the budget.
//!
//! The boolean rules (`x == 1` is `x`, double negation, comparison polarity)
//! are stated at width one: for those, "the operand is a proven boolean" is
//! exactly the statement that its value has one bit, so the width *is* the
//! precondition. [`super::superopt`] still checks the operand's boolean
//! domain at the match site and, for the rules marked [`Row::exhaustive`],
//! still runs its exhaustive verifier.
//!
//! # Modes and cost
//!
//! The check is off by default. `BADGER_MIR_SMT=1` runs it once per process,
//! before the first function is optimised: 81 claims on the rule set as it
//! stands, each a small bitvector query with a per-query timeout, 162 ms in
//! total (release-built CLI, 2026-10-20). The verdicts are recorded in a
//! [`Report`] (printed to stderr when the check runs) and a rule the solver
//! does not prove is not applied, so a dropped rule is visible rather than
//! silent. With the variable unset the optimiser never calls the solver: the
//! hot path is unchanged, and the compiler's output is the same either way.

use std::ops::Not;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use z3::ast::{Ast, BV, Bool};
use z3::{Params, SatResult, Solver};

/// Wall-clock budget for one claim; a query past it is [`Verdict::Unknown`].
const TIMEOUT: Duration = Duration::from_secs(10);

/// The widths the rules are stated at: the widths of the optimiser's integer
/// types (`Repr::I8` through `Repr::I64`).
const WIDTHS: &[u8] = &[8, 16, 32, 64];

/// Which algebraic rule a rewrite instance belongs to. One `Rule` may be
/// stated as several [`Row`]s (one per mirrored comparison, say); the verdict
/// is the worst over its rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    /// `x * 2^k → x << k`, wrapping (also the reverse of `x + x → x << 1`).
    MulPow2,
    /// `x / 2^k → x >> k`, unsigned.
    UDivPow2,
    /// `x % 2^k → x & (2^k - 1)`, unsigned.
    URemPow2,
    /// `(x << k) >> k → x & ~(2^k - 1)`, unsigned.
    ShrShlMask,
    /// A proven-boolean comparison against 0/1 kept as the boolean itself.
    BoolCmpKeep,
    /// A proven-boolean comparison against 0/1 as the negation of the boolean.
    BoolCmpFlip,
    /// `!!x → x` for a proven boolean.
    DoubleNeg,
    /// `!(a op b) → a (op') b`, the opposite comparison.
    CmpInvert,
    /// `(x + c) - c → x` and `(x - c) + c → x`, wrapping.
    AddSubCancel,
    /// `x + x → x << 1`, wrapping.
    AddSelfShl,
    /// `(x op c) op c → x op c` for `&` and `|`.
    MaskIdempotent,
}

/// Every rule, in the order of [`Rule`]'s variants; the report and the
/// recorded verdicts are indexed by [`Rule::index`].
pub const RULES: [Rule; 11] = [
    Rule::MulPow2,
    Rule::UDivPow2,
    Rule::URemPow2,
    Rule::ShrShlMask,
    Rule::BoolCmpKeep,
    Rule::BoolCmpFlip,
    Rule::DoubleNeg,
    Rule::CmpInvert,
    Rule::AddSubCancel,
    Rule::AddSelfShl,
    Rule::MaskIdempotent,
];

impl Rule {
    /// The position of this rule in [`RULES`]; the index of its verdict.
    pub fn index(self) -> usize {
        RULES.iter().position(|r| *r == self).expect("rule is in RULES")
    }

    /// A short description of the rewrite, for the check's report.
    pub fn name(self) -> &'static str {
        match self {
            Rule::MulPow2 => "x * 2^k -> x << k",
            Rule::UDivPow2 => "x / 2^k -> x >> k (unsigned)",
            Rule::URemPow2 => "x % 2^k -> x & (2^k - 1) (unsigned)",
            Rule::ShrShlMask => "(x << k) >> k -> x & (2^(bits-k) - 1) (unsigned)",
            Rule::BoolCmpKeep => "proven bool: (x == 1) -> x, (x != 0) -> x",
            Rule::BoolCmpFlip => "proven bool: (x == 0) -> !x, (x != 1) -> !x",
            Rule::DoubleNeg => "proven bool: !!x -> x",
            Rule::CmpInvert => "!(a op b) -> a op' b",
            Rule::AddSubCancel => "(x +/- c) -/+ c -> x",
            Rule::AddSelfShl => "x + x -> x << 1",
            Rule::MaskIdempotent => "(x op c) op c -> x op c for & and |",
        }
    }
}

/// The solver's verdict on a rule at every width it is stated at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The negation is unsatisfiable at every width: the rewrite preserves
    /// the value.
    Valid,
    /// A counterexample exists: the rule is wrong and is not applied.
    Invalid,
    /// The solver neither proved nor refuted the claim within the budget.
    Unknown,
}

impl Verdict {
    /// The worse of two verdicts: a rule is only valid when every width is.
    fn worse(self, other: Verdict) -> Verdict {
        match (self, other) {
            (Verdict::Invalid, _) | (_, Verdict::Invalid) => Verdict::Invalid,
            (Verdict::Unknown, _) | (_, Verdict::Unknown) => Verdict::Unknown,
            _ => Verdict::Valid,
        }
    }

    /// The word used in the report.
    fn word(self) -> &'static str {
        match self {
            Verdict::Valid => "valid  ",
            Verdict::Invalid => "INVALID",
            Verdict::Unknown => "unknown",
        }
    }
}

/// The width of a rule's free variable.
#[derive(Debug, Clone, Copy)]
enum VWidth {
    /// The rule's own width.
    W,
    /// One bit: a proven-boolean operand.
    One,
    /// The shift amount, enumerated over the values the optimiser can use
    /// (`0..width`) rather than left symbolic: the optimiser only applies a
    /// shift rule at a concrete amount, and the enumeration replaces the
    /// "amount in range" premise. At 64 bits Z3 does not decide the
    /// variable-divisor division identity within the budget.
    Shift,
}

impl VWidth {
    fn bits(self, w: u32) -> u32 {
        match self {
            VWidth::W | VWidth::Shift => w,
            VWidth::One => 1,
        }
    }
}

/// A value expression over a rule's free variables: the solver-facing form
/// of one side of a rule.
#[derive(Debug, Clone)]
enum Term {
    /// Free variable `n`, by its position in the rule's variable list.
    Var(u8),
    /// A literal at the rule's width.
    Const(u64),
    /// The rule's width, as a value: the shift-pair mask is `2^(width - k) - 1`.
    Width,
    /// Boolean negation of a 1-bit value.
    Not(Box<Term>),
    Op(Op, Box<Term>, Box<Term>),
}

/// The bitvector operation a [`Term`] node states. The signedness is the
/// rule's, not MIR's type: a MIR `Shr` is `Lshr` or `Ashr` by its type, and
/// the rules that use it state which.
#[derive(Debug, Clone, Copy)]
enum Op {
    Add,
    Sub,
    Mul,
    And,
    Or,
    Udiv,
    Urem,
    Shl,
    Lshr,
    /// Equality; a comparison is a 1-bit value, like MIR's `Cmp`.
    Eq,
    Ne,
    Ult,
    Ule,
    Ugt,
    Uge,
    Slt,
    Sle,
    Sgt,
    Sge,
}

/// One claim in the rule table.
struct Row {
    rule: Rule,
    /// The width the rule's own variables have; boolean rules are stated at
    /// width one.
    widths: &'static [u8],
    /// Free variables: name and width.
    vars: &'static [(&'static str, VWidth)],
    /// Assumptions drawn from the match site; the rule is applied only where
    /// the site establishes them.
    premises: Vec<Term>,
    src: Term,
    dst: Term,
    /// Whether the match site must still run the exhaustive verifier for
    /// this rule (the boolean rules, whose operand domain the site proves).
    exhaustive: bool,
}

/// One row's verdict at one width.
struct RowReport {
    rule: Rule,
    width: u8,
    verdict: Verdict,
    /// The counterexample (invalid) or the reason (unknown).
    detail: Option<String>,
}

/// The recorded result of the check: the per-row verdicts, the worst verdict
/// per rule, and how long the whole check took.
pub struct Report {
    rows: Vec<RowReport>,
    verdicts: [Verdict; 11],
    elapsed: Duration,
}

impl Report {
    /// The verdict for one rule; [`Verdict::Valid`] before the check runs.
    pub fn verdict(&self, rule: Rule) -> Verdict {
        self.verdicts[rule.index()]
    }

    /// The rules the solver did not prove, each with the width and reason of
    /// its first failing claim.
    pub fn rejected(&self) -> Vec<(Rule, &'static str, String)> {
        RULES
            .iter()
            .filter(|r| self.verdict(**r) != Verdict::Valid)
            .map(|r| {
                let row = self
                    .rows
                    .iter()
                    .find(|row| row.rule == *r && row.verdict != Verdict::Valid)
                    .expect("a rejected rule has a failing row");
                let reason = row.detail.clone().unwrap_or_default();
                (*r, r.name(), format!("width {}: {reason}", row.width))
            })
            .collect()
    }

    /// How long the whole check took.
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }
}

/// Whether the check is enabled (`BADGER_MIR_SMT=1`). Off, the optimiser
/// never touches the solver and the rule table is not even built. Read once:
/// the optimiser asks per candidate, and the environment does not change.
pub fn enabled() -> bool {
    static ENABLED: LazyLock<bool> = LazyLock::new(|| {
        std::env::var_os("BADGER_MIR_SMT").is_some_and(|v| v == "1")
    });
    *ENABLED
}

/// The check's result. Built once per process: the verdicts cannot change
/// between functions.
static REPORT: LazyLock<Report> = LazyLock::new(check_all);

/// Run the check, or return the result of the previous run.
pub fn run() -> &'static Report {
    &REPORT
}

/// The verdict for one rule: [`Verdict::Valid`] when the check has not run.
pub fn verdict(rule: Rule) -> Verdict {
    run().verdict(rule)
}

/// Whether a rule's rewrites may be applied: always, unless the check is
/// enabled and the solver did not prove the rule.
pub fn accepted(rule: Rule) -> bool {
    !enabled() || verdict(rule) == Verdict::Valid
}

/// Whether the match site must still run the exhaustive verifier for a rule:
/// true for a rule one of whose claims needs the site's operand domain.
pub fn needs_exhaustive(rule: Rule) -> bool {
    static TABLE: LazyLock<[bool; 11]> = LazyLock::new(|| {
        let mut table = [false; 11];
        for row in rows() {
            if row.exhaustive {
                table[row.rule.index()] = true;
            }
        }
        table
    });
    TABLE[rule.index()]
}

/// Print the report, once per process. Called by the optimiser when the
/// check runs, so a rule the solver dropped is visible in the build's log.
pub fn report() {
    static PRINTED: LazyLock<()> = LazyLock::new(|| {
        let report = run();
        eprintln!(
            "BADGER_MIR_SMT: {} rules, {} claims, {:.1} ms",
            RULES.len(),
            report.rows.len(),
            report.elapsed.as_secs_f64() * 1e3,
        );
        for rule in RULES {
            eprintln!("  {} {}", report.verdict(rule).word(), rule.name());
            for (rejected, _, detail) in report.rejected() {
                if rejected == rule {
                    eprintln!("      {detail}");
                }
            }
        }
    });
    LazyLock::force(&PRINTED);
}

/// The rule table, built once.
fn rows() -> &'static [Row] {
    static ROWS: LazyLock<Vec<Row>> = LazyLock::new(build_rows);
    &ROWS
}

fn var(n: u8) -> Term {
    Term::Var(n)
}

fn op(o: Op, a: Term, b: Term) -> Term {
    Term::Op(o, Box::new(a), Box::new(b))
}

fn not(a: Term) -> Term {
    Term::Not(Box::new(a))
}

/// A claim over the rule's widths with no boolean-domain premise: the
/// ordinary algebraic rules.
fn plain(
    rule: Rule,
    vars: &'static [(&'static str, VWidth)],
    premises: Vec<Term>,
    src: Term,
    dst: Term,
) -> Row {
    Row { rule, widths: WIDTHS, vars, premises, src, dst, exhaustive: false }
}

/// A claim over one-bit values: a boolean rule, whose precondition -- the
/// operand is a proven boolean -- is the statement that the value has one
/// bit. The match site still proves the domain and still runs the exhaustive
/// verifier, hence `exhaustive: true`.
fn bool_claim(rule: Rule, src: Term, dst: Term) -> Row {
    Row {
        rule,
        widths: &[1],
        vars: &[("x", VWidth::One)],
        premises: vec![],
        src,
        dst,
        exhaustive: true,
    }
}

/// `k != 0`: the shift pair clears at least one bit, the shift amount the
/// match site requires.
fn shift_nonzero() -> Term {
    op(Op::Ne, var(1), Term::Const(0))
}

fn build_rows() -> Vec<Row> {
    let mut rows = Vec::new();

    // x * 2^k -> x << k. The multiplier is written as `1 << k` so the claim
    // is the rule's, not one instance of it.
    rows.push(Row {
        rule: Rule::MulPow2,
        widths: WIDTHS,
        vars: &[("x", VWidth::W), ("k", VWidth::Shift)],
        premises: vec![],
        src: op(Op::Mul, var(0), op(Op::Shl, Term::Const(1), var(1))),
        dst: op(Op::Shl, var(0), var(1)),
        exhaustive: false,
    });

    // x / 2^k -> x >> k, unsigned.
    rows.push(Row {
        rule: Rule::UDivPow2,
        widths: WIDTHS,
        vars: &[("x", VWidth::W), ("k", VWidth::Shift)],
        premises: vec![],
        src: op(Op::Udiv, var(0), op(Op::Shl, Term::Const(1), var(1))),
        dst: op(Op::Lshr, var(0), var(1)),
        exhaustive: false,
    });

    // x % 2^k -> x & (2^k - 1), unsigned.
    rows.push(Row {
        rule: Rule::URemPow2,
        widths: WIDTHS,
        vars: &[("x", VWidth::W), ("k", VWidth::Shift)],
        premises: vec![],
        src: op(Op::Urem, var(0), op(Op::Shl, Term::Const(1), var(1))),
        dst: op(
            Op::And,
            var(0),
            op(Op::Sub, op(Op::Shl, Term::Const(1), var(1)), Term::Const(1)),
        ),
        exhaustive: false,
    });

    // (x << k) >> k -> x & (2^(bits-k) - 1), unsigned.
    rows.push(Row {
        rule: Rule::ShrShlMask,
        widths: WIDTHS,
        vars: &[("x", VWidth::W), ("k", VWidth::Shift)],
        premises: vec![shift_nonzero()],
        src: op(Op::Lshr, op(Op::Shl, var(0), var(1)), var(1)),
        dst: op(
            Op::And,
            var(0),
            op(
                Op::Sub,
                op(Op::Shl, Term::Const(1), op(Op::Sub, Term::Width, var(1))),
                Term::Const(1),
            ),
        ),
        exhaustive: false,
    });

    // x + x -> x << 1.
    rows.push(plain(
        Rule::AddSelfShl,
        &[("x", VWidth::W)],
        vec![],
        op(Op::Add, var(0), var(0)),
        op(Op::Shl, var(0), Term::Const(1)),
    ));

    // (x + c) - c -> x and (x - c) + c -> x, wrapping.
    rows.push(plain(
        Rule::AddSubCancel,
        &[("x", VWidth::W), ("c", VWidth::W)],
        vec![],
        op(Op::Sub, op(Op::Add, var(0), var(1)), var(1)),
        var(0),
    ));
    rows.push(plain(
        Rule::AddSubCancel,
        &[("x", VWidth::W), ("c", VWidth::W)],
        vec![],
        op(Op::Add, op(Op::Sub, var(0), var(1)), var(1)),
        var(0),
    ));

    // (x & c) & c -> x & c, and the same for |.
    rows.push(plain(
        Rule::MaskIdempotent,
        &[("x", VWidth::W), ("c", VWidth::W)],
        vec![],
        op(Op::And, op(Op::And, var(0), var(1)), var(1)),
        op(Op::And, var(0), var(1)),
    ));
    rows.push(plain(
        Rule::MaskIdempotent,
        &[("x", VWidth::W), ("c", VWidth::W)],
        vec![],
        op(Op::Or, op(Op::Or, var(0), var(1)), var(1)),
        op(Op::Or, var(0), var(1)),
    ));

    // The boolean rules, at width one: a proven boolean *is* a one-bit value.
    rows.push(bool_claim(
        Rule::BoolCmpKeep,
        op(Op::Eq, var(0), Term::Const(1)),
        var(0),
    ));
    rows.push(bool_claim(
        Rule::BoolCmpKeep,
        op(Op::Ne, var(0), Term::Const(0)),
        var(0),
    ));
    rows.push(bool_claim(
        Rule::BoolCmpFlip,
        op(Op::Eq, var(0), Term::Const(0)),
        not(var(0)),
    ));
    rows.push(bool_claim(
        Rule::BoolCmpFlip,
        op(Op::Ne, var(0), Term::Const(1)),
        not(var(0)),
    ));
    rows.push(bool_claim(
        Rule::DoubleNeg,
        not(not(var(0))),
        var(0),
    ));

    // !(a op b) -> a op' b. Eq and Ne are complements of each other for
    // every type MIR has (`Rvalue::Cmp`'s Ne is `Eq`'s negation by
    // definition), including floating point; the integer orderings are
    // complements for both signednesses.
    let invert = |rows: &mut Vec<Row>, src_op, dst_op| {
        rows.push(Row {
            rule: Rule::CmpInvert,
            widths: WIDTHS,
            vars: &[("a", VWidth::W), ("b", VWidth::W)],
            premises: vec![],
            src: not(op(src_op, var(0), var(1))),
            dst: op(dst_op, var(0), var(1)),
            exhaustive: false,
        });
    };
    invert(&mut rows, Op::Eq, Op::Ne);
    invert(&mut rows, Op::Ne, Op::Eq);
    invert(&mut rows, Op::Ult, Op::Uge);
    invert(&mut rows, Op::Ule, Op::Ugt);
    invert(&mut rows, Op::Ugt, Op::Ule);
    invert(&mut rows, Op::Uge, Op::Ult);
    invert(&mut rows, Op::Slt, Op::Sge);
    invert(&mut rows, Op::Sle, Op::Sgt);
    invert(&mut rows, Op::Sgt, Op::Sle);
    invert(&mut rows, Op::Sge, Op::Slt);

    rows
}

/// Run every claim and aggregate the verdicts.
fn check_all() -> Report {
    let start = Instant::now();
    let mut reports = Vec::new();
    let mut verdicts = [Verdict::Valid; 11];
    for row in rows() {
        for &width in row.widths {
            let report = check_row(row, width);
            verdicts[row.rule.index()] = verdicts[row.rule.index()].worse(report.verdict);
            reports.push(report);
        }
    }
    Report { rows: reports, verdicts, elapsed: start.elapsed() }
}

/// Decide one claim at one width: every enumerated shift amount in turn, and
/// the worst verdict over them.
fn check_row(row: &Row, width: u8) -> RowReport {
    let shift = row.vars.iter().position(|(_, var_width)| matches!(var_width, VWidth::Shift));
    let amounts: Vec<u64> = match shift {
        Some(_) => (0..u64::from(width)).collect(),
        None => vec![0],
    };
    let mut report = RowReport { rule: row.rule, width, verdict: Verdict::Valid, detail: None };
    for amount in amounts {
        let fixed: Vec<(usize, u64)> = shift.map(|at| vec![(at, amount)]).unwrap_or_default();
        let instance = check_instance(row, width, &fixed);
        if instance.verdict != Verdict::Valid && report.detail.is_none() {
            report.detail = instance.detail;
        }
        report.verdict = report.verdict.worse(instance.verdict);
    }
    report
}

/// Decide one instantiation of a claim: assert the premises and the negation
/// of the conclusion, and read the solver's answer. `fixed` pins a variable
/// to a concrete value (an enumerated shift amount); the rest are free.
fn check_instance(row: &Row, width: u8, fixed: &[(usize, u64)]) -> RowReport {
    let w = u32::from(width);
    let vars: Vec<BV> = row
        .vars
        .iter()
        .enumerate()
        .map(|(index, (name, var_width))| {
            let bits = var_width.bits(w);
            match fixed.iter().find(|(at, _)| *at == index) {
                Some((_, value)) => BV::from_u64(*value, bits),
                None => BV::new_const(*name, bits),
            }
        })
        .collect();
    let solver = Solver::new();
    let mut params = Params::new();
    params.set_u32("timeout", TIMEOUT.as_millis() as u32);
    solver.set_params(&params);
    for premise in &row.premises {
        solver.assert(bit_eq(&encode(premise, &vars, w), &one(1)));
    }
    let src = encode(&row.src, &vars, w);
    let dst = encode(&row.dst, &vars, w);
    solver.assert(bit_eq(&src, &dst).not());
    match solver.check() {
        SatResult::Unsat => RowReport { rule: row.rule, width, verdict: Verdict::Valid, detail: None },
        SatResult::Unknown => RowReport {
            rule: row.rule,
            width,
            verdict: Verdict::Unknown,
            detail: Some(format!("did not decide within {:?}", TIMEOUT)),
        },
        SatResult::Sat => {
            let detail = solver
                .get_model()
                .map(|model| counterexample(row, &vars, &model))
                .unwrap_or_default();
            RowReport { rule: row.rule, width, verdict: Verdict::Invalid, detail: Some(detail) }
        }
    }
}

/// A counterexample as `name = value` pairs, so a rule that needs a stronger
/// premise shows the assignment that breaks it.
fn counterexample(row: &Row, vars: &[BV], model: &z3::Model) -> String {
    let mut parts = Vec::new();
    for ((name, _), var) in row.vars.iter().zip(vars) {
        if let Some(value) = model.eval(var, true).and_then(|v| v.as_u64()) {
            parts.push(format!("{name} = {value}"));
        }
    }
    parts.join(", ")
}

/// A 1-bit value from a comparison's boolean, as MIR's `Cmp` produces.
fn bit(b: Bool) -> BV {
    b.ite(&one(1), &BV::from_u64(0, 1))
}

fn one(width: u32) -> BV {
    BV::from_u64(1, width)
}

/// Bitvector equality as a `Bool`; `BitVec` also has an `eq`, so this names
/// the one the encoding wants.
fn bit_eq(a: &BV, b: &BV) -> Bool {
    <BV as Ast>::eq(a, b.clone())
}

/// Encode a term at width `w`. A comparison always yields a 1-bit value; the
/// arithmetic and bitwise operations have the width of their operands.
fn encode(term: &Term, vars: &[BV], w: u32) -> BV {
    match term {
        Term::Var(n) => vars[usize::from(*n)].clone(),
        Term::Const(n) => BV::from_u64(*n, w),
        Term::Width => BV::from_u64(u64::from(w), w),
        Term::Not(a) => encode(a, vars, w).bvnot(),
        Term::Op(o, a, b) => {
            let a = encode(a, vars, w);
            let b = encode(b, vars, w);
            match o {
                Op::Add => a.bvadd(b),
                Op::Sub => a.bvsub(b),
                Op::Mul => a.bvmul(b),
                Op::And => a.bvand(b),
                Op::Or => a.bvor(b),
                Op::Udiv => a.bvudiv(b),
                Op::Urem => a.bvurem(b),
                Op::Shl => a.bvshl(b),
                Op::Lshr => a.bvlshr(b),
                Op::Eq => bit(bit_eq(&a, &b)),
                Op::Ne => bit(bit_eq(&a, &b).not()),
                Op::Ult => bit(a.bvult(b)),
                Op::Ule => bit(a.bvule(b)),
                Op::Ugt => bit(a.bvugt(b)),
                Op::Uge => bit(a.bvuge(b)),
                Op::Slt => bit(a.bvslt(b)),
                Op::Sle => bit(a.bvsle(b)),
                Op::Sgt => bit(a.bvsgt(b)),
                Op::Sge => bit(a.bvsge(b)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The optimiser's whole rule set must be proven: a rule the solver
    /// cannot prove is dropped, so this is the check that the algebraic
    /// rewrites the optimiser applies are sound.
    #[test]
    fn every_rule_is_proven() {
        let report = run();
        for rule in RULES {
            assert_eq!(
                report.verdict(rule),
                Verdict::Valid,
                "{} not proven: {:?}",
                rule.name(),
                report.rejected(),
            );
        }
    }

    /// Each rule must be stated at least once, the report must cover exactly
    /// the claims of the table, and every claim must carry its width.
    #[test]
    fn the_report_covers_every_rule() {
        let report = run();
        assert_eq!(report.rows.len(), rows().iter().map(|r| r.widths.len()).sum::<usize>());
        for row in &report.rows {
            let widths = rows().iter().find(|r| r.rule == row.rule).unwrap().widths;
            assert!(widths.contains(&row.width), "{} at {}", row.rule.name(), row.width);
        }
        for rule in RULES {
            assert!(
                report.rows.iter().any(|row| row.rule == rule),
                "{} has no claim in the table",
                rule.name(),
            );
        }
    }

    /// A rewrite the solver proved is applied; the check's own verdict is
    /// what `accepted` reports when the check is enabled.
    #[test]
    fn a_proven_rule_is_accepted() {
        for rule in RULES {
            assert_eq!(verdict(rule), Verdict::Valid);
        }
        // `accepted` returns true regardless of the environment when the
        // verdict is valid, and true unconditionally when the check is off.
        assert!(accepted(Rule::MulPow2));
    }
}
