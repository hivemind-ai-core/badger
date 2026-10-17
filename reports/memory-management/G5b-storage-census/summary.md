| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| buffers-capacity-32 | 1 | inline0 | 27.008 [27.008, 27.008] | 2.017 | 40.1 |
| buffers-capacity-32 | 1 | inline256 | 20.153 [20.153, 20.153] | 1.218 | 44.3 |
| buffers-capacity-32 | 8 | inline0 | 27.089 [27.089, 27.089] | 2.038 | 40.4 |
| buffers-capacity-32 | 8 | inline256 | 22.119 [22.119, 22.119] | 1.011 | 43.9 |
| buffers-capacity-grow | 1 | inline0 | 54.170 [54.170, 54.170] | 2.602 | 39.2 |
| buffers-capacity-grow | 1 | inline256 | 53.464 [53.464, 53.464] | 3.180 | 40.1 |
| buffers-capacity-grow | 8 | inline0 | 62.138 [62.138, 62.138] | 3.019 | 39.9 |
| buffers-capacity-grow | 8 | inline256 | 55.615 [55.615, 55.615] | 3.043 | 38.6 |
| buffers-capacity-pointers | 1 | inline0 | 25.818 [25.818, 25.818] | 2.671 | 52.8 |
| buffers-capacity-pointers | 1 | inline256 | 24.161 [24.161, 24.161] | 2.128 | 55.2 |
| buffers-capacity-pointers | 8 | inline0 | 30.317 [30.317, 30.317] | 2.454 | 53.0 |
| buffers-capacity-pointers | 8 | inline256 | 28.987 [28.987, 28.987] | 2.041 | 57.2 |
| messages-int-sustained | 1 | inline0 | 121.970 [121.970, 121.970] | 9.387 | 42.7 |
| messages-int-sustained | 1 | inline256 | 92.206 [92.206, 92.206] | 3.882 | 46.7 |
| messages-int-sustained | 8 | inline0 | 109.467 [109.467, 109.467] | 7.870 | 47.2 |
| messages-int-sustained | 8 | inline256 | 88.777 [88.777, 88.777] | 4.183 | 48.2 |

Diagnostic GC phases summed across measured repetitions (wall time):

