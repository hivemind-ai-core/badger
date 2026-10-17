# Compiler/runtime optimization results — 2026-10-17

The targeted costs improved substantially: local scalar Options are **13.2× faster**, a visible range/map/filter/fold pipeline is **39.4× faster**, indexed array reads are **11.5× faster**, and array iteration is **3.2× faster** in the one-worker probes. Byte storage uses **87.5% fewer element bytes**. The composite probe's generated function code is **51.3% smaller**.

Application performance is mixed. On the larger telemetry workload, **11 of 16 throughput medians decreased**, by up to **14.0%**; five increased, by up to **11.2%**. The median source-compilation/JIT interval decreased from **118.268 ms to 93.798 ms**, or **20.7%**. This bundle has measurable runtime tradeoffs and remains isolated from main.

## Baseline and provenance

- Baseline production revision: `babfdab`, already containing the earlier local pointer-Option niche optimization. Gains below exclude that earlier improvement.
- Benchmark-only baseline commit: `60a9b91717c803dbc89a36568a5afc11c2b92632`, built in `/tmp/language-opportunities-baseline-2026-10-16`.
- Final optimization revision: `3927dcc0f5e0ac3bfaec1d096763a0459cc764fa`, branch `audit/performance-2026-10-16`, worktree `/tmp/language-performance-audit-2026-10-16`.
- Host: AMD Ryzen 7 8745HS, x86-64 Linux, Rust 1.98.1. Rust builds use Cargo release with thin LTO; language execution uses `--release` (Cranelift speed optimization).
- [Provenance and executable hashes](performance-audit-2026-10-16/opportunities/final/provenance.json) identify both compiler binaries and their matching runtime/MIR probes. No dependency versions changed.

When benchmarking was requested, scalar-enum and array/text slices had already been committed. Those commits were preserved, the unfinished GC work was stashed, and a clean worktree at `babfdab` established the baseline. The WIP was restored only after baseline and telemetry runs completed. The original stash is retained as a rollback backup.

The [initial baseline](performance-audit-2026-10-16/opportunities/baseline/measurements.json) is preserved. A [fresh baseline recheck](performance-audit-2026-10-16/opportunities/baseline/recheck.json) uses the final fixture paths and canonical formatting. Final before/after comparisons use identical fixture and harness hashes, checked programmatically. The initial text fixture required an indentation-only formatter correction.

## Targeted measurements

Five shuffled fresh-process repetitions follow a warmup for each binary/case/worker cell. In-program timers exclude compilation. Setup and array construction are outside the read/iteration timers. Worker counts configure the runtime; these microbenchmarks execute their timed loop in one language task.

| Probe, one worker | Before median | After median | Speedup |
|---|---:|---:|---:|
| Local `Option<Int>`, 200,000 iterations | 3.000 ms | 0.228 ms | 13.17× |
| Range/map/filter/fold, 200,000 inputs | 7.675 ms | 0.195 ms | 39.35× |
| Indexed array reads, 200,000 reads | 3.283 ms | 0.286 ms | 11.49× |
| Integer array iteration, 2,048,000 elements | 7.045 ms | 2.213 ms | 3.18× |
| Byte array iteration, 2,000,000 elements | 6.662 ms | 1.992 ms | 3.34× |
| 100 Unicode slices `[0, 1)`, one-million-character input | 118.868 ms | 0.0042 ms | ~28,000× |
| 100 ASCII slices `[0, 1)`, one-million-character input | 94.192 ms | 0.0033 ms | ~29,000× |

The slice gain removes whole-input indexing for a one-character request. It is specific to small endpoints on long inputs; longer requested prefixes still require scanning their characters. The optimized timings are only a few microseconds per 100 operations and show host jitter.

All 1/2/4/8-worker medians and raw ranges are in the [comparison table](performance-audit-2026-10-16/opportunities/final/comparison.md), backed by [120 raw timed runs](performance-audit-2026-10-16/opportunities/final/measurements.json). Scalar Options improved 12.1–13.2×, fused sequences 36.4–39.4×, indexed array reads 11.3–11.7×, and integer iteration 3.2–3.4× across these runtime configurations. Checksums and direct-loop controls are retained.

