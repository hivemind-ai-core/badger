| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| buffers-capacity-32 | 1 | inline0 | 24.816 [24.289, 25.396] | 1.146 | 38.9 |
| buffers-capacity-32 | 1 | inline256 | 18.168 [17.707, 18.619] | 0.385 | 43.4 |
| buffers-capacity-32 | 1 | inline256-pool256 | 26.075 [25.936, 26.508] | 1.022 | 38.2 |
| buffers-capacity-32 | 8 | inline0 | 24.542 [24.489, 24.731] | 1.177 | 39.2 |
| buffers-capacity-32 | 8 | inline256 | 18.481 [18.174, 18.845] | 0.399 | 43.4 |
| buffers-capacity-32 | 8 | inline256-pool256 | 26.141 [25.731, 26.590] | 1.031 | 38.6 |
| bytes-survivors | 1 | inline0 | 122.523 [110.099, 125.387] | 1.429 | 39.7 |
| bytes-survivors | 1 | inline256 | 120.658 [120.240, 122.734] | 0.942 | 44.2 |
| bytes-survivors | 1 | inline256-pool256 | 127.925 [111.200, 128.510] | 1.417 | 38.7 |
| bytes-survivors | 8 | inline0 | 118.075 [108.839, 122.766] | 1.475 | 40.0 |
| bytes-survivors | 8 | inline256 | 115.110 [106.971, 121.450] | 0.894 | 44.2 |
| bytes-survivors | 8 | inline256-pool256 | 121.056 [119.651, 126.972] | 1.435 | 39.0 |
| messages-int-sustained | 1 | inline0 | 94.173 [93.977, 96.911] | 4.578 | 39.1 |
| messages-int-sustained | 1 | inline256 | 81.618 [80.632, 85.425] | 1.597 | 45.2 |
| messages-int-sustained | 1 | inline256-pool256 | 95.710 [93.577, 96.537] | 4.088 | 38.4 |
| messages-int-sustained | 8 | inline0 | 101.562 [98.947, 108.745] | 4.636 | 47.8 |
| messages-int-sustained | 8 | inline256 | 81.265 [80.629, 84.696] | 1.934 | 43.9 |
| messages-int-sustained | 8 | inline256-pool256 | 96.415 [94.612, 98.456] | 3.672 | 44.8 |

Diagnostic GC phases summed across measured repetitions (wall time):

