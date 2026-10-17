# Memory management implementation tracker

Work branch: `perf/memory-management-2026-10-17`.
Initial source revision: `926cdbd19e2bcc889d2499d646a8a0ad0eb69b0c`.
Initial work was isolated from main. Commit each completed implementation slice with its
correctness checks, raw benchmark evidence, and results recorded here.

## Measurement and acceptance

- [x] M0: Bring the existing opt-in GC diagnostics and coverage harness onto this branch.
- [x] M1: Add missing immutable-builder and producer/consumer coverage; save a fresh
  fixed-revision baseline before changing production code.
- [x] M2: Compare immutable baseline and candidate executables with shuffled,
  interleaved repetitions, independent output checks, executable/source hashes,
  cleared inherited LANG settings, and raw results retained in the repository.
- [ ] M3: Improve robustness of short multi-worker controls: vary randomized
  orders/seeds, pair related runs, retain baseline-versus-itself calibrations,
  and investigate cross-workload/scheduling sensitivity before attributing small changes.

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

- [x] I1: True transient HAMT insertion for fresh, affine MapBuilder/SetBuilder
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
  - [x] G1a: Add opt-in managed allocation counts/bytes by type and runtime Rust
    call site, pool-size census, first-survival/later-reclamation accounting,
    and benchmark conservation checks. No normal-build instrumentation.
  - [ ] G1b: Add language allocation-site attribution, native allocation/buffer
    churn, task-lifetime/escape observations and workload-boundary snapshots.
    GC survival alone is not escape or uniqueness evidence.
- [ ] G2: Optimize pool refill/reuse and clearing based on initialization contracts;
  evaluate size-class pages, occupancy metadata, and bulk empty-page reclamation.
  - [x] G2a: Add persistent integer-map baselines and diagnostic comparisons of
    256/512/1024-byte pooled cells, with reuse/zeroing/quarantine/trim tests.
  - [x] G2b: Select a production pool limit only after measuring application
    throughput, pauses, peak RSS and retention; retain unsuccessful experiments.
  - [ ] G2c: Bound larger-cell pool reservation/refill; compare targeted classes
    or pages and burst/idle/post-major memory, preserving reuse and quarantine.
    - [x] Evaluate smaller global refill chunks with sparse-survivor coverage;
      retain 2 MiB default after diagnostic throughput regressions.
    - [ ] Evaluate targeted classes/pages without losing large-chunk benefits.
- [ ] G3: Reduce sweep bookkeeping and repeated helper-thread/scratch allocation;
  preserve finalizers, stable addresses, and bounded collection coordination.
  - [x] G3a: Measure worker sweep skew and evaluate bounded native helper reuse;
    retain scoped helpers in ordinary builds after the CPU application regression.
    Reusable helpers remain a diagnostic-only experiment with regression tests.
  - [ ] G3b: Investigate sweep/finalizer scratch reuse and cheaper helper
    publication/completion; require broader wins before changing the default.
- [ ] G4: Prototype adaptive nursery sizing and major cadence using survival,
  external bytes, memory limits and pause goals. Compare throughput/RSS/pauses;
  a larger fixed nursery is not an accepted substitute.
- [ ] G5: Reuse task/TaskCtx/queue metadata and owned buffers where lifetimes are
  explicit. Bound retained pool capacity and account for native resources.
  - [ ] G5a: Embed Buffer/Array metadata in its stable managed cell, eliminating
    a native Box allocation/free per buffer while preserving tracing and the
    generated access layout. Compare native/managed bytes and applications.
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
  completion needs ordered acknowledgements or separate segments. Before arena
  integration, add a pinned-first-message/escaping-result benchmark that reads
  an early payload after the entire stream; a retained rolling tail alone can
  hide premature slot reuse. Add cancellation/close ownership benchmarks. Support task
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

### I1 — implementation and correctness

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
stress configurations and the saved-MIR check. Full release workspace coverage
and the benchmark comparison are now complete; see the results and validation below.

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

