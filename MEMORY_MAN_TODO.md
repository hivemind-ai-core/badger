# Memory management implementation tracker

Work branch: `perf/memory-management-2026-10-17`.
Current follow-up branch: `perf/gc-reclaim-2026-10-18` (same isolated worktree).
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
  - [x] M3a: Retain per-process warm-kernel block means alongside medians,
    with tail-reversal checks, long-window repeats and a reusable raw-data analyzer.

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
  - [x] I2a: Elide proven local Map/List lookup Option boxes using split results;
    retain boxed escaping/stored results, typed roots and live-reload behavior.
  - [x] I2b: Elide immediate Map.entries() method lists/tuples via the existing
    cursor; retain stored lists, escaping tuples, order and typed pointer roots.
  - [x] I2c: Remove internal entry lists/tuples from std.map keys/values,
    map_values/filter/merge_with while retaining observable public results.
    Projection time falls 62–68% and serial composition 21–22%; same-native
    counts prove 524,288/961,024 tuple allocations removed in those cases.
    [Results, broader controls and memory limits](reports/memory-management/I2c-summary.md).
- [ ] I3: Extend sequence fusion through stored/composed pipelines where semantics
  permit; benchmark visible, stored, collected, and eager forms separately.
  - [x] I3a: Fuse immutable nonescaping stored range/list descriptions consumed
    by count/fold/for, preserving construction snapshots and reusable traversal.
    Stored scalar kernels improve 96–97%; scalar/pointer lists 75–76%/45–56%.
    Keep executable-level byte-control regressions and mixed applications visible.
    [Allocation proof, profiles and limits](reports/memory-management/I3a-summary.md).
- [ ] I4: Pack fixed-size pointer-free records directly into collection storage,
  then evaluate pointer-bearing layouts with precise tracing.
- [ ] I5: Reuse consumed immutable structures when uniqueness is proven, including
  descendant ownership; retain copying when versions share nodes.
  - [x] I5a: Reuse private, fresh-origin scalar-map updates only when all old
    aliases die and no earlier version/cursor escapes before later mutations.
  - [x] I5b: Reuse nodes between observable scalar-map snapshots with frozen
    wrappers and unique ownership epochs (49f281c). Preserve cheaper private
    construction; record mixed controls. [Proof](reports/memory-management/I5b-proof-plan.md),
    [implementation and measurements](reports/memory-management/I5b-summary.md).
  - [ ] I5c: Evaluate epochs for maps entering through parameters/call boundaries
    or an affine edit-from-map API. Prove caller/producer aliases and publication
    before relaxing I5b's fresh-origin restriction; save new batch-update baselines.
- [ ] I6: Evaluate compact small-map, singleton-collection, and short-string forms
  against observed size distributions and object-size inflation.
- [ ] I7: Borrow parsing/string/byte views where lifetime permits; copy small
  escaping views when necessary to avoid retaining large source buffers.
  - [x] I7a/C1: Borrow rooted codec input; remove successful-path error strings,
    cloned wire-name lists and temporary encoder quoting/integer buffers.
    JSON improves about 39–40%; managed cell traffic stays unchanged, RSS does
    not improve. [Results and limits](reports/memory-management/C1-summary.md).
  - [ ] I7b/C2: Evaluate eliminating the owned intermediate JSON tree with
    schema-directed/borrowed parsing. Preserve validation/error precedence,
    aliases, unknown fields, depth limits, rooting and saved-MIR behavior.

### Allocator, collector, and runtime work

- [ ] G1: Measure allocation counts/bytes by source/type and survival/escape class;
  distinguish native allocations from managed allocations and live snapshots.
  - [x] G1a: Add opt-in managed allocation counts/bytes by type and runtime Rust
    call site, pool-size census, first-survival/later-reclamation accounting,
    and benchmark conservation checks. No normal-build instrumentation.
  - [ ] G1b: Add language allocation-site attribution, native allocation/buffer
    churn, task-lifetime/escape observations and workload-boundary snapshots.
    GC survival alone is not escape or uniqueness evidence.
    - [x] Add scoped Items native allocation/growth/adoption/free and inline
      placement/growth counters with ownership-conservation validation (G5b).
    - [ ] Complete language sites, other native allocations and task escape attribution.
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
  - [x] G5a: Embed Buffer/Array metadata in its stable managed cell, eliminating
    a native Box allocation/free per buffer. Preserve tracing and version the
    generated access ABI when needed; compare native/managed bytes and applications.
  - [x] G5b: Inline small fixed-capacity element storage; eliminate native
    payload allocation and frozen-buffer finalizer dispatch where no native
    resources remain. Prove grow-to-native, packed tracing, budgets and handoff.
- [ ] G6: Shorten unnecessary shadow-root retention, including pointer provenance
  through word-based runtime calls, with stress coverage for hidden aliases.
  - [x] G6a: Stop tracing entry closures/arguments after terminal task completion;
    preserve results/cleanup aliases and avoid stale-entry inspection.
  - [ ] G6b: Investigate unnecessary runtime delivery/auxiliary and compiler
    shadow roots with lifetime/provenance proof and broader coverage.

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

