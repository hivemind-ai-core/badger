# Optimisation backlog: Phase 0 and the Phase 1 models

Branch `perf/optimization-backlog-2026-10-19`, based on `main` at `c82d139`.
The branch carries `OPTIMIZATION_BACKLOG.md` (and the research notes 09-13 it
refers to) and works through the backlog's Phase 0 items, one commit per
item, each gated on `cargo test --workspace`.

| commit | item | what |
|--------|------|------|
| `d3553e9` | — | the backlog and the research notes |
| `66a5222` | — | a collector bug the work found (below) |
| `e5967d3` | §0.6 | reuse the collector's mark stack and remembered buffer |
| `5bae53c` | §0.7 | index the chunks instead of sorting every cell address |
| `274743a` | §0.8 | account the channel's native buffers as external memory |
| `e470191` | §0.3 | AHash under fixed seeds -- **reverted**, it regressed the map cells |
| `2854c83`, `21303fa` | §0.4 | store maps of at most eight entries as a flat array |
| `beeb63a` | §0.2 | per-worker Chase-Lev ready queues |
| `5182315` | §0.9 | pick two random victims to steal from |
| `c6428ee` | §0.10 | steal half of a long ready queue |

§0.8, §0.3, §0.4, §0.5, §0.2, §0.9 and §0.10 were implemented by delegated
workers in their own worktrees and branches, and were integrated here by
cherry-pick after the full test suite ran on the result. §0.3 was reverted
after it was measured (§ below).

## The measurement that matters: the cost of a full collection

Phase 0's GC items (§0.6, §0.7) only show up in *full* collections, and the
end-to-end benchmarks are too noisy to see them (see "what did not move").
A direct measurement of the collector's pause does:

`crates/badger_runtime/tests/gc_collect_cost.rs` (a scratch test, not
committed) builds a heap with 200 000 live objects and 200 000 garbage ones,
then times 20 full collections. `BADGER_GC_GENERATIONAL=0` makes every
collection full, which is the path `trim_empty_chunks` runs on.

| build | full collection (min of 20, ms) |
|-------|-------------------------------|
| `main` (c82d139) | 186.4, 189.8, 193.3, 189.8 |
| §0.6 only | 189.7, 191.6 |
| §0.7 only | 65.9, 64.1 |
| §0.6 + §0.7 | 65.5, 66.5, 65.5, 65.3 |

**A full collection over a 200 000-object heap is 2.9x faster** (190 ms to
65 ms, -65%). The whole effect is §0.7: the collector used to collect the
address of every live and quarantined cell and sort it (1.6 MB of vector per
collection, O(n log n)) only to ask which chunks hold a cell; it now sorts
the chunk ranges (a few dozen entries) and looks each cell up, which is
O(n log chunks) with O(chunks) memory.

§0.6 is **neutral in this measurement**: reusing the mark stack and the
remembered buffer removes two allocations per collection, which is below the
noise floor here. It is kept because it is strictly less work, not because
it is faster.

The end-to-end benchmarks do not move (medians over 7 interleaved runs,
`bench/collections.bdg`, `bench/records.bdg`, `bench/messages.bdg`, 1 and 4
workers, release builds): the deltas are within +-1% and the per-round
spread is 10-40% on a machine running other agents' builds. Those workloads
mostly run *minor* collections, where the changed code does not run.

## The collector bug the work found

`tests/run/stress/channel_handoff.bdg` under chaos scheduling and
`BADGER_GC_VERIFY=1` aborted with "the runtime used freed object ... (a
`Page`)". **It reproduces on `main` (c82d139)**, so it is not caused by this
branch; it blocked the test gate for every item.

The mechanism (traced with a diagnostic build that printed the owner of
every freed page and the directory slots that named it):

- A list directory holds weak page slots. `copy_tail` and `put_page` copy
  the slot *pointers* into a new directory, so the old directory and the new
  one name the same pages.
- When the version that owns the new directory dies, the pages it shares
  with the old directory die with it: the old directory is not traced in a
  minor collection (only its owner list is), so the shared page is unmarked
  and swept, while the old directory survives (it is old).
- The next full collection walked that dead directory's slots to decide
  whether to clear them, and read a freed cell. The verifier aborts; in a
  normal build it reads a cell the collector may already have reused.

Fix (`66a5222`): the clearing pass skips the directories that are not
marked. A dead directory dies with the collection and its slots are never
used again, so its slots are never read.

## §0.2, §0.9, §0.10: the scheduler (delegated worker)

`beeb63a` replaces the per-worker `LocalQueue` (a spinlock around a
`VecDeque`) with a `crossbeam_deque::Worker` + `Stealer` pair per worker, so
the owner is wait-free and thieves are lock-free; the next slot stays a
separate queue with the visibility it had. `5182315` picks two random victims
and steals from the better one, and `c6428ee` steals half of a long ready
queue instead of one task.

