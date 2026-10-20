# TLA+ models of the runtime's synchronization

Phase 1 of [`OPTIMIZATION_BACKLOG.md`](../../OPTIMIZATION_BACKLOG.md): §1.1 (the
per-channel state machine) and §1.3 (the stop-the-world handshake). The channel
model describes `crates/badger_runtime` as of `9f1ff4c` (the Go-style channel
protocol of §1.2: one state word, one waiter list, `c4750f8`'s hand-off rule);
the GC model describes it as of `13e7756` (the handshake has not changed since).
They are the contract that the QSBR handshake (§2.1) and any polling page
(§0.1) must keep. Nothing here changes the runtime.

| File | What |
|---|---|
| `ChannelBadger.tla` | the channel state machine: the state word, the waiter list, fast paths, waits without the lock, locked paths, select, close, cancellation (plain TLA+) |
| `ChannelBadger.cfg` | the current code, prompt channels, safety (the regression gate) |
| `ChannelBadgerLive.cfg` | the current code, prompt channels, liveness |
| `ChannelBadgerContract.cfg`, `ChannelBadgerContractLive.cfg` | the §1.2 contract (`Impl = "contract"`), prompt and batch channels, every property; `run.sh` also runs them with `Impl = "current"` |
| `GcHandshake.tla`, `GcHandshake.cfg` | the stop-the-world handshake (PlusCal, with its translation) |
| `run.sh` | runs every check below and compares each outcome with the expected one |
| `repro/*.bdg` | Badger programs that reproduce findings F1, F2 (fixed) and F4 (open) on the real runtime |

## Running

TLC 2.19 (`tla2tools.jar` from the [TLA+ releases](https://github.com/tlaplus/tlaplus/releases),
Java 11 or later). The jar is not in the repository.

```sh
TLA2TOOLS=~/.local/share/tla/tla2tools.jar spec/tla/run.sh          # every check
spec/tla/run.sh ~/.local/share/tla/tla2tools.jar gc channel           # some
```

`run.sh` copies the models to a scratch directory and runs TLC there (no
`states/` or trace files in the repository), derives the finding and mutation
configurations from the committed `.cfg` files with `sed` substitutions (and
appended property sections), prints one line per check, and exits non-zero
when an outcome differs from the expected one (printing TLC's output). A
single model by hand:

```sh
cd spec/tla
java -cp "$TLA2TOOLS" tlc2.TLC -workers auto -metadir /tmp/tlc-states \
    -config ChannelBadger.cfg ChannelBadger.tla
```

After editing the PlusCal in `GcHandshake.tla`, regenerate its translation with
`java -cp "$TLA2TOOLS" pcal.trans -nocfg GcHandshake.tla` (and delete the
`GcHandshake.old` it leaves).

## Results

| Check | Expected | Got | States | Time |
|---|---|---|---:|---:|
| `channel` | pass | pass | 280,301 | 3s |
| `channel-live` | pass | pass | 275,956 | 33s |
| `channel-full` | pass | pass | 331,448 | 5s |
| `channel-full-live` | pass | pass | 420,003 | 53s |
| `contract` | pass | pass | 331,497 | 4s |
| `contract-live` | pass | pass | 419,763 | 54s |
| `contract-forgets-watchers` | WatchKept | WatchKept | 15,482 | 1s |
| `f1-cancel-after-handoff` | pass | pass | 280,301 | 4s |
| `f2-batch-one-wake` | pass | pass | 51,147 | 2s |
| `f2-batch-one-wake-live` | pass | pass | 144,047 | 19s |
| `f3-one-step` | pass | pass | 280,301 | 4s |
| `f3-registration-served` | pass | pass | 280,301 | 4s |
| `f4-cancel-after-retry-wake` | NoLostWake | NoLostWake | 5,125 | 1s |
| `f4-cancel-after-retry-wake-live` | temporal | temporal | 140,722 | 17s |
| `f4-no-cancel` | pass | pass | 2,749 | 1s |
| `m-fast-ignores-watchers` | NoLostWake | NoLostWake | 1,336 | 1s |
| `m-fast-ignores-watchers-live` | temporal | temporal | 275,206 | 20s |
| `m-select-no-claim` | NoLostWake | NoLostWake | 1,565 | 1s |
| `m-no-stale` | PhasePublished | PhasePublished | 309 | 1s |
| `m-no-stale-safe` | pass | pass | 279,466 | 3s |
| `m-cancel-retracts` | ExactlyOnce | ExactlyOnce | 5,803 | 2s |
| `m-one-wake-per-batch` | NoLostWake | NoLostWake | 15,622 | 1s |
| `m-one-wake-per-batch-live` | temporal | temporal | 144,473 | 13s |
| `m-queue-forgets-watchers` | OneStep | OneStep | 6,059 | 1s |
| `m-queue-forgets-watchers-reg` | RegistrationServed | RegistrationServed | 4,805 | 1s |
| `gc` | pass | pass | 80,106 | 18s |
| `gc-check-before-inc` | EveryWorkerObservesEpoch | EveryWorkerObservesEpoch | 26,376 | 2s |
| `gc-unlock-before-inc` | EveryWorkerObservesEpoch | EveryWorkerObservesEpoch | 32,030 | 3s |
| `gc-dec-before-publish` | RootsPublished | RootsPublished | 18,503 | 2s |
| `gc-nested-collect` | ScanUnderStop | ScanUnderStop | 25,065 | 3s |

In short: every check matched its expected outcome, so the refactored
protocol (`9f1ff4c`) satisfies the properties the backlog asks for --
`NoLostWake`, `RegisteredWhileWaiting`, `AtMostOnce`,
`EveryBlockedRecvWoken`, `OneStep`, `RegistrationServed` and `WatchKept` --
for prompt and batch channels, at one and at three workers, with
cancellation and spurious wakes. The findings F1, F2 and F3 of the
pre-refactor code are fixed: their checks pass, and each comes back as a
mutation that puts the old code back (`m-cancel-retracts`,
`m-one-wake-per-batch`, `m-queue-forgets-watchers`). The GC handshake
model still passes, with every mutation caught.

**One finding is open: F4** (below). It needs four tasks and so is out of
the three-task bounds the backlog names; the model finds it at four tasks
and one worker, and the same configuration without the cancellation
passes.

## The channel model (`ChannelBadger.tla`)

### What is modelled

The channel model is plain TLA+ with an explicit `pc` per task rather than
PlusCal: one step of the code updates the same variable several times (a close
wakes and removes every waiter; a publication seals a batch, pops and wakes
receivers and wakes the watchers), which PlusCal's one-assignment-per-label
rule would split into steps the code does not have. The GC model is PlusCal.

One channel, three tasks (`Tasks`, a symmetry set; four in the F4 checks).
Each task picks a role at start and performs `Ops` operations of it:

- prompt channels: `send` (`chan_send`, `intrinsics_task.rs:479`: `send_fast`,
  then `send_free`, then the locked `send`), `recv` (`recv_fast`, `recv_free`,
  the locked `recv`), `select` over one receive case (`chan_select`,
  `intrinsics_task.rs:321`: probe, `watch_recv`, `offer_cases`, park; on wake
  `unwatch` and `take_committed` or probe again), `close` (`close_fast`, then
  the locked `close`);
- batch channels (`MaxBatch = 2`, capacity 2): `append` twice (`append_fast`,
  the locked `append`, which publishes a full batch and flushes before waiting
  for capacity; the producer's staging is published when it finishes),
  `recvone` (`recv_one_fast`, the locked `recv_one`), `select` (its probe is
  `recv_one(ch, false)`), `close` (publishes every producer's staging).

The environment injects up to `MaxCancels` cancellations (`cancel_with`,
`task.rs:2160`: set the flag, `wake_waiting(Cancelled)`) and `MaxSpurious`
wakes that deliver nothing (a timer, another select case). The channel kind,
the prompt capacity (0 and 1) and the worker count (1, or one worker per task)
are chosen in `Init`, so one run covers all of them.

The channel is `ChanData` as the refactor left it:

- **The state word** (`ChanData::state`, `channel.rs:70`): `c.own` is the owner
  (`free`, or `claim` for the runtime lock's holder; a fast path's `FAST` is
  never visible, its body being one step), `c.pub` and `c.pclosed` the
  published phase (`EMPTY`, `VALUE`, `BLOCKED`) and closed bit, `c.stale` the
  one-worker `STALE` bit. `Word(c)` is `ChanData::word` of the contents.
- **The waiter list** (`Waiters`, `channel.rs:125`): `c.ws`, one sequence of
  entries (task, kind, wait generation, a sender's value) in arrival order,
  with the per-kind counts `c.cnt` kept as the code keeps them. The kinds are
  `Recv`, `Send`, `WatchRecv` and `Capacity` (`WatchSpace` belongs to select
  send cases, which are not modelled). An operation serves the first live
  entry of a kind and pops the dead ones before it, as `Waiters::pop` loops do.

The atomicity follows the code's ownership protocol:

- a fast path owns the channel from its compare-and-swap in `try_fast`
  (`channel.rs:595`) to `end_fast` (`channel.rs:629`), so its body is one
  step, enabled only while no claim is held, ending with the store that
  publishes `Word(c)` and clears `STALE`; giving up after the spin is the same
  step's decline branch. `recv_fast` and `recv_one_fast` load the word first
  and decline on a published `EMPTY` phase (`published`, `channel.rs:577`)
  without a compare-and-swap; the decline is a branch of the same step (a
  decline only moves the task to its slower path, so splitting the load from
  the compare-and-swap adds no behaviour);
- code holding the runtime lock claims the channel on its first `data()`
  (`claim`, `channel.rs:527`) and keeps the claim until the lock is released
  (`release_claims`, `channel.rs:553`, called from `unlock`, `task.rs:783`),
  which publishes the phase: in the model, `Claim` at the step that takes the
  lock and `Rel` at the step that releases it. With one worker nothing claims:
  `data()` sets `STALE` (`Claim` and `Rel` do), so the next fast path decides
  from the contents. No fast path runs inside a locked operation; a locked
  operation that parks releases the lock in the parking step (the real worker
  may keep it longer, which only removes interleavings);
- park, wake and deliver are atomic per task (`with_task`); `Waiting` follows
  the task states of `task.rs:100` (`Parking` with `pending_wake`, `Parked`),
  `wake_waiting` drops a wake to a running task (`task.rs:1958`), `deliver`
  and `waits` (`task.rs:413`, `2026`) check the wait generation, the cancel
  flag and the state; `park` honours a cancellation that arrived during the
  wait only when no waker completed it with status 1
  (`interrupts_after_wait`, `task.rs:1386`);
- with one worker at most one task runs and runs until it parks or finishes;
  the waits without the lock do not exist then (`can_suspend_free`,
  `task.rs:1422`).

The operations decide as the code does: a send hands its value to the first
live parked receiver, else commits it to the first parked selection offering
to receive (at every capacity; `commit_select_receive`, `channel.rs:1204`,
which checks `waits`), else queues it without waking anyone, else parks; a
selection's `WatchRecv` entry is removed only by its `unwatch`, its end or the
close; a publication (`channel.rs:1472`) pops and wakes one live receiver per
message (`wake_receivers`) and wakes every watcher without removing it
(`wake_watchers`); a receive that frees a slot moves the first live parked
sender's value into it; `recv_one` wakes and removes every producer waiting
for capacity.

`Impl = "contract"` is the §1.2 design as the model encoded it before the
refactor: a send that finds a parked selection commits to it at every capacity,
a queued value leaves the watchers registered, a publication wakes one
receiver per message, and a completed hand-off is taken before a pending
cancellation is honoured. The code meets it, and differs from it in two
places, neither a deviation from §1.2:

- the code is stricter: a publication wakes the watchers and keeps their
  registrations, where the contract let it wake and forget them (a woken
  selection probes before it registers again, so forgetting loses nothing).
  `WatchKept` holds for the code and fails for the contract (check
  `contract-forgets-watchers`);
- the published phase and the declines on it are an implementation device the
  contract does not have; the contract's receives never decline on it.

Everything else the two variants share (in the model before the refactor, the
contract also kept the old code's re-registration of a producer woken
spuriously from a capacity wait, which `OneEntryPerKind` would now reject; the
code removes its entry when it wakes, and so does the contract here).

### Properties

The four the backlog asks for, as TLA+ (definitions at the end of the module):

1. **One step at a time** — `OneStep` (action property, prompt channels). The
   abstract state is `Closed`, `WithValue` (a queued value or a live parked
   sender), `Blocked` (a receiver waits with no wake pending) or `Empty`;
   every step keeps it, moves it to a neighbour (`Empty`–`WithValue`,
   `Empty`–`Blocked`) or closes the channel. For the implementation it means
   that a value never arrives at a channel with a waiting receiver without
   being handed to that receiver in the same step.
2. **No lost wake-up** — the invariant `NoLostWake` over the pending-wake
   relation: a receiver (recv, recv_one or select) that is parked, or parking
   with no wake pending, never coexists with a closed channel, nor with a
   value it could take that no other receiver is on its way to. A prompt value
   is handed over, so none may be available while a receiver waits. A batch
   publication wakes a receiver per message *to retry*, so a published message
   may wait for a woken receiver that has not run yet; `Retrying` counts the
   receivers on their way (woken in a batch receive or a selection and not
   about to honour a cancellation, or holding the lock in their body), and the
   published messages may not exceed them. (With three tasks there are at most
   two receivers and the bound is 0 for batches too — the form the model used
   before the refactor; with three receivers a batch of two legitimately
   leaves the third parked.) The liveness form `EveryBlockedRecvWoken`: every
   waiting receiver stops waiting, unless nothing is deliverable and every task
   that could still send or close has finished. The literal "every blocked
   recv is eventually woken" is false of a receiver nobody sends to; this is
   the strongest form a correct channel satisfies.
3. **Delivered exactly once** — `Holders(v)` counts every place value `v` can
   be (queue, sealed batch, staging, a parked sender's entry, a receiver's
   delivered slot, a completed receive). `AtMostOnce`: never two.
   `ExactlyOnce`: also exactly one once its send returned success, none once it
   returned closed.
4. **Registration is not lost** — `RegisteredWhileWaiting`: a waiting task has
   an entry of its kind for its current wait in the waiter list.
   `RegistrationServed` (action property, prompt channels): a waiting receiver
   stops waiting only by being handed a value or a committed case, by the
   close, by its cancellation, or by an environment wake — not by a send that
   wakes it and forgets it. `WatchKept` (action property, both kinds): a
   selection's `WatchRecv` entry disappears only in a step of the selection
   itself or by the close (`channel.rs:38-43`).

And three about the refactor's data structures:

- `CountsMatch`: the per-kind counts the fast paths read are the list's.
- `OneEntryPerKind`: a task has at most one entry of each kind (the
  `debug_assert!` in `Waiters::push`, which release builds do not check).
- `PhasePublished`: whenever the channel is free and, with one worker, not
  `STALE`, the word's phase and closed bit are those of the contents (the
  claim in `ChanData`'s documentation, `channel.rs:224-227`).

### What the channel model does not cover

- Unbounded runs: three tasks, one channel, at most two operations per task
  (one in the liveness runs and the four-task runs), one cancellation and one
  spurious wake. A defect that needs more of any of these is not excluded
  (F4 needs four tasks).
- Selections over several channels, select send cases (`WatchSpace`,
  `commit_select_send`, `wake_space`), timeouts as such (a timer wake is a
  spurious wake), byte budgets, `try_send` / `try_recv`, leased batches
  (`recv_batch`, `release`), `flush` / `flush_due`, `close_receiver`,
  `discard_on_close`, the `pre_park` / `early_wake` / `publishing` states of a
  task that is about to park (joins, a producer publishing as it parks), and
  the deadlock detector. A task has one role, so no task both stages and
  receives.
- Scheduler fairness beyond the assumption that a ready task eventually runs
  and a waiting lock acquisition eventually succeeds (strong fairness per
  task); task migration between workers (each task has its own worker);
  interleavings inside a fast path or inside a locked body (both are single
  steps, justified by the ownership protocol above); the bounded spin of
  `try_fast` (a fast path that finds the channel claimed declines at once;
  waiting for the claim to end is the same as being scheduled later).
- The memory model and the compiler: the model assumes every step reads the
  current value of every field, which the code obtains from the acquire
  compare-and-swap and the release store on the word; `raw()`
  (`channel.rs:515`) still hands every caller a `&'a mut ChanData` with an
  unbounded lifetime. `data()` without the runtime lock claims nothing with
  several workers; the model assumes, as the code's comments do, that only
  the lock's holder calls it.

### What the model establishes for the new protocol

Under the bounds above, for prompt and batch channels, one worker and three,
capacity 0 and 1, with a cancellation and a spurious wake:

- the four properties the backlog asks for hold for the current code, safety
  and liveness — `NoLostWake`, `RegisteredWhileWaiting`, `ExactlyOnce` (so
  `AtMostOnce`), `OneStep`, `RegistrationServed`, `EveryBlockedRecvWoken`
  (checks `channel`, `channel-live`, `channel-full`, `channel-full-live`);
- the refactor's own claims hold: a selection's registration is removed only
  by the selection or the close (`WatchKept`), the counts match the list, no
  task has two entries of a kind, and the published phase is the contents'
  whenever the channel is free and not stale;
- the published phase is advisory: with `Mutation = "no_stale"` (one worker,
  locked code that forgets to mark the word `STALE`) the phase is wrong
  (`PhasePublished` fails) and every other property still holds (check
  `m-no-stale-safe`), because a decline on the phase only sends the operation
  to a path that decides from the contents. A wrong phase costs a slow path,
  never a wake-up;
- the defects of the code before the refactor, put back one at a time as
  mutations, are caught by the same properties as before (checks
  `m-cancel-retracts`, `m-one-wake-per-batch`, `m-one-wake-per-batch-live`,
  `m-queue-forgets-watchers`, `m-queue-forgets-watchers-reg`), and so are the
  two mutations of the slice-4 analysis.

What it cannot establish: anything beyond the bounds (in particular more than
two receivers, except in the F4 checks), the unmodelled operations above, and
the memory-model question below.

### The slice-4 lead

The report (`reports/2026-10-19-select-fast-path-fragility.md`) ends with a
`send_fast` that queued on a channel whose `recv_watchers` held the parked
selector. Against the old code the model showed that state unreachable by any
interleaving of the modelled steps (so the guard read a stale
`recv_watchers`, which pointed at `raw()`'s references, not at the wake-up
protocol), and reproduced it as `Mutation = "fast_ignores_watchers"`. Against
the new code:

- the guard is now the count of `WatchRecv` entries, read by the owner after
  the compare-and-swap that made it the owner (an acquire), and the entry is
  written by the claimed selection and published by the release store of
  `release_claims`; the mutation that ignores the count still loses the wake
  (checks `m-fast-ignores-watchers`, `m-fast-ignores-watchers-live`), and the
  code without the mutation does not;
- `Mutation = "select_no_claim"` drops the claim between the probe and the
  registration (the report's "check-then-register window"): TLC still finds
  the lost wake (a `close_fast` between them). The window is closed by the
  claim, which the selection holds from its probe until it parks;
- the report's point 2 (the queue branch of `send` woke every watcher and
  forgot it) is gone (F3 below).

### Findings

F1, F2 and F3 were found in the code before the refactor (`13e7756`) and are
fixed at `9f1ff4c`: their checks now pass, and each comes back as a mutation
that puts the old code back. F4 is new and open.

**F1 — a value handed to a receiver that is then cancelled was lost (fixed in
`c4750f8`).** A receiver parked in the locked `recv` was handed a value
(`wake_with` or `deliver`, status 1) and became ready; before it ran, it was
cancelled. When it resumed, `park()` honoured the cancellation before the
receive took the delivered value, and the task abandoned: the sender got
success and nobody had the value (spec 05: "After ownership commits,
cancellation does not silently retract a message"); a committed selection lost
its value the same way. `park()` now honours an interrupt after a wait only
when no waker completed the wait with status 1 (`interrupts_after_wait`,
`task.rs:1386`); the model's resume steps take a completed hand-off first
(`Honors`). Check `f1-cancel-after-handoff` (`ExactlyOnce`, prompt) passes;
`Mutation = "cancel_retracts"` brings the violation back
(`m-cancel-retracts`). `repro/cancel_after_handoff.bdg` printed "empty: the
sent value is gone" at `13e7756` with one worker.

**F2 — a published batch woke one receiver and stranded the rest of its
messages (fixed in `9f1ff4c`).** `publish` woke one parked receiver per sealed
batch, and `recv_one` takes one message: with two receivers parked and a
batch of two published, the second stayed parked with a message published.
`publish` now wakes a receiver per message (`wake_receivers(d, n)`). Checks
`f2-batch-one-wake` and `f2-batch-one-wake-live` pass; `Mutation =
"one_wake_per_batch"` brings both violations back. `repro/batch_two_receivers.bdg`
deadlocked at `13e7756` with one and with four workers.

**F3 — the queue branch of `send` woke the watchers and forgot them (fixed in
`9f1ff4c`).** A locked send committed to a parked selection only at capacity
0; at capacity 1 it queued its value and woke and dropped every watcher, so
the abstract state went `Blocked` → `WithValue` in one step and the
selection's registration was lost (not a lost wake-up: the woken selection
probed again under its claim). A send now commits to a parked selection at
every capacity and queueing wakes nobody (every watcher still registered is
not parked and probes before it parks). Checks `f3-one-step` and
`f3-registration-served` pass, and `WatchKept` holds; `Mutation =
"queue_forgets_watchers"` brings `OneStep` and `RegistrationServed` back.

**F4 — a batch receiver woken to retry and then cancelled takes its wake with
it (open).** Checks `f4-cancel-after-retry-wake` (`NoLostWake`) and
`f4-cancel-after-retry-wake-live` (liveness): four tasks (three `recv_one`
receivers and a producer), batch channels, one worker, `Ops = 1`, one
cancellation, no spurious wake. A publication wakes one receiver per message, but the wake is a
wake to *retry*: the woken receiver's `Recv` entry is popped
(`wake_receivers`, `channel.rs:1012`) and it takes a message only when it runs.
If it is cancelled (or a child failure is pending) before it runs, `park()`
honours the interrupt — rightly, nothing was handed to it, its status is not
1 — and the task abandons; nothing passes its wake on. TLC's safety trace
(5,125 states, one worker):

1. receivers t1, t2, t3 park in `recv_one` (entries `Recv` t1, t2, t3);
2. producer t4 appends twice; the second append fills the batch and publishes
   it: `wake_receivers(d, 2)` pops and wakes t1 and t2; t3 stays parked;
3. t4 finishes; t1 is cancelled before it runs (the flag is set; the wake
   finds t1 ready and does nothing). Two messages are published, one
   uncancelled receiver is on its way, and t3 waits: `NoLostWake` fails.

The liveness trace goes on: t1 resumes, `park()` honours the cancellation and
t1 abandons (`task_finished` has nothing to remove); t2 takes the first message
and finishes; the last receiver stays parked forever with the second message
published (the trace ends in a stuttering loop there).

The same configuration without the cancellation passes (`f4-no-cancel`), and
prompt channels are not affected (a prompt value is handed over with status 1,
which F1's fix protects; a woken selection is one of all the watchers woken).
The §1.2 contract has the same gap: it also wakes one receiver per message.
Three tasks cannot show it: with at most two receivers, a batch of two wakes
both, and a batch of one is published only by a close (which wakes everyone)
or by a cancelled producer (which spends the one cancellation).
`repro/cancel_after_retry_wake.bdg` is the trace as a program (three
receivers, a batch of two, the first woken receiver cancelled at once): one
receiver gets a message, and the last one is left parked with the second
message published: at `9f1ff4c` it prints "r2 got 1", "r1 cancelled", then
"deadlock: all tasks are blocked" with the third receiver waiting on batch
receive — 3 runs of 3 with one worker, 2 of 3 with four (where the woken
receiver sometimes runs before the cancellation reaches it).

Two ways to fix it, for the runtime's owner:

- *Pass the wake on.* A task that leaves a batch channel (`task_finished`, or
  the abandon path of `recv_one`) while messages are published wakes one more
  parked receiver (`wake_receivers(d, 1)`; a spare wake is harmless, the
  receiver retries and registers again). In a scratch copy of the model (not
  committed) with `Leave` doing exactly that, `f4-cancel-after-retry-wake-live`
  passes: 140,650 states, 11 min. The safety form needs `Retrying` to count a
  cancelled woken receiver as on its way until it leaves, so the invariant
  would change with the fix.
- *Hand off.* Deliver a message to each woken `recv_one` receiver with status 1
  (as a prompt send does), so the cancellation is honoured after the receive;
  this is the Go design and makes the batch receive one step like the prompt
  one.

## The GC handshake model (`GcHandshake.tla`)

### What is modelled

One process per worker thread, three workers, `MaxEpochs = 2` collections.
Each label is one sequentially consistent access, following the code:

| Code | Model |
|---|---|
| `stop_the_world` (`task.rs:938`): `STOP := true`, spin until the counters sum to 0, read one slot at a time | `CStop`, `CRestart`, `CRead` (one slot per step, any order), `CCheck` |
| `collect` (`gc.rs:650`): trace and sweep, then `resume_the_world` (`task.rs:949`) | `CScan`, `CSweep`, `CResume` |
| `gc_enter` (`task.rs:1496`): count, read `STOP`; if set, uncount, spin, retry | `GEFirst`, `GECheck`, `GEDec`, `GESpin` |
| `badger_rt_enter` (`task.rs:875`): uncount, take the lock | `EnterDec`, `EnterLock` |
| `badger_rt_leave` (`task.rs:882`): count (holding the lock), unlock | `LeaveFirst`, `LeaveUnlock` |
| `enter_runtime_holding` (`task.rs:923`): publish `RETURNED`, uncount, lock, root, clear | `HFirst`, `HSecond`, `HLock`, `HRoot` |
| `badger_rt_safe_begin` / `_end` (`task.rs:890-899`): uncount; lock, count, unlock | `ForeignDec`, `ForeignLock`, `ForeignInc`, `ForeignUnlock` |
| `suspended_free` (`task.rs:3073`): a task suspended without the lock uncounts its worker | `SuspendDec` |
| the scheduler with the lock: run a locked task, `run_free`, go idle, or collect | `Runtime`, `IdleLock`, `FreeSched`, `FreeLock` |

A worker "may mutate" (`MutatorPCs`) while it runs compiled code, or runtime
code that compiled code calls without the lock (allocation, the channel fast
paths), up to the step that uncounts it.

### Properties

- **Every worker observes the new epoch** — `EveryWorkerObservesEpoch`: while
  the collector traces or sweeps, no other worker may mutate, and each of them
  acknowledged the epoch (`ack[w] = epoch`: the collector read its slot as 0
  during this epoch's stop and it has not run compiled code since). Being an
  invariant of every state of the scan, it also says that no worker enters
  compiled code after the scan started.
- **No root published after the scan starts** — `RootsPublished`: during the
  scan, a pointer that compiled code returned to a worker that is not yet
  holding the lock is in `RETURNED`.
- **The resume is ordered after the scan** — `ScanUnderStop` (tracing and
  sweeping happen with `STOP` set and the lock held by the collector) and
  `ResumeAfterSweep` (`STOP` clears only from the collector's resume step after
  its sweep).
- `CountMatches`: each slot is exactly 1 while its worker may run compiled code
  and 0 otherwise.
- Liveness: `CollectionCompletes` (a started collection ends; this needs every
  worker in compiled code to reach a safe point, the model's weak fairness for
  the poll) and `SpinnersProceed` (a worker waiting in `gc_enter` runs again).

### What the current code relies on

The handshake satisfies every property above in the model (check `gc`). The
model makes explicit what that depends on:

- **Store-then-load on both sides, sequentially consistent.** The worker counts
  itself before it reads `STOP` (`gc_enter`); the collector sets `STOP` before
  it reads the counters. Reading `STOP` first (`Mutation = "check_before_inc"`)
  lets a worker read `STOP = false`, the collector stop and read all zeros, and
  the worker count itself and run compiled code during the scan (a 20-state trace). The model is sequentially consistent; the code gets the same
  guarantee only because all four accesses are `SeqCst` (`task.rs:942-943`, `950`,
  `1498-1499`). With release/acquire on either side the store-load pair may
  reorder — the same interleaving.
- **Counting before unlocking.** `badger_rt_leave` counts while it still holds
  the lock; unlocking first (`"unlock_before_inc"`) lets a collector take the
  lock and read the slot as 0 before the worker counts and returns to compiled
  code, which does not check `STOP` (21-state trace). `badger_rt_safe_end` relies on
  the same order.
- **Publishing before uncounting.** `enter_runtime_holding` stores `RETURNED`
  before `badger_rt_enter` uncounts; the reverse order
  (`"dec_before_publish"`) lets the scan miss the pointer (`RootsPublished`, 17-state
  trace).
- **No nested stop.** `STOP` is a flag, not a count or an epoch: a
  `stop_the_world` / `resume_the_world` pair inside a collection clears it
  while the outer collection still sweeps, and workers spinning in `gc_enter`
  then run compiled code during the sweep (`"nested_collect"`, `ScanUnderStop`,
  20-state trace). Today nothing nests: a collection inside a finalizer is ruled out
  by `debug_assert!(!FINALIZING…)` (`gc.rs:347`), which release builds do not
  check, and `for_each_object_of` (`gc.rs:1339`), which also stops and resumes
  the world, is not called during a collection. A QSBR epoch (§2.1) removes the
  dependence.
- **Balanced slots.** The collector tests the sum, not every slot; a slot that
  went negative would hide another worker's 1. `CountMatches` holds for the
  modelled paths; the claim that every count and uncount on a thread pair up
  (`task.rs:793-796`) is the code's, and the model only covers the paths in
  the table.
- **Reading the slots one at a time is safe** because outside the lock a slot
  can only rise in `gc_enter`, which re-reads `STOP` and backs out; the model
  reads the slots in every order.
- **Acknowledgement is implicit.** `ack` is a ghost variable: the code has no
  per-worker epoch, only the sum. §2.1 should make the acknowledgement real
  (a per-worker observed epoch) and keep `EveryWorkerObservesEpoch`; §0.1's
  polling page would replace the `gc_enter` / `badger_rt_enter` paths and must
  keep the same invariant.

### What the GC model does not cover

The memory model (it is sequentially consistent), the roots themselves (task
shadow stacks, `Roots`, the remembered sets: only `RETURNED` is modelled),
allocation debt and the collection trigger, the polling page (§0.1 is not
adopted), and runs with more than three workers or two collections.
