# G7d retained: prefetch object headers during sweep

Keep `38855c7`: x86-64 sweep requests the header 32 objects ahead, while retaining
all existing marking, compaction, finalization, pool ownership and freeing order.
Bounds checks skip the tail; future entries have not been compacted or freed
while the world is stopped. Other architectures retain the original sweep path.
No managed allocation, header layout, roots, budgets or collection policy changes.

## Baseline, isolation and attribution

The immutable baseline is `fc759e3` (G7a, I2b and I5a). A fresh 30-process baseline
was saved before edits, after reverting both finalizer-cache prototypes. The
final evidence contains **1,382 measured processes across nine datasets**, with
no measured interval overlap and no removed outliers. Source/executable hashes,
commands, every sample and pressure context remain. Own builds/tests did not
overlap accepted timing. [Baseline](G7d-baseline.md), [manifest/audit](G7d-binaries.json).

The [200-process diagnostic distance study](G7d-distances/summary.md) compares
0/16/32/64-object lookahead in ONE executable, with grouped shuffled variants.
This controls executable layout; zero disables hints. The default 32 was chosen
before the study. Workload-only sweep phases fall substantially for retained-map
cases, while dense records/native buffers benefit less. Other distances and all
unfavorable observations remain; diagnostic timings are not ordinary speed claims.
This supports memory-latency sensitivity as an inference, not a PMU cache-miss
measurement. [Native hint/bounds inspection](G7d-validation/prefetch-assembly.log).

## Ordinary coverage and applications

Seven adjacent randomized pairs per cell, workers 1/2/4/8:

| Case | Work w1 | w2 | w4 | w8 |
|---|---:|---:|---:|---:|
| Scalar snapshots | -16.5% | -12.0% | -6.3% | -10.8% |
| Pointer snapshots, major coverage | -16.7% | -10.8% | -11.4% | -14.0% |
| Record window | -2.6% | -2.3% | -1.9% | -1.2% |
| Fixed eight-task pointer snapshots | -14.0% | -9.3% | -1.7% | +1.3% |

Both retained-map cases have about 40–49% lower ordinary workload GC pause
totals. Buffer-pointer initial work +1.6–4.7% and no-GC private-map controls are
also retained; no universal improvement or better worker scaling is claimed.
[All 336 coverage runs, ranges, GC and RSS](G7d-coverage/summary.md).

The [504-process unchanged application matrix](G7d-applications/summary.md) has
realworld state work -15.2/-14.9/-21.7/-11.9% by worker count; paired estimates
-13.6/-17.5/-17.2/-20.5%, 7/7 faster at workers 1/2/4 and 6/7 at eight.
Compare snapshots group work -12.1/-17.0/-2.4/-10.0%; paired -8.2/-13.9/-9.8/-13.4%.
Warm-kernel block means agree on direction: state -11.9–21.0%, snapshots -3.3–15.1%.
CPU/pipeline stay roughly flat; reconciliation/JSON/messaging and other short
cells remain mixed. No new four-language rank is inferred from these paired runs.

## Regression probes and benchmark coverage

Initial compare-records/four +14.3% group/+16.8% paired shrinks to +6.6%/+7.8%
when using each process's mean measured kernel instead of its median. A separate
[15-pair, 5-warmup/15-sample repeat](G7d-record-followup/summary.md) gives +0.7%
group/+0.4% paired median work and +0.2%/-1.2% block means. Its [same-binary
control](G7d-record-control/summary.md) is +1.5%/+0.3% medians and -1.1%/-0.7%
block means. Preserve the initial slowdown; do not silently discard it.

The [15-pair buffer/private-map repeat](G7d-buffer-followup/summary.md) has buffer
work -0.8%/-0.4% group and -0.5%/-0.4% paired at workers 1/8. Its [same-binary
control](G7d-buffer-control/summary.md) is +0.4%/-1.4% group and -0.2%/+0.1% paired.
The no-GC private-map/eight repeat is +1.2% group/+0.8% paired versus control
-1.5%/-1.7%, with broad ranges. Small layout/noise tradeoffs are not ruled out.

The harness now reports BOTH per-process warm-kernel medians and block means,
retaining costly GC iterations. Inner iterations are never independent samples.
A tail-reversal regression check covers this distinction. The new analyzer
recomputes both from old raw runs without rewriting their bytes:

```sh
python3 bench/gc/analyze_applications.py reports/memory-management/G7d-applications
```

## Allocation identity, validation and next target

Separate diagnostic censuses retain exact Map/MapNode counts and cell bytes:
state 131,080 wrappers / 285,200 nodes; compare snapshots 131,080 / 261,888 across
eight inner iterations. Selected record families also match; only formatted
String traffic differs in one record probe. This optimizes collection, not
allocation volume or live memory. [Family proof](G7d-validation/allocation-proof.json).

All 76 program tests, 10 ordinary/12 diagnostic runtime tests, 72 focused
source/old/new saved-MIR stress checks, 16 diagnostic allocation conservation
checks plus ordinary-profile-absence probe, 22 harness tests, strict runtime
Clippy in both modes and scoped formatting pass. Real reuse/quarantine, both GC
generation modes, workers 1/2/4/8, chaos, native finalizers and coordinator order
are covered. [Logs/captures](G7d-validation/). No request p99 or non-x86 speedup
is measured.

The eight-task pointer-snapshot diagnostic shifts attention to tracing:
without hints, median workload trace/sweep totals are 70.2/23.2 ms at eight
workers; with 32-object hints, 69.3/17.8 ms. At one worker they are 25.7/75.5→
26.7/39.7 ms. Collection counts differ slightly with scheduling (32–33 versus
37), so this does not prove per-object trace cost or lock contention. Stop-world
rendezvous is much smaller in these probes. Trace work/cardinality and batched
pointer-header prefetching are next hypotheses; all queues must drain before
weak-page clearing, verification or sweep. Reducing garbage can reduce how often
large live graphs are traced too. [Snapshot ownership proposal](I5b-proof-plan.md).

Parent G7 and the wider roadmap remain open. G7b/G7c classification prototypes
were reverted; the retained optimization here is the bounded sweep hint.
