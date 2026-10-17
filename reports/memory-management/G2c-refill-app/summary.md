| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | refill2048 | 453.053 [443.174, 499.744] | 73.416 | 58.1 |
| backpressure | 1 | refill256 | 502.558 [433.778, 544.320] | 83.601 | 56.4 |
| backpressure | 1 | refill512 | 467.134 [439.441, 521.991] | 77.341 | 56.5 |
| backpressure | 8 | refill2048 | 147.651 [125.481, 908.173] | 45.279 | 83.7 |
| backpressure | 8 | refill256 | 141.860 [125.255, 801.797] | 44.706 | 74.9 |
| backpressure | 8 | refill512 | 144.328 [129.562, 145.767] | 44.872 | 71.5 |
| balanced | 1 | refill2048 | 455.223 [421.162, 1067.885] | 84.071 | 85.6 |
| balanced | 1 | refill256 | 491.955 [466.422, 500.000] | 93.995 | 84.9 |
| balanced | 1 | refill512 | 496.251 [445.870, 538.347] | 95.878 | 85.2 |
| balanced | 8 | refill2048 | 142.387 [133.639, 143.929] | 55.988 | 107.8 |
| balanced | 8 | refill256 | 147.871 [136.157, 179.905] | 57.709 | 98.1 |
| balanced | 8 | refill512 | 147.061 [145.602, 717.051] | 57.836 | 105.0 |
| cpu | 1 | refill2048 | 888.226 [836.162, 899.143] | 84.714 | 81.2 |
| cpu | 1 | refill256 | 966.624 [930.021, 1480.452] | 98.527 | 79.3 |
| cpu | 1 | refill512 | 938.492 [839.445, 1007.692] | 89.350 | 79.5 |
| cpu | 8 | refill2048 | 358.433 [300.438, 397.300] | 66.130 | 133.2 |
| cpu | 8 | refill256 | 408.485 [379.692, 441.594] | 107.739 | 133.2 |
| cpu | 8 | refill512 | 415.170 [370.349, 1064.456] | 111.267 | 130.8 |
| fanout | 1 | refill2048 | 1267.032 [1073.129, 1641.000] | 207.690 | 88.7 |
| fanout | 1 | refill256 | 1315.384 [1169.850, 1370.573] | 245.474 | 89.3 |
| fanout | 1 | refill512 | 1310.086 [1136.601, 2346.097] | 218.065 | 89.3 |
| fanout | 8 | refill2048 | 804.640 [769.211, 867.326] | 143.805 | 104.7 |
| fanout | 8 | refill256 | 826.124 [783.718, 866.204] | 154.069 | 101.3 |
| fanout | 8 | refill512 | 832.332 [820.303, 840.459] | 148.568 | 103.0 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | refill2048 | 49152 | 1.024 | 5.120 |
| backpressure | 1 | refill256 | 49152 | 1.088 | 5.632 |
| backpressure | 1 | refill512 | 49152 | 1.088 | 5.376 |
| backpressure | 8 | refill2048 | 49152 | 0.224 | 11.264 |
| backpressure | 8 | refill256 | 49152 | 0.224 | 9.728 |
| backpressure | 8 | refill512 | 49152 | 0.184 | 2.304 |
| balanced | 1 | refill2048 | 49152 | 15.872 | 59.392 |
| balanced | 1 | refill256 | 49152 | 15.872 | 34.816 |
| balanced | 1 | refill512 | 49152 | 15.872 | 32.768 |
| balanced | 8 | refill2048 | 49152 | 4.352 | 15.872 |
| balanced | 8 | refill256 | 49152 | 4.608 | 18.432 |
| balanced | 8 | refill512 | 49152 | 4.864 | 61.440 |
| cpu | 1 | refill2048 | 49152 | 29.696 | 43.008 |
| cpu | 1 | refill256 | 49152 | 32.768 | 86.016 |
| cpu | 1 | refill512 | 49152 | 31.744 | 47.104 |
| cpu | 8 | refill2048 | 49152 | 11.776 | 26.624 |
| cpu | 8 | refill256 | 49152 | 12.288 | 45.056 |
| cpu | 8 | refill512 | 49152 | 13.824 | 55.296 |
| fanout | 1 | refill2048 | 49152 | 45.056 | 77.824 |
| fanout | 1 | refill256 | 49152 | 43.008 | 73.728 |
| fanout | 1 | refill512 | 49152 | 45.056 | 131.072 |
| fanout | 8 | refill2048 | 49152 | 26.624 | 43.008 |
| fanout | 8 | refill256 | 49152 | 26.624 | 49.152 |
| fanout | 8 | refill512 | 49152 | 27.648 | 43.008 |