### I1 / M2 — measured result, completed

Implementation: `19d35fc3a7593fa1464bd23a2b447dcd52182ec3`. Ordinary immutable
executables and build/revision/hash provenance are in
[binaries.json](reports/memory-management/binaries.json). Baseline binary is
`c7c7e25`, candidate binary is `19d35fc`; the measurement harness was `e0f6378`.
No production changes were made during measurement. The same strengthened
workload source was used for both binaries in every comparison.

```
python3 bench/gc/run.py --baseline /tmp/language-memory-binaries/baseline --candidate /tmp/language-memory-binaries/transient-hamt --baseline-revision c7c7e25fc382f41df1c5b8c8053a12f0f9551abf --candidate-revision 19d35fc3a7593fa1464bd23a2b447dcd52182ec3 --suite coverage --workers 1 2 4 8 --reps 5 --out reports/memory-management/I1-coverage
python3 bench/gc/run.py --baseline /tmp/language-memory-binaries/baseline --candidate /tmp/language-memory-binaries/transient-hamt --baseline-revision c7c7e25fc382f41df1c5b8c8053a12f0f9551abf --candidate-revision 19d35fc3a7593fa1464bd23a2b447dcd52182ec3 --suite telemetry --events 131072 --workers 1 2 4 8 --reps 5 --out reports/memory-management/I1-telemetry
```

640 coverage and 160 application measurements, plus one warmup per cell. Every
run passed its independent oracle. All medians below are from these interleaved
runs, not from the earlier baseline-only session. Negative time change is better.

| Builder case, one worker | Before ms | After ms | Time change | Collections before / after | Peak process RSS MiB before / after |
|---|---:|---:|---:|---:|---:|
| map-new | 31.702 | 11.468 | -63.8% | 5 / 1 | 41.8 / 41.4 |
| map-replace | 71.581 | 14.959 | -79.1% | 15 / 0 | 39.5 / 28.6 |
| map-strings | 37.300 | 9.542 | -74.4% | 7 / 0 | 38.7 / 37.7 |
| set-builder | 118.769 | 16.032 | -86.5% | 30 / 0 | 40.1 / 28.7 |

Across worker settings, new-key map time fell 63.8–69.0%, replacement-heavy map
78.6–79.4%, string-valued map 70.8–74.7%, and set builder 86.5–87.8%. These are
single-task builders running with different configured runtime worker counts,
not parallel-builder scaling. Peak RSS includes frontend/JIT/output and can hide
managed-heap savings behind other process peaks. No total allocation-volume claim
is made from heap snapshots; the model independently proves zero managed
allocations for warmed replacement-only trie updates.

[Coverage ranges, GC and RSS](reports/memory-management/I1-coverage/summary.md),
[raw coverage](reports/memory-management/I1-coverage/measurements.json.gz).

**Application result: no consistent broad improvement established.** Across four
profiles and workers 1/2/4/8, elapsed changes range from -2.3% to +3.5%; collection
counts remain 12 per run for balanced/backpressure/CPU and 22 for fanout. These
workloads primarily update ordinary persistent maps and allocate messages/tasks,
which this builder change does not eliminate. End-to-end p50/p99 histogram bucket
upper bounds are retained (40,960 samples per cell); changes are mixed and do not
establish a general tail-latency improvement. No p99.9 claim is made.

[Application results and sampled latency](reports/memory-management/I1-telemetry/summary.md),
[raw applications](reports/memory-management/I1-telemetry/measurements.json.gz).

Two eight-worker controls initially appeared slower: snapshots +14.7%, yielding
capacity-one messages +16.8%. Retain those observations. A 15-repetition follow-up
found snapshots +0.7% and slow messages +6.6%. A separate 25-repetition slow-message
comparison found +0.8% (41.109 vs 41.429 ms, overlapping ranges); a 25-repetition
baseline-versus-itself calibration found effectively 0% (40.654 vs 40.647 ms).
There is no stable material slowdown established by these checks, but short
multi-worker cases show substantial run/workload sensitivity. Keep M3 and these
controls in subsequent comparisons; do not dismiss every small change as noise.

