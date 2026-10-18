# Channel topology and specialization investigation

Reviewed 2026-10-16 at `ed0f0a11b089c94271e46bd454eb2a779cb2469e`, including
the existing uncommitted wait-generation edits in `task.rs`. This is an
investigation and implementation proposal. Production compiler/runtime sources
were not changed. The native SPSC code accompanying this report is a restricted
queue experiment, not a complete channel or a production-ready implementation.

Final source check: `main` advanced to `bfce97d` (wait-registration lifetime),
and further uncommitted atomic task-state edits appeared after measurement.
The measurements below retain their original snapshot and do not measure those
later edits. Atomic task-state reads alone do not reserve a task against waking
or migration, so the locality/proof conclusions still apply.

## Findings and recommended direction

Our channels are **N:M (MPMC)**: sender and receiver handles can both be shared
across tasks. Every message goes to one receiver; this is competing consumption,
not broadcast. A particular instance may have 1:1, N:1 or 1:N use, but the runtime
currently uses the same general channel structure for these cases.

**Automatic specialization is feasible without changing ordinary channel syntax.**
Add an analysis that proves endpoint use by task instance, conservatively keeps
the general implementation for unknown uses, and chooses hidden construction
and operation variants. Prioritize proven SPSC and producer-local batching.
Add stronger language contracts only where they demonstrably improve coverage.

| Candidate | Assessment | Necessary guarantee |
| --- | --- | --- |
| Automatically inferred SPSC | High priority after lock/GC prerequisites | One producer and one consumer operation stream per channel instance |
| Producer-local staging and bulk queue commits | High priority | Exclusive staging ownership and bounded capacity credits |
| MPSC/SPMC variants | Worth measuring after SPSC | Only one side requires multi-party coordination |
| Task-local channel without atomics | Worth a small compiler optimization | All accesses, control operations and aliases confined to one task |
| Two different tasks permanently on one worker | Conditional, later | Worker confinement, stealing exclusion and an ownership-transfer protocol |
| Both tasks happen to be on the same worker now | Unsafe basis for lock elision | A snapshot provides no future exclusion |
| Other endpoint currently parked/not accessing | Usually poor tradeoff | Exclusive access reservation, wake/steal/migration interlock and revocation |
| General MPMC atomic queue | Profile-driven | Queue publication plus complete wait/select/close/GC protocols |

## What is implemented now

