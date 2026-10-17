# Concurrent telemetry indexer

Revision `9a2d68e4f829b4732aa4d4a74973102cf4ff406d`; AMD Ryzen 7 8745HS w/ Radeon 780M Graphics; allowed CPUs [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15].

Pipeline throughput uses in-program monotonic timing from setup through drain. Latency is sampled per event, starting before its batch is built and ending after its aggregation. Quantiles below are pooled histogram bucket upper bounds. RSS, CPU, context switches, and optional perf counters cover the whole process, including JIT.

| Profile | Workers | Events/s (median) | Speedup | Efficiency | p50 ≤ ms | p99 ≤ ms | p99.9 ≤ ms | Peak RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| balanced | 1 | 377,235 | 1.00 | 100.0% | 23.552 | 36.864 | 43.008 | 51.5 |
| balanced | 4 | 428,948 | 1.14 | 28.4% | 21.504 | 32.768 | 36.864 | 110.0 |

Process-wide counters (including compilation), medians:

| Profile | Workers | CPU % of one core | Voluntary switches | Involuntary switches |
|---|---:|---:|---:|---:|
| balanced | 1 | 99.3 | 1 | 40 |
| balanced | 4 | 138.2 | 2,828 | 207 |

All measured runs passed the independent payload/hash/routing/cache oracle. Raw runs, histograms, managed heap snapshots, process counters, config, and executable/source hashes are in the JSON.

Semantic payload/task/channel allocation counts are recorded; allocator calls are unavailable and reported as null. Managed heap snapshots are neither cumulative allocations nor peak live heap. At least 100,000 latency samples per cell is recommended before interpreting p99.9. Memory scaling requires repeating at larger event counts (see bench/telemetry.md).