Follow-ups used `--suite coverage --workers 8 --cases snapshots messages-slow
--reps 15`, then `--cases messages-slow --reps 25`; calibration passed the baseline
path/revision for both binary roles. Exact arguments and hashes are in each raw file.

- [15-repeat control follow-up](reports/memory-management/I1-controls-followup/summary.md)
- [25-repeat slow-consumer comparison](reports/memory-management/I1-slow-consumer-paired/summary.md)
- [25-repeat baseline-versus-itself calibration](reports/memory-management/I1-slow-consumer-AA/summary.md)

All workspace targets are covered by successful release checks (146 distinct
passing tests, one ignored native-library doctest), with focused debug GC/stress
coverage and strict lint checks. The initial MCP port-permission failure and the
incomplete broad debug run are explicitly retained in the
[validation record](reports/memory-management/validation/README.md).

Next implementation priority: G1 allocation/survival attribution to guide G2/G3
and the R1 ring prototype. R1–R6, lifetime-based placement, and multiple-collector
architecture are still unimplemented; R0 is a design/protocol checkpoint only.

### Integration with Badger rename

The user authorized merging completed work to main and continuing on this branch.
Committed main updates through `4bb3386` were integrated, including the Badger
rename. Runtime paths now use `crates/badger_runtime`, new sources use `.bdg`,
saved MIR uses `.bdgmir`, and current diagnostic controls use `BADGER_*`.
Historical performance reports and their original source/binary hashes remain
unchanged. Current commands are in `bench/gc/README.md`.

The rename was rehearsed separately while it was uncommitted. Main committed it
before publication; a state guard caught that update before any writes to main.
The final merge therefore uses the committed rename directly. Resolutions retain
both the sampling-scope guard and the new Badger names, relocate our diagnostics
module/tests, and update the new benchmark runner/source extensions.

Focused release HAMT/saved-MIR stress tests and diagnostic-feature runtime model
tests passed in the renamed rehearsal; benchmark-harness tests (7) and formatting
also passed. Final merged-tree GC retention, scalar enums, sequence fusion and
transient-map focused checks passed; logs are in `reports/memory-management/integration/`.
G1 attribution remains the next implementation slice.

### G1a: managed allocation and survival census

The completed I1 branch was merged to main at `0d34cee`, after integrating the
Badger rename and editor support. All 69 uncommitted files were SHA-256 checked
before/after the merge and preserved. Further work continues in the isolated
`perf/memory-management-2026-10-17` branch.

`gc-profile` now supports `BADGER_ALLOC_PROFILE` independently of phase profiling.
The census follows the collector's per-worker object-list compaction with side
metadata; it leaves object headers and ordinary builds unchanged. A separate
`allocation_census` phase accounts for the diagnostic scan before sweeping.
Totals distinguish requested managed bytes from allocated cell bytes, early
reclamation, first survival and later reclamation. Benchmark diagnostics also
record small-pool allocation classes. Remaining objects are unswept, not a
reachability census; native/immortal allocations and true escape classification
remain G1b. Profiling overhead is measured separately and is not an optimization.

The first census exposed a coverage confound: the existing U64 message workloads
perform checked numeric conversions in payload generation and every-word oracle
validation. Boxed `Result` values make up over 99% of their managed requested
bytes; this does not include native payload buffers. Added four `messages-int*`
controls with the same 64-bit payload width, topology and checksum, retaining
the original cases to measure conversion costs. This is additional coverage,
not a language/runtime performance change. The raw reports pin each workload
source hash; the earlier coverage report uses the original workload revision.

G1a measurements and validation are saved in
[`reports/memory-management/G1-summary.md`](reports/memory-management/G1-summary.md).
The fixed ordinary baseline is main `0d34cee`; immutable diagnostic compilers
are pinned to `82e409e` / `a015467` in `G1-binaries.json`. All matrices use a
warmup plus three measured repetitions with workers 1/2/4/8:

