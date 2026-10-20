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

Re-measured at the end of the branch on a quiet machine (load average 2
instead of 17-33), with the branch's other changes included: `main` 91.0,
91.5, 92.6 ms, the branch 36.1, 35.6, 36.2 ms -- **2.6x faster, -61%**.

§0.6 is **neutral in this measurement**: reusing the mark stack and the
remembered buffer removes two allocations per collection, which is below the
noise floor here. It is kept because it is strictly less work, not because
it is faster.

The end-to-end benchmarks move a little, not a lot (medians over 5-7
interleaved runs, release builds, the whole branch against `main`):

| workload | 1 worker | 4 workers |
|----------|---------:|----------:|
| `bench/messages.bdg` | 80.0 -> 78.9 ms (-1.4%) | 81.2 -> 77.8 ms (-4.2%) |
| `bench/collections.bdg` | 188.7 -> 185.4 ms (-1.7%) | 188.7 -> 187.7 ms (-0.5%) |
| `bench/records.bdg` | 762.1 -> 761.4 ms (-0.1%) | 1207.6 -> 1219.6 ms (+1.0%) |
| `bench/parallel_alloc.bdg` | 91.6 -> 89.7 ms (-2.1%) | 74.7 -> 76.5 ms (+2.4%) |

The workloads mostly run *minor* collections, where the changed collector
code does not run, so the end-to-end numbers are within a few percent; the
channel refactor's round-trip p99 (below) is the tail measurement the
end-to-end medians cannot show. Measurements taken while the machine was
running other agents' builds (load 17-33) had a per-round spread of 10-40%
and are not quoted.

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
equal to itself). **This is the win of the collections work**: comparing two
related 100 000-entry versions went from 4.7 ms to 0.32 us.

The item's O(1) `map::hashcode` is **not** done: it would need a per-map hash
cached in the payload, which collides with the 16/24-byte payload layout, and
it would hash every value on the insert path.

**§0.4's expected win did not materialize.** Measured on release builds
(median of 5 interleaved runs): a small-map lookup is 17 -> 17 ns at 4 and at
8 entries, a small-map build is 34 -> 41 ns/insert at 4 entries and 35 -> 40
at 8 (with the same minimums, 33 and 35-36), and `bench/collections` map
insert and lookup are 67 -> 68 and 46 -> 46 ns. The reason is in the trie: a
root of at most eight entries is *already* a single bitmap-indexed node, so
the flat array has nothing to save. The item is kept because it is correct
and tested, and because the measurement belongs in the record.

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

**Four findings.** The model of the *current* code passes the properties
the backlog asks for, and still finds three things (a fourth, below, needs
the refactored code):

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

**F4 -- a batch receiver woken to retry and then cancelled takes its wake
with it (fixed).** Found by the model of the *refactored* protocol
(`16bbcbb`): a publication wakes one receiver per message, but the wake is
a wake to *retry* -- the woken receiver takes a message only when it runs
-- and if it is cancelled before it runs, `park()` honours the interrupt
(rightly: nothing was handed to it, its status is not 1) and the task
abandons without passing its wake on. The message is stranded: no receiver
was on its way to take it, and the next parked receiver waits for a message
nobody is coming for. Reproduced on the real runtime at `9f1ff4c` with one
worker. It needs four tasks, so it was outside the three-task bounds the
backlog names, and the §1.2 contract had the same gap.

The fix (`ac3f85c`): a channel keeps the receivers it woke for a published
message and that have not taken it yet. A receiver releases its claim when
it takes the message, when it finds none (it registers again), and when it
dies -- `task_finished` releases the claim of a task that died holding one.
Releasing a claim passes it on: a published message that no woken receiver
is on its way to is offered to another receiver, so a parked receiver is
never left asleep with a message waiting. The regression test
(`crates/badger_runtime/tests/channel_state.rs`) fails without the fix (the
third receiver waits forever) and passes with it; the repro program is
rewritten to be deterministic ("stranded 1" before the fix, "taken" after).