### G5a complete — direct Buffer metadata retained

Final production candidate `b8b6935` uses a 64-byte managed cell (baseline
32-byte cell plus 48-byte native Box); the intermediate 80-byte cell with an
extra compatibility pointer is superseded. Managed Buffer counts are unchanged;
one native metadata allocation/free per Buffer and one array metadata-pointer
load are removed. Saved MIR moves to format 6 and format-5 files fail with a
recompile diagnostic.

Final accepted measurements: 224 ordinary application runs, 264 coverage runs,
80 identical-binary controls, 144 isolated adjacent-pair follow-ups and 64
separate diagnostic/census controls. All independent checks pass. Initial
intermediate results remain revision-pinned; one accidentally overlapping
repeat/census attempt is explicitly excluded and rerun in isolated directories.
The GC runner now prevents concurrent invocations and supports randomized
adjacent pairs, different shuffle seeds, and per-run host-pressure context.
M3 remains open for broader host/scheduling sensitivity.

Nine-pair application follow-up at four workers: backpressure −7.8%, balanced
−11.0%, CPU −6.3%, fanout −5.2%; corresponding GC pause medians improve about
15–23%. CPU/8's −1.6% elapsed is within control variation. Memory is mixed
(e.g. balanced/8 +3.4 MiB in the repeat). This is not a universal memory or
tail-latency win. One-worker empty-buffer coverage improves 48.2%, growing
buffers 15.1%, sustained integer messages 11.6%; persistent-map control 1.5%.

Validation passes: 9 ordinary/11 diagnostic runtime tests, 3 MIR tests, 66
CLI/program tests, 24 dedicated source/binary-MIR GC stress checks, strict
runtime/MIR Clippy in both modes, scoped formatting and 15 harness tests.
[Full G5a decision, ranges, accounting and evidence](reports/memory-management/G5a-summary.md).

Next priority is G5b: small fixed-capacity payloads (especially 256-byte message
arrays) still allocate native element storage and later dispatch finalizers.
Embedding that storage under existing GC ownership is a smaller next step than
arenas and can reduce native churn without requiring escape/topology proofs.

### G5b baseline — small fixed-capacity element storage

Before changing production storage, saved 12 smoke, 68 coverage and 32
application measurements from immutable direct-metadata binary `b8b6935`. New
cases cover 31/32/33-word capacities (248/256/264 bytes), small pointer arrays,
preallocated buffers that outgrow their embedded capacity, and packed bytes
just above the proposed limit. All checks pass. Existing format-6 layout and
MIR field offsets will remain compatible; inline state fits native metadata
padding. Native growth must copy the initialized prefix, charge its entire new
capacity (old inline bytes remain managed), and retain ordinary finalization.
Only frozen storage that owns no native allocation may skip finalizer dispatch.

### G5b complete — bounded inline payload storage retained

Ordinary candidate `a0a9838` embeds positive capacities up to 256 bytes after
managed Buffer metadata and removes finalization for frozen inline storage.
Existing precise tracing, write barriers and shared ownership remain; format-6
MIR offsets stay compatible. Growth copies the initialized prefix to native
storage and charges its full capacity; original inline bytes remain managed.

Saved 12 smoke/68 coverage/32 application baseline runs before implementation.
Final ordinary evidence: 96 application, 408 coverage, 216 paired follow-up,
80 same-binary control and 84 CPU confirmation runs. Separately saved 170
diagnostic policy/census runs. Every oracle passes; all 1,166 saved measured
intervals are nonoverlapping. Native ledger proves 262,144 payload allocations
avoided in sustained integer messages while managed object count is unchanged.

Nine-pair follow-up at 4/8 workers: backpressure −12.1%/−9.6%, balanced
−6.0%/−10.3%. Exact 32-word arrays improve 23.8%/26.2%, sustained integer
messages 12.5%/14.4%. RSS often rises a few MiB. CPU/8 varies: first flat,
follow-up +5.3%, new 21-pair confirmation −4.7%; no stable CPU/8 or general
tail-latency win is established. Smaller global pool routing reduces memory
but sacrifices storage speed, so keep the default pool policy. Targeted pages
remain open. Full tradeoffs, paired ranges, counts and provenance:
[G5b results](reports/memory-management/G5b-summary.md).

Validation: 10 ordinary/12 diagnostic runtime tests, all 66 program tests,
42 source/MIR GC stress checks, 15 harness tests, strict Clippy in both modes
and scoped formatting. G1b now includes Items-owned native capacity events,
not complete malloc interception, language sites or escape inference.

Next priority G6a: measure unnecessary completed-task entry/argument roots,
with preserved result aliases and cleanup lifetimes. Clear only at synchronized
completion points; do not mutate roots after lock-free execution permits GC.

### G6a baseline — completed tasks retain consumed input graphs

Added scoped completed-input and returned-alias controls. The scope and child
records stay open through 524,288 persistent-map updates; full payload checks
run before churn, and returned aliases are joined/checked again afterward.
Before any production change, saved four smoke and 24 full-size measurements
at 1/2/4/8 workers from the immutable G5b ordinary binary (`a0a9838`). Both
cases retain 16 MiB native element storage through churn and idle in all runs.