| Matrix | Measured runs | Evidence |
|---|---:|---|
| Original allocation/survival coverage | 384 | `G1-coverage/` |
| Original/Int matched message controls | 384 | `G1-message-controls/` |
| Application attribution, 65,536 events/profile | 192 | `G1-telemetry/` |
| Sustained Int messages, 262,144 messages/case | 192 | `G1-sustained/` |

All 1,152 runs pass independent oracles; census variants also pass object/byte
conservation across sites, size classes and reclamation. Runtime unit tests,
CLI retention/builder suites, strict diagnostic Clippy, default runtime checking,
formatting, eight Python runner tests and census stress/verification (both
collection modes, all four worker counts) pass. Logs are under `G1-validation/`.

Key findings: matched Int message cases allocate 0.38–0.46 MiB managed requested
bytes at one worker, versus 48.38–80.42 MiB for the original U64 conversion-heavy
cases. This is a coverage comparison, not a compiler speedup. Small Int cases
perform no collections; sustained versions perform five in every ordinary cell.
External pressure remains substantial (74.31 MiB accumulated at five collection
boundaries in one phase-only FIFO run, versus 5.89 MiB managed pressure).

Application managed allocations are dominated by `MapNode` (61.9–66.5% of bytes
across the one-worker profiles); the ordinary persistent path-copy helper is the
main site. Ordinary GC pause/elapsed ratios range about 9–28% across the worker
matrix. G1a confirms G3 and I5 as strong follow-ups, while G1b native-buffer
accounting is needed before making arena claims. G2/G3/I5 and all arena
implementation boxes remain unchecked. No default collector policy or production
representation changed in this measurement slice.

### G2a: pool boundary experiments

Initial source inspection after G1a found that dense HAMT root nodes occupy
288 bytes including the header, beyond the 256-byte pooled-cell limit.
G1a phase profiles attribute about 64–94% of GC time to sweep/finalization in
the application cells inspected. Larger pool classes are a candidate, not yet
an accepted production change. Persistent integer-map cases (64/256/4096 keys)
isolate ordinary path copying without string/conversion churn.

The before-change baseline is saved under `reports/memory-management/G2-baseline/`:
ordinary binary `main-0d34cee`, benchmark revision `55d0a43`, workers 1/2/4/8,
five measured repetitions and one warmup. All 60 measured checksums pass.
Diagnostic builds can compare fixed 256/512/1024-byte pool limits in the same
binary. The initial normal baseline uses 256 bytes; a 512-byte normal-build
candidate is under evaluation. Unit tests isolate their global heap
in child processes and cover every payload size through 1,040 bytes, cleared
reuse, crossing chunk boundaries, a retained quarantined cell, and purging all
links before chunk retirement (with verification both enabled and disabled).

### G2 integration checkpoint: latest main before remeasurement

Merged committed main `09580597ddb62a2db732ec20b70325abacbf5cea` at
`8596e6d23b3774e71111d275edfa1ab77700d2b5` before further benchmarking.
Main's unrelated uncommitted changes remain outside this worktree. Earlier
unpublished G2 matrices are archived under `reports/memory-management/G2-pre-main/`;
they are historical evidence, not acceptance results for the merged tree.

The matched baseline `e8f91e0a1b42d35816e9154023b016f9e7cfe572` differs from
the merged candidate only in the pooled-cell limit (256 versus 512 bytes).
Both ordinary compilers were rebuilt and copied to immutable paths. A shared
Cargo target initially reused the baseline artifact; that copy was rejected
before timing, runtime artifacts were cleaned and the candidate rebuilt.
`bench/gc/check_pool_limit.py` verifies actual allocator behavior as well as
distinct executable hashes: identical retained data reserves 0.25 MiB versus
6.25 MiB of pools, with no intervening collection. Fresh performance comparisons
and the production-policy decision remain pending.

