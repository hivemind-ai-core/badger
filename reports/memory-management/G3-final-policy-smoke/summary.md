| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| balanced | 2 | sweep-reuse | 169.670 [169.670, 169.670] | 21.466 | 69.9 |
| balanced | 2 | sweep-scoped | 85.959 [85.959, 85.959] | 14.565 | 70.0 |
| balanced | 8 | sweep-reuse | 35.084 [35.084, 35.084] | 9.269 | 86.6 |
| balanced | 8 | sweep-scoped | 32.593 [32.593, 32.593] | 8.915 | 88.2 |
| fanout | 2 | sweep-reuse | 303.283 [303.283, 303.283] | 49.839 | 90.5 |
| fanout | 2 | sweep-scoped | 253.600 [253.600, 253.600] | 49.026 | 89.4 |
| fanout | 8 | sweep-reuse | 257.968 [257.968, 257.968] | 40.338 | 105.5 |
| fanout | 8 | sweep-scoped | 238.650 [238.650, 238.650] | 38.184 | 111.3 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| balanced | 2 | sweep-reuse | 4096 | 24.576 | 32.768 |
| balanced | 2 | sweep-scoped | 4096 | 11.776 | 16.384 |
| balanced | 8 | sweep-reuse | 4096 | 4.864 | 6.144 |
| balanced | 8 | sweep-scoped | 4096 | 4.352 | 5.632 |
| fanout | 2 | sweep-reuse | 4096 | 38.912 | 59.392 |
| fanout | 2 | sweep-scoped | 4096 | 34.816 | 55.296 |
| fanout | 8 | sweep-reuse | 4096 | 32.768 | 49.152 |
| fanout | 8 | sweep-scoped | 4096 | 31.744 | 43.008 |

Diagnostic GC phases summed across measured repetitions (wall time):

- ('balanced', 2, 'sweep-reuse'): sweep_finalize 15.175 ms, trace 5.786 ms, roots 0.133 ms, allocation_census 0.119 ms, stop_world 0.080 ms, prepare 0.031 ms, accounting 0.008 ms, finish 0.007 ms, weak_pages 0.004 ms, resume_world 0.003 ms, verify 0.001 ms, trim 0.001 ms
- ('balanced', 2, 'sweep-scoped'): sweep_finalize 10.631 ms, trace 3.688 ms, roots 0.086 ms, allocation_census 0.037 ms, prepare 0.023 ms, accounting 0.007 ms, finish 0.006 ms, stop_world 0.005 ms, resume_world 0.004 ms, weak_pages 0.002 ms, trim 0.001 ms, verify 0.001 ms
- ('balanced', 8, 'sweep-reuse'): sweep_finalize 5.632 ms, trace 3.417 ms, roots 0.085 ms, allocation_census 0.046 ms, prepare 0.022 ms, stop_world 0.015 ms, accounting 0.004 ms, finish 0.004 ms, resume_world 0.002 ms, weak_pages 0.002 ms, trim 0.001 ms, verify 0.001 ms
- ('balanced', 8, 'sweep-scoped'): sweep_finalize 5.550 ms, trace 3.136 ms, roots 0.067 ms, stop_world 0.044 ms, allocation_census 0.035 ms, prepare 0.031 ms, accounting 0.004 ms, finish 0.004 ms, resume_world 0.002 ms, weak_pages 0.001 ms, trim 0.001 ms, verify 0.001 ms
- ('fanout', 2, 'sweep-reuse'): sweep_finalize 36.135 ms, trace 9.321 ms, weak_pages 1.416 ms, prepare 1.393 ms, roots 0.695 ms, trim 0.598 ms, allocation_census 0.039 ms, stop_world 0.014 ms, accounting 0.012 ms, finish 0.012 ms, resume_world 0.008 ms, verify 0.002 ms
- ('fanout', 2, 'sweep-scoped'): sweep_finalize 35.024 ms, trace 9.290 ms, weak_pages 1.722 ms, prepare 1.410 ms, roots 0.711 ms, trim 0.588 ms, allocation_census 0.029 ms, accounting 0.015 ms, finish 0.014 ms, stop_world 0.007 ms, resume_world 0.007 ms, verify 0.002 ms
- ('fanout', 8, 'sweep-reuse'): sweep_finalize 25.763 ms, trace 10.140 ms, weak_pages 1.416 ms, prepare 1.354 ms, roots 0.803 ms, trim 0.508 ms, allocation_census 0.063 ms, stop_world 0.053 ms, finish 0.016 ms, accounting 0.016 ms, resume_world 0.008 ms, verify 0.002 ms
- ('fanout', 8, 'sweep-scoped'): sweep_finalize 23.734 ms, trace 9.876 ms, prepare 1.413 ms, weak_pages 1.360 ms, roots 1.021 ms, trim 0.560 ms, allocation_census 0.039 ms, stop_world 0.016 ms, accounting 0.012 ms, finish 0.010 ms, resume_world 0.007 ms, verify 0.002 ms

Sweep work distribution (workload collections only):

Small helper jobs contain fewer than 4,096 objects; native thread creation is counted separately.
these counts alone do not measure thread-startup cost.

| Case | Workers | Variant | Collections / parallel | Helper jobs | New native threads | Small helper jobs | Median helper objects | Median largest-list share in parallel collections |
|---|---:|---|---:|---:|---:|---:|---:|---:|
| balanced | 2 | sweep-reuse | 5 / 5 | 5 | 1 | 0 | 59527 | 53.8% |
| balanced | 2 | sweep-scoped | 5 / 5 | 5 | 5 | 0 | 61287 | 51.0% |
| balanced | 8 | sweep-reuse | 5 / 5 | 35 | 7 | 0 | 16343 | 15.0% |
| balanced | 8 | sweep-scoped | 5 / 5 | 35 | 35 | 0 | 15604 | 15.5% |
| fanout | 2 | sweep-reuse | 10 / 10 | 10 | 1 | 0 | 78660 | 70.3% |
| fanout | 2 | sweep-scoped | 10 / 10 | 10 | 10 | 0 | 82027 | 62.3% |
| fanout | 8 | sweep-reuse | 10 / 10 | 70 | 7 | 0 | 19858 | 14.4% |
| fanout | 8 | sweep-scoped | 10 / 10 | 70 | 70 | 3 | 19685 | 14.2% |