Use the existing acquire/release terminal `Done` state to stop tracing only
entry closures/arguments after execution and cleanup finish. Keep task results
and all other roots unchanged. Inspection must avoid following completed entry
pointers after GC can reclaim them. This avoids introducing concurrent writes
to currently immutable entry/argument fields, which inspection reads while
other workers run. It retains the small native argument vector until record
release; it does not recycle task records or coroutine stacks.

### G6a complete — consumed completed-task inputs can be reclaimed

Production `82642bb` uses the existing acquire/release terminal Done state to
omit only entry-closure/argument roots. Results and all other roots remain;
fields stay immutable, and inspection avoids following completed stale entries.
No new GC collection or scheduling policy, arena, uniqueness inference or task
record reuse is added. Native argument-vector capacity remains until removal.

Saved four smoke/24 full-size baseline runs first. Final paired coverage has
112 runs: consumed native payload storage falls exactly 16 MiB→0 after churn
and idle in all 1/2/4/8-worker cases/repetitions. Returned-alias controls retain
all useful payloads and pass full checksums again after collections. The 224-run
application comparison ranges −3.3% to +3.6% work time, with noisy small mixed
RSS/pause changes; no broad speedup is established.

Memory improves at a measured cost: the short consumed-input benchmark changes
+5.7%/+7.1%/+0.3%/+12.2% elapsed at 1/2/4/8 workers and GC pause medians rise.
One-worker peak RSS falls about 7.1 MiB; multiworker peak changes are smaller.
Accept earlier reclaimability without claiming equal RSS return or faster GC.
All 364 saved measured intervals are nonoverlapping. Full tradeoffs and exact
checkpoint/binary proof: [G6a results](reports/memory-management/G6a-summary.md).

Validation: all 66 program tests, 10 ordinary/12 diagnostic runtime tests,
88 dedicated source/MIR lifecycle and inspection stress checks with actual
freeing and quarantine, 16 harness tests, strict Clippy in both modes and scoped
formatting pass. Cancellation/failure cleanups and returned closure aliases
retain their pointer graphs safely.

Next priority I2: eliminate lookup-result boxing across proven local intrinsic
boundaries. The fresh application census still has about 0.76M MapNode/0.38M
Option allocations at one worker; channel split-results provide a mechanism
to evaluate for lookup calls. Immutable-node/record allocation and targeted
pool/finalizer work follow; architectural regions/rings remain unimplemented.

### Main integration and I2a baseline

Completed memory work `adf29aa` and committed main `40291dc` are integrated at
`fab8a02`, now on local main as requested. Its parser/symbol/driver changes
build and pass 10 runtime, 3 MIR and all 66 program tests, 88 task-root and
42 buffer/MIR stress checks. All 87 unrelated dirty files and the index were
preserved exactly while advancing main. No remote is configured. Validation:
[main integration](reports/memory-management/main-integration-2026-10-17/README.md).

I2a starts from a fresh immutable ordinary/diagnostic pair built from `fab8a02`.
Before changing production code, saved 10 smoke, 60 mixed-lookup coverage,
32 application and 10 separate diagnostic census runs. New map/list immediate
lookups, stored-Option control and array control include misses/negative indices,
independent arithmetic and full pointer-string comparisons without per-read
expected-value allocation. The full-size one-worker census allocates 524,289
Options in map/list/stored cases, but only one in array control: existing MIR
lowering already eliminates local array result boxes. Focus this slice on Map
and List intrinsic boundaries; do not claim a newly discovered array win.

### I2a complete — local Map/List lookup Option elimination

Keep production `fd333a5`: optimized nonescaping Map/List lookup results use a
split tag/payload protocol with immediate auxiliary-slot clearing and typed
roots. Escaping/stored/compared/reassigned results retain the boxed ABI; live
reload remains unchanged. Backward traversal fixes insertion offsets for
multiple eligible calls within a block. Standard Option tags preserve zero,
dynamic null, nested Options and packed values; MIR format remains 6.

Baselines were saved before implementation. There are now 1,630 measured runs
across 20 datasets, with no overlapping measured intervals and no outlier
removal. Seven-pair final lookup coverage at 1/2/4/8 workers improves mixed
Map/List work about 20–37%, with 39–44% lower process peak RSS. Diagnostic
family counts prove 524,288 Option objects / exactly 16 MiB allocation traffic
removed per mixed case; array/stored/miss controls remove zero. All-miss list
coverage caught the initial +35% regression; the refined fast miss path removes
it, though final one-worker misses still measure +3.0% (about 0.2 ns/lookup).

The final application matrix is mostly faster with noisy contrary cells. A
separate 15-pair follow-up at 4/8 workers gives balanced -8.1%/-6.8% and fanout
-5.9%/-5.2% group-median elapsed time; fanout paired medians are only -1.9%/
-3.2%. Application diagnostic census removes about 13.43 MiB balanced and
27.40 MiB fanout Option cell traffic, leaving 73 setup/escaping objects.
RSS and sampled p99 remain mixed; no universal memory or latency win is claimed.

