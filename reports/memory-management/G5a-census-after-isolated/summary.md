| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| buffers-empty | 1 | baseline | 12.202 [12.202, 12.202] | 0.968 | 46.8 |
| buffers-empty | 1 | diagnostic | 12.478 [12.478, 12.478] | 1.045 | 47.3 |
| buffers-empty | 1 | diagnostic-allocations | 24.383 [24.383, 24.383] | 2.116 | 53.2 |
| buffers-empty | 1 | diagnostic-off | 12.801 [12.801, 12.801] | 1.042 | 47.0 |
| buffers-empty | 8 | baseline | 12.249 [12.249, 12.249] | 1.016 | 46.5 |
| buffers-empty | 8 | diagnostic | 12.601 [12.601, 12.601] | 1.093 | 47.9 |
| buffers-empty | 8 | diagnostic-allocations | 24.534 [24.534, 24.534] | 2.097 | 53.3 |
| buffers-empty | 8 | diagnostic-off | 12.934 [12.934, 12.934] | 1.138 | 47.2 |
| buffers-grow | 1 | baseline | 27.374 [27.374, 27.374] | 1.126 | 40.3 |
| buffers-grow | 1 | diagnostic | 27.782 [27.782, 27.782] | 1.204 | 38.5 |
| buffers-grow | 1 | diagnostic-allocations | 30.069 [30.069, 30.069] | 1.647 | 39.4 |
| buffers-grow | 1 | diagnostic-off | 27.408 [27.408, 27.408] | 1.285 | 40.4 |
| buffers-grow | 8 | baseline | 27.824 [27.824, 27.824] | 1.035 | 38.3 |
| buffers-grow | 8 | diagnostic | 27.040 [27.040, 27.040] | 1.077 | 41.0 |
| buffers-grow | 8 | diagnostic-allocations | 30.520 [30.520, 30.520] | 1.753 | 39.8 |
| buffers-grow | 8 | diagnostic-off | 27.478 [27.478, 27.478] | 1.124 | 38.7 |
| buffers-pointers | 1 | baseline | 17.033 [17.033, 17.033] | 1.285 | 45.0 |
| buffers-pointers | 1 | diagnostic | 17.446 [17.446, 17.446] | 1.411 | 45.3 |
| buffers-pointers | 1 | diagnostic-allocations | 23.396 [23.396, 23.396] | 2.247 | 53.0 |
| buffers-pointers | 1 | diagnostic-off | 17.501 [17.501, 17.501] | 1.306 | 47.2 |
| buffers-pointers | 8 | baseline | 23.033 [23.033, 23.033] | 1.362 | 47.2 |
| buffers-pointers | 8 | diagnostic | 23.600 [23.600, 23.600] | 1.474 | 45.1 |
| buffers-pointers | 8 | diagnostic-allocations | 31.579 [31.579, 31.579] | 2.445 | 54.8 |
| buffers-pointers | 8 | diagnostic-off | 23.215 [23.215, 23.215] | 1.333 | 44.9 |
| messages-int-sustained | 1 | baseline | 82.989 [82.989, 82.989] | 3.549 | 38.9 |
| messages-int-sustained | 1 | diagnostic | 87.352 [87.352, 87.352] | 4.713 | 38.8 |
| messages-int-sustained | 1 | diagnostic-allocations | 92.534 [92.534, 92.534] | 5.035 | 42.3 |
| messages-int-sustained | 1 | diagnostic-off | 90.710 [90.710, 90.710] | 4.921 | 39.0 |
| messages-int-sustained | 8 | baseline | 91.156 [91.156, 91.156] | 4.129 | 45.4 |
| messages-int-sustained | 8 | diagnostic | 92.858 [92.858, 92.858] | 4.089 | 47.6 |
| messages-int-sustained | 8 | diagnostic-allocations | 97.575 [97.575, 97.575] | 5.056 | 49.0 |
| messages-int-sustained | 8 | diagnostic-off | 102.843 [102.843, 102.843] | 4.253 | 63.9 |

