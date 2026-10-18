# Runtime concurrency and memory performance review

Reviewed 2026-10-15 at `c5c7b484c37bbc6dab46b23471bce80626ff01b1`, including the
existing dirty working tree. Production code was not changed by this audit. In particular,
the existing per-task temporary-root and GC-verification changes in `gc.rs` and
`task.rs` were included and preserved. This report extends the runtime portions
of [the existing optimization plan](../OPTIMIZATION_PLAN.md).

**Checkout changes during review:** HEAD subsequently advanced to
`665448d2f0b7d0a1ccf22e6b91d25b617c31ed1b`. The measured `channel.rs` and
`task.rs` hashes exactly match those files in that commit. Additional incoming
working-tree edits now add byte charges for blocked senders and task-owned
channel registrations. The retained reproducers now pass against those incoming
edits, including forced GC and verification. Baseline findings, line references and timings below
describe the measured version; follow-up verification is recorded separately
at the end. Those production edits were made outside this audit.

## Conclusion and measured evidence

**Compiled CPU work scales; the runtime remains globally serialized. Locks are
not currently fine grained.** Independent channels and independent allocating
tasks contend on the same runtime lock. Improving the collector alone cannot
remove that serialization.

Every generated runtime call in multi-worker mode passes through
[`rt_call`](../crates/lang_codegen/src/lib.rs#L1112). It decrements the global
`IN_COMPILED` counter and acquires `LOCKED`, then increments that counter and
releases the lock on return
([task.rs:332–380](../crates/lang_runtime/src/task.rs#L332),
[443–454](../crates/lang_runtime/src/task.rs#L443)). Runtime callbacks into
compiled code release it, so this is not a lock held for the whole task.
Nevertheless, allocation, collection access, channel access, task bookkeeping,
and polling slow paths all cross the same boundary.

Fresh measurements, median execution milliseconds:

| Workload | 1 worker | 2 workers | 4 workers | 8 workers |
| --- | ---: | ---: | ---: | ---: |
| Four CPU tasks | 696.730 | 344.933 | 176.073 | 177.796 |
| Existing prompt/batch/round-trip benchmark | 48.293 | 144.753 | 175.236 | 248.110 |
| Collections, no explicit parallel tasks | 583.510 | 693.181 | 694.194 | 682.837 |
| Four independent channel pairs, 200,000 messages total | 18.473 | 41.641 | 82.829 | 141.760 |
| Four allocating tasks, 800,000 record/window updates total | 99.167 | 195.472 | 309.046 | 336.946 |

Four workers give **3.96× CPU speedup**, but independent channel pairs take
**4.48× as long** and allocating tasks take **3.12× as long**. The CPU fixture
has four tasks: eight workers do not add useful CPU parallelism to it. The
collections row measures the overhead of enabling multiple workers, not
collection parallelism. The allocation fixture also performs persistent-list
operations, so its result is not a measurement of `malloc` alone. The messaging
row combines three distinct operations; individual timings are in the raw data.

Method: optimized host and JIT; fresh sequential processes; one warmup and three
measured repetitions for each of 20 case/worker combinations (80 runs total).
Inherited `LANG_*` variables were removed. The existing `--bench` execution
boundary excludes compilation. CPU affinity/frequency were uncontrolled and
caches warm. These are directional baselines, not tail-latency or lock profiles;
the totals do not attribute a percentage of time to any lock.

Retained [commands, outputs, ranges, machine details, source hashes and binary hash](2026-10-15-runtime-concurrency-measurements.json),
[measurement runner](runtime-audit/measure.py),
[independent-channel fixture](runtime-audit/independent_channels.lang), and
[parallel-allocation fixture](runtime-audit/parallel_alloc.lang).

## Correctness and retention gates before optimization

### G1 — Blocked prompt senders bypass byte accounting: reproduced, incoming fix verified

[`send`](../crates/lang_runtime/src/channel.rs#L341) computes a byte charge and
normal queue insertion records it. However,
[`admit_sender`](../crates/lang_runtime/src/channel.rs#L436) pushes a blocked
sender's value without checking remaining byte capacity or updating `bytes`
and `sizes`. A receive invokes this admission immediately after freeing one
queued message. Message capacity and byte capacity are different constraints.

The [language reproducer](runtime-audit/byte_budget.lang) uses capacity 2 and
budget 4. After receiving `"aaaa"`, a blocked `"bbbb"` is admitted. A subsequent
`try_send("cccc")` succeeds, although `"bbbb"` already consumes the entire
budget. Output is `BUG: byte budget exceeded` with workers 1 and 4, including
optimized execution with both GC stress and verification enabled.

Fix the admission transaction first: retain each waiting value's computed
charge, recheck both limits, and update queue/value/charge metadata together.
Define the fairness policy when the oldest sender cannot fit but later ones
can. Use checked or saturating size accumulation with explicit over-budget
handling; `bytes + size` must not wrap into successful admission.

Acceptance: mixed message sizes, close, cancellation, direct handoff and select
preserve the budget and return unsent ownership correctly. A logical byte
budget still does not account for blocked producers' owned payloads or all
physical runtime memory; producer admission must be bounded separately.

### G2 — Cancelled channel waiters retain payloads: reproduced, incoming fix verified

[`send`](../crates/lang_runtime/src/channel.rs#L379) registers `(task, value)`
before parking. Its withdrawal code runs only after `park_for` returns.
[Cancellation abandons the stack](../crates/lang_runtime/src/task.rs#L781), so
that code is skipped. [`finish`](../crates/lang_runtime/src/task.rs#L1658)
releases leases but does not unregister other channel waits.
[Channel tracing](../crates/lang_runtime/src/channel.rs#L98) visits every
waiting sender value, including removed tasks. Stale registrations remain
until another operation happens to drain them. Closing a channel drains its
waiters, but a long-lived full channel need not be closed or drained.

The retained [single-worker runtime-ABI reproducer](runtime-audit/cancelled_waiters.rs)
cancels and removes eight blocked tasks per round, then explicitly collects:

```text
round 1: stale senders=8, heap objects=10
round 2: stale senders=16, heap objects=18
```

This is a runtime-API reproduction, not a new language acceptance test. The
same output holds with GC stress and verification. It confirms registration
and managed-payload retention independently of compiler root liveness.

Give waits explicit registration tokens owned by the task. Unregister them
when the task abandons the wait, including cancellation, failure and select
loss. Prefer indexed removal or intrusive entries over repeated full scans.
Preserve exactly-once delivery and rendezvous commitment. Do not merely skip
stale values during tracing while retaining dangling words in registrations.

Acceptance: thousands of cancellation rounds on one retained, undrained channel
leave registration count and post-GC memory bounded. Cover receive, batch
capacity, select and timer registrations as well as prompt senders.

### G3 — GC stop request races with generated budget updates: code finding

[`stop_the_world`](../crates/lang_runtime/src/task.rs#L500) writes another
worker's plain `TaskCtx.budget: i64` using `write_volatile`. Generated polls
perform ordinary loads and stores of the same field
([codegen:636](../crates/lang_codegen/src/lib.rs#L636)). Volatile access does
not establish atomic synchronization. The worker can also overwrite a forced
zero with a previously loaded budget. `STOP` is stored but not consulted by
these generated polls. This needs a synchronization correction before claiming
robust GC rendezvous under parallel load. No sanitizer reproduction was run.

Keep the decrementing task budget owned by the running task. Publish collection
requests through a separate atomic flag/epoch checked by generated polls, and
use an explicit worker acknowledgement protocol before scanning roots. Specify
release/acquire edges for root publication and resumption. Avoid making every
budget decrement a contended atomic read-modify-write.

Acceptance: collector requests overlap polls, migration, foreign calls and
runtime returns; every worker acknowledges the requested epoch before tracing;
no root writes occur while the collector scans. Model the native handshake
separately: ordinary chaos tests do not prove memory-order correctness, and
JIT code is not automatically covered by Rust sanitizer instrumentation.

## Prioritized opportunities

Benefits below are hypotheses unless an algorithmic operation is directly
identified. No optimized implementation or achieved improvement is claimed.

| Priority | Opportunity and evidence | Concrete change |
| --- | --- | --- |
| P1 | Batch occupancy scans queued batches and producer staging on every append: [channel:68](../crates/lang_runtime/src/channel.rs#L68), [639](../crates/lang_runtime/src/channel.rs#L639). | Maintain accepted-message occupancy in O(1). Acceptance increments; publication and leasing transfer ownership without changing it; release, per-message receive and disposal decrement exactly once. Keep a debug recomputation invariant. |
| P1 | Batch single-message receive uses `front.remove(0)`: [channel:715](../crates/lang_runtime/src/channel.rs#L715). Draining B items shifts B(B−1)/2 words. | Store a read cursor into a sealed page. Trace only unconsumed/leased items and recycle at exhaustion. This preserves packed pages without a memmove per receive. |
| P1 | Every append pops a recycled page even when staging already exists: [channel:641](../crates/lang_runtime/src/channel.rs#L641). An occupied map entry drops the unused page. | Acquire a page only for a vacant producer entry; reserve its bounded batch capacity. Share staging/time metadata in one producer entry. Measure buffer reuse, allocations and peak reserved bytes. |
| P1 | Batch release recomputes recursive payload sizes: [channel:544](../crates/lang_runtime/src/channel.rs#L544). Admission materializes map entries, enum fields and cloned buffers: [252–324](../crates/lang_runtime/src/channel.rs#L252). | Record admitted byte charges in staged/sealed/leased metadata and subtract stored charges. Iterate borrowed immutable data for initial sizing; avoid materialization. Preserve the existing structural charging semantics before adding caches for immutable values. |
| P1 | Capacity and select notifications broadcast to all waiters: [channel:236](../crates/lang_runtime/src/channel.rs#L236), [327](../crates/lang_runtime/src/channel.rs#L327), [publish](../crates/lang_runtime/src/channel.rs#L587). | Wake an admissible bounded number of producers. Use explicit select registrations with one winner and unlink losers. Preserve FIFO/fairness and byte-fit policy; close may legitimately wake everyone. Measure useful work per wake and repeated parks. |
| P1, incoming work | Every finished task scans all live channels and their leases in the baseline: [task:1666](../crates/lang_runtime/src/task.rs#L1666), [channel:740](../crates/lang_runtime/src/channel.rs#L740). | Incoming edits replace the global scan with task-owned channel registrations. Refine that into active wait/lease indexes: normal completion and unwatch should retire registrations so a long-lived task does not root every channel it has ever used. Preserve explicit release idempotence. |
| P1 | Every collection creates child buffers for traced objects and a new survivor vector: [gc:294–396](../crates/lang_runtime/src/gc.rs#L294). | Reuse the mark worklist, mark children directly through a visitor, and compact object pointers in place. Preserve verifier attribution, finalization order constraints and quarantine behavior. Avoid an unnecessary fresh vector for each pointer-bearing object. |
| P1 | GC pressure and live-byte counts omit most external storage: [gc:172](../crates/lang_runtime/src/gc.rs#L172), [179](../crates/lang_runtime/src/gc.rs#L179), [365](../crates/lang_runtime/src/gc.rs#L365), [395](../crates/lang_runtime/src/gc.rs#L395); [buffer](../crates/lang_runtime/src/buffer.rs#L43), [map](../crates/lang_runtime/src/map.rs#L83). | Account for buffer/channel capacities, directories, boxes and shared trie storage. Track capacity growth/release and shared-node ownership without double counting versions. Distinguish live bytes, pressure since collection, reserved pools, immortals and verification quarantine. External growth must be able to request GC, not depend solely on a later managed allocation. |
| P1 | Output writes and flushes hold the runtime lock: [I/O:42–63](../crates/lang_runtime/src/intrinsics_io.rs#L42). A full pipe can block unrelated runtime work. | Move owned output bytes to a bounded writer/blocking operation, then construct managed results on return. Preserve write ordering and current flush semantics; changing buffering is an API policy decision. |
| P1 | The blocking pool limits threads, but its job deque is unbounded: [task:1061–1146](../crates/lang_runtime/src/task.rs#L1061). Thread creation errors are ignored after reserving a thread count. | Bound queued work and bytes with task-level admission; handle thread-start/job failures and settle completion bookkeeping. Instrument queue wait separately from service time. Preserve the documented behavior that an executing blocking call completes before cancellation settles. |
| P1 | HTTP spawns a thread per accepted connection, reads the request, then checks handler admission: [http:199–220](../crates/lang_runtime/src/http.rs#L199), [370–377](../crates/lang_runtime/src/http.rs#L370). | Add bounded connection/read admission before thread creation and parsing, distinct from handler limits. A bounded network pool is an incremental step; a readiness reactor is a later candidate. Keep long-lived streams from exhausting the general file/FFI pool. |
| P2 | Batch consumption still enters the runtime for length/get on each item: [stdlib](../crates/lang_hir/std/channel.lang#L178), [intrinsics](../crates/lang_runtime/src/intrinsics_task.rs#L438). | Compile a lease-aware iterator over the sealed page with a rooted lease and lifetime checks. Batching should amortize runtime crossings as well as publication/wakeups. Scalar replacement can remove some wrappers, so count actual crossings and allocations before claiming savings. |
| P2 | Ready tasks share one queue; every wake may `notify_all` idle workers: [task:795–816](../crates/lang_runtime/src/task.rs#L795), [1465](../crates/lang_runtime/src/task.rs#L1465). | After task/GC ownership is explicit, add worker-local ready queues, a remote injector and stealing. Wake according to newly available work, reserving broadcasts for shutdown/global transitions. Preserve sleep/wake sequencing and deterministic scheduling mode. |
| P2 | Global counter traffic and lock handoff affect every runtime crossing: [task:332–380](../crates/lang_runtime/src/task.rs#L332). Contended acquisition spins 200 times then uses timed condition-variable waits. | After measuring hold/wait distributions, replace the global compiled counter with cache-separated worker state and a proven GC handshake. Measure adaptive parking and cache-line placement; timed waits do not imply a fixed 1 ms operation delay because notifications may wake sooner. |
| P2 | Task entries, arguments and runtime roots remain traced after completion until scope removal: [task roots](../crates/lang_runtime/src/task.rs#L549), [finish](../crates/lang_runtime/src/task.rs#L1658), [scope removal](../crates/lang_runtime/src/scope.rs#L129). | Once cleanups finish, clear roots/captures no longer needed by execution while retaining the typed join result and diagnostic state. Clear abandoned temporary roots at the correct transition. Long-lived scopes need bounded execution-state retention, not just stack recycling. |
| P2 | Every version of a persistent map walks its entries during GC despite sharing unmanaged trie nodes: [map:43–59](../crates/lang_runtime/src/map.rs#L43). | Measure tracing against number of retained versions and logical entries. Consider node-aware tracing/deduplication only with a supported traversal and ownership design; wrapper-level object marks do not deduplicate trie traversal. |
| P2/P3 | Allocation is one system `alloc_zeroed` per managed object under the global lock: [gc:155–174](../crates/lang_runtime/src/gc.rs#L155). | First reduce boxes/wrappers and scratch allocations. Then introduce nonmoving size-class pages and worker-local allocation caches, with batched publication to the collector. Pooling alone under the global lock cannot provide allocation parallelism. |

Additional local memory savings: narrow list pages reserve `cap * size.max(8)`
despite packed access using `cap * size`
([list:136](../crates/lang_runtime/src/list.rs#L136)); persistent append copies
the page directory ([207](../crates/lang_runtime/src/list.rs#L207)). Use exact
layout sizing and builder/bulk APIs where ownership permits. Replacing the
directory with a persistent structure is a separate measured design choice.
Avoid eager `chunks()` copies when a rooted page view can satisfy the operation.

## Safe path to finer locking

The target is **no shared lock for task-owned work or proven immutable reads,
and short locks around independently mutable resources**. Adding a mutex to
every object would add overhead without solving collector coordination.

1. **Make synchronization ownership explicit.** `rt()` and `task()` expose
   unrestricted mutable references; collection `data()` helpers also fabricate
   mutable references to shared objects. Replace these with handles, scoped
   guards and separate immutable/mutable access. Do not hold mutable references
   over suspension or callbacks into compiled code. Stable task addresses need
   a lifetime/reclamation protocol when task-table locking is narrowed.
2. **Classify runtime operations in the compiler.** Separate pure nonallocating
   reads, task-local operations, allocation, shared mutation and potentially
   blocking calls. Start with string length/bytes and sealed-page access, then
   proven immutable map/array reads. Keep roots published and workers counted
   as active while performing managed reads, so GC cannot reclaim beneath them.
   Only validated operations bypass the existing lock.
3. **Move allocation and scheduling toward worker ownership.** Per-worker heap
   segments, scratch space and ready queues remove global hot paths. Roots and
   cancellation/cleanup state remain task-owned across migration; thread-local
   caches must not be assumed to follow a suspended task. Publish heap segments
   and returned pointer roots before acknowledging a collection epoch.
4. **Separate channel mutation from scheduler mutation.** A channel lock covers
   queue/staging/admission state. Produce wake intents under it, then release it
   before scheduling. Parking requires an atomic register/check/suspend protocol
   to avoid a wake arriving between the check and suspension. Multi-channel
   selection needs one atomic winner and a documented lock order.
5. **Never collect or park while holding a resource lock needed by another
   worker to acknowledge GC.** Allocate before locking, reserve through a
   noncollecting path, or retry after releasing the lock. Similarly, callbacks
   and user equality/hash/display hooks cannot execute under channel or heap
   shard locks. Preserve current returned-pointer rooting across callbacks.

Published lists deserve special care: current persistent append can extend a
shared partial tail and updates its length
([list:196–234](../crates/lang_runtime/src/list.rs#L196)). It is not sufficient
to label every persistent collection physically immutable and remove its lock.
Establish sealed tails or synchronized extension before enabling concurrent
access that touches this metadata. Shared `Atom<T>` access already uses atomics,
but generated intrinsic entry still takes the runtime lock; bypassing that lock
also requires GC-safe pointer publication and root handling.

## Collector evolution under heavy parallel workloads

Keep the nonmoving collector and stop-the-world correctness model initially.
First separate pause measurements into **request-to-acknowledgement, root scan,
mark, sweep/finalization, and resume**. Current pause totals combine these
([gc:282–291](../crates/lang_runtime/src/gc.rs#L282)); there is no p99 histogram.
Maximum pause alone cannot identify handshake versus traversal cost.

After allocation paths are independent, parallel marking/sweeping during a
stop-the-world phase is a candidate for large live sets. It requires safe mark
ownership or atomic marks, thread-safe tracing, and isolation of finalizers that
mutate global registries or close external resources. Measure bandwidth and
worker utilization: more collector threads can add contention for small heaps.

A nursery or concurrent marking is a later architectural step. Old-to-young
references and concurrently mutated roots/objects need barriers and remembered
sets. Shared channels, atoms, persistent-list tail updates and builders all
participate. A moving nursery would conflict with the current pointer-stability
contract unless an independently proven eligible allocation category is added.
Do not adopt concurrent GC merely by allowing allocations during today's mark
phase. Measure graph shape, survival and live-set size before choosing this work.

## Implementation order and acceptance proof

| Slice | Deliverable | Required proof |
| --- | --- | --- |
| 0 | Retain regressions for incoming byte-admission/cancelled-wait fixes; correct GC stop signalling. | Retained reproductions become invariant assertions; memory/registrations plateau over cancellation soak; handshake ordering model plus optimized native runs. |
| 1 | Worker-local measurement buffers and lock/GC/channel counters. | Hold/wait and queue/service times separated; timer lateness and p50/p95/p99/p99.9 measured; quantify instrumentation overhead. No global histogram lock per operation. |
| 2 | O(1) occupancy, stored charges, read cursors, correct page reuse, task-owned lease index. | Existing conservation/order/select/cleanup suite passes; vary batch size, capacity, producer count and live-channel count; show allocations/copies/wakes per message decline. |
| 3 | Direct GC visitor, reused worklist, in-place sweep, external memory accounting and completed-task root cleanup. | Heap verification and forced collection pass; post-GC RSS and accounted live bytes plateau; compare pause phase distributions at equal live sets. |
| 4 | Bounded blocking and network admission; output outside the runtime lock. | Slow pipe/slow client/blocked FFI saturation keeps unrelated task/timer latency bounded; shutdown settles every admitted operation; queue memory remains bounded. |
| 5 | Guarded immutable/task-local fast paths and sealed batch iteration. | Same program results under 1/2/4/8 workers; lower measured runtime crossings; improve independent-pair and runtime-only overhead cases without weakening GC or ownership. |
| 6 | Per-worker nonmoving allocation, local queues and per-channel synchronization. | Independent channels and heaps make simultaneous progress; no missed wake, use-after-free or double release; useful work scales without transferring contention to task tables, registries or GC state. |
| 7 | Profile-driven parallel/concurrent collector work. | Demonstrated benefit at large live sets, acceptable small-heap overhead, explicit barrier/finalizer design and retained stress/sanitizer/model evidence. |

Benchmark fixed total work separately from fixed work per worker. Add tiny,
medium and large pointer-bearing messages; rendezvous and byte budgets; one hot
channel versus independent pairs; many producers/consumers; mixed allocation,
I/O and CPU tasks; bursty overload; shared map versions; deep roots; prolonged
cancellation and shutdown. Record throughput, CPU time, context switches,
peak/post-GC RSS, allocation counts and useful wakes alongside latency. Do not
claim a latency improvement from the current elapsed-time baselines alone.

## Verification and reproduction

Completed in this review:

- `cargo build --release --locked --offline` passed.
- `cargo test -p lang_runtime --locked --offline` passed: one parser test;
  this crate test alone provides almost no scheduler/GC coverage.
- `cargo test -p lang_cli --test stress stress_programs --locked --offline -- --nocapture`
  passed: five programs across seven configurations, including 1/4/8 workers,
  chaos, a verification configuration and an optimized-JIT configuration.
- All 80 baseline benchmark processes completed successfully. No performance
  measurement overlapped the stress matrix.
- Byte-budget reproduction confirmed on workers 1 and 4, with/without combined
  GC stress and verification. Cancelled-waiter reproduction confirmed normally
  and with combined GC stress and verification. See
  [retained reproduction outputs](runtime-audit/reproductions.json).

The stress suite's quick tier does not combine every option, does not include
all worker counts, and does not establish a memory plateau. No full workspace
suite, long soak, sanitizer, perf lock profile or p99 load test was run for this
analysis. The GC budget finding is source evidence, not a reproduced crash.

Reproduce the byte-budget finding:

```sh
target/release/lang run --release --workers 1 reports/runtime-audit/byte_budget.lang
LANG_GC_VERIFY=1 LANG_GC_STRESS=1 target/release/lang run --release --workers 4 reports/runtime-audit/byte_budget.lang
```

Build the isolated runtime reproducer after the release build (choose the
newest release runtime artifact if several exist):

```sh
python3 - <<'PY'
from pathlib import Path
import subprocess
artifact = max(Path('target/release/deps').glob('liblang_runtime-*.rlib'), key=lambda p: p.stat().st_mtime)
subprocess.run(['rustc', '--edition=2024', 'reports/runtime-audit/cancelled_waiters.rs',
                '-L', 'dependency=target/release/deps', '--extern', f'lang_runtime={artifact}',
                '-o', '/tmp/language-runtime-cancelled-waiters'], check=True)
subprocess.run(['/tmp/language-runtime-cancelled-waiters'], check=True)
PY
```

Two corrections to earlier planning material matter for prioritization:
`Vec<u64>::truncate` does not shift the remaining values, so substituting
`set_len` does not remove the claimed memmove; and an `Int` list page here holds
512 elements (`4096 / 8`), not 4096. Existing task roots are now per-task, not
one shared temporary-root vector. This review uses the current source rather
than the older memory-plan assumptions. It also does not attribute any measured
fraction of record time to `malloc` without a profile.

### Follow-up verification of incoming changes

Rebuilt the externally changed working tree with
`cargo build --release --locked --offline` and reran the 35-case quick stress
matrix; both passed. The byte-budget reproducer now prints
`byte budget respected` on workers 1 and 4, normally and with combined forced
GC/verification. Recompiled the isolated waiter reproducer against the new
runtime; both rounds now report **zero stale senders and two heap objects**,
normally and with forced GC/verification. The prior 8/16 retained registrations
were eliminated in this targeted reproduction. Source hashes and all six
follow-up runs are [retained separately](runtime-audit/followup-verification.json).

The incoming code also eliminates the unconditional all-channel completion scan.
Its `chan_regs` list is rooted and deduplicated with a linear search, and entries
are removed at task completion rather than normal wait/lease completion. A
long-lived task therefore still accumulates historical channel registrations;
active-registration retirement remains an optimization/retention opportunity.
This is source evidence, not a newly reproduced long-duration memory leak.

The GC stop-budget synchronization concern remains. Performance baselines were
not rerun after the incoming fixes: the table and raw benchmark outputs retain
their original source identity and must not be presented as post-fix timings.