Validation: 10 ordinary/12 diagnostic runtime tests, six MIR tests, all 66
program tests, 88 focused source/saved-MIR GC stress checks plus two MIR
probes, 17 harness tests, strict all-target runtime/MIR/codegen Clippy in both
modes and scoped formatting pass. Actual baseline format-6 programs still run.
I2 remains open for broader scalar replacement; architectural arenas/rings and
other unchecked items remain unimplemented. Full results, provenance, raw runs,
miss regression/refinement and controls: [I2a report](reports/memory-management/I2a-summary.md).

### Next priorities from current-main profiles (2026-10-18)

I2a and its evidence are published on main at `fdf36dc`, with 76 current program
tests plus focused lookup/GC stress checks passing after integration. Preserve
historical timing revisions: newer main includes JIT/PIC changes too.

Work in this order, keeping only measured improvements:

1. [ ] G7: Reduce reclamation overhead in the existing collector. The supplied
   `1c8d910` heap phase profiles attribute about 85% of GC time to the combined
   sweep/finalize phase (about 22 ms per whole run, not per collection). Separate
   ordinary timing from diagnostic phase measurements. Start with known-owner
   pool returns and verification-only quarantine routing; then measure batched
   dead-cell handling, sweep partitioning and cache locality. Finalizer ordering,
   worker publication, verifier quarantine and actual freeing must remain valid.
   - [x] G7a: Separate known-owner pool returns from quarantine routing; keep
     narrow wide-map improvements and record mixed applications (69b3a61).
   - [x] G7b experiment: Measure per-object finalizer flags; leave the production
     prototype out of main due to mixed applications and repeat state regression.
     Keep the benchmark evidence and saved-MIR stress checker.
   - [x] G7c experiment: Measure descriptor-only finalizer caching; do not retain
     the prototype because application/phase evidence overlaps noise controls.
   - [x] G7d: Keep bounded x86-64 sweep header prefetching (38855c7), with
     same-binary distance controls and state/snapshot application improvements.
   - [ ] G7e: Attribute trace work/cardinality after G7d, then evaluate bounded
     mark-header batching/prefetching without changing reachability or GC policy.
   Source-level finalizer guard stores are already removed by release LLVM;
   do not claim savings for deleting them.
2. [ ] I2b/I8: Remove materialized map entry tuples/lists when immediate iteration
   can use the existing cursor, and broaden proven nonescaping aggregates. Profile
   snapshots and realworld state/reconcile; retained maps and stored entry lists
   must keep their semantics. The runtime already has CHAMP-layout HAMT nodes.
   Evaluate compact-node/path-copy improvements from measured node distributions,
   rather than treating a CHAMP conversion as new work.
3. [ ] G8: Improve allocation throughput and pool occupancy from size/survival
   evidence. Benchmark per-worker paths, zeroing and allocation debt separately.
   Do not remove required initialization/accounting or shrink chunks blindly:
   earlier small-refill experiments traded speed for memory and were rejected.
   - [x] G8a experiment: Evaluate constant-size recycled-cell clearing; revert
     the prototype after byte-heavy coverage regresses 9–17% and application
     repeats stay small/mixed. [Negative evidence](reports/memory-management/G8a-summary.md).
   - [x] G8b: Count pooled reuse/bump/system allocation requests and native pool
     refill/trim events in diagnostic builds. Calibrate instrumentation and
     save both coverage and application censuses. This is measurement, not a
     GC speed improvement. [Allocation-path evidence](reports/memory-management/G8b-summary.md).
   - [x] G8c experiment: Evaluate skipping redundant HAMT initialization;
     reject/revert after small application gains, repeated JSON regression and
     8–14% byte-heavy coverage regression. Keep modern-compiler compatibility
     checks and bounded Miri model coverage. [Evidence](reports/memory-management/G8c-summary.md).
   - [x] G8d: Optimize bounded scalar buffer initialization/push or proven bulk
     fills. I3a byte profiles put 67–71% of self samples in buffer::push and
     whole-kernel GC pause near 0.5%. Preserve callback count/order, failure,
     suspension, pointer barriers/roots and growth semantics. Save fresh baselines
     and standalone/saved-MIR controls; native executable sensitivity remains open.
     Cached representation and reserved stores improve byte/word churn 15–21%
     and messaging 7–15% in longer repeats. Allocations stay unchanged; retain
     small/mixed JSON/batch costs and slightly higher RSS.
     [Corrected-main evidence and limits](reports/memory-management/G8d-summary.md).
4. [ ] I9/G9: Extend escape/lifetime proofs to stack or region storage. First
   specify roots across suspension, failure cleanup, aliases and cross-task
   publication. Evaluate scoped transient regions before general shared arenas;
   explicit owned-message/batch handoff is a candidate boundary. Pools reduce
   allocator work but still require GC tracing unless ownership proves reclamation.
