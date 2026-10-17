# Memory management implementation tracker

Work branch: `perf/memory-management-2026-10-17`.
Initial source revision: `926cdbd19e2bcc889d2499d646a8a0ad0eb69b0c`.
Main remains untouched. Commit each completed implementation slice with its
correctness checks, raw benchmark evidence, and results recorded here.

## Measurement and acceptance

- [x] M0: Bring the existing opt-in GC diagnostics and coverage harness onto this branch.
- [x] M1: Add missing immutable-builder and producer/consumer coverage; save a fresh
  fixed-revision baseline before changing production code.
- [ ] M2: Compare immutable baseline and candidate executables with shuffled,
  interleaved repetitions, independent output checks, executable/source hashes,
  cleared inherited LANG settings, and raw results retained in the repository.

Before each applicable slice, cover its lifetime/failure cases. Track workload elapsed time, managed allocation counts/bytes where available,
collection count and pause time, process peak RSS, and application throughput at
1/2/4/8 workers. Separate workload timing from frontend/JIT and process RSS.
Include transient and retained graphs, true snapshots, replacement-heavy and
new-key map building, pointer-containing values, long-lived consumers,
backpressure, slow consumers, fanout, cancellation, and escaping results.
Tail-latency claims require enough samples; maxima from a few runs are not p99.
Diagnostic builds and experimental GC settings are separate from default builds.
A change is not complete until semantic checks, GC stress/verification where
relevant, applicable tests, formatting, and lint checks pass. Report mixed or
negative results; do not mark architectural sketches as implemented.

## Implement in small runnable slices

### Reduce allocation and improve immutable collections

- [ ] I1: True transient HAMT insertion for fresh, affine MapBuilder/SetBuilder
  nodes. Reuse only exclusively owned nodes; preserve immutable snapshots,
  collision handling, pointer tracing, and old-to-young write barriers.
- [ ] I2: Broaden enum/record/closure scalar replacement across proven nonescaping
  uses and specialized call boundaries; preserve scheduling and GC liveness.
- [ ] I3: Extend sequence fusion through stored/composed pipelines where semantics
  permit; benchmark visible, stored, collected, and eager forms separately.
- [ ] I4: Pack fixed-size pointer-free records directly into collection storage,
  then evaluate pointer-bearing layouts with precise tracing.
- [ ] I5: Reuse consumed immutable structures when uniqueness is proven, including
  descendant ownership; retain copying when versions share nodes.
- [ ] I6: Evaluate compact small-map, singleton-collection, and short-string forms
  against observed size distributions and object-size inflation.
- [ ] I7: Borrow parsing/string/byte views where lifetime permits; copy small
  escaping views when necessary to avoid retaining large source buffers.

### Allocator, collector, and runtime work

- [ ] G1: Measure allocation counts/bytes by source/type and survival/escape class;
  distinguish native allocations from managed allocations and live snapshots.
- [ ] G2: Optimize pool refill/reuse and clearing based on initialization contracts;
  evaluate size-class pages, occupancy metadata, and bulk empty-page reclamation.
- [ ] G3: Reduce sweep bookkeeping and repeated helper-thread/scratch allocation;
  preserve finalizers, stable addresses, and bounded collection coordination.
- [ ] G4: Prototype adaptive nursery sizing and major cadence using survival,
  external bytes, memory limits and pause goals. Compare throughput/RSS/pauses;
  a larger fixed nursery is not an accepted substitute.
- [ ] G5: Reuse task/TaskCtx/queue metadata and owned buffers where lifetimes are
  explicit. Bound retained pool capacity and account for native resources.
- [ ] G6: Shorten unnecessary shadow-root retention, including pointer provenance
  through word-based runtime calls, with stress coverage for hidden aliases.

### Request regions and producer/consumer ring arenas

- [x] R0: Specify the ownership protocol and a benchmarked minimal scope before
  introducing cross-heap references. Transfer is not proof of graph uniqueness.
- [ ] R1: Prototype bounded FIFO storage shared by a producer/consumer pair for
  pointer-free message data. Reserve/write, publish, receive, acknowledge, reclaim;
  wraparound, alignment, oversize messages, and full/empty behavior must be tested.
- [ ] R2: Integrate channel topology/capability selection: prove single producer
  and consumer statically or opt in explicitly; retain general channel behavior
  for aliases, extra participants, select, fanout, and unknown topology.
- [ ] R3: Reclamation follows completed processing, not dequeue. A borrowed message
  must not escape its lease or be retained in a closure/task/result. Out-of-order
  completion needs ordered acknowledgements or separate segments. Support task
  migration, close, cancellation, blocked senders, and failed publication.
- [ ] R4: Add request/batch scratch regions and separate result/message regions;
  reset each request rather than retaining scratch for a long-lived worker task.
