# G7a: known-owner small-cell reclamation

## Decision and mechanism

Keep the bounded reclamation-path simplification at `69b3a61`, with narrow
measured benefits and mixed application results. This is not a general GC or
application speedup, and G7 remains open for larger collector improvements.

Normal sweeps already know each object's worker owner. Remove the per-cell
verification-mode test and quarantine-owner discovery code from that path;
quarantine draining alone finds the owning pool. Large cells still use the
system allocator. Pool sizes/refill, GC thresholds, zeroing, root publication,
object/header layout and finalizer ordering are unchanged. No arena or allocation
elimination is implemented here. Native symbol inspection confirms the smaller
return path inlines, while baseline keeps an out-of-line `free_cell` function.
Release LLVM already removes finalizer-guard stores; deleting those is not a win.

A cross-worker regression checks that quarantined pooled cells return to worker
7 rather than the coordinator, large system cells free without a pool owner,
zeroed cells still reuse correctly and empty chunks retire. Existing pool-policy
and quarantine variants exercise the same fixture.

## Baseline and coverage

Production baseline is `fdf36dc`; `cbb799d` adds only harnesses/coverage. All
242 baseline measurements were saved and committed at `1fe2d02` before collector
edits. The ordinary candidate is pinned to `69b3a61`. Full evidence contains
**1762 runs in 12 datasets**, no overlapping measured intervals and no
outlier removal. [Baseline](G7-baseline.md), [hashes/commands/audit](G7-binaries.json).

Seven shuffled adjacent pairs per 1/2/4/8-worker coverage cell:

| Case | Work change, w1 | w2 | w4 | w8 | GC pause change, w1 / w8 |
|---|---:|---:|---:|---:|---:|
| buffers-grow | +1.4% | +1.3% | +1.7% | +1.7% | +7.0% / +2.5% |
| bytes-transient | -6.3% | -10.6% | -4.2% | -3.5% | -5.0% / -1.3% |
| parallel-maps | -0.3% | -1.6% | -3.9% | -5.8% | +2.4% / -8.6% |
| parallel-snapshots | -1.9% | -2.9% | -3.2% | +2.7% | -3.1% / +6.3% |
| persistent-4096-major | -6.8% | -6.1% | -4.1% | -6.0% | -12.0% / -16.2% |
| persistent-64-major | +2.5% | +2.3% | -2.9% | -2.3% | +6.7% / -3.1% |
| records-window | +2.0% | +1.2% | +2.3% | +2.1% | -0.9% / +2.6% |
| snapshots-major | -3.4% | -5.7% | -1.3% | -3.3% | -6.0% / -3.3% |

Wide persistent-map updates improve about 4–7% elapsed and 12–20% GC pause,
while heap-shaped record churn is essentially flat/slower (about +1–2% work).
Smaller maps, buffers and parallel snapshots remain mixed. These controls prevent
claiming that a simpler free path universally speeds allocation-heavy programs.
Peak RSS mostly moves slightly upward; no allocation reduction or broad memory
win is expected from this change. [All medians, ranges and pairs](G7-coverage/summary.md).

## Applications, controls and follow-up

The full 504-process comparison reuses unchanged realworld CPU/reconcile/JSON/
state/messaging/pipeline and compare records/snapshots/batches contracts at all
four worker settings. It includes three warmups and five measured iterations
per fresh process, using one warm median per process. Independent suite oracles
check every iteration. Native child-only RSS excludes Python parent inheritance.

Results are mixed: JSON and snapshots often improve a little; records and
reconcile vary by worker and repetition. Messaging/four initially shows +27.0%
group-median work (+25.2% median paired ratio), prompting a fresh 15-pair repeat
and same-binary controls rather than discarding the result.

The follow-up reduces that messaging/four difference to +5.4% group/+2.5%
paired (only 5/15 candidate pairs faster); a slowdown remains in this experiment.
One-worker state changes -3.1% group/-3.4% paired (13/15 faster), while four-worker
state changes +2.2%/+1.3% (5/15 faster). Records/eight +2.5%/+1.4%, with 7/15
faster. CPU controls stay around flat. Same-binary controls themselves show
records/four +6.1%, records/eight +6.5% and state/four +5.8%, with wide paired
ranges. These observations do not establish that every small change is causal.
Keep the narrow wide-map benefit without hiding the application tradeoffs.

[Full application comparison](G7-applications/summary.md),
[15-pair confirmation](G7-app-followup/summary.md),
[application self-control](G7-app-self-control/summary.md),
[coverage self-control](G7-self-control/summary.md).

## GC attribution limits

Separate diagnostic probes across all eight inner application iterations:

| Workload | Workers | Base / candidate process GC ms | Base / candidate collections |
|---|---:|---:|---:|
| messaging | 1 | 0.00 / 0.00 | 0 / 0 |
| messaging | 8 | 0.00 / 0.00 | 0 / 0 |
| reconcile | 1 | 53.58 / 53.73 | 28 / 28 |
| reconcile | 8 | 56.82 / 53.32 | 28 / 28 |
| state | 1 | 14.51 / 13.67 | 9 / 9 |
| state | 8 | 13.18 / 13.71 | 9 / 9 |

Messaging performs zero collections in these probes. Its timing differences
cannot be attributed to executing the modified sweep path; scheduling, binary
layout and host variation remain possible explanations, not proven causes.
State/reconcile do collect, but even removing their measured collection cost
would leave much of the current cross-language gap. These one-process probes
include warmups/startup and are not independent statistical timing comparisons.
The phase-only record-window probe preserves the supplied profile's roughly
85% sweep share but does not show a stable sweep improvement on this workload.
A phase share within GC is not the same as its share of total program time.

The diagnostic allocation census retains full conservation checks before/after.
This patch does not change allocation sites or claim fewer objects. No general
RSS, tail latency or worker-scaling improvement is established. The existing
CPU pipeline's scaling and serialized workload stages require separate analysis.

## Validation and next work

Ten ordinary and twelve diagnostic runtime tests, all 76 program tests, 88
source/saved-MIR ABI/root/GC stress checks plus two MIR probes, 20 Python harness
tests, strict all-target runtime Clippy in both modes and scoped Rust formatting
pass. [Logs, ownership test/native symbols and hashes](G7-validation/).

Next remove the immediate `Map.entries()` list/tuple materialization using the
existing map cursor. Direct `map.to_list(m)` loops already elide it, while the
method form misses that lowering. Preserve retained versions, stored entry lists,
receiver evaluation, iteration order and pointer lifetimes. The broader map
implementation already uses CHAMP nodes; copy-on-write transient epochs, compact
node representations, stack/region proofs and owned-message regions remain
separate unimplemented items. Avoid shrinking arenas merely to scan less: the
collector already limits minor sweeps to young suffixes, and earlier small-chunk
experiments lost throughput.