[The specification](../spec/05-tasks-and-channels.md#channel-types-and-endpoints)
allows shared sender and receiver endpoints. In
[the standard library](../crates/lang_hir/std/channel.lang), `create` constructs
one channel and returns two typed handles through `identity`; `send` and `recv`
borrow `&Self`. There is no owned, nonshareable single-producer endpoint type.
[Ownership checking](../crates/lang_typeck/src/ownership.rs), in `affine_in`,
explicitly treats these handles as shared runtime objects, independently of
whether their message type is affine. One endpoint variable can therefore feed
many producer/consumer tasks.

[The conservation stress program](../tests/stress/channel_conservation.lang)
uses 4 producers/3 consumers on rendezvous channels, 3/4 at capacity one, 6/2 at
capacity sixteen and 2/6 at capacity 1024. This is executable MPMC coverage.
[The affine-message fixture](../tests/run/channel_affine_shared.lang) also shares
the sender of a channel carrying owned buffers between two producers.

[`ChanData`](../crates/lang_runtime/src/channel.rs) has the general queue,
multiple sender/receiver wait queues, watchers, and producer-indexed batch
staging. Each successful fast operation still acquires its channel's atomic
ownership byte through compare-and-swap. The byte is a local lock. Contention,
waiters, byte-budget sends and publication can trigger global-lock slow paths.

Changes already landed while lock optimization is underway include lock reason
attribution (`ec88d1e`), unlocked buffer operations and batch-lease reads
(`3958d77`), worker-local publication of allocation pressure (`17472a8`), and
stale-wait generation checks (`ed0f0a1`). This report accounts for those changes;
it does not propose doing them again. The collector and scheduler still use
global runtime exclusion in the inspected source. Coordinate new work with
[LOCK_OPTIMIZATION.md](../LOCK_OPTIMIZATION.md).

## Measurements

### Current language runtime

Built with `cargo build --release --locked --offline`, then copied the executable
to a private temporary path so concurrent builds could not replace it. Runtime
source hashes were unchanged during the measurement window; the dirty source
diff, start/end revisions and binary hash are retained in
[language.json](channel-topology-investigation/language.json).

Each topology fixture sends exactly 200,000 integer messages and checks count
and sum. Fresh sequential processes, optimized host and JIT, one warmup plus
three measured runs; `--bench` excludes compilation. No CPU affinity/frequency
control for these language runs. These short probes do not measure tail latency.

| Queue use | Capacity | 1 worker | 2 workers | 4 workers |
| --- | ---: | ---: | ---: | ---: |
| 1:1 | 1 | 21.919 ms | 29.634 ms | 28.647 ms |
| 1:1 | 64 | 11.201 ms | 11.145 ms | 11.760 ms |
| 1:1 | 1024 | 10.410 ms | 9.689 ms | 10.287 ms |
| 1:1, 512-byte budget | 1024 | 11.720 ms | 15.464 ms | 15.622 ms |
| 4:1 | 1024 | 10.573 ms | 10.720 ms | 10.473 ms |
| 1:4 | 1024 | 10.470 ms | 11.236 ms | 10.486 ms |
| 4:4 | 1024 | 10.552 ms | 19.152 ms | 23.218 ms |

The budgeted case admits 64 eight-byte integers, so its timing cannot isolate
byte-accounting cost from effective capacity. Source evidence separately shows
that budgeted sends miss the current fast path. These runs demonstrate that
using more workers does not automatically improve a small single-channel stream.

Separate instrumented four-worker runs recorded:

- Capacity-one 1:1: 266,665 global channel-slow acquisitions and 133,338 task
  resumptions. This is substantially a scheduling/wakeup workload.
- Capacity-1024 1:1: 777 global channel-slow acquisitions and 394 resumptions.
  Avoiding the local ownership CAS and per-message runtime overhead can matter
  even when global-lock acquisitions are already rare.
- Capacity-1024 4:4: 76,741 global channel-slow acquisitions; 67,566 lock
  acquisitions succeeded after spinning and 840 after parking. Those spin/park
  counts cover all reasons, overwhelmingly channel slow paths in this run.

Do not interpret sampled lock-hold estimates as a reliable percentage of elapsed
time, especially low-count startup reasons. Counter runs were separate from the
uninstrumented timing medians.

The existing messaging benchmark gave these medians of its printed figures:

| Operation | 1 worker | 2 workers | 4 workers |
| --- | ---: | ---: | ---: |
| Prompt, capacity 256 | 52 ns/message | 48 ns/message | 55 ns/message |
| Batch, capacity 4096/max batch 256 | 51 ns/message | 32 ns/message | 35 ns/message |
| Prompt round trip | 264 ns/round | 326 ns/round | 334 ns/round |

The last row is a request/reply round, despite the fixture's `ns/message` label.
Batch and prompt use different capacities and have different publication
semantics, so this is directional evidence, not an isolated batching effect.

### Queue-only native experiment

To isolate the proposed mechanisms, compared a fixed ring behind a per-channel
CAS lock, Crossbeam's general `ArrayQueue`, and a restricted SPSC ring with
producer-owned write and consumer-owned read indices. SPSC caches the opposite
index and uses acquire/release publication. Both custom rings also support up
to 32 messages per commit. Every consumed value is checked in order, plus a final
checksum. No per-message allocation occurs.

Producer pinned to CPU 1, consumer to CPU 2, different physical cores on this
Ryzen 7 8745HS. Two million transfers; one warmup plus three measured fresh
processes. Results are elapsed nanoseconds per transferred message, covering both
ends. Full ranges, commands and source/binary hashes are in
[native.json](channel-topology-investigation/native.json).

| Primitive, exercised with one producer/one consumer | Capacity 64 | Capacity 1024 |
| --- | ---: | ---: |
| Local CAS lock, one item/commit | 33.515 ns | 27.988 ns |
| General MPMC `ArrayQueue`, one item/operation | 35.685 ns | 33.685 ns |
| SPSC indices, one item/commit | 4.656 ns | 3.440 ns |
| Local CAS lock, up to 32 items/commit | 4.447 ns | 3.690 ns |
| SPSC indices, up to 32 items/commit | 2.848 ns | 1.075 ns |

**Interpretation:** topology and amortization are worth pursuing. On this probe,
SPSC avoids repeated exclusive ownership transfers on a shared lock word.
Batching makes the locked queue competitive with unbatched SPSC. An atomic MPMC
algorithm is not automatically faster than a short lock for this SPSC workload.

These figures are not expected language speedups. The native probe busy-spins;
it has no cooperative parking, close, select, cancellation, byte budgets, managed
pointer tracing or result wrappers. It tests the general queue only under SPSC
load, so says nothing about its relative MPMC performance. The small custom
prototype has not been model-checked or sanitizer-validated. Fifteen additional
smoke runs check capacity 1/2/8, batch limits larger than capacity and a
nonmultiple message count. Background machine activity was uncontrolled.

## Automatic selection: proposed compiler analysis

### Preserve ordinary channel syntax

Keep `channel.create` and the existing endpoint types. Prove properties of each
channel instance, then select internal queue/control implementations. Separate
these facts rather than collapsing them into one mode:

- Producer and consumer cardinality: none, one proven execution stream, many,
  or unknown. Unknown always chooses the general implementation.
- Escape/alias provenance: which values and aggregate fields can refer to the
  endpoint, and where operations through those references execute.
- Execution placement: one task, migrating tasks, or proven worker confinement.
- Protocol requirements: buffering/rendezvous, byte budgets, selection and
  batching. A proven 1:1 rendezvous needs a rendezvous protocol, not a ring.

One producer task may perform millions of sends through several aliases and
still be single-producer. One closure body spawned a hundred times is not.
Exactly one consumer is a separate property from the producer count. Close,
inspection, GC and cleanup are additional accesses even if they do not count
as message producers or consumers.

### Pass location and existing infrastructure

[HIR ownership analysis](../crates/lang_typeck/src/ownership.rs) already records
closure captures and propagates `transfer_params` summaries through forwarding
helpers. These are useful building blocks; `Transfer` by itself does not prove
single-producer use. Some existing analyses intentionally skip stdlib bodies,
whereas this pass needs trusted summaries or analysis of those wrappers.

Start on typed HIR, before closure environments and endpoint aliases become
mostly raw pointer loads/stores. Attach provenance and cardinality metadata to
channel construction and operation sites during
[MIR lowering](../crates/lang_mir/src/lower.rs). Refine known closures/calls after
[inlining](../crates/lang_mir/src/inline.rs), before code generation, without
depending on an inlining heuristic for correctness. The driver already exposes
that point in the private `Session::lower_to_mir` method, immediately after
optional inlining and before code generation.

The retained [MIR dump](channel-topology-investigation/independent_channels.mir)
shows `chan_new`, `task_spawn`, `chan_send`, `chan_recv`, known function identities
and closure environment loads in the existing independent-pairs program.
It demonstrates retained analysis inputs, not a successful topology proof.

### Concrete analysis steps

1. Assign a symbolic channel origin at construction. Track sender and receiver
   projections separately through bindings, records, tuples, options/results,
   closure captures, arguments and returns. In particular, `identity(s)` in the
   stdlib preserves origin while changing endpoint role; it does not create a
   new channel. Whole-record capture must remain field-sensitive: capturing
   `ends` in both tasks does not imply both tasks send and receive.
2. Build function summaries per endpoint/field path: sends, receives, closes,
   selects, stores/returns/escapes, synchronous callback calls, and spawned
   callback multiplicity. Propagate to a fixed point over direct calls and
   known closures. Unknown dynamic/foreign calls conservatively escape affected
   origins. Bounded analysis limits must degrade to general channels, not
   assume exclusivity.
3. Distinguish task domains. `task.scope` calls its body in the current task;
   `TaskScope.spawn` creates a new task. A loop within a task does not multiply
   producers; spawning within a loop can. Repeated, recursive or higher-order
   invocation of the same spawn site also creates distinct task instances.
   Summaries must record this multiplicity instead of using lambda IDs as task
   identities. Initially prove only bounded, simple cases; widen the rest.
4. Analyze per channel instance with bounded context sensitivity. Four calls to
   `pair()` each constructing a fresh channel can each be SPSC. Four calls to
   a producer helper capturing one channel are multiple producers on that
   channel. A repeated allocation site represents fresh instances, not a
   singleton; inability to separate them loses an optimization, never safety.
5. Account for phase separation. After a joined producer transfers all endpoint
   uses to a successor, a channel can remain single-writer-at-a-time if no old
   alias is used again and state is safely transferred. Defer this temporal
   proof until basic lifetime-wide SPSC analysis works. Cardinality of active
   operations alone is insufficient without a handoff protocol for caches.
6. Choose hidden constructor flags or specialized intrinsics. Keep a general
   fallback for escaping endpoints, unresolved calls and mixed modes. A stdlib
   `Sender<Int>.send` instance may serve both SPSC and MPMC channels: specialize
   call contexts or dispatch by the instance's mode. Never rewrite that entire
   type-based monomorphization as SPSC just because one caller qualifies.
7. Emit an optimization explanation: creation site, inferred topology, selected
   implementation, and the reason an optimization was refused. This makes
   coverage and conservative fallbacks reviewable without user annotations.

For example, the source of `bench/independent_channels.lang` creates four fresh
channels. Each `pair()` uses one spawned producer and receives in its owning
task. A context/field-sensitive analysis should eventually prove four separate
SPSC instances. A sender captured by a loop of four `spawn` calls must classify
as MPSC even when every closure has the same function identity.

The experiment's topology fixtures use loops/maps to construct tasks. A first
analysis may conservatively reject even the bounds-one fixture; coverage of
simple lexical spawn cases should land before constant-loop/higher-order proofs.

### Does the language need changes?

Not for the first version. Existing structured tasks, typed captures and visible
constructors provide enough information for simple nonescaping cases. Two useful
extensions, if coverage measurements justify them, are:

- Checked callback summaries describing no escape, current-task execution and
  invocation multiplicity. A `once fn` restricts the callback's call count; it
  does not prove that captured shared endpoints have no aliases in other tasks.
- Optional affine endpoint capabilities or checked scoped pipeline builders
  that infer exclusive producer/consumer ownership. These can preserve proof
  across library boundaries. Making existing shareable endpoints affine by
  default would change accepted programs and is unnecessary for automatic
  optimization. Keep existing code valid and infer whenever proof is available.

Do not implement automatic selection by counting currently observed task IDs,
endpoint variables or native reference counts. Current implicit sharing does not
register every alias, and a later task may begin using an endpoint. Starting in
SPSC mode and upgrading on a second user is possible only with entry exclusion,
in-flight-operation draining and transfer of private staging/index caches. It
adds mode checks and protocol complexity to every operation; static proof with
an MPMC fallback is the better initial investment.

Live reload is a proof boundary. The current driver disables inlining for live
dispatch; topology assumptions can also become invalid when functions change.
Start with general channels under watch/inspection replacement, or attach
dependency fingerprints and reconstruct affected channels before changing their
use. Existing specialized instances must never silently accept incompatible
code. Serialize required metadata in binary MIR, validate its version, and
preserve semantics under worker-count overrides. Topology proof does not imply
worker affinity.

## Same-worker and temporarily inactive peer optimizations

[Scheduling](../crates/lang_runtime/src/task.rs), in `make_ready`/`take_ready`,
already keeps a newly woken task in the waker's next slot for locality, but idle
workers can steal it. Tasks migrate at suspension points. Looking at the last
worker ID or the peer's parked state cannot exclude a simultaneous wake/steal.

Three cases need different treatment:

1. **All channel uses in one task:** a nonescaping origin can use a local queue
   with ordinary indices. The compiler must still preserve bounded-channel
   behavior: a full blocking send or empty blocking receive may deadlock;
   specialization cannot invent progress or erase cancellation checks. Keep
   GC roots, close and inspection semantics correct.
2. **Different tasks confined to one worker:** a local implementation is viable
   if all data/control operations run on that worker, stealing is prohibited,
   and remote requests are marshalled to it. Moving the channel requires a
   safepoint handoff that drains operations and publishes its state. This is a
   scheduler ownership feature; measure its loss of parallelism/load balance.
3. **Migrating tasks with a peer momentarily inactive:** a fast ordinary write
   requires an exclusive access lease. A worker may reserve the peer's parked
   state against resumption, or acquire channel ownership for several operations.
   Both require synchronization and careful progress rules. A raw state check
   followed by an unlocked write has a check/use race. Prefer bounded batch
   ownership and private staging to a second speculative scheduling protocol.
   Reserving exclusion with a CAS on every access can simply recreate a
   per-channel lock; amortization needs a bounded ownership interval across
   several accesses, with release before suspension, callbacks or collection.

A migrating SPSC pair still benefits from SPSC indices: there remains only one
writer on each side, and scheduler handoff publishes that side's state. It does
not need permanent affinity. Keep cached indices attached to the endpoint/task
ownership context, not a worker-local slot that another producer can inherit.

## Atomic indices and committing multiple operations

SPSC changes the synchronization problem. Each side owns one index; ordinary
payload writes become visible through a release publication that the other side
observes with acquire. Cached opposite indices avoid reloading shared state
until potentially full/empty. This is a practical established design:
[rtrb's source](https://docs.rs/rtrb/0.4.0/src/rtrb/lib.rs.html) uses cached indices
and release publication. Its endpoints are movable but not concurrently shared.

In MPMC, several producers must reserve slots and publish initialized contents,
and several consumers arbitrate removal. Crossbeam's `ArrayQueue` uses per-slot
stamps and atomic reservation; local cached SPSC indices cannot replace those
steps. A queue algorithm also does not implement our fibers, waiter registration,
two-selection rendezvous, cancellation or managed rooting.

Bulk operations are useful independently of topology. Reuse the existing batch
channel contract for delayed publication; move append staging into task-owned
pages and reserve bounded credits. A bounded `recv_many` on prompt channels can
drain already-published messages under one acquisition, return available work
after the first item, and amortize result/wakeup overhead without delaying sends.
[Tokio's `recv_many`](https://docs.rs/tokio/latest/tokio/sync/mpsc/struct.Receiver.html#method.recv_many)
is a useful reference for that receive-side contract; it is not our runtime's
implementation or an integration recommendation.

Ordinary prompt sends must remain promptly observable. The compiler cannot
silently publish only once after a loop if intermediate `try_recv`, select or
task interaction can observe the difference. Even delaying publication until
yield can change inter-worker observations. Fuse only explicit bulk/batch APIs
or code regions with a complete unobservability proof. Capacity, byte credits,
fairness, cancellation and flush-before-park still apply to bulk operations.

## Suggested implementation sequence

1. Coordinate with lock slices 1–4: stable task/wait records, safe mutator/GC
   handshake and independent channel state. Do not attach SPSC data paths to
   unsafe task-state snapshots or skip the wake/register/recheck protocol.
2. Add a report-only topology pass for simple nonescaping lexical channels.
   Test positive 1:1, N:1, 1:N and N:M cases; negative cases with aliases,
   aggregates, repeatable closures, nested/repeated spawns, recursion, escaping
   endpoints and live reload. Measure coverage on examples, stdlib pipelines
   and application fixtures before changing language syntax.
3. Implement a proven SPSC buffered ring behind the same endpoint API. Preserve
   control semantics and a general path for unsupported byte/select/rendezvous
   combinations. Add per-origin optimization reports and differential tests
   against forced-general mode. Later extend the single-side variants.
4. Improve producer-owned batches/credits and add bounded bulk receive if useful.
   The native results suggest this can provide much of the synchronization
   saving even before all compiler topology proofs are available.
5. Consider worker-confined channels only alongside explicit scheduler ownership
   support and load-balancing measurements. Profile MPMC atomics after local
   locks/wakeups are independent; retain the simpler implementation as a control.

Acceptance must include pointer-bearing payloads, forced GC, exact ownership on
cancel/close, selection races, same-side sequential migration, escaped aliases,
binary MIR and reconstruction. Measure low-load p99 latency as well as throughput
and memory under backpressure. Existing native integer probes do not establish
these properties. No model checking, sanitizer run, tail-latency test or production
specialization was completed in this investigation.

## Reproduction and retained artifacts

```sh
cargo build --release --locked --offline
python3 reports/channel-topology-investigation/measure.py
cargo build --manifest-path reports/channel-topology-investigation/Cargo.toml --release --locked --offline --target-dir /tmp/lang-channel-topology-target
python3 reports/channel-topology-investigation/measure_native.py
cargo fmt --manifest-path reports/channel-topology-investigation/Cargo.toml -- --check
```

Runners overwrite this investigation's JSON; copy results to a new evidence
directory when comparing revisions. [Language runner](channel-topology-investigation/measure.py),
[generated fixtures](channel-topology-investigation/fixtures),
[native source](channel-topology-investigation/src/main.rs),
[native runner](channel-topology-investigation/measure_native.py) and the nested
Cargo lockfile are retained. The nested crate has its own workspace and uses a
temporary target directory; it does not change the language workspace's dependencies.
