| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| buffers-empty | 1 | baseline | 26.171 [26.171, 26.171] | 6.477 | 44.4 |
| buffers-empty | 1 | diagnostic | 24.967 [24.967, 24.967] | 6.846 | 44.5 |
| buffers-empty | 1 | diagnostic-allocations | 35.726 [35.726, 35.726] | 8.396 | 50.4 |
| buffers-empty | 1 | diagnostic-off | 24.515 [24.515, 24.515] | 6.885 | 46.5 |
| buffers-empty | 8 | baseline | 24.340 [24.340, 24.340] | 6.541 | 43.9 |
| buffers-empty | 8 | diagnostic | 24.944 [24.944, 24.944] | 6.821 | 46.8 |
| buffers-empty | 8 | diagnostic-allocations | 35.305 [35.305, 35.305] | 8.234 | 48.6 |
| buffers-empty | 8 | diagnostic-off | 24.750 [24.750, 24.750] | 6.754 | 44.6 |
| buffers-grow | 1 | baseline | 30.820 [30.820, 30.820] | 1.704 | 38.6 |
| buffers-grow | 1 | diagnostic | 30.181 [30.181, 30.181] | 1.782 | 39.7 |
| buffers-grow | 1 | diagnostic-allocations | 31.538 [31.538, 31.538] | 1.983 | 40.4 |
| buffers-grow | 1 | diagnostic-off | 33.252 [33.252, 33.252] | 1.757 | 40.0 |
| buffers-grow | 8 | baseline | 30.674 [30.674, 30.674] | 1.722 | 39.3 |
| buffers-grow | 8 | diagnostic | 30.410 [30.410, 30.410] | 1.782 | 39.5 |
| buffers-grow | 8 | diagnostic-allocations | 31.644 [31.644, 31.644] | 1.963 | 40.5 |
| buffers-grow | 8 | diagnostic-off | 30.418 [30.418, 30.418] | 1.765 | 39.8 |
| buffers-pointers | 1 | baseline | 17.660 [17.660, 17.660] | 1.389 | 43.2 |
| buffers-pointers | 1 | diagnostic | 18.013 [18.013, 18.013] | 1.458 | 43.0 |
| buffers-pointers | 1 | diagnostic-allocations | 24.121 [24.121, 24.121] | 2.344 | 50.7 |
| buffers-pointers | 1 | diagnostic-off | 17.847 [17.847, 17.847] | 1.459 | 43.3 |
| buffers-pointers | 8 | baseline | 25.475 [25.475, 25.475] | 1.518 | 43.2 |
| buffers-pointers | 8 | diagnostic | 23.458 [23.458, 23.458] | 1.486 | 43.5 |
| buffers-pointers | 8 | diagnostic-allocations | 30.928 [30.928, 30.928] | 2.420 | 52.8 |
| buffers-pointers | 8 | diagnostic-off | 23.672 [23.672, 23.672] | 1.435 | 43.4 |
| messages-int-sustained | 1 | baseline | 99.524 [99.524, 99.524] | 9.213 | 39.6 |
| messages-int-sustained | 1 | diagnostic | 98.340 [98.340, 98.340] | 8.491 | 41.9 |
| messages-int-sustained | 1 | diagnostic-allocations | 100.851 [100.851, 100.851] | 9.243 | 40.7 |
| messages-int-sustained | 1 | diagnostic-off | 93.744 [93.744, 93.744] | 8.699 | 39.6 |
| messages-int-sustained | 8 | baseline | 111.567 [111.567, 111.567] | 7.559 | 65.1 |
| messages-int-sustained | 8 | diagnostic | 112.043 [112.043, 112.043] | 7.674 | 66.8 |
| messages-int-sustained | 8 | diagnostic-allocations | 102.776 [102.776, 102.776] | 8.452 | 47.2 |
| messages-int-sustained | 8 | diagnostic-off | 101.557 [101.557, 101.557] | 8.347 | 48.4 |

