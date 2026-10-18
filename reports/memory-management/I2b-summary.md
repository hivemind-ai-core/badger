# I2b: elide immediate Map.entries() lists and tuples

## Decision and behavior

Keep `67d3f73`. Immediate standard `Map.entries()` loops now use the existing
map cursor, as direct `map.to_list(m)` loops already do. This removes materialized
entry lists and, for destructuring patterns, their tuple objects. The receiver
is evaluated once and cursor order matches materialization. Tuples bound as
values still allocate if they escape; stored entry lists remain ordinary lists.
User methods named `entries` retain their behavior: resolution uses the actual
standard-library definition, not the method spelling. New elision requires the
existing optimized/non-live/inlining-enabled lowering. Debug, live dispatch and
`BADGER_NO_INLINE` keep the method call. Existing global to_list lowering stays.

This changes one compiler file. Runtime/layout/GC policy are unchanged from the
G7a baseline. No new runtime API or MIR format is required (still format 6).
Actual baseline/new saved programs run on both runtime versions in focused tests.

## Baselines and final coverage

Production baseline `69b3a61` includes G7a. The fixture/coverage-only change is
`b1335db`; all 104 baselines were committed at `c1d25b3` before compiler edits.
Both ordinary variants include G7a, so its effects are not attributed to I2b.
There are **952 measured processes in 9 datasets**, with no overlapping
measured intervals or removed outliers. [Baseline](I2b-baseline.md),
[exact binaries, source hashes, commands and audit](I2b-binaries.json).

Seven adjacent shuffled pairs at each worker setting; negative is faster:

| Case | Work w1 | w2 | w4 | w8 |
|---|---:|---:|---:|---:|
| entries-immediate | -68.6% | -56.4% | -54.4% | -55.4% |
| entries-stored | -1.3% | -1.7% | -1.1% | +0.1% |
| parallel-maps | -0.4% | -0.3% | +4.4% | -1.4% |
| records-window | +1.5% | +1.1% | -2.5% | -2.2% |
| snapshots-major | +2.0% | -0.1% | +0.3% | -0.2% |

Immediate iteration improves 55–69% while stored lists stay roughly flat.
Already-elided snapshots and other GC controls remain mixed: do not claim a
new global to_list/compare-snapshots optimization. These are single-task loops;
worker settings are runtime context, not parallel iteration scaling.
[Every work/GC/RSS median and paired range](I2b-coverage/summary.md),
[same-binary controls](I2b-self-control/summary.md).

## Allocation proof

At one and eight workers the diagnostic census removes exactly **524,288 Tuple
objects (16 MiB of tuple cell traffic)** in immediate iteration, plus 2,560 Pages,
2,048 PageDirectories and 512 Lists. It introduces 512 MapCursors. Net across
those families: **528,896 fewer managed allocations and 21.40625 MiB fewer cell
bytes**. Setup MapNode counts/bytes are identical. Stored controls retain all
Tuple/Page/PageDirectory/List/MapNode counts and bytes. Tiny startup/output string
differences are excluded from exact family-level claims. Native scratch vectors
inside old map materialization are not included in these managed-family counts.
These are allocation traffic figures, not live-memory/RSS savings.

Realworld state across eight inner iterations removes **520,192 tuples** plus
list storage, replacing 520 entry lists with 520 cursors. MapNode traffic stays
285,200 objects / 126,898,688 bytes per process. Reconcile removes only 8,192
tuples and eight lists, while MapNode traffic stays **897,160 objects /
402,771,648 bytes**. Counts include warmups and output; diagnostic speed is not
ordinary timing. [Full before/after family proof](I2b-validation/allocation-proof.json).

## Application results

Seven fresh processes per variant/cell, each with three warmups and five measured
iterations; one warm median per process. All independently checked. State shows
all seven candidate processes faster in every worker cell:

| Workers | State baseline ms | Candidate ms | Work change | Peak RSS change |
|---:|---:|---:|---:|---:|
| 1 | 9.392 | 4.163 | -55.7% | -20.4% |
| 2 | 9.069 | 4.083 | -55.0% | -21.0% |
| 4 | 8.596 | 4.165 | -51.5% | -20.8% |
| 8 | 8.703 | 4.472 | -48.6% | -20.6% |

This is a serial state/snapshot workload: its worker sweep does not demonstrate
parallel speedup. The unchanged realworld workload improves about 2x by avoiding
intermediates. It does not establish parity with other languages on a new quiet
host, and no JVM steady-state comparison or general tail-latency win is claimed.

The other applications remain mixed. Reconcile changes about -2.9%/+0.2%/+6.3%/
+1.1% at 1/2/4/8 workers; paired medians differ and ranges are broad. Its small
entry-list allocation reduction cannot remove the dominant persistent-node
traffic. Compare snapshots were already directly lowered through global
`map.to_list`; their noisy changes are controls, not a claimed new allocation
win. CPU/messaging/pipeline and records likewise retain every result rather
than assuming small movements are causal. [All application cells, RSS and pair
ranges](I2b-applications/summary.md). Ordinary kernel timing excludes frontend/JIT;
process peak RSS includes them. No own compilation/tests overlapped measurements.

## Correctness and next opportunity

All 76 program tests, six MIR tests, 72 focused source/saved-MIR GC stress checks
and eight optimized/debug/no-inline MIR probes pass, alongside 21 harness tests,
strict all-target MIR Clippy in ordinary/diagnostic configurations and scoped
formatting. Focused checks include true freeing/quarantine, generations on/off,
workers 1/4/8, scheduler perturbation, ephemeral cursor-only graphs, retained old
versions, receiver side effects, exact order, stored entry lists, escaping tuples,
empty loops and early break. [Validation logs and captures](I2b-validation/),
[replay checker](../../bench/gc/check_map_entries.py).

Next investigate private repeated Map updates: prove the map starts empty,
never exposes an earlier version, and has no live old aliases at an update.
Then reuse the existing transient insertion path. Snapshot-producing code must
keep persistent copies. This directly targets reconcile's remaining node
traffic, while broader pointer-bearing ownership, stack/region allocation,
collector batching and worker-scale attribution remain open.
