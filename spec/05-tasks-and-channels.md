# Tasks, channels, and supervision

Status: draft 0 proposal, incorporating the accepted execution model.
See [the specification index](../SPEC.md).

## One task primitive

A task runs sequential code, may suspend, and eventually returns a value,
fails, or is cancelled. Tasks can be short requests or long-running activities.
They are not OS processes, and do not each require an OS thread or mandatory
inbox. Ordinary calls remain ordinary calls within the current task.

The runtime schedules many tasks on a configurable number of workers. One
worker is a supported mode, not a semantic restriction or determinism guarantee.
Task isolation/transfer rules apply even on one worker. Thread-affine foreign
resources require a pinned execution context; they cannot silently migrate.
Spawn checks both closure captures and returned values for safe transfer, with
scope bounds retained. A task-local value cannot be smuggled back to the parent
through `join` just because it was created inside a child task.

Tasks are stackful native tasks, with bounded guarded
stack reservations, demand-backed storage where supported, explicit root
tracking, and compiler-generated budget/cancellation polls. This preserves
ordinary direct calls across suspension. Memory per task is measured, including
stack commitments, captures, inboxes, and roots. No fixed task-size promise.
Alternative stack representations may replace this without changing source
semantics. Migration requires a proven-safe suspension boundary and transferable
task state, not merely moving a saved stack pointer to another thread.
Implementation: each task snapshot (`badger inspect tasks`, `inspect.tasks()`)
has `memory`: `stack_reserved` and `stack_resident` (address space and
pages in use), `captures` (its closure object), `roots` (values the
runtime holds for it), and `channels` (registrations).

Runtime I/O parks instead of blocking a worker. Opaque blocking calls execute
through a bounded foreign/blocking pool. The compiler checks long loops and
recursive calls against a scheduling budget. Scheduling is fair at supported
safe points, not a hard wall-clock deadline or arbitrary native preemption.
Uninstrumented foreign code has a separate contract.

