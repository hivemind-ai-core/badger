# Lock optimization plan

Status: implementation roadmap, reviewed 2026-10-16 against `a9584fa` and the
working tree. The changes below are proposed unless explicitly described as
existing. This document orders synchronization work for multithreaded workloads;
it does not claim that finer locking or the stated acceptance targets have
already been achieved. Preserve language semantics and the single-worker path.

## Goal and current baseline

Independent channels, tasks and allocation streams should make simultaneous
progress. A queue operation should synchronize with the channel it uses;
waking a task should synchronize briefly with scheduling; ordinary managed
memory access should not acquire an exclusive collector lock.

Current implementation anchors:

- [Runtime and scheduler](crates/lang_runtime/src/task.rs): `LOCKED`,
  `lang_rt_enter`, `lang_rt_leave`, `Runtime`, `park`, `wake_with`, `make_ready`,
  `worker_loop`, `finish`, and `SCHED`.
- [Channels](crates/lang_runtime/src/channel.rs): `ChanData`, `try_fast`,
  `claim`, `send`, `recv`, selection commits, batching and task cleanup.
- [Channel intrinsics](crates/lang_runtime/src/intrinsics_task.rs): unlocked
  entry wrappers, `locked`, selection registration and completion.
- [Collector](crates/lang_runtime/src/gc.rs): worker-local pools/object lists,
  shared heap metadata, roots, `NoGc`, `alloc` and `collect`.
- [Code generator](crates/lang_codegen/src/lib.rs): `rt_call`, `UNLOCKED`,
  allocation lowering and poll lowering.
- [Scopes](crates/lang_runtime/src/scope.rs): registry, child completion,
  cancellation and release. [Output](crates/lang_runtime/src/intrinsics_io.rs):
  `write_out` performs synchronous writes and flushing.

The global runtime lock currently protects scheduler state, channel slow paths,
scope/registry mutations and GC coordination. Compiled code runs outside it.
Allocation already uses worker-local pools and object lists. Selected collection
operations and channel fast paths already bypass the global lock. A channel's
atomic ownership byte provides local exclusion, but waiters, selection,
publication and many admission cases still enter the global lock. Do not plan
these existing fast paths as new deliverables.

The committed [2026-10-16 benchmark snapshot](reports/benchmarks/2026-10-16-after-lock.md)
records these historical execution medians at revision `0cee2ae710`:

| Workload | 1 worker | 2 workers | 4 workers | 8 workers |
| --- | ---: | ---: | ---: | ---: |
| CPU tasks | 655.672 ms | 335.371 ms | 168.227 ms | 168.464 ms |
| Independent channel pairs | 17.198 ms | 16.729 ms | 12.174 ms | 15.565 ms |
| Messaging | 43.646 ms | 58.787 ms | 61.184 ms | 62.171 ms |
| Allocation/list updates | 92.830 ms | 91.256 ms | 79.759 ms | 76.914 ms |

These are evidence of workload-dependent scaling, not a lock-time attribution
or a baseline for every subsequent revision. Allocation/list updates also
exercise list operations and GC. Fresh measurements must capture source and
binary identity; compare fixed work on the same machine and configuration.

## Implementation order

Implement numbered slices in order. Keep each slice runnable with its stated
acceptance proof. Stages that retain the legacy scheduler lock must say which
operations still use it. Never advertise independent channel slow paths while
all of them remain wrapped in the global lock.

| Order | Priority | Deliverable | Prerequisites | Acceptance demonstration |
| --- | --- | --- | --- | --- |
| 0 | P0 | Contention and latency measurements | Existing runtime | Attribute wait/hold time and capture repeatable baseline |
| 1 | P0 | Explicit ownership and bounded access APIs | 0 | No shared mutable references escape newly guarded domains |
| 2 | P0 | GC coordination independent of runtime exclusion | 1 | Forced collection overlaps new runtime paths without races or deadlock |
| 3 | P1 | Per-channel buffered operations and waiter protocol | 2 | Independent buffered pairs progress concurrently, including blocking waits |
| 4 | P1 | Rendezvous, selection, close and cancellation integration | 3 | Exactly one completion under conflicting operations and cancellation |
| 5 | P1 | Batch publication, leases and admission improvements | 4 | Lower synchronization per message with conserved capacity and ownership |
| 6 | P1 | Worker-local scheduling and independent timers | 4; integrate 5 | Wake-heavy traffic avoids the global scheduler lock |
| 7 | P2 | Remaining runtime domains and blocking output | 2; integrate 6 | Slow output and unrelated metadata work do not serialize channel traffic |
| 8 | P2 | Atomic rings and ownership-based specializations | 3–6 | Profiled improvement over local locks at equal semantics and memory bounds |

