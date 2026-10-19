# Channel fast-path instrumentation perturbs a select golden

Recorded 2026-10-19 while working backlog item "sync slice 0, remaining
counters" (spin time, fast-to-slow fallback reasons). The counters were
implemented, measured, and then **not committed**: adding them turns
`tests/run/select_and_budget.bdg` into a deterministic deadlock. This file
records what was seen, what was ruled out, and what to look at next. Nothing
here is a landed change.

## What was added

Slice 0's remaining counters, in `channel.rs` + `task.rs`:

1. `task::fallback` (reason constants + names), six `AtomicU64` statics,
   `task::stat_fallback(reason, spins, spin_ns)` — all gated on the existing
   `task::stats_on()` (a `OnceLock` read of `BADGER_RUNTIME_STATS`), plus a
   `print_fallbacks()` table next to `print_reasons()`.
2. In `try_fast`: `let started = task::stats_on().then(std::time::Instant::now)`
   before the CAS loop, `iters += spins` per backoff round, and one
   `stat_fallback(BUSY, iters, elapsed_ns)` when the loop gives up.
3. In `send_fast`/`append_fast`/`recv_one_fast`/`recv_fast`: a *read-only*
   reason expression (`Option<usize>`, computed only when `stats_on()`) and a
   `stat_fallback(reason, 0, 0)` call on the decline path. No condition, no
   ordering, no state write in any fast path was changed.

## What it did

| build | `select_and_budget` (6 runs) |
|---|---|
| `main` (`d94bb08`) | passes 6/6, golden md5 `5570caef` |
| `main` + the counters above | **fails 6/6**, deterministic |

The failure is a scheduler deadlock, not a wrong value:

```
received on 1: from b
timed out as expected
sent on 1
won
panic: deadlock: all tasks are blocked
  note: task 1 (main) waits on select (channel #1, #4)
```

`BADGER_DEBUG_SCHED=1` at the deadlock check:

```
[sched] task 1 Parked cleanups=2 name=main
[sched] task 2 Parked cleanups=0 name=child of task 1
[sched] task 1 Parked cleanups=2 name=main
[sched] task 2 Done   cleanups=0 name=child of task 1
```

So the spawned child (which does `task.sleep(2ms)` then `late.sender.send(7)`)
**ran to completion**, while main stayed parked in its `select` on
`late.receiver`. `wait_events()` then found no timer, no outstanding external,
no listener and no queued event, `auto_advance()` found no controlled-clock
timer, and the loop reported a deadlock.

A trace of the same build (`watch_recv`/`unwatch`/`send`/`send_fast`/
`wake_waiting` prints, single worker) ends with:

```
[dbg] watch_recv ch=<late> task=1
[dbg] send_fast ch=<late> ok=true waiters=0 watchers=1 q=1
[dbg] wake t=1 reason=Cancelled state=Parked
```

i.e. a fast-path send **succeeded** on the very channel whose
`recv_watchers` contains task 1 at the moment of the print. `send_fast` only
queues when `d.recv_watchers.is_empty()` (and `d.send_waiters.is_empty()`), so
either the guard and the print saw different state for the same fabricated
`&mut ChanData`, or the watcher was installed between the two. Single worker,
so no other thread should be in either function.

## Ruled out

- **Scheduler interleaving alone.** `BADGER_CHAOS` sweeps on the *unperturbed*
  build: 40 seeds at 1 worker, 30 seeds at 2 and at 4 workers — no deadlock and
  no output difference. The window is not reachable by task-order perturbation
  at these seeds.
- **A check-then-register window in the select loop** (probe, then register,
  then park: a message queued between the probe and the registration would wake
  no one, since an unlocked send finds no watcher and a locked send pushes
  without waking this task). This is a real window, and `chan_select` /
  `chan_select_cases` were restructured to register *before* probing, with
  `unwatch_all` on every return path. **It did not change the perturbed
  outcome**: still 6/6 deadlock (the report now listing stale select offers —
  `channel #1, #2, #3, #4`). The restructure was reverted rather than shipped
  unverified: it is a change to the selection protocol and belongs with the
  sync workstream's slice 4 (selection via shared descriptors), which also
  covers two-party rendezvous ordering.
- **The counters' numbers themselves.** They were collected and are consistent
  with the 2026-10-19 backlog audit: on `bench/messages.bdg` at 1/2/4 workers
  the fast path declined 83 890 times with **1** "channel busy" (contention)
  and the rest semantic — 40 775 "a task waits on the channel", 43 115 "no
  room, or nothing to take" — plus 4351 spin iterations total. Multi-worker
  channel cost is not fast-path contention.

## Where to look next

1. Why `send_fast`'s guard and the trailing `d.recv_watchers.len()` disagree in
   the trace. Both go through `raw(ch)`'s fabricated `&'a mut ChanData`
   (`channel.rs:228`), and `data(ch)` is a second accessor — check they derive
   the same `ChanData`.
2. `recv_watchers` is **drained** by `send`'s queue-push branch and by
   `publish`/`close` (`std::mem::take(&mut d.recv_watchers)`): a send wakes
   every registered watcher *and* forgets them. Combined with `unwatch` after a
   park returns, a task is unregistered for the whole interval between
   `unwatch` and the next `watch_recv`, while it is running and a wake to it
   would be dropped (`wake_waiting` only wakes `Parked`/`Parking`, "never
   recorded as an early wake of a running task").
3. The `wake t=1 reason=Cancelled` in the trace: who cancels main there, and
   what the select loop does with a cancellation wake before `take_committed`.
4. The deadlock detector's inputs: `wait_events()` treats "no timer, nothing
   outstanding, no listener, nothing queued" as "nothing can happen", but a
   message sitting in a channel's queue whose watcher was forgotten is also
   "nothing can happen" — `auto_advance()` cannot help there.

## Reproducing

Rebuild `main` with the two hunks described under "What was added" (the
`try_fast` and `send_fast` ones are enough; the other three fast paths and the
`task.rs` statics are inert), then:

```sh
./target/release/badger run --release tests/run/select_and_budget.bdg
```

Six runs fail identically; the same binary without the hunks passes six out of
six. `cargo test -p badger_cli --release --test programs` (76 cases) passes on
both, so only this golden's timing exposes it.
