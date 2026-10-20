# TLA+ models of the runtime's synchronization

Phase 1 of [`OPTIMIZATION_BACKLOG.md`](../../OPTIMIZATION_BACKLOG.md): §1.1 (the
per-channel state machine) and §1.3 (the stop-the-world handshake). The channel
model describes `crates/badger_runtime` as of `ac3f85c` (the Go-style channel
protocol of §1.2 from `9f1ff4c`: one state word, one waiter list, `c4750f8`'s
hand-off rule, and `ac3f85c`'s claims, the fix for F4; line references are to
`ac3f85c`); the GC model describes it as of `6cd8e06` (the QSBR handshake
of §2.1). They are the contract that any polling page (§0.1) must keep.
Nothing here changes the runtime.

| File | What |
|---|---|
| `ChannelBadger.tla` | the channel state machine: the state word, the waiter list, fast paths, waits without the lock, locked paths, select, close, cancellation (plain TLA+) |
| `ChannelBadger.cfg` | the current code, prompt channels, safety (the regression gate) |
| `ChannelBadgerLive.cfg` | the current code, prompt channels, liveness |
| `ChannelBadgerContract.cfg`, `ChannelBadgerContractLive.cfg` | the §1.2 contract (`Impl = "contract"`), prompt and batch channels, every property; `run.sh` also runs them with `Impl = "current"` |
| `GcHandshake.tla`, `GcHandshake.cfg` | the stop-the-world handshake (PlusCal, with its translation) |
| `run.sh` | runs every check below and compares each outcome with the expected one |
| `repro/*.bdg` | Badger programs that reproduce findings F1, F2 and F4 (all fixed) on the real runtime |

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
| `channel` | pass | pass | 280,301 | 7s |
| `channel-live` | pass | pass | 275,956 | 51s |
| `channel-full` | pass | pass | 332,232 | 6s |
| `channel-full-live` | pass | pass | 421,815 | 132s |
| `contract` | pass | pass | 332,281 | 6s |
| `contract-live` | pass | pass | 421,575 | 84s |
| `contract-forgets-watchers` | WatchKept | WatchKept | 17,072 | 2s |
| `f1-cancel-after-handoff` | pass | pass | 280,301 | 5s |
| `f2-batch-one-wake` | pass | pass | 51,931 | 3s |
| `f2-batch-one-wake-live` | pass | pass | 145,859 | 26s |
| `f3-one-step` | pass | pass | 280,301 | 6s |
| `f3-registration-served` | pass | pass | 280,301 | 6s |
| `f4-cancel-after-retry-wake` | pass | pass | 6,446 | 1s |
| `f4-cancel-after-retry-wake-live` | pass | pass | 145,114 | 38s |
| `f4-no-cancel` | pass | pass | 2,749 | 2s |
| `f4-workers` | pass | pass | 359,150 | 8s |
| `f4-contract` | pass | pass | 6,386 | 4s |
| `f4-contract-live` | pass | pass | 143,274 | 49s |
| `m-fast-ignores-watchers` | NoLostWake | NoLostWake | 2,845 | 2s |
| `m-fast-ignores-watchers-live` | temporal | temporal | 275,206 | 28s |
| `m-select-no-claim` | NoLostWake | NoLostWake | 2,137 | 1s |
| `m-no-stale` | PhasePublished | PhasePublished | 377 | 1s |
| `m-no-stale-safe` | pass | pass | 279,466 | 5s |
| `m-cancel-retracts` | ExactlyOnce | ExactlyOnce | 20,912 | 3s |
| `m-one-wake-per-batch` | NoLostWake | NoLostWake | 14,950 | 1s |
| `m-one-wake-per-batch-live` | temporal | temporal | 144,473 | 19s |
| `m-one-wake-claimed` | NoLostWake | NoLostWake | 16,049 | 2s |
| `m-one-wake-claimed-live` | pass | pass | 146,147 | 39s |
| `m-queue-forgets-watchers` | OneStep | OneStep | 7,113 | 1s |
| `m-queue-forgets-watchers-reg` | RegistrationServed | RegistrationServed | 5,497 | 2s |
| `m-dead-keeps-claim` | NoLostWake | NoLostWake | 5,808 | 2s |
| `m-dead-keeps-claim-live` | temporal | temporal | 149,938 | 25s |
| `m-dead-keeps-claim-held` | ClaimsHeld | ClaimsHeld | 10,620 | 2s |
| `gc` | pass | pass | 60,597 | 11s |
| `gc-acks-stay` | AcksStay | AcksStay | 10,994 | 1s |
| `gc-recheck-parity` | pass | pass | 111,663 | 16s |
| `gc-online-no-publish` | EveryWorkerObservesEpoch | EveryWorkerObservesEpoch | 28,547 | 2s |
| `gc-online-no-publish-slot` | SlotMatches | SlotMatches | 1,020 | 1s |
| `gc-online-no-recheck` | EveryWorkerObservesEpoch | EveryWorkerObservesEpoch | 13,519 | 1s |
| `gc-online-no-backout` | temporal | temporal | 86,763 | 3s |
| `gc-online-no-backout-spin` | temporal | temporal | 50,307 | 4s |
| `gc-unlock-before-online` | EveryWorkerObservesEpoch | EveryWorkerObservesEpoch | 20,481 | 1s |
| `gc-offline-before-publish` | RootsPublished | RootsPublished | 12,492 | 2s |
| `gc-read-before-advance` | EveryWorkerObservesEpoch | EveryWorkerObservesEpoch | 15,340 | 1s |
| `gc-scan-before-grace` | EveryWorkerObservesEpoch | EveryWorkerObservesEpoch | 9,092 | 1s |
| `gc-nested-grace` | ScanUnderStop | ScanUnderStop | 14,410 | 1s |
| `gc-nested-grace-resume` | ResumeAfterSweep | ResumeAfterSweep | 14,434 | 2s |
| `gc-nested-resume-ends` | ScanUnderStop | ScanUnderStop | 14,237 | 1s |
| `gc-nested-resume-ends-resume` | ResumeAfterSweep | ResumeAfterSweep | 14,243 | 1s |
| `gc-poll-acks` | SlotMatches | SlotMatches | 3,279 | 1s |
| `gc-poll-acks-observes` | EveryWorkerObservesEpoch | EveryWorkerObservesEpoch | 13,705 | 1s |