Moving synchronous output outside the runtime lock can be delivered after 1
as an independent latency fix if its owned-byte/root protocol is established.
It must not delay the GC/channel prerequisites. Atomic queue experiments can
run earlier in isolation, but production adoption depends on the waiter and
GC protocols. This ordering prioritizes common channel traffic over rare
registry contention.

## Synchronization ownership and rules

The target domains are:

| Domain | Owner or synchronization | Shared data covered |
| --- | --- | --- |
| Running task | Sole executing worker, independent of task migration | Stack, shadow frames, temporary roots and local cleanup state |
| Task control | Stable task handle and short control lock/atomics | Lifecycle, notification, join completion and cancellation requests |
| Channel | One lock per channel initially | Queue, wait registrations, closed state, capacity/byte accounting and leases |
| Wait/selection descriptor | Stable native record, short completion lock initially | One terminal decision and its payload/status |
| Scheduling | Local ready deque and per-worker notification inbox | Runnable work and remote notifications |
| Timers | Dedicated timer lock initially | Deadlines and timer generations; later consider worker ownership |
| Heap allocation | Existing per-worker pools and object lists | Allocation cursors, free lists and local accounting |
| GC | Collector election plus worker safepoint epochs | Root/object snapshot, tracing and reclamation |
| Scopes and registries | Scoped/sharded locks or owner messages | Scope membership, inspection indexes, live resources and shared metadata |

Rules for every migrated path:

1. No resource guard or borrowed mutable reference survives task suspension,
   stack abandonment, a user callback, or a possible collection. Task defects
   can abandon Rust runtime frames without ordinary destructor execution;
   release guards explicitly before invoking defect/cancellation machinery.
2. Do not acquire the legacy scheduler lock while holding a channel or
   completion-descriptor lock. Commit and root the outcome, release those
   locks, then notify scheduling. Legacy scheduler code may temporarily take
   a channel lock only for bounded, noncollecting work; remove this nesting
   as domains migrate. Never acknowledge GC while retaining such a lock.
3. The collector elects a leader, requests quiescence and waits with no channel,
   scheduler, task-control, registry or root-registration lock held. Workers
   release resource locks and publish roots before acknowledging. Inspection
   and finalization obey the same ownership rules.
4. Ordinary paths take one channel lock at a time. For rendezvous between
   selection descriptors, take the channel lock, then descriptor locks in
   stable ID order. Cancellation/timeout takes a descriptor lock, releases it,
   then unlinks channel registrations; it never reverses that nesting.
5. No managed allocation under these short locks. Reserve native storage
   before locking where possible; move expensive sizing, encoding and copying
   out. Managed results are constructed after unlocking, with explicit roots.
6. Use acquire/release publication for transferred payloads and notifications.
   Start complex state machines with sequentially consistent atomics; weaken
   ordering only with an identified happens-before proof and retained model.

Use lock-order assertions in debug builds and document any exception alongside
its proof. Keep instrumentation outside shared histogram locks. Adding one
runtime-wide read lock around every operation would retain a shared hot cache
line and complicate allocation-to-collection transitions; prefer worker epochs.

## 0. Measure the actual contention

Extend `WorkerStats` using worker-local, cache-line-separated storage. Existing
fast/spin/park acquisition counts and parked nanoseconds do not measure lock
hold time, spinning time or the reason for acquisition.

Record:

- Runtime-lock acquisitions by category: scheduling, channel wake/wait,
  selection, batch publication/release, GC, registry and output.
- Sampled hold time, acquisition wait including spinning, queue depth, useful
  wakeups, repeated parks, steals, migrations and lock fallback reason.
- Channel fast/slow path counts by reason: contention, full/empty, byte budget,
  waiters/watchers, closure and batch publication. Bound per-channel metrics
  storage; do not create unbounded labels or a counter lock per message.
- GC request-to-acknowledgement, roots, marking, sweep/finalization and resume
  durations separately. Measure shared allocation-pressure atomics too.
