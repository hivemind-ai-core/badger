| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 98.618 [97.862, 102.176] | 13.051 | 50.2 |
| backpressure | 1 | diagnostic | 101.594 [97.821, 102.199] | 14.405 | 51.0 |
| backpressure | 1 | diagnostic-off | 99.429 [98.111, 107.390] | 13.874 | 50.8 |
| backpressure | 2 | baseline | 69.368 [67.213, 72.013] | 12.492 | 57.9 |
| backpressure | 2 | diagnostic | 71.475 [70.312, 75.472] | 13.591 | 58.1 |
| backpressure | 2 | diagnostic-off | 69.961 [69.031, 71.337] | 13.066 | 56.2 |
| backpressure | 4 | baseline | 42.897 [42.064, 43.552] | 9.292 | 64.3 |
| backpressure | 4 | diagnostic | 44.787 [43.120, 46.769] | 9.723 | 68.7 |
| backpressure | 4 | diagnostic-off | 44.582 [42.347, 46.012] | 9.675 | 70.1 |
| backpressure | 8 | baseline | 30.470 [28.806, 34.264] | 7.905 | 74.9 |
| backpressure | 8 | diagnostic | 30.335 [29.424, 33.882] | 7.591 | 75.2 |
| backpressure | 8 | diagnostic-off | 32.435 [32.380, 32.823] | 8.063 | 79.1 |
| balanced | 1 | baseline | 99.217 [97.429, 109.628] | 12.674 | 62.5 |
| balanced | 1 | diagnostic | 101.187 [101.184, 110.652] | 13.378 | 62.9 |
| balanced | 1 | diagnostic-off | 98.409 [97.510, 103.184] | 13.704 | 62.7 |
| balanced | 2 | baseline | 70.373 [61.242, 71.965] | 10.620 | 69.3 |
| balanced | 2 | diagnostic | 70.302 [69.808, 83.855] | 12.149 | 71.8 |
| balanced | 2 | diagnostic-off | 64.846 [64.259, 73.720] | 10.904 | 69.8 |
| balanced | 4 | baseline | 40.773 [38.118, 42.175] | 8.746 | 69.3 |
| balanced | 4 | diagnostic | 46.654 [42.913, 53.900] | 9.564 | 77.8 |
| balanced | 4 | diagnostic-off | 41.054 [40.466, 42.863] | 8.739 | 75.7 |
| balanced | 8 | baseline | 29.856 [29.419, 52.146] | 7.872 | 87.0 |
| balanced | 8 | diagnostic | 31.282 [29.349, 36.265] | 9.528 | 85.7 |
| balanced | 8 | diagnostic-off | 29.788 [28.937, 31.713] | 7.922 | 86.4 |
| cpu | 1 | baseline | 206.815 [205.930, 211.834] | 12.832 | 59.2 |
| cpu | 1 | diagnostic | 218.061 [206.553, 220.551] | 13.492 | 59.6 |
| cpu | 1 | diagnostic-off | 208.790 [208.202, 214.199] | 12.780 | 60.0 |
| cpu | 2 | baseline | 121.348 [119.089, 127.480] | 10.993 | 63.7 |
| cpu | 2 | diagnostic | 119.407 [113.578, 124.947] | 11.561 | 64.7 |
| cpu | 2 | diagnostic-off | 124.869 [116.696, 134.265] | 11.161 | 64.3 |
| cpu | 4 | baseline | 65.004 [61.916, 69.509] | 8.472 | 76.4 |
| cpu | 4 | diagnostic | 65.970 [64.436, 67.247] | 9.549 | 72.8 |
| cpu | 4 | diagnostic-off | 66.179 [64.961, 66.683] | 8.758 | 74.7 |
| cpu | 8 | baseline | 62.274 [57.430, 62.422] | 7.950 | 87.5 |
| cpu | 8 | diagnostic | 58.983 [57.026, 68.079] | 8.714 | 91.1 |
| cpu | 8 | diagnostic-off | 62.736 [56.352, 65.436] | 8.563 | 89.2 |
| fanout | 1 | baseline | 281.983 [271.584, 283.568] | 40.079 | 80.1 |
| fanout | 1 | diagnostic | 293.918 [284.711, 300.801] | 42.638 | 80.6 |
| fanout | 1 | diagnostic-off | 291.507 [286.229, 310.462] | 42.926 | 80.0 |
| fanout | 2 | baseline | 223.291 [204.658, 263.636] | 37.633 | 93.5 |
| fanout | 2 | diagnostic | 234.368 [193.264, 248.156] | 37.288 | 93.4 |
| fanout | 2 | diagnostic-off | 210.638 [200.086, 254.249] | 38.941 | 92.0 |
| fanout | 4 | baseline | 170.360 [169.579, 173.704] | 31.644 | 104.9 |
| fanout | 4 | diagnostic | 191.360 [182.530, 202.929] | 34.161 | 107.1 |
| fanout | 4 | diagnostic-off | 189.930 [176.255, 192.821] | 32.199 | 108.2 |
| fanout | 8 | baseline | 190.220 [185.025, 197.192] | 29.049 | 100.3 |
| fanout | 8 | diagnostic | 192.624 [192.355, 196.411] | 29.687 | 101.8 |
| fanout | 8 | diagnostic-off | 207.793 [194.213, 209.056] | 30.887 | 99.1 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 12288 | 0.896 | 4.352 |
| backpressure | 1 | diagnostic | 12288 | 0.928 | 4.352 |
| backpressure | 1 | diagnostic-off | 12288 | 0.928 | 4.608 |
| backpressure | 2 | baseline | 12288 | 0.464 | 3.328 |
| backpressure | 2 | diagnostic | 12288 | 0.480 | 3.712 |
| backpressure | 2 | diagnostic-off | 12288 | 0.464 | 3.456 |
| backpressure | 4 | baseline | 12288 | 0.256 | 2.432 |
| backpressure | 4 | diagnostic | 12288 | 0.272 | 2.432 |
| backpressure | 4 | diagnostic-off | 12288 | 0.272 | 2.560 |
| backpressure | 8 | baseline | 12288 | 0.168 | 2.048 |
| backpressure | 8 | diagnostic | 12288 | 0.168 | 1.984 |
| backpressure | 8 | diagnostic-off | 12288 | 0.176 | 2.048 |
| balanced | 1 | baseline | 12288 | 13.312 | 19.456 |
| balanced | 1 | diagnostic | 12288 | 13.824 | 19.456 |
| balanced | 1 | diagnostic-off | 12288 | 13.312 | 18.432 |
| balanced | 2 | baseline | 12288 | 8.704 | 15.872 |
| balanced | 2 | diagnostic | 12288 | 9.728 | 15.872 |
| balanced | 2 | diagnostic-off | 12288 | 9.216 | 13.312 |
| balanced | 4 | baseline | 12288 | 5.632 | 7.680 |
| balanced | 4 | diagnostic | 12288 | 5.888 | 12.800 |
| balanced | 4 | diagnostic-off | 12288 | 5.632 | 7.680 |
| balanced | 8 | baseline | 12288 | 4.096 | 9.216 |
| balanced | 8 | diagnostic | 12288 | 3.968 | 7.680 |
| balanced | 8 | diagnostic-off | 12288 | 3.968 | 5.376 |
| cpu | 1 | baseline | 12288 | 28.672 | 34.816 |
| cpu | 1 | diagnostic | 12288 | 28.672 | 36.864 |
| cpu | 1 | diagnostic-off | 12288 | 28.672 | 34.816 |
| cpu | 2 | baseline | 12288 | 16.384 | 25.600 |
| cpu | 2 | diagnostic | 12288 | 16.384 | 25.600 |
| cpu | 2 | diagnostic-off | 12288 | 16.384 | 26.624 |
| cpu | 4 | baseline | 12288 | 9.216 | 12.288 |
| cpu | 4 | diagnostic | 12288 | 9.216 | 11.264 |
| cpu | 4 | diagnostic-off | 12288 | 9.216 | 11.776 |
| cpu | 8 | baseline | 12288 | 8.192 | 11.776 |
| cpu | 8 | diagnostic | 12288 | 7.936 | 11.776 |
| cpu | 8 | diagnostic-off | 12288 | 8.192 | 12.288 |
| fanout | 1 | baseline | 12288 | 36.864 | 53.248 |
| fanout | 1 | diagnostic | 12288 | 38.912 | 55.296 |
| fanout | 1 | diagnostic-off | 12288 | 38.912 | 63.488 |
| fanout | 2 | baseline | 12288 | 30.720 | 51.200 |
| fanout | 2 | diagnostic | 12288 | 29.696 | 47.104 |
| fanout | 2 | diagnostic-off | 12288 | 28.672 | 43.008 |
| fanout | 4 | baseline | 12288 | 21.504 | 31.744 |
| fanout | 4 | diagnostic | 12288 | 24.576 | 43.008 |
| fanout | 4 | diagnostic-off | 12288 | 22.528 | 49.152 |
| fanout | 8 | baseline | 12288 | 24.576 | 38.912 |
| fanout | 8 | diagnostic | 12288 | 24.576 | 38.912 |
| fanout | 8 | diagnostic-off | 12288 | 26.624 | 40.960 |