Merged-tree validation passes: 7 ordinary runtime tests, 9 diagnostic runtime
tests, 67 CLI/program/retention/builder tests, 9 Python harness tests, strict
diagnostic runtime Clippy and formatting. Build identities and logs are saved in
`G2-main-binaries.json` and `G2-main-validation/` beneath the report directory.

### G2b: measured production pool choice

Keep the 512-byte ordinary pool cutoff: frequent 288-byte HAMT roots now reuse
worker-local cells. Object/header layouts, GC thresholds, marking, finalizer
order and chunk sizes are unchanged. This lowers allocation/reclamation cost;
it does not reduce managed object counts or collection frequency.

Saved 1,214 fresh, ordinary-build measurements on the matched merged-main pair:
180 repeated controls, 224 application runs, 640 broader coverage runs, 120
application follow-ups and 50 message follow-ups. Every independent oracle
passes. [G2 results](reports/memory-management/G2-summary.md) links all raw data,
ranges, hashes, latency histograms, pool reservations and validation logs.

- Persistent-map elapsed improves 1.6–27.7% in short cases, 1.8–10.5% in longer
  cases including major collections; snapshot cases improve 5.8–12.6%.
- Balanced application elapsed improves 5.3–13.6% across 1/2/4/8 workers.
  Backpressure improves 3.0–5.5% at 1/2/4 workers, with +2.1% at 8.
- CPU/fanout regressions in the initial application matrix do not reproduce in
  a separate 15-repeat follow-up: CPU is +0.4%/−0.1%, fanout −0.1%/−7.2% at
  1/2 workers. CPU GC time still falls 8.3–12.6%, without a throughput gain.
- Application peak RSS rises 1.7–16.4 MiB. Long integer maps reserve 10–12 MiB
  more pools at one worker. This is an accepted throughput/GC-cost tradeoff,
  not a memory-saving change or a universal tail-latency improvement.
- An initial +7.0% four-worker message result becomes −2.2% in 25 follow-up
  repetitions, with RSS +0.3%. The one-worker 15-repeat controls are effectively
  flat; keep M3 open for workload/order sensitivity.

G2 remains open for bounded reserves and page/refill/reclamation improvements.
Next GC priorities are G3 sweep overhead and G1b native-buffer attribution;
I5 and region/arena protocols target allocation reduction rather than cell reuse.

The new automated boundary-handoff regression passes in 16 combinations (both
collection modes, 1/2/4/8 workers, optimized/unoptimized language code) with GC
stress, verification and chaos. Together with the earlier merged-tree checks,
68 distinct CLI/program/retention/builder tests pass. Formatting remains clean.

### G2 publication and next slice

At the user's request, integrated newer main `35bf751` (including `c3d4bd9`)
without conflicts at `b00ea24` before publishing G1a/G2 back to main. The combined
tree passes 7 ordinary and 9 diagnostic runtime tests, 68 CLI/retention/builder
tests and 7 driver tests. Logs are in `G2-latest-main-integration/` beneath the
report directory. Existing G2 performance claims remain pinned to the earlier
matched `0958059` pair; new experiments start from the integrated revision.

Next slice: profile the distribution of swept work and native helper-thread
creation before selecting a G3 scheduling/bookkeeping change. Preserve the
current stop-world and finalizer-order contracts.

### G3 measurement checkpoint

Published G1a/G2 to main at `5d25647` with all 236 current dirty/untracked paths
and the complete staged diff preserved. New work continues on this branch.
The ordinary G3 application baseline at that revision is saved in `G3-baseline/`:
48 oracle-checked runs, three repetitions plus a warmup, 262,144 events/profile,
all four profiles and 1/2/4/8 workers. No G3 production policy has changed yet.

Diagnostic builds now record per-worker swept-object counts and actual helper
job sizes/launches. The runner checks count conservation; the analyzer reports
small helper jobs and skew, filtered to workload collections. These diagnostic
counts are evidence for selecting a sweep policy, not thread-startup timing.