**The model also settles §0.1.** The GC handshake model says the same thing
the phase 0 analysis said, in the model's own terms: the protocol is
sequentially consistent and every property holds; what it relies on is
store-then-load on both sides (`STOP` and the counters, all `SeqCst`),
counting before unlocking, publishing a root before uncounting, and no
nested stop. A polling page would have to keep the same invariant, and
without polls in the generated code it buys nothing: the collector still
waits for the counters to drain.

## §1.2: the Go-style channel refactor (delegated worker)

`c4750f8` and `9f1ff4c` replace the channel's parallel waiter lists and
ad-hoc guards with one state word and one waiter list, and give every
operation a linearization point:

- one `AtomicU8` word holds the owner (free / fast / claimed), the phase
  (empty / value / blocked) and the closed bit;
- one `Waiters` list with per-kind counts replaces `recv_waiters`,
  `send_waiters`, `cap_waiters`, `space_waiters` and `recv_watchers`;
- every operation linearizes at the CAS that makes it the owner (the fast
  path) or at the claim (the locked path); the releasing store publishes the
  phase; `recv_fast` and `recv_one_fast` decline on a published empty phase
  with one load and no CAS;
- a locked send commits to a parked selection at **every** capacity (today
  only at capacity 0), and queueing, publishing and waiting for space no
  longer drain the watcher registrations (F3);
- `publish` wakes one receiver per published message (F2);
- a selection is committed only through `task::waits` on its registered wait.

`c4750f8` fixes F1: `park()` no longer honours a cancellation that arrives
after a waker completed the wait with a value, so the handed-off value
reaches the receiver.

**Both repro programs now pass on this branch**: `cancel_after_handoff.bdg`
prints "victim got 111" (the value arrives), and `batch_two_receivers.bdg`
prints "r1 got 1", "r2 got 2", "done" (no deadlock). The new
`tests/run/channel_handoff_contract.bdg` and
`crates/badger_runtime/tests/channel_state.rs` fail on the old runtime, so
they are regression tests.

The audit's fast-path counters are kept behind `BADGER_RUNTIME_STATS=1` and
printed with the lock stats. With them on, `select_and_budget` passes 6 of 6
and the golden is unchanged -- so the audit's "adding the counters flips the
test into a deterministic deadlock" was timing-sensitive, and the model says
the same: the reconstructed counters do not reproduce the deadlock either.

The worker's measurements (medians, old to new; the machine was at load
average 9-20 from other agents):

| workload | before | after |
|----------|-------:|------:|
| `bench/messages.bdg` prompt, 1 worker | 25 ns/msg | 26 ns/msg |
| `bench/messages.bdg` prompt, 4 workers | 30 ns/msg | 30 ns/msg |
| `bench/messages.bdg` batch, 1 / 4 workers | 34 / 38 ns | 33 / 38 ns |
| round trip, 1 / 4 workers | 238 / 267 ns | 251 / 246 ns |
| round-trip p99, 1 worker (25 runs) | 351 ns | 280 ns |
| round-trip p99, 4 workers (9 runs) | 622 ns | 461 ns |
| `bench/independent_channels.bdg`, 4 workers | 68 ms | 66 ms |
| fast send + recv pair | 12.0 ns | 13.7 ns |
| empty recv | 3.3 ns | 2.5 ns |

The throughput medians hold; the round-trip **p99 improves by 20-26%**, which
is what the item was for (the tail, not the mean). The fast send+recv pair
costs 1.7 ns more (the phase now lives in the same word as the owner) and an
empty recv is 0.8 ns cheaper (one load, no CAS).

**Not done:** the TLA+ model was not re-run against the new protocol -- the
model's "current code" configuration still describes the old one, so
checking the new implementation against the contract is the next step of
§1.2, not a gap in the refactor.

## §2.1: the QSBR handshake (delegated worker)

`6cd8e06` replaces the `STOP` flag and the sum of the `IN_COMPILED`
counters with a grace period: a global epoch is odd while a collector stops
the world, each worker's slot holds the even epoch it observed when it went
online (or `OFFLINE` in a quiescent state), and the collector reads each
slot until it has acknowledged the new epoch. A stop depth keeps a nested
stop and resume from restarting the world.