- Throughput and p50/p95/p99/p99.9 latency under steady and bursty load, plus
  timer lateness while channels are saturated.

Retain runs with metrics disabled and enabled to quantify observer overhead.
Run benchmarks sequentially, apart from correctness stress. Capture revision,
dirty diff/source hashes, binary hash, CPU topology/affinity, environment,
sample count, raw distributions and memory high-water marks. Do not add worker
wait totals and interpret the sum as elapsed time.

**Proof:** repeated fixed-work runs on 1/2/4/8 workers distinguish CPU scaling,
independent-channel contention and one-hot-channel contention. Counters explain
where the global lock is still acquired. Establish variance before setting
performance thresholds; retain the baseline before each implementation slice.

## 1. Establish safe ownership boundaries

Before relaxing exclusion, replace broad `rt()`, `task()`, `heap()`, scope
`get()` and channel `data()` access on migrated paths. Returning unrestricted
`&'static mut` references makes a later lock split unsound even when callers
appear to take different locks.

- Separate immutable channel configuration from `UnsafeCell`-backed mutable
  state. Acquire the channel guard before creating `&mut ChanState`; reading
  an atomic ownership flag must not first fabricate a mutable reference to
  the entire shared object. Immutable getters must not pass through that API.
- Introduce stable task handles with generation checks or nonreused IDs plus
  strong lifetime retention. A copied task ID or stable address alone does
  not prevent removal/reuse while another worker delivers a completion.
- Separate running-task-owned fields from cross-task control. Remote workers
  publish cancellation/completion requests through the control API; they do
  not mutate another running task's plain `TaskCtx.flags`, shadow stack or
  temporary roots. The owner applies requests at a poll/safe point. Preserve
  the generated-code ABI or update offsets and generated loads together.
- Give waiter records stable storage independent of suspended coroutine
  frames. Record operation generation so a delayed timeout/wakeup cannot
  satisfy a later wait. Define unlink, terminal completion and reclamation.
- Retain records while linked to a channel, a task or a notification queue.
  A native `Arc` retains native memory only: managed payloads need traced roots.

Keep global exclusion during this refactor. Introduce guards/handles one domain
at a time and update cleanup, inspection and GC tracing alongside normal calls.

**Proof:** task removal races with notification without stale dereferences;
no guard survives yield, park, callback or abandon; ordinary task migration
keeps task roots and cleanup state attached to the task. Existing stress and
forced-GC tests keep their results. Model handle generations and record lifetime.

## 2. Separate GC coordination from runtime exclusion

The existing `IN_COMPILED` counter is insufficient once multiple runtime
mutators proceed under different locks. Today a runtime call decrements it,
then the global lock excludes that caller from collection. Replacing that lock
with local locks without replacing this handshake permits tracing during
runtime mutation.

Introduce a GC coordinator and cache-line-separated worker state:

- Collector election allows one collection leader; competitors publish roots
  and wait for that collection instead of holding runtime/resource locks.
- A request epoch identifies the collection. Worker states distinguish active
  managed access, quiescent acknowledgement of that epoch, and outside-managed
  execution such as owned-byte blocking I/O. Both compiled code and unlocked
  runtime code accessing managed objects count as active.
- Entry/reentry publishes active state and rechecks the collection request.
  If a request raced with entry, the worker must acknowledge/wait before
  accessing managed memory. The collector scans only after every registered
  worker is quiescent for its request, or safely outside managed execution.
  Design the publication/recheck ordering as one modeled protocol; an epoch
  number with relaxed loads alone is not sufficient.
- Before acknowledgement, publish stable task/shadow roots, runtime temporary
  roots, in-flight managed results, native waiter payload roots, and worker
  allocation lists. No root, managed object or allocation-list writes follow
  acknowledgement until resume. Account for worker start/exit and migration.
- While waiting for a resource lock, use bounded attempts. If progress requires
  parking an OS thread, release managed borrows, retain rooted handles, enter
  quiescence safely, then retry through the entry handshake after waking.
  An active worker blocked behind a collector-owned lock creates a deadlock.
- Split shared root/immortal/registry metadata from collector-private marking
  state. Define root registration/removal relative to epochs. Rework global
  `NoGc` suppression so unrelated workers cannot indefinitely postpone GC;
  keep nonquiescent sections bounded and visible to the coordinator.

