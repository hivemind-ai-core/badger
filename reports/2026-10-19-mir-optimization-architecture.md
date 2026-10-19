# Release MIR optimization and closure specialization

Reviewed 2026-10-19 against `78cc5a4fbfe57750e2b2db2d2eb6d772da0555e3`.
Status: current implementation findings and proposed architecture. The
runnable slices in "Optimization priorities" are implemented on
`perf/mir-superopt-2026-10-19`; see [Implementation](#implementation) below
for the pass list, controls, and structural checks. No timing or
allocation-telemetry claim is made here.

## Recommendation

Make language-aware optimization an explicit HIR/MIR responsibility. Preserve
semantic information through early lowering, optimize MIR aggressively within
compile-time and code-size budgets, then lower to an explicit representation
that codegen can translate mechanically into Cranelift IR. Let Cranelift perform
its own backend optimizations.

The intended release behavior is that ordinary higher-order code loses
avoidable closure dispatch and allocation costs. Eligible pipelines should
produce the same loop structure as equivalent handwritten code. Establish the
structure with IR inspection and establish performance separately with matched
benchmarks.

This extends existing machinery: bounded MIR inlining, local scalar replacement,
selected sequence fusion, and other release MIR passes already exist. The
[October 15 codegen audit](2026-10-15-compiler-codegen-optimization-audit.md)
describes an older snapshot; its missing-optimization findings are historical,
not a statement of current implementation status. The broader original plan is
now [archived](../archive/plans/OPTIMIZATION_PLAN.md).

## Current closure behavior

The [MIR inliner](../crates/badger_mir/src/inline.rs) recognizes closures allocated
in the caller whose code word identifies a lambda, including single-definition
copies. It can inline a helper receiving that known closure, inline the exposed
lambda calls, and run [scalar replacement](../crates/badger_mir/src/sra.rs) to
remove the environment object when its remaining uses permit that removal.
Captured values then become ordinary MIR locals.

The present policy has two rounds, size limits of 40 for ordinary callees and
80 for direct callees receiving known closures, and a caller growth threshold
of 4,000. Size is counted from statements excluding source-location markers,
plus terminators in nontrivial blocks. The growth threshold is checked before
an insertion, not a strict maximum final size. Representation compatibility and
recursion restrictions also apply. Live dispatch disables the pass.

Therefore, a closure passed directly to `map`, `filter`, or `fold` can already
be inlined in release mode. This is a bounded optimization, not a guarantee for
every callback parameter or forwarding chain. Merely proving that a parameter
does not escape does not identify a runtime-selected callback's target. Escape
analysis, known-target analysis, and the inlining policy answer different
questions.

The [driver](../crates/badger_driver/src/lib.rs), in `lower_to_mir`, currently runs
inlining, private/snapshot map reuse, split-result rewriting, local Option
elision, enum scalarization, and reachability pruning for optimized non-live
programs. `BADGER_NO_INLINE` currently disables this entire block and selected
lowering optimizations, so it is not an isolated switch for benchmarking the
inliner. Local scalar replacement also runs in `FnLower::finish`, independently
of the release-only block.

Current [CLI compilation](../crates/badger_cli/src/main.rs) sets `optimize: true`
for `badger compile` and `badger release`, so newly saved MIR receives release
MIR optimizations. Loading saved MIR does not rerun those passes; the run mode
controls Cranelift optimization. Older artifacts are not retroactively optimized
at MIR level. This is more recent behavior than historical notes describing
saved compilation as bypassing the release MIR pipeline.

## Fresh structural evidence

Rebuilt the release CLI through the focused integration-test commands below.
Ran five small programs with and without language `--release`, clearing inherited
`BADGER_*` and `LANG_*` variables. Every program produced its expected output
and exited successfully in both modes.

The toolchain was Rust `1.98.1 (48a229cea 2026-09-01)` with locked Cranelift
`0.136.3`. The tested `target/release/badger` executable had SHA-256
`67e9a6933c59a0684c84040135b2d9dc6edbf069d08ee39892cadfe555d8c4fa`.

| Probe | Expected output | Explicit allocations in main, normal / release | Closure call sites in all emitted MIR bodies, normal / release |
| --- | --- | --- | --- |
| Capturing `List.map` callback | `[3, 6, 9, 12]` | 1 / 0 | 1 / 0 |
| Capturing `List.filter` callback | `[2, 4]` | 1 / 0 | 1 / 0 |
| Capturing `List.fold` callback | `30` | 1 / 0 | 1 / 0 |
| Eager map/filter/fold chain | `18` | 3 / 0 | 3 / 0 |
| Capturing callback passed to `twice` | `18` | 1 / 0 | 2 / 0 |

These are static MIR site counts, not dynamic allocation counts. `Alloc` does
not include allocations performed inside runtime helpers. Normal-mode callback
calls occur in helper bodies, so counting only main would incorrectly report
zero callback calls before optimization. Main's explicit allocations in these
probes are the closure objects.

The release chain retains two list-builder creation sites and two list-finish
sites in main. Its callback overhead is removed, but the map and filter output
lists are still materialized. This does not demonstrate equal performance to a
single handwritten loop. No timing, assembly comparison, allocation telemetry,
or worker-scaling experiment was performed for this report.

Validation passed:

```sh
cargo test -p badger_cli --release --test programs inlined_lambda_defect_location -- --exact
cargo test -p badger_cli --release --test programs inlining_removes_helper_calls_and_allocations -- --exact
```

Each command passed its one selected test. The first checks overflow reporting
at the lambda's source location in normal and optimized modes. The second checks
that a small helper call and its record allocation disappear from release MIR.
These focused checks are not a full compiler/runtime validation run.

## Inlining and fusion are separate transformations

Inlining removes a call boundary. Scalar replacement can then remove the closure
environment and other aggregates. Neither transformation alone removes the
intermediate collections in an eager pipeline.

Selected lazy `Seq` pipelines already undergo
[fusion during HIR-to-MIR lowering](../crates/badger_mir/src/lower/fusion.rs).
The implementation recognizes finite list/range sources, map/filter/take stages,
and fold/count/for consumers. It evaluates construction arguments once in source
order and retains stage ordering. Unknown and scoped iterators retain their
normal iterator and cleanup behavior.

Eager `List` fusion requires additional legality analysis. For example, eager
map followed by filter runs every map callback before any filter callback;
a single fused loop interleaves them. Even callbacks without external effects
may overflow or fail, so purity alone does not establish that this reordering
preserves observable behavior. Fuse only when the compiler proves the relevant
ordering changes harmless, or when the source API already specifies lazy
iteration with the same order.

## Proposed phase boundaries

| Phase | Responsibility |
| --- | --- |
| Typed HIR and early lowering | Retain concrete types, effect information, capture identities, collection operations, and source positions; expose specialization opportunities. |
| Semantic MIR optimization | Specialize known callbacks, inline profitable calls, analyze escape and aliases, replace aggregates with scalars, propagate constants, remove dead work, optimize loops, and eliminate checks only with proof. |
| Final MIR lowering and analysis | Select concrete representations and runtime operations; derive safepoint roots, barrier requirements, and calling conventions after transformations affecting liveness. |
| Cranelift emission | Translate explicit operations and analysis results into valid CLIF, including ABI and runtime protocol details. |
| Cranelift compilation | Apply its enabled IR/backend optimizations and generate machine code. |

Significant optimization should be represented as named, independently
inspectable passes, rather than hidden inside MIR construction. Early semantic
MIR should retain enough information that optimizations do not have to reverse
engineer generic runtime calls. A separate IR type is not necessarily required
initially; explicit stages and invariants are.

Codegen currently sets Cranelift `opt_level` to `speed` in optimized mode and
`none` otherwise. It also tracks fresh allocations to omit certain write
barriers and emits root publication around safepoints. Move language-level
decisions such as barrier elision into explicit MIR analyses; retain mechanical
ABI emission, root-frame layout, and runtime protocol expansion where appropriate
in the backend. Recompute or invalidate derived analyses after MIR mutations.

This proposal does not require duplicating every optimization Cranelift already
performs. Prioritize transformations that use language semantics or expose work
hidden behind calls and allocations. Add overlapping generic passes when they
enable further MIR transformations or measured missed cases justify them.

## Optimization priorities and runnable delivery slices

1. **Make the pass pipeline explicit.** Centralize release policy and pass
   ordering, add before/after dumps and per-pass disable controls, and verify
   invariants at pass boundaries in compiler validation builds. Acceptance:
   source release and newly compiled MIR show equivalent optimization choices;
   disabling one pass does not silently disable unrelated passes.
2. **Specialize closure parameters independently of inlining.** Clone an
   eligible helper for a known lambda and pass captures directly, even when the
   helper is too large to inline. Then apply budgeted inlining and cleanup.
   Acceptance: direct lambdas, captured scalars/pointers, and forwarding wrappers
   lose indirect calls and environment allocations where proven unnecessary;
   unknown or escaping callbacks retain correct fallback behavior.
3. **Strengthen simplification and allocation removal.** Establish suitable
   dataflow or SSA infrastructure, add constant/copy propagation, unreachable
   block and dead-definition elimination, and repeat scalar replacement after
   specialization. Acceptance: inspectable removal of newly exposed branches,
   calls, and aggregates without losing captured managed pointers across GC.
4. **Expand collection and loop optimization.** Broaden legal lazy fusion and
   typed traversal, and add eager fusion only with the necessary effect/failure
   proofs. Add bounds-check elimination and loop-invariant motion with explicit
   legality rules. Acceptance: eligible pipelines retain no intermediate
   builders and match a handwritten loop's operation structure; ordered-effect
   and early-exit controls remain equivalent.
5. **Move remaining language decisions out of emission.** Make barrier-elision
   and root-analysis results explicit inputs, derived after optimization.
   Acceptance: codegen consumes the decisions, with allocation-heavy and GC
   stress checks preserving liveness and write-barrier correctness.
6. **Evaluate an optional superoptimization mode.** Start with small pure
   integer/boolean MIR regions, enforce search/time budgets, cache proven
   rewrites, and compare against the conventional release pipeline. Acceptance:
   replacements are verified under Badger semantics, timeouts retain the
   original region, and end-to-end benchmarks demonstrate a useful tradeoff.

Each slice should preserve a working compiler and carry its own focused proof.
Use benefit-aware inlining budgets and specialization deduplication to control
code growth; indiscriminate inlining is not the performance objective.

## Optional superoptimization

Superoptimization is a proposal, not an existing setting. A conventional release
pipeline should remain predictable. An opt-in expensive mode can search for
better equivalent expressions in bounded regions; an SMT-backed IR optimizer
such as [Souper](https://github.com/google/souper) illustrates this approach.

Prove equivalence including checked overflow, conversions, failure conditions,
and any relevant source attribution. Treat floating-point transformations
according to the language's specified semantics. Do not search across arbitrary
effects, allocations, or synchronization without a model proving the rewrite
legal. Cache keys must include semantic and compiler versions and any
target-dependent assumptions. A smaller IR expression is a candidate, not proof
of faster generated code.

## Performance acceptance criteria

Compare each abstraction with equivalent handwritten code on fixed-revision,
clean builds and immutable binaries. Check outputs independently before timing;
separate compile/startup cost from warmed execution and source from saved MIR.
Retain raw repetitions, binary/source hashes, relevant environment settings,
code-size results, and actual allocation counters where claiming allocation
reductions. Interleave cases to reduce order bias.

Include captured scalars, captured managed pointers, known function values,
large callbacks, forwarding helpers, runtime-selected callbacks, and escaping
callbacks. Validate checked arithmetic and source locations, capture snapshots,
evaluation and cleanup order, cancellation, GC safepoints, and live reload.
Measure worker-sensitive cases at 1/2/4/8 workers; do not describe serial
collection processing as parallel scaling merely because workers are enabled.

Structural acceptance for a nonescaping known callback is no indirect callback
dispatch and no environment allocation on the optimized path. For a pipeline
eligible for fusion, additionally require no avoidable intermediate collection
construction. Report runtime equivalence only after matched measurements.

## Reproducing the five probes

Run the two validation commands above to build the current release CLI, then
run this script from the repository root. It writes source and MIR to a new
temporary directory, checks results, and prints the structural counts. The
normal/release distinction below is the language run mode; both use the same
release-built Rust executable.

```python
from pathlib import Path
import os
import re
import subprocess
import tempfile

directory = Path(tempfile.mkdtemp(prefix="badger-closure-audit-"))
cases = {
    "map": ('let k = 3\nlet result = [1, 2, 3, 4].map(fn(x) { x * k })', '[3, 6, 9, 12]'),
    "filter": ('let k = 2\nlet result = [1, 2, 3, 4].filter(fn(x) { x % k == 0 })', '[2, 4]'),
    "fold": ('let k = 3\nlet result = [1, 2, 3, 4].fold(0, fn(a, x) { a + x * k })', '30'),
    "chain": ('let k = 3\nlet result = [1, 2, 3, 4].map(fn(x) { x * k }).filter(fn(x) { x % 2 == 0 }).fold(0, fn(a, x) { a + x })', '18'),
    "parameter": ('let k = 3\nlet result = twice(fn(x) { x * k }, 2)', '18'),
}
environment = {
    key: value for key, value in os.environ.items()
    if not key.startswith(("BADGER_", "LANG_"))
}
for name, (body, expected) in cases.items():
    source = directory / (name + ".bdg")
    source.write_text(
        'import std.io\nimport std.list\n'
        'fn twice(f: fn(Int) -> Int, x: Int) -> Int { f(f(x)) }\n'
        'pub fn main() -> Unit {\n' + body + '\n'
        'let _ = io.println($"{result}")\n}\n'
    )
    for mode, flags in [("normal", []), ("release", ["--release"])]:
        result = subprocess.run(
            ["target/release/badger", "run", *flags, "--dump-mir", str(source)],
            capture_output=True, text=True, env=environment, check=True,
        )
        assert result.stdout == expected + "\n", (name, mode, result.stdout)
        mir = result.stderr
        (directory / (name + "." + mode + ".mir")).write_text(mir)
        main = re.split(r"\nfn ", mir, maxsplit=1)[0]
        print(name, mode, "main Alloc sites:", main.count("Alloc {"),
              "all closure call sites:", mir.count("callee: Closure("),
              "main builder sites:", main.count('name: "badger_rt_i_list_builder"'),
              "main finish sites:", main.count('name: "badger_rt_i_list_finish"'))
print("Sources and MIR:", directory)
```

## Implementation

The release pipeline is now explicit in `badger_mir::opt`: a fixed,
named pass order runs before the MIR is dumped or handed to code
generation, and before it is saved by `badger compile`.

| Level | Passes |
| --- | --- |
| no `--release` | none |
| `--release` (default `release`) | inline, closure specialization, full scalar replacement, simplification, map/snapshot reuse, split receives, local option elision, enum scalarization, simplification, CSE, reachability pruning |
| `--opt aggressive` | the above, then another inlining + scalar-replacement + simplification + CSE round |
| `--opt superopt` | the above, then the verified superoptimizer and a final simplification round |

`--opt superopt` is opt-in; plain `--release` remains the predictable
pipeline. `BADGER_NO_PASS=name[,name...]` disables individual passes,
`BADGER_DUMP_MIR_PASSES=1` prints the MIR after each pass,
`BADGER_TIMINGS=1` reports pass counts, and `BADGER_NO_INLINE=1` keeps its
historical meaning of skipping the whole release pipeline. `badger compile`
records the level in the binary (`mir.opt`); running the file uses that MIR
unchanged, and a higher level named at run time applies only the extra
passes. The level therefore selects the same MIR for a source run, a dump,
and a saved program.

New transformations in `release`:

- **Simplification** (`opt::simplify`): integer constant folding with the
  same defect rules as codegen, arithmetic identities, constant/copy
  propagation into dominated uses, dead-assignment elimination (checked
  operations that may defect are never removed), constant-branch and switch
  folding, unreachable-block clearing, empty-goto threading, and
  single-predecessor block merging.
- **CSE** (`opt::cse`): block-local reuse of identical pure computations.
  Checked operations keep their defect location in the key; stores, calls,
  polls, and reassigned operands invalidate affected entries.
- **Closure specialization** (`opt::specialize`): a helper called with a
  known closure is cloned once per (callee, argument position, lambda) and
  calls through that closure become `Callee::Lambda` — a direct call that
  still passes the closure as its environment. Non-capturing lambdas are
  called with a null environment; when the clone no longer reads the
  closure parameter it is dropped from the clone signature, after which the
  caller's closure object has no non-store use and scalar replacement
  removes it. Unknown or escaping callbacks are unchanged.

Superoptimization (`opt::superopt`, `--opt superopt`) rewrites straight-line
integer/boolean regions with exactly equivalent forms: power-of-two
multiplication, unsigned division and remainder to shifts/masks, shift pairs
to masks, wrapping `(x + c) - c` cancellation, comparison inversion, boolean
comparison normalization, double negation, and canonical operand order for
commutative operations. Boolean rewrites are additionally checked
exhaustively over every assignment of their free boolean variables (≤16
bits) before being applied; arithmetic rewrites are exact two's-complement
identities at the operation width.

Structural checks with a release-built CLI on the five closure probes from
this report (same sources): every mode printed the expected output; all MIR
bodies in `release` contained zero `callee: Closure` sites and zero `Alloc`
sites in the entry function, matching the earlier optimized baseline. A
helper padded past the inliner's size limit (90 statements) and called with
a capturing lambda produced a `…apply_many#1.spec0` clone containing
`callee: Lambda` and no indirect closure call; with a non-capturing lambda
the closure allocation disappeared as well. A wrapping
`wrapping_add(1).wrapping_sub(1)` probe kept its result and lost the
`Sub` (and the `Add`) under `--opt superopt`, while `--release` kept it.
Saving the same program with `compile --opt superopt` and running the file
produced the same output and the same optimized MIR structure.

Validation run: `cargo test -p badger_cli --release --test programs`
(76 passed), plus `option_niche`, `scalar_enums`, `sequence_fusion`,
`packed_arrays`, `adversarial`, `gc_retention`, and the new
`mir_optimization` integration tests (3 passed). This is structural and
behavioural validation, not a benchmark comparison.

### Benchmark comparison

Measured 2026-10-19 on the same host (AMD Ryzen 7 8745HS, 16 logical CPUs,
Linux 7.2.2), with release binaries built from `main` at `d1dac09` and from
this branch at `14c54af` (the branch includes a merge of `d1dac09`).
Runs alternated between the two binaries in both orders; no CPU pinning or
frequency isolation was imposed.

- **Realworld suite** (`bench/realworld`, Badger only): six workloads,
  workers 1 and 4 (workers are only exercised by messaging and pipeline),
  three fresh processes per cell, two warmups plus five samples per process;
  three alternating rounds per binary. Values below are the median of the
  three per-run kernel medians, in milliseconds.

| workload | workers | main | branch | delta |
|---|---:|---:|---:|---:|
| cpu | 1 | 49.38 | 49.24 | −0.3% |
| json | 1 | 4.77 | 4.83 | +1.2% |
| messaging | 1 | 0.92 | 0.93 | +1.6% |
| messaging | 4 | 1.00 | 0.98 | −1.6% |
| pipeline | 1 | 50.23 | 50.41 | +0.4% |
| pipeline | 4 | 13.30 | 13.42 | +0.9% |
| reconcile | 1 | 5.45 | 5.46 | +0.3% |
| state | 1 | 1.12 | 1.18 | +4.8% |

Startup/compilation in that suite increased by 8–15% (60–70 ms range),
consistent with the added MIR passes. Peak RSS was within ±2% of main.

- **Language microbenchmarks** (`bench/baseline.py`): every `bench/*.bdg`
  program at 1/2/4/8 workers, two rounds of three fresh runs per binary.
  Selected results (median run time in ms):

| program | workers | main | branch | delta |
|---|---:|---:|---:|---:|
| runtime | 1 | 26.81 | 19.73 | −26.4% |
| runtime | 2 | 27.17 | 19.45 | −28.4% |
| runtime | 4 | 27.55 | 19.81 | −28.1% |
| runtime | 8 | 28.63 | 21.10 | −26.3% |
| records | 1 | 421.20 | 406.14 | −3.6% |
| collections | 1 | 68.24 | 65.71 | −3.7% |
| heap | 4 | 148.69 | 141.48 | −4.8% |
| parallel | all | ±1% | unchanged | — |
| parallel_alloc | 2 | 28.11 | 26.91 | −4.3% |
| independent_channels | 4 | 2.86 | 2.39 | −16.4% |

The per-operation `runtime.bdg` output explains the win: scalar calls
1.51–1.65 ns → 1.07–1.28 ns (about −30%), call plus inner call 1.74–1.79 ns
→ 1.10–1.19 ns (about −37%), and calls with four rooted pointers 1.62–1.73
ns → 1.15–1.21 ns (about −31%). Recursion timing stayed within its
run-to-run noise. The other microbenchmarks are mostly unchanged; a few
high-worker cells vary by 5–10% in both directions, consistent with the
unpinned host.

- **Compile cost** (`badger compile`, median of six runs): small programs
  +1–10%; `bench/runtime.bdg` 61.95 → 71.06 ms (+14.7%, of which 17.8 ms is
  the MIR pipeline); generated programs of 200 and 800 functions 81.00 →
  92.34 ms (+14.0%) and 262.46 → 312.44 ms (+19.0%, 104 ms MIR pipeline).
  Binary MIR for `bench/records.bdg` shrank from 22,077 to 16,197 bytes.

Interpretation: the release pipeline materially improves repeated direct
calls in `runtime.bdg`, is neutral on the application-shaped realworld
workloads, and costs a modest amount of compile/startup time. The realworld
`state` kernel is 5% slower in this sample; the 1 ms kernel makes it the
least stable cell, but the direction was reproduced across rounds and is not
explained by a known transformation. No claim of Go/Java-competitive
application performance follows from these numbers; the eager list-building
pipelines and persistent-map snapshot work remain the larger structural
costs.