**It is not a latency win, and the item says so.** The wait still ends when
the slowest worker reaches its next safe point, which no epoch shortens. The
idle case improves (`bench/records.bdg` 0.9 -> 0.3 us at 4 workers, 570
collections) because the live workers' slots are read instead of all 64
counters -- which the old protocol could have had in one line -- and
stop-world time was already 0.07-0.16% of GC time, so nothing moves end to
end. The `bench/parallel_alloc.bdg` numbers at 4 workers (2.5 -> 2.3 ms and
5.4 -> 4.3 ms) are preemption of a running worker, not a credible
comparison.

What it does buy is robustness, which is why it lands: the model's "no
nested stop" and "balanced slots" dependencies go away, so release builds
no longer rely on the finalizer's debug assertion for nesting safety, and
the acknowledgement becomes a real per-worker epoch rather than a sum. The
gates pass (the corpus 76/76 single-threaded and at 4 workers, the stress
and gc_retention suites, `select_and_budget` 6/6 at 1 and at 4 workers,
`BADGER_GC_VERIFY=1` clean), and the stress test catches the model's
`check_before_inc` race in 6 of 10 mutated runs.

## F4, and the other work after Phase 2.1

**F4 is fixed** (`ac3f85c`): a channel keeps the receivers it woke for a
published message and that have not taken it yet; a receiver releases its
claim when it takes the message, when it finds none, and when it dies; a
released claim is passed on, so a parked receiver is never left asleep with
a message waiting. The regression test fails without the fix and passes
with it, and the repro program is deterministic now ("stranded 1" before
the fix, "taken" after it).

**§2.4 (NUMA pinning) landed behind a flag** (`808f3b2`): worker `w` pins
itself to CPU `w` when `BADGER_PIN_WORKERS=1` is set. The item's gate is a
measurement on a two-socket host, and this host has one socket, where the
notes put the gain under 5% -- so the change is opt-in and the measurement
is left to a two-socket deployment.

**§2.2's premise is already satisfied**: the bump pool and the free lists
are per-worker (`POOLS.0[w]`, `alloc_cell(w, ...)`, `free_cell(w, ...)`),
and they were per-worker at the snapshot the backlog was written against.
The missing part in that area is §2.3's block and line layer.

**The suite was red on `main` for a while**, for a reason outside this
work: the migration of the derive clauses to prefix attributes put two of
them inside their struct bodies, so `std.compiler` did not parse and the
MCP session's compiler queries failed (`7ca370b`).

## The wave after Phase 2.1

**§2.3 (Immix-style blocks and lines)** landed (`9d6d5b1`, `5a14cda`):
chunks are divided into 32 KiB blocks of 16-byte lines, each headed by its
line bitmap; cells are bump-allocated in holes, the sweep marks the lines of
its list's survivors, and the pool is rebuilt from the bitmaps after each
collection -- partly live blocks are reused in place, blocks with no live line
are reused whole, and a full collection releases the chunks with no live line
by bitmap. The size-class free lists are gone. The item's expected win holds:
the telemetry work ratio 0.79/0.90/0.82/0.85 (balanced and fanout, at one and
four workers), the mutator time 13-22% lower, the process CPU 11-18% lower,
and the full-collection trim down from up to 24 ms to 1-3 ms. The trade-off:
the peak RSS of a heap fragmented at the grain of single cells goes from 59 to
66 MiB.

**§3.1's arena half is not adopted**, and the reason is the item's premise:
the typeck never touches the managed heap, so an arena there would be dead
code. The traffic the item aimed at is real, though (~831K allocations per
check), and the fix was better than the arena would have been: the type tree
is shared when nothing changed, and the children walk does not allocate
(`090efbc`). Allocations go 831K -> 386K, the type check phase over the
610-file corpus 46.2 s -> 31.2 s (-33%), the whole check 80.0 s -> 57.3 s,
with stdout, stderr and exit status identical on every file.