In short: every check matched its expected outcome, so the channel protocol
as of `ac3f85c` (the refactor of `9f1ff4c` with the F4 fix) satisfies the
properties the backlog asks for -- `NoLostWake`, `RegisteredWhileWaiting`,
`AtMostOnce` / `ExactlyOnce`, `EveryBlockedRecvWoken`, `OneStep`,
`RegistrationServed` and `WatchKept` -- and the data-structure invariants
(`CountsMatch`, `OneEntryPerKind`, `PhasePublished`, `ClaimsHeld`), for
prompt and batch channels, at one and at three workers, with cancellation
and spurious wakes; and, at four tasks (three batch receivers), with a woken
receiver cancelled before it runs. All four findings are fixed: the checks of
F1-F4 pass, and each comes back as a mutation that puts the old code back
(`m-cancel-retracts`, `m-one-wake-per-batch`, `m-queue-forgets-watchers`,
`m-dead-keeps-claim`). The §1.2 contract now carries F4's rule (a woken
receiver's claim passes on) and passes at four tasks too. The model finds no
new defect. One observation: the claims also make one wake per publication
live (each receiver that takes a message passes the next wake on), so F2's
per-message wake is now needed for `NoLostWake` but no longer for
`EveryBlockedRecvWoken` (`m-one-wake-claimed`, `m-one-wake-claimed-live`).

