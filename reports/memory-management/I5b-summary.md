# I5b retained: reuse scalar-map nodes between snapshots

Keep production `49f281c` (runtime core `0cc0d7e`, compiler `f2ae752`, conservative
ABI guard `8ffce23`, static specialization `49f281c`). This completes the narrow
I5b roadmap item; broader I5, arenas, stack/region allocation and G7 remain open.
[Manifest, raw hashes and interval audit](I5b-binaries.json).

## Behavior and proof boundary

Eligible maps still have one static fresh-empty origin, consistent scalar K/V
representations and copy/insert-only producers. Existing private-map I5a groups
keep their cheaper path. The second MIR pass handles eligible local forks and
publications: freeze before uncertain observations/stores/captures/calls/cursors
and returns, and tell every owned insert whether an old local alias is live.
CFG/backedge liveness excludes the call's result destination. Both legacy
transient APIs are rejected; parameters/env/unknown producers and pointer maps
remain persistent. Release/inlining/non-live gates stay unchanged.

An active wrapper uses a globally unique checked 59-bit token. A fork freezes
its original before sharing descendants; frozen/legacy inputs start a new wrapper
and token. Reuse a node only when its tag matches. All reused-node/wrapper write
barriers and the 256-byte insert budget check remain. Freeze never writes an
already-zero shared token, allowing concurrent readers. Exhaustion freezes and
falls back to persistent insertion; tokens never wrap or recur.

Ordinary map payloads remain 16 bytes, internal owned payloads 24; owned cells
can round to 48 rather than 32 bytes. Trie nodes do not grow. Thirty token bits
use spare node flags, 29 explicitly reserved GC client bits. Pointer flags are
masked for ordinary copies/trace; GC masks preserve client bits and payload size
remains high 32 bits. Legacy allocation/reuse clears tags. Old saved MIR works
with the new runtime; new optimized MIR needs the new helper symbols. No GC
policy, mutator/collector publication or finalizer ordering changes.

## Baseline, methodology and controls

The fixed prefetch baseline is `38855c7`; its immutable binary and existing
baselines were saved before these edits. There are **2,744 measured processes in
15 disjoint datasets**, including intermediate/repeat/same-binary/same-runtime
attribution studies. No measured intervals overlap; no outliers are removed.
Own builds/tests finished before timing. The separate official four-language
cohort has 60 performance and 48 validation processes; do not add its missing
absolute child intervals to that audit. All sources, commands, executable hashes,
checksums and raw samples remain.

Ordinary main matrices use seven randomized adjacent process pairs per cell at
workers 1/2/4/8. The application contract is unchanged: three warmups/five measured
kernels, one process median AND one block mean; inner iterations are not independent
samples. Extended repeats use five/fifteen. Extended state pairs use fifteen/fifteen.
Diagnostic counts/phases are perturbed and separate from ordinary timing.

The [initial 908-process matrix](I5b-initial.md) has strong snapshot wins but a
private-map/eight slowdown (+22.6% group/+11.4% paired), reconciliation/eight
+7.1%, and batches/eight +32.8%. It remains intact. The first implementation
passed a dynamic token through the legacy private path. Static zero-sized
Persistent/Builder modes now remove that argument and ownership tests entirely;
only Epoch carries ownership data. [Three native versions](I5b-validation/specialized-assembly.log).

## Allocation reduction and ordinary coverage

The final diagnostic census matches the initial candidate's exact Map/MapNode
counts at both workers one/eight. Across eight inner iterations:

| Workload | Map objects before / after | MapNode objects before / after | Map+node cell bytes before / after | Cell traffic reduction |
|---|---:|---:|---:|---:|
| state | 131080 / 520 | 285200 / 46304 | 131093248 / 15322304 | 88.3% |
| snapshots | 131080 / 2056 | 261888 / 53392 | 71311616 / 12106368 | 83.0% |
| reconcile control | 8 / 8 | 7848 / 7848 | 2835968 / 2835968 | 0% |

State diagnostic collections fall 7→0; snapshots 4→0; reconciliation remains 3.
Cell traffic includes headers/pool rounding, excludes native buffers and unused
pool capacity, and is not live memory. [Exact family proof](I5b-validation/allocation-proof.json),
[final census](I5b-specialized-census/summary.md).

Final ordinary coverage changes (negative is faster):

| Case | Work w1 | w2 | w4 | w8 |
|---|---:|---:|---:|---:|
| scalar-snapshots | -60.4% | -60.5% | -60.5% | -62.1% |
| parallel-scalar-snapshots | -69.3% | -68.4% | -68.3% | -67.5% |
| parallel-maps | +2.9% | -1.6% | -1.4% | -3.4% |
| parallel-snapshots | -0.3% | +1.2% | -2.5% | +1.8% |
| persistent-64 | +1.5% | +0.7% | +0.0% | -2.9% |
| records-window | -2.1% | -1.8% | +0.7% | +0.7% |
| buffers-capacity-32 | -2.7% | -4.7% | -2.2% | -2.6% |

Serial scalar snapshots remove both workload collections and reduce full-process
peak RSS about 27–28%. Eight fixed scalar-snapshot tasks go 16→2 collections,
with GC pause totals 74–84% lower and peak RSS 12–28% lower. Every snapshot
coverage pair is faster. Other cases retain small mixed effects; control RSS is
often 0–4% higher, including frontend/JIT. No universal memory reduction is claimed.
[All 392 final coverage runs](I5b-specialized-coverage/summary.md).

Fixed topology: baseline: 149.774→49.638 ms, 3.02x scaling; candidate: 46.035→16.139 ms, 2.85x scaling.
This is a large throughput improvement, not a claim that scheduling or pointer-graph
worker scaling is solved. Serial cases' worker sweeps measure runtime context.

