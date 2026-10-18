| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| parallel-snapshots | 1 | baseline | 246.129 [244.172, 246.398] | 65.228 | 81.2 |
| parallel-snapshots | 1 | diagnostic | 259.840 [253.763, 272.207] | 74.988 | 80.9 |
| parallel-snapshots | 1 | diagnostic-off | 258.928 [257.120, 260.917] | 71.137 | 83.1 |
| parallel-snapshots | 2 | baseline | 206.828 [201.960, 215.928] | 66.551 | 79.0 |
| parallel-snapshots | 2 | diagnostic | 215.395 [204.727, 217.143] | 69.392 | 79.1 |
| parallel-snapshots | 2 | diagnostic-off | 203.159 [202.579, 207.000] | 64.310 | 80.1 |
| parallel-snapshots | 4 | baseline | 210.363 [209.695, 222.080] | 75.745 | 98.0 |
| parallel-snapshots | 4 | diagnostic | 224.536 [219.054, 228.307] | 84.391 | 98.6 |
| parallel-snapshots | 4 | diagnostic-off | 220.243 [219.643, 221.390] | 81.438 | 99.9 |
| parallel-snapshots | 8 | baseline | 397.205 [391.783, 409.866] | 105.921 | 133.3 |
| parallel-snapshots | 8 | diagnostic | 402.395 [402.309, 404.668] | 110.145 | 140.3 |
| parallel-snapshots | 8 | diagnostic-off | 398.700 [383.139, 406.458] | 112.420 | 135.9 |
| records-window | 1 | baseline | 121.828 [121.065, 130.397] | 18.930 | 52.8 |
| records-window | 1 | diagnostic | 129.193 [128.302, 130.633] | 22.877 | 51.9 |
| records-window | 1 | diagnostic-off | 133.746 [130.361, 140.925] | 23.010 | 52.6 |
| records-window | 2 | baseline | 123.570 [122.199, 124.386] | 19.124 | 51.5 |
| records-window | 2 | diagnostic | 132.959 [131.766, 135.252] | 23.934 | 53.4 |
| records-window | 2 | diagnostic-off | 129.880 [126.280, 133.774] | 22.616 | 52.1 |
| records-window | 4 | baseline | 123.724 [119.196, 125.269] | 19.636 | 51.8 |
| records-window | 4 | diagnostic | 130.672 [128.604, 130.802] | 23.297 | 52.3 |
| records-window | 4 | diagnostic-off | 126.407 [126.145, 128.319] | 22.505 | 52.3 |
| records-window | 8 | baseline | 121.152 [117.976, 122.355] | 18.402 | 51.6 |
| records-window | 8 | diagnostic | 129.586 [127.891, 135.613] | 23.794 | 52.7 |
| records-window | 8 | diagnostic-off | 129.542 [125.401, 134.456] | 23.072 | 52.4 |
| scalar-snapshots | 1 | baseline | 10.872 [10.831, 11.059] | 1.437 | 50.5 |
| scalar-snapshots | 1 | diagnostic | 11.755 [11.337, 14.502] | 1.686 | 52.0 |
| scalar-snapshots | 1 | diagnostic-off | 11.254 [10.993, 11.308] | 1.524 | 51.4 |
| scalar-snapshots | 2 | baseline | 11.134 [10.911, 11.321] | 1.437 | 50.7 |
| scalar-snapshots | 2 | diagnostic | 11.870 [11.144, 12.389] | 1.578 | 52.2 |
| scalar-snapshots | 2 | diagnostic-off | 11.592 [11.519, 11.649] | 1.627 | 52.4 |
| scalar-snapshots | 4 | baseline | 11.591 [11.265, 12.205] | 1.522 | 50.8 |
| scalar-snapshots | 4 | diagnostic | 12.066 [11.578, 12.267] | 1.819 | 52.2 |
| scalar-snapshots | 4 | diagnostic-off | 11.533 [11.412, 12.042] | 1.571 | 51.5 |
| scalar-snapshots | 8 | baseline | 11.707 [10.909, 12.712] | 1.507 | 50.9 |
| scalar-snapshots | 8 | diagnostic | 12.411 [11.921, 15.059] | 1.838 | 52.5 |
| scalar-snapshots | 8 | diagnostic-off | 11.366 [11.016, 11.645] | 1.578 | 52.1 |