5. [ ] W1: Attribute worker ceilings per workload, recording 1/2/4/8-worker
   work distribution, CPU, allocations and rendezvous/queue costs. CPU pipeline
   already scales about 6.6x in the supplied baseline. Serial map loops and
   serialized actor stages cannot gain from idle extra workers alone; investigate
   independent allocation-heavy tasks before changing scheduler/GC synchronization.
   - [x] W1a: Add fixed-eight-consumer baselines and CPU profiles at 1/2/4/8
     workers. CPU-heavy pipeline scales 6.72x; cheap messages stop gaining after
     two. Idle/wake and channel spinning grow; GC is a small sampled share in
     that message shape. [Attribution and unresolved source regression](reports/memory-management/W1-followup.md).
   - [ ] W1b: Count channel spins/fallbacks, wake/idle/steal paths and useful
     per-worker work before changing backoff, wake or reserved-slot policy.

Add the unchanged realworld and compare contracts to adjacent-pair compiler
comparisons, including records and snapshots. Save current-production baselines
before edits and keep controls, negative results and all raw measurements.

### G7a complete — bounded pool-return simplification, mixed broader results

Keep `69b3a61` on the isolated follow-up branch. Normal sweep returns no longer
check verification or discover owners; only quarantine draining searches pools.
Cross-worker pooled/system release, zeroing and trimming are regression checked.
No allocation-count, GC-policy, layout or finalizer-order change is made.

Saved 242 baselines before production edits; final evidence has 1762 measured
runs with independent oracles and nonoverlapping intervals. Wide persistent-map
coverage improves 4–7% elapsed / 12–20% GC pause, but record-window work stays
about +1–2% and applications remain mixed. The 15-pair follow-up has messaging/
four +5.4% group / +2.5% paired, state/one -3.1% / -3.4%, state/four +2.2% /
+1.3%. Same-binary application controls have large records/state swings too.
Messaging diagnostic probes perform zero GCs, so do not attribute its timing
changes to the modified sweep. No general RSS, p99 or scaling win is established.

All 76 program tests, 10 ordinary/12 diagnostic runtime tests, 88 focused
source/MIR GC checks plus two MIR probes, 20 harness tests, strict runtime
Clippy in both modes and scoped formatting pass. G7 remains open; reducing
materialized map-entry allocations is the next bounded change. Complete raw
results, controls and limits: [G7 report](reports/memory-management/G7-summary.md).

### I2b complete — remove immediate map-method entry materialization

Keep `67d3f73`: optimized standard Map.entries() loops reuse the existing cursor,
with one receiver evaluation and unchanged order. Destructuring avoids tuple
boxes; stored entry lists and escaping tuples retain value representations.
User-defined entries methods, debug/live/no-inline behavior and runtime/MIR
layout contracts remain unchanged. Both old/new saved programs run on both
runtime versions; no new API or MIR format is introduced.

Saved 104 baselines before compiler edits. Final evidence has 952 measured
processes, independent oracles, no interval overlap and no removed outliers.
Immediate entry iteration improves 55–69%; stored-list control stays roughly
flat. Diagnostic counts prove 524,288 fewer tuple objects plus list storage,
net 528,896 fewer managed allocations / 21.40625 MiB of cell traffic per full
immediate case. Stored-family and MapNode counts/bytes are unchanged.

Realworld state work improves 49–56% (all seven paired processes faster at every
worker setting): about 8.6–9.4 ms becomes 4.1–4.5 ms. This is serial state work,
not worker scaling. Diagnostic state removes 520,192 tuple allocations across
eight inner iterations. Reconcile removes only 8,192 tuples while still creating
897,160 MapNodes / 402.8 million cell bytes per process; its timing remains mixed.
No general application, compare-snapshots, RSS or p99 improvement is claimed.

All 76 program/six MIR tests, 72 focused source/MIR GC checks and eight MIR
probes, 21 harness tests, strict MIR Clippy in both modes and formatting pass.
Next I5a: prove private, empty-origin scalar maps can reuse transient updates;
preserve any live aliases and every observable snapshot. [I2b results](reports/memory-management/I2b-summary.md).

### I5a complete — prove private scalar maps can reuse their trie

Keep `fc759e3`: one fresh empty origin, consistent scalar K/V descriptors,
copy/insert-only producers, no old aliases live after an update, and no cursor
or publication before reachable later updates. Convert every insert in a group
or none. Reuse existing transient roots/barriers and retain the budget check.
Final maps are ordinary immutable shared values; inputs/pointer maps/uncertain
cases retain persistence. New optimized MIR requires the private helper, while
old format-6 saved programs continue to run.

Saved 132 baseline measurements before code edits. Final evidence: 1116 GC/
application comparison processes with no interval overlap/outlier removal,
plus separately scoped official four-language short/long-warm studies.
Reconcile improves 71–73% at all worker counts (all seven pairs faster); RSS
falls 18–19%. Diagnostic eight-iteration MapNode counts 897,160→7,848 and Map
wrappers 411,432→8 remove about 394 MiB allocation traffic; collections 28→3.
True snapshot counts/bytes remain exact controls, with noisy small timing changes.

Eight fixed independent map tasks improve 201.0→30.4 ms at one worker and
65.7→6.9 ms at eight: scaling grows 3.1x→4.4x; eight-worker RSS -50.9%, workload
collections 62→0. Serial state/actor stages are not claimed to scale from this.