## Memory and GC

The standalone runtime probes run in fresh processes, with five repetitions for each generational setting. Current RSS snapshots measure resident memory at the named phase; they differ from process peak RSS, cumulative allocation, and GC pressure.

| Probe | Before | After | Observation |
|---|---:|---:|---|
| Storage for 8,000,000 byte elements | 64,000,000 bytes | 8,000,000 bytes | 87.5% fewer element bytes |
| Resident RSS with that buffer, median | 64,968 KiB | 10,304 KiB | 84.1% lower process RSS |
| Strings retained by a 512-item shared prefix after a full collection | 2,048 | 512 | 75% fewer retained strings |
| RSS after a 500,000-object peak and nine collections, full-GC mode | 35,396 KiB | 2,732 KiB | 92.3% lower resident RSS |
| Same peak/reclamation probe, generational mode | 35,392 KiB | 2,756 KiB | 92.2% lower resident RSS |
| Dead promoted strings after roots drop and nine collections | 100,000 | 0 | Periodic full collection reclaims them |

The shared-prefix improvement applies after a full collection. Minor collections retain the directory scanning needed for old-to-young writes. A shared partial tail can retain up to one physical page of invisible elements; stable element addresses remain valid.

The generated-code lifetime probe releases the temporary array's 800,048 external bytes before returning from the function. Its object-count snapshots reflect different collection phases and are not cumulative allocation counters. A separate regression verifies that two 2 MB arrays can be used sequentially with a 3 MB heap budget on one and four workers.

Buffer capacity and backing storage now count toward heap budgets and GC scheduling. Verification mode intentionally quarantines freed objects, so physical-memory release results above use normal collection mode. Explicit old-object, shared-prefix, branch-append, stable-address, and pool-reuse tests also pass with collection stress and heap verification.

## Generated code and compilation

| Fixture | Functions before/after | Generated function bytes before/after | Saved MIR bytes before/after |
|---|---:|---:|---:|
| Composite compute probe | 56 / 18 | 26,437 / 12,869 (−51.3%) | 50,906 / 31,269 (−38.6%) |
| Array probe | 28 / 12 | 10,811 / 7,314 (−32.3%) | 20,074 / 15,676 (−21.9%) |

These are post-compilation function byte counts from the code generator, excluding adapters, trampolines, constant data, and mapping granularity. They combine changes from fusion, scalarization, rooting, and function pruning. The remaining functions are all reachable under the conservative entry/hook/static-closure/address-reference graph. This does not imply every retained function executes.

Across the 160 larger application runs, the median compiler-reported source-compilation/JIT interval fell 20.7%. The small frontend-only compile probes did not establish a consistent speedup: aggregate host CPU time was approximately 31.8→31.4 ms for compute and 27.3→29.2 ms for arrays. Code-size reductions are clearer than frontend-latency gains.

## Application throughput and latency

The telemetry topology validates payload bytes, event conservation, routing, cache requests/misses, and aggregation with an independent Python oracle. All four profiles keep eight parser tasks while sweeping 1/2/4/8 workers. Final runs use 65,536 events, five repetitions, one warmup per binary/cell, and shuffled before/after processes. Every measured run passed the oracle.

Throughput changes below compare medians; a negative value means slower:

| Profile | 1 worker | 2 workers | 4 workers | 8 workers |
|---|---:|---:|---:|---:|
| balanced | -12.8% | +8.9% | +11.2% | -11.3% |
| backpressure | -14.0% | -11.8% | -11.8% | -2.4% |
| fanout | -6.3% | -13.3% | -8.0% | -11.4% |
| cpu | +4.2% | +9.8% | +6.5% | -8.1% |

