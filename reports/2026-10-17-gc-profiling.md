# Application and GC profiling — 2026-10-17

**GC and allocation are high-priority application targets.** In ordinary release telemetry runs, GC accounts for roughly 12–38% of elapsed time. Diagnostic phase measurements assign 61–89% of collector time to sweep/finalization. Runtime CPU sampling also repeatedly identifies pool allocation and persistent-map updates as hotspots. Worker-stop rendezvous is tiny in the phase-only runs; it is not the first GC component to optimize.

A larger young-generation budget improves sustained balanced throughput by 44% with one worker and 16% with four, but increases peak RSS by 60–70% and increases the largest observed pauses. This is evidence for adaptive GC work, not a proposal to ship a larger fixed default.

## Scope and method

- Ordinary baseline: `fa2b80e`, including the merged MIR/scalar-enum/array/fusion/GC work. Main continued advancing during this investigation; these are fixed-revision measurements, not measurements of its later commits.
- Diagnostic runtime: `0fd91cf`; final measurement harness: `7d76119`. Immutable normal and diagnostic executables, source hashes, commands, host and toolchain are recorded in [provenance](gc-profile-2026-10-17/provenance.json) and each raw matrix.
- Telemetry: 262,144 events, eight fixed parser tasks, four profiles, workers 1/2/4/8, one warmup and three shuffled repetitions per cell. Ordinary, diagnostic-disabled and phase-enabled builds are compared separately.
- Policy sweep: same telemetry inputs and worker matrix; diagnostic control versus a 64 MiB young budget, at most 32 consecutive minors, or disabled pool/metadata trimming. These controls exist only behind the optional `gc-profile` Cargo feature.
- Sustained follow-up: 1,048,576 balanced events, workers 1/4, five repetitions, control versus 64 MiB. CPU sampling plus existing lock statistics is a separate perturbed experiment: two repetitions on workers 1/4.
- Every timed subprocess passes its independent checksum/oracle. Timers exclude frontend/JIT; reported process CPU/RSS includes them. GC records are filtered by workload collection indices to exclude later output encoding.
- Peak RSS comes from a clean native launcher measuring the actual language child. A regression verifies that a 64 MiB allocation in Python cannot become the reported language RSS floor. Sampling allocates its own buffers and loads symbols, so sampling RSS is not an application-memory comparison.

## Ordinary application cost

| Profile | Workers | Work median ms | GC median ms | Median per-run GC share | Peak RSS MiB |
|---|---:|---:|---:|---:|---:|
| backpressure | 1 | 468.067 | 80.139 | 17.4% | 52.3 |
| backpressure | 2 | 362.701 | 77.587 | 22.1% | 54.3 |
| backpressure | 4 | 241.537 | 60.373 | 25.5% | 70.2 |
| backpressure | 8 | 164.623 | 51.357 | 31.2% | 74.6 |
| balanced | 1 | 545.296 | 118.572 | 21.5% | 81.2 |
| balanced | 2 | 376.063 | 98.326 | 26.2% | 85.2 |
| balanced | 4 | 257.740 | 79.820 | 30.7% | 96.3 |
| balanced | 8 | 180.586 | 68.802 | 38.7% | 99.9 |
| cpu | 1 | 984.545 | 117.968 | 12.0% | 78.1 |
| cpu | 2 | 593.066 | 98.596 | 16.6% | 84.6 |
| cpu | 4 | 340.431 | 75.575 | 22.4% | 89.4 |
| cpu | 8 | 358.881 | 78.426 | 22.1% | 127.0 |
| fanout | 1 | 1456.195 | 284.832 | 19.2% | 80.8 |
| fanout | 2 | 1157.437 | 250.544 | 21.7% | 103.2 |
| fanout | 4 | 776.263 | 191.888 | 25.0% | 106.3 |
| fanout | 8 | 836.042 | 160.935 | 19.2% | 101.3 |

[All raw ranges and diagnostic calibration](gc-profile-2026-10-17/telemetry/summary.md). Phase instrumentation and frame-pointer builds perturb timing and allocation layout; their timing differences are not an optimization result.

## Inside the collector

Fractions below sum disjoint measured GC phases across the three phase-only repetitions. They are collector wall-time shares, not application CPU shares.

| Profile, four workers | Sweep/finalize | Trace | Pool/metadata trim | Stop-world rendezvous | Reclaimed managed objects/event |
|---|---:|---:|---:|---:|---:|
| balanced | 64.1% | 26.3% | 1.0% | 0.07% | 15.3 |
| backpressure | 85.0% | 7.9% | 0.6% | 0.06% | 15.8 |
| fanout | 63.8% | 20.5% | 5.8% | 0.08% | 31.8 |
| cpu | 68.5% | 22.8% | 0.9% | 0.16% | 16.1 |