Retain nonmoving stop-the-world tracing initially. A collector mutex alone
does not protect objects that other workers mutate. Parallel marking or
concurrent collection is a separate project requiring its own barriers and
finalizer design. Finalizers touching channel registries must work while
mutators are stopped, without depending on a parked worker releasing a lock.

Update `lang_rt_enter`/`leave`, foreign-call begin/end, callback entry/return,
allocation-triggered collection and poll lowering together. Classify entries
as immutable/task-owned, local-resource mutation, scheduler mutation or
outside-managed blocking work. Extend `UNLOCKED` only after the new contract
is implemented; keep a legacy-domain wrapper for remaining operations.

**Proof:** model request/entry, request/return, simultaneous collectors and
worker exit. Force GC during pointer-bearing channel traffic, native wait-record
completion, callbacks and migration. Retain artifacts on failure. Demonstrate
no writes after acknowledgement and no missed roots. Repeat optimized native
runs; chaos tests alone do not prove atomic ordering, and JIT accesses are not
automatically covered by Rust sanitizers.

## 3. Make buffered channels independently synchronized

Replace the split `state = fast/legacy-claimed` protocol with a scoped channel
guard covering queue, closed state, accounting and waiter registrations. Remove
the global `CLAIMED` list and `unlock()` side effect once all channel paths use
the guard. A local adaptive lock may spin briefly then park; unbounded spinning
is unsuitable for oversubscribed workers. Preserve an efficient one-worker path.

First migrate buffered send/receive, including byte-budget channels, through
the following protocol. Keep selection/rendezvous on the legacy bridge until
slice 4; interoperability still uses the same channel guard.

1. Root the channel and outbound managed value. Prepare message size, result
   storage and a stable wait record before acquiring the guard. On retry,
   recheck mutable queue/closure conditions while holding it.
2. Under the guard, either commit a queue transfer, complete an eligible
   opposite waiter, return full/empty/closed, or register the current wait.
   Queue/value/byte metadata changes form one transaction. Size arithmetic
   is checked or uses subtraction-based fit tests to prevent overflow.
3. Transfer ownership of a consumed message into a traced completion record
   or the current task's roots before releasing its previous root. Close,
   cancellation and timeout compete with transfer through the same descriptor.
4. Release the guard. Construct managed `Option`/`Result` wrappers and deliver
   scheduling notifications afterward. Notifications retain the wait record;
   they never pass an unrooted managed payload between workers.
5. For a registered wait, enter a scheduler `park_if_pending(record)` bridge.
   Under the legacy scheduler lock, reconcile completion and task state.
   Completion before parking means continue; completion after parking means
   enqueue exactly once. Notification cannot resume a coroutine that has not
   yet finished suspending. Mark the task eligible for dispatch only after
   the scheduler has regained control of its coroutine stack.
6. Spurious notifications retry by operation generation. Cancellation cleanup
   can unlink a registered wait even when its original stack was abandoned.

Use terminal states such as pending, committed, cancelled, timed-out and closed;
the exact enum may differ. Initially a descriptor lock protects the decision
and payload together. If converted to atomics, distinguish claiming completion
from publishing a fully initialized outcome. Never expose a terminal payload
before it is initialized. Cancellation that loses to committed transfer must
not retract or resend the transferred value.

Initially notifications can briefly acquire the legacy scheduler lock, after
releasing the channel guard. Queue operations themselves must not. This makes
independent queues concurrent while keeping scheduler migration incremental.

**Proof:** buffered independent pairs make concurrent progress on 1/2/4/8
workers; full/empty transitions, byte limits and many producers/consumers keep
FIFO rules, conservation and exactly-once delivery. Inject completion before
registration finishes, before suspension, after suspension and during task
removal. Pointer messages survive collection at every ownership transition.
Measure global acquisitions/message and p99 latency, not just throughput.

## 4. Integrate rendezvous, selection, cancellation and close

Current selection commits inspect and change another task's `select_offer`,
`select_done` and delivery fields under global exclusion. Replace that access
with a shared selection descriptor; each case registration references it and
the operation generation. Preserve the existing fair rotating start policy.

- Under a channel guard, acquire the relevant completion descriptor(s), check
  eligibility and reserve the winning case before consuming any message or
  capacity. Publish transfer and terminal state as one protected decision.
  Losing cases cannot consume a value or decrement admission counters.