See the [application table](performance-audit-2026-10-16/opportunities/final/application-comparison.md) for actual event rates, minima/maxima, RSS, and pooled latency bucket bounds, and [raw configurations/histograms/process counters](performance-audit-2026-10-16/opportunities/final/telemetry-large.json.gz) for all 160 runs. An additional [8,192-event comparison](performance-audit-2026-10-16/opportunities/final/telemetry-small.json.gz) contains 96 measured runs.

Application RSS also varies by profile. Backpressure at four workers decreased from 63.5 to 55.7 MiB, while balanced at four workers increased from 71.9 to 77.8 MiB. Several process/throughput ranges contain substantial outliers. The host was not isolated or frequency-pinned; interleaving reduces drift but does not eliminate scheduling and GC variability.

Each larger cell pools 20,480 latency samples. Histogram bucket bounds are exploratory, especially at p99.9; these runs do not establish a general tail-latency improvement. The application regressions remain measured outcomes. This experiment does not attribute them to a particular pass or runtime change.

The [first comparison](performance-audit-2026-10-16/opportunities/first-comparison/measurements.json) exposed unnecessary root writes on every fast poll. Moving publication into the slow path improved the integer iteration probe from roughly 3.43 ms to 2.21 ms. That refinement preserves roots when the worker can actually stop, suspend, or collect. The application results above include it.

## Implementation and eligibility

- Local scalar enums use separate tag and payload locals, preserving copies, branch merges, mixed payload representations, and inactive pointer clearing. Escaping values keep the existing boxed ABI.
- Arrays/buffers pack elements at their representation width. Known typed array reads and iteration use direct loads; mutable buffer iteration retains checked runtime access. Saved MIR format advances from 4 to 5 for the changed layout ABI.
- Text slices scan only through the requested endpoint. Text lines/split avoid a full temporary `Vec<String>`; that streaming change has no separate timing claim here.
- GC bounds consecutive minor collections, returns wholly empty pool chunks, drops invisible list-directory pages during full collection, and exposes external/pool storage counters. Root frames omit null-only locals, pack live roots into reusable slots, and publish poll roots on the slow path. Raw-word pointer sources retain conservative whole-call roots.
- Function pruning preserves entries, tests, display hooks, static closures, relocation origins, direct calls, and function/adapter addresses. IDs are remapped consistently; live dispatch keeps its original IDs.
- Sequence fusion recognizes directly visible standard range/list sources with map/filter/take, consumed by fold/count/for. Construction arguments execute once in order, limits preserve pull/effect order, and inclusive Int::MAX terminates safely. Reusable sequences stored in locals, unknown sources, scoped sources, collect, flat_map, and zip retain the ordinary iterator path. Debug and live builds retain ordinary sequence lowering.

## Validation and review state

`cargo test --workspace --release --offline` passes: **121 tests passed, zero failed, one ignored**, across 44 targets. The ignored test is the existing native-crate documentation example. Coverage includes source, optimized, multicore, saved-MIR corpus modes; callback/cleanup and live reload; byte/word/float/codecs; GC stress and verification; and concurrent channel/task stress.

`cargo fmt --all -- --check`, repository language formatting, and Clippy with `-D warnings` pass. [Workspace test output](performance-audit-2026-10-16/opportunities/final/workspace-tests.txt) and lint logs accompany the evidence.

The stress suite caught an intermediate stop-the-world defect in the new weak-page clearing: a nested public inspection helper resumed workers before sweep completed. The collector now uses a stopped-world internal iterator. An explicit regression runs task-result/list workloads under both generational settings with four workers, chaos scheduling, and heap verification.

Implementation commits: `ea47906` (scalar enums), `6ff46b4` (arrays/text/accounting), `0d00f6d` (GC/rooting/pruning), `eef6fd8` (sequence fusion), `a325294` (collection synchronization regression/fix), and `3927dcc` (poll publication). Benchmark harnesses and raw evidence are separate commits. Main has not been merged or edited by this work.

Reproduction commands are in [bench/opportunities/README.md](../bench/opportunities/README.md). Runtime/application tradeoffs remain visible in the tables above; the targeted microbenchmark gains should not be used as a whole-language speedup claim.
