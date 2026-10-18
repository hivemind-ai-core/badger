# I5a: reuse proven private scalar maps

## Decision and scope

Keep `fc759e3`. Optimized/non-live MIR after inlining recognizes fresh scalar
maps whose old versions cannot be observed, and routes their inserts through
the existing transient HAMT path. Source Map semantics remain immutable. The map
and trie are still managed objects; this avoids creating most intermediate
objects, rather than adding an arena or bypassing tracing for live values.

The proof joins plain copies and insert input/results. Every producer must be
one static fresh empty origin, a group copy or a matching insert. Parameters,
environments, other producers, merged origins and pointer key/value types
conservatively retain persistent operations. All inserts in the group must pass;
a partially converted persistent fork could share descendants and is rejected.
Backward CFG liveness finds any old alias surviving each call (excluding its
result destination). Scalar get/get_split/contains/len/index reads are allowed.
Other observations, stores, closure captures, unknown calls and cursor creation
freeze the map if a later update is reachable, including through loop backedges.
The final map can be returned/shared once construction is over. [Proof plan](I5a-proof-plan.md).

The new helper preserves the ordinary 256-byte budget check, returns the same
wrapper and uses existing roots, descendant-private edits and old-to-young
barriers. Loop polls/checks, allocation policy and global synchronization stay.
MIR format remains 6; old saved programs run on the candidate. New optimized
programs need the new `badger_rt_i_map_insert_private` runtime binding. Debug,
live and no-inline builds keep ordinary insertion. No general pointer-bearing
ownership, mutable-map API, region or task-arena implementation is claimed.

## Baseline and evidence

Production baseline `67d3f73` already includes G7a and I2b. Before any I5a changes,
132 measurements were saved/committed at `9e47ccc`; coverage-only `c518383`
added full scalar snapshot/alias checks. Final GC/application evidence has
**1116 measured processes in 10 datasets**, with no overlapping measured
intervals and no removed outliers. Separately, official four-language studies add
240/12 performance and 192/16 small-validation processes in short/long-warm
cohorts. Their runner is sequential but lacks the same per-child absolute UTC
audit fields. No own builds/tests overlap accepted measurements.
[Baseline](I5a-baseline.md), [binaries/sources/commands/audit](I5a-binaries.json).

## Ordinary coverage

Seven randomized adjacent pairs per cell; negative is faster:

| Case | Work w1 | w2 | w4 | w8 |
|---|---:|---:|---:|---:|
| entries-stored | +1.3% | -1.6% | +1.5% | -0.6% |
| parallel-maps | -84.9% | -88.2% | -90.1% | -89.5% |
| persistent-4096-major | -82.5% | -81.1% | -82.8% | -81.9% |
| persistent-64-major | -73.6% | -74.8% | -73.7% | -74.5% |
| records-window | -0.5% | +2.1% | -1.7% | -0.4% |
| scalar-snapshots | -1.5% | -0.1% | +2.6% | +7.6% |

Private 64/4,096-key source-form loops improve about 74%/82%, eliminating their
14/23 workload collections (now zero). The scalar-snapshot control retains every
old version/key/value and its two collections. Stored entries and record churn
are controls, not newly optimized cases. [All GC/RSS values and paired ranges](I5a-coverage/summary.md).

Snapshot/eight initially measures +7.6% group-median work. The separate 15-pair
repeat is +2.2% group / -0.3% paired (8/15 faster), with wide ranges; one-worker
repeat -0.4% group / +2.8% paired. Do not claim either a true-snapshot speedup
or a uniformly flat control from these estimates. [Snapshot repeat](I5a-snapshot-followup/summary.md),
[same-binary controls](I5a-self-control/summary.md).

## Realworld reconciliation and allocation proof

The unchanged reconcile application drops from about 16 ms to 4.4–4.6 ms:
**71–73% less elapsed work at 1/2/4/8 workers**, all seven pairs faster in every
cell. Process peak RSS falls about 18–19%. This is serial reconciliation; its
worker sweep measures runtime context, not parallel processing.

Separate diagnostic process census, across eight inner iterations:

| Family | Baseline objects | Candidate objects | Baseline cell bytes | Candidate cell bytes |
|---|---:|---:|---:|---:|
| Map | 411,432 | 8 | 13,165,824 | 256 |
| MapNode | 897,160 | 7,848 | 402,771,648 | 2,835,712 |