- A rendezvous can involve two selections. Acquire both descriptors in stable
  ID order under the channel guard, reject self-matching or already-completed
  descriptors, then commit both sides together. A single winner CAS on one
  selection does not make this two-party transaction atomic. Keep this
  short-lock arbitration until an alternative has a complete proof.
- Selection registers cases one channel at a time, rechecking readiness under
  each channel guard. A concurrent winner causes later registrations to be
  skipped/retired. Recheck the descriptor before parking; retain completed
  registrations until no partner can reference them. Avoid locking every
  selected channel simultaneously.
- Cancellation and timeout compete for the descriptor's terminal decision,
  then release its lock before unlinking registrations channel by channel.
  Remove obsolete timers by generation; late events become harmless.
- Close linearizes under its channel guard. Preserve sender-close drain
  behavior, receiver-close disposal, staged-data policy, returned unsent values
  and lease ownership. Detach completed waiters, then notify outside the guard.
  Process large close lists in bounded chunks with retained state if needed.
- Retire normal completed registrations promptly. `chan_regs` currently keeps
  historical channels until task end; long-lived tasks should retain active
  waits/leases only, with explicit tracing while a registration remains live.

**Proof:** race two channels against one select, two selects in rendezvous,
duplicate cases, simultaneous timeout/cancel/close, and stale registration
cleanup. Verify one winning case, one ownership transfer, no lost wake, no
duplicate queue entry and bounded retained memory after repeated cancellation.
Keep closed-send and oversized-send selection behavior unchanged.

## 5. Amortize batch synchronization and admission work

Batch channels should synchronize primarily when reserving capacity, publishing
a page and releasing a lease. Current `append_fast` still takes the channel's
ownership byte for each message and uses channel-resident producer staging.

- Start with the per-channel guard and correct accounting. Store admitted byte
  charges alongside staged/sealed/leased items; release/disposal subtracts the
  stored charge rather than recursively sizing payloads again. Preserve the
  existing structural charge semantics, including oversized-message rejection.
- Obtain a recycled page only when creating staging for a producer. Reserve
  appropriate bounded storage; do not pop/drop a page on every occupied entry.
  Keep existing constant-time occupancy and front-pop behavior.
- Move staging toward producer-task ownership, so migration carries staging
  with the task. Reserve message slots/byte credits from the channel in bounded
  blocks. Track unused reserved credits separately from accepted messages;
  they must not alter public occupancy semantics or strand channel capacity.
  Reclaim credits on task end, cancellation, close and idle staging. Use small
  reservations or disable block reservations when fairness/capacity demands it.
- Publish immutable sealed pages in one channel transaction. Preserve flush
  on yield, park and completion, explicit flush, close/discard policy and flush
  latency. The existing deschedule/safepoint hooks must release scheduler/task
  guards before acquiring a channel guard in the final design.
- Trace unpublished staging through task-owned roots and published pages through
  channels. Transfer roots atomically with ownership; inspection and GC must
  account for reserved, accepted, published and leased states distinctly.
- Wake an eligible bounded number of capacity waiters, preserving byte-fit and
  fairness policy. Batch iteration over a valid task-owned lease should require
  no channel lock per element. Release returns capacity exactly once and is
  coordinated with task cleanup and close.

**Proof:** vary page size, producers, capacity, byte budgets and consumer delay.
Accepted messages equal staged + published + leased; accepted plus reserved
credits never exceeds admission capacity. Credits return on every exit path.
Show fewer locks, allocations and useful-wakeup costs per message without
starvation, stranded staging or unbounded pooled buffers.

## 6. Remove scheduler contention from channel wakeups

After channel state is independent, the global ready queue and task-control
mutations can become the next bottleneck. Preserve the existing locality
benefit of `Runtime.next`; replace its globally guarded slots with worker-owned
queues and independently synchronized notification inboxes.

- A worker owns its normal ready deque. Remote completions enqueue stable task
  handles/notification records into its inbox; idle workers steal bounded
  batches through a proven deque implementation. Prefer an established
  implementation compatible with task lifetime rules over a new unsafe deque.
- Task control arbitrates running, parking, parked, runnable, cleaning and done.
  A notification sets pending work; only the suspension-complete transition
  permits another worker to dispatch the coroutine. Deduplicate ready entries
  with state/generation checks. Joining, cancellation and child failure use the
  same control interface; owner-only stack/context writes remain owner-only.