Diagnostic GC phases summed across measured repetitions (wall time):

- ('buffers-empty', 1, 'diagnostic'): sweep_finalize 1.006 ms, roots 0.014 ms, prepare 0.004 ms, resume_world 0.004 ms, trace 0.003 ms, allocation_census 0.003 ms, finish 0.001 ms, accounting 0.001 ms, trim 0.000 ms, stop_world 0.000 ms, verify 0.000 ms, weak_pages 0.000 ms
- ('buffers-empty', 1, 'diagnostic-allocations'): allocation_census 1.093 ms, sweep_finalize 0.975 ms, roots 0.015 ms, resume_world 0.010 ms, prepare 0.005 ms, trace 0.003 ms, finish 0.001 ms, accounting 0.001 ms, stop_world 0.000 ms, verify 0.000 ms, trim 0.000 ms, weak_pages 0.000 ms
- ('buffers-empty', 8, 'diagnostic'): sweep_finalize 1.059 ms, roots 0.006 ms, prepare 0.005 ms, allocation_census 0.003 ms, trace 0.003 ms, stop_world 0.003 ms, finish 0.001 ms, accounting 0.001 ms, resume_world 0.001 ms, trim 0.000 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('buffers-empty', 8, 'diagnostic-allocations'): allocation_census 1.086 ms, sweep_finalize 0.979 ms, roots 0.006 ms, prepare 0.005 ms, trace 0.003 ms, stop_world 0.002 ms, finish 0.001 ms, resume_world 0.001 ms, accounting 0.001 ms, trim 0.000 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('buffers-grow', 1, 'diagnostic'): sweep_finalize 1.158 ms, roots 0.007 ms, allocation_census 0.004 ms, prepare 0.004 ms, resume_world 0.004 ms, trace 0.004 ms, finish 0.001 ms, accounting 0.000 ms, trim 0.000 ms, verify 0.000 ms, weak_pages 0.000 ms, stop_world 0.000 ms
- ('buffers-grow', 1, 'diagnostic-allocations'): sweep_finalize 1.326 ms, allocation_census 0.233 ms, roots 0.007 ms, prepare 0.006 ms, resume_world 0.005 ms, trace 0.004 ms, accounting 0.003 ms, finish 0.001 ms, trim 0.000 ms, stop_world 0.000 ms, verify 0.000 ms, weak_pages 0.000 ms
- ('buffers-grow', 8, 'diagnostic'): sweep_finalize 1.042 ms, roots 0.005 ms, trace 0.004 ms, allocation_census 0.004 ms, prepare 0.003 ms, stop_world 0.002 ms, finish 0.001 ms, accounting 0.000 ms, resume_world 0.000 ms, weak_pages 0.000 ms, trim 0.000 ms, verify 0.000 ms
- ('buffers-grow', 8, 'diagnostic-allocations'): sweep_finalize 1.483 ms, allocation_census 0.220 ms, prepare 0.005 ms, roots 0.004 ms, trace 0.004 ms, finish 0.002 ms, stop_world 0.002 ms, accounting 0.001 ms, trim 0.000 ms, resume_world 0.000 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('buffers-pointers', 1, 'diagnostic'): sweep_finalize 1.357 ms, trace 0.009 ms, roots 0.008 ms, prepare 0.005 ms, resume_world 0.005 ms, allocation_census 0.002 ms, finish 0.001 ms, accounting 0.001 ms, weak_pages 0.001 ms, stop_world 0.000 ms, trim 0.000 ms, verify 0.000 ms
- ('buffers-pointers', 1, 'diagnostic-allocations'): sweep_finalize 1.207 ms, allocation_census 0.973 ms, trace 0.009 ms, roots 0.008 ms, resume_world 0.007 ms, prepare 0.006 ms, finish 0.001 ms, accounting 0.001 ms, verify 0.000 ms, stop_world 0.000 ms, trim 0.000 ms, weak_pages 0.000 ms
- ('buffers-pointers', 8, 'diagnostic'): sweep_finalize 1.419 ms, trace 0.009 ms, roots 0.006 ms, prepare 0.004 ms, stop_world 0.002 ms, allocation_census 0.002 ms, finish 0.001 ms, accounting 0.001 ms, resume_world 0.000 ms, trim 0.000 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('buffers-pointers', 8, 'diagnostic-allocations'): sweep_finalize 1.390 ms, allocation_census 0.994 ms, roots 0.016 ms, trace 0.011 ms, prepare 0.004 ms, stop_world 0.002 ms, finish 0.001 ms, accounting 0.001 ms, resume_world 0.001 ms, trim 0.000 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('messages-int-sustained', 1, 'diagnostic'): sweep_finalize 4.556 ms, roots 0.020 ms, trace 0.015 ms, allocation_census 0.011 ms, prepare 0.011 ms, resume_world 0.005 ms, finish 0.004 ms, accounting 0.002 ms, weak_pages 0.001 ms, stop_world 0.001 ms, trim 0.001 ms, verify 0.000 ms
- ('messages-int-sustained', 1, 'diagnostic-allocations'): sweep_finalize 4.186 ms, allocation_census 0.723 ms, roots 0.018 ms, trace 0.012 ms, prepare 0.009 ms, resume_world 0.004 ms, finish 0.003 ms, accounting 0.002 ms, stop_world 0.001 ms, weak_pages 0.001 ms, verify 0.000 ms, trim 0.000 ms
- ('messages-int-sustained', 8, 'diagnostic'): sweep_finalize 4.007 ms, trace 0.017 ms, roots 0.015 ms, allocation_census 0.012 ms, prepare 0.009 ms, stop_world 0.003 ms, finish 0.003 ms, accounting 0.002 ms, resume_world 0.001 ms, trim 0.001 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('messages-int-sustained', 8, 'diagnostic-allocations'): sweep_finalize 4.215 ms, allocation_census 0.769 ms, roots 0.017 ms, prepare 0.012 ms, trace 0.011 ms, finish 0.003 ms, stop_world 0.003 ms, accounting 0.002 ms, trim 0.001 ms, resume_world 0.001 ms, verify 0.001 ms, weak_pages 0.000 ms

