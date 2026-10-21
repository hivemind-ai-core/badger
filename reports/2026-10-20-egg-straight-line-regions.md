# egg for straight-line MIR regions (§4.2): not adopted

Measured 2026-10-20 against `c954d5a`. Status: **not adopted**. The item
asks for the straight-line MIR regions to be lifted into the egg
equality-saturation engine (Willsey et al., PLDI 2021), with the rule set
§4.1 proves with Z3. It expects ~3–5 % more rewrites on the inner-loop MIR.
The pass was built and measured: it meets the gates (a 50 ms budget per
function, a size-bounded extraction, the §4.1 rule set as the only source of
rewrites), and the corpus behaves identically with it on and off. It finds
**2 rewrites** the hand-written rule set does not, over the 263 programs of
`tests/run` (2 483 functions, 36 798 lifted statements), and **none** on any
`bench/*.bdg` program. That is +0.15 % on the 1 323 rewrites `superopt` makes,
for a new dependency (egg and 13 transitive crates) and +3–4 % on the MIR
optimisation phase of the large programs. With the §4.1 rule set the search
has nothing to find: the limit is the rules, not the order they are applied
in. The implementation is kept as a patch
([`egg-survey/egg.patch`](egg-survey/egg.patch)) and does not land.

## What was built

The patch adds `crates/badger_mir/src/opt/egg.rs` (about 1 130 lines, and
8 unit tests), a pass after `superopt` at `--opt superopt` that is off unless
`BADGER_MIR_EGG=1` is set (`BADGER_MIR_EGG=stats` also prints what it found and
how long it took).

- **The language.** These integer operations are lifted, the ones the item
  names: `Rvalue::Bin`, `Cmp`, `Not`, `Convert`, `Neg`. Only the forms that
  cannot trap become e-nodes: wrapping arithmetic, bitwise operations, a
  shift by an in-range constant, a division by a nonzero constant (and not
  by −1 when signed), comparisons, and conversions that are lossless or
  unchecked. Every other value a region reads is an opaque leaf: a checked
  operation, a load, or a local's value at the region's start. So no rewrite
  can move, add or remove a trap. MIR's signedness is part of the operation
  (`Shr` is a logical or an arithmetic shift, `Lt` an unsigned or a signed
  comparison), as in the §4.1 encoding.
- **The rules.** The rewrites are compiled from the rows of `opt::smt`'s rule
  table, the claims `BADGER_MIR_SMT=1` proves. There is no second rule list,
  and a rule the solver has not proved is not compiled. A row's
  constant-only subterms (`1 << k`, `2^k − 1`) match constant classes by
  evaluation at each shift amount the claim enumerates, and its premises
  are evaluated at that amount. So every rewrite instance is an instance of
  a claim the solver checked. A boolean row, stated at width one, is
  instantiated at the 8-bit width MIR stores a boolean in, with its
  variables required to be proven booleans (`bool_locals`, carried as an
  e-class analysis). This is the reading the §4.1 table gives those rows.
  One rule was added to the table: **commutativity**, `a op b → b op a` for
  `+ * & | ^ == !=`, and the mirrored comparisons (`a < b → b > a`, both
  directions). `superopt` canonicalises every statement to put the
  constant first, which is the order its own rules match only because each
  match site tries both orders by hand. An e-graph pattern matches one
  order, so it needs the claim. The solver proves it at all four widths: 60
  more claims, 141 in all, checked in 169 ms (debug build) against 162 ms for
  the previous 81. The 39 rows compile to 141 rewrites.
- **The regions.** A region is a run of one block between effects. A store,
  a poll, or an assignment from a call, an allocation or a concatenation
  ends it; this is the break-point list of the research note (§2.2).
