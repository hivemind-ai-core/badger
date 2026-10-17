# Concurrent telemetry indexer

Revision `9a2d68e4f829b4732aa4d4a74973102cf4ff406d`; AMD Ryzen 7 8745HS w/ Radeon 780M Graphics; allowed CPUs [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15].

Pipeline throughput uses in-program monotonic timing from setup through drain. Latency is sampled per event, starting before its batch is built and ending after its aggregation. Quantiles below are pooled histogram bucket upper bounds. RSS, CPU, context switches, and optional perf counters cover the whole process, including JIT.

| Profile | Workers | Events/s (median) | Speedup | Efficiency | p50 ≤ ms | p99 ≤ ms | p99.9 ≤ ms | Peak RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| backpressure | 1 | 520,444 | 1.00 | 100.0% | 1.152 | 6.400 | 6.912 | 47.4 |
| backpressure | 2 | 677,538 | 1.30 | 65.1% | 0.832 | 5.888 | 5.888 | 48.8 |
| backpressure | 4 | 644,693 | 1.24 | 31.0% | 0.864 | 5.888 | 6.144 | 74.3 |
| backpressure | 8 | 588,985 | 1.13 | 14.1% | 1.024 | 5.632 | 5.888 | 99.1 |
| balanced | 1 | 505,007 | 1.00 | 100.0% | 18.432 | 20.480 | 20.480 | 50.7 |
| balanced | 2 | 582,016 | 1.15 | 57.6% | 15.360 | 22.528 | 22.528 | 62.9 |
| balanced | 4 | 486,801 | 0.96 | 24.1% | 18.432 | 25.600 | 26.624 | 84.0 |
| balanced | 8 | 445,055 | 0.88 | 11.0% | 20.480 | 27.648 | 27.648 | 105.2 |
| cpu | 1 | 195,084 | 1.00 | 100.0% | 47.104 | 51.200 | 51.200 | 47.8 |
| cpu | 2 | 321,572 | 1.65 | 82.4% | 27.648 | 34.816 | 34.816 | 49.6 |
| cpu | 4 | 522,035 | 2.68 | 66.9% | 17.408 | 31.744 | 31.744 | 57.0 |
| cpu | 8 | 523,944 | 2.69 | 33.6% | 15.360 | 28.672 | 29.696 | 67.3 |
| fanout | 1 | 156,480 | 1.00 | 100.0% | 59.392 | 77.824 | 77.824 | 54.9 |
| fanout | 2 | 149,288 | 0.95 | 47.7% | 63.488 | 73.728 | 73.728 | 75.0 |
| fanout | 4 | 147,615 | 0.94 | 23.6% | 61.440 | 77.824 | 77.824 | 105.0 |
| fanout | 8 | 137,241 | 0.88 | 11.0% | 65.536 | 81.920 | 81.920 | 155.8 |

Process-wide counters (including compilation), medians:

| Profile | Workers | CPU % of one core | Voluntary switches | Involuntary switches |
|---|---:|---:|---:|---:|
| backpressure | 1 | 96.8 | 1 | 6 |
| backpressure | 2 | 127.7 | 203 | 17 |
| backpressure | 4 | 137.3 | 531 | 16 |
| backpressure | 8 | 139.1 | 1,338 | 16 |
| balanced | 1 | 97.0 | 1 | 8 |
| balanced | 2 | 113.8 | 170 | 15 |
| balanced | 4 | 111.7 | 361 | 18 |
| balanced | 8 | 111.7 | 780 | 29 |
| cpu | 1 | 99.2 | 1 | 9 |
| cpu | 2 | 148.8 | 166 | 21 |
| cpu | 4 | 198.0 | 421 | 18 |
| cpu | 8 | 239.6 | 1,425 | 40 |
| fanout | 1 | 97.9 | 1 | 9 |
| fanout | 2 | 99.9 | 622 | 27 |
| fanout | 4 | 102.6 | 1,730 | 33 |
| fanout | 8 | 107.8 | 4,928 | 50 |

All measured runs passed the independent payload/hash/routing/cache oracle. Raw runs, histograms, managed heap snapshots, process counters, config, and executable/source hashes are in the JSON.

Semantic payload/task/channel allocation counts are recorded; allocator calls are unavailable and reported as null. Managed heap snapshots are neither cumulative allocations nor peak live heap. At least 100,000 latency samples per cell is recommended before interpreting p99.9. Memory scaling requires repeating at larger event counts (see bench/telemetry.md).