Diagnostic GC phases summed across measured repetitions (wall time):

- ('parallel-snapshots', 1, 'diagnostic'): sweep_finalize 101.357 ms, trace 72.561 ms, trim 24.617 ms, prepare 12.752 ms, weak_pages 10.190 ms, roots 0.673 ms, allocation_census 0.354 ms, finish 0.088 ms, accounting 0.058 ms, resume_world 0.046 ms, stop_world 0.025 ms, verify 0.018 ms
- ('parallel-snapshots', 2, 'diagnostic'): sweep_finalize 83.520 ms, trace 70.084 ms, trim 29.438 ms, prepare 12.293 ms, weak_pages 10.207 ms, roots 0.799 ms, stop_world 0.571 ms, allocation_census 0.375 ms, finish 0.133 ms, accounting 0.076 ms, resume_world 0.038 ms, verify 0.022 ms
- ('parallel-snapshots', 4, 'diagnostic'): trace 108.612 ms, sweep_finalize 62.014 ms, trim 42.884 ms, prepare 15.961 ms, weak_pages 14.180 ms, roots 0.842 ms, stop_world 0.667 ms, allocation_census 0.454 ms, finish 0.127 ms, accounting 0.070 ms, resume_world 0.041 ms, verify 0.022 ms
- ('parallel-snapshots', 8, 'diagnostic'): trace 198.044 ms, sweep_finalize 48.969 ms, trim 43.233 ms, prepare 17.471 ms, weak_pages 15.964 ms, stop_world 0.700 ms, roots 0.691 ms, allocation_census 0.471 ms, finish 0.217 ms, accounting 0.070 ms, resume_world 0.042 ms, verify 0.021 ms
- ('records-window', 1, 'diagnostic'): sweep_finalize 57.763 ms, trim 3.629 ms, prepare 2.456 ms, weak_pages 2.018 ms, trace 1.241 ms, roots 0.117 ms, allocation_census 0.067 ms, finish 0.049 ms, resume_world 0.033 ms, accounting 0.033 ms, stop_world 0.011 ms, verify 0.007 ms
- ('records-window', 2, 'diagnostic'): sweep_finalize 62.021 ms, trim 3.665 ms, prepare 2.510 ms, weak_pages 2.256 ms, trace 1.332 ms, roots 0.144 ms, allocation_census 0.072 ms, finish 0.051 ms, stop_world 0.035 ms, accounting 0.035 ms, resume_world 0.022 ms, verify 0.009 ms
- ('records-window', 4, 'diagnostic'): sweep_finalize 58.744 ms, trim 3.633 ms, prepare 3.123 ms, weak_pages 2.032 ms, trace 1.250 ms, roots 0.139 ms, allocation_census 0.071 ms, finish 0.052 ms, accounting 0.036 ms, stop_world 0.032 ms, resume_world 0.020 ms, verify 0.008 ms
- ('records-window', 8, 'diagnostic'): sweep_finalize 61.226 ms, trim 4.381 ms, prepare 2.434 ms, weak_pages 2.238 ms, trace 1.268 ms, roots 0.135 ms, allocation_census 0.074 ms, finish 0.052 ms, accounting 0.036 ms, stop_world 0.035 ms, resume_world 0.023 ms, verify 0.008 ms
- ('scalar-snapshots', 1, 'diagnostic'): sweep_finalize 3.047 ms, trace 1.970 ms, roots 0.032 ms, prepare 0.023 ms, resume_world 0.019 ms, allocation_census 0.018 ms, finish 0.006 ms, accounting 0.003 ms, stop_world 0.002 ms, weak_pages 0.001 ms, trim 0.001 ms, verify 0.001 ms
- ('scalar-snapshots', 2, 'diagnostic'): sweep_finalize 3.025 ms, trace 1.805 ms, roots 0.023 ms, prepare 0.021 ms, allocation_census 0.019 ms, stop_world 0.008 ms, finish 0.005 ms, accounting 0.004 ms, weak_pages 0.003 ms, resume_world 0.001 ms, trim 0.001 ms, verify 0.001 ms
- ('scalar-snapshots', 4, 'diagnostic'): sweep_finalize 3.260 ms, trace 1.826 ms, roots 0.025 ms, prepare 0.022 ms, allocation_census 0.019 ms, finish 0.014 ms, stop_world 0.007 ms, accounting 0.003 ms, resume_world 0.002 ms, trim 0.001 ms, weak_pages 0.001 ms, verify 0.001 ms
- ('scalar-snapshots', 8, 'diagnostic'): sweep_finalize 3.348 ms, trace 2.115 ms, roots 0.024 ms, prepare 0.021 ms, allocation_census 0.021 ms, stop_world 0.009 ms, finish 0.006 ms, accounting 0.003 ms, resume_world 0.002 ms, trim 0.001 ms, verify 0.001 ms, weak_pages 0.001 ms