Sweep work distribution (workload collections only):

Small helper jobs contain fewer than 4,096 objects; native thread creation is counted separately.
these counts alone do not measure thread-startup cost.

| Case | Workers | Variant | Collections / parallel | Helper jobs | New native threads | Small helper jobs | Median helper objects | Median largest-list share in parallel collections |
|---|---:|---|---:|---:|---:|---:|---:|---:|
| buffers-empty | 1 | diagnostic | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-empty | 1 | diagnostic-allocations | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-empty | 8 | diagnostic | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-empty | 8 | diagnostic-allocations | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-grow | 1 | diagnostic | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-grow | 1 | diagnostic-allocations | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-grow | 8 | diagnostic | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-grow | 8 | diagnostic-allocations | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-pointers | 1 | diagnostic | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-pointers | 1 | diagnostic-allocations | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-pointers | 8 | diagnostic | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-pointers | 8 | diagnostic-allocations | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| messages-int-sustained | 1 | diagnostic | 4 / 0 | 0 | 0 | 0 | n/a | n/a |
| messages-int-sustained | 1 | diagnostic-allocations | 4 / 0 | 0 | 0 | 0 | n/a | n/a |
| messages-int-sustained | 8 | diagnostic | 4 / 1 | 1 | 1 | 0 | 52733 | 99.8% |
| messages-int-sustained | 8 | diagnostic-allocations | 4 / 1 | 1 | 1 | 0 | 52733 | 99.8% |