The worker's measurement (release builds, 4 workers, 9 runs each, interleaved
because the machine was at load average 17-32 on 16 cores):

| workload | before (min/median, ms) | after (min/median, ms) |
|----------|------------------------|-----------------------|
| `bench/messages.bdg` | 121.1 / 159.2 | 112.0 / 148.7 (-6%) |
| `bench/parallel_alloc.bdg` | 110.8 / 136.3 | 101.4 / 135.0 (parity) |

The backlog's predicted wins (the parked time on `messages` halved; a 2-5x
cut in steal traffic on `parallel_alloc`) **are not visible in these
benchmarks, and the worker says why**: with `BADGER_RUNTIME_STATS=1`,
`messages` takes ~4000 runtime lock acquisitions, parks 0.01 ms and waits
25-35 times idle, and `parallel_alloc` runs 7-8 tasks in total. The
per-worker queue is a tail-latency and cross-core-traffic fix, and these
throughput benchmarks do not exercise it. No functional or ordering
deviation was found: the corpus (76 programs, single-threaded, `--release`
and `--workers 4`) passes, and the new `crates/badger_runtime/tests/
scheduler.rs` asserts the order (FIFO for the owner and for thieves,
next-slot first), the exact queued accounting, the half-of-nine batch bound
and that two distinct random victims are picked.

## §0.3: AHash is not adopted (delegated worker, measured and reverted)

`ed47f20` hashed map keys with AHash under **fixed** seeds, so a key hashes
the same in every process. The worker measured it and **reverted** it
(`e470191`): the item's gate is "the map cells within noise", and AHash is
1.6-2x slower than FxHash on the map cells.

| workload (200k-entry maps, median ns/op) | FxHash | AHash |
|--------------------------------------------|-------:|------:|
| `bench/collections` persistent map insert | 120 | 221 |
| `bench/collections` map lookup | 108 | 235 |
| stride-1024 keys, insert / lookup | 129 / 110 | 166 / 189 |
| pseudo-random keys, insert / lookup | 117 / 124 | 212 / 188 |
| String keys, insert / lookup | 1115 / 441 | 1066 / 450 |

Hashing an Int costs the same with both (1.4 ns vs 1.1 ns): the cost is
the trie *shape*. FxHash's multiply-and-rotate acts like Fibonacci hashing
on arithmetic key sequences and spreads them near-perfectly over the 5-bit
chunks, so the tries stay shallow; AHash spreads keys at random (Poisson)
and the tries get deeper. The item's premise -- that FxHash's chunks collide
more than AHash's -- does not hold for these workloads. String keys, where
both hashes look random, are neutral.

## §0.4, §0.5: the collections (delegated worker)

`2854c83` stores a map of at most eight entries as a flat array (the
Clojure `PersistentArrayMap` idea): a root with both bitmaps zero, scanned
linearly, promoted to the trie at the ninth entry and flattened again when a
removal brings it back to eight (`21303fa` keeps the order stable across
those transitions). The flat root is an ordinary `MapNode`, so tracing, the
cursor layout and the map payload are unchanged, and the flat entries are
kept in the order the trie would visit them, so iteration order does not
depend on the representation.

`dee35dd` makes `map::eq` a paired walk of the two tries that skips a
subtree the two maps share, with a reflexivity check (a float NaN is not
equal to itself). The item's O(1) `map::hashcode` is **not** done: it would
need a per-map hash cached in the payload, which collides with the 16/24-byte
payload layout, and it would hash every value on the insert path.

## §0.8: external memory accounting (delegated worker)

`274743a` charges the channel's native buffers -- the prompt queue, the
message sizes, the sealed batches, the per-producer staging, the unreleased
leases, the recycled page pool -- and the admission gates' waiter queues to
the collector's external counters as they grow and as they are freed, moved
or dropped, and reports resident memory next to managed, external and
pressure in the GC profile snapshots.

Evidence from the worker: with `BADGER_GC_PROFILE=1`, `bench/messages.bdg`
now reports `external_before=37536`/`external_after=33440` at one worker and
`35488`/`31392` at four, where the unmodified build reported nothing; a
balance test returns the external total to zero; `BADGER_GC_VERIFY=1` over
`bench/messages.bdg` and `tests/run/batch_lease_task_exit.bdg` is clean; the
corpus passes 76/76.

## §0.1: the safepoint polling page is not adopted as written

The item asks for a HotSpot-style "mprotect a per-worker page" trap in
place of the `STOP` poll. Two things make it unsound as specified:

1. **The race it removes is already gone.** The item cites the audit's "G3"
   (GC stop request races with generated budget updates), which describes
   `stop_the_world` writing *another worker's* `TaskCtx.budget` with
   `write_volatile` (`reports/2026-10-15-runtime-concurrency-review.md`).
   That code is gone: the budget belongs to the running task, and the
   handshake is now `STOP: AtomicBool` plus a per-worker `IN_COMPILED`
   counter (`task.rs:938-947`, `task.rs:1496-1508`). That protocol is
   race-free under its sequentially consistent ordering: the worker
   increments `IN_COMPILED` *before* it reads `STOP`, and the collector
   stores `STOP` *before* it reads the counters. Either the collector sees
   the worker and waits for it, or the worker sees `STOP` and backs out.
   There is no window in between.
