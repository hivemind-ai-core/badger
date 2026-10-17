# G6a: completed-task input roots

## Decision

Keep `82642bb`: entry closures and explicit arguments stop rooting their input
graphs once a task is Done, while the scope can retain its record and result.
In the new scoped-input benchmark, native payload capacity after churn falls
from exactly 16 MiB to zero at 1/2/4/8 workers in every repetition. Returned
aliases retain their full 16 MiB payload and remain readable through later
collections. This is an earlier-reclamation improvement, not a broad speedup.
The short retention workload has timing regressions, documented below.

G6a is complete. G6's compiler shadow-root/provenance work, task-record reuse,
native argument-vector reuse, regions, rings and escape inference remain open.

## Lifetime and publication

The collector reads existing acquire/release task state and omits only entry
closure/argument roots for terminal Done records. Both completion paths already
publish Done after body execution and cleanup; the lock-free path releases its
coroutine first. Results are published before Done and retain existing tracing.
All other task roots, cancellation/failure handling, cleanup roots, root frames,
write barriers and scheduling/collection policy are unchanged.

Entry/argument fields remain unchanged: no concurrent writes are introduced for
inspection or GC to race. They can contain stale words after Done. Task memory
inspection therefore checks terminal state before dereferencing entry metadata.
The runtime lock prevents collection between inspecting a still-live entry's
state and its header. A task never leaves Done, and its record remains stable
under that lock. Do not clear input fields in `finish_free` after the compiled
counter is decremented: GC can run there. This implementation needs no new
synchronization or counter changes.

Only GC reachability changes. Collection must still occur; already-promoted
input graphs can wait until a full collection. Native argument-vector capacity
and task records remain until normal removal. Other legitimate aliases keep
shared inputs alive. This does not infer unique ownership or deterministically
free input memory at task completion.

## Baseline and proof

Before production changes, saved four smoke and 24 full-size baseline runs
from immutable G5b binary `a0a9838`. New coverage creates 64 children with
32,768-word arrays (16 MiB total), joins them, keeps their scope open through
524,288 persistent-map updates, then yields idle. Full checksums are independently
calculated in Python. Returned-array control joins and checks all results again
after churn, requiring useful aliased inputs to survive.

Final paired comparison: 112 ordinary runs, seven repetitions per case at
1/2/4/8 workers. Every after-churn/idle checkpoint has:

| Case | Baseline native live bytes | Candidate native live bytes |
|---|---:|---:|
| Consumed task inputs | 16,777,216 | 0 |
| Returned aliases, including 512 bytes for retained job handles | 16,777,728 | 16,777,728 |

No own builds/tests/experiments overlap the 364 saved measured intervals across
baseline and final matrices. Executables, revisions, source hashes, randomized
adjacent orders and host context are retained. Main `56f5983` was integrated
before the earlier baseline; main continues independently. Only task.rs changes
production behavior against G5b; expanded Buffer tests compile out of binaries.
[Provenance and machine-checked checkpoint proof](G6a-binaries.json),
[raw comparisons, ranges and checkpoints](G6a-retention/summary.md).

## Performance limits

The consumed-input test's one-worker peak RSS drops about 7.1 MiB; multiworker
peak reductions are much smaller (about 0.2–1.4 MiB). Process peak RSS can occur
before reclamation, and native allocators/pools retain reservation. Saving
16 MiB of live capacity is not proof of a 16 MiB RSS reduction or OS page return.

Work-time medians for consumed inputs change +5.7%/+7.1%/+0.3%/+12.2% at
1/2/4/8 workers; GC pause medians also rise. The seven eight-worker pairs all
slow down (per-pair median +10.9%). Returned-alias control timings also vary
(-3.0% to +7.0%). All outliers remain. Earlier reclamation moves native frees
into earlier collections, but the measurements do not isolate that as the sole
cause; code layout, allocation behavior and shared-host scheduling may contribute.
Accept the lifetime/memory reduction while recording the short-workload cost;
do not advertise this slice as a throughput/GC-pause improvement.

The 224-run application comparison (seven repetitions, four profiles,
1/2/4/8 workers) ranges from -3.3% to +3.6% median work time, with mixed
RSS/pause deltas. These small changes and noisy pairs establish no broad
speedup or stable broad regression. Existing applications do not keep the same
large unused inputs under long-open scopes. No tail-latency improvement claim.
[Applications with all paired ranges and sampled latency](G6a-applications/summary.md),
[compact application table](G6a-validation/application-comparison.md).

## Correctness checks

All 66 CLI/program tests pass, including the new lifecycle fixture in the
source/compiled and single/multiworker corpora. Ten ordinary and twelve
diagnostic runtime tests pass; strict runtime Clippy with all targets in both
modes, scoped formatting and 16 harness tests pass.

The focused checker passes 88 runs: source and saved MIR, workers 1/4/8,
generational on/off, scheduler perturbation on/off, GC stress, verification on
and off. It exercises repeated joins, returned closures capturing input aliases,
normal completion, cancellation, failure, deferred cleanup using captured
pointer graphs, and task inspection after collection. Verification off covers
actual managed-cell freeing; verification on covers poisoning/quarantine. The
inspection probe requires completed capture-byte counts to be zero. Additional
small coverage runs validate both input and result checksums under perturbation.
The earlier 44-run verification-only result is retained separately.
[Logs and exact commands](G6a-validation/).

## Next priority

Fresh balanced-application census still reports about 759,532 MapNode and
377,748 Option allocations at one worker (diagnostic, process-wide). HAMT nodes
account for about 209 MiB of managed cells. Prioritize eliminating ordinary
lookup-result boxes at proven local intrinsic boundaries (I2), then validate
immutable-map/record allocation reductions without violating snapshot ownership.
The existing channel split-result path offers a concrete mechanism to evaluate
for map/buffer lookups. These are candidates, not implemented results. Targeted
pool reservation/finalizer work remains relevant to G5b's memory tradeoff and
G6a's short-workload cost. Arena/ring policies still need their ownership proof.
