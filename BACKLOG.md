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
  control run.
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
  remains; it needs a *shared standard-library HIR* first, because each
  entry lowers its own `Hir` with its own `DefId`s, and
  `badger_typeck::check_program` builds a fresh global context plus schemes
  per program.
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
- R3: admission sizes without materializing map entries (a map-entry `Vec`
  is built per admission at `channel.rs:645-658`, and `uncharge` re-sizes at
  `:936-944`); sizes stored with batch, staged, and leased items; indexed
  leases. Overlaps R1's channel accounting.
- R4: a typed per-intrinsic contract table in place of the `UNLOCKED` name
  list (~130 strings at `codegen/lib.rs:1646`). Its only cost is
  compile-time and only for `workers > 1` (the lookup is short-circuited),
  so this is maintainability, not throughput; codegen owner.
- R5: decode `Data` without the intermediate JSON value. Text decoding is
  done 2026-10-19 (`badger_runtime::json` streams into the target value);
  `from_data`/`from_sql_row` still build a `serde_json::Value` (`codec::dec`).
  Routing them through the text decoder was measured at +25% (850 → 1065
  ns/op on a 64-row round-trip probe). A direct walk of the `Data`
  representation is what remains; note it needs a second small path for map
  keys (a `Data` object's keys are plain strings, not `Data`), i.e. it is a
  third decoder rather than a deletion, until `params_decode`,
  `codec_to_data` and `data_parse` can all stop building values.

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
