# Backlog

What remains of the archived plans ([archive/plans](archive/plans)), each
item with where it came from. Everything else in those plans is
implemented; their status headers say what and where. Framework leftovers
are in [FRAMEWORK_DESIGN.md](FRAMEWORK_DESIGN.md) §25.

## Runtime synchronization (LOCK_OPTIMIZATION.md)

Measurements after the landed slices attribute most remaining multi-worker
cost to moving channel state between cores rather than to lock waits
([2026-10-16 channel round report](reports/archive/benchmarks/2026-10-16-lock-round1-report.md)),
so these are correctness-and-scale work to schedule against new profiles.

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

- C1.2b: parse once per session across a project's `[entries]` and
  `[tasks]` (each entry re-parses and re-checks today).
- C1.3/C1.4: a reusable compiler context with a content-keyed parse cache
  (remapping spans, file ids, and symbols), then checked-library reuse.
- C2: an effect worklist (or SCC summaries) in `effect_fixpoint`; solver
  retry counters; scaling fixtures.
- C3: cross-build instance and code reuse for watch/REPL, and code
  reclamation instead of leaking replaced builds.
- C4: a root-store/frame counter to prove the landed root reductions.
- R1 (memory-management owner): keep the mark stack and remembered buffer
  across collections; no `occupied` sort in `trim_empty_chunks`; account
  channel queues and staging as external memory; report managed,
  external, pressure, and RSS separately.
- R2 (memory-management owner): bulk copies for scalar `list.concat`,
  `slice`, and `chunks` (`chunks` copying costs ~12x its traversal in
  `bench/collections`).
- R3: admission sizes without materializing map entries; sizes stored
  with batch, staged, and leased items; indexed leases.
- R4: a typed per-intrinsic contract table in place of the `UNLOCKED`
  name list.
- R5: decode `Data` without the intermediate JSON value (the text step
  is gone; a descriptor-guided decoder would skip the value tree too).

## Language, libraries, and tools (ENHANCEMENTS.md)

- T3: more code actions: protocol method stubs, hash preconditions on
  edits.
- T4: expression holes for partial programs.
- T6: lints (immutable appends in loops, unbounded `stream.collect`).
- T7: `badger new`, endpoint- and interceptor-only generators, and
  semantic wiring of generated modules into a composition root.
- T9: run the evaluation (eval/) with agents, and grow the task set.
- S5: capacity-aware builders and bulk operations.