Cancelling a task that waits on an operation outside the scheduler
(**Implementation**, tested by `tests/run/cancel_matrix.bdg`): a
subprocess is killed (the task waits for it to exit, then leaves); a
SQLite query is interrupted and a native module's async call withdrawn
(its `cancel` hook runs, a late result's handles are released); a blocking
file or native call that owns its arguments is abandoned: the task leaves
at once, and the result, when the call ends, is dropped with its handles
released (an external effect such as a write may still happen). A blocking
foreign (`extern "C"`) call may use the task's memory, so the task waits
for it to return before the cancellation takes effect. Under `--record`,
every operation is waited for, so the bundle holds its outcome.

## Scopes, children, and results

```badger fragment
use scope <- task.scope()
let first = scope.spawn(fn() { load_first() })
let second = scope.spawn(fn() { load_second() })
let (a, b) = scope.try_join(first, second)?
let result = combine(a, b)
ok(result)
```

The example assumes the loaders return `Result` and the callback's error type
accepts their errors. `join()` yields the child's returned value; it does not
flatten a returned `Result` automatically. Task handles retain their scope
brand and cannot outlive it. `try_join` observes both fallible children, returns
their successful values together, and on the first expected error cancels and
settles the selected unfinished sibling before returning that error. It accepts
one common error type or a declared direct conversion target. A collection form
`try_all` applies the same rule to homogeneous tasks. This is explicit result-aware
coordination; `spawn` does not reinterpret every arbitrary return value as failure.

A normal scope owns every spawned child. The `task.scope` body returns
`Result<R, E>`: `ok` waits for children, while `err` cancels and settles its
children before returning that error. This policy concerns the scope body's
result, not arbitrary values returned by spawned children. Infallible scoped
work simply ends with `ok(value)`.
An unhandled task defect fails the scope, cancels remaining work, runs cleanup,
and propagates to its owning task. Expected `Result.err` values are ordinary
task results; callers must inspect them. Discarding a task containing an
unhandled must-use result is diagnosed. Dropping a handle never detaches a task.

`scope.capture(fn() { ... })` contains a child's failure and returns a task of
`Exit<T>`; its join yields `.returned(T)`, `.failed(PanicInfo)`, or
`.cancelled(CancelReason)`. This is the explicit observation boundary used by
worker management and isolation tests. Its child cleanup still runs. There is
no unowned global spawn in draft 0.

A server owns sibling request scopes. One request failure does not fail its
siblings by default. Within one request, the normal task scope is fail-fast.
Scopes may register deterministic resource cleanup; shutdown first stops
admission, cancels/drains owned work according to policy, and then releases
resources in a valid dependency order.
Fallible resource helpers declare how cleanup errors enter their typed result;
system cleanup aggregates them as `system.Error`. An unhandled failure of task
scope machinery is a task defect. An original error remains attached to any
escalated cleanup failure rather than being silently replaced or discarded.
Implementation: a cleanup failing after a defect is a note on the defect
(`cleanup failed: ...`); one failing (or exceeding the grace period) while
a scope returns an error is the defect, with the error as a note (`while
returning the error: ...`).

Timeouts request cancellation at safe points. Cleanup can delay completion and
foreign calls may remain outstanding. A timed-out caller does not prove its
remote side effect was rolled back. A runtime-controlled cleanup grace period
escalates an uncooperative service instead of silently freeing resources still
in use. Cancellation is not swallowed as a normal recoverable `Result` error.

## Channel types and endpoints

`Channel<T>` is a typed bounded queue. `BatchChannel<T>` additionally permits
deferred publication. They return sender/receiver endpoints; supported endpoints
can be shared across tasks and coordinate access internally. Multiple receivers
distribute messages, rather than broadcast them. Each delivered message has one
consumer. Broadcast is a separate library abstraction with its own retention
policy. Payload `T` must be transferable, with any region bound preserved.

Constructor functions (`channel.create`, `channel.batch`, and
`channel.batch_with_policy`) take complete options records, usually by
updating `channel.defaults` or `channel.batch_defaults`. Construction validates limits
and returns `Result<Endpoints<T>, channel.ConfigError>`. `channel.open<T>` and
`channel.open_batch<T>` are scoped helpers that close and account for pending
data when their callback ends. Unscoped queue values are managed allocations;
GC of a last sender is not a timely end-of-stream signal. Close explicitly or
use the scoped helper.

The core endpoint operations are:

| Operation | Contract |
|---|---|
| `sender.send(value)` | Wait for capacity and promptly publish; success transfers ownership |
| `batch_sender.append(value)` | Wait for capacity and accept into runtime-owned staging |
| `batch_sender.flush()` | Publish accepted messages preceding this flush for this producer |
| `receiver.recv()` | Wait for one message; return `.none` after closure and drain |
| `receiver.recv_batch()` | Receive a leased sealed batch when supported; the lease is linear (release it) |
| `sender.close()` | Close admission and initiate publication/drain of accepted data |
| `try_send` / `try_recv` | Never park; distinguish full, empty, and closed states |

Fallible sends return the unsent owned value inside `SendError<T>`. Waiting
operations cooperate with cancellation; a send/select reservation that has not
committed is released on cancellation. After ownership commits, cancellation
does not silently retract a message. Blocking-until-publication and waiting for
processing acknowledgment are different operations. Neither means durability.

Capacity zero selects a rendezvous `Channel`: send commits when matched with a
receiver. Batch channels require positive capacity. Options also allow a byte
budget; a single oversized message returns an error rather than waiting forever.
Known direct byte storage can be charged precisely; other payloads use their
declared/derived admission size. This is a queue budget, not a universal bound
on everything the application retains after delivery. Runtime heap and pooled
page budgets are separate limits.

### Replies and request/response

`channel.reply<T>()` makes a one-shot reply: `{ sender, receiver }`. The
sender sends one value (a second send, or one after `close`, returns
`SendError`); `receiver.recv()` waits for it and returns `.none` if the
sender closed without one. A reply is one small object — no queue — so a
request carries its own reply sender:

```badger fragment
type Lookup = { key: Int, reply: channel.ReplySender<String> }
let tag = channel.call(lookups, fn(r) { { key: 42, reply: r } })   // Option<String>
```

`channel.call(to, make)` sends `make(reply_sender)` and waits for the
reply. To have many requests in flight from one task, send them with
their own replies first and receive the replies after; no task per
request is needed.

### Patterns that scale across workers

Each worker runs its own queue; a woken task prefers its waker's worker,
so tasks that only talk to each other tend to stay on one worker, while
idle workers steal from busy ones and independent groups run in parallel. Throughput across
workers is limited by what is shared:

- A single actor serving every request is a serial bottleneck: shard it
  by key (`channel.route(shards, key)` picks a shard's sender) so each
  shard is an independent actor.
- Prefer replies (`channel.call`, or replies sent ahead) to a task per
  request when the request itself is the work.
- Batch small messages (`channel.batch`, or lists) when a consumer does
  little per message.
- Keep data a task works on owned by it (send it, don't share it):
  shared mutable state (`Atom`) serializes its users.

## Selection and ordering

`channel.select` accepts a runtime list of typed homogeneous operations and
returns the winning operation/value, with optional timeout. Heterogeneous
selection uses an explicit enum to package outcomes, or typed combinators that
produce a known enum. Do not turn heterogeneous payloads into implicit `Any`.
Selection reserves and commits exactly one operation; losers release capacity
and retain their ownership. Dropping a losing send must not consume its payload.
The default selection policy rotates fairly; explicit priority is opt-in.

Implementation note: plain `channel.select` (whose operations may include
sends) and `channel.on_send` require `T: Share`, so a losing send leaves the
caller's shareable value untouched. Affine payloads use `channel.select_keep` or
`channel.on_send_keep`, which hand a losing payload back to the caller. A
send operation on a closed channel resolves `select` as `.closed(index)`
(nothing is sent; the value stays with the caller) and drops out of `choose`;
a send larger than the byte budget resolves `select` as `.too_large(index)`
and likewise drops out of `choose`. A selection parked on a rendezvous
channel can be committed by a partner (a plain operation or another
selection), so two selections on opposite ends meet.
Batch receivers select one published message through `Op.recv_batch` and
`channel.on_recv_batch`.

Preserve FIFO within each logical producer. Different producers have no
cross-producer source-call order unless an ordered merge protocol establishes
one. Multiple consumers can complete processing out of dequeue order. A task
moving to another worker retains its producer ordering identity. Cloning an
endpoint does not grant a total ordering across concurrently used clones.

## Paged batches and scheduling boundaries

A producer appends into an exclusively writable page chain. Pages and capacity
credits are acquired from bounded pools; a published descriptor transfers a
sealed chain. **Implementation:** each producer task's staging lives in the
channel and is written under the channel's lock (a short critical section,
not a worker-local buffer), so migrating a task needs no detach step. Multicore
publication uses release/acquire-equivalent synchronization and a tested
receiver registration/wakeup protocol. It is not synchronization-free.

Automatically publish a batch when it reaches configured limits, when the task
schedules out, before parking on dependent work or capacity, on explicit flush,
and during modeled task completion/failure. A flush-latency target is checked
at scheduler safe points; it is not a wall-clock guarantee through foreign code.
Ordinary `Channel.send` cannot defer observable publication merely because the
implementation would prefer a larger batch.

Accepted staged messages already belong to the runtime. Pending state is rooted
independently of the producer task. `close()` rejects new messages and triggers
safe publication or configured disposal of accepted data; end-of-stream is not
reported while accepted messages can still appear. Closing a consumer with a
discard policy releases/destroys pending owned data exactly once.

Flush pending messages before waiting for additional capacity. Count staged,
published, and reserved data against admission limits. Otherwise batching can
deadlock a sender with a receiver that sees no work. Publication does not make
a whole application operation atomic across channels or other effects.

Thread-local allocator caches are permitted; persistent producer ownership is
task/endpoint-aware. Yielding does not invalidate aliases. The type system prevents retained mutable
references into sealed data. Pool reuse waits for every leased view to end;
leased page bytes remain charged to the page budget. Small values may move into
a slot once; in-place slot construction avoids that when required.

Two buffers suffice only when consumer release keeps pace. A bounded pool and
backpressure handle imbalance. A partially filled page may be published; the
consumer must not wait for it to become full. Fair draining prevents one large
batch from monopolizing the runtime.

## Defects and internal errors

A defect (failed check, `panic`, overflow, stack overflow, exceeded heap
budget) fails its task: cleanups run, the task's scope sees the failure
(siblings are cancelled and the owner fails, unless the child was
`capture`d), and supervision decides about restarts. A defect cannot be
caught inside the task that raised it: containment is per task, as in
Erlang.

An internal error (a bug in the compiler or runtime rather than in the
program) ends the process at once with exit status 70 and a report, on
whichever thread it happened, since runtime invariants may no longer hold.

## Static concurrency safety

What the compiler guarantees:

- **No data races.** Values are immutable; a value sent to another task
  (spawned closures, channel payloads) must be `Transfer`, and shared
  mutable state exists only as explicit `Cell` (one task) and `Atom`
  (atomic, shareable) values. A borrow cannot be stored into a container
  that may outlive it.
- **No lock-order deadlocks.** There are no locks; tasks coordinate by
  channels, replies, joins, and atoms.
- **Children cannot leak.** Every task belongs to a scope, which waits
  for (or cancels) its children before it ends.

What it reports (warnings, for patterns that deadlock on every run):

- **E0450 join before close:** a task that receives from a channel until
  it is closed is joined before that channel's sender is closed, later in
  the same block.

At run time, when every task is blocked, the program fails with
`deadlock: all tasks are blocked` and a note per task naming what it
waits for (channel, join, reply, ...).

## Minimal worker supervision

Managed workers are component descriptions with a restartable factory and an
explicit policy. Ordinary request tasks do not automatically restart. A worker
factory returns `Result<Unit, E>`; `ok(())` means normal completion, while an
error or task defect is available to its declared restart policy.

Start with `never` and `on_failure` policy constructors. `on_failure` specifies
a restart budget/window, bounded backoff, and escalation to its owning scope.
`worker.supervise(name, factory, policy)` runs a factory this way in any
task (systems run their workers through it): each attempt is a fresh
task; a failure past the budget, or under `never`, fails the caller.
A restart rebuilds worker-local resources through its factory; it does not
replay requests or recover an already-consumed in-memory job. Durable jobs need
a storage acknowledgment protocol. Policies may keep a parent-owned queue, but
must define what happens to pending work and producer handles.

Lifecycle dependency graphs and supervision scopes remain distinct. Restarting
a shared resource requires stable typed handles with generation/availability
checks or rebuilding the dependent closure in lifecycle order. Draft 0 defaults
to rebuilding the declared affected dependency group; transparent rebinding is
not assumed. Stop admission and settle in-flight work before invalidating a
resource. Full distributed actors and transparent remote references are out of
scope for this draft.

Precedents: [runtime-managed tasks](https://docs.rs/tokio/latest/tokio/task/),
[bounded channels](https://docs.rs/tokio/latest/tokio/sync/mpsc/),
[stackful suspension](https://docs.rs/corosensei/latest/corosensei/), and
[OTP restart budgets](https://www.erlang.org/doc/system/sup_princ.html).
The exact contracts above are this language's own design, and the toolchain
implements them.