**§4.3 (translation validation)** landed in its cheap form (`a9618b9`):
`BADGER_MIR_VALIDATE=1` runs a bit-for-bit whole-program MIR comparator over
two optimiser runs, and a behavioural gate (the program runs optimised and
with the MIR pipeline off). The item's literal "optimised against
unoptimised, bit for bit" is not a gate -- the release passes restructure by
design, so it rejects correct and broken optimisations alike -- and the
behavioural check is the sound one: it catches the deliberately broken rewrite
(`n*4` -> `n<<3`, silent `y=24`).

**§1.3's model of the GC handshake** was rewritten for the QSBR grace period
(`8a9f2a4`): 51 checks, ten mutations, every property has a mutation that
breaks it, and the model finds no defect in the protocol.

## Phase 3 and Phase 4

**§3.2 (MIR escape analysis) is not adopted** (2026-10-20): after `sra`,
`enums` and `niche`, the only allocation in a timed loop of `bench/records` is
the record stored with `Atom.set` -- a real escape by design (19 998 710
objects over the loop's 20 M iterations, unchanged by any escape analysis).
Every hot allocation on the other benchmarks escapes for real (returned,
appended, inserted, spawned, sent or registered). Over the 242 `tests/run`
programs, 9 207 of 10 279 sites escape; the remaining 1 072 are merges (360),
arguments of direct or static-closure calls (461) and arguments of read-only
runtime functions (~220, which needs the runtime to accept a non-heap object).
None of them is in a measured hot loop, so the expected win is 0%, not 5-15%,
and the use-after-free surface (a new rvalue, root exclusion, publishing
pointer fields at safe points, loop-reuse liveness, callee summaries) does not
pay for a measured 0%. The survey and its script are in
`reports/2026-10-20-mir-escape-analysis.md`.

**§3.4 (linear resources) landed**: the typeck tracks a `Linear` mode (use at
least once) next to `Affine` (use at most once), and the standard library's
file and batch-channel resources are linear. The new diagnostic `E0413` says
so, with a span and a message naming the resource. The runtime does not
change: the mode is a static guarantee, and the finalizers stay.

**§4.1 (SMT-validated rules) landed**: `opt::smt` states every algebraic rule
`opt::superopt` applies as a bitvector claim `premises => src == dst` and asks
Z3 for a counterexample, so an unsatisfiable answer proves the rewrite is
exact at every bit of the operation's width. `Candidate` now names the rule it
matched instead of carrying a hand-written "proven" flag, and the match site's
premises (an operand's boolean domain, a nonzero shift amount) are stated with
the rule. Each shift amount the optimiser can use is checked as a concrete
constant rather than quantified -- Z3 does not decide the 64-bit
variable-divisor division identity within the budget. `BADGER_MIR_SMT=1` runs
the check once per compilation (11 rules, 81 claims, ~170 ms), prints the
verdict for each rule, and withholds any rule the solver does not prove; the
optimiser's hot path never calls the solver, and the corpus is identical with
the check on and off at `--opt superopt`.

**The check found a wrong rule**: `(x << k) >> k` was masked with `~(2^k - 1)`,
which keeps the high bits, while the left-then-right shift pair keeps the low
`bits - k` bits. The mask is corrected and the rule is proved. That is the
item's point: the rules are proven, not assumed.

## §4.4 is not adopted either (the survey)

The item asks the inliner and the dead-assignment eliminator to use the
type checker's lifetime information. The type checker has none that the two passes lack:
`ownership.rs` is a checker over affine and linear values whose facts end with the
check, and the MIR erases what it distinguishes (`&x` lowers to `x`'s value, so a
moved, borrowed or exclusive argument is the same word). An oracle computing the
strongest such fact on the MIR itself (liveness after each call and assignment) over
the 256 `tests/run` programs changes **0** inlining decisions and removes **0**
assignments. Of the 438 size-rejected candidates of round 0 (21 527 taken by the cost
model's other tests), 52 have an unused result and none fits the budget with its
return slice removed; "the argument is never used again" holds at 87% of accepted and
74% of rejected sites, so it does not discriminate. After `reach`, all 12 147
assignments with a dead destination are kept for their effect (12 141 calls, 5 checked
operations, 1 concatenation) -- `dead_assignments` already removes every removable one.
No code changed, so the timing and `bench/messages.bdg` gates hold by construction.
What would pay instead is **effect** information (a total, effect-free callee with an
unused result), not lifetimes. The report and the survey script are in
`reports/2026-10-20-lifetime-driven-optimisation.md`.

## The last two language features

**§3.3 (region blocks and region records)** landed: `region Scratch { ... }`
names a lexical region that ends with the block (Tofte-Talpin's letregion,
Cyclone's region blocks), and `region` is contextual. A struct or enum whose
last type parameter is a region (`struct Node<R: Scope>`) is a *region record*,
branded with its region in `Ty`. The block is checked one level deeper with a
fresh lexical region skolem, so its records cannot be the block's result, be
returned or broken out of it, or reach an outer var or captured state; the
checker treats the block the way it treats a `for<S>` callback for closures,
tasks, dyn values and stores. The violations are the new **E0414**. HIR lowers
the block to an ordinary block, so the MIR, the codegen and the runtime see a
plain block, and `TypeDesc` gains a static `region` flag that tracing and
collection ignore.

**§3.5 (move-only types)** landed: a struct or enum marked `@move_only` is
affine by declaration -- its values, and the aggregates holding them, are moved
and never copied, and each copy the affine check would reject is reported as
one. A generic function or closure that uses its argument twice, and taking a
value out of a borrow, are copies too. The diagnostic is **E0415** (E0414 went
to the region blocks, which landed first). Unlike a linear resource (E0413), a
move-only value may be dropped unused. The item's escape-analysis half does not
apply: §3.2 was surveyed and not adopted, so the marker's value is the static
guarantee, not the scalar replacement it would have enabled.

## Where the backlog stands

| phase | item | outcome |
|-------|------|---------|
| 0 | 0.1 safepoint polling page | **not adopted** (the handshake is already race-free; the mechanism needs polls in the generated code) |
| 0 | 0.2 Chase-Lev ready queues | landed |
| 0 | 0.3 AHash | **reverted** (1.6-2x regression on the map cells) |
| 0 | 0.4 flat map of at most eight entries | landed, but the expected win did not materialise |
| 0 | 0.5 CHAMP invariants | landed as a paired walk (the O(1) hashcode is not) |
| 0 | 0.6 scratch buffers | landed (neutral) |
| 0 | 0.7 chunk index | **landed: a full collection 91 ms -> 36 ms** |
| 0 | 0.8 external memory accounting | landed |
| 0 | 0.9 two random victims | landed |
| 0 | 0.10 half-steal | landed |
| 1 | 1.1 channel model | landed (38 checks, then 51 with the GC model) |
| 1 | 1.2 Go-style channel refactor | landed, with F1 and F2 fixed |
| 1 | 1.3 GC handshake model | landed, rewritten for the QSBR grace period |
| 2 | 2.1 QSBR handshake | landed as a robustness change, not a latency win |
| 2 | 2.2 per-worker heap segments | **already satisfied** (it was, before the backlog was written) |
| 2 | 2.3 Immix-style blocks and lines | **landed: the 10-20% win holds** |
| 2 | 2.4 NUMA pinning | landed behind a flag (its gate needs a two-socket host) |
| 3 | 3.1 the typeck's arena | **not adopted**; the traffic it aimed at was fixed instead (typecheck -33%) |
| 3 | 3.2 MIR escape analysis | **not adopted** (surveyed: the hot allocations escape for real) |
| 3 | 3.3 region annotations | landed (`region` blocks and region records, E0414) |
| 3 | 3.4 linear resources | landed (E0413) |
| 3 | 3.5 move-only marker | landed (`@move_only`, E0415) |
| 4 | 4.1 SMT-validated rules | **landed: the check found a wrong shift mask** |
| 4 | 4.2 egg for straight-line MIR | open |
| 4 | 4.3 translation validation | landed in its cheap form; the SMT version is open |
| 4 | 4.4 lifetime-driven inlining | **not adopted** (surveyed: the MIR's liveness already has every lifetime fact the type checker knows -- 0 inlining decisions and 0 assignments change) |

## What is left

Phase 0 is done, with three items measured against their gates rather than
assumed: §0.1 is not adopted (with the reason), §0.3 is reverted (measured
regression), §0.4 is kept without its expected win (the premise did not
hold), and §0.5 is partly done (the O(1) `map::hashcode` is not, because it
needs a payload word the map layout does not have). Phase 1 is done as far
as the refactor goes; the model of the *new* protocol is being updated.

The rest of the backlog is not started, and each phase is a project of its
own:

| item | what it needs | why it is not done here |
|------|---------------|------------------------|
| §1.1/§1.3 follow-up | the model of the refactored protocol, and the TLC run against it | **done** (`16bbcbb`): the model now describes the new protocol, all 33 checks match, and it found F4 (above), which is open |
| F4 | a batch receiver woken to retry and then cancelled must pass its wake on, or the message must go to another receiver | **fixed** (`ac3f85c`): the channel keeps the claims, and a released claim is passed on |
| §2.2 per-worker heap segments | a per-worker bump pool and free list | **already satisfied**: `POOLS.0[w]`, `alloc_cell(w, ...)` and `free_cell(w, ...)` are per-worker, and were at the snapshot the backlog was written against; the missing part was §2.3's block and line layer, which has landed |
| §2.3 Immix-style blocks and lines | a line bitmap and in-place reuse of dead lines | **landed**: the expected 10-20% holds (the work ratio 0.79-0.90, the mutator time 13-22% lower, the trim 24 ms -> 1-3 ms) |
| §3.1 the type check's allocation traffic | an arena for the typeck's short-lived nodes | **not adopted as an arena** (the typeck never touches the managed heap); the traffic was fixed instead -- allocations 831K -> 386K, the type check phase -33% |
| §4.3 translation validation | an end-to-end validator over the MIR pipeline | **landed in its cheap form** (the structural comparator and the behavioural gate); the SMT version is open |
| §3.2 MIR escape analysis | stack-allocate the records that do not escape | **not adopted**: after `sra`/`enums`/`niche` the hot allocations on every benchmark escape for real, and the leftovers run once or cross into the runtime -- the expected win is 0% |
| §3.4 linear resources | a `Linear` mode in the typeck, applied to the file, socket and channel modules | **landed**: `E0413` says when a linear resource was not used |
| §4.1 SMT-validated rules | every algebraic rule proven with Z3 | **landed**: 11 rules, 81 claims; the check found a wrong shift mask |
| §2.1 QSBR | a per-worker epoch in the handshake, and the model's `EveryWorkerObservesEpoch` as the gate | **landed** (above) as a robustness change: the epoch makes the acknowledgement real and removes the "no nested stop" and "balanced slots" dependencies, but its latency claim did not hold |
| §2.2 per-worker heap segments | a design phase: which objects are worker-local, the segment layout, the migration rule | needs the QSBR handshake first |
| §2.3 Immix-style block reclamation | the block allocator of §2.2 | depends on §2.2 |
| §2.4 NUMA pinning | the segment layout (§2.2) to place | a ~30-line change that only pays off with §2.2 |
| §3.1-§3.5 type-system memory management | region types, escape analysis, region annotations, linear types | a language change: typeck, HIR, MIR and the standard library, with a spec to write first |
| §4.1-§4.5 compiler work | Z3-validated rules, egg, translation validation, lifetime-driven inlining | the biggest phase: the SMT and translation-validation infrastructure does not exist yet |

The one thing that is cheap and still open is §0.5's O(1) `map::hashcode`:
it needs a payload word, which the map's 16/24-byte layout (root, length,
and the ownership epoch for a wrapper) does not have, so it is a layout
change rather than a hash change.

## What did not move

- The end-to-end benchmarks (§ above): the GC items only run on full
  collections, and the workloads are dominated by minor collections and by
  noise from a busy machine.
- §0.6: two allocations per collection, below the noise.
- The `badger check` / `badger test` front end: untouched by Phase 0.