## Application gains, warmup and remaining regressions

[All 504 final application processes](I5b-specialized-applications/summary.md):
state group medians improve 42.4/49.8/45.5/49.8%; paired medians 44.2/48.1/47.9/50.3%.
Compare snapshots improve 67.5/69.5/69.9/70.2% group; all seven pairs are faster
at each worker count for BOTH workloads. Block means agree: state about 50–57%
and snapshots 68–69% faster. CPU/pipeline are roughly flat; other controls vary.

[Fresh 15-warmup/15-sample state pairs](I5b-state-longwarm/summary.md) show 5.478→
1.096 ms at one worker (-80.0% group/-79.6% paired), 5.001→1.104 ms at eight
(-77.9%/-77.9%). All seven pairs are faster; block means improve 75–76%, RSS
-23.4%. Keep BOTH warmup cohorts: reduced allocation changes the collection/pool
regime, and warming affects Badger too. The cause of its warmup shift is not
isolated by these timings. Do not describe it as a Badger JIT optimization.

[Fresh longer-warm four-language check](I5b-realworld/summary.md): state is Badger
1.113 ms, Go 2.217, Node 2.316, Java 1.168. Badger state CV 11.6% makes the small
Java difference unreliable. This workload is competitive; reconciliation (Badger
5.406 vs Java 1.462 ms) and JSON (18.386 vs Node 6.927/Java 11.132) still have large
gaps. No general language rank or JVM/V8 convergence is established.

The final short matrix retains batches/eight +55.4% group/+58.9% paired, JSON/eight
+4.3%/+4.5%, and messaging/two +6.0%/+17.9%. They are not omitted. The
[15-pair followup](I5b-control-followup/summary.md) shrinks JSON/eight to +1.0% group/
+0.1% paired, comparable to its [same-binary control](I5b-control-followup-noise/summary.md)
(+0.1%/-0.4%). Batches/eight is +7.2%/+3.6% median but +4.6%/-0.4% block means;
its earlier [15-pair repeat](I5b-app-repeat/summary.md) is -0.5%/-4.3% median.
Keep the wide ranges and +12.9% followup RSS; these are not universal no-regression
results. Reconcile/eight repeat is +2.0%/+1.6% median and +3.3%/+2.2% block means;
[its same-binary control](I5b-app-control/summary.md) is -2.4%/+0.2% and +2.1%/+0.7%.

Messaging/two retains a possible tradeoff: +5.6% group/+7.0% paired median and
+6.4%/+3.2% block means, only 3/15 median pairs faster, versus near-zero paired
[same-binary control](I5b-control-followup-noise/summary.md). A separate
[same-runtime old/new saved-MIR study](I5b-messaging-mir/summary.md) gives +1.0%/
+0.4% medians, while [identical new-MIR control](I5b-messaging-mir-control/summary.md)
is +1.3%/+1.2%. It does not reproduce the full source-run shift, but also does
not prove absence or isolate native layout/frontend heap-state effects. Keep this
as a followup; don't blame or exonerate GC/scheduler from aggregate times alone.
These attribution studies load saved programs and do not establish source startup
or ordinary source/native optimization deltas.

Private no-GC controls: [15-pair repeat](I5b-private-repeat/summary.md) is near flat
for single-task private maps; fixed tasks/eight +5.0% group/+5.6% paired has very
wide ranges. Its [same-binary control](I5b-private-control/summary.md) has +11.1%
group at eight, and the main final matrix is -3.4%/-6.4%. Small code/layout tradeoffs
remain possible; the initial large slowdown is not repeated consistently after
specialization. Do not attribute every noisy variation to the optimizer.

## Validation and next work

Final 11 ordinary/13 diagnostic runtime tests, 14 MIR tests, strict four-crate
Clippy in both modes and formatting pass. Focused final checks: 136 source/old/new
saved-MIR GC runs, 28 MIR probes, 36 generated differential cases (repeated with
compatible compilers before and after I5a), 72 native-container
checks, 16 allocation conservation checks plus ordinary absence, and 23 harness
tests. Cover every retained key/value, intervals 1/2/64/256, negative/packed/float
values, forks updated on both sides, hidden stores/captures/cursors, task handoff,
512 full-hash collision entries, high token bits, exhaustion, barriers, both GC
modes, reuse/quarantine and workers 1/2/4/8 with stress/chaos.

The initial integration program suite passes all 76 tests. The specialized full
suite passes 75 and fails one corpus due to cancel_matrix's `some completed`
race output changing from true to false; the entire failed saved-MIR corpus then
passes its focused rerun. Both captures remain. The first sandboxed attempt also
fails local-network/debugger permissions; its permitted integration rerun passes.
No test oracle was weakened to hide those failures. [Logs/artifacts](I5b-validation/).

Retain this bounded allocation win, with the mixed controls explicitly recorded.
Next profile the remaining record/JSON and generic scalar-map read costs, and
isolate the two-worker messaging shift with native/source layout controls.
Pointer-map tracing/scaling, broader parameter/interprocedural ownership proofs,
packed records and task/transfer arenas remain open; this is not GC bypass.

Followup on `b080078`: [fresh source/native controls](W1-followup.md) reproduce
the two-worker source slowdown (+13.3% paired median/+11.1% block mean), while
identical old MIR across native binaries is -3.2%/-2.8% versus a near-zero control.
The source/JIT/layout question remains open. Fixed-eight-consumer coverage now
separates task count from runtime workers; its profiles identify growing channel
spin and worker wake/park CPU in the cheap messaging shape. This does not revise
the historical I5b measurements or attribute the source regression to one cause.