Fresh four-language reconcile: Badger 4.561 ms, Go 3.056, Node 8.548, Java
2.678 (noisy). State remains behind Go/Java; record and JSON gaps remain. Longer
JSON warmup changes Java's result substantially, exposing a benchmark convergence
issue; both cohorts are retained. No universal language or p99 win is claimed.

All 76 program/12 MIR tests, 10 ordinary/12 diagnostic runtime tests, 72 focused
source/saved-MIR GC checks, 28 MIR probes, 36 differential cases, 22 harness tests,
strict four-crate Clippy in both modes and formatting pass. Full proof, counts,
controls and cross-language evidence: [I5a report](reports/memory-management/I5a-summary.md).

Next G7b experiment: cache finalizer presence in spare object metadata bits and
reuse descriptor shape-cache initialization, avoiding dead nonfinalizer descriptor
lookups without changing object layout or pool policy. Preserve lazy kind
backpatch/reset semantics and old MIR compatibility. Broader I5, pointer-map
ownership, true-snapshot epochs, packed records and arenas remain open.

### G7b evaluated — object-header finalizer flag not retained

972 measured processes across six nonoverlapping datasets compare `fc759e3`
with `957ff40`. Descriptor and buffer-transition flags pass 76 program tests,
11 normal/13 diagnostic runtime tests, 72 source/saved-MIR stress checks,
allocation-census checks and Clippy. Phase savings do not translate uniformly:
the 15-pair repeat has true compare snapshots -11.7% group/-11.4% paired, but
state +2.7%/+6.6% (only 5/15 faster); the state same-binary control is near flat.
All initial/repeat/control results remain, including the initial snapshot
regression and noisy control. Revert the production prototype before publishing.

Next G7c tests descriptor-only cached classification, avoiding per-allocation
flag stamping and buffer-transition maintenance. G7 remains open. [G7b report](reports/memory-management/G7b-summary.md).

### G7c evaluated — descriptor-only finalizer cache not retained

996 measured processes across six disjoint datasets compare `fc759e3` with
`b1fffcf`. All 76 program/12 MIR/11 normal/13 diagnostic runtime tests,
source/saved-MIR GC checks, allocation conservation and strict Clippy pass.
Some snapshot coverage improves, but the 15-pair application repeat and its
same-binary controls do not establish a broad throughput win. Diagnostic sweep
totals also vary: compare snapshots -6.9%/+4.5% at workers 1/8, records +4.8%/+3.4%.
Revert this production prototype too; keep the evidence. [G7c report](reports/memory-management/G7c-summary.md).

Next G7d: test ahead-of-use header prefetching in sweep, with diagnostic same-binary
distance controls before ordinary application comparisons. No allocator/object
metadata changes. Prefetch is a hypothesis about memory latency, not a measured
attribution yet. Keep the allocation-elision wins and avoid publishing collector
complexity justified only by isolated microbenchmarks.

### G7d complete — sweep header prefetch retained

Keep `38855c7`, now also present in main `a21998b`. 1,382 measured processes in
nine nonoverlapping datasets, with baseline saved before edits, immutable
binaries, independent oracles and all samples retained. Same-binary diagnostic
0/16/32/64-object distance controls support latency sensitivity in scattered
retained maps; other architectures keep their existing sweep path.

Ordinary scalar/pointer snapshot cases improve 6–17% across 1/2/4/8 workers,
with workload GC pauses about 40–49% lower. Record churn improves 1–3%.
Realworld state improves 12–22% group medians; compare snapshots 2–17%, with
supporting block-mean changes. Initial record/four and buffer regressions are
retained; longer repeats are close to their same-binary controls. No universal
win, allocation-volume/RSS reduction, request p99 or eight-worker scaling win
is claimed. Full results and limits: [G7d report](reports/memory-management/G7d-summary.md).

All 76 program/10 normal/12 diagnostic runtime tests, 72 source/saved-MIR GC
checks, allocation conservation, 22 harness tests and strict Clippy pass.
The application harness now retains block means alongside medians so expensive
kernel iterations remain visible. Exact Map/MapNode family counts/bytes stay
unchanged, confirming collection optimization rather than allocation elision.

Next G7e: in the pointer-snapshot diagnostic at eight workers, trace remains
about 69 ms while sweep is about 18 ms after prefetch. Add trace-work counts
before inferring per-object cost. Audit order-sensitive/weak tracers before a
bounded mark queue; drain every pending word before weak clearing/verification/
sweep. I5b remains the next allocation-reduction proposal, and G7 stays open.

### G7e0 complete — trace cardinality and calibration saved

108 measured processes establish workload trace counts at 1/2/4/8 workers.
Eight fixed pointer-snapshot tasks have 2.69M → 5.46M candidates and 145,560 →
284,404 traces from one to eight workers; trace wall time grows 23.3 → 66.3 ms.
This includes substantially more live-graph work, not just scheduler/collector
cost per object. Serial records/scalar snapshot counts stay identical. Instrumented
builds have material overhead; retain ordinary binaries for performance claims.
12 diagnostic runtime/23 harness tests and strict runtime Clippy pass.
[G7e0 evidence](reports/memory-management/G7e-summary.md). G7e batching is not
implemented; move to I5b snapshot ownership epochs next, preserving I5a's path.