Diagnostic GC phases summed across measured repetitions (wall time):

- ('buffers-empty', 1, 'diagnostic'): sweep_finalize 6.738 ms, allocation_census 0.015 ms, roots 0.012 ms, resume_world 0.007 ms, prepare 0.006 ms, trace 0.002 ms, finish 0.002 ms, accounting 0.001 ms, stop_world 0.001 ms, trim 0.000 ms, verify 0.000 ms, weak_pages 0.000 ms
- ('buffers-empty', 1, 'diagnostic-allocations'): sweep_finalize 6.818 ms, allocation_census 1.428 ms, roots 0.012 ms, resume_world 0.010 ms, prepare 0.007 ms, finish 0.003 ms, trace 0.003 ms, accounting 0.002 ms, stop_world 0.001 ms, weak_pages 0.000 ms, trim 0.000 ms, verify 0.000 ms
- ('buffers-empty', 8, 'diagnostic'): sweep_finalize 6.707 ms, allocation_census 0.022 ms, roots 0.008 ms, prepare 0.006 ms, trace 0.002 ms, stop_world 0.002 ms, finish 0.002 ms, accounting 0.002 ms, verify 0.000 ms, resume_world 0.000 ms, weak_pages 0.000 ms, trim 0.000 ms
- ('buffers-empty', 8, 'diagnostic-allocations'): sweep_finalize 6.609 ms, allocation_census 1.529 ms, roots 0.008 ms, prepare 0.006 ms, stop_world 0.003 ms, trace 0.003 ms, finish 0.002 ms, accounting 0.002 ms, resume_world 0.001 ms, trim 0.000 ms, verify 0.000 ms, weak_pages 0.000 ms
- ('buffers-grow', 1, 'diagnostic'): sweep_finalize 1.733 ms, roots 0.008 ms, resume_world 0.006 ms, prepare 0.004 ms, allocation_census 0.004 ms, trace 0.004 ms, finish 0.001 ms, accounting 0.001 ms, weak_pages 0.000 ms, trim 0.000 ms, stop_world 0.000 ms, verify 0.000 ms
- ('buffers-grow', 1, 'diagnostic-allocations'): sweep_finalize 1.745 ms, allocation_census 0.171 ms, roots 0.012 ms, trace 0.007 ms, resume_world 0.007 ms, prepare 0.005 ms, finish 0.001 ms, accounting 0.001 ms, stop_world 0.000 ms, weak_pages 0.000 ms, trim 0.000 ms, verify 0.000 ms
- ('buffers-grow', 8, 'diagnostic'): sweep_finalize 1.720 ms, prepare 0.005 ms, roots 0.004 ms, trace 0.004 ms, stop_world 0.002 ms, allocation_census 0.002 ms, finish 0.001 ms, accounting 0.001 ms, resume_world 0.000 ms, trim 0.000 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('buffers-grow', 8, 'diagnostic-allocations'): sweep_finalize 1.713 ms, allocation_census 0.177 ms, prepare 0.005 ms, roots 0.005 ms, trace 0.004 ms, stop_world 0.002 ms, finish 0.001 ms, accounting 0.001 ms, trim 0.000 ms, verify 0.000 ms, weak_pages 0.000 ms, resume_world 0.000 ms
- ('buffers-pointers', 1, 'diagnostic'): sweep_finalize 1.393 ms, trace 0.009 ms, roots 0.008 ms, resume_world 0.006 ms, prepare 0.004 ms, allocation_census 0.002 ms, finish 0.001 ms, accounting 0.001 ms, trim 0.000 ms, stop_world 0.000 ms, verify 0.000 ms, weak_pages 0.000 ms
- ('buffers-pointers', 1, 'diagnostic-allocations'): sweep_finalize 1.354 ms, allocation_census 0.930 ms, resume_world 0.009 ms, trace 0.009 ms, roots 0.008 ms, prepare 0.005 ms, finish 0.001 ms, accounting 0.001 ms, stop_world 0.000 ms, trim 0.000 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('buffers-pointers', 8, 'diagnostic'): sweep_finalize 1.427 ms, prepare 0.010 ms, trace 0.009 ms, roots 0.008 ms, stop_world 0.004 ms, allocation_census 0.002 ms, finish 0.001 ms, accounting 0.001 ms, resume_world 0.001 ms, trim 0.000 ms, verify 0.000 ms, weak_pages 0.000 ms
- ('buffers-pointers', 8, 'diagnostic-allocations'): sweep_finalize 1.422 ms, allocation_census 0.944 ms, trace 0.010 ms, roots 0.006 ms, prepare 0.005 ms, stop_world 0.002 ms, finish 0.001 ms, accounting 0.001 ms, resume_world 0.000 ms, trim 0.000 ms, verify 0.000 ms, weak_pages 0.000 ms
- ('messages-int-sustained', 1, 'diagnostic'): sweep_finalize 8.277 ms, trace 0.021 ms, roots 0.019 ms, allocation_census 0.017 ms, prepare 0.011 ms, resume_world 0.007 ms, finish 0.004 ms, accounting 0.003 ms, trim 0.001 ms, verify 0.001 ms, weak_pages 0.000 ms, stop_world 0.000 ms
- ('messages-int-sustained', 1, 'diagnostic-allocations'): sweep_finalize 8.213 ms, allocation_census 0.794 ms, roots 0.021 ms, trace 0.017 ms, prepare 0.011 ms, resume_world 0.009 ms, finish 0.005 ms, accounting 0.003 ms, trim 0.001 ms, verify 0.001 ms, weak_pages 0.000 ms, stop_world 0.000 ms
- ('messages-int-sustained', 8, 'diagnostic'): sweep_finalize 7.565 ms, roots 0.019 ms, allocation_census 0.019 ms, trace 0.016 ms, prepare 0.011 ms, finish 0.005 ms, accounting 0.005 ms, stop_world 0.004 ms, resume_world 0.001 ms, trim 0.001 ms, verify 0.001 ms, weak_pages 0.001 ms
- ('messages-int-sustained', 8, 'diagnostic-allocations'): sweep_finalize 7.535 ms, allocation_census 0.820 ms, roots 0.020 ms, trace 0.019 ms, prepare 0.012 ms, finish 0.005 ms, stop_world 0.004 ms, accounting 0.004 ms, resume_world 0.002 ms, trim 0.001 ms, verify 0.001 ms, weak_pages 0.001 ms

