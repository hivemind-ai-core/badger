# Backlog

What remains of the archived plans ([archive/plans](archive/plans)), each
item with where it came from. Everything else in those plans is
implemented; their status headers say what and where. Framework leftovers
are in [FRAMEWORK_DESIGN.md](FRAMEWORK_DESIGN.md) §25.

Premises below were re-measured on 2026-10-19; the evidence, file:line
pointers, and a ranked verdict per item are in
[reports/2026-10-19-backlog-audit.md](reports/2026-10-19-backlog-audit.md).

## Runtime synchronization (LOCK_OPTIMIZATION.md)

Multi-worker cost is moving channel state between cores, not lock waits
([2026-10-16 channel round report](reports/archive/benchmarks/2026-10-16-lock-round1-report.md));
re-measured 2026-10-19: `bench/messages.bdg` is 78 ms at 4 workers against
36.6 ms at 1, with 3941 lock acquisitions and 0.96 ms parked. These slices
are correctness-and-scale work, not current throughput wins.

- Slice 0, remaining counters: spin time, channel fast-to-slow fallback
  reasons, GC request-to-acknowledge latency, timer lateness, a metrics-off
  control run. The channel fast-path counters were implemented and measured on
  2026-10-19 and **not kept**: adding them flips `tests/run/select_and_budget`
  into a deterministic deadlock. The measurement is still recorded in
  [2026-10-19-select-fast-path-fragility.md](reports/2026-10-19-select-fast-path-fragility.md)
  (the fast path declines for semantic reasons, not contention: 1 "busy"
  against 83 890 declines on `bench/messages.bdg` at 1-4 workers), and that
  file is also the lead for the wakeup fragility itself — the highest-value
  thing in this section right now. **The Phase 1 model (2026-10-19,
  `perf/optimization-backlog-2026-10-19`, `spec/tla/`) reproduces the
  audit's counter *without* the deadlock** (6/6 passes with the
  instrumentation, and a chaos sweep of 25 seeds at 1, 2 and 4 workers with
  the counters on and off), so the audit's failure was timing-sensitive, not
  a counter defect. The model of the current code satisfies `NoLostWake`,
  `RegisteredWhileWaiting`, `AtMostOnce` and `EveryBlockedRecvWoken` at its
  bounds, and finds three things instead:
  - **F1 (spec violation):** a value handed to a receiver that is then
    cancelled is lost — the sender got success and nobody has the value,
    against `spec/05-tasks-and-channels.md:141` ("After ownership commits,
    cancellation does not silently retract a message"). Reproduced with one
    worker (`spec/tla/repro/cancel_after_handoff.bdg`).
  - **F2 (spec violation):** a published batch wakes one receiver and the
    rest of its messages strand, against `spec/05-tasks-and-channels.md:113`
    ("multiple receivers distribute messages"). Reproduced with one and with
    four workers (`spec/tla/repro/batch_two_receivers.bdg` deadlocks).
  - **F3:** the queue branch of `send` wakes the watchers and forgets them;
    not a lost wake-up, but the woken selection races every other receiver
    for the value.
  F1, F2 and F3 were part of the slice 4 lead's acceptance, and the
  Go-style channel refactor (`9f1ff4c`, in `main`) fixes all three: their
  checks pass, each comes back as a mutation that puts the old code back,
  and the two repro programs pass.
- **F4 (open, found by the model of the refactored protocol, 2026-10-20):**
  a batch receiver woken to *retry* and then cancelled takes its wake with
  it -- the publication wakes one receiver per message, but the woken
  receiver takes a message only when it runs, and a cancellation before it
  runs makes `park()` honour the interrupt and abandon, so the message is
  stranded. Reproduced on the real runtime
  (`spec/tla/repro/cancel_after_retry_wake.bdg`: "r2 got 1", "r1 cancelled",
  then a deadlock with the third receiver waiting). It needs four tasks, so
  it is outside the three-task bounds the model normally uses, and the §1.2
  contract has the same gap: the contract must pass a cancelled receiver's
  wake on to another receiver.
- Slice 1, ownership APIs: split `ChanData` into immutable configuration
  and guarded state; stable waiter records independent of `Task`.
- Slice 2, GC coordination without the runtime lock: collector election,
  request epochs, per-worker states with a modelled protocol, scoped
  `NoGc`. Highest risk; needs a state-machine model first.
- Slice 3, rest: a scoped channel guard replacing `CLAIMED` /
  `release_claims`; byte-budget waits, watcher cases, and batch slow paths
  off the lock.
- Slice 4: selection, close, and cancellation through shared descriptors
  (two-party rendezvous ordering).
- Slice 4 lead (found 2026-10-19): a selection parked on a channel can stay
  parked while the only sender takes the unlocked fast path and queues its
  message without waking it. A ~1 ns/call addition to the fast paths makes
  `tests/run/select_and_budget` deadlock deterministically; `BADGER_DEBUG_SCHED`
  shows the sender task `Done` with the selector still `Parked`. Registering
  the selection *before* probing its cases (the obvious window) did **not**
  change that outcome, so the cause is elsewhere — the trace ends with a
  fast-path send succeeding on a channel whose `recv_watchers` holds the
  parked selector. Evidence, trace, what is ruled out (chaos sweeps at
  seeds/workers that do not reproduce it) and what to check next:
  [2026-10-19-select-fast-path-fragility.md](reports/2026-10-19-select-fast-path-fragility.md).
- Slice 5: producer-owned batch staging and credits; stored charges.
- Slice 6, rest: a timer lock (timers are on the runtime lock), per-worker
  park tokens in place of the global idle state.
- Slice 7, rest: per-scope locks; inspection snapshots formatted outside
  the lock; the per-entry lock/GC audit table. (Console output is done.)
- Slice 8: specialized rings, only with measured benefit.
- Gates: debug lock-order assertions, state-machine models for slices 2
  and 4, p99 latency and timer-lateness gates.

## Compiler and runtime performance (OPTIMIZATION_PLAN.md)

- C1.2b: parse and check once per session across a project's `[entries]`
  and `[tasks]`. Parsing is done 2026-10-19
  (`perf/frontend-reuse-2026-10-19`): `Session` keeps the parsed standard
  modules, so a four-entry `badger check` parses 34.10 → 12.34 ms and its
  front end 119.34 → 98.53 ms. The typecheck half (≈16 ms per extra entry)
  remains; the plan for it is [2026-10-19-std-check-reuse-plan.md](reports/2026-10-19-std-check-reuse-plan.md)
  (reuse the std prefix of the check, keyed by the std module set) and is
  being implemented. The `badger test` gap is closed: selections reuse one
  session per package (`Session::reset_for_build`), so they get both caches;
  the first build in each loop still pays the standard library.
- C1.3/C1.4: a reusable compiler context with a content-keyed parse cache
  (remapping spans, file ids, and symbols), then checked-library reuse.
  Parsing is the landed C1.2b work (above). Checked-library reuse landed
  2026-10-19 (`perf/std-check-reuse-2026-10-19`): `Session` keeps a `StdCache`
  keyed by the std module set, so each further entry re-checks only its own
  code — one entry's typecheck 16.4 → 2.9 ms (binding groups 9.75 → 0.44 ms),
  a five-check project 76.8 → 28.2 ms, verified by a 559-file differential
  `badger check` sweep with no output differences. Watch/REPL and `badger test` reuse one
  session across a rebuild loop, so the cache applies there too: a watch
  reload's front end 38.8 → 5.9 ms, a five-item REPL session 279.6 → 91.2 ms,
  a five-selection `badger test` 208.1 → 70.4 ms
  ([2026-10-19-session-reuse.md](reports/2026-10-19-session-reuse.md)).
  Remains: step 8 of the plan (`TypeEnv::build`'s std prefix, ≈0.9 ms/check),
  a path-keyed source reuse (a long watch/REPL session appends a `FileId` per
  re-read file), and the LSP's per-query session (`analysis.rs:219`).
- C2: re-measured 2026-10-19 (`BADGER_TIMINGS=1/2`): the standard library's
  binding groups take **10.5–12.5 ms** of a check (the ~39 ms claim is
  stale) — now only on a session's *first* check, since C1.4 caches them
  (0.44 ms on a hit) — and the effect fixpoint 0.53 ms. Solver retry counters
  and scaling fixtures remain.
- C3: cross-build instance and code reuse for watch/REPL, and code
  reclamation instead of leaking replaced builds (`mem::forget(built)` at
  `live.rs:144,153` and `inspect.rs:208`). Reclaiming needs a liveness
  argument first: the replaced build's compiled code may still be running.
  The front-end half is done: watch, `badger test` and the REPL reuse one
  session, so a rebuild does not re-parse or re-check the standard library.
- C4: a root-store/frame counter to prove the landed root reductions
  (no counter exists yet; codegen owner).
- R1 (memory-management owner): keep the mark stack and remembered buffer
  across collections; no `occupied` sort in `trim_empty_chunks`; account
  channel queues and staging as external memory; report managed,
  external, pressure, and RSS separately. Done 2026-10-19
  (`perf/optimization-backlog-2026-10-19`): the mark stack and the
  remembered buffer are reused across collections (`e5967d3`, neutral in
  the measurement), `trim_empty_chunks` indexes the chunk ranges instead
  of sorting every cell address (`5bae53c`: a full collection over a
  200 000-object heap goes from 190 ms to 65 ms), and the channel's native
  buffers and the admission gates' waiter queues are charged to the
  external counters, with resident memory reported next to managed,
  external and pressure (`274743a`). The work also found a collector bug
  that the R1 items had to work around: a dead list directory's weak slot
  named a page freed in an earlier minor collection, and the clearing pass
  read the freed cell (`66a5222`, reproduced on `main`).
- R2: done 2026-10-19 (`perf/runtime-stdlib-2026-10-19`): `chunks`,
  `to_array`, `bytes`/`buffer` conversions and `concat`/`slice` copy whole
  packed runs; on the measured workloads the wall-clock is dominated by
  allocation, so the per-element savings are not separately visible.
- R3: done 2026-10-19 (`perf/channel-sizing-2026-10-19`): `admission_size`
  walks maps and sets in place (`map::for_each`) instead of collecting entries
  into a `Vec` per admission and per `uncharge` — 104 → 70 ns/op on a
  byte-budgeted channel carrying an 8-entry map (−42% on the charge; the
  unbudgeted path is untouched). Storing sizes with batch and staged items and
  indexed leases remain, and overlap R1's channel accounting.
- R4: a typed per-intrinsic contract table in place of the `UNLOCKED` name
  list (~130 strings at `codegen/lib.rs:1646`). Its only cost is
  compile-time and only for `workers > 1` (the lookup is short-circuited),
  so this is maintainability, not throughput; codegen owner.
- R5: done 2026-10-19 (`perf/codec-data-walk-2026-10-19`): `from_data` and
  `from_sql_row` walk the `Data` representation directly (`dec_data`, plus
  `dec_str_key` for object keys, which are strings) instead of building a
  `serde_json::Value` per decode — `from_data` 686 → 457 ns/op (−33%), a
  64-row round trip −13%, `from_sql_row` −12%. `dec` stays for `params_decode`,
  which still decodes from path-string JSON; `data_value` is gone. Text
  decoding already streamed; routing `from_data` through it was measured at
  +25% and remains rejected.

## Language, libraries, and tools (ENHANCEMENTS.md)

- T3: more code actions: protocol method stubs, hash preconditions on
  edits (only E0127/E0342 quick fixes exist today).
- T4: holes could list the locals in scope that have the hole's type
  (E0345 reports the expected type only).
- T6: lints (immutable appends in loops, unbounded `stream.collect`); no
  lint subsystem exists yet.
- T7: endpoint- and interceptor-only generators, and
  semantic wiring of generated modules into a composition root (`feature`
  and `worker` generators exist; no composition-root wiring).
- T9: run the evaluation (eval/) with agents, and grow the task set. The
  harness runs its three reference tasks 3/3 in 1.8 s; no agent run is
  recorded.
- S5: capacity-aware builders and bulk operations. Done 2026-10-19
  (`perf/runtime-stdlib-2026-10-19`): `list::fill` bulk-fills a list at its
  final size, `ListBuilder.extend` copies a whole list, and `range`/`flat_map`
  /`text.chars` use them — range 2.3 → 1.1 ns/op (−54%), `flat_map` −84%,
  `text.chars` −80%. Builder capacity hints were implemented, measured as
  *not* faster (directory pre-sizing is <3% of builder cost), and dropped;
  `buffer.with_capacity` already beats growth. Remaining: `bytes.to_list`,
  `text.lines`/`split`/`words` (`strings_to_list`) still push per element.