Diagnostic GC phases summed across measured repetitions (wall time):

- ('backpressure', 1, 'refill2048'): sweep_finalize 201.077 ms, trace 12.805 ms, prepare 4.872 ms, weak_pages 3.934 ms, trim 1.220 ms, roots 0.665 ms, allocation_census 0.154 ms, finish 0.081 ms, accounting 0.058 ms, resume_world 0.056 ms, verify 0.011 ms, stop_world 0.009 ms
- ('backpressure', 1, 'refill256'): sweep_finalize 211.596 ms, trace 14.148 ms, prepare 5.002 ms, weak_pages 4.350 ms, trim 1.264 ms, roots 0.736 ms, allocation_census 0.207 ms, finish 0.084 ms, resume_world 0.070 ms, accounting 0.069 ms, verify 0.012 ms, stop_world 0.008 ms
- ('backpressure', 1, 'refill512'): sweep_finalize 202.179 ms, trace 13.567 ms, prepare 5.108 ms, weak_pages 4.208 ms, trim 1.249 ms, roots 0.726 ms, allocation_census 0.183 ms, finish 0.082 ms, resume_world 0.063 ms, accounting 0.061 ms, verify 0.012 ms, stop_world 0.008 ms
- ('backpressure', 8, 'refill2048'): sweep_finalize 207.608 ms, stop_world 29.974 ms, trace 22.584 ms, prepare 8.405 ms, weak_pages 8.192 ms, trim 1.466 ms, roots 1.373 ms, allocation_census 0.277 ms, finish 0.100 ms, accounting 0.097 ms, resume_world 0.046 ms, verify 0.016 ms
- ('backpressure', 8, 'refill256'): sweep_finalize 199.646 ms, trace 26.199 ms, stop_world 24.276 ms, prepare 6.217 ms, weak_pages 5.419 ms, trim 1.487 ms, roots 1.252 ms, allocation_census 0.304 ms, finish 0.105 ms, accounting 0.083 ms, resume_world 0.049 ms, verify 0.016 ms
- ('backpressure', 8, 'refill512'): sweep_finalize 105.499 ms, trace 14.198 ms, prepare 4.308 ms, weak_pages 4.019 ms, trim 1.185 ms, roots 0.711 ms, allocation_census 0.249 ms, stop_world 0.142 ms, finish 0.063 ms, accounting 0.062 ms, resume_world 0.027 ms, verify 0.013 ms
- ('balanced', 1, 'refill2048'): sweep_finalize 237.837 ms, trace 59.074 ms, prepare 9.665 ms, weak_pages 8.490 ms, trim 4.340 ms, roots 1.281 ms, allocation_census 0.403 ms, finish 0.113 ms, accounting 0.071 ms, resume_world 0.066 ms, verify 0.016 ms, stop_world 0.009 ms
- ('balanced', 1, 'refill256'): sweep_finalize 201.774 ms, trace 45.945 ms, prepare 8.711 ms, weak_pages 7.847 ms, trim 3.436 ms, roots 0.885 ms, allocation_census 0.282 ms, finish 0.069 ms, resume_world 0.053 ms, accounting 0.053 ms, verify 0.013 ms, stop_world 0.008 ms
- ('balanced', 1, 'refill512'): sweep_finalize 202.835 ms, trace 47.857 ms, prepare 8.746 ms, weak_pages 7.864 ms, trim 3.702 ms, roots 1.057 ms, allocation_census 0.390 ms, finish 0.077 ms, resume_world 0.057 ms, accounting 0.055 ms, verify 0.013 ms, stop_world 0.008 ms
- ('balanced', 8, 'refill2048'): sweep_finalize 103.570 ms, trace 42.527 ms, weak_pages 7.272 ms, prepare 7.065 ms, trim 2.516 ms, roots 0.843 ms, stop_world 0.410 ms, allocation_census 0.285 ms, finish 0.064 ms, accounting 0.059 ms, resume_world 0.025 ms, verify 0.013 ms
- ('balanced', 8, 'refill256'): sweep_finalize 112.856 ms, trace 48.122 ms, prepare 7.900 ms, weak_pages 7.836 ms, trim 2.617 ms, roots 0.900 ms, stop_world 0.428 ms, allocation_census 0.365 ms, finish 0.067 ms, accounting 0.064 ms, resume_world 0.030 ms, verify 0.013 ms
- ('balanced', 8, 'refill512'): sweep_finalize 229.893 ms, trace 81.230 ms, stop_world 28.887 ms, weak_pages 15.147 ms, prepare 11.473 ms, trim 3.999 ms, roots 1.771 ms, allocation_census 0.558 ms, finish 0.095 ms, accounting 0.079 ms, resume_world 0.046 ms, verify 0.017 ms
- ('cpu', 1, 'refill2048'): sweep_finalize 185.155 ms, trace 37.873 ms, prepare 7.539 ms, weak_pages 6.293 ms, trim 2.874 ms, roots 1.012 ms, allocation_census 0.280 ms, accounting 0.075 ms, finish 0.073 ms, resume_world 0.050 ms, verify 0.012 ms, stop_world 0.009 ms
- ('cpu', 1, 'refill256'): sweep_finalize 263.222 ms, trace 54.904 ms, prepare 11.317 ms, weak_pages 9.806 ms, trim 3.952 ms, roots 1.673 ms, allocation_census 0.385 ms, finish 0.098 ms, accounting 0.075 ms, resume_world 0.069 ms, verify 0.015 ms, stop_world 0.010 ms
- ('cpu', 1, 'refill512'): sweep_finalize 200.137 ms, trace 41.894 ms, prepare 8.325 ms, weak_pages 7.397 ms, trim 2.950 ms, roots 1.095 ms, allocation_census 0.317 ms, finish 0.081 ms, accounting 0.062 ms, resume_world 0.057 ms, verify 0.012 ms, stop_world 0.010 ms
- ('cpu', 8, 'refill2048'): sweep_finalize 132.679 ms, trace 45.475 ms, prepare 8.179 ms, weak_pages 7.270 ms, trim 2.011 ms, roots 0.922 ms, allocation_census 0.333 ms, stop_world 0.248 ms, finish 0.074 ms, accounting 0.067 ms, resume_world 0.032 ms, verify 0.013 ms
- ('cpu', 8, 'refill256'): sweep_finalize 145.812 ms, trim 107.253 ms, trace 47.213 ms, prepare 8.706 ms, weak_pages 8.228 ms, roots 0.937 ms, allocation_census 0.312 ms, stop_world 0.283 ms, finish 0.074 ms, accounting 0.066 ms, resume_world 0.036 ms, verify 0.013 ms
- ('cpu', 8, 'refill512'): sweep_finalize 228.725 ms, trim 78.507 ms, trace 65.889 ms, stop_world 42.253 ms, prepare 10.274 ms, weak_pages 8.885 ms, roots 1.603 ms, allocation_census 0.431 ms, finish 0.094 ms, accounting 0.093 ms, resume_world 0.051 ms, verify 0.016 ms
- ('fanout', 1, 'refill2048'): sweep_finalize 475.908 ms, trace 107.289 ms, prepare 18.898 ms, weak_pages 16.763 ms, trim 9.807 ms, roots 7.769 ms, allocation_census 0.317 ms, finish 0.166 ms, accounting 0.121 ms, resume_world 0.108 ms, verify 0.023 ms, stop_world 0.014 ms
- ('fanout', 1, 'refill256'): sweep_finalize 466.484 ms, trim 104.417 ms, trace 102.056 ms, prepare 19.281 ms, weak_pages 16.388 ms, roots 7.103 ms, allocation_census 0.290 ms, finish 0.154 ms, accounting 0.111 ms, resume_world 0.097 ms, verify 0.022 ms, stop_world 0.014 ms
- ('fanout', 1, 'refill512'): sweep_finalize 535.143 ms, trace 120.976 ms, prepare 21.373 ms, weak_pages 18.733 ms, roots 11.708 ms, trim 9.980 ms, allocation_census 0.372 ms, finish 0.179 ms, accounting 0.153 ms, resume_world 0.121 ms, verify 0.026 ms, stop_world 0.014 ms
- ('fanout', 8, 'refill2048'): sweep_finalize 264.820 ms, trace 121.347 ms, prepare 17.879 ms, weak_pages 17.451 ms, roots 6.918 ms, trim 5.471 ms, allocation_census 0.460 ms, stop_world 0.222 ms, accounting 0.145 ms, finish 0.139 ms, resume_world 0.070 ms, verify 0.024 ms
- ('fanout', 8, 'refill256'): sweep_finalize 262.272 ms, trace 126.986 ms, trim 18.920 ms, prepare 18.109 ms, weak_pages 16.934 ms, roots 7.025 ms, allocation_census 0.464 ms, stop_world 0.244 ms, accounting 0.132 ms, finish 0.129 ms, resume_world 0.061 ms, verify 0.025 ms
- ('fanout', 8, 'refill512'): sweep_finalize 272.209 ms, trace 131.439 ms, prepare 19.275 ms, weak_pages 17.424 ms, roots 7.175 ms, trim 5.863 ms, stop_world 0.665 ms, allocation_census 0.459 ms, accounting 0.130 ms, finish 0.129 ms, resume_world 0.059 ms, verify 0.025 ms