### I5b complete — reuse scalar-map nodes between observable snapshots

Retain `49f281c`: checked 59-bit ownership epochs, freeze before publication and
local forks, node reuse only under matching ownership, ordinary wrapper/node
layouts preserved, barriers/budget polls retained. Compiler proof stays limited
to fresh-origin scalar maps; private-only I5a groups retain their cheaper path.
Static zero-sized Persistent/Builder modes remove ownership arguments/tests from
legacy paths after the initial dynamic-token candidate slows private controls.

2,744 measured processes across 15 nonoverlapping datasets, immutable binaries,
independent oracles, all raw samples and intermediate unfavorable results saved.
State Map+MapNode cell traffic drops 131.1→15.3 MB across eight inner iterations
(88.3%); compare snapshots 71.3→12.1 MB (83.0%). Diagnostic collections go 7/4→0;
reconcile's exact Map/MapNode traffic stays unchanged. No GC bypass is claimed.

Short-warm application pairs: state 42–50% faster, snapshots 68–70%, every pair
faster across 1/2/4/8 workers. Extended 15-warmup/15-sample state pairs improve
78–80% medians and 75–76% block means, RSS -23.4%; both warmup regimes remain.
Eight fixed scalar-snapshot tasks improve 149.8→46.0 ms at one worker and
49.6→16.1 at eight, GC 16→2; scaling is 3.02x→2.85x, not a scheduler scaling win.
Pointer maps and architectural arenas remain outside this proof.

Fresh long-warm state: Badger 1.113 ms, Go 2.217, Node 2.316, Java 1.168. Badger
state CV 11.6% prevents a reliable Java rank; reconciliation and JSON gaps remain.
JSON/eight and batches/eight slow cells shrink substantially in longer repeats,
but messaging/two retains +7.0% paired median/+3.2% block mean versus near-zero
same-binary controls. Same-runtime old/new saved MIR does not reproduce the full
source-run shift; native layout/frontend effects remain unisolated. Preserve this
possible tradeoff and mixed RSS instead of claiming universal improvements.

Final runtime 11/13 ordinary/diagnostic tests, 14 MIR tests, strict four-crate
Clippy, 136 GC checks/28 MIR probes/36 differential cases, 72 native-container
checks, allocation conservation and 23 harness tests pass. Initial full program
suite passes 76; specialized suite has one cancellation-race output mismatch in
its saved-MIR corpus, whose full focused rerun passes. Both logs remain and no
oracle was weakened. [Full I5b evidence](reports/memory-management/I5b-summary.md).

Next prioritize fresh profiles of remaining record/JSON allocation and generic
scalar-map read costs. Investigate the two-worker messaging shift with native/
source layout controls. I5c parameter/call-boundary ownership, I4 packed records,
G7e marking, G8 allocator work and I9 stack/region proofs remain unchecked.

### I7a/C1 complete — codec temporaries removed, fixed-task baselines saved

Retain production `b080078` after fresh profiles identify native accepted-name,
path formatting and encoder scratch work in JSON. Inputs stay rooted while
borrowed; current wire names precede aliases, and nested error paths/validation
stay exact. Encoding appends quoted text/integers to its output buffer. The
owned serde JSON tree and managed result allocation remain.

The before-edit coverage baseline is saved. Final ordinary seven-pair matrices:
JSON 17.473→10.585 ms at one worker, about 39–40% faster throughout 1/2/4/8;
all seven pairs improve medians and block means at each setting. New flat,
nested, wire-alias and invalid-path codec coverage improves 30–37%, 46–50%,
46–48% and 23–24% group medians; every pair is faster in every cell.

All 36 diagnostic census runs conserve exact managed object/cell-byte traffic:
JSON 576,265 objects / 40,132,939 bytes; records 1,603,420 / 89,865,136;
reconcile 902,239 / 70,586,320 across eight inner iterations plus startup/output.
This removes native temporary allocation work, not managed GC traffic. Peak
RSS is small/mixed and often higher, including +0.4–0.5% for JSON. Initial
messaging/four +18.5% paired median shrinks to +2.2% in 15-pair repeats, within
wide same-binary controls; small eight-worker tradeoffs remain possible.

Fresh official JSON: Badger 10.507 ms, Go 13.980, Node 6.511, Java 10.592.
The Java difference is too small to rank; Node still leads, and Badger's RSS
remains much higher than Go's. No general language/cold-start ranking is claimed.

All 76 program tests, ordinary/diagnostic runtime suites, strict runtime/CLI
Clippy, formatting, 25 harness tests and 224 source/old/new-MIR codec checks pass.
1,168 measured processes in 16 disjoint datasets retain profiles, unfavorable
results, controls and worker baselines; the official 20 performance/16 validation
processes are separate. [Full results, audit and limitations](reports/memory-management/C1-summary.md).