Diagnostic GC phases summed across measured repetitions (wall time):

- ('backpressure', 1, 'diagnostic'): sweep_finalize 40.443 ms, trace 2.029 ms, roots 0.150 ms, prepare 0.037 ms, allocation_census 0.029 ms, resume_world 0.024 ms, finish 0.016 ms, accounting 0.013 ms, trim 0.003 ms, verify 0.003 ms, weak_pages 0.002 ms, stop_world 0.002 ms
- ('backpressure', 2, 'diagnostic'): sweep_finalize 37.968 ms, trace 2.270 ms, stop_world 0.193 ms, roots 0.168 ms, prepare 0.060 ms, allocation_census 0.046 ms, finish 0.018 ms, accounting 0.017 ms, resume_world 0.007 ms, trim 0.004 ms, weak_pages 0.003 ms, verify 0.003 ms
- ('backpressure', 4, 'diagnostic'): sweep_finalize 27.120 ms, trace 2.145 ms, roots 0.168 ms, stop_world 0.125 ms, prepare 0.047 ms, allocation_census 0.042 ms, finish 0.015 ms, accounting 0.015 ms, resume_world 0.005 ms, trim 0.003 ms, weak_pages 0.003 ms, verify 0.002 ms
- ('backpressure', 8, 'diagnostic'): sweep_finalize 21.374 ms, trace 2.153 ms, roots 0.167 ms, allocation_census 0.069 ms, prepare 0.053 ms, stop_world 0.019 ms, accounting 0.014 ms, finish 0.014 ms, resume_world 0.005 ms, trim 0.004 ms, weak_pages 0.003 ms, verify 0.002 ms
- ('balanced', 1, 'diagnostic'): sweep_finalize 32.597 ms, trace 8.690 ms, roots 0.155 ms, allocation_census 0.065 ms, prepare 0.042 ms, resume_world 0.031 ms, finish 0.014 ms, accounting 0.013 ms, weak_pages 0.005 ms, trim 0.003 ms, verify 0.003 ms, stop_world 0.002 ms
- ('balanced', 2, 'diagnostic'): sweep_finalize 26.221 ms, trace 9.152 ms, roots 0.173 ms, allocation_census 0.081 ms, prepare 0.052 ms, stop_world 0.043 ms, finish 0.017 ms, accounting 0.016 ms, resume_world 0.006 ms, weak_pages 0.004 ms, trim 0.003 ms, verify 0.002 ms
- ('balanced', 4, 'diagnostic'): sweep_finalize 21.135 ms, trace 8.221 ms, roots 0.198 ms, allocation_census 0.086 ms, stop_world 0.079 ms, prepare 0.044 ms, finish 0.014 ms, accounting 0.014 ms, resume_world 0.005 ms, trim 0.003 ms, weak_pages 0.003 ms, verify 0.002 ms
- ('balanced', 8, 'diagnostic'): sweep_finalize 16.296 ms, trace 10.441 ms, roots 0.219 ms, allocation_census 0.098 ms, prepare 0.091 ms, stop_world 0.043 ms, finish 0.013 ms, accounting 0.012 ms, resume_world 0.005 ms, weak_pages 0.004 ms, trim 0.003 ms, verify 0.003 ms
- ('cpu', 1, 'diagnostic'): sweep_finalize 31.514 ms, trace 8.112 ms, roots 0.215 ms, allocation_census 0.076 ms, prepare 0.042 ms, resume_world 0.019 ms, finish 0.013 ms, accounting 0.011 ms, weak_pages 0.004 ms, trim 0.003 ms, verify 0.003 ms, stop_world 0.002 ms
- ('cpu', 2, 'diagnostic'): sweep_finalize 25.828 ms, trace 7.924 ms, roots 0.223 ms, allocation_census 0.069 ms, prepare 0.044 ms, stop_world 0.026 ms, finish 0.016 ms, accounting 0.014 ms, resume_world 0.006 ms, trim 0.004 ms, weak_pages 0.003 ms, verify 0.002 ms
- ('cpu', 4, 'diagnostic'): sweep_finalize 20.273 ms, trace 8.131 ms, roots 0.202 ms, allocation_census 0.083 ms, stop_world 0.067 ms, prepare 0.050 ms, finish 0.013 ms, accounting 0.013 ms, resume_world 0.005 ms, weak_pages 0.003 ms, trim 0.003 ms, verify 0.003 ms
- ('cpu', 8, 'diagnostic'): sweep_finalize 16.912 ms, trace 8.241 ms, roots 0.265 ms, stop_world 0.085 ms, allocation_census 0.071 ms, prepare 0.055 ms, finish 0.012 ms, accounting 0.011 ms, resume_world 0.005 ms, trim 0.004 ms, weak_pages 0.003 ms, verify 0.003 ms
- ('fanout', 1, 'diagnostic'): sweep_finalize 94.052 ms, trace 21.092 ms, prepare 3.759 ms, weak_pages 3.405 ms, trim 1.750 ms, roots 1.733 ms, allocation_census 0.067 ms, resume_world 0.037 ms, finish 0.031 ms, accounting 0.024 ms, verify 0.006 ms, stop_world 0.002 ms
- ('fanout', 2, 'diagnostic'): sweep_finalize 79.944 ms, trace 22.248 ms, prepare 3.718 ms, weak_pages 3.094 ms, roots 1.613 ms, trim 1.073 ms, allocation_census 0.088 ms, stop_world 0.052 ms, finish 0.031 ms, accounting 0.028 ms, resume_world 0.013 ms, verify 0.005 ms
- ('fanout', 4, 'diagnostic'): sweep_finalize 68.441 ms, trace 23.256 ms, prepare 3.896 ms, weak_pages 3.447 ms, roots 1.572 ms, trim 1.158 ms, allocation_census 0.132 ms, stop_world 0.104 ms, finish 0.032 ms, accounting 0.030 ms, resume_world 0.016 ms, verify 0.005 ms
- ('fanout', 8, 'diagnostic'): sweep_finalize 56.626 ms, trace 21.918 ms, prepare 3.553 ms, weak_pages 3.364 ms, roots 1.456 ms, trim 1.341 ms, allocation_census 0.132 ms, stop_world 0.063 ms, accounting 0.030 ms, finish 0.028 ms, resume_world 0.012 ms, verify 0.005 ms