- ('buffers-capacity-32', 1, 'inline0'): sweep_finalize 3.249 ms, roots 0.032 ms, trace 0.018 ms, prepare 0.015 ms, resume_world 0.014 ms, allocation_census 0.009 ms, finish 0.003 ms, accounting 0.002 ms, trim 0.001 ms, verify 0.001 ms, weak_pages 0.001 ms, stop_world 0.001 ms
- ('buffers-capacity-32', 1, 'inline256'): sweep_finalize 1.078 ms, roots 0.031 ms, trace 0.018 ms, prepare 0.012 ms, resume_world 0.011 ms, allocation_census 0.007 ms, finish 0.002 ms, accounting 0.002 ms, stop_world 0.001 ms, weak_pages 0.001 ms, trim 0.000 ms, verify 0.000 ms
- ('buffers-capacity-32', 1, 'inline256-pool256'): sweep_finalize 2.932 ms, roots 0.031 ms, trace 0.021 ms, prepare 0.014 ms, resume_world 0.013 ms, allocation_census 0.012 ms, finish 0.003 ms, accounting 0.002 ms, verify 0.001 ms, stop_world 0.001 ms, weak_pages 0.001 ms, trim 0.000 ms
- ('buffers-capacity-32', 8, 'inline0'): sweep_finalize 3.369 ms, roots 0.023 ms, trace 0.018 ms, prepare 0.014 ms, allocation_census 0.011 ms, stop_world 0.006 ms, finish 0.003 ms, accounting 0.002 ms, resume_world 0.001 ms, trim 0.001 ms, verify 0.000 ms, weak_pages 0.000 ms
- ('buffers-capacity-32', 8, 'inline256'): sweep_finalize 1.106 ms, trace 0.019 ms, roots 0.018 ms, prepare 0.012 ms, stop_world 0.007 ms, allocation_census 0.007 ms, finish 0.002 ms, accounting 0.001 ms, resume_world 0.001 ms, weak_pages 0.001 ms, verify 0.001 ms, trim 0.000 ms
- ('buffers-capacity-32', 8, 'inline256-pool256'): sweep_finalize 2.889 ms, trace 0.022 ms, roots 0.015 ms, prepare 0.011 ms, allocation_census 0.008 ms, stop_world 0.005 ms, finish 0.003 ms, accounting 0.002 ms, trim 0.001 ms, weak_pages 0.001 ms, resume_world 0.000 ms, verify 0.000 ms
- ('bytes-survivors', 1, 'inline0'): sweep_finalize 3.361 ms, trace 0.743 ms, roots 0.027 ms, resume_world 0.027 ms, prepare 0.015 ms, allocation_census 0.012 ms, finish 0.003 ms, accounting 0.003 ms, trim 0.001 ms, weak_pages 0.001 ms, stop_world 0.001 ms, verify 0.001 ms
- ('bytes-survivors', 1, 'inline256'): sweep_finalize 2.015 ms, trace 0.674 ms, roots 0.025 ms, prepare 0.016 ms, resume_world 0.012 ms, allocation_census 0.009 ms, finish 0.002 ms, accounting 0.002 ms, trim 0.001 ms, weak_pages 0.001 ms, stop_world 0.001 ms, verify 0.000 ms
- ('bytes-survivors', 1, 'inline256-pool256'): sweep_finalize 3.318 ms, trace 0.746 ms, roots 0.028 ms, prepare 0.019 ms, resume_world 0.014 ms, allocation_census 0.006 ms, finish 0.003 ms, accounting 0.002 ms, trim 0.001 ms, stop_world 0.001 ms, weak_pages 0.001 ms, verify 0.000 ms
- ('bytes-survivors', 8, 'inline0'): sweep_finalize 3.458 ms, trace 0.791 ms, allocation_census 0.024 ms, roots 0.024 ms, prepare 0.016 ms, stop_world 0.006 ms, finish 0.003 ms, accounting 0.003 ms, weak_pages 0.002 ms, resume_world 0.001 ms, verify 0.001 ms, trim 0.001 ms
- ('bytes-survivors', 8, 'inline256'): sweep_finalize 1.928 ms, trace 0.647 ms, roots 0.018 ms, prepare 0.016 ms, allocation_census 0.007 ms, stop_world 0.006 ms, finish 0.003 ms, accounting 0.002 ms, resume_world 0.001 ms, weak_pages 0.001 ms, trim 0.001 ms, verify 0.000 ms
- ('bytes-survivors', 8, 'inline256-pool256'): sweep_finalize 3.383 ms, trace 0.784 ms, roots 0.019 ms, prepare 0.014 ms, allocation_census 0.010 ms, stop_world 0.006 ms, finish 0.003 ms, accounting 0.002 ms, resume_world 0.001 ms, weak_pages 0.001 ms, trim 0.001 ms, verify 0.000 ms
- ('messages-int-sustained', 1, 'inline0'): sweep_finalize 13.030 ms, roots 0.064 ms, trace 0.044 ms, allocation_census 0.042 ms, prepare 0.031 ms, resume_world 0.019 ms, finish 0.014 ms, accounting 0.012 ms, trim 0.002 ms, weak_pages 0.002 ms, stop_world 0.001 ms, verify 0.001 ms
- ('messages-int-sustained', 1, 'inline256'): sweep_finalize 4.590 ms, roots 0.061 ms, trace 0.040 ms, prepare 0.033 ms, allocation_census 0.025 ms, resume_world 0.011 ms, finish 0.009 ms, accounting 0.006 ms, weak_pages 0.002 ms, trim 0.002 ms, verify 0.002 ms, stop_world 0.001 ms
- ('messages-int-sustained', 1, 'inline256-pool256'): sweep_finalize 11.554 ms, roots 0.062 ms, trace 0.045 ms, allocation_census 0.039 ms, prepare 0.033 ms, resume_world 0.022 ms, finish 0.011 ms, accounting 0.008 ms, weak_pages 0.002 ms, trim 0.002 ms, verify 0.001 ms, stop_world 0.001 ms
- ('messages-int-sustained', 8, 'inline0'): sweep_finalize 15.016 ms, roots 0.053 ms, allocation_census 0.044 ms, trace 0.036 ms, prepare 0.032 ms, finish 0.011 ms, stop_world 0.010 ms, accounting 0.009 ms, resume_world 0.002 ms, trim 0.002 ms, weak_pages 0.002 ms, verify 0.002 ms
- ('messages-int-sustained', 8, 'inline256'): sweep_finalize 5.182 ms, roots 0.054 ms, trace 0.040 ms, prepare 0.031 ms, allocation_census 0.025 ms, stop_world 0.013 ms, finish 0.011 ms, accounting 0.006 ms, resume_world 0.002 ms, trim 0.002 ms, weak_pages 0.002 ms, verify 0.002 ms
- ('messages-int-sustained', 8, 'inline256-pool256'): sweep_finalize 10.722 ms, roots 0.058 ms, allocation_census 0.043 ms, prepare 0.035 ms, trace 0.035 ms, accounting 0.017 ms, finish 0.011 ms, stop_world 0.010 ms, resume_world 0.002 ms, trim 0.002 ms, weak_pages 0.002 ms, verify 0.001 ms

Sweep work distribution (workload collections only):

Small helper jobs contain fewer than 4,096 objects; native thread creation is counted separately.
these counts alone do not measure thread-startup cost.

| Case | Workers | Variant | Collections / parallel | Helper jobs | New native threads | Small helper jobs | Median helper objects | Median largest-list share in parallel collections |
|---|---:|---|---:|---:|---:|---:|---:|---:|
| buffers-capacity-32 | 1 | inline0 | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-32 | 1 | inline256 | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-32 | 1 | inline256-pool256 | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-32 | 8 | inline0 | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-32 | 8 | inline256 | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-32 | 8 | inline256-pool256 | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| bytes-survivors | 1 | inline0 | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| bytes-survivors | 1 | inline256 | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| bytes-survivors | 1 | inline256-pool256 | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| bytes-survivors | 8 | inline0 | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| bytes-survivors | 8 | inline256 | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| bytes-survivors | 8 | inline256-pool256 | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| messages-int-sustained | 1 | inline0 | 12 / 0 | 0 | 0 | 0 | n/a | n/a |
| messages-int-sustained | 1 | inline256 | 12 / 0 | 0 | 0 | 0 | n/a | n/a |
| messages-int-sustained | 1 | inline256-pool256 | 12 / 0 | 0 | 0 | 0 | n/a | n/a |
| messages-int-sustained | 8 | inline0 | 12 / 4 | 4 | 4 | 1 | 52733 | 99.8% |
| messages-int-sustained | 8 | inline256 | 12 / 2 | 3 | 3 | 1 | 52480 | 99.6% |
| messages-int-sustained | 8 | inline256-pool256 | 12 / 3 | 4 | 4 | 1 | 52604 | 99.8% |