- Start with a short task-directory lock to obtain a strong handle; release
  before control/queue work. Then shard lookup by task ID or remove lookup from
  common wake paths by retaining handles directly. Reclamation waits for queued
  handles, timer records, joiners and inspection references to drain.
- Split timers into a dedicated lock. Extract due records under it, release it,
  then notify tasks. Worker-owned timer heaps can follow if measurement justifies
  them. Deadline generations preserve sleep/select cancellation behavior.
- Replace global `IDLE`/`WAKING` coordination as needed with per-worker park
  tokens or sequence counters. Register intent to sleep, recheck local queues
  and inboxes, then wait. Producer publication racing idle entry must cause
  either observation of work or a wake. Avoid notify-all for one runnable task.
- Preserve deterministic scheduling/replay as an explicit execution mode.
  Freeze/check that mode outside the normal scheduling loop so ordinary workers
  do not acquire `SCHED` merely to discover that controls are disabled.
- Shutdown/reconstruction must settle running tasks, pending notifications,
  timers and external completions before reclaiming task records or worker
  state. Keep bounded external/blocking-job admission as a separate policy.

**Proof:** independent pairs and wake-heavy rendezvous workloads no longer acquire
one runtime-wide scheduling lock per transfer. Verify no task runs concurrently
on two workers, no work is lost during idle transitions, bounded timer lateness,
correct replay, and clean shutdown under bursts/cancellation. Compare migration,
cache misses, queue depth and p99 latency at equal work.

## 7. Split remaining domains and remove long critical sections

Audit every remaining `rt_call` outside `UNLOCKED`, every shared `UnsafeCell`,
`static mut` registry and runtime callback. Record an owner, guard and GC mode
for each entry before removing its legacy wrapper.

- Scopes: use per-scope membership/failure state and stable child handles. Copy
  cancellation/notification work out under the scope guard, release it, then
  act. Never settle/join children while holding the scope registry lock.
- Inspection/live registries: short registration locks and stable snapshot
  handles; copy metadata under guards, format JSON after releasing them. A
  consistent snapshot may use a GC/quiescence epoch explicitly. Registry locks
  cannot pin unrelated mutators behind slow serialization or compilation.
- Output: prepare rooted/owned bytes, release runtime exclusion, then use a
  bounded writer or blocking operation. Return managed errors after reentry.
  Preserve per-stream write ordering and current flush semantics. Define
  backpressure and cancellation settlement; an unbounded output queue simply
  converts blocking into memory growth.
- Atom/cell and other intrinsics: classify physical mutability and ownership
  before bypassing guards. An atomic word containing a managed pointer also
  needs a rooted load/store protocol. Do not assume persistent containers are
  physically immutable wherever their implementation extends shared storage.
- Allocation accounting: retain existing local pools. If `ALLOCATED.fetch_add`
  or another shared counter dominates, accumulate bounded worker-local debt
  and publish in blocks. Bound aggregate threshold overshoot by worker count
  and credit size, and flush debt at collection. Keep accurate live statistics
  separate from approximate collection pressure.

**Proof:** a blocked output pipe does not stall unrelated channels/timers;
scope failures still cancel/settle correctly; inspection and finalization remain
safe under GC; registry and root memory plateau during long runs. Compare cache
contention after removing each hot global counter/lock.

## 8. Specialize queues only after the protocols are stable

Per-channel locking provides independence; a single busy channel still shares
queue state. Optimize that case from measured profiles:

- Preallocate a bounded ring and co-locate each message's charge with its slot.
  Avoid two growable queues and managed wrapper allocations where scalar
  replacement or internal ABI changes can preserve public `Result` semantics.
- Consider an MPMC ring with per-slot sequence/publication state. Specify
  reservation, initialization, publication, consumption and reuse ordering;
  preserve the channel's documented FIFO guarantees. Account for counter wrap.
  A producer paused after reserving a slot can obstruct a consumer; atomics do
  not by themselves establish lock-free progress or better tail latency.
- For GC, distinguish reserved/uninitialized slots from published pointer slots.
  Reserved payloads remain rooted by their producer; the collector must never
  interpret uninitialized storage as a managed pointer. Close, cancellation
  and waiter registration still need a coherent protocol around the ring.
- Use SPSC/MPSC variants only when endpoint ownership proves the topology for
  the complete lifetime. Shared endpoint values cannot silently become SPSC;
  any upgrade/fallback must exclude concurrent operations safely.