Sweep work distribution (workload collections only):

Small helper jobs contain fewer than 4,096 objects; native thread creation is counted separately.
these counts alone do not measure thread-startup cost.

| Case | Workers | Variant | Collections / parallel | Helper jobs | New native threads | Small helper jobs | Median helper objects | Median largest-list share in parallel collections |
|---|---:|---|---:|---:|---:|---:|---:|---:|
| backpressure | 1 | diagnostic | 15 / 0 | 0 | 0 | 0 | n/a | n/a |
| backpressure | 2 | diagnostic | 15 / 15 | 15 | 15 | 0 | 62635 | 51.4% |
| backpressure | 4 | diagnostic | 15 / 15 | 45 | 45 | 0 | 32265 | 28.1% |
| backpressure | 8 | diagnostic | 15 / 15 | 105 | 105 | 0 | 16117 | 14.2% |
| balanced | 1 | diagnostic | 15 / 0 | 0 | 0 | 0 | n/a | n/a |
| balanced | 2 | diagnostic | 15 / 15 | 15 | 15 | 0 | 61038 | 55.7% |
| balanced | 4 | diagnostic | 15 / 15 | 45 | 45 | 2 | 30563 | 29.1% |
| balanced | 8 | diagnostic | 15 / 15 | 105 | 105 | 0 | 15664 | 15.7% |
| cpu | 1 | diagnostic | 15 / 0 | 0 | 0 | 0 | n/a | n/a |
| cpu | 2 | diagnostic | 15 / 15 | 15 | 15 | 0 | 65394 | 53.3% |
| cpu | 4 | diagnostic | 15 / 15 | 45 | 45 | 0 | 33261 | 29.4% |
| cpu | 8 | diagnostic | 15 / 15 | 105 | 105 | 0 | 15327 | 18.8% |
| fanout | 1 | diagnostic | 30 / 0 | 0 | 0 | 0 | n/a | n/a |
| fanout | 2 | diagnostic | 30 / 30 | 30 | 30 | 0 | 78183 | 51.9% |
| fanout | 4 | diagnostic | 30 / 30 | 90 | 90 | 4 | 38374 | 29.9% |
| fanout | 8 | diagnostic | 30 / 30 | 210 | 210 | 0 | 19858 | 14.1% |
