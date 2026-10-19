# Optimisation backlog

> Staged work derived from the research notes in `research/09..13` on
> 2026-10-19, against `main` at `f46d9bb`. This is the
> *research-driven* plan, complementary to `BACKLOG.md` (the in-flight
> plan for the codebase before the research). The two backlogs share
> the same measured state (BACKLOG's re-measured premises) and the same
> benchmark gates; this file is the long-form version of "what comes
> next, ordered".

## How to read this

Each phase is a **set of work items** that share a dependency. Within
a phase, items are ordered by **bang for buck** (impact × inverse cost).
Each item names:

- the **goal** (what changes for the user / the runtime / the compiler);
- the **prior art** (which research-note section motivates it, with a
  link to the canonical paper or library);
- the **code touchpoints** (`file_path:line_number` form, against the
  snapshot at `f46d9bb`);
- the **rough cost** (LOC band; the budget is the size of the
  existing file, not a precise estimate);
- the **expected win** (what benchmark moves, and by roughly how much);
- the **gates** it must pass before merge (per the BACKLOG gates and
  the new gates added in §Gates below);
- the **not-adopted** items are listed in §"Not adopted" at the end
  with the *reason* from the research.

The four measured cost anchors used throughout (per BACKLOG, 2026-10-19):

- `bench/messages.bdg` 78 ms at 4 workers / 36.6 ms at 1 (BACKLOG
  re-measured 2026-10-19). The 4-worker number is what the runtime
  phases target.
- GC accounts for 12–38 % of elapsed time across the four profiles
  (`reports/2026-10-17-gc-profiling.md`). Of that, 61–89 % is sweep,
  7–27 % is trace, 0.07–0.16 % is the stop-world rendezvous. The
  sweep and trace numbers are what the GC phases target.
- The MAP (multi-worker) regression in the previous runtime
  concurrency review: independent channel pairs 4.48× slower at 4
  workers than at 1 (`reports/2026-10-15-runtime-concurrency-review.md`).
- The M3 (medium-collection) overhead on the I3 / I5 workloads (per
  the BACKLOG "C2" re-measure).

## Phase 0 — Cheap wins (no language change, no new crates, no typeck surface)

Every item in Phase 0 is a 50–500 LOC change with independent
measurement. The work is parallelisable across two engineers; the
benchmarks are shared. No item depends on another; pick by benchmark
match.

### 0.1 HotSpot-style polling page for the GC handshake

**Status:** **not adopted as written** (2026-10-19). The race it removes is gone: the
handshake is `STOP` plus a per-worker `IN_COMPILED` counter under
sequentially consistent ordering, and the worker increments the counter
before it reads `STOP` while the collector stores `STOP` before it reads the
counters, so there is no window. The mechanism also only pays off when the
*generated code* polls at safepoints; with the poll in the runtime the
collector must still wait for the counter to drain, so the fault would
replace the load, not the wait. The prerequisite is safepoint polls in the
generated code, with Phase 2's QSBR work; see
[the Phase 0 report](reports/2026-10-19-optimization-backlog-phase-0.md).

- **Goal**: replace the current `STOP: AtomicU8` poll-check in the GC
  handshake with a HotSpot-style "mprotect a per-worker page" trap
  (BACKLOG sync slice 0, audit G3 race).
- **Prior art**: HotSpot's "safepoint polling page" (Dice 2001;
  HotSpot `safepoint.cpp`; cited in `research/12-runtime-scaling-extra.md`
  §16.3, sketch in §A.2).
- **Code touchpoints**: new
  `crates/badger_runtime/src/safepoint.rs` (~50 LOC); replace the
  `STOP` check in `task.rs:1503-1506` and the GC handshake in
  `gc.rs:638-660`.
- **Cost**: ~50 LOC new + ~10 LOC edited.
- **Expected win**: removes the G3 race (`reports/2026-10-17-gc-profiling.md`
  G3); per-collection stop-world latency drops from "synchronous poll
  spin" to "mprotect + page fault" (~1 µs per worker). On 4 workers
  the expected saving is ~4 µs per collection; on 64 workers ~64 µs.
- **Gates**: `BADGER_GC_VERIFY=1`; `tests/run/select_and_budget` still
  passes; `bench/messages.bdg` 4-worker median unchanged within noise
  (this is a latency-tail fix, not a throughput fix).

### 0.2 Per-worker Chase-Lev deque (replace `LocalQueue`)

**Status:** landed 2026-10-19 (`beeb63a`): a `crossbeam_deque::Worker` + `Stealer`
pair per worker, with the next slot as a second pair. The corpus and
the new `crates/badger_runtime/tests/scheduler.rs` pass; `messages` at 4
workers is 6% faster (wall clock, interleaved medians), `parallel_alloc` is
at parity. The backlog's predicted wins (the parked time halved, a 2-5x cut
in steal traffic) are not visible in those benchmarks: the programs barely
touch the scheduler (`messages` parks 0.01 ms, `parallel_alloc` runs 7-8
tasks), so the per-worker queue is a tail-latency fix the throughput
benchmarks cannot show.

- **Goal**: replace the per-worker `LocalQueue`
  (`task.rs:1596-1664`, gated by `lock: Mutex<()>`) with a
  `crossbeam-deque::Worker<T>` + `Stealer<T>` pair, so the owner is
  wait-free and the thief is lock-free.
- **Prior art**: Chase & Lev, "Dynamic Circular Work-Stealing Deque",
  SPAA 2005; the canonical Rust implementation is the `crossbeam-deque`
  crate (`research/12-runtime-scaling-extra.md` §13.1, §13.2; the
  `crossbeam-deque` source is the implementation reference).