Sweep work distribution (workload collections only):

Small helper jobs contain fewer than 4,096 objects; native thread creation is counted separately.
these counts alone do not measure thread-startup cost.

| Case | Workers | Variant | Collections / parallel | Helper jobs | New native threads | Small helper jobs | Median helper objects | Median largest-list share in parallel collections |
|---|---:|---|---:|---:|---:|---:|---:|---:|
| buffers-empty | 1 | diagnostic | 2 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-empty | 1 | diagnostic-allocations | 2 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-empty | 8 | diagnostic | 2 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-empty | 8 | diagnostic-allocations | 2 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-grow | 1 | diagnostic | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-grow | 1 | diagnostic-allocations | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-grow | 8 | diagnostic | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-grow | 8 | diagnostic-allocations | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-pointers | 1 | diagnostic | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-pointers | 1 | diagnostic-allocations | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-pointers | 8 | diagnostic | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-pointers | 8 | diagnostic-allocations | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| messages-int-sustained | 1 | diagnostic | 5 / 0 | 0 | 0 | 0 | n/a | n/a |
| messages-int-sustained | 1 | diagnostic-allocations | 5 / 0 | 0 | 0 | 0 | n/a | n/a |
| messages-int-sustained | 8 | diagnostic | 5 / 2 | 2 | 2 | 1 | 25690 | 99.9% |
| messages-int-sustained | 8 | diagnostic-allocations | 5 / 1 | 1 | 1 | 0 | 51356 | 99.8% |
