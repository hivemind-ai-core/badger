# G7b experiment: per-object finalizer bit

Do not publish the production object-header implementation `957ff40`. It is
correct in the exercised checks and sometimes reduces collection work, but adds
classification work on every allocation and has mixed application results. Keep
all evidence and the reusable source/saved-MIR checker; try descriptor-only
classification caching next. This is not a general collector speedup claim.

The prototype uses spare bit 3 of the existing 16-byte object header. It shares
lazy classification with the descriptor leaf cache, preserving descriptor
backpatch resets. Serial/parallel sweep skips kind lookup for nonfinalizers;
callbacks still run in the original coordinator order. Freezing inline buffers
must clear the flag when switching to their no-finalizer descriptor. No GC
thresholds, pool layouts, roots or allocator clearing policies change.

## Evidence

The baseline was saved before edits. There are **972 measured processes in six
datasets**, with no overlapping measured intervals and no removed outliers.
Immutable normal/diagnostic binaries, production/source hashes, all commands and
samples are retained in [the manifest](G7b-binaries.json). Diagnostic observations
are separate from normal timings. Builds/tests do not overlap accepted timing.

[Coverage](G7b-coverage/summary.md): seven adjacent randomized pairs for six
cases at workers 1/2/4/8. Retained scalar snapshots have lower GC pauses, while
record churn and native buffers are largely small/noisy effects. The private-map
control performs zero workload collections and still pays the added allocation
classification cost. No allocation elimination or memory-footprint win is claimed.

[Applications](G7b-applications/summary.md): five pairs for all nine contracts at
workers 1/2/4/8. Compare records vary -0.3% to -5.1% group medians. Initial
one-worker state is +13.8% group/+17.6% paired, snapshots +9.8%/+10.1%, and
batches +3.7%/+4.7%. Other workloads are mixed; preserve every cell.

A [15-pair repeat](G7b-followup/summary.md) gives state +2.7% group/+6.6% paired
(5/15 faster), snapshots -11.7%/-11.4% (15/15 faster), batches -1.0%/-1.9%.
[Same-binary controls](G7b-self-control/summary.md) show state -0.4% group/-0.7%
paired, snapshots -3.7%/-7.2% (9/15 faster), batches +0.8%/+0.6%. The snapshot
reversal demonstrates why five noisy short-process pairs are insufficient alone;
the state repeat still gives reason to avoid the per-allocation tradeoff.

[Separate diagnostic phase pairs](G7b-phases/summary.md) show compare snapshots
sweep -16.9% to -20.6%, records about -8% to -9%, but one-worker state +6.5%
and JSON +10% to +23%. These are whole-process phase totals across eight inner
iterations, including warmups/output, not normal-throughput measurements.
Batches perform zero collections. Do not use isolated phase savings to dismiss
application regressions.

## Validation

Before reverting the prototype: all 76 program tests, 11 normal/13 diagnostic
runtime tests, 72 focused source/old-saved-MIR/new-saved-MIR checks, 16 diagnostic
allocation conservation/stress checks plus the ordinary-profile-absence probe,
22 Python harness tests, strict runtime Clippy in both modes and scoped format
pass. Saved-MIR checks cover buffer freezing, packed/pointer data, old containers,
workers 1/8, generational/non-generational GC, real reuse/quarantine and chaos.
The old-container matrix retains its assertions with churn bounded to eight
iterations under forced GC; the full original fixture also passes in the full
program suite. [Logs and captures](G7b-validation/),
[checker](../../bench/gc/check_finalizers.py).

Next evaluate using the already cached descriptor classification during sweep,
without stamping every allocation or changing buffer transitions. This preserves
G7b as a negative/mixed experiment, not a landed optimization.