- **Code touchpoints**: `task.rs:1596-1664` (`LocalQueue` and its
  `with` method); `task.rs:1797-1813` (`take_ready` / `steal`).
- **Cost**: ~120 LOC (replace the per-worker queue type, keep the
  `WorkerId` / `Stealer` discovery in the global `Workers`).
- **Expected win**: removes the per-steal `lock: Mutex<()>` cache-line
  ping-pong; expected ~5–10 ns per steal on Skylake (per
  `crossbeam-deque`'s own benches). On `bench/messages.bdg` 4-worker
  the 0.96 ms parked time is expected to halve.
- **Gates**: `tests/run/parallel` still passes; 4-worker median
  improves; p99 unchanged within noise (this is a tail-latency /
  cross-core traffic fix).

### 0.3 AHash hasher (replace the map hasher)

**Status:** **not adopted** (2026-10-19): implemented (`c502f14`) and reverted
(`d3f0b60`) -- it fails the item's gate. Measured on release builds with
Int keys, AHash makes `persistent map insert` 120 -> 221 ns/op and `map
lookup` 108 -> 235 ns/op, and 1.6-2x slower on stride-1024 and
pseudo-random keys (String keys are neutral). Hashing an Int costs the same
(1.4 ns vs 1.1 ns): the cost is the trie shape. FxHash's multiply-and-rotate
acts like Fibonacci hashing on arithmetic key sequences and spreads them
near-perfectly over the 5-bit chunks, so the tries stay shallow, while AHash
spreads keys at random (Poisson) and the tries get deeper. The item's
premise, that FxHash's chunks collide more than AHash's, does not hold for
these workloads.

- **Goal**: replace the current hasher in `map.rs:39` (FNV / default
  Rust hasher) with `AHash` (Lemire, 2019+). AHash produces enough
  entropy to avoid 5-bit hash-chunk collisions in the HAMT
  (`hamt.rs:29`).
- **Prior art**: AHash (https://github.com/tkaitchuck/aHash); the
  comparison in `research/10-persistent-data-structures.md` §5
  (Murmur3, xxHash, AHash, SipHash).
- **Code touchpoints**: `crates/badger_runtime/src/map.rs:39` (the
  `BuildHasher` type); `crates/badger_runtime/src/hamt.rs:52-56` (the
  hash-split code, the 64-bit-to-5-bit-chunks).
- **Cost**: ~5 LOC (change the type alias; ensure the hasher is
  deterministic per language spec, not per process).
- **Expected win**: better distribution of hash bits in 5-bit chunks;
  fewer collision buckets in the HAMT. Expected ~5–15 % on
  `bench/parallel_alloc` and `bench/records`.
- **Gates**: all `bench/collections` and `bench/maps` workloads within
  noise on existing benchmarks; no new test required (AHash has the
  same `BuildHasher` API surface).

### 0.4 Small-map flat-array fallback (n ≤ 8)

**Status:** landed 2026-10-19 (`2854c83`, `21303fa`): a root of at most eight entries
is a single node with both bitmaps zero, scanned linearly, promoted to the
trie at the ninth entry and flattened again on a removal that brings it back
to eight. The flat root is an ordinary `MapNode`, so tracing, the cursor
layout and the map payload are unchanged, and the flat entries keep the order
the trie would visit, so iteration order does not depend on the
representation.

- **Goal**: when a map's payload is ≤ 8 entries, store it as a flat
  array of `(K, V)` pairs and linear-scan on lookup, instead of the
  HAMT trie. Once the 9th entry is added, copy into the HAMT.
- **Prior art**: Clojure's `PersistentArrayMap` (≤ 8 entries), the
  same fallback in Scala's `immutable.HashMap` and Haskell's
  `unordered-containers`. Covered in
  `research/10-persistent-data-structures.md` §1.4 (Steindorfer's
  thesis) and the ranked recommendation A.
- **Code touchpoints**: `crates/badger_runtime/src/map.rs` (a new
  `MapRepr::{Flat, Hamt}` enum); the `MapRepr::from_list` /
  `MapRepr::assoc` / `MapRepr::dissoc` paths; the hash-collision
  threshold.
- **Cost**: ~200 LOC (new variant; `assoc` / `dissoc` / `get` /
  `iter` / `len` / `eq` / `hashcode` methods for the flat repr;
  the `assoc`-promotes-to-HAMT path).
- **Expected win**: ~3–5× on small maps; this is the single biggest
  collection win per `research/10-persistent-data-structures.md` A.
- **Gates**: `BADGER_GC_VERIFY=1`; the `bench/collections` map-dominant
  cells improve; the `bench/records` cell improves; existing
  `tests/run/map_*` and `tests/run/persistent_*` pass.

### 0.5 CHAMP invariants (sorted entries, disjoint bitmaps)

**Status:** partly landed 2026-10-19 (`dee35dd`): `map::eq` is now a paired walk of
the two tries that skips a subtree the two maps share, with a reflexivity
check (a float NaN is not equal to itself). The O(1) `map::hashcode` of the
item is **not** done: it would need a per-map hash cached in the payload,
which collides with the 16/24-byte payload layout, and hashing every value
on the insert path. The model is the contract for the rest.

- **Goal**: the existing HAMT in `hamt.rs` is a "Bin" (Bitmap-
  Indexed Node) variant; the formal CHAMP invariants from
  Steindorfer 2017 are "no duplicate entries", "entries are sorted
  by hash chunk", "datamap and nodemap are disjoint". These give
  O(1) `eq` / `hashcode` (no per-element comparison walk) and O(1)
  `iter` (sorted children).
- **Prior art**: Steindorfer, "Optimizing Hash-Array Mapped Tries
  for Fast and Compact Immutable Sets", ECOOP 2017;
  `research/10-persistent-data-structures.md` §1.3, recommendation B.
- **Code touchpoints**: `crates/badger_runtime/src/hamt.rs:81-94`
  (the two bitmaps); `hamt.rs:185-203` (`alloc_node`); the
  `splice_owned` path at `hamt.rs:240-287`.
- **Cost**: ~50 LOC + tests.
- **Expected win**: free `Map::eq` / `Map::hashcode` (currently
  walks the whole tree); free O(1) `iter` start; ~10–20 % on
  equality-heavy workloads.
- **Gates**: `bench/collections` equality cells within noise or
  better; the `tests/run/map_eq` test passes (it currently has a
  flag for "structured equality").

### 0.6 Keep mark stack and remembered buffer across collections (R1)

**Status:** landed 2026-10-19 (`e5967d3`): the buffers are handed out under a flag
and cleared before use. Neutral in the measurement (two allocations per
collection, below the noise); kept because it is strictly less work.

- **Goal**: the mark stack (`gc.rs:690` `let mut stack: Vec<u64>`)
  and the remembered buffer (`gc.rs:700-701`) are allocated fresh
  on every collection. Reuse the allocation across collections,
  truncate before use.
- **Prior art**: BACKLOG R1 (re-measured 2026-10-19);
  `research/11-gc-and-memory-management.md` §1, §2.
- **Code touchpoints**: `crates/badger_runtime/src/gc.rs:690`,
  `:700-701`; the `collect_inner` signature.
- **Cost**: ~10 LOC.
- **Expected win**: removes a small but constant allocation per
  collection; expected ~2–5 % on the `fanout` profile (which does
  15–30 reclaim per event, per the GC profile report).
- **Gates**: `BADGER_GC_VERIFY=1`; the diagnostic "objects_before"
  snapshot unchanged; existing tests pass.

### 0.7 Drop the `occupied` sort in `trim_empty_chunks` (R1)

**Status:** landed 2026-10-19 (`5bae53c`): the chunk ranges are sorted once and each
cell is looked up, instead of sorting every cell address. A full collection
over a 200 000-object heap goes from 190 ms to 65 ms (-65%, 2.9x); see the
[Phase 0 report](reports/2026-10-19-optimization-backlog-phase-0.md).

- **Goal**: `trim_empty_chunks` (`gc.rs:966-979`) sorts the
  `occupied` array; this is O(n log n) on a list of length n at
  every collection. The sort is not necessary — the only thing
  `occupied` is used for is the per-page free list, which is
  unordered.
- **Prior art**: BACKLOG R1;
  `research/11-gc-and-memory-management.md` §1.
- **Code touchpoints**: `crates/badger_runtime/src/gc.rs:966-979`.
- **Cost**: ~3 LOC (delete the sort; remove the import).
- **Expected win**: removes a measurable fraction of the sweep
  cost on large heaps; the GC profile report says sweep is 61–89 %
  of GC time, so even a 1 % saving is visible.
- **Gates**: `BADGER_GC_VERIFY=1`; all GC tests pass; the
  `BADGER_GC_PROFILE` "trim_empty_chunks" phase counter shortens.

### 0.8 External memory accounting (R1)

**Status:** landed 2026-10-19 (`274743a`): the channel's queues, sizes, batches,
staging, leases and page pool, and the admission gates' waiter queues, are
charged to the external counters as they grow and released as they are
freed, moved or dropped; resident memory is reported next to managed,
external and pressure.

- **Goal**: channel queues (`channel.rs:38` `queue: VecDeque<u64>`),
  staging (`channel.rs:46` `staging: FxHashMap<TaskId, Vec<u64>>`),
  leases (`channel.rs:55` `leases: Vec<(TaskId, u64)>`), admission
  gates (`admission.rs:18-31`), and any `Bytes` / `Buffer` payload
  (the `items` storage at `allocations.rs:65-104`) are reported
  as "external" memory, separately from the managed heap.
- **Prior art**: BACKLOG R1; Immix-style external accounting
  (`research/11-gc-and-memory-management.md` §1, §6.4).
- **Code touchpoints**: the `EXTERNAL_LIVE` / `EXTERNAL_PRESSURE`
  atomics in `gc.rs`; the `BufferStorageEvent` accounting at
  `gc/allocations.rs:65-104`; the `external_alloc` / `external_free`
  API at `gc.rs:416-425`.
- **Cost**: ~80 LOC (the `BufferStorageEvent` exists; extend it to
  the channel queues + admission gates + leases).
- **Expected win**: makes the GC pressure threshold correct; with
  correct external accounting, the collection trigger fires at the
  right time, which is the precondition for the Immix-style block
  reclamation in §2.3.
- **Gates**: `BADGER_GC_VERIFY=1`; the
  `BADGER_GC_PROFILE=external_before` / `external_after` snapshots
  are non-zero on every workload.

### 0.9 Two-random-choice victim selection

**Status:** landed 2026-10-19 (`5182315`): two random victims are picked and the
better one is tried first, the other second.

- **Goal**: `take_ready` (`task.rs:1797-1813`) currently picks one
  random victim and steals one task. Two-random-choice (the
  "power of two random choices" line) is well-known to balance
  better.
- **Prior art**: Mitzenmacher, "The Power of Two Choices in
  Randomized Load Balancing", 2001 (the original);
  `research/12-runtime-scaling-extra.md` §8, §13.9.
- **Code touchpoints**: `task.rs:1797-1813`.
- **Cost**: ~10 LOC.
- **Expected win**: a few percent on the imbalanced-cell benchmark
  (the 4-worker independent-channels measurement).
- **Gates**: `tests/run/parallel` passes; the 4-worker
  `bench/messages.bdg` median improves or holds.

### 0.10 Half-steal on overflow

**Status:** landed 2026-10-19 (`c6428ee`): a thief takes at most about half of what
it observed, never the victim's next slot, never the whole queue; the rule
and its one-task slack under a concurrent owner pop are stated in the code
and in the commit message. (work-stealing `Steal::steal_batch`)

- **Goal**: badger's per-worker steal currently pops one task
  (`pop(false)` at `task.rs:1799`); Go's `runtime.runqsteal` pops
  half the runqueue on overflow. The Tokio LIFO_BATCH pattern
  (`research/12-runtime-scaling.md` §1, Tokio cite) is the same
  idea for a different data structure.
- **Prior art**: Go's `runtime.runqsteal` (`research/12-runtime-scaling.md`
  §1); `crossbeam-deque::Stealer::steal_batch` (the Rust
  implementation).
- **Code touchpoints**: `task.rs:1797-1813` (the steal loop);
  `task.rs:1553-1574` (the global `SCHED_Q`).
- **Cost**: ~30 LOC (only meaningful after 0.2 lands).
- **Expected win**: on bursty workloads (the `backpressure`
  profile), a 2–5× reduction in cross-core steal traffic.
- **Gates**: `tests/run/parallel`; 4-worker `bench/parallel_alloc`
  median improves.

## Phase 1 — State-machine for selection (correctness gate)

Phase 1 is the **correctness gate** for the Phase 2 and Phase 3
changes. The BACKLOG Slice 4 lead bug
(`reports/2026-10-19-select-fast-path-fragility.md`) is a missing
invariant in a multi-lock edge-state machine, and Phase 2's lock-free
deques can only be adopted safely once the state machine is in
TLA+ form. The work is one engineer's; the gate is the existing
`tests/run/select_and_budget` test plus the new TLA+ model checker.

### 1.1 TLA+ model for the per-channel state machine (BACKLOG slice 4)

- **Goal**: write a TLA+ / PlusCal spec for the per-channel state
  machine; the spec is the contract that the channel refactor in
  1.2 must satisfy.
- **Prior art**: Lamport, *Specifying Systems* (2002);
  `research/12-runtime-scaling-extra.md` §15.2, §A.3.
- **Code touchpoints**: `spec/tla/ChannelBadger.tla` (new, ~150
  lines); the TLC config (`spec/tla/ChannelBadger.cfg`).
- **Cost**: ~150 LOC (PlusCal + config).
- **Expected win**: a property-based test that catches the slice 4
  lead bug class without instrumenting the production code (the
  instrumented code is the fragile path that broke the audit's
  measurement).
- **Gates**: TLC reports no counterexample for the safety property
  ("every state transition moves `chan_state` by at most one step")
  and the liveness property ("every blocked recv is eventually
  woken") for the bounded 3-task / 1-channel model.

### 1.2 Go-style `runtime.chan.go` refactor (BACKLOG slice 4)

**Status:** landed 2026-10-19 (`b6e0e2a`, `69b35c1`): one atomic word per
channel holds the owner, the phase (`EMPTY`, `VALUE`, `BLOCKED`) and the
closed bit; one waiter list replaces the five parallel ones; each
operation linearizes at the compare-and-swap that makes it the channel's
owner (`try_fast`, `claim`), and the releasing store publishes the phase.
It meets the 1.1 model's contract: a send commits to a parked selection
at every capacity, a selection's registration is removed only by the
selection, a publication wakes a receiver per message (F2), and a wait a
waker completed is not undone by a later cancellation (F1). The audit's
fast-path counters are kept behind `BADGER_RUNTIME_STATS=1`;
`select_and_budget` passes 6/6 with them on and off, though the audit's
deadlock did not reproduce on the unmodified code either (it was
timing-sensitive; see the report's resolution). `bench/messages.bdg`
(medians, machine shared with other jobs): prompt throughput 25 → 26
ns/message at 1 worker and 30 → 30 at 4; round-trip p99 351 → 280 ns at
1 worker (25 runs) and 622 → 461 ns at 4 (9 runs), p50 within 10 ns;
`bench/independent_channels.bdg` at 4 workers 68 → 66 ms. A send/receive
pair on the fast paths costs 12.0 → 13.7 ns (the phase in the word); a
receive on an empty channel 3.3 → 2.5 ns (it declines on the published
phase without a compare-and-swap).

- **Goal**: refactor `channel.rs:271-510` (the `try_fast` /
  `send_fast` / `append_fast` / `recv_one_fast` / `recv_fast`
  paths) to a single atomic `chan_state: {Empty, WithValue,
  Blocked}` with a single `waiters: Vec<TaskId>`, in the style of
  Go's `runtime.chan.go`.
- **Prior art**: Go's `runtime.chan.go` (Griesemer et al.);
  `research/12-runtime-scaling.md` §4;
  `research/12-runtime-scaling-extra.md` §15.5, §A.3.
- **Code touchpoints**: `crates/badger_runtime/src/channel.rs:71-72`
  (the parallel `recv_watchers` and `space_waiters` lists, to be
  merged); `channel.rs:271-510` (the fast paths); the per-channel
  `state: AtomicU8` at `channel.rs:22`.
- **Cost**: ~400 LOC refactor + the linearization-point comment for
  every operation.
- **Expected win**: the slice 4 lead bug is fixed by construction
  (the state machine is the contract); the ~1 ns/call addition
  measured in the audit is the cost of the additional CAS and is
  the right cost.
- **Gates**: `tests/run/select_and_budget` (the audit's
  deterministic-deadlock test) passes with the new fast-path
  counters re-enabled; TLC reports no counterexample for the new
  implementation; the p99 latency on `bench/messages.bdg` is
  unchanged or improved.

### 1.3 TLA+ model for the GC handshake (audit G3)

- **Goal**: a TLA+ / PlusCal model for the `gc.rs:638-660`
  stop-the-world handshake that uses 0.1's polling-page trap.
- **Prior art**: `research/12-runtime-scaling-extra.md` §15.2.
- **Code touchpoints**: `spec/tla/GcHandshake.tla` (new, ~100
  lines).
- **Cost**: ~100 LOC.
- **Expected win**: the TLA+ model is the contract for 0.1 and the
  2.1 QSBR adoption.
- **Gates**: TLC reports no counterexample for the
  "every worker observes the new epoch" invariant.

## Phase 2 — Per-worker heap segments and lock-free scheduling (depends on Phase 1)

Phase 2 work is structural. The per-worker deques from 0.2 are
already in; Phase 2 makes the GC and the runtime symmetric to them.

### 2.1 QSBR (userspace-RCU) for the per-worker grace period

- **Goal**: replace the current `STOP: AtomicU8` handshake
  (audit G3) with a userspace-RCU-style QSBR (Quiescent-State-Based
  Reclamation) per-worker epoch + global grace period.
- **Prior art**: Desnoyers et al., "User-Level Implementations of
  Read-Copy Update" (ACM TOPC 2012); the Rust `crossbeam-epoch`
  crate; the `urcu-qsbr` library; `research/12-runtime-scaling-extra.md`
  §13.4, §13.5, §13.7, §A.1.
- **Code touchpoints**: new `crates/badger_runtime/src/gc_handshake.rs`
  (~80 LOC); the `task.rs:1485-1507` poll-cycle entry; the
  `gc.rs:collect()` entry.
- **Cost**: ~80 LOC new + ~20 LOC edited.
- **Expected win**: removes the audit G3 race by construction; the
  4-worker stop-world latency drops by ~5–10×.
- **Gates**: `BADGER_GC_VERIFY=1`; the 1.3 TLA+ model holds; the
  existing `tests/run/gc_*` and `tests/run/select_and_budget` pass.

### 2.2 Per-worker heap segments (R1, design phase)

- **Goal**: the runtime's per-worker object lists
  (`gc.rs::objects(w)`) already exist; this work moves the *bump
  pool* to per-worker and the *free list* to per-worker, so that
  the per-worker deques (0.2) and the per-worker heap are
  symmetric.
- **Prior art**: `research/11-gc-and-memory-management.md` §6
  (the arena allocator line: bmalloc, PartitionAlloc, jemalloc,
  mimalloc, snmalloc); the per-thread arena design in jemalloc.
- **Code touchpoints**: `crates/badger_runtime/src/gc.rs:241-280`
  (the `alloc_cell` / `free_cell` paths); the per-worker pool at
  `gc.rs:269`.
- **Cost**: ~250 LOC refactor.
- **Expected win**: ~5–15 % on the multi-worker allocation-heavy
  workloads (`parallel_alloc`); the per-worker free list eliminates
  cross-core free-list traffic.
- **Gates**: the diagnostic `PoolEvent::{Refill, Trim}` accounting
  at `gc/allocations.rs:38-61` is preserved; existing GC tests pass.

### 2.3 Immix-style block reclamation (sub-strategy of the bump pool)

- **Goal**: keep the existing bump pool as the primary allocator;
  add an Immix-style line-bitmap "is this 32-byte line live?"
  check on free, so partially-used blocks can be reused in place
  rather than kept in a free list.
- **Prior art**: Yang, Blackburn, Frampton, Hosking, Moss, "Immix"
  (OOPSLA 2012); the "Sticky Immix" line; the follow-up work
  documented in `research/11-gc-and-memory-management.md` §4.
- **Code touchpoints**: `crates/badger_runtime/src/gc.rs:241-280`
  (the `free_cell` path); the per-block header.
- **Cost**: ~300 LOC (block header, line-bitmap, free path).
- **Expected win**: ~10–20 % reduction in allocation cost on
  workloads with mixed lifetimes (the `balanced` and `fanout`
  profiles); the line-bitmap lets short-lived and long-lived
  allocations share a block.
- **Gates**: `BADGER_GC_VERIFY=1`; the
  `BADGER_GC_PROFILE` `objects_before` / `objects_after` snapshots
  are unchanged.

### 2.4 NUMA worker-core pinning

- **Goal**: the per-worker deques (0.2) and the per-worker heap
  segments (2.2) are the prerequisite for NUMA pinning; once
  they land, pin each worker to a core via
  `pthread_setaffinity_np`.
- **Prior art**: Lozi et al., "Fast and Convenient Online Task
  Placement" (SOSP 2019); Karandikar et al., "Carrefour" (OSDI
  2023); the Linux EEVDF line (`research/12-runtime-scaling-extra.md`
  §14.5, §14.6, §14.7).
- **Code touchpoints**: `task.rs:2925-2947` (`run_main_multi`).
- **Cost**: ~30 LOC.
- **Expected win**: the literature is clear that this only pays
  off with per-worker heap segments (2.2). Expected ~5–10 % on
  2-socket hosts on the multi-worker benchmarks.
- **Gates**: per-CPU cache miss rate is reduced in the diagnostic
  CPU sampling; 4-worker `bench/messages.bdg` improves on a
  2-socket host.

## Phase 3 — Type-system memory management (language-side, no language change for §3.1–§3.3)

Phase 3 is the research-driven version of the BACKLOG "C3 / R1"
typeck work. It builds on the existing `ownership.rs` and
`regions.rs`; no user-facing surface changes for §3.1–§3.3.

### 3.1 Region / arena for typeck and AST

- **Goal**: add a `bumpalo` / `typed-arena`-style arena for
  short-lived AST nodes during typeck; an `Immix`-style or
  `mimalloc`-style sub-allocator for the GC's bump pool refill
  path.
- **Prior art**: ML Kit (region + GC hybrid;
  `research/13-type-system-memory-management.md` §6.2); jemalloc /
  mimalloc / snmalloc (`research/13-type-system-memory-management.md`
  §6.9, §6.10); bumpalo / typed-arena (§6.5); Cone's `region T`
  (§4.6).
- **Code touchpoints**: new
  `crates/badger_runtime/src/arena.rs` (the runtime primitive,
  ~150 LOC); `crates/badger_typeck/src/ownership.rs` (per-binding
  mode); `crates/badger_runtime/src/gc.rs:241-266` (the per-worker
  pool, swap in the sub-allocator).
- **Cost**: ~300 LOC.
- **Expected win**: ~10–20 % on `badger check` wall-time (the
  typeck half of `BACKLOG.md` C1.4); the per-worker arena
  eliminates the malloc traffic for typeck nodes.
- **Gates**: `badger check` is bit-for-bit identical on a
  559-file sweep; the per-binding mode diagnostics are unchanged.

### 3.2 MIR escape analysis (extend `sra.rs`)

- **Goal**: the existing `sra.rs` does partial scalar replacement;
  this extends it to a Whaley-Rinard-style compositional escape
  analysis, so records that don't escape the function are
  stack-allocated.
- **Prior art**: Choi 1999 (§5.1); Whaley-Rinard 1999 (§5.2);
  HotSpot C2 escape analysis (§5.5);
  `research/13-type-system-memory-management.md` §5.
- **Code touchpoints**: `crates/badger_mir/src/sra.rs` (extend
  the analysis to enums; add the NoEscape-with-pointers path);
  `crates/badger_mir/src/opt/simplify.rs` (re-run after `sra`);
  `crates/badger_codegen/src/` (the shadow-stack opcode for
  stack-allocated managed pointers).
- **Cost**: ~500 LOC.
- **Expected win**: ~5–15 % on the `bench/records` workload (the
  I3 / I5 numbers); the existing `sra.rs` is partial, so closing
  the gap removes most short-lived object allocations from
  `gc.rs::alloc`.
- **Gates**: `BADGER_GC_VERIFY=1` (no missing-root regression);
  the existing `bench/parallel_alloc` allocation counter drops;
  the diff on the IR is reviewed for false positives.

### 3.3 Region annotation on builtin types

- **Goal**: add a region brand to `TypeDesc` for selected builtin
  types (records of pointers); add a `region` qualifier in the
  user-facing surface.
- **Prior art**: Tofte-Talpin 1994 (`research/13-type-system-memory-management.md`
  §6.1); Cyclone (§6.4); Cone's `region T` (§4.6); ML Kit's
  ReML (§6.2).
- **Code touchpoints**: `crates/badger_typeck/src/ty.rs` (the
  `Ty` enum); `crates/badger_typeck/src/regions.rs:1-704` (the
  region brand tracking); `crates/badger_runtime/src/desc.rs`
  (the `TypeDesc`).
- **Cost**: ~150 LOC.
- **Expected win**: closes the gap for the records that 3.2's
  escape analysis does not catch (records passed to a function
  that escapes them); ~5 % on the `bench/parallel_alloc` workload.
- **Gates**: the existing typeck region taint test set passes; a
  new test that proves "a `region` record cannot be returned from
  a function whose region does not outlive the return site" holds.

### 3.4 Linear types for resources (file / socket / channel)

- **Goal**: extend `ownership.rs` to track a `linear` mode (use at
  least once) in addition to `affine` (use at most once); apply
  it to the standard library's file / socket / channel modules.
- **Prior art**: Wadler 1990 (`research/13-type-system-memory-management.md`
  §1.3, §4.1); Turner-Mossin-Wadler 1995 (§4.2); Austral
  (§4.5 / §8.1); Swift's `~Copyable` (§3.1, §3.3).
- **Code touchpoints**: `crates/badger_typeck/src/ownership.rs:25-30`
  (add the `Linear` mode); `crates/badger_hir/std/` (the file /
  socket / channel modules).
- **Cost**: ~200 LOC (mode + diagnostics + std updates).
- **Expected win**: removes the finalizer cost on
  file / socket / channel resources; the drop becomes a static
  guarantee. Expected ~3–5 % on the I3 / I5 benchmarks.
- **Gates**: the existing `tests/run/linear_*` and
  `tests/run/file_*` pass; the new diagnostics
  `E04xx` (linear-use-before-move) are reviewed.

### 3.5 Move-only marker for user-defined types

- **Goal**: add a `~Copyable` / `move` marker to user-defined
  types; the existing `sra.rs` (extended in 3.2) then operates
  on them.
- **Prior art**: Swift's `~Copyable`
  (`research/13-type-system-memory-management.md` §3.1, §3.3);
  Mezzo's type-state (§4.11); Austral's linear capabilities
  (§4.5 / §8.1).
- **Code touchpoints**: `crates/badger_typeck/src/ty.rs` (the
  `Ty` enum); `crates/badger_mir/src/sra.rs` (extend to handle
  the marker).
- **Cost**: ~150 LOC.
- **Expected win**: ~3–5 % on user code that opts in; opens the
  door to user-driven scalar replacement.
- **Gates**: opt-in only; existing user code is unaffected;
  `tests/run/move_only_*` passes.

## Phase 4 — Compiler / MIR optimisation (depends on Phase 3 for the lifetime-driven pieces)

Phase 4 is the MIR superoptimisation research. The cheap items
(4.1, 4.2) are independent; 4.3 and 4.4 depend on Phase 3's
lifetime information.

### 4.1 SMT-validated algebraic rules with Z3 (Alive-style)

- **Goal**: refactor `superopt.rs::Candidate` (`superopt.rs:30-32`)
  into `(precondition, transformation)` form, in the style of
  Alive / Alive2 (Lopes et al., PLDI 2015 / 2021); every
  algebraic rule is checked by Z3 over the bitvectors. The
  existing 16-bit brute-force verifier at `superopt.rs:478-511`
  is kept as a fast-path; Z3 is the correctness gate.
- **Prior art**: Alive (Lopes et al., PLDI 2015); Alive2
  (PLDI 2021); `research/09-mir-superoptimisation.md` §1 (the
  "What this means for badger" recommendation 1).
- **Code touchpoints**: `crates/badger_mir/src/opt/superopt.rs:30-32`
  (refactor `Candidate`); `crates/badger_mir/src/opt/superopt.rs:168-369`
  (the algebraic rules); the new `z3::Solver` invocation.
- **Cost**: ~400 LOC (rule definitions + Z3 glue + tests).
- **Expected win**: the existing rules are correct, but a
  Z3-validated set can be *expanded* safely — the rule set
  grows by ~2×; expected ~5–10 % more MIR rewrites, primarily
  on the codegen path.
- **Gates**: every rule has a Z3 proof that the
  `precondition ∧ source ≡ target` invariant holds; the existing
  `tests/run/mir_opt_*` passes; the brute-force verifier still
  returns the same answer (fast-path parity).

### 4.2 egg for straight-line MIR regions

- **Goal**: lift straight-line MIR regions (only `Rvalue::Bin`
  / `Cmp` / `Not` / `Convert` / `Neg` per `mir.rs:218-294`, with
  effects as break points) into the egg equality-saturation
  engine. The extraction step uses the CAV 2022 / PLDI 2021
  extraction line.
- **Prior art**: Tate et al., "Equality Saturation" (POPL 2009);
  Willsey et al., "egg" (PLDI 2021); the Ruler line
  (`research/09-mir-superoptimisation.md` §2.1, §2.2, §2.4).
- **Code touchpoints**: new `crates/badger_mir/src/opt/egg.rs`
  (~600 LOC); the call site in `crates/badger_mir/src/opt/mod.rs`.
- **Cost**: ~600 LOC.
- **Expected win**: the existing superopt is at parity with
  InstCombine for its subset; egg lifts it to *more rules at
  compile time*, with the same correctness guarantee. Expected
  ~3–5 % more rewrites on the inner-loop MIR.
- **Gates**: per-function egg time budget of 50 ms; the rule set
  is the same as the 4.1 rule set (so Z3-validity is a single
  source of truth); the extraction step is size-bounded.

### 4.3 Alive2-style translation validation across the MIR pipeline

- **Goal**: an end-to-end translation validator: for every
  compiled `badger run` of a test program, the validator proves
  the post-MIR-optimisation program is equivalent to the
  pre-MIR-optimisation program, in the style of Alive2.
- **Prior art**: Alive2; the CertiKOS / CompCert translation
  validation line (`research/12-runtime-scaling-extra.md` §15.4);
  `research/09-mir-superoptimisation.md` §1.
- **Code touchpoints**: the codegen driver, after the MIR
  pipeline; the `BADGER_MIR_VALIDATE=1` runtime check.
- **Cost**: ~800 LOC (an SMT model of MIR, the validator, the
  test scaffolding).
- **Expected win**: this is a *correctness* win, not a
  throughput win. It catches any rule that 4.1 missed.
- **Gates**: every existing `tests/run/*` is paired with a
  validator run; the validator must return "equivalent" within
  10 s on a 50-KB MIR.

### 4.4 Lifetime-driven inlining and dead-assignment elimination

- **Goal**: extend the inliner (`inline.rs:1-349`) and the
  dead-assignment eliminator (`simplify.rs:1-780`) to use the
  typeck lifetime information (Phase 3).
- **Prior art**: the bridge from
  `research/13-type-system-memory-management.md` §9.3;
  `research/09-mir-superoptimisation.md` §3.
- **Code touchpoints**: `crates/badger_mir/src/inline.rs:1-349`
  (the inliner's callee-cost model); `crates/badger_mir/src/opt/simplify.rs`
  (the dead-assignment eliminator).
- **Cost**: ~250 LOC.
- **Expected win**: ~3–5 % more inlining decisions (the
  lifetime information rules out a class of false-positive
  inlining candidates); ~2 % more dead-assignment removal.
- **Gates**: the `BADGER_TIMINGS=2` per-pass timing
  distribution is unchanged or improved; the existing
  `bench/messages.bdg` is unchanged within noise.

## Phase 5 — Deferred / not adopted (explicit list)

Each item here was a *candidate* in one of the six research notes,
and the research concluded "do not adopt" for one of the reasons
listed.

| Item | Reason | Research ref |
|------|--------|--------------|
| Full concurrent mark-sweep (C4 / ZGC / Shenandoah) | STW is 0.07–0.16 % of GC time on the measured workloads; the concurrent cost is not justified | `research/11-gc-and-memory-management.md` §2 |
| On-stack replacement (OSR) | badger's two-tier compilation is off-stack; OSR is only justified when compiling *during* execution | `research/12-runtime-scaling-extra.md` §17 |
| Full STOKE / AlphaDev-style stochastic search over assembly | Cranelift is the right layer for that; the language IR gains little | `research/09-mir-superoptimisation.md` §1 |
| Full Polonius / Oxide borrow checker | badger's existing `ownership.rs` is *strictly more permissive* than Rust's borrow checker; porting Polonius is a multi-quarter effort for no clear benefit | `research/13-type-system-memory-management.md` §9.5 |
| RRB persistent vectors | workload mix does not show enough O(log n) `concat` / `slice` to justify ~600 LOC | `research/10-persistent-data-structures.md` §3 |
| Ropes for strings | badger does not have editor-like workloads with frequent mid-string inserts | `research/10-persistent-data-structures.md` §6 |
| `OrdMap` / `OrdSet` (sorted maps) | no current standard-library function is hot enough | `research/10-persistent-data-structures.md` §7 |
| Async / signal-based preemption | the BUDGET poll is sufficient; signal preemption is ~10 µs/signal | `research/12-runtime-scaling-extra.md` §16 |
| Scheduler Activations | not in mainline Linux; badger cannot adopt them on stock Linux | `research/12-runtime-scaling-extra.md` §14.1 |
| HotSpot-style safepoint polling page (§0.1) | the race it removes is gone (the `STOP` + `IN_COMPILED` handshake is race-free under sequentially consistent ordering), and the mechanism only pays off when the *generated code* polls: with the poll in the runtime the collector still waits for the counter to drain, so a page fault would replace the load on a hot path | `research/12-runtime-scaling-extra.md` §16.3 |
| Full Tofte-Talpin region inference | whole-program analysis; badger compiles per function | `research/13-type-system-memory-management.md` §9.5 |
| Hazard Eras | hardware support (TSX-NI) is deprecated on desktop Intel | `research/12-runtime-scaling-extra.md` §13.6 |
| Flat-combining | the cited gain is on MPSC scenarios where one producer is faster than all consumers combined; badger's slow path is rare (1 of 83 891 declines) | `research/12-runtime-scaling-extra.md` §13.8 |
| Full Herbie-style FP precision recovery | badger does not have a numerical hot-path | `research/09-mir-superoptimisation.md` §2.6 |
| MLIR Affine wholesale adoption | badger is not a polyhedral compiler; the per-loop IR model does not fit | `research/09-mir-superoptimisation.md` §1 |

## Gates (in addition to the BACKLOG gates)

The BACKLOG already has the p99 latency and timer-lateness gates;
this research adds four more, and the Phase 1 TLA+ state machine
is the *correctness gate* for the Phase 2 lock-free work.

| Gate | Description | Where enforced |
|------|-------------|----------------|
| TLA+ safety | every Phase 1 refactor must pass the TLA+ model checker on the bounded 3-task / 1-channel model | `spec/tla/ChannelBadger.tla`, TLC config |
| TLA+ liveness | every blocked recv is eventually woken | `spec/tla/ChannelBadger.tla`, TLC config |
| QSBR invariant | the GC handshake's "every worker observes the new epoch" invariant | `spec/tla/GcHandshake.tla` |
| IR parity | the post-MIR-optimisation program must be bit-equivalent to the pre-MIR-optimisation program | `BADGER_MIR_VALIDATE=1` (4.3) |
| 559-file check | the 559-file `badger check` differential sweep has no output differences | the existing test set |
| Workload coverage | every Phase 0 / Phase 1 / Phase 2 change is measured on `bench/messages.bdg`, `bench/parallel_alloc`, `bench/collections`, `bench/records`, and the GC profile workloads | the existing test set |

## Cross-references

The research that motivates each phase:

- `research/09-mir-superoptimisation.md` — Phase 4.
- `research/10-persistent-data-structures.md` — §0.3, §0.4, §0.5.
- `research/11-gc-and-memory-management.md` — §0.6, §0.7, §0.8, §2.3.
- `research/12-runtime-scaling.md` and
  `research/12-runtime-scaling-extra.md` — §0.1, §0.2, §0.9, §0.10,
  §1.1, §1.2, §1.3, §2.1, §2.4.
- `research/13-type-system-memory-management.md` — Phase 3.
- `BACKLOG.md` — the existing in-flight plan; this file is
  *complementary*.
- `reports/2026-10-15-compiler-codegen-optimization-audit.md` —
  the audit that motivates Phase 4's language-aware work.
- `reports/2026-10-17-gc-profiling.md` — the audit that motivates
  Phase 0.6, 0.7, 0.8 and the Phase 2 work.
- `reports/2026-10-19-mir-optimization-architecture.md` — the
  current MIR optimisation architecture.
- `reports/2026-10-19-select-fast-path-fragility.md` — the
  audit that motivates Phase 1.
- `reports/2026-10-15-runtime-concurrency-review.md` — the
  four-worker scaling anomaly that motivates §0.9, §0.10, §2.4.