Trace cardinality: medians of workload totals per fresh process.
Candidates include duplicate, old and immortal pointers; trace counts include remembered parents.
Timing is instrumented wall time, not per-object latency. Separate medians need not conserve counts.

| Case | Workers | Variant | Collections | Candidates | Newly marked | Marked leaves | Traced | Remembered traces | Trace ms |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|
| parallel-snapshots | 1 | diagnostic | 37 | 2.69242e+06 | 460053 | 316655 | 145560 | 2162 | 23.317 |
| parallel-snapshots | 2 | diagnostic | 37 | 2.61884e+06 | 439871 | 302081 | 140555 | 2390 | 23.069 |
| parallel-snapshots | 4 | diagnostic | 36 | 3.71028e+06 | 629063 | 433815 | 199010 | 3762 | 36.555 |
| parallel-snapshots | 8 | diagnostic | 33 | 5.46006e+06 | 797674 | 519295 | 284404 | 6025 | 66.348 |
| records-window | 1 | diagnostic | 16 | 51229 | 24198 | 20 | 24202 | 24 | 0.416 |
| records-window | 2 | diagnostic | 16 | 51229 | 24198 | 20 | 24202 | 24 | 0.450 |
| records-window | 4 | diagnostic | 16 | 51229 | 24198 | 20 | 24202 | 24 | 0.415 |
| records-window | 8 | diagnostic | 16 | 51229 | 24198 | 20 | 24202 | 24 | 0.414 |
| scalar-snapshots | 1 | diagnostic | 2 | 33357 | 25580 | 9 | 25574 | 3 | 0.637 |
| scalar-snapshots | 2 | diagnostic | 2 | 33357 | 25580 | 9 | 25574 | 3 | 0.574 |
| scalar-snapshots | 4 | diagnostic | 2 | 33357 | 25580 | 9 | 25574 | 3 | 0.620 |
| scalar-snapshots | 8 | diagnostic | 2 | 33357 | 25580 | 9 | 25574 | 3 | 0.680 |

Sweep work distribution (workload collections only):

Small helper jobs contain fewer than 4,096 objects; native thread creation is counted separately.
these counts alone do not measure thread-startup cost.

| Case | Workers | Variant | Collections / parallel | Helper jobs | New native threads | Small helper jobs | Median helper objects | Median largest-list share in parallel collections |
|---|---:|---|---:|---:|---:|---:|---:|---:|
| parallel-snapshots | 1 | diagnostic | 111 / 0 | 0 | 0 | 0 | n/a | n/a |
| parallel-snapshots | 2 | diagnostic | 111 / 111 | 111 | 111 | 0 | 35270 | 56.2% |
| parallel-snapshots | 4 | diagnostic | 108 / 108 | 320 | 320 | 0 | 16069 | 28.9% |
| parallel-snapshots | 8 | diagnostic | 98 / 98 | 660 | 660 | 4 | 8711 | 15.8% |
| records-window | 1 | diagnostic | 48 / 0 | 0 | 0 | 0 | n/a | n/a |
| records-window | 2 | diagnostic | 48 / 0 | 0 | 0 | 0 | n/a | n/a |
| records-window | 4 | diagnostic | 48 / 0 | 0 | 0 | 0 | n/a | n/a |
| records-window | 8 | diagnostic | 48 / 0 | 0 | 0 | 0 | n/a | n/a |
| scalar-snapshots | 1 | diagnostic | 6 / 0 | 0 | 0 | 0 | n/a | n/a |
| scalar-snapshots | 2 | diagnostic | 6 / 0 | 0 | 0 | 0 | n/a | n/a |
| scalar-snapshots | 4 | diagnostic | 6 / 0 | 0 | 0 | 0 | n/a | n/a |
| scalar-snapshots | 8 | diagnostic | 6 / 0 | 0 | 0 | 0 | n/a | n/a |