- ('buffers-capacity-32', 1, 'inline0'): sweep_finalize 1.697 ms, allocation_census 0.239 ms, resume_world 0.011 ms, roots 0.011 ms, trace 0.009 ms, prepare 0.005 ms, finish 0.001 ms, accounting 0.001 ms, trim 0.000 ms, stop_world 0.000 ms, verify 0.000 ms, weak_pages 0.000 ms
- ('buffers-capacity-32', 1, 'inline256'): sweep_finalize 0.598 ms, allocation_census 0.567 ms, roots 0.011 ms, trace 0.009 ms, resume_world 0.007 ms, prepare 0.007 ms, finish 0.002 ms, accounting 0.001 ms, trim 0.000 ms, verify 0.000 ms, weak_pages 0.000 ms, stop_world 0.000 ms
- ('buffers-capacity-32', 8, 'inline0'): sweep_finalize 1.709 ms, allocation_census 0.251 ms, trace 0.011 ms, roots 0.007 ms, prepare 0.006 ms, stop_world 0.002 ms, finish 0.002 ms, accounting 0.001 ms, resume_world 0.000 ms, trim 0.000 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('buffers-capacity-32', 8, 'inline256'): allocation_census 0.505 ms, sweep_finalize 0.466 ms, trace 0.007 ms, roots 0.007 ms, prepare 0.006 ms, stop_world 0.003 ms, finish 0.001 ms, accounting 0.001 ms, resume_world 0.001 ms, trim 0.000 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('buffers-capacity-grow', 1, 'inline0'): sweep_finalize 2.120 ms, allocation_census 0.287 ms, roots 0.023 ms, prepare 0.017 ms, trace 0.015 ms, resume_world 0.007 ms, finish 0.004 ms, accounting 0.003 ms, weak_pages 0.001 ms, trim 0.001 ms, stop_world 0.001 ms, verify 0.001 ms
- ('buffers-capacity-grow', 1, 'inline256'): sweep_finalize 2.397 ms, allocation_census 0.570 ms, trace 0.021 ms, roots 0.019 ms, prepare 0.016 ms, resume_world 0.007 ms, finish 0.006 ms, accounting 0.003 ms, stop_world 0.001 ms, weak_pages 0.001 ms, trim 0.001 ms, verify 0.001 ms
- ('buffers-capacity-grow', 8, 'inline0'): sweep_finalize 2.450 ms, allocation_census 0.317 ms, roots 0.021 ms, trace 0.020 ms, prepare 0.015 ms, stop_world 0.006 ms, finish 0.005 ms, accounting 0.003 ms, resume_world 0.001 ms, trim 0.001 ms, weak_pages 0.001 ms, verify 0.001 ms
- ('buffers-capacity-grow', 8, 'inline256'): sweep_finalize 2.251 ms, allocation_census 0.574 ms, trace 0.023 ms, roots 0.019 ms, prepare 0.015 ms, stop_world 0.005 ms, finish 0.005 ms, accounting 0.003 ms, resume_world 0.002 ms, trim 0.001 ms, weak_pages 0.001 ms, verify 0.001 ms
- ('buffers-capacity-pointers', 1, 'inline0'): sweep_finalize 1.492 ms, allocation_census 1.101 ms, roots 0.015 ms, trace 0.010 ms, prepare 0.007 ms, resume_world 0.006 ms, finish 0.001 ms, accounting 0.001 ms, stop_world 0.000 ms, trim 0.000 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('buffers-capacity-pointers', 1, 'inline256'): allocation_census 1.206 ms, sweep_finalize 0.862 ms, trace 0.014 ms, resume_world 0.011 ms, roots 0.010 ms, prepare 0.007 ms, finish 0.001 ms, accounting 0.001 ms, verify 0.000 ms, trim 0.000 ms, weak_pages 0.000 ms, stop_world 0.000 ms
- ('buffers-capacity-pointers', 8, 'inline0'): sweep_finalize 1.375 ms, allocation_census 1.017 ms, roots 0.009 ms, trace 0.009 ms, prepare 0.005 ms, stop_world 0.002 ms, accounting 0.001 ms, finish 0.001 ms, resume_world 0.000 ms, trim 0.000 ms, verify 0.000 ms, weak_pages 0.000 ms
- ('buffers-capacity-pointers', 8, 'inline256'): allocation_census 1.174 ms, sweep_finalize 0.823 ms, trace 0.013 ms, roots 0.006 ms, prepare 0.005 ms, stop_world 0.002 ms, finish 0.001 ms, resume_world 0.001 ms, accounting 0.001 ms, trim 0.000 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('messages-int-sustained', 1, 'inline0'): sweep_finalize 7.897 ms, allocation_census 1.071 ms, roots 0.029 ms, trace 0.022 ms, prepare 0.015 ms, resume_world 0.010 ms, accounting 0.006 ms, finish 0.006 ms, weak_pages 0.001 ms, verify 0.001 ms, trim 0.001 ms, stop_world 0.001 ms
- ('messages-int-sustained', 1, 'inline256'): allocation_census 1.974 ms, sweep_finalize 1.762 ms, roots 0.025 ms, trace 0.018 ms, prepare 0.017 ms, resume_world 0.008 ms, finish 0.004 ms, accounting 0.003 ms, weak_pages 0.001 ms, verify 0.001 ms, stop_world 0.001 ms, trim 0.001 ms
- ('messages-int-sustained', 8, 'inline0'): sweep_finalize 6.776 ms, allocation_census 0.998 ms, roots 0.022 ms, prepare 0.013 ms, trace 0.012 ms, finish 0.005 ms, accounting 0.005 ms, stop_world 0.004 ms, resume_world 0.002 ms, trim 0.001 ms, weak_pages 0.001 ms, verify 0.001 ms
- ('messages-int-sustained', 8, 'inline256'): sweep_finalize 2.182 ms, allocation_census 1.895 ms, roots 0.023 ms, trace 0.015 ms, prepare 0.014 ms, finish 0.006 ms, accounting 0.005 ms, stop_world 0.005 ms, resume_world 0.002 ms, trim 0.001 ms, weak_pages 0.001 ms, verify 0.001 ms

Sweep work distribution (workload collections only):

Small helper jobs contain fewer than 4,096 objects; native thread creation is counted separately.
these counts alone do not measure thread-startup cost.

| Case | Workers | Variant | Collections / parallel | Helper jobs | New native threads | Small helper jobs | Median helper objects | Median largest-list share in parallel collections |
|---|---:|---|---:|---:|---:|---:|---:|---:|
| buffers-capacity-32 | 1 | inline0 | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-32 | 1 | inline256 | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-32 | 8 | inline0 | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-32 | 8 | inline256 | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-grow | 1 | inline0 | 4 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-grow | 1 | inline256 | 5 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-grow | 8 | inline0 | 4 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-grow | 8 | inline256 | 5 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-pointers | 1 | inline0 | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-pointers | 1 | inline256 | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-pointers | 8 | inline0 | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-pointers | 8 | inline256 | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| messages-int-sustained | 1 | inline0 | 4 / 0 | 0 | 0 | 0 | n/a | n/a |
| messages-int-sustained | 1 | inline256 | 4 / 0 | 0 | 0 | 0 | n/a | n/a |
| messages-int-sustained | 8 | inline0 | 4 / 1 | 1 | 1 | 0 | 52733 | 99.8% |
| messages-int-sustained | 8 | inline256 | 4 / 1 | 1 | 1 | 0 | 52729 | 99.8% |
