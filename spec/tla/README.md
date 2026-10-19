# TLA+ models of the runtime's synchronization

Phase 1 of [`OPTIMIZATION_BACKLOG.md`](../../OPTIMIZATION_BACKLOG.md): §1.1 (the
per-channel state machine) and §1.3 (the stop-the-world handshake). The models
describe `crates/badger_runtime` as of `13e7756`; they are the contract that the
Go-style channel refactor (§1.2), the QSBR handshake (§2.1) and any polling page
(§0.1) must keep. Nothing here changes the runtime.

| File | What |
|---|---|
| `ChannelBadger.tla` | the channel state machine: fast paths, waits without the lock, locked paths, select, close, cancellation (plain TLA+) |
| `ChannelBadger.cfg` | the current code, prompt channels, safety (the regression gate) |
| `ChannelBadgerLive.cfg` | the current code, prompt channels, liveness |
| `ChannelBadgerContract.cfg`, `ChannelBadgerContractLive.cfg` | the §1.2 contract (`Impl = "contract"`), prompt and batch channels, every property |
| `GcHandshake.tla`, `GcHandshake.cfg` | the stop-the-world handshake (PlusCal, with its translation) |
| `run.sh` | runs every check below and compares each outcome with the expected one |
| `repro/*.bdg` | Badger programs that reproduce findings F1 and F2 on the real runtime |

## Running

