| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| balanced | 1 | inline0 | 553.531 [553.531, 553.531] | 116.664 | 90.9 |
| balanced | 1 | inline256 | 554.952 [554.952, 554.952] | 123.125 | 96.6 |
| balanced | 8 | inline0 | 180.464 [180.464, 180.464] | 77.397 | 117.0 |
| balanced | 8 | inline256 | 182.824 [182.824, 182.824] | 81.565 | 124.6 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| balanced | 1 | inline0 | 16384 | 18.432 | 32.768 |
| balanced | 1 | inline256 | 16384 | 18.432 | 36.864 |
| balanced | 8 | inline0 | 16384 | 6.400 | 15.872 |
| balanced | 8 | inline256 | 16384 | 6.400 | 16.384 |

Diagnostic GC phases summed across measured repetitions (wall time):

- ('balanced', 1, 'inline0'): sweep_finalize 57.509 ms, allocation_census 40.231 ms, trace 11.200 ms, prepare 2.796 ms, weak_pages 2.483 ms, trim 1.061 ms, roots 0.282 ms, finish 0.028 ms, resume_world 0.020 ms, accounting 0.019 ms, verify 0.003 ms, stop_world 0.001 ms
- ('balanced', 1, 'inline256'): sweep_finalize 54.479 ms, allocation_census 48.931 ms, trace 11.296 ms, prepare 3.800 ms, weak_pages 3.161 ms, trim 0.709 ms, roots 0.268 ms, resume_world 0.026 ms, finish 0.024 ms, accounting 0.018 ms, verify 0.003 ms, stop_world 0.002 ms
- ('balanced', 8, 'inline0'): allocation_census 36.609 ms, sweep_finalize 23.511 ms, trace 10.411 ms, prepare 2.500 ms, weak_pages 2.474 ms, trim 0.906 ms, stop_world 0.466 ms, roots 0.280 ms, finish 0.021 ms, accounting 0.018 ms, resume_world 0.011 ms, verify 0.009 ms
- ('balanced', 8, 'inline256'): allocation_census 44.007 ms, sweep_finalize 16.942 ms, trace 12.415 ms, prepare 3.445 ms, weak_pages 3.327 ms, trim 0.661 ms, roots 0.280 ms, stop_world 0.255 ms, finish 0.021 ms, accounting 0.020 ms, resume_world 0.010 ms, verify 0.004 ms

Sweep work distribution (workload collections only):

Small helper jobs contain fewer than 4,096 objects; native thread creation is counted separately.
these counts alone do not measure thread-startup cost.

| Case | Workers | Variant | Collections / parallel | Helper jobs | New native threads | Small helper jobs | Median helper objects | Median largest-list share in parallel collections |
|---|---:|---|---:|---:|---:|---:|---:|---:|
| balanced | 1 | inline0 | 22 / 0 | 0 | 0 | 0 | n/a | n/a |
| balanced | 1 | inline256 | 22 / 0 | 0 | 0 | 0 | n/a | n/a |
| balanced | 8 | inline0 | 21 / 21 | 147 | 147 | 0 | 16857 | 14.7% |
| balanced | 8 | inline256 | 21 / 21 | 147 | 147 | 0 | 17219 | 14.3% |