Fresh phase-only application profiles (`G3-sweep-telemetry/`, 144 measured runs
including ordinary/diagnostic-disabled controls) show only 6 of 825 helper jobs
below 4,096 objects. Most parallel work is balanced. The next candidate therefore
reuses native helpers rather than suppressing useful parallelism.

The diagnostic-only `BADGER_GC_SWEEP_REUSE=0|1` experiment compares scoped and
cached helpers in one binary. Cache slots are bounded by `MAX_WORKERS`, created
lazily, parked on capacity-one channels, and receive at most one outstanding
job each. Every result is received before finalizers run in worker/list order
on the collector thread. Profiler/process finish closes and joins the cache;
late collections fall back to scoped helpers without reopening it. Ordinary
builds still select scoped helpers until measurements accept a default change.

An isolated native test covers sparse worker indices, old prefixes, sticky and
cleared marks, exact collector-thread finalizer order, reuse, joining, repeated
shutdown and scoped fallback. Default/diagnostic runtime and harness tests pass.

The five-repeat diagnostic policy comparison (`G3-helper-policies/`, 160 runs)
reduces eight-worker native helper creation from 154–287 to 7 per
process. GC medians improve about 3–9% there, while elapsed results remain mixed.
This selects a candidate, not an ordinary-build performance claim. Integrated
main's new archive cleanup `36091e7` at `b3602ac` before the ordinary comparison;
a separate clean main checkout supplies its fixed baseline. The normal-build
cached-helper candidate was selected for ordinary integration/performance tests;
the final policy decision is recorded below.

Added two fixed-eight-task coverage cases, `parallel-maps` and
`parallel-snapshots`, since existing collection cases run on one task. Parallel
snapshots additionally check every retained key/string against its capture step
after churn; old root lengths alone cannot detect lost descendants. A fresh
24-run baseline with this stronger validation is saved in `G3-parallel-baseline/`.
The earlier shallow exploratory run is excluded from accepted comparisons.

The ordinary candidate passes 8 default/10 diagnostic runtime tests, strict
Clippy, 68 CLI/retention/builder tests, 11 harness tests and formatting. A procfs
behavior probe checks independent final maps and GC activity, observes 0 versus
3 cached helpers, and rejects diagnostic-feature binaries. Baseline/candidate
runtime artifacts were explicitly cleaned/rebuilt across source checkouts.

### G3a decision: retain scoped helpers; keep reuse diagnostic-only

Saved 1,440 oracle-checked measurements: the 48-run initial application baseline,
144-run work census/build controls, 160-run diagnostic policies, 24-run stronger
parallel baseline, 224-run ordinary application matrix, 720-run ordinary coverage
matrix, and 120-run application follow-up. Full evidence and interpretation are
in [G3 results](reports/memory-management/G3-summary.md).

The cached ordinary candidate (`564b492`, baseline main `36091e7`) improves
eight-worker parallel maps by 12.1% elapsed / 32.5% GC pause time, with RSS
+5.8 MiB. Application gains are mixed. A separate 15-repeat CPU/fanout follow-up
reproduces CPU/8 **+4.1% elapsed** despite GC pause −2.6%; fanout/8 becomes −3.5%
elapsed, GC −6.5%. Serial controls also vary despite having no helpers, so small
differences cannot all be attributed to reuse. No broad tail/RSS win is accepted.

Do not ship this prototype as the ordinary default. The scoped sweep body is
restored verbatim from main, with a saved source-body hash. Cached helper state,
threads and shutdown hooks compile only for `gc-profile` or unit tests; the
diagnostic default is also scoped. Existing G2's 512-byte pool choice is retained.
The rejected ordinary candidate remains revision-pinned in history and its raw
results are preserved, rather than being relabeled as the final default.

G3a is checked as a completed evaluation, while production G3 remains open.
Next work should measure cheaper batch helper publication/completion and bounded
sweep scratch reuse (G3b), alongside native-buffer attribution (G1b) and bounded
pool reservations (G2c). All region/arena implementations remain unimplemented.

