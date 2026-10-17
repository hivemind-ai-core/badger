# Concurrent telemetry indexer

Revision `60a9b91717c803dbc89a36568a5afc11c2b92632`; AMD Ryzen 7 8745HS w/ Radeon 780M Graphics; allowed CPUs [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15].

Pipeline throughput uses in-program monotonic timing from setup through drain. Latency is sampled per event, starting before its batch is built and ending after its aggregation. Quantiles below are pooled histogram bucket upper bounds. RSS, CPU, context switches, and optional perf counters cover the whole process, including JIT.

| Profile | Workers | Events/s (median) | Speedup | Efficiency | p50 ≤ ms | p99 ≤ ms | p99.9 ≤ ms | Peak RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| backpressure | 1 | 646,568 | 1.00 | 100.0% | 0.992 | 2.048 | 2.304 | 44.9 |
| backpressure | 2 | 964,026 | 1.49 | 74.5% | 0.496 | 1.344 | 1.600 | 44.9 |
| backpressure | 4 | 1,417,346 | 2.19 | 54.8% | 0.304 | 1.280 | 1.408 | 44.9 |
| backpressure | 8 | 1,284,143 | 1.99 | 24.8% | 0.304 | 1.280 | 1.472 | 50.4 |
| balanced | 1 | 759,142 | 1.00 | 100.0% | 5.632 | 10.240 | 10.752 | 44.9 |
| balanced | 2 | 994,630 | 1.31 | 65.5% | 3.840 | 6.400 | 6.400 | 44.9 |
| balanced | 4 | 1,457,482 | 1.92 | 48.0% | 2.688 | 3.712 | 3.840 | 44.9 |
| balanced | 8 | 1,488,037 | 1.96 | 24.5% | 2.176 | 2.816 | 2.944 | 50.9 |
| cpu | 1 | 288,342 | 1.00 | 100.0% | 14.336 | 28.672 | 29.696 | 44.9 |
| cpu | 2 | 464,749 | 1.61 | 80.6% | 9.216 | 15.872 | 16.384 | 44.9 |
| cpu | 4 | 806,209 | 2.80 | 69.9% | 5.120 | 8.192 | 8.704 | 45.8 |
| cpu | 8 | 839,352 | 2.91 | 36.4% | 4.352 | 7.168 | 7.424 | 51.4 |
| fanout | 1 | 209,751 | 1.00 | 100.0% | 20.480 | 38.912 | 38.912 | 51.0 |
| fanout | 2 | 232,044 | 1.11 | 55.3% | 16.384 | 38.912 | 40.960 | 52.1 |
| fanout | 4 | 289,746 | 1.38 | 34.5% | 14.336 | 34.816 | 34.816 | 56.7 |
| fanout | 8 | 268,023 | 1.28 | 16.0% | 14.336 | 27.648 | 27.648 | 62.7 |

Process-wide counters (including compilation), medians:

| Profile | Workers | CPU % of one core | Voluntary switches | Involuntary switches |
|---|---:|---:|---:|---:|
| backpressure | 1 | 93.7 | 1 | 4 |
| backpressure | 2 | 102.5 | 86 | 7 |
| backpressure | 4 | 109.3 | 260 | 8 |
| backpressure | 8 | 116.2 | 838 | 37 |
| balanced | 1 | 95.8 | 1 | 16 |
| balanced | 2 | 97.7 | 132 | 21 |
| balanced | 4 | 105.7 | 271 | 18 |
| balanced | 8 | 115.2 | 561 | 14 |
| cpu | 1 | 95.0 | 1 | 8 |
| cpu | 2 | 108.6 | 192 | 18 |
| cpu | 4 | 114.9 | 304 | 9 |
| cpu | 8 | 131.2 | 1,484 | 24 |
| fanout | 1 | 97.1 | 1 | 10 |
| fanout | 2 | 111.2 | 839 | 37 |
| fanout | 4 | 131.2 | 3,015 | 35 |
| fanout | 8 | 179.6 | 9,240 | 34 |

All measured runs passed the independent payload/hash/routing/cache oracle. Raw runs, histograms, managed heap snapshots, process counters, config, and executable/source hashes are in the JSON.

Semantic payload/task/channel allocation counts are recorded; allocator calls are unavailable and reported as null. Managed heap snapshots are neither cumulative allocations nor peak live heap. At least 100,000 latency samples per cell is recommended before interpreting p99.9. Memory scaling requires repeating at larger event counts (see bench/telemetry.md).