- [ ] R5: Transfer sealed owned segments between tasks, with bounded spare segments
  so producers can continue while consumers process earlier segments.
- [ ] R6: Support managed references only after defining outgoing GC roots,
  incoming region ownership, publication barriers, external-memory accounting,
  resource cleanup, and finalizer behavior on abnormal task exit.

### Compiler lifetime analysis and multiple reclamation strategies

- [ ] L1: Add interprocedural escape summaries (returned/stored/captured/sent);
  choose register, stack, region or GC allocation with a conservative fallback.
- [ ] L2: Support stack/region objects containing pointers in the root/tracing
  model before redirecting existing heap-object allocations there.
- [ ] H1: Prototype task-local heaps for proven isolated graphs; ownership follows
  migrating tasks, and task completion cannot invalidate pending join results.
- [ ] H2: Evaluate shared frozen-region ownership/refcounts for immutable batches;
  measure atomic traffic, retention, fanout, and destruction latency.
- [ ] H3: Define cross-tier reference rules and cycle handling. Shared mutable or
  uncertain graphs retain tracing GC; immutable does not imply acyclic.
- [ ] H4: Evaluate hierarchical heaps and selective promotion only after arbitrary
  channel handoff, FFI/pinning, and stable-address requirements are covered.

## Producer/consumer design constraints

The proposed ring arena is a pair-owned allocation domain, not merely a faster
channel queue. Queue slots may be reused after dequeue, but message storage can
only be reused after processing/lease release. Read/write cursors alone are
sufficient only with suitable FIFO processing and lifetime guarantees. Multiple
in-flight borrows need separate reservation, publication, and reclamation state.
A tiny surviving value must not retain an unbounded arena; escaping results need
an owned copy or a separately retained segment. Fanout cannot consume the same
unique lease twice. Bound capacity and apply backpressure instead of growing
without limit. Cancellation and failed sends must return or release ownership.

## Baseline and results ledger

Pending M0/M1. Historical profiling is useful for selecting targets, but is not a
baseline for this branch. Save raw results under `reports/memory-management/` and
record exact commands, executable hashes, revisions, input sizes, repetitions,
worker settings, and limitations with each completed item below.

### M0 — diagnostic/benchmark foundation

Merged existing profiling work at `c7c7e25`; production defaults are unchanged.
Added independently checked builder and message-processing cases and ordinary
baseline/candidate comparison support. Seven benchmark-harness tests pass.

### R0 — initial pair-owned ring protocol

Initial scope is pointer-free payload bytes/records with an explicitly bounded
capacity, one producer and one consumer. General channel semantics remain the
fallback. A prototype alone is not automatic topology detection or language
integration; R1/R2 remain unchecked until their acceptance checks pass.

- Producer reserves aligned storage, writes a complete payload, then publishes
  with release ordering. Consumer observes publication with acquire ordering.
- Allocate storage lazily; do not reserve a large arena for every idle or
  one-shot reply channel. Pool segments with bounded retained capacity and account
  for memory across all active channel domains, not only each pair individually.
- Reservation, publication, delivery and reclamation are distinct states. A
  consumer may suspend while processing; dequeue never advances reclamation.
- One FIFO borrow/lease initially; acknowledgement ends its lifetime and allows
  storage reuse. Compiler escape checks must reject retaining its view in an
  escaping result, closure, task or collection. An explicit owned copy can escape.
- Monotonic cursors and wrap padding avoid confusing empty/full states. Define
  overflow-safe arithmetic. Oversize payloads use a separately bounded owned
  segment/fallback; capacity exhaustion applies backpressure.
- Queue handoff owns pending messages. Failed reserve/publish/send returns the
  reservation; cancellation/close wakes blocked peers and releases undelivered
  reservations without overwriting a live consumer lease.
- Ownership follows tasks across workers. A third endpoint, fanout, selection or
  uncertain aliases cannot silently retain the single-producer/single-consumer
  specialization. Shared immutable fanout needs reference-counted segments or GC.
- No managed pointers, callbacks or native-resource finalizers in the first ring
  payload. Supporting those requires R6 and explicit tracing/cleanup contracts.
- Compare with current prompt/batch channels using equal payloads, processing,
  capacities and correctness oracles. Include slow consumers, wraparound, full
  and empty queues, retained/escaping results, cancellation and 1/2/4/8 workers.

### Saved baseline — coverage

Production source: `c7c7e25fc382f41df1c5b8c8053a12f0f9551abf` (main
`926cdbd` plus the previously committed diagnostic feature, default-disabled).
Harness source: `9218316`. Built with `cargo build --release --offline -p lang_cli
--target-dir /tmp/language-gc-profile-target`; copied before further builds to
`/tmp/language-memory-binaries/baseline`. No production optimization changed.

