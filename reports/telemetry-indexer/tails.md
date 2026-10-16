# Concurrent telemetry indexer

Revision `9a2d68e4f829b4732aa4d4a74973102cf4ff406d`; AMD Ryzen 7 8745HS w/ Radeon 780M Graphics; allowed CPUs [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15].

Pipeline throughput uses in-program monotonic timing from setup through drain. Latency is sampled per event, starting before its batch is built and ending after its aggregation. Quantiles below are pooled histogram bucket upper bounds. RSS, CPU, context switches, and optional perf counters cover the whole process, including JIT.

| Profile | Workers | Events/s (median) | Speedup | Efficiency | p50 ≤ ms | p99 ≤ ms | p99.9 ≤ ms | Peak RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| backpressure | 1 | 517,350 | 1.00 | 100.0% | 1.216 | 5.632 | 5.888 | 47.2 |
| backpressure | 2 | 671,394 | 1.30 | 64.9% | 0.864 | 6.144 | 6.656 | 48.0 |
| backpressure | 4 | 625,794 | 1.21 | 30.2% | 0.960 | 5.632 | 6.400 | 71.4 |
| backpressure | 8 | 587,049 | 1.13 | 14.2% | 1.024 | 5.632 | 6.400 | 88.2 |
| balanced | 1 | 506,241 | 1.00 | 100.0% | 18.432 | 20.480 | 21.504 | 50.8 |
| balanced | 2 | 587,425 | 1.16 | 58.0% | 13.824 | 21.504 | 23.552 | 59.0 |
| balanced | 4 | 478,460 | 0.95 | 23.6% | 19.456 | 25.600 | 26.624 | 93.5 |
| balanced | 8 | 472,556 | 0.93 | 11.7% | 19.456 | 28.672 | 28.672 | 103.3 |
| cpu | 1 | 195,975 | 1.00 | 100.0% | 47.104 | 49.152 | 51.200 | 47.7 |
| cpu | 2 | 325,374 | 1.66 | 83.0% | 27.648 | 34.816 | 34.816 | 49.5 |
| cpu | 4 | 540,596 | 2.76 | 69.0% | 17.408 | 20.480 | 22.528 | 53.5 |
| cpu | 8 | 412,540 | 2.11 | 26.3% | 19.456 | 36.864 | 36.864 | 77.3 |
| fanout | 1 | 154,223 | 1.00 | 100.0% | 59.392 | 69.632 | 69.632 | 54.8 |
| fanout | 2 | 152,005 | 0.99 | 49.3% | 61.440 | 73.728 | 73.728 | 74.0 |
| fanout | 4 | 149,247 | 0.97 | 24.2% | 61.440 | 73.728 | 77.824 | 103.8 |
| fanout | 8 | 140,595 | 0.91 | 11.4% | 63.488 | 77.824 | 77.824 | 159.1 |

Process-wide counters (including compilation), medians:

| Profile | Workers | CPU % of one core | Voluntary switches | Involuntary switches |
|---|---:|---:|---:|---:|
| backpressure | 1 | 98.3 | 1 | 6 |
| backpressure | 2 | 128.1 | 206 | 12 |
| backpressure | 4 | 141.1 | 664 | 19 |
| backpressure | 8 | 148.0 | 1,828 | 20 |
| balanced | 1 | 97.5 | 1 | 6 |
| balanced | 2 | 116.4 | 193 | 7 |
| balanced | 4 | 111.3 | 412 | 20 |
| balanced | 8 | 118.5 | 1,648 | 28 |
| cpu | 1 | 98.6 | 1 | 10 |
| cpu | 2 | 148.3 | 155 | 13 |
| cpu | 4 | 205.8 | 430 | 24 |
| cpu | 8 | 196.9 | 1,505 | 41 |
| fanout | 1 | 97.9 | 1 | 9 |
| fanout | 2 | 100.5 | 654 | 28 |
| fanout | 4 | 102.7 | 1,719 | 38 |
| fanout | 8 | 109.4 | 5,826 | 66 |

All measured runs passed the independent payload/hash/routing/cache oracle. Raw runs, histograms, managed heap snapshots, process counters, config, and executable/source hashes are in the JSON.

Semantic payload/task/channel allocation counts are recorded; allocator calls are unavailable and reported as null. Managed heap snapshots are neither cumulative allocations nor peak live heap. At least 100,000 latency samples per cell is recommended before interpreting p99.9. Memory scaling requires repeating at larger event counts (see bench/telemetry.md).