TLC 2.19 (`tla2tools.jar` from the [TLA+ releases](https://github.com/tlaplus/tlaplus/releases),
Java 11 or later). The jar is not in the repository.

```sh
TLA2TOOLS=~/.local/share/tla/tla2tools.jar spec/tla/run.sh          # every check
TLA2TOOLS=~/.local/share/tla/tla2tools.jar spec/tla/run.sh gc channel  # some
```

`run.sh` copies the models to a scratch directory and runs TLC there (no
`states/` or trace files in the repository), derives the finding and mutation
configurations from the committed `.cfg` files with one `sed` substitution
each, prints one line per check, and exits non-zero when an outcome differs
from the expected one (printing TLC's output). A single model by hand:

```sh
cd spec/tla
java -cp "$TLA2TOOLS" tlc2.TLC -workers auto -metadir /tmp/tlc-states \
    -config ChannelBadger.cfg ChannelBadger.tla
```

After editing the PlusCal in `GcHandshake.tla`, regenerate its translation with
`java -cp "$TLA2TOOLS" pcal.trans -nocfg GcHandshake.tla` (and delete the
`GcHandshake.old` it leaves).

## Results

`spec/tla/run.sh` on 2026-10-19 (TLC 2.19 rev 5a47802, OpenJDK 25, 16 cores,
`-workers auto`), against `13e7756`. "States" is the number of distinct states
TLC found; for a check that expects a violation it is the count at the point
TLC stopped, and the trace length is given in the findings below. Every outcome
matched the expected one (exit 0).

| Check | Config | Expected | Got | States | Time |
|---|---|---|---|---:|---:|
| `channel` | `ChannelBadger.cfg` | pass | pass | 294,181 | 12 s |
| `channel-live` | `ChannelBadgerLive.cfg` | pass | pass | 249,238 | 63 s |
| `contract` | `ChannelBadgerContract.cfg` | pass | pass | 314,420 | 13 s |
| `contract-live` | `ChannelBadgerContractLive.cfg` | pass | pass | 386,649 | 158 s |
| `f1-cancel-after-handoff` | `channel`, `ExactlyOnce` for `AtMostOnce` | `ExactlyOnce` | `ExactlyOnce` | 13,815 | 10 s |
| `f2-batch-one-wake` | `channel`, `Kinds = {"batch"}` | `NoLostWake` | `NoLostWake` | 16,780 | 10 s |
| `f2-batch-one-wake-live` | `channel-live`, `Kinds = {"batch"}` | liveness | liveness | 139,211 | 17 s |
| `f3-one-step` | `channel` + `OneStep` | `OneStep` | `OneStep` | 7,802 | 8 s |
| `f3-registration-served` | `channel` + `RegistrationServed` | `RegistrationServed` | `RegistrationServed` | 10,698 | 6 s |
| `m-fast-ignores-watchers` | `channel`, the mutation | `NoLostWake` | `NoLostWake` | 1,958 | 1 s |
| `m-fast-ignores-watchers-live` | `channel-live`, the mutation | liveness | liveness | 249,472 | 41 s |
| `m-select-no-claim` | `channel`, the mutation | `NoLostWake` | `NoLostWake` | 1,685 | 2 s |
| `gc` | `GcHandshake.cfg` | pass | pass | 80,106 | 76 s |
| `gc-check-before-inc` | `gc`, the mutation | `EveryWorkerObservesEpoch` | `EveryWorkerObservesEpoch` | 25,765 | 21 s |
| `gc-unlock-before-inc` | `gc`, the mutation | `EveryWorkerObservesEpoch` | `EveryWorkerObservesEpoch` | 32,244 | 27 s |
| `gc-dec-before-publish` | `gc`, the mutation | `RootsPublished` | `RootsPublished` | 17,808 | 6 s |
| `gc-nested-collect` | `gc`, the mutation | `ScanUnderStop` | `ScanUnderStop` | 25,286 | 15 s |

In short: under the bounds below, the current prompt-channel protocol has no
lost wake-up, keeps every registration, and never duplicates a value; the GC
handshake satisfies every property. The current code has three findings, two
of them reproduced on the real runtime (F1, F2, F3 below). The §1.2 contract
satisfies every property, safety and liveness, for prompt and batch channels.
Each deliberate mutation of either protocol is caught.

Bounds. Channel safety: 3 tasks, 1 channel, `Ops = 2` operations per task,
prompt capacity 0 and 1, batch capacity 2 with `MaxBatch = 2`, 1 worker and 3
workers, one cancellation and one spurious wake; symmetry over the tasks.
Channel liveness: the same with `Ops = 1` and no symmetry (TLC's liveness
checking is not sound under symmetry reduction; an `Ops = 2` liveness run
without symmetry was stopped unfinished after about eight minutes). GC: 3 workers, 2 collections, no symmetry.
All state spaces are finite, so every property, liveness included, is decided
exhaustively for these bounds and for nothing larger.

## The channel model (`ChannelBadger.tla`)

### What is modelled

The channel model is plain TLA+ with an explicit `pc` per task rather than
PlusCal: one step of the code updates the same variable several times (a close
wakes every waiter and drains four lists; a publish seals a batch, wakes a
receiver and drains the watchers), which PlusCal's one-assignment-per-label
rule would split into steps the code does not have. The GC model is PlusCal.

One channel, three tasks (`Tasks`, a symmetry set). Each task picks a role at
start and performs `Ops` operations of it:

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
`task.rs:2018`: set the flag, `wake_waiting(Cancelled)`) and `MaxSpurious`
wakes that deliver nothing (a timer, another select case). The channel kind,
the prompt capacity (0 and 1) and the worker count (1, or one worker per task)
are chosen in `Init`, so one run covers all of them.

The atomicity follows the code's ownership protocol:

- a fast path owns the channel's `state` byte from `try_fast` to `end_fast`
  (`channel.rs:352`), so its body is one step, enabled only while no claim is
  held; giving up after the spin is the same step's decline branch;
- code holding the runtime lock claims the channel on its first `data()`
  (`channel.rs:301`) and keeps the claim until the lock is released
  (`release_claims`, `task.rs:777`), so no fast path runs inside a locked
  operation; a locked operation that parks releases the lock in the parking
  step (the real worker may keep it longer, which only removes interleavings);
- park, wake and deliver are atomic per task (`with_task`); `Waiting` follows
  the task states of `task.rs:100` (`Parking` with `pending_wake`, `Parked`),
  `wake_waiting` drops a wake to a running task (`task.rs:1816`), `deliver`
  and `waits` check the wait generation and the cancel flag;
- with one worker at most one task runs and runs until it parks or finishes;
  the waits without the lock and the claims do not exist then
  (`can_suspend_free`, `task.rs:1409`; `data()` claims only with several
  workers).

`Impl = "contract"` is the §1.2 design reduced to what the properties need: a
send that finds a parked selection commits to it at every capacity (today only
at capacity 0, `channel.rs:797`), a queued value leaves the watchers
registered, a published batch wakes one receiver per message, and a completed
hand-off is taken before a pending cancellation is honoured.

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
   with no wake pending, never coexists with a value it could take or with a
   closed channel. The liveness form `EveryBlockedRecvWoken`: every waiting
   receiver stops waiting, unless nothing is deliverable and every task that
   could still send or close has finished. The literal "every blocked recv is
   eventually woken" is false of a receiver nobody sends to; this is the
   strongest form a correct channel satisfies. For the implementation: every
   path that queues or publishes a value either hands it to the first waiter
   or leaves a wake pending for every waiter it does not serve.
3. **Delivered exactly once** — `Holders(v)` counts every place value `v` can
   be (queue, sealed batch, staging, a parked sender's entry, a receiver's
   delivered slot, a completed receive). `AtMostOnce`: never two.
   `ExactlyOnce`: also exactly one once its send returned success, none once it
   returned closed.
4. **Registration is not lost** — `RegisteredWhileWaiting`: a waiting task is in
   the channel's list for its current wait (`recv_waiters`, `send_waiters`,
   `cap_waiters`, `recv_watchers`). `RegistrationServed` (action property,
   prompt channels): a waiting receiver stops waiting only by being handed a
   value or a committed case, by the close, by its cancellation, or by an
   environment wake — not by a send that wakes it and forgets it.

### What the channel model does not cover

- Unbounded runs: three tasks, one channel, at most two operations per task
  (one in the liveness runs), one cancellation and one spurious wake. A defect
  that needs more of any of these is not excluded.
- Selections over several channels, select send cases (`space_waiters`,
  `commit_select_send`), timeouts as such (a timer wake is a spurious wake),
  byte budgets, `try_send` / `try_recv`, leased batches (`recv_batch`,
  `release`), `flush` / `flush_due`, `close_receiver`, the `pre_park` /
  `early_wake` path of joins, and the deadlock detector.