- Keep payload/queue mutation and wait/close control coordinated. A successful
  fast enqueue must notify a registered receiver; the check/register/recheck
  protocol must cover the atomic ring too. Avoid a fast path that only performs
  well when no consumer ever blocks.
- Apply cache-line separation to independently written indices and worker
  records where measured. Bound ring capacity, waiting descriptors, retained
  pages and credit caches. Report memory cost alongside throughput.

**Proof:** compare against the per-channel lock implementation for SPSC, MPSC,
SPMC and MPMC; one hot queue and many independent queues; empty/full oscillation,
oversubscription, close/cancel/select and GC. Adopt a specialization only with
repeatable benefit and unchanged semantic/correctness tests. Keep the simpler
implementation available until the alternative meets those gates.

## Verification and performance gates

For each slice, add focused regressions at the ownership/scheduling boundaries
it changes, plus small native state-machine models where atomic ordering or
multi-party completion is involved. Retain reproducing seeds, worker counts,
source/binary hashes and failing traces. Do not weaken assertions to hide a race.

Use existing self-checking [stress programs](tests/stress) and
[stress runner](crates/lang_cli/tests/stress.rs), extending them with:

- Buffered and capacity-zero channels; prompt and batch operations; tiny,
  medium and large pointer-bearing messages; many independent pairs and one
  hot channel; multiple producers/consumers with per-producer order checks.
- Count/byte admission, mixed sizes, lease release, close/discard, duplicate
  selection cases, two-selection rendezvous and cancellation at every wait
  registration/commit/suspend/cleanup boundary.
- Forced GC with pending notifications, native descriptors, unpublished staging,
  returned pointers, task migration and worker idle/entry/exit transitions.
- Mixed CPU, allocation, channels, timers, inspection and slow output; bursty
  overload; task-ID reuse/generation races; repeated restart and shutdown.
- Long-lived tasks repeatedly visiting channels, with active registrations,
  roots, native records and post-GC resident memory returning to a steady bound.

Run 1/2/3/4/8 workers and oversubscription separately; the existing quick matrix
is a starting point, not the complete cross-product. Combine optimized JIT,
verification, forced GC and binary MIR explicitly. A default `stress_soak`
without `LANG_STRESS_SECS` does not run a soak.

Useful existing commands for future implementation work:

```sh
cargo build --release --locked --offline
cargo test -p lang_runtime --locked --offline
cargo test -p lang_cli --test stress stress_programs --locked --offline -- --nocapture
LANG_STRESS_SECS=300 LANG_STRESS_SEED=1 cargo test -p lang_cli --test stress stress_soak --locked --offline -- --nocapture
python3 bench/baseline.py --out reports/benchmarks/lock-before --reps 5
LANG_RUNTIME_STATS=1 target/release/lang run --release --bench --workers 4 bench/independent_channels.lang
git diff --check
```

Choose new baseline output names for subsequent slices rather than overwriting
retained evidence. Crate unit tests alone provide limited scheduler/GC coverage.
The existing baseline records elapsed medians; extend the load harness for tail
latency and contention attribution. Run soak tests and benchmarks separately.

Before declaring the lock split complete, require:

1. No global runtime lock acquisition on ordinary independent-channel queue
   operations, and no global scheduler lock per channel wake after slice 6.
2. Independent channels and task-owned work demonstrably overlap on multiple
   workers. At sufficiently large fixed work, four independent pairs improve
   throughput beyond two workers on a machine with four available physical
   cores; explain and profile any plateau instead of assuming lock removal
   guarantees linear scaling.
3. Compare one-worker and CPU-only control workloads to baseline. Investigate
   repeatable regressions above 5% outside measured noise before accepting a
   default path. This is a review threshold, not a claimed achieved result.
4. Under equal offered load below saturation, p99 channel latency and timer
   lateness do not regress materially. Report saturation throughput separately;
   median total execution time cannot establish this property.
5. Exactly-once completion, documented ordering/fairness, ownership/byte/lease
   accounting and bounded memory hold through cancellation, collection and
   shutdown. Model proofs, native stress and platform coverage are reported
   separately with their limits.

Land each slice with its measured before/after evidence and remaining legacy
lock users. Update architecture documentation when the implementation changes.
Keep GC global pauses explicit; removing routine runtime serialization does not
make the collector concurrent.