### Next slice — G2c bounded pool refills

Integrated committed main `56f5983` at `1b47a02` before building the fresh
ordinary baseline. Main's unrelated uncommitted changes remain outside this
worktree. Prioritize refill reservation/retention because G2 improved throughput
but raised application RSS, and each post-first worker chunk currently reserves
2 MiB. New sparse-survivor burst checkpoints cover a missing memory lifetime.
Next compare diagnostic refill sizes, then an ordinary candidate only if the
measurements support one. Native-buffer attribution and allocation elimination
follow; arena integration still requires the ownership/escape groundwork.

### G2c evaluation — keep 2 MiB refills; targeted pages remain open

Fresh ordinary baselines saved before production changes: 24 sparse-burst and
32 application runs. The diagnostic selection compares 256/512/2048 KiB refills
in 162 oracle-checked measurements at 1/8 workers, with three repetitions. All
54 full-size burst policy runs include a verified post-burst major collection.
Sixteen scattered survivors leave 34.25/24.25/20.25 MiB reserved for
2048/512/256 KiB refills, but allocation-heavy cases and most application cells
slow down. Diagnostic perturbation/shared-host variation applies; no ordinary
smaller-refill candidate is selected. Preserve the 2 MiB default and its
huge-page support; targeted classes/pages remain open. See
[G2c evaluation](reports/memory-management/G2c-summary.md) for all ranges,
RSS tradeoffs, major-collection checks, raw data and exact provenance.

Next priority G5a: remove the separate native Buffer/Array metadata Box by
embedding it in the already stable managed cell, preserving generated access
offsets and precise tracing. This attacks allocation churn directly.

### G5a baseline — Buffer metadata allocation

Before changing Buffer production code, saved 88 ordinary coverage measurements
(11 cases, 1/2/4/8 workers, two repetitions) and six new-case smoke measurements.
The immutable baseline remains `1b47a02` (main `56f5983` plus restored-default G3
diagnostics); Buffer code is unchanged by G2c. Added empty, growing and
pointer-valued buffer cases with independent checksums and full retained-tail
validation. Existing G2c's 32-run telemetry baseline uses the same executable
and unchanged application workload; final comparison will run both binaries
interleaved on the current harness. Raw data: `G5a-coverage-baseline/`.

### G5a implementation and validation in progress

Candidate `75f9c98` embeds BufData in the Buffer's stable managed cell. The
payload's first word still points to BufData, preserving existing generated
pointer/length access offsets and packed element storage. There is one fewer
native metadata allocation/free per buffer. The metadata bytes move from
external accounting into the managed payload; managed object counts do not
decrease. Finalization drops element storage in place instead of freeing a Box.

The isolated test checks zero native metadata charge, Vec adoption, storage
growth, promoted-buffer/young-string barriers, generated offsets, and eventual
external-storage reclamation in both GC modes with quarantined cells. Nine
ordinary runtime tests, strict runtime Clippy, 66 CLI/program tests and 24
source/binary-MIR/multiworker GC stress checks pass. The first sandboxed CLI run
could not use localhost listeners/PTY/debugging; its complete rerun with local
permissions passes. Performance acceptance and diagnostic validation remain
pending; do not mark G5a complete from the native allocation reduction alone.

### G5a refinement — direct managed metadata / MIR format 6

The first candidate's 160 application and 440 coverage comparisons are retained
in `G5a-applications/` and `G5a-coverage/`. Application timing generally improves,
but multiworker RSS and some timing cells need further evaluation. Before
acceptance, remove the redundant self-pointer from the first layout: BufData
now occupies the payload directly, reducing a 64-bit Buffer cell from 80 to
64 bytes and removing the MIR metadata-pointer load. Saved MIR's toolchain
format advances from 5 to 6 because old field loads would otherwise interpret
the new layout incorrectly. Old files must fail with a recompile diagnostic.
This is a refinement under evaluation; no arena or ownership inference is added.