This removes 1,300,736 managed map/wrapper allocations and about 394 MiB of cell
traffic per process; collections change 28→3 in these diagnostic probes. Native
buffer traffic and the remaining Order/list allocations are outside this exact
family reduction. Allocation traffic is not live heap or an equal RSS reduction.

The full private 64-key coverage case changes MapNode allocations 999,422→70
and Map wrapper allocations 524,291→3 (two remaining setup wrappers). True scalar
snapshots retain exact Map/MapNode counts and bytes. Realworld state likewise
retains 131,080 Maps / 285,200 MapNodes and seven diagnostic process collections.
[Exact before/after family proof](I5a-validation/allocation-proof.json).

Other applications remain mixed: state work +0.7–5.2%, compare-snapshot changes
vary by estimator, and small messaging/four work +9.2% (+13.8% paired) remains
noisy. No broad latency or no-regression claim is made. CPU/pipeline stay roughly
flat; keep every result. [Complete application matrix](I5a-applications/summary.md).

## Fixed-task worker scaling

Eight independent map tasks, fixed topology/input at every worker count:

| Workers | Baseline ms | Candidate ms |
|---:|---:|---:|
| 1 | 201.035 | 30.351 |
| 2 | 139.943 | 16.489 |
| 4 | 93.027 | 9.235 |
| 8 | 65.667 | 6.899 |

Candidate speedup from one to eight workers is 4.4x (baseline 3.1x); eight-worker
elapsed improves about 89.5% and peak RSS about 50.9%, with workload collections
62→0. This demonstrates improved scaling on independent allocation-heavy work,
not on serial loops or actor bottlenecks, and not linear scaling for every task.

## Fresh cross-language check and remaining gaps

A new unchanged four-language suite passes all 48 cells, 192 small validations
and 1,920 full-size iteration outputs, with source/tool/executable/JVM/Gson hashes.
Observed reconcile medians: Badger 4.561 ms, Go 3.056 ms, Node 8.548 ms,
Java 2.678 ms (Java process CV 14.6%). Badger closes much of the prior gap but
still trails Go/Java here. State remains 4.553 ms versus Go 2.081, Node 3.317
(CV 18.2%) and Java 2.394. Eight-consumer pipeline: Badger 7.903 ms, Go 9.795,
Node 10.734, Java 6.993; consumer count is not equal total runtime-thread budget.

JSON exposes a coverage issue: short Java runs have not converged despite low
CV across process medians. A separate 15-warmup/15-sample cohort measures Badger
17.516 ms, Go 14.625, Node 6.594, Java 10.460. Badger's process CV there is 11.8%;
keep both studies, do not cherry-pick warm windows or assert JVM steady state.
[Four-language tables, raw cohorts, commands and limits](I5a-realworld/summary.md).
This slice does not establish universal competitiveness or complete the roadmap.

## Validation and next collector target

All 76 program tests, 12 MIR tests (including six new multi-case proof tests),
10 ordinary/12 diagnostic runtime tests, 72 source/saved-MIR GC checks (including
4,096-key deep tries), 28 MIR mode/eligibility probes and 36 independent generated
Python-map differential cases pass. Actual freeing/quarantine, both generations,
workers 1/4/8, scheduler perturbation, packed/float values, live/hidden/captured
aliases, forks, old cursors, map parameters, pointer fallbacks and returned-map
handoff are covered. Twenty-two Python harness tests, scoped formatting and strict
all-target runtime/MIR/codegen/driver Clippy in both modes pass.
[Logs, MIR captures and replay data](I5a-validation/),
[replay checker](../../bench/gc/check_private_maps.py).

Next measure a finalizer-presence bit in spare Header.meta bits, with descriptor
shape-cache support, to skip descriptor/kind lookup for dead objects requiring no
finalizer. Preserve callbacks/order, lazy descriptor backpatching, quarantine and
old format-6 compatibility. This is a proposed next experiment, not implemented.
True snapshot path copying (e.g. ownership epochs), pointer-bearing map proofs,
packed record storage, JSON DOM/native allocation and task/message regions remain
open; fresh profiles should guide their order.
