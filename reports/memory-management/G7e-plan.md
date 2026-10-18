# G7e: measure tracing work before changing its algorithm

Status: diagnostic cardinality counters implemented; batched marking is only a
proposal. Ordinary builds retain the G7d collector algorithm and contain no new
counters. The fixed ordinary baseline is `38855c7`, now integrated in main.

G7d's eight-task pointer-snapshot profile has about 69 ms in tracing versus
18 ms in sweep after prefetch. One-worker trace work is about 27 ms. Collection
counts/root timing differ, so first measure candidate words, new marks, leaf
marks, traced objects and remembered-object traces. Record instrumented-off versus
phase-on versus ordinary calibration. Do not infer per-object memory latency or
lock contention from total times alone.

A possible next collector experiment is a bounded stack-local ring (e.g. 32 raw
pointer words) between discovering pointers and reading their headers. Hint the
headers when enqueuing and mark an older pointer when the ring fills. Existing
marking still filters duplicates/old/immortal/leaf objects and pushes nonleaf
objects onto the trace stack. Drain pending pointers when there is no traced
node ready; finish ONLY when BOTH queues are empty. This bounds deferred words,
even for huge arrays containing repeated references; avoid enqueueing unbounded
raw edges onto the heap worklist.

Audit all native tracers before changing visit order. Existing list directory
slots are weak on full collections; lists report visible pages, and unmarked
weak slots are cleared only after tracing. Their clearing, minor verification,
allocation census/sweep and worker resumption must follow complete queue drain.
The world remains stopped and validation of emitted pointers retains their
original root/parent diagnostic context. No finalizer runs during marking, and
resource finalizer ordering must remain unchanged. Do not change GC publication,
mark-bit synchronization or mutator/barrier contracts.

If evaluated, separate immediate marking, batching without hints and batching
with hints within one diagnostic executable. This distinguishes traversal/order
cost from prefetch effects. Only then build an ordinary constant-path candidate
and measure unchanged applications at workers 1/2/4/8, with immutable binaries,
independent oracles, raw samples and median/block-mean controls. Run old/new saved
MIR, reuse/quarantine, both generations, chaos, list slices/weak pages, native
buffers, channels and closure/task roots before retention. No p99 or ARM speedup
is implied by a header hint.

The trace-work baseline is saved in [G7e-summary.md](G7e-summary.md). The subsequent
[I5b snapshot reuse](I5b-summary.md) is retained at `49f281c`, with substantial
state/snapshot allocation reductions and mixed controls recorded. It is a separate
optimization; the G7e counters themselves implement no ownership epochs or batched
marking. Pointer-map tracing/scaling and the bounded marking proposal remain open.