- **The readback.** The result is written back one statement at a time, in
  place. Each lifted statement takes the cheapest member of its class that
  can be written at that point: a constant, a local that holds the value
  there, or one operation over such operands. The cost is egg's tree cost
  (the operation, plus the cheapest cost of each operand's class), so
  `!(a < b)` costs two and `a >= b` one. A statement is rewritten only when
  that is cheaper than the statement as written (costed after saturation,
  so an operand that became cheaper is not a rewrite of the statement) and
  cheaper than the best the e-graph offered before saturation (hash-consing
  alone finds common subexpressions, which `cse` already removes). No
  statement is added, removed or moved.
- **The bounds.** A region holds at most 64 lifted statements; a longer run
  is cut. A function's regions share one e-graph (their leaves are
  distinct, so they do not interact) and one saturation run. The run stops
  after 8 iterations or at 20 000 e-nodes. Both bounds are deterministic, and
  a partly saturated e-graph is still sound. On top of that, a function has
  a wall-clock budget of 50 ms; one whose run does not finish within it is
  left exactly as it was. The size bound never binds on the corpus: the
  largest function e-graph has 2 347 nodes, no run reached a limit, and
  regions hold 1.8 lifted statements on average (median 1, maximum 7).

## Method

All counts are over the 263 programs of `tests/run` and the ten
`bench/*.bdg` and `bench/realworld/*.bdg` programs, compiled with
`--release --opt superopt`.

- **Rewrites beyond the rule set.** The pass runs where it would ship, after
  `superopt`. It counts the statements it rewrites.
- **Parity.** The pass is moved before `superopt` (one line in
  `opt::passes`). Each rule-driven rewrite `superopt` makes afterwards is
  counted by rule. Rewrites that only reorder operands are excluded. The
  same count without egg gives the rule set's own total.
- **Time.** Each function's time in the pass is measured by the
  `BADGER_MIR_EGG=stats` report, with a release build of the compiler and
  the programs run one at a time. The cost to the compiler is the
  `BADGER_TIMINGS=1` `optimize` phase, as the median of 11 interleaved runs
  with the pass off and on.

## Results

| | Rewrites |
|---|---:|
| `superopt`'s rule-driven rewrites (no egg) | 1 323 |
| — `x * 2^k → x << k` | 1 269 |
| — `!(a op b) → a op' b` | 45 |
| — boolean `x == 0 → !x` | 5 |
| — unsigned `/ 2^k`, `% 2^k` | 4 |
| egg before `superopt` | 1 320 |
| `superopt` after egg | 5 |
| **egg after `superopt`** | **2** |
| egg after `superopt`, every `bench/*.bdg` | 0 |

- **Parity holds.** Run first, egg finds 1 318 of the rule set's 1 323
  rewrites. The five it leaves are boolean comparisons of a proven boolean
  held in a 64-bit local. The 8-bit instantiation of the boolean rows does
  not cover these.
- **The two rewrites the rule set misses** are both `!(c < x) → x <= c`
  (`collections_misc.main`, `sequence_search.main`). `superopt` refuses them
  because the comparison reads a local with several definitions in the
  function, and its `stable_operands` test cannot tell whether the local
  changed between the comparison and the negation. Inside a region, egg
  tracks values rather than locals and knows it did not. Each rewrite
  leaves the comparison it read dead, which saves one instruction in one
  function of one program.
- **Nothing composes.** The rules do not feed each other: no rule's result
  is another rule's source, except through commutativity. The one form where
  the claims say more than the hand-written match does never occurs in the
  corpus. That form is `(x + y) − y` with a variable `y`: the claim holds for
  every `c`, but the match site needs a constant. A unit test shows the pass
  finds it.
- **The regions are not the limit.** With no break points (a whole block as
  one region, effects as opaque leaves), the counts are the same: 2 after
  `superopt`, 1 320 before it.

### Time

Release build, programs one at a time, every function with a lifted
region (2 483):

| Build | p50 | p90 | p99 | p99.9 | max | Over budget |
|---|---:|---:|---:|---:|---:|---:|
| release | 51 µs | 0.27 ms | 1.4 ms | 3.3 ms | 4.7 ms | 0 |
| debug | 0.46 ms | 2.2 ms | 5.9 ms | 26 ms | 31 ms | 0 |