Sweep work distribution (workload collections only):

Small helper jobs contain fewer than 4,096 objects; native thread creation is counted separately.
these counts alone do not measure thread-startup cost.

| Case | Workers | Variant | Collections / parallel | Helper jobs | New native threads | Small helper jobs | Median helper objects | Median largest-list share in parallel collections |
|---|---:|---|---:|---:|---:|---:|---:|---:|
| backpressure | 1 | refill2048 | 66 / 0 | 0 | 0 | 0 | n/a | n/a |
| backpressure | 1 | refill256 | 66 / 0 | 0 | 0 | 0 | n/a | n/a |
| backpressure | 1 | refill512 | 66 / 0 | 0 | 0 | 0 | n/a | n/a |
| backpressure | 8 | refill2048 | 66 / 66 | 462 | 462 | 3 | 17278 | 14.9% |
| backpressure | 8 | refill256 | 66 / 66 | 462 | 462 | 1 | 17132 | 14.6% |
| backpressure | 8 | refill512 | 66 / 66 | 462 | 462 | 0 | 17278 | 14.2% |
| balanced | 1 | refill2048 | 66 / 0 | 0 | 0 | 0 | n/a | n/a |
| balanced | 1 | refill256 | 66 / 0 | 0 | 0 | 0 | n/a | n/a |
| balanced | 1 | refill512 | 66 / 0 | 0 | 0 | 0 | n/a | n/a |
| balanced | 8 | refill2048 | 66 / 66 | 462 | 462 | 0 | 17103 | 14.4% |
| balanced | 8 | refill256 | 66 / 66 | 462 | 462 | 0 | 17204 | 14.6% |
| balanced | 8 | refill512 | 66 / 66 | 462 | 462 | 2 | 17274 | 14.7% |
| cpu | 1 | refill2048 | 66 / 0 | 0 | 0 | 0 | n/a | n/a |
| cpu | 1 | refill256 | 66 / 0 | 0 | 0 | 0 | n/a | n/a |
| cpu | 1 | refill512 | 66 / 0 | 0 | 0 | 0 | n/a | n/a |
| cpu | 8 | refill2048 | 66 / 66 | 462 | 462 | 0 | 13852 | 24.4% |
| cpu | 8 | refill256 | 66 / 66 | 462 | 462 | 0 | 11653 | 27.6% |
| cpu | 8 | refill512 | 66 / 66 | 462 | 462 | 1 | 13774 | 26.3% |
| fanout | 1 | refill2048 | 126 / 0 | 0 | 0 | 0 | n/a | n/a |
| fanout | 1 | refill256 | 126 / 0 | 0 | 0 | 0 | n/a | n/a |
| fanout | 1 | refill512 | 126 / 0 | 0 | 0 | 0 | n/a | n/a |
| fanout | 8 | refill2048 | 124 / 124 | 868 | 868 | 0 | 20467 | 13.8% |
| fanout | 8 | refill256 | 123 / 123 | 861 | 861 | 0 | 20377 | 13.6% |
| fanout | 8 | refill512 | 124 / 124 | 868 | 868 | 0 | 20386 | 13.7% |