2. **The mechanism only pays off when the generated code polls.** HotSpot's
   polling page works because compiled code polls at safepoints, so the
   collector knows a running thread will reach a poll. In badger the poll
   is in the runtime (`gc_enter`, called before a coroutine resumes), so
   the collector must still wait for `IN_COMPILED` to drain; the page fault
   would replace the load, not the wait. A fault per task resume (~1 us)
   against a load (~1 ns) is a regression on a hot path, and the stop-world
   pause it would shorten is 0.07-0.16% of GC time
   (`reports/2026-10-17-gc-profiling.md`).

The real prerequisite is safepoint polls in the *generated* code (a compiler
change, with the poll at back-edges and calls). That belongs with the
Phase 2 QSBR work, which needs the same contract, and with the Phase 1
TLA+ model of the handshake. §0.1 is recorded in the backlog's "Not
adopted" table with this reason, and the handshake is instead specified by
`spec/tla/GcHandshake.tla`.

## Phase 1: the TLA+ models, and three findings

`aec138e` adds the Phase 1 models (`spec/tla/`): the per-channel state
machine (plain TLA+, with the code's ownership protocol as the atomicity
rule) and the stop-the-world handshake (PlusCal), with a runner that
executes every check and compares the outcome with the expected one, and
two Badger programs that reproduce the findings on the real runtime.

The models are the contract the later phases must keep, and they are
checked against the current code: the prompt-channel protocol satisfies
`NoLostWake`, `RegisteredWhileWaiting`, `AtMostOnce` and
`EveryBlockedRecvWoken` at the model's bounds (3 tasks, 1 channel, prompt
capacity 0 and 1, batch capacity 2, one and three workers, cancellation and
spurious wakes), and the GC handshake satisfies every property. Each
deliberate mutation of either protocol is caught (a fast path that ignores
the watchers, a selection that does not claim, a worker that checks `STOP`
before it counts itself, one that unlocks before it counts, one that
publishes a root after it uncounts, a nested stop). All 17 checks were
re-run on this branch after the models were integrated: every outcome
matched its expectation.

**Three findings.** The model of the *current* code passes the properties
the backlog asks for, and still finds three things:

- **F1 -- a value handed to a receiver that is then cancelled is lost.** The
  receiver is handed the value and becomes ready; before it runs, it is
  cancelled, so the wake finds it ready and does nothing; when it resumes,
  `park()` honours the cancellation before the receive takes the delivered
  value, and the task abandons. The sender got success and nobody has the
  value, which contradicts "After ownership commits, cancellation does not
  silently retract a message" (`spec/05-tasks-and-channels.md:141`).
  Reproduced at `13e7756` with one worker (`spec/tla/repro/
  cancel_after_handoff.bdg` prints "empty: the sent value is gone") and
  reproduced again on this branch.
- **F2 -- a published batch wakes one receiver; the rest of its messages
  strand.** `publish` wakes one parked receiver per sealed batch, but
  `recv_one` takes one message, so with two receivers parked and a batch of
  two published the second receiver stays parked with a message published,
  until another publish or the close. That contradicts "multiple receivers
  distribute messages" (`spec/05-tasks-and-channels.md:113-114`).
  Reproduced at `13e7756` with one and with four workers
  (`spec/tla/repro/batch_two_receivers.bdg` prints "r1 got 1" and then
  deadlocks) and reproduced again on this branch.
- **F3 -- the queue branch of `send` wakes the watchers and forgets them.**
  With a selection parked on a capacity-1 channel, a locked send queues its
  value and wakes the selection without handing it the value, so the
  abstract state goes `Blocked` -> `WithValue` in one step and the
  selection's registration is dropped. Not a lost wake-up in the current
  code (the model says so), but the woken selection then races every other
  receiver for the value.

F1 and F2 are spec violations, not just smells: they are recorded in
`BACKLOG.md` and are part of the §1.2 refactor's acceptance.

**The model also settles §0.1.** The GC handshake model says the same thing
the phase 0 analysis said, in the model's own terms: the protocol is
sequentially consistent and every property holds; what it relies on is
store-then-load on both sides (`STOP` and the counters, all `SeqCst`),
counting before unlocking, publishing a root before uncounting, and no
nested stop. A polling page would have to keep the same invariant, and
without polls in the generated code it buys nothing: the collector still
waits for the counters to drain.

## What did not move

- The end-to-end benchmarks (§ above): the GC items only run on full
  collections, and the workloads are dominated by minor collections and by
  noise from a busy machine.
- §0.6: two allocations per collection, below the noise.
- The `badger check` / `badger test` front end: untouched by Phase 0.