The 50 ms budget is met with a release build by an order of magnitude. Time
does not follow e-graph size. The slowest functions have about 100 e-nodes,
and run alone they take 0.9–1.1 ms, so the tail is load from other processes
on this shared machine. An earlier sequential run had a maximum of 16.9 ms.
In a parallel corpus run on a loaded machine, one function took 91 ms and
was left alone. A **debug build** of the compiler is about ten times slower:
egg's generic code is instantiated in `badger_mir`, which the dev profile
compiles at `opt-level = 0`. Under parallel load, 2–5 functions per debug
corpus run pass the budget and are left alone. That is sound, but it makes
the output depend on timing, which `BADGER_MIR_VALIDATE=1`'s determinism
check would report.

The `optimize` phase, with the pass off and on (medians of 11 interleaved
runs, release build):

| Program | Off | On | Δ |
|---|---:|---:|---:|
| `tests/run/collections_misc.bdg` | 349.6 ms | 363.0 ms | +3.8 % |
| `tests/run/data_structures.bdg` | 388.0 ms | 400.2 ms | +3.1 % |
| `tests/run/acceptance_static.bdg` | 7.9 ms | 8.8 ms | +11 % |
| `bench/records.bdg` | 5.3 ms | 6.4 ms | +21 % |

On the small programs the fixed cost dominates. The 141 rewrites are
compiled once per process (0.15 ms), and every function with a lifted
region pays a saturation run and two extractions, however few its
statements. `bench/records.bdg` gets no rewrite, so its compiled code is
unchanged by construction.

### Correctness

- With the pass on, every corpus program gives the same stdout and exit
  status as with it off and as its `.out`/`.exit` file (263 of 263, with 1
  and with 4 workers).
- The rule table, with commutativity, is proved: `opt::smt`'s
  `every_rule_is_proven` passes.
- The unit tests cover a variable cancellation the rule set does not find,
  a match through the canonical operand order, a checked operation left
  alone, an effect as a break point, the region bound, a function past its
  budget left unchanged, and the boolean premise.

## Why the premise does not hold

Equality saturation pays when rewrites interfere: one rule's result enables
another, or two rules compete for the same term, and a greedy pass picks the
wrong order. The §4.1 rule set has neither property. It is 12 local
strength reductions and boolean identities, each of which `superopt` applies
in one pass at its match site. `simplify` and `cse` then clean up after it
before the pass runs. The gate keeps the egg rule set equal to that one, so
Z3 stays the single source of validity, and the e-graph can only find what
those rules find, in fewer passes. The 2 extra rewrites are not equality
saturation at all. They come from tracking values rather than locals within
a block, and `superopt`'s match site could do that directly.

## What would pay instead

- **A larger proved rule set.** The rewrites that would make saturation pay
  are the ones that interfere: reassociation of constants (`(x + c1) + c2`),
  distribution, `x << a << b`, and the bitwise identities a Ruler-style
  synthesis (research note §2.4) would propose. Each needs a constant-folding
  term in its target, which the §4.1 encoding does not have today. Each is
  stated, and proved, in the §4.1 table first. Until that table has rules
  that compose, `superopt`'s single pass finds what egg would.
- **The commutativity claim, on its own.** `superopt` canonicalises the
  operand order of every statement without a proved claim. The patch's 15
  commutativity rows prove it (60 claims), and could land in `opt::smt` as a
  §4.1 follow-up, with `canonicalize` gated on the rule like the others.
- **Value tracking in `superopt`.** The two misses are a match-site
  limitation. `earlier_def` and `stable_operands` could accept a local
  defined more than once in the function when it is not reassigned between
  the definition and the use, the test egg's regions apply.

## Reproducing

`git apply reports/egg-survey/egg.patch` on `c954d5a` adds the pass, its
tests, the commutativity rows and the dependency. Then:

```sh
cargo build --release -p badger_cli
BADGER_MIR_EGG=stats target/release/badger run --release --opt superopt tests/run/collections_misc.bdg
```

The report line on stderr gives the rewrites, functions, regions, lifted
statements, functions at a limit or over budget, the largest e-graph, and
the per-function p50 / p99 / max.
