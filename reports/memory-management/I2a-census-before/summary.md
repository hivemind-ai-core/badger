| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| lookup-array-int | 1 | control | 2.968 [2.968, 2.968] | 0.000 | 28.6 |
| lookup-array-int | 8 | control | 4.090 [4.090, 4.090] | 0.000 | 29.5 |
| lookup-list-pointers | 1 | control | 37.537 [37.537, 37.537] | 2.973 | 60.8 |
| lookup-list-pointers | 8 | control | 46.479 [46.479, 46.479] | 3.119 | 60.0 |
| lookup-map-int | 1 | control | 44.208 [44.208, 44.208] | 2.981 | 61.3 |
| lookup-map-int | 8 | control | 42.962 [42.962, 42.962] | 2.976 | 59.5 |
| lookup-map-pointers | 1 | control | 51.145 [51.145, 51.145] | 2.925 | 62.6 |
| lookup-map-pointers | 8 | control | 56.335 [56.335, 56.335] | 2.900 | 63.6 |
| lookup-map-stored | 1 | control | 52.185 [52.185, 52.185] | 3.462 | 68.5 |
| lookup-map-stored | 8 | control | 51.580 [51.580, 51.580] | 4.075 | 71.1 |

Diagnostic GC phases summed across measured repetitions (wall time):

- ('lookup-list-pointers', 1, 'control'): allocation_census 1.695 ms, sweep_finalize 1.226 ms, trace 0.011 ms, roots 0.010 ms, resume_world 0.007 ms, prepare 0.006 ms, finish 0.001 ms, accounting 0.001 ms, stop_world 0.000 ms, trim 0.000 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('lookup-list-pointers', 8, 'control'): allocation_census 1.657 ms, sweep_finalize 1.416 ms, trace 0.011 ms, roots 0.007 ms, prepare 0.005 ms, finish 0.002 ms, stop_world 0.002 ms, accounting 0.001 ms, resume_world 0.001 ms, trim 0.000 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('lookup-map-int', 1, 'control'): allocation_census 1.705 ms, sweep_finalize 1.222 ms, resume_world 0.019 ms, roots 0.008 ms, prepare 0.006 ms, trace 0.006 ms, finish 0.002 ms, accounting 0.001 ms, stop_world 0.000 ms, trim 0.000 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('lookup-map-int', 8, 'control'): allocation_census 1.711 ms, sweep_finalize 1.226 ms, roots 0.007 ms, prepare 0.006 ms, trace 0.005 ms, stop_world 0.002 ms, finish 0.002 ms, accounting 0.001 ms, resume_world 0.001 ms, weak_pages 0.000 ms, trim 0.000 ms, verify 0.000 ms
- ('lookup-map-pointers', 1, 'control'): allocation_census 1.643 ms, sweep_finalize 1.221 ms, trace 0.017 ms, roots 0.011 ms, resume_world 0.009 ms, prepare 0.006 ms, finish 0.001 ms, accounting 0.001 ms, trim 0.000 ms, stop_world 0.000 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('lookup-map-pointers', 8, 'control'): allocation_census 1.669 ms, sweep_finalize 1.182 ms, trace 0.014 ms, roots 0.009 ms, prepare 0.005 ms, stop_world 0.002 ms, finish 0.001 ms, accounting 0.001 ms, resume_world 0.001 ms, trim 0.000 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('lookup-map-stored', 1, 'control'): trace 1.999 ms, allocation_census 1.071 ms, sweep_finalize 0.362 ms, roots 0.008 ms, prepare 0.005 ms, resume_world 0.004 ms, finish 0.001 ms, accounting 0.001 ms, weak_pages 0.000 ms, stop_world 0.000 ms, trim 0.000 ms, verify 0.000 ms
- ('lookup-map-stored', 8, 'control'): trace 1.783 ms, allocation_census 1.767 ms, sweep_finalize 0.494 ms, roots 0.007 ms, prepare 0.006 ms, stop_world 0.002 ms, finish 0.001 ms, accounting 0.001 ms, weak_pages 0.001 ms, resume_world 0.000 ms, verify 0.000 ms, trim 0.000 ms

Sweep work distribution (workload collections only):

Small helper jobs contain fewer than 4,096 objects; native thread creation is counted separately.
these counts alone do not measure thread-startup cost.

| Case | Workers | Variant | Collections / parallel | Helper jobs | New native threads | Small helper jobs | Median helper objects | Median largest-list share in parallel collections |
|---|---:|---|---:|---:|---:|---:|---:|---:|
| lookup-list-pointers | 1 | control | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| lookup-list-pointers | 8 | control | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| lookup-map-int | 1 | control | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| lookup-map-int | 8 | control | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| lookup-map-pointers | 1 | control | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| lookup-map-pointers | 8 | control | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| lookup-map-stored | 1 | control | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| lookup-map-stored | 8 | control | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
