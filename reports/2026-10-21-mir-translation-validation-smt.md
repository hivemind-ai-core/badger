# Alive2-style translation validation of the MIR passes (§4.3): adopted for the passes in reach

Measured 2026-10-21 against `81a7dda`. Status: **adopted, for the passes the
model covers**. §4.3 asks for a validator that proves each MIR pass preserves
the meaning of the program, in the style of Alive2 (Lopes et al., PLDI 2021).
It should answer within 10 s on a 50-KB MIR, with every `tests/run` program
paired with a validator run. The cheap form that landed on 2026-10-20 is a
structural determinism check plus a behavioural run
([`2026-10-20-mir-translation-validation.md`](2026-10-20-mir-translation-validation.md)).
That report left "an SMT model of MIR" open; this is it.

The survey came first: what can a translation validator decide about this
pipeline, and at what cost? It built the validator (`badger_mir::opt::tv`)
and ran it on every pass of every `tests/run` program. The answer splits the
pipeline in two.

- **In reach.** The simplification pass (`simplify`, all four instances), the
  common-subexpression pass (`cse`) and the superoptimiser. At the release
  level, **18 666 of the 18 695** functions these passes change are proven
  equivalent (99.84 %), and **none is refuted**. At `--opt superopt`, the
  superoptimiser's 2 488 changed functions are 2 477 proven, 7 not proven
  and **4 refuted**. The 4 refutations are a real, if narrow, finding (see
  [What the validator found](#what-the-validator-found)).
- **Out of reach.** Inlining, specialisation, scalar replacement, the enum
  and niche layout passes, the map and receive passes, and reachability.
  They change what the model leaves uninterpreted: calls, objects, runtime
  entry points, the function table. Covering them is a different and much
  larger model (below).

The cost pays:

- **One function.** A function proof takes 0.69 ms at p50 and 4.1 ms at p90.
- **The gate.** Validated one at a time with the check on, the 40 programs
  of 40–60 KB of MIR answer in 55 ms (median) and 5.2 s at most.
- **A bound.** A 9 s budget for the validator's own time bounds every
  compilation's check, so the 10 s gate holds by construction.

§4.1 proves the algebraic rules in the abstract. This proves whole passes on
the actual program, including the simplification pass, which §4.1 did not
cover. The validator is in the tree behind `BADGER_MIR_TV=1`. With the check
off it costs nothing; with it on, the compiler's output is byte for byte the
same.

## The model

`opt::smt` already stated the algebraic rules over bitvectors. It now also
states the MIR operations as code generation compiles them: each function
mirrors the Cranelift sequence in `badger_codegen` it is named after. That
covers `int_bin`'s wrapping arithmetic, its checked arithmetic, and its
division and shift defects; `icmp`; `int_convert`'s round-trip loss flag;
`Overflows`; and `coerce`'s narrowing and widening. The validator
(`opt/tv.rs`) builds a function's meaning from them:

- **Places.** The locals are a map from place to bitvector. Every local
  starts at zero (code generation zeroes them), and the parameters start at
  inputs shared by both versions.
- **Control flow.** Control flow is a path condition, and values are merged
  with `ite` where paths join. A defect (an overflow, a division by zero, a
  shift out of range, a lossy checked conversion) is an exit, with its kind
  and source location, under the condition the generated code tests.
- **Everything else is uninterpreted.** Memory and every effect (a call, an
  allocation, a store, a poll, a concatenation) thread an opaque *world*: an
  effect is a function of the world and its arguments. A load, a tag or a
  float operation is a function of its operands and of the world it reads.
  An uninterpreted operation is equal on both sides only when its arguments
  are, so the abstraction can fail a proof but never make one. The only
  facts stated about float operations are the ones IEEE 754 guarantees bit
  for bit: ordered comparisons mirror (`a > b` is `b < a`), equality is
  symmetric, and `!=` negates `==`.
- **Equivalence.** Two versions are equivalent when, from equal inputs and an
  equal world, they leave the same way: the same returned value in the same
  world, the same defect at the same location in the same world, or
  `Unreachable` in the same world.

**Loops** are cut at the headers both versions have, so each region between
cuts is acyclic. At a cut the two versions are related by an invariant: a
conjunction of candidate facts from which Houdini (Flanagan and Leino,
FME 2001) removes every fact some region does not re-establish. The
candidates are:

- a local both sides read is equal on both;
- a source copy (through copies and identity conversions) equals its original
  in the target, which is what copy propagation relies on;
- a local holds the constant its single reachable definition folds to, which
  is what constant folding relies on (found by evaluating the definition with
  the same encoding);
- a boolean local holds 0 or 1.

Each region is then proved, under its invariant, to reach the same exit as
the other version with the invariant of that exit established. That is a
simulation, so the proof covers every iteration, and it is sound:
"equivalent" means the negated claim is unsatisfiable for every region. A
loop only one version has (a loop the pass removed because it never iterates)
is cut at its back edges, so the proof goes through only if the back edge is
unreachable.

**Verdicts.** A satisfying assignment from the function's entry is
**refuted**. The verdict names the claim that fails and the inputs, for
example "the source returns 6148914691236517204, the target
12297829382473034408 when _1 = 1537228672809129301" for the deliberately
broken `n * 4 → n << 3`. It is a difference *in the model*: real when no
uninterpreted operation is involved, and otherwise a fact the pass relied on
that the model does not state. One from a loop region may start from a state
no run reaches, so it is **not proven**, as is a changed signature or locals,
an exhausted budget, and anything else. Nothing outside the model is ever
called equivalent.

**What was needed to make it decide.** The first prototype proved 96.5 % of
`simplify`'s functions, in 16 CPU-minutes over the corpus. These changes,
each measured, took it to 99.84 % in about 2 CPU-minutes:

1. **Substitution, not assumptions.** The invariant's equalities are a
   substitution: a union-find gives each equality class one variable or
   constant, so equal values are one term on both sides.
2. **Normalised commutative operations.** The solver gets them in a
   canonical operand order, the overflow test is stated as
   `ext(a) op ext(b) ≠ ext(a op b)`, and `rewriter.mul2concat` is set.
   Without this, `a * b` against `b * a` is a 64-bit multiplier equivalence
   for the SAT solver, and `x * 8` against `x << 3` (the superoptimiser's
   rewrite) takes it seconds.
3. **A fresh solver per query.** Z3 only applies its QF_BV preprocessing to
   a query it sees whole.
4. **Cached Houdini checks.** The checks are cached per region, and only the
   regions whose invariant changed are executed again.
5. **Facts from reachable definitions.** Constant and copy facts follow only
   the definitions a run can reach.

## Measurements

All times are from the debug build of the compiler (Z3 itself is built
optimised by `z3-sys`), on the 283 programs of `tests/run`, on a 16-core
machine shared with other builds. The survey validated every pass with no
budget (`BADGER_MIR_TV=all`), 6 programs at a time, writing one line per
changed function (`BADGER_MIR_TV_LOG`,
[`tv-survey/survey.sh`](tv-survey/survey.sh)). The unproven functions are in
[`tv-survey/release-unproven.tsv`](tv-survey/release-unproven.tsv) and
[`tv-survey/superopt-unproven.tsv`](tv-survey/superopt-unproven.tsv).

### The cost of one proof

| | |
|---|---|
| One §4.1 rule claim (for comparison) | ~2 ms (81 claims in 170 ms) |
| One function, in-reach pass, p50 / p90 / p99 | 0.69 ms / 4.1 ms / 34 ms |
| The broken superoptimiser rewrite, refuted | 2.6 ms for the 2 functions it changes |

### How it scales with the function

Release level, the 18 695 function changes of the in-reach passes, by the
function's statement count before the pass:

| statements | functions | p50 | p90 | p99 | max | not proven |
|---|---|---|---|---|---|---|
| < 10 | 10 704 | 0.55 ms | 0.90 ms | 6.9 ms | 26 ms | 0 |
| 10–29 | 3 768 | 0.83 ms | 1.3 ms | 4.2 ms | 12 ms | 0 |
| 30–99 | 2 652 | 1.6 ms | 4.4 ms | 14 ms | 2.0 s | 3 |
| 100–299 | 1 030 | 5.9 ms | 13 ms | 290 ms | 2.1 s | 5 |
| 300–999 | 450 | 19 ms | 55 ms | 660 ms | 2.6 s | 3 |
| 1 000–2 999 | 80 | 97 ms | 2.1 s | 2.3 s | 2.3 s | 13 |
| ≥ 3 000 | 11 | 1.2 s | 4.1 s | 10 s | 10 s | 5 |

The cost is roughly linear in the function up to a few hundred statements.
Beyond that it is dominated by the few queries that relate two differently
built path conditions or worlds, which is where the 2 s per-query and 10 s
per-function budgets are spent. Every function a budget stops is "not
proven".

### The gate: 10 s on a 50-KB MIR

The MIR size is the length of `mir::dump` of the program before the pipeline
(`tests/run`: 4 KB to 537 KB, median 44 KB).

- **Without a budget** (the survey), the 40 programs of 40–60 KB take 89 ms
  (median) and 6.5 s at most (`local_fn`, 40 KB, three functions of 7 400
  statements). One program in 283 exceeds 10 s in all: `data_structures`,
  362 KB, 11.0 s.
- **With the check on** (`BADGER_MIR_TV=1`), the validator is held to 9 s of
  its own time per compilation (the passes' time does not count; the last
  second is for copying and comparing the programs). Each pass's changed
  functions are validated smallest first, and the functions left when it is
  spent are not proven. The check therefore answers within 10 s whatever the
  program. Validated one at a time, the 40 programs of 40–60 KB answer in
  **55 ms (median) and 5.2 s at most**, with 1 890 of their 1 893 changed
  functions proven. On the machine at a load average of 56,
  `local_fn` took 9.9 s with 49 of its 50 simplified functions proven, where
  it had taken 20 s before the budget.

The gate holds.

**Determinism.** A refutation is a property of the formula, and the same
pass output gives the same refutations in every run. The time a query takes
is not: the MIR carries runtime addresses (descriptors, strings, source
locations) that differ from run to run, and Z3's internal hashing follows
them. So a function near the 2 s per-query budget can be proven in one run
and stopped in another. This moves a function between "equivalent" and "not
proven", never to "refuted", and never changes the compiler's output.

## Which passes are in reach

The survey validated every pass at the release level (32 465 changed
functions) and at `--opt superopt` (35 162):

| pass | changed functions | proven | refuted | not proven | in reach |
|---|---|---|---|---|---|
| `simplify` | 18 023 | 17 999 | 0 | 24 | yes |
| `simplify2` | 565 | 560 | 0 | 5 | yes |
| `simplify3`, `simplify4` (superopt) | 239 | 238 | 0 | 1 | yes |
| `cse` | 107 | 107 | 0 | 0 | yes |
| `superopt` (superopt) | 2 488 | 2 477 | 4 | 7 | yes |
| `inline`, `inline2` | 7 449 | — | — | — | no: the locals and blocks change |
| `reach` | 5 532 | — | — | — | no: the function table is renumbered |
| `split`, `maps-private`, `maps-snapshot` | 328 | — | — | — | no: runtime entry points change |
| `enums`, `niche` | 309 | — | — | — | no: object layouts change |
| `sra`, `sra2` | 176 | — | — | — | no: allocations become locals |
| `specialize` | 80 (+ 44 added) | — | — | — | no: closure calls become direct |

For the out-of-reach passes, the survey mode still ran the per-function
validator. It reports "not proven" for a changed shape, and spurious
refutations where an uninterpreted call changes name: 27 for `specialize`,
which turns `Closure(c)` into `Lambda(f, c)`, and 1 for `maps-snapshot`.
What each pass would need:

- **Inlining**: expanding the source's call into the callee's body during
  symbolic execution, with the callee's own loops cut and related to the
  inlined copy. That is interprocedural translation validation, beyond the
  function-at-a-time check Alive2 itself performs.
- **Specialisation**: the fact that a closure's code pointer is the lambda
  it was built from, a memory fact over the closure object.
- **`sra`, `enums`, `niche`**: a memory model with fresh allocations (their
  non-aliasing, their fields as places) and the object layouts. This is
  Alive2's block memory model, the larger half of that system.
- **`split` and the map passes**: specifications of the runtime functions
  they swap (a private map's update, a snapshot's reuse).
- **`reach`**: no SMT. A structural check that the renumbering is a
  bijection on the functions kept.

Each is the size of the work done here or larger. The in-reach passes are the
ones the existing semantics already states, and the ones that make by far the
most changes (`simplify` alone makes 2 324 on
`tests/run/acceptance_static.bdg`, against `inline`'s 83).

## What it does not decide

At the release level, 29 of the 18 695 in-reach function changes are not
proven; none is called equivalent, and none is refuted.

- **15 stopped by the 2 s query budget.** These are mostly large `main`
  functions (up to 5 900 statements) and a few lambdas of about 1 000
  statements. The query that times out relates two differently built path
  conditions over uninterpreted loads (`admission_gate.main`: 277 `ite`s).
  Two are small (90 statements): one query over their path conditions
  exceeds 2 s.
- **14 whose loop invariant is too weak**: a fact the candidates do not
  offer. One is `collections_extra.main`, where `simplify` merged a block
  chain so that a loop header is reached from another inlined copy of the
  loop. Correspondence by block index does not hold there. Others are two
  lambdas each of `gc_pool_boundaries` and `worker_supervise`, rewritten
  inside a receive loop.

## What the validator found

- **The superoptimiser reorders the operands of float `+` and `*`.**
  `canonicalize` treats `Add` and `Mul` as commutative for every type. On
  x86-64, `a * b` and `b * a` propagate different NaN payloads when both
  operands are NaNs, and the payload is observable: floats are hashed by
  their bits (`badger_runtime::value`) and the binary codec writes them bit
  for bit. The model leaves float arithmetic uninterpreted, so it refutes the
  4 functions where this happens (`basics.main`, `basics.area`, and two
  `area`s of `dyn_protocol`), at `--opt superopt` only. The comparisons the
  superoptimiser mirrors are exact for floats and are proven. The fix
  belongs in `superopt.rs` (restrict the operand swap to integers, or define
  NaN payloads as unspecified); it is not made here.
- **The deliberately broken rewrite** (`BADGER_MIR_BREAK=superopt`) is refuted
  before the program runs, and the compilation fails with E9001.
- **No refutation in `simplify` or `cse`**: the corpus has no miscompile in
  the passes the model covers.

## What landed

- `opt::smt` gained the MIR operations' bitvector semantics (`int_bin`,
  `int_cmp`, `int_convert`, `int_overflows`, `coerce`, the defect kinds), next
  to the rule table. The file is rustfmt-clean in its own commit.
- `opt::tv` is the validator. It provides `validate(src, tgt)`,
  `check_pass(pass, before, after, validate, budget)`, the modes, the summary
  and the log, with 8 unit tests:
  - a fold proven;
  - a wrong fold refuted with both values;
  - a dropped overflow check refuted;
  - reordered effects refuted;
  - a loop after copy propagation proven, and a wrong loop body not proven;
  - a changed shape not proven;
  - swapped float equality proven, and a swapped float product refuted;
  - the in-reach pass list.
- `opt::optimize_checked` is the pipeline with a per-pass hook. The program
  is copied only when a check is given.
- `badger_driver` has three modes:
  - `BADGER_MIR_TV=1` validates the in-reach passes within the budget and
    counts the other passes' changes as outside the model. It prints a
    summary per pass to stderr when a function is not proven, so a fully
    proven compilation's stderr is unchanged. A refutation fails the
    compilation (E9001), as `BADGER_MIR_VALIDATE=1` does.
  - `BADGER_MIR_TV=all` validates every pass, with no budget and no failure
    (the survey).
  - `BADGER_MIR_TV_LOG=FILE` appends one line per changed function.
- `badger_cli/tests/mir_validation.rs` checks that the broken rewrite is
  refuted, and that every `tests/run` program compiles with the check on with
  no refutation, to byte-identical saved MIR. The corpus suite
  (`programs.rs`, one worker and four) passes unchanged with
  `BADGER_MIR_TV=1`.

## Reproducing

```sh
cargo build -p badger_cli
reports/tv-survey/survey.sh target/debug/badger release /tmp/tv-release.tsv
reports/tv-survey/survey.sh target/debug/badger superopt /tmp/tv-superopt.tsv
BADGER_MIR_TV=1 target/debug/badger run --release --opt superopt tests/run/basics.bdg
```