```
python3 bench/gc/run.py --baseline /tmp/language-memory-binaries/baseline --baseline-revision c7c7e25fc382f41df1c5b8c8053a12f0f9551abf --suite coverage --workers 1 2 4 8 --reps 5 --out reports/memory-management/baseline-coverage
```

320 measured runs plus one warmup per cell; all independent checks passed.
[Raw data](reports/memory-management/baseline-coverage/measurements.json.gz) and
[all ranges/RSS/GC summaries](reports/memory-management/baseline-coverage/summary.md)
include exact executable and source hashes, host and toolchain. The one-worker
medians were: map-new 32.918 ms / 5 collections; map-replace 79.662 ms / 15;
map-strings 38.904 ms / 7; set-builder 124.270 ms / 30. These loops are single-task
workloads; additional runtime workers do not imply parallel builder execution.
Message medians: ordinary 19.911 ms; slow/capacity-one 23.299 ms; retained 20.685 ms;
fanout 28.237 ms. Different cases have different input/lifetime contracts.

### Saved baseline — applications

Same fixed ordinary binary as the coverage baseline. All four telemetry profiles,
131,072 events, workers 1/2/4/8, one warmup and three measured repetitions per
cell (48 measured runs). All independent payload/routing/cache oracles passed.

```
python3 bench/gc/run.py --baseline /tmp/language-memory-binaries/baseline --baseline-revision c7c7e25fc382f41df1c5b8c8053a12f0f9551abf --suite telemetry --events 131072 --workers 1 2 4 8 --reps 3 --out reports/memory-management/baseline-telemetry
```

[Raw application data](reports/memory-management/baseline-telemetry/measurements.json.gz),
[full ranges and memory/GC results](reports/memory-management/baseline-telemetry/summary.md).
Shared-host measurements are directional, not significance or tail-latency claims.

| Profile | Workers | Baseline work ms | Baseline GC ms | Peak RSS MiB |
|---|---:|---:|---:|---:|
| backpressure | 1 | 224.122 | 38.787 | 49.6 |
| backpressure | 4 | 120.059 | 31.446 | 66.4 |
| balanced | 1 | 244.708 | 52.813 | 76.1 |
| balanced | 4 | 129.298 | 39.339 | 86.6 |
| cpu | 1 | 471.002 | 49.607 | 74.9 |
| cpu | 4 | 183.289 | 35.307 | 86.2 |
| fanout | 1 | 675.253 | 119.406 | 81.1 |
| fanout | 4 | 430.845 | 89.400 | 112.8 |

### I1 — implementation awaiting final validation and comparison

Fresh map/set builders now update exclusively owned HAMT nodes in place when
edits fit their original allocation. Growing nodes still allocate; ancestors
whose child address is unchanged require no replacement. No reference counters,
new object headers, ABI changes, or mutable access to finished maps were added.
Original allocation sizes are retained for freeing/size-class correctness;
mutated old nodes receive the generational write barrier.

The existing model test now additionally checks three builder insertion orders,
full hash collisions, 4,096 replacement updates per order without any managed
allocation, and persistent update/remove behavior after freezing. A language
regression covers pointer keys/values, yielded builders, concurrent tasks,
finished snapshots, fresh subsequent builders, merge/from-list/map-values, sets,
and saved MIR. Six optimized stress/verification/chaos configurations (both
collection modes, workers 1/4/8) passed. The dedicated integration suite also passed all 12 debug/optimized
stress configurations and the saved-MIR check; the full workspace run and
benchmark comparison are still pending.

Coverage fixtures now also assert exact FIFO payload words and final builder
values during execution. The original baseline above retains its earlier source
hash; report optimization changes only from new interleaved comparisons where
both binaries run the same strengthened fixture. Application fixtures are unchanged.

Default workspace/all-target Clippy and diagnostic runtime/all-target Clippy both
pass with `-D warnings`; runtime unit/integration tests and formatting pass.

### Investigated candidate — finalizer guard

The source-level serial finalizer helper writes a debug allocation guard even for
objects without a finalizer. Inspection of the ordinary optimized binary found
no `gc::FINALIZING` storage or standalone `gc::finalize` symbol (`nm -a -C`).
The release optimizer appears to eliminate the guard already; no production cost
or improvement is established. Do not spend a performance change on this source
pattern without generated-code or profile evidence. No runtime change was made.

### Validation execution note

The full debug workspace run passed adversarial, docs and fuzz checks, but its
first GC soak was still running after five minutes (the same optimized workload
passes on baseline/candidate in about 10 seconds). That broad debug run was
explicitly stopped and replaced with `cargo test --release --offline --workspace
--target-dir /tmp/language-gc-profile-target -- --test-threads=1`. Do not report the
partial debug run as a complete pass. Dedicated debug runtime/model tests and the
12-case builder debug/optimized stress matrix have passed independently.
