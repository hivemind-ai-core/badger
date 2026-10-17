# Concurrent telemetry indexer

Revision `ef8d846a3515bd7c8d5bbe46a5e3602bb051c013`; AMD Ryzen 7 8745HS w/ Radeon 780M Graphics; allowed CPUs [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15].

Pipeline throughput uses in-program monotonic timing from setup through drain. Latency is sampled per event, starting before its batch is built and ending after its aggregation. Quantiles below are pooled histogram bucket upper bounds. RSS, CPU, context switches, and optional perf counters cover the whole process, including JIT.

| Profile | Workers | Events/s (median) | Speedup | Efficiency | p50 ≤ ms | p99 ≤ ms | p99.9 ≤ ms | Peak RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| backpressure | 1 | 636,192 | 1.00 | 100.0% | 0.960 | 3.712 | 3.968 | 46.3 |
| backpressure | 2 | 768,374 | 1.21 | 60.4% | 0.608 | 3.712 | 4.352 | 48.8 |
| backpressure | 4 | 1,095,644 | 1.72 | 43.1% | 0.320 | 4.352 | 4.608 | 54.1 |
| backpressure | 8 | 1,434,023 | 2.25 | 28.2% | 0.240 | 2.560 | 2.688 | 59.1 |
| balanced | 1 | 720,556 | 1.00 | 100.0% | 9.216 | 12.800 | 12.800 | 46.3 |
| balanced | 2 | 865,702 | 1.20 | 60.1% | 6.912 | 10.752 | 11.264 | 47.2 |
| balanced | 4 | 1,247,576 | 1.73 | 43.3% | 4.352 | 7.424 | 7.680 | 52.0 |
| balanced | 8 | 1,672,738 | 2.32 | 29.0% | 2.816 | 4.864 | 4.864 | 62.6 |
| cpu | 1 | 310,408 | 1.00 | 100.0% | 26.624 | 29.696 | 30.720 | 45.4 |
| cpu | 2 | 458,563 | 1.48 | 73.9% | 15.872 | 27.648 | 27.648 | 48.2 |
| cpu | 4 | 762,478 | 2.46 | 61.4% | 9.216 | 13.312 | 13.312 | 53.5 |
| cpu | 8 | 828,450 | 2.67 | 33.4% | 7.424 | 12.288 | 12.288 | 68.9 |
| fanout | 1 | 258,272 | 1.00 | 100.0% | 30.720 | 36.864 | 38.912 | 53.9 |
| fanout | 2 | 331,025 | 1.28 | 64.1% | 22.528 | 34.816 | 34.816 | 59.0 |
| fanout | 4 | 389,902 | 1.51 | 37.7% | 20.480 | 31.744 | 32.768 | 69.2 |
| fanout | 8 | 339,701 | 1.32 | 16.4% | 23.552 | 27.648 | 27.648 | 73.5 |

Process-wide counters (including compilation), medians:

| Profile | Workers | CPU % of one core | Voluntary switches | Involuntary switches |
|---|---:|---:|---:|---:|
| backpressure | 1 | 96.6 | 1 | 10 |
| backpressure | 2 | 109.4 | 157 | 26 |
| backpressure | 4 | 119.5 | 448 | 12 |
| backpressure | 8 | 142.4 | 1,354 | 21 |
| balanced | 1 | 92.6 | 1 | 3 |
| balanced | 2 | 106.1 | 274 | 11 |
| balanced | 4 | 115.8 | 628 | 9 |
| balanced | 8 | 134.0 | 1,103 | 16 |
| cpu | 1 | 98.2 | 1 | 10 |
| cpu | 2 | 115.9 | 115 | 23 |
| cpu | 4 | 132.9 | 648 | 21 |
| cpu | 8 | 162.9 | 2,635 | 30 |
| fanout | 1 | 98.7 | 1 | 9 |
| fanout | 2 | 119.5 | 978 | 14 |
| fanout | 4 | 154.5 | 5,004 | 28 |
| fanout | 8 | 230.1 | 18,946 | 34 |

All measured runs passed the independent payload/hash/routing/cache oracle. Raw runs, histograms, managed heap snapshots, process counters, config, and executable/source hashes are in the JSON.

Semantic payload/task/channel allocation counts are recorded; allocator calls are unavailable and reported as null. Managed heap snapshots are neither cumulative allocations nor peak live heap. At least 100,000 latency samples per cell is recommended before interpreting p99.9. Memory scaling requires repeating at larger event counts (see bench/telemetry.md).