W1a fixes eight consumers while varying runtime workers: messaging 1.323/0.861/
1.253/1.420 ms versus CPU pipeline 50.104/25.338/13.045/7.454. Fixed-topology
profiles show growing spin and wake/park CPU; GC is small in that cheap-message
shape. The earlier I5b source regression also reproduces (+13.3% paired median),
while identical saved-MIR/native controls do not isolate it. Preserve that open
question. [Worker attribution and baseline](reports/memory-management/W1-followup.md).

Next prioritize G8 record/leaf-object allocation and collection work, with
initialization/root/publication proofs before changing the fast path. Keep W1b
channel spin/wake/idle/steal counters as a separate measured followup. Scalar-map
reads, packed records, bounded marking, parameter ownership, and task/transfer
arenas remain open; C1 implements none of those architectural proposals.

### G8 allocation-path census and rejected initialization experiments

G8a and G8c are evaluated, reverted production experiments; G8b's diagnostic
source-path and refill/trim counters are retained. The new coverage distinguishes
fresh bump allocation from recycled cells, and includes parallel, collection-heavy
snapshots alongside the shorter comparison/application blocks. Raw baselines,
all samples, controls and rejected-prototype hashes remain in the linked G8
reports. Neither clearing experiment establishes a broad application win.

The G8c census conserves object/cell-byte totals exactly across its 48 diagnostic
processes. High system-allocation byte shares alone did not establish a large
clearing-time bottleneck. Next prefer existing I2/I4/I9 allocation elimination
and packing work, plus G7e trace-cardinality attribution, over another zeroing
micro-optimization without ordinary allocation-call CPU evidence. G8, regions,
ring ownership and the broader memory-management roadmap remain open.

### I3a complete — stored finite sequence fusion, with negative controls retained

`10dde6a` removes iterator descriptions and per-element Options for immutable
local range/list pipelines used only by supported count/fold/for consumers.
Constructor arguments/callbacks stay at the declaration; each traversal has its
own counters. Captured/escaping/mutable descriptions, unknown/scoped sources,
stored aliases and collect/eager consumers keep ordinary semantics. Broad I3,
interprocedural escape analysis, stack/region storage and arenas remain open.

A committed before-edit baseline and 2,756 measured processes plus 12 perturbed
profiles cover workers 1/2/4/8, exact same-native allocation counts, old/new MIR,
standalone controls and unchanged original applications. Scalar time falls
96–97%, with 1,048,576 Options removed; list time falls 45–76%. The scalar case
has 60 total managed objects instead of 1,048,716, and two kernel GCs become zero.
All 304 focused GC/semantic checks, 48 MIR probes, CLI/MIR tests and lint pass.

Broader application changes remain mixed. A standalone byte control is still
10.6–13.9% slower with separate source compilers despite byte-identical MIR;
same-MIR cross-native comparison is +3.7–5.5%, same-native controls mixed.
Matching relocated runtime instructions narrow but do not explain this sensitivity.
Keep the raw negatives. Profiles identify per-element buffer push, not collection,
as the dominant byte-fill cost; prioritize G8d alongside the remaining allocation
elimination work. [Full evidence and validation](reports/memory-management/I3a-summary.md).

### October 19 refreshed-main checkpoint and next priorities

G8d has 2,020 corrected-main measured processes plus 272 focused semantic/GC
checks. The broad run found and fixed an unrelated misaligned list-directory
capacity store; that regression-tested correction is published as `b7cf709`.
The follow-up baseline adds 426 disjoint ordinary/diagnostic map and collector
processes, before further production changes. Current-main builder-node
headroom is integrated separately and is not credited to G8d.

The trie already uses 32-way CHAMP. Reference retention is cheap: 131,072 aliases
take about 0.5–0.7 ms over 64–4096-key maps. Reading all retained versions and
copying edited shared paths are separate costs. Private 1024-key string-valued
maps allocate 143,010 nodes/65,541 wrappers for 65,536 updates versus scalar
1,017/6. Scope the next ownership extension to immutable leaf keys/values;
composite graphs and parameter origins remain excluded.

- [x] P1a: Save split update/retention/read, pointer/scalar, per-update/periodic
  retention, parallel-map and collector baselines with independent oracles.
  [Findings and raw-data pointers](reports/memory-management/P1-baseline-findings.md).
- [ ] I5d: Extend fresh-origin ownership reuse to String/Bytes leaf keys/values.
  Preserve all alias/publication boundaries, root pointer arguments before
  allocation, and test old-to-young edges, retained lookup payloads and versions.
- [ ] I8c: Audit allocation-free immutable query calls, starting with string
  length/byte length/empty checks. The pointer read coverage nearly doubles
  with idle extra workers, and these queries currently take the runtime lock.
  Prove rooting/GC coordination and measure lock counts before broadening.
- [ ] I8d: Evaluate contiguous replacement copies and bitmap-rank fast paths.
  Portable `hamt::get` currently uses software popcount. Keep a safe portable
  fallback and require assembly evidence and application/lookup controls.
- [ ] G10: Evaluate scratch/remembered capacity reuse and chunk-range occupancy
  instead of sorting every occupied address. Preparation plus trim is under
  1.5% of record-heavy total work here; prioritize allocation avoidance and
  investigate larger leaf/slab reclamation separately. Sweep still dominates
  collection time and is not considered solved.