- Scheduler fairness beyond the assumption that a ready task eventually runs
  and a waiting lock acquisition eventually succeeds (strong fairness per
  task); task migration between workers (each task has its own worker);
  interleavings inside a fast path or inside a locked body (both are single
  steps, justified by the ownership protocol above).
- The memory model and the compiler: the model assumes every step reads the
  current value of every field. `raw()` (`channel.rs:309`) hands every caller
  a `&'a mut ChanData` with an unbounded lifetime; whether the generated code
  can observe a stale field through such a reference is outside the model.

### The slice-4 lead

The report (`reports/2026-10-19-select-fast-path-fragility.md`) ends with a
`send_fast` that queued on a channel whose `recv_watchers` held the parked
selector. In the model:

- The current prompt protocol satisfies `NoLostWake`, `RegisteredWhileWaiting`,
  `AtMostOnce` and `EveryBlockedRecvWoken` at one worker and at three, at
  capacity 0 and 1, with cancellation and spurious wakes (check `channel`,
  `channel-live`). The selection registers under its claim (probe,
  `watch_recv`, park are one locked region), so no send can slip between its
  probe and its registration; a send that finds the selector registered
  declines every unlocked path (`channel.rs:384`, `channel.rs:553`) and the
  locked send wakes it.
- The trace's last state is exactly what `Mutation = "fast_ignores_watchers"`
  (the guard at `channel.rs:384` without `recv_watchers.is_empty()`) produces:
  TLC reaches it in a 7-state trace — the selection parks, a sender takes the fast
  path and queues, the selection stays parked with the value queued — and
  reports it both as a `NoLostWake` violation and as a liveness violation
  (checks `m-fast-ignores-watchers`, `m-fast-ignores-watchers-live`).
- `Mutation = "select_no_claim"` drops the claim between the probe and the
  registration (the report's "check-then-register window"): TLC finds the lost
  wake in a 7-state trace (a `close_fast` between them). The window is real in the
  protocol and closed today only by the claim.

So the trace's state is not reachable by any interleaving of the modelled
steps: the guard and the print in the report cannot disagree unless the guard
read a value of `recv_watchers` that was not the current one. That points at
the report's question 1 (how `raw()` and `data()` derive their references,
`channel.rs:301-311`) and at what the model assumes away (above), not at the
wake-up protocol. The model does not establish the cause.

The report's point 2 (the queue branch of `send` wakes every watcher and forgets
it, `channel.rs:806-808`) is finding F3: it breaks `OneStep` and
`RegistrationServed`, but it does not lose a wake-up, because the woken
selection probes before it registers again and does so under its claim.

### Findings

**F1 — a value handed to a receiver that is then cancelled is lost** (check
`f1-cancel-after-handoff`, `ExactlyOnce`). A receiver parked in the locked
`recv` is handed a value (`wake_with` or `deliver`, status 1) and becomes
ready; before it runs, it is cancelled (the flag is set; the wake finds it
ready and does nothing). When it resumes, `park()` honours the cancellation
(`check_interrupts`, `task.rs:1375`) before the receive takes the delivered
value (`take_delivered`, `channel.rs:935`), and the task abandons. The sender
got success; nobody has the value. The language specification says "After
ownership commits, cancellation does not silently retract a message"
(`spec/05-tasks-and-channels.md:141`). By the same code path (not reproduced
separately), a parked selection that a rendezvous committed loses its value
too: `take_committed` runs after `park_for` returns. `repro/cancel_after_handoff.bdg` reproduces it
at `13e7756` with one worker ("empty: the sent value is gone"); with four
workers the receive takes the free path, which reads the delivery first
(`recv_free`), and the value arrives. The contract takes a completed hand-off
before honouring the cancellation.

**F2 — a published batch wakes one receiver; the rest of its messages strand**
(checks `f2-batch-one-wake`, `f2-batch-one-wake-live`, `NoLostWake` and
liveness). `publish` (`channel.rs:1081`) wakes one parked receiver
(`wake_one_receiver`, `channel.rs:659`) per sealed batch, but `recv_one` takes
one message: with two receivers parked and a batch of two published, the woken
receiver takes one message and returns, and the other stays parked with a
message published, until another publish or the close. Channel endpoints may be
shared and "multiple receivers distribute messages"
(`spec/05-tasks-and-channels.md:113-114`). `repro/batch_two_receivers.bdg`
reproduces it at `13e7756` with one and with four workers ("r1 got 1", then
"deadlock: all tasks are blocked", the second receiver waiting on batch
receive). The contract wakes one receiver per published message.

**F3 — the queue branch of `send` wakes the watchers and forgets them**
(checks `f3-one-step`, `f3-registration-served`). With a selection parked on a
channel of capacity 1, a locked send queues its value and wakes the selection
without handing it the value (`channel.rs:797` commits to a selection only at
capacity 0): the abstract state goes `Blocked` → `WithValue` in one step, and
the selection's registration is dropped. Not a lost wake-up (the current code satisfies
`NoLostWake`), but the woken selection races every other receiver for the value
and loops; §1.2's single waiter list with hand-off removes it, which the
contract checks.

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
