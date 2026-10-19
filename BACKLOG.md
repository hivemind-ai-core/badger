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
  thing in this section right now.
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
  being implemented. Separate small gap: `badger test` builds a fresh
  `Session` per `[entries]`/`[tasks]` selection (`badger_cli/src/main.rs:637`),
  so it gets no parse cache at all; reusing one session needs a diagnostics
  reset that keeps `std_parsed` (diagnostics accumulate and short-circuit
  `check_one` at `driver/src/lib.rs:746`).
- C1.3/C1.4: a reusable compiler context with a content-keyed parse cache
  (remapping spans, file ids, and symbols), then checked-library reuse.
  Prerequisite as above; the parse half is the landed C1.2b work.
- C2: re-measured 2026-10-19 (`BADGER_TIMINGS=1/2`): the standard library's
  binding groups take **10.5–12.5 ms** of a check (the ~39 ms claim is
  stale) and the effect fixpoint 0.53 ms. Solver retry counters and scaling
  fixtures remain.
- C3: cross-build instance and code reuse for watch/REPL, and code
  reclamation instead of leaking replaced builds (`mem::forget(built)` at
  `live.rs:144,153` and `inspect.rs:208`). Reclaiming needs a liveness
  argument first: the replaced build's compiled code may still be running.
- C4: a root-store/frame counter to prove the landed root reductions
  (no counter exists yet; codegen owner).
- R1 (memory-management owner): keep the mark stack and remembered buffer
  across collections; no `occupied` sort in `trim_empty_chunks`; account
  channel queues and staging as external memory; report managed,
  external, pressure, and RSS separately.
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