Across all worker/profile cells, sweep/finalize is 60.6–88.9%, trace 5.9–29.3%, and stop-world rendezvous at most 0.16%. Trimming is usually around 1% (one four-worker fanout cell reaches 5.8%). Root enumeration, weak-page clearing, preparation and accounting comprise the rest. Sampling runs sometimes exhibit much larger rendezvous delays; use the phase-only matrix for these phase shares.
Reclaimed-object counts are measured frees during workload collections, not total allocator calls. In the four-worker matrix, full collections are 6 of 72 balanced collections and 15 of 135 fanout collections across repetitions, but consume about 34% and 41% of the respective collection wall times. Both minor sweep cost and major cadence merit attention.

## CPU hotspots

CPU sampling starts at runtime entry, excluding frontend/JIT, and includes diagnostic sweep helpers. Counts aggregate both measured profiles per cell. Inclusive percentages overlap; do not add them, and do not equate CPU shares across workers with stop-the-world elapsed time.

| Profile | Workers | CPU samples | Pool allocation self | GC inclusive union | Map insert inclusive |
|---|---:|---:|---:|---:|---:|
| balanced | 1 | 1936 | 12.6% | 20.0% | 26.2% |
| balanced | 4 | 1556 | 14.3% | 13.5% | 23.1% |
| backpressure | 1 | 1668 | 6.8% | 16.0% | 21.3% |
| backpressure | 4 | 2104 | 18.0% | 11.4% | 18.7% |
| fanout | 1 | 5725 | 13.0% | 15.9% | 19.2% |
| fanout | 4 | 5129 | 13.2% | 11.1% | 18.4% |
| cpu | 1 | 2046 | 8.7% | 12.6% | 13.3% |
| cpu | 4 | 2237 | 8.7% | 7.9% | 14.1% |

- [`gc::alloc_cell`](../crates/lang_runtime/src/gc.rs): size-class free-list access, whole-cell clearing on reuse, bump refill and chunk allocation. The sample identifies the function; it does not isolate zeroing versus cache misses or other instructions.
- [`gc::sweep_from` and `sweep_parallel`](../crates/lang_runtime/src/gc.rs): object/header walks, survivor compaction, reclamation and finalizers. Parallel collections currently spawn scoped OS helper threads per collection; helper lifecycle, partitioning and reuse are candidates for a separate comparison.
- [`map::insert`](../crates/lang_runtime/src/map.rs) and [`hamt::insert_at`](../crates/lang_runtime/src/hamt.rs): copying persistent update paths and constructing new map roots drive allocator/collector work. Even `map::insert_mut` currently calls the persistent insertion engine; a transient-node builder could be investigated with snapshot semantics protected.
- Channel/scheduler work remains visible in native samples, including syscall/yield/atomic and wake/park paths. Existing held-time statistics sample only every 64th acquisition and summed parked time is worker time; neither establishes a wall-time bottleneck on its own.

[Sampling summaries](gc-profile-2026-10-17/samples/summary.md), [exact counts](gc-profile-2026-10-17/analysis.json), and compressed collapsed stacks accompany the report. Native stacks require frame pointers and self-memory reads; the first sandboxed exploratory profile lacked caller frames and is excluded.

## Policy experiments: gains and tradeoffs

64 MiB young-budget throughput changes below compare medians against the diagnostic control, on the same fixed binary. Peak RSS changes accompany them. Other policy variants and raw ranges are retained in the [policy summary](gc-profile-2026-10-17/policy/summary.md).

| Profile | Workers | Throughput change | Peak RSS change |
|---|---:|---:|---:|
| balanced | 1 | +23.2% | +52.4% |
| balanced | 2 | +18.9% | +61.9% |
| balanced | 4 | +29.3% | +46.6% |
| balanced | 8 | +24.7% | +51.1% |
| backpressure | 1 | -1.5% | +101.2% |
| backpressure | 2 | +23.7% | +97.9% |
| backpressure | 4 | -0.5% | +98.5% |
| backpressure | 8 | +23.3% | +75.5% |
| fanout | 1 | +23.1% | +87.4% |
| fanout | 2 | +7.4% | +67.6% |
| fanout | 4 | +15.0% | +63.1% |
| fanout | 8 | +0.1% | +81.8% |
| cpu | 1 | +24.8% | +56.7% |
| cpu | 2 | +7.0% | +50.0% |
| cpu | 4 | -11.4% | +53.7% |
| cpu | 8 | -8.3% | +37.9% |

The 32-minor and no-trim controls do not establish a general throughput win. Deferring major collection increases retention, while trimming is a small direct fraction of most GC pauses. Several policy ranges contain substantial outliers; three repetitions on a shared host are directional evidence, not a significance test.
A larger young budget also reduces the frequency of count-triggered major collections, so this is not an isolated change to minor-collection overhead. The longer balanced run checks that the improvement survives repeated major collections:

| Workers | Policy | Work median ms [min, max] | GC median ms | Peak RSS MiB | Minor/full per run | Largest observed GC pause ms |
|---|---|---:|---:|---:|---:|---:|
| 1 | control | 2719.100 [2668.335, 2881.001] | 650.535 | 84.2 | 89/11 | 24.77 |
| 1 | young64m | 1887.186 [1883.502, 2232.239] | 361.913 | 143.4 | 23/2 | 40.72 |
| 4 | control | 1141.799 [1135.631, 1288.437] | 449.635 | 99.7 | 89/11 | 33.69 |
| 4 | young64m | 984.581 [972.968, 1121.662] | 302.754 | 160.0 | 23/2 | 39.42 |

The throughput improvement is real in this experiment, with clear memory and observed-pause costs. The larger setting still performs two full collections per run; the control performs eleven. Five repetitions and longer input improve the check, but this is one application profile and does not justify a universal default or tail-latency claim. [Sustained raw ranges](gc-profile-2026-10-17/sustained/summary.md).

## Added coverage

Each row includes construction and consumption. Worker settings configure the runtime; these coverage loops execute in one language task. Three repetitions per cell run on workers 1/2/4/8.

| Case, one worker | Work median ms | GC median ms | Peak RSS MiB |
|---|---:|---:|---:|
| bytes-survivors | 110.636 | 1.613 | 40.7 |
| bytes-transient | 101.613 | 1.630 | 39.0 |
| collected | 20.117 | 2.260 | 46.1 |
| eager | 12.574 | 0.791 | 39.6 |
| snapshots | 28.332 | 7.082 | 40.5 |
| stored | 18.822 | 2.273 | 46.0 |
| visible | 0.586 | 0.000 | 24.6 |
| words-transient | 113.888 | 5.535 | 38.4 |

The 524,288-input visible pipeline takes 0.586 ms versus 18.822 ms stored, 20.117 ms collected, and 12.574 ms eager. These are equivalent checksummed computations with different materialization/reuse semantics; the stored shape alone is about 32 times slower. Current fusion coverage therefore remains a substantial opportunity. Retained map histories spend approximately 25% of their measured time in GC. Buffer construction is mostly outside GC in these cases, showing why GC work alone will not accelerate every workload. [Coverage fixtures and oracle](../bench/gc/README.md), [all ranges](gc-profile-2026-10-17/coverage/summary.md).

## Recommended order

1. **Allocator and sweep fast paths.** Profile free-list reuse, clearing, object-list metadata and sweep memory access more finely. Compare bulk reclamation for empty chunks, simpler reclamation metadata, and reused sweep workers/scratch storage. Preserve nonmoving addresses, initialization before safepoints, finalizer ownership and heap verification. These paths are substantial across all sampled profiles.
2. **Adaptive generation sizing and major cadence.** Use survival/allocation rates, external bytes, a resident-memory target and a pause target. The measured 64 MiB gains justify this work, while the RSS/pause results argue against replacing one fixed size with another.
3. **Reduce the allocation sources.** Investigate transient HAMT updates/builders with snapshot correctness, and escaping channel/task-result boxing where allocation attribution supports it. Lower churn can improve throughput and pauses without accepting a larger working set.
4. **Broaden fusion deliberately.** Start with single-use stored sequence values, then selected collecting consumers, preserving reuse, effect order, cleanup and early termination. Keep the new stored/collected/eager controls alongside the visible fast path.
5. **Keep application gates.** Repeat the 1/2/4/8-worker matrix, include longer steady-state runs, and judge throughput, observed/tail latency and memory together. Use isolated hardware and finer latency samples for release-level conclusions.

## Validation and delivery

Implementation and benchmark tooling remain isolated on `audit/gc-profile-2026-10-17`. Normal GC defaults and representation are unchanged; collector phase instrumentation and experimental policies are compiled out without `gc-profile`. Reproduction commands are in [bench/gc/README.md](../bench/gc/README.md).

The feature-enabled release workspace suite passes: **143 passed, zero failed, one existing ignored native documentation example**, across 62 targets. Default and feature-enabled Clippy pass with warnings denied; Rust and language formatting pass; five new benchmark/RSS regressions and nine existing telemetry-runner tests pass. [Validation logs](gc-profile-2026-10-17/validation/workspace-tests.txt) are retained.

The final evidence contains **468 measured fresh-process runs**, plus warmups, with **22,401 runtime CPU samples** in the separate sampling suite. Every run passes its oracle. Collection records were checked for monotonic indices and valid object/external/pool reclamation bounds. Diagnostic sampling and lock statistics can cause large timing outliers; sampled-run timings are not used as throughput evidence. All main application and policy conclusions use the separate unsampled matrices.