The GC handshake model now describes the QSBR grace period of `6cd8e06`,
with a nested stop in every collection: `EveryWorkerObservesEpoch`,
`RootsPublished`, `ScanUnderStop`, `ResumeAfterSweep`, `SlotMatches` (was
`CountMatches`), `CollectionCompletes` and `SpinnersProceed` all hold
(`gc`), and all fifteen checks of the ten mutations catch them. The
model finds no defect; it finds that `stop_the_world`'s comment overstates
(G1: a slot read as acknowledged can fall back to the old epoch in
`online`'s window, harmlessly, `gc-acks-stay`), and that `online`'s re-read
could test the parity alone (`gc-recheck-parity`).

Times are wall-clock for one `run.sh` run (10m20s in all) on a 16-core
machine that other jobs were loading (load average 15-25), so they are
higher than on a quiet machine (`channel-live` took 33s on one, 51s here).
The state count of a violated property is where TLC stopped, which varies
slightly between runs with several TLC workers; the counts of passing checks
do not vary.

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
`task.rs:2144`: set the flag, `wake_waiting(Cancelled)`) and `MaxSpurious`
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
- **The claims** (`ChanData::woken`, `channel.rs:257`): `c.woken`, the
  receivers a batch publication woke (`wake_receivers`, `channel.rs:1022`) and
  that have not taken a message yet. A receiver releases its claim
  (`release_claim`, `channel.rs:1044`) when it takes a message, when it finds
  the channel closed, when it finds nothing and registers again (every way
  out of `recv_one`, `channel.rs:1654`), and when it dies (`task_finished`,
  `channel.rs:1729`, the model's `Leave`). Releasing a claim passes it on:
  if more messages are published than claims are held, one more parked
  receiver is woken, and holds a claim in turn. The model keeps a set:
  a task holds at most one claim (it registers again only after releasing).

The atomicity follows the code's ownership protocol:

- a fast path owns the channel from its compare-and-swap in `try_fast`
  (`channel.rs:602`) to `end_fast` (`channel.rs:636`), so its body is one
  step, enabled only while no claim is held, ending with the store that
  publishes `Word(c)` and clears `STALE`; giving up after the spin is the same
  step's decline branch. `recv_fast` and `recv_one_fast` load the word first
  and decline on a published `EMPTY` phase (`published`, `channel.rs:584`)
  without a compare-and-swap; the decline is a branch of the same step (a
  decline only moves the task to its slower path, so splitting the load from
  the compare-and-swap adds no behaviour);
- code holding the runtime lock claims the channel on its first `data()`
  (`claim`, `channel.rs:534`) and keeps the claim until the lock is released
  (`release_claims`, `channel.rs:560`, called from `unlock`, `task.rs:784`),
  which publishes the phase: in the model, `Claim` at the step that takes the
  lock and `Rel` at the step that releases it. With one worker nothing claims:
  `data()` sets `STALE` (`Claim` and `Rel` do), so the next fast path decides
  from the contents. No fast path runs inside a locked operation; a locked
  operation that parks releases the lock in the parking step (the real worker
  may keep it longer, which only removes interleavings);
- park, wake and deliver are atomic per task (`with_task`); `Waiting` follows
  the task states of `task.rs:101` (`Parking` with `pending_wake`, `Parked`),
  `wake_waiting` drops a wake to a running task (`task.rs:1942`), `deliver`
  and `waits` (`task.rs:414`, `2010`) check the wait generation, the cancel
  flag and the state; `park` honours a cancellation that arrived during the
  wait only when no waker completed it with status 1
  (`interrupts_after_wait`, `task.rs:1380`);
- with one worker at most one task runs and runs until it parks or finishes;
  the waits without the lock do not exist then (`can_suspend_free`,
  `task.rs:1416`).

The operations decide as the code does: a send hands its value to the first
live parked receiver, else commits it to the first parked selection offering
to receive (at every capacity; `commit_select_receive`, `channel.rs:1234`,
which checks `waits`), else queues it without waking anyone, else parks; a
selection's `WatchRecv` entry is removed only by its `unwatch`, its end or the
close; a publication (`channel.rs:1502`) pops and wakes one live receiver per
message (`wake_receivers`), each of which holds a claim, and wakes every
watcher without removing it (`wake_watchers`); a receive that frees a slot
moves the first live parked sender's value into it; `recv_one` wakes and
removes every producer waiting for capacity. A selection never holds a claim
(it is woken as a watcher), so the `release_claim` in its `recv_one(ch,
false)` probe changes nothing and the model leaves it out; `recv_one_fast`
releases nothing either, since a woken receiver resumes inside `recv_one`.

`Impl = "contract"` is the §1.2 design as the model encoded it before the
refactor: a send that finds a parked selection commits to it at every capacity,
a queued value leaves the watchers registered, a publication wakes one
receiver per message, and a completed hand-off is taken before a pending
cancellation is honoured. Since F4 the contract also states that the wake a
publication gives a receiver is a claim on a message, and that a receiver
which leaves without taking one -- cancelled before it ran, or finding
nothing -- passes its claim on (the same claims as the code: §1.2 had the same
gap, F4 below). The code meets it, and differs from it in two places, neither
a deviation from §1.2:

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
   published messages may not exceed them. A woken receiver about to honour a
   cancellation takes no message, so it counts only while it holds a claim,
   which it passes on as it leaves (without that clause the fixed code would
   fail `NoLostWake` in the one state between the cancellation and the leave,
   whose next step wakes another receiver; see F4). With three tasks there are
   at most two receivers and the bound is 0 for batches too (the form the
   model used before the refactor); with three receivers a batch of two
   legitimately leaves the third parked. The liveness form
   `EveryBlockedRecvWoken`: every waiting receiver stops waiting, unless
   nothing is deliverable and every task that could still send or close has
   finished. The literal "every blocked
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

And one about the F4 fix:

- `ClaimsHeld`: every receiver holding a claim is on its way to the channel
  (woken in `recv_one` and not yet run, or in its body under the lock). A
  claim kept by a task that left undercounts the unclaimed messages for
  good; `ClaimsHeld` sees that at once, three tasks are enough, where
  `NoLostWake` needs a third receiver to be left waiting.

### What the channel model does not cover

- Unbounded runs: three tasks, one channel, at most two operations per task
  (one in the liveness runs and the four-task runs), one cancellation and one
  spurious wake. The four-task runs (the F4 checks) are batch channels only,
  one operation per task; at one worker without a spurious wake, except
  `f4-workers` (one worker and one per task, a spurious wake, safety only).
  A defect that needs more of any of these is not excluded (F4 needed four
  tasks).
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
  (`channel.rs:522`) still hands every caller a `&'a mut ChanData` with an
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
- with three receivers (four tasks, batch channels), the fixed code and the
  contract satisfy `NoLostWake`, `RegisteredWhileWaiting`, `AtMostOnce` /
  `ExactlyOnce`, `ClaimsHeld` and the other invariants, and
  `EveryBlockedRecvWoken`, with a cancellation: a woken receiver that is
  cancelled before it runs passes its claim on (checks
  `f4-cancel-after-retry-wake`, `f4-cancel-after-retry-wake-live`,
  `f4-workers`, `f4-contract`, `f4-contract-live`);
- the defects of the code before the refactor and before the F4 fix, put back
  one at a time as mutations, are caught by the same properties as before
  (checks `m-cancel-retracts`, `m-one-wake-per-batch`,
  `m-one-wake-per-batch-live`, `m-queue-forgets-watchers`,
  `m-queue-forgets-watchers-reg`, `m-dead-keeps-claim`,
  `m-dead-keeps-claim-live`, `m-dead-keeps-claim-held`), and so are the two
  mutations of the slice-4 analysis;
- the claims also repair a publication that wakes too few receivers: with
  one wake per publication, `NoLostWake` fails but `EveryBlockedRecvWoken`
  holds (`m-one-wake-claimed`, `m-one-wake-claimed-live`; F2 below).

What it cannot establish: anything beyond the bounds (in particular more than
two receivers, except in the F4 checks, and more than three receivers at
all), the unmodelled operations above, and the memory-model question below.

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
fixed at `9f1ff4c`; F4 was found in the refactored code and is fixed at
`ac3f85c`. Every check of a finding now passes, and each finding comes back as
a mutation that puts the old code back.

**F1 — a value handed to a receiver that is then cancelled was lost (fixed in
`c4750f8`).** A receiver parked in the locked `recv` was handed a value
(`wake_with` or `deliver`, status 1) and became ready; before it ran, it was
cancelled. When it resumed, `park()` honoured the cancellation before the
receive took the delivered value, and the task abandoned: the sender got
success and nobody had the value (spec 05: "After ownership commits,
cancellation does not silently retract a message"); a committed selection lost
its value the same way. `park()` now honours an interrupt after a wait only
when no waker completed the wait with status 1 (`interrupts_after_wait`,
`task.rs:1380`); the model's resume steps take a completed hand-off first
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
"one_wake_per_batch"` (one wake per publication and, as before `ac3f85c`, no
claims) brings both violations back. With the claims, one wake per
publication is no longer a liveness defect (`Mutation = "one_wake_claimed"`):
the woken receiver takes a message, releases its claim, finds a message
nobody is on its way to and wakes the next receiver, so `NoLostWake` still
fails (a receiver waits while a message no one is coming for is published:
`m-one-wake-claimed`) but every blocked receive is woken
(`m-one-wake-claimed-live` passes). The per-message wake is now a latency
matter -- receivers wake together rather than one after another -- with the
claims as a second line of defence. `repro/batch_two_receivers.bdg`
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

**F4 — a batch receiver woken to retry and then cancelled took its wake with
it (fixed in `ac3f85c`).** A publication wakes one receiver per message, but
the wake is a wake to *retry*: the woken receiver's `Recv` entry is popped
(`wake_receivers`, `channel.rs:1022`) and it takes a message only when it
runs. If it was cancelled (or a child failure was pending) before it ran,
`park()` honoured the interrupt — rightly, nothing was handed to it, its
status is not 1 — and the task abandoned; nothing passed its wake on. TLC's
safety trace (four tasks: three `recv_one` receivers and a producer, batch
channels, one worker, `Ops = 1`, one cancellation, no spurious wake):

1. receivers t1, t2, t3 park in `recv_one` (entries `Recv` t1, t2, t3);
2. producer t4 appends twice; the second append fills the batch and publishes
   it: `wake_receivers(d, 2)` pops and wakes t1 and t2; t3 stays parked;
3. t4 finishes; t1 is cancelled before it runs (the flag is set; the wake
   finds t1 ready and does nothing). Two messages are published, one
   uncancelled receiver is on its way, and t3 waits: `NoLostWake` failed
   (5,125 states). The liveness trace went on: t1 resumes, `park()` honours
   the cancellation and t1 abandons (`task_finished` had nothing to remove);
   t2 takes the first message and finishes; t3 stays parked forever with the
   second message published.

Three tasks cannot show it: with at most two receivers, a batch of two wakes
both, and a batch of one is published only by a close (which wakes everyone)
or by a cancelled producer (which spends the one cancellation). The §1.2
contract had the same gap (it also woke one receiver per message and said
nothing about a woken receiver that leaves). Prompt channels are not affected
(a prompt value is handed over with status 1, which F1's fix protects; a
woken selection is one of all the watchers woken).

The fix (`ac3f85c`) is the "pass the wake on" option this finding proposed,
made exact: the channel keeps the receivers it woke for a published message
(`ChanData::woken`), each receiver releases its claim when it takes a message,
finds the channel closed, finds nothing (and registers again) or dies
(`task_finished`), and releasing a claim wakes another parked receiver while
more messages are published than claims are held. The model now has the
claims (`c.woken`, `WakeRecvs`, `ReleaseClaim`, `Leave`), for the code and for
the contract, and:

- checks `f4-cancel-after-retry-wake` (safety) and
  `f4-cancel-after-retry-wake-live` (`EveryBlockedRecvWoken`) pass, and so
  does `f4-no-cancel`; in the trace above, t1's leave wakes t3, which takes
  the second message;
- `f4-workers` passes too: the same four tasks at one worker and at one worker
  per task, with a spurious wake;
- the contract with the claims passes at four tasks (`f4-contract`,
  `f4-contract-live`);
- `Mutation = "dead_keeps_claim"` (a woken receiver that dies keeps its claim:
  `Leave` withdraws its registrations but not its claim — the code before
  `ac3f85c` as far as the message count goes) brings F4 back:
  `m-dead-keeps-claim` (`NoLostWake`, the trace above, one step later: t1
  leaves holding its claim, so the unclaimed count is 0 and nobody is woken),
  `m-dead-keeps-claim-live` (liveness) and, already at three tasks,
  `m-dead-keeps-claim-held` (`ClaimsHeld`: a task that has finished holds a
  claim).

`Retrying` changed with the fix, as the finding predicted: a woken receiver
that is about to honour a cancellation is on its way while it holds a claim
(its leave passes the claim on). Without that clause `NoLostWake` would flag
the fixed code in the single state between the cancellation and the leave;
the liveness checks, which do not depend on `Retrying`, pass either way, and
the mutation still fails `NoLostWake` with the new clause, in the state after
the leave.

`repro/cancel_after_retry_wake.bdg` is the bug as a deterministic program
(two receivers parked on a batch channel, one message published by `flush`,
which wakes the first, which is cancelled before it runs): it reports what the
channel holds afterwards, "stranded 1" before `ac3f85c` and "taken" after it
(the second receiver was woken in the first one's place). `flush` is not in
the model; there, the batch of two with three receivers plays its part. The
regression test is in `crates/badger_runtime/tests/channel_state.rs`.

## The GC handshake model (`GcHandshake.tla`)

### What is modelled

The handshake of `6cd8e06`: a quiescent-state grace period (QSBR,
`gc_handshake.rs`), which replaced the `STOP` flag and the sum of the
`IN_COMPILED` counters of `13e7756`. One process per worker thread, three
workers, `MaxCollections = 2` collections, each of which may contain one
nested stop and resume. Each label is one sequentially consistent access,
following the code:

| Code | Model |
|---|---|
| `EPOCH` (`gc_handshake.rs:25`): even while workers may run compiled code, odd from a collector's request until its resume | `epoch` |
| `DEPTH` (`gc_handshake.rs:30`): the lock holder's stops in progress | `depth` |
| `OBSERVED` (`gc_handshake.rs:38`): per worker, the even epoch it observed going online, or `OFFLINE` (greater than every epoch) | `slot`, with `Offline` above every epoch the bounded model reaches |
| `online` (`gc_handshake.rs:48`, from `gc_enter`, `task.rs:1503`): read the epoch; if even, publish it in the slot and re-read the epoch; if it moved, go offline; spin while it is odd; retry | `GERead`, `GEPub`, `GERecheck`, `GEBack`, `GESpin` |
| `online_locked` (`gc_handshake.rs:69`): publish the epoch, lock held (its load and its swap are one step: the lock excludes a stop) | `LeaveFirst`, `ForeignOnline` |
| `offline` (`gc_handshake.rs:79`): the slot becomes `OFFLINE` | `EnterOff`, `HSecond`, `SuspendOff`, `ForeignOff` |
| `stop_the_world` (`gc_handshake.rs:93`): count the stop; the outermost one advances the epoch and waits until each slot is at least the new epoch, one slot at a time | `CStop`, `CAdvance`, `CWait` (one slot per step, in every order); a nested one `NStop` |
| `resume_the_world` (`gc_handshake.rs:109`): uncount; the outermost one advances the epoch | `CResume`; a nested one `NResume` |
| `collect` (`gc.rs:650`): stop, trace and sweep, resume; `for_each_object_of` (`gc.rs:1339`) stops and resumes inside it | `CScan`, `CSweep` (then `NStop` ... `NResume` or not), `CResume` |
| `badger_rt_enter` (`task.rs:874`): offline, take the lock | `EnterOff`, `EnterLock` |
| `badger_rt_leave` (`task.rs:881`): online (lock held), unlock | `LeaveFirst`, `LeaveSecond` |
| `enter_runtime_holding` (`task.rs:922`): publish `RETURNED`, offline, lock, root, clear | `HFirst`, `HSecond`, `HLock`, `HRoot` |
| `badger_rt_safe_begin` / `_end` (`task.rs:889-899`): offline; lock, online, unlock | `ForeignOff`, `ForeignLock`, `ForeignOnline`, `ForeignUnlock` |
| `suspended_free` (`task.rs:3254`): a task suspended without the lock takes its worker offline | `SuspendOff` |
| the scheduler with the lock: run a locked task, `run_free`, go idle, or collect | `Runtime`, `IdleLock`, `FreeSched`, `FreeLock` |

A worker "may mutate" (`MutatorPCs`) while it runs compiled code, or runtime
code that compiled code calls without the lock (allocation, the channel fast
paths), up to the step that takes it offline. The ghost variables are `seen`
(the epoch a worker observed going online: `online`'s local `e`), `ack` (the
epoch whose grace period last read the worker's slot as acknowledged) and
`phase` / `collector`. Every worker starts online at epoch 0.

### Properties

The properties of the model of `13e7756` carry over; one is renamed because
the counters are gone.

- **Every worker observes the new epoch** — `EveryWorkerObservesEpoch`: while
  the collector traces or sweeps, no other worker may mutate, each of them was
  read as acknowledged by this epoch's grace period (`ack[w] = epoch`), and
  its slot still acknowledges the epoch (`slot[w] >= epoch`) unless the worker
  is in `online`'s window between publishing and backing out (finding G1).
  Being an invariant of every state of the scan, it also says that no worker
  enters compiled code after the scan started. The acknowledgement is no
  longer a ghost: it is the slot the collector reads (`ack` only records the
  read). **Established.**
- **No root published after the scan starts** — `RootsPublished`: during the
  scan, a pointer that compiled code returned to a worker that is not yet
  holding the lock is in `RETURNED`. **Established.**
- **The resume is ordered after the scan** — `ScanUnderStop` (tracing and
  sweeping happen with the epoch odd, inside a stop, with the lock held by
  the collector) and `ResumeAfterSweep` (the epoch turns even only at the
  collector's outermost resume, after its sweep, at depth 1; it used to say
  "`STOP` clears only..."). **Both established**, now with nested stops in
  the model rather than as a mutation.
- `SlotMatches` (was `CountMatches`: there are no counts to match): a slot is
  `OFFLINE` exactly while its worker is in a quiescent state, and otherwise
  the even epoch the worker observed going online. **Established.** It is no
  longer restricted to the unmutated model, so it catches mutations too.
- Liveness: `CollectionCompletes` (a started collection ends; this needs every
  worker in compiled code to reach a safe point, the model's fairness for the
  poll) and `SpinnersProceed` (a worker waiting in `online` runs again).
  **Both established.**

`AcksStay` (the claim of `stop_the_world`'s comment, "a slot read as
acknowledged stays so for the stop") is defined too, and fails: finding G1.

### Mutations

Each property has a mutation that breaks it, and every mutation is caught.
The checks keep `TypeOK` and the one property they name (`run.sh`'s
`gconly`), so each shows that this property alone catches the defect; the
trace length is that of TLC's counterexample (one TLC worker thread;
with several, the trace found can differ).

| Mutation | The defect | Check | Caught by | Trace |
|---|---|---|---|---:|
| `online_no_publish` | `online` returns without publishing its slot | `gc-online-no-publish`, `gc-online-no-publish-slot` | `EveryWorkerObservesEpoch`; `SlotMatches` | 20; 6 |
| `online_no_recheck` | `online` publishes and does not re-read the epoch (a stop that began between the read and the publication passes the worker's `OFFLINE` slot) | `gc-online-no-recheck` | `EveryWorkerObservesEpoch` | 19 |
| `online_no_backout` | `online`, seeing a stop after publishing, waits for it with its slot published: the collector waits for the worker, the worker for the collector | `gc-online-no-backout`, `gc-online-no-backout-spin` | `CollectionCompletes`; `SpinnersProceed` (a deadlock: `SlotMatches` sees it too) | 18 + stutter |
| `unlock_before_online` | `badger_rt_leave` unlocks, then goes online: a collector takes the lock, reads the slot as `OFFLINE`, and the worker publishes the odd epoch and runs | `gc-unlock-before-online` | `EveryWorkerObservesEpoch` | 20 |
| `offline_before_publish` | `enter_runtime_holding` goes offline before publishing `RETURNED` | `gc-offline-before-publish` | `RootsPublished` | 16 |
| `read_before_advance` | the collector waits for the slots, then advances the epoch: a worker read before the advance goes online at the old epoch | `gc-read-before-advance` | `EveryWorkerObservesEpoch` | 20 |
| `scan_before_grace` | the collector reads each slot once and goes on (the scan starts before the grace period ends) | `gc-scan-before-grace` | `EveryWorkerObservesEpoch` | 12 |
| `nested_grace` | no `DEPTH`: a nested stop advances the epoch (to an even one: the world runs during the sweep) and waits again | `gc-nested-grace`, `gc-nested-grace-resume` | `ScanUnderStop`; `ResumeAfterSweep` | 19 |
| `nested_resume_ends` | `DEPTH` counted on the stop only: the nested resume advances the epoch, ending the stop while the outer collection sweeps (the old `nested_collect`; the outer resume then stops the world for good) | `gc-nested-resume-ends`, `gc-nested-resume-ends-resume` | `ScanUnderStop`; `ResumeAfterSweep` | 19 |
| `poll_acks` | a worker acknowledges an epoch it has not observed going online: compiled code at a poll publishes the current epoch and runs on (urcu-qsbr's `rcu_quiescent_state`) | `gc-poll-acks`, `gc-poll-acks-observes` | `SlotMatches`; `EveryWorkerObservesEpoch` | 8; 14 |

The mutations of the model of `13e7756`, and what became of them:

- `gc-check-before-inc` (`gc_enter` read `STOP` before counting itself) is
  gone: `gc_enter` no longer counts. Its rule -- store, then load, on both
  sides -- is now `online`'s publish-then-re-read and the collector's
  advance-then-read: `gc-online-no-recheck` and `gc-read-before-advance`.
- `gc-unlock-before-inc` is now `gc-unlock-before-online`: the same rule
  (`online_locked` must publish before the unlock) for the new primitive.
- `gc-dec-before-publish` is now `gc-offline-before-publish`: the same rule
  (`RETURNED` before the worker goes offline).
- `gc-nested-collect` (an inner resume cleared the `STOP` flag under the
  outer collection) is gone as a mutation of the code: `DEPTH` makes nested
  stops correct, so the base model now nests one in every collection and
  `gc` passes with it. The two ways `DEPTH` could be wrong are
  `gc-nested-grace` and `gc-nested-resume-ends` (the latter is the old
  defect, made possible again).

The mutations `poll_acks` and `online_no_backout` are new kinds: the first
shows why the classic QSBR announcement does not fit a stop-the-world (a
grace period alone only waits out the readers that were there; the collector
also needs every worker to stay out until the resume, which only the safe
point's offline-then-lock gives), the second why `online` goes offline while
it waits.

### Findings

**G1 — a slot read as acknowledged can fall back to the old epoch (harmless;
the comment overstates).** `stop_the_world`'s comment says "A slot read as
acknowledged stays so for the stop: outside the lock a worker goes online
only in `online`, which backs out." The conclusion holds, but the slot does
not stay acknowledged: a worker that read the even epoch `e` before the
collector advanced it, and whose `OFFLINE` slot the collector has already
read, then publishes `e` (below the stop's `e + 1`) and only backs out after
its re-read. TLC's trace (`gc-acks-stay`, 13 states): w1 takes the lock and
starts a stop; w2 suspends without the lock (offline) and enters `online`,
reading epoch 0; w1 advances the epoch to 1 and reads w2's slot as `OFFLINE`;
w2 publishes 0 -- the slot of a worker the grace period acknowledged is now
below the epoch. w2's re-read sees 1, it goes offline and spins, so no worker
runs compiled code during the scan (`EveryWorkerObservesEpoch` holds, with
the window as its stated exception). Nothing reads the slots after the grace
period, so nothing depends on the stronger claim today; an assertion at the
resume that every slot acknowledges the epoch, or a collector that re-reads
the slots, would trip on it. The comment could say: "a worker whose slot was
read as acknowledged does not run compiled code until the resume: outside the
lock it goes online only in `online`, which re-reads the epoch and backs
out". (The runtime is not changed by this task.)

**Observation — `online`'s re-read could test the parity alone.**
`Mutation = "recheck_parity"` makes the re-read accept any even epoch, not
only the one published, and every property still holds (`gc-recheck-parity`,
111,663 states): a worker that publishes `e` after a whole stop and resume
(`e + 2`) runs with a slot below the current epoch, which only makes the next
stop wait for it, as it must. The equality test is stricter than it needs to
be, and costs nothing; this is not a proposal to change it.

The model finds no defect in the QSBR protocol.

### What the QSBR protocol relies on

The handshake satisfies every property above in the model (check `gc`). What
that depends on, each shown by a mutation:

- **Publish, then re-read; advance, then read; sequentially consistent.** A
  worker going online without the lock publishes its slot before it re-reads
  the epoch, and the collector advances the epoch before it reads the slots,
  so one of them sees the other (`online_no_recheck`, `read_before_advance`;
  `online_no_publish` takes the worker's half away altogether). The model is
  sequentially consistent; the code gets the same guarantee because the four
  accesses are `SeqCst` (`gc_handshake.rs:51-55`, `98-100`). With
  release/acquire the store-load pairs may reorder -- the same interleavings.
- **Online before unlocking.** `badger_rt_leave` and `badger_rt_safe_end`
  publish their slot while still holding the lock (`unlock_before_online`).
- **Publishing the root before going offline.** `enter_runtime_holding`
  stores `RETURNED` before `badger_rt_enter` goes offline
  (`offline_before_publish`).
- **The grace period waits for each slot.** The collector spins on each slot
  until it acknowledges the epoch (`scan_before_grace`), and a worker
  acknowledges it only by going offline, never by announcing the epoch from
  compiled code (`poll_acks`).
- **Backing out while waiting.** `online` stores `OFFLINE` before it spins on
  an odd epoch (`online_no_backout`: a deadlock).
- **`DEPTH` on both sides.** Only the outermost stop advances the epoch and
  waits, and only the outermost resume ends the stop (`nested_grace`,
  `nested_resume_ends`). `DEPTH` itself is `Relaxed` and touched only by the
  lock holder, which the model takes as given (one step of the lock holder).
- **Reading the slots one at a time is safe** because outside the lock a
  worker goes online only in `online`, which re-reads the epoch and backs out
  (finding G1 is the precise form); the model reads the slots in every order.

The dependencies the model of `13e7756` spelled out and that are gone:

- **No nested stop.** A nested stop and resume is now in every collection of
  the base model, and the properties hold.
- **Balanced slots.** The collector tests each slot against the epoch rather
  than a sum, so no slot can hide another; `SlotMatches` checks each slot
  against its own worker's state. (The code's `debug_assert!`s for going
  online or offline twice are the release-build-unchecked remnant of the old
  pairing claim; the model covers the paths in the table.)
- **Implicit acknowledgement.** The per-worker observed epoch §2.1 asked for
  is the slot; `ack` only records that the collector read it.

§0.1's polling page would replace the `online` / `badger_rt_enter` paths and
must keep `EveryWorkerObservesEpoch`.

### What the GC model does not cover

The memory model (it is sequentially consistent), the roots themselves (task
shadow stacks, `Roots`, the remembered sets: only `RETURNED` is modelled),
allocation debt and the collection trigger, the polling page (§0.1 is not
adopted), the `stopping()` hint in `yield_free` (it only sends a poll to the
locked path sooner), compiled code run by the collector during its own stop
(a finalizer calling back into compiled code would go `online_locked` at an
odd epoch, which `online_locked`'s `debug_assert!` rules out in debug builds
and the model does not include), epoch wrap-around (`u64`), more than one
nested stop per collection, and runs with more than three workers or two
collections.
