| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| lookup-array-int | 1 | control | 4.493 [4.493, 4.493] | 0.000 | 29.3 |
| lookup-array-int | 8 | control | 3.169 [3.169, 3.169] | 0.000 | 28.9 |
| lookup-list-pointers | 1 | control | 19.736 [19.736, 19.736] | 0.000 | 29.4 |
| lookup-list-pointers | 8 | control | 27.006 [27.006, 27.006] | 0.000 | 29.0 |
| lookup-map-int | 1 | control | 20.601 [20.601, 20.601] | 0.000 | 29.2 |
| lookup-map-int | 8 | control | 20.630 [20.630, 20.630] | 0.000 | 29.5 |
| lookup-map-pointers | 1 | control | 28.182 [28.182, 28.182] | 0.000 | 31.7 |
| lookup-map-pointers | 8 | control | 35.861 [35.861, 35.861] | 0.000 | 31.4 |
| lookup-map-stored | 1 | control | 50.811 [50.811, 50.811] | 3.714 | 71.0 |
| lookup-map-stored | 8 | control | 51.853 [51.853, 51.853] | 3.431 | 69.4 |

Diagnostic GC phases summed across measured repetitions (wall time):

- ('lookup-map-stored', 1, 'control'): trace 2.054 ms, allocation_census 1.100 ms, sweep_finalize 0.518 ms, resume_world 0.011 ms, roots 0.009 ms, prepare 0.006 ms, finish 0.001 ms, accounting 0.001 ms, weak_pages 0.001 ms, stop_world 0.001 ms, verify 0.000 ms, trim 0.000 ms
- ('lookup-map-stored', 8, 'control'): trace 1.956 ms, allocation_census 1.081 ms, sweep_finalize 0.359 ms, roots 0.007 ms, prepare 0.004 ms, stop_world 0.002 ms, finish 0.001 ms, accounting 0.001 ms, resume_world 0.000 ms, weak_pages 0.000 ms, trim 0.000 ms, verify 0.000 ms

Sweep work distribution (workload collections only):

Small helper jobs contain fewer than 4,096 objects; native thread creation is counted separately.
these counts alone do not measure thread-startup cost.

| Case | Workers | Variant | Collections / parallel | Helper jobs | New native threads | Small helper jobs | Median helper objects | Median largest-list share in parallel collections |
|---|---:|---|---:|---:|---:|---:|---:|---:|
| lookup-map-stored | 1 | control | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
| lookup-map-stored | 8 | control | 1 / 0 | 0 | 0 | 0 | n/a | n/a |
