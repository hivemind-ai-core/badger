| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| map-api-composed | 1 | baseline | 207.045 [206.982, 217.525] | 32.500 | 49.4 |
| map-api-composed | 1 | candidate | 152.425 [149.515, 164.032] | 23.912 | 47.1 |
| map-api-composed | 8 | baseline | 217.364 [212.417, 221.676] | 33.371 | 48.7 |
| map-api-composed | 8 | candidate | 153.153 [149.667, 158.098] | 21.931 | 48.2 |
| map-api-entry-control | 1 | baseline | 27.405 [27.016, 31.180] | 2.046 | 47.6 |
| map-api-entry-control | 1 | candidate | 26.284 [25.901, 28.296] | 2.028 | 46.3 |
| map-api-entry-control | 8 | baseline | 30.171 [29.887, 30.526] | 2.042 | 46.9 |
| map-api-entry-control | 8 | candidate | 30.304 [29.676, 31.232] | 2.147 | 47.5 |
| map-api-filter | 1 | baseline | 38.774 [37.887, 41.088] | 4.433 | 44.2 |
| map-api-filter | 1 | candidate | 21.948 [21.737, 25.065] | 2.925 | 43.0 |
| map-api-filter | 8 | baseline | 41.295 [40.105, 41.553] | 4.467 | 43.2 |
| map-api-filter | 8 | candidate | 25.270 [25.011, 26.367] | 3.224 | 44.5 |
| map-api-merge | 1 | baseline | 114.587 [113.776, 117.333] | 14.504 | 54.7 |
| map-api-merge | 1 | candidate | 82.408 [82.366, 86.778] | 10.249 | 50.0 |
| map-api-merge | 8 | baseline | 124.077 [118.294, 127.698] | 14.750 | 54.1 |
| map-api-merge | 8 | candidate | 86.345 [85.234, 89.465] | 9.781 | 52.6 |
| map-api-parallel | 1 | baseline | 256.031 [254.358, 262.549] | 53.102 | 67.7 |
| map-api-parallel | 1 | candidate | 192.398 [184.258, 194.032] | 36.748 | 59.2 |
| map-api-parallel | 8 | baseline | 96.707 [95.506, 102.570] | 39.501 | 87.9 |
| map-api-parallel | 8 | candidate | 70.723 [70.330, 83.495] | 27.708 | 79.6 |
| map-api-projections | 1 | baseline | 41.211 [40.566, 41.214] | 2.295 | 47.5 |
| map-api-projections | 1 | candidate | 9.810 [9.770, 14.515] | 0.000 | 31.1 |
| map-api-projections | 8 | baseline | 44.988 [43.798, 49.741] | 2.314 | 48.2 |
| map-api-projections | 8 | candidate | 14.607 [12.802, 15.876] | 0.000 | 32.6 |
| map-api-values | 1 | baseline | 57.322 [55.338, 62.977] | 8.932 | 44.8 |
| map-api-values | 1 | candidate | 37.719 [37.499, 42.726] | 5.071 | 43.1 |
| map-api-values | 8 | baseline | 56.899 [56.731, 62.088] | 8.090 | 44.7 |
| map-api-values | 8 | candidate | 41.168 [40.812, 43.230] | 5.213 | 44.3 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| map-api-composed | 1 | -26.4% | -26.4% | -4.8% | 21 / 19 |
| map-api-composed | 8 | -29.5% | -34.3% | -1.0% | 21 / 19 |
| map-api-entry-control | 1 | -4.1% | -0.9% | -2.7% | 1 / 1 |
| map-api-entry-control | 8 | +0.4% | +5.2% | +1.3% | 1 / 1 |
| map-api-filter | 1 | -43.4% | -34.0% | -2.7% | 3 / 3 |
| map-api-filter | 8 | -38.8% | -27.8% | +3.0% | 3 / 3 |
| map-api-merge | 1 | -28.1% | -29.3% | -8.5% | 8 / 7 |
| map-api-merge | 8 | -30.4% | -33.7% | -2.9% | 8 / 7 |
| map-api-parallel | 1 | -24.9% | -30.8% | -12.5% | 21 / 19 |
| map-api-parallel | 8 | -26.9% | -29.9% | -9.4% | 21 / 19 |
| map-api-projections | 1 | -76.2% | -100.0% | -34.5% | 1 / 0 |
| map-api-projections | 8 | -67.5% | -100.0% | -32.5% | 1 / 0 |
| map-api-values | 1 | -34.2% | -43.2% | -3.8% | 6 / 5 |
| map-api-values | 8 | -27.6% | -35.6% | -0.7% | 6 / 5 |

Diagnostic GC phases summed across measured repetitions (wall time):

- ('map-api-composed', 1, 'baseline'): allocation_census 46.110 ms, sweep_finalize 39.615 ms, trace 6.667 ms, prepare 2.721 ms, trim 1.131 ms, weak_pages 1.130 ms, roots 0.207 ms, finish 0.052 ms, accounting 0.030 ms, resume_world 0.026 ms, stop_world 0.013 ms, verify 0.007 ms
- ('map-api-composed', 1, 'candidate'): allocation_census 32.859 ms, sweep_finalize 27.877 ms, trace 4.802 ms, trim 4.657 ms, prepare 2.397 ms, weak_pages 1.017 ms, roots 0.169 ms, finish 0.043 ms, accounting 0.026 ms, resume_world 0.024 ms, stop_world 0.011 ms, verify 0.006 ms
- ('map-api-composed', 8, 'baseline'): allocation_census 47.008 ms, sweep_finalize 39.128 ms, trace 6.793 ms, prepare 2.806 ms, weak_pages 1.273 ms, trim 1.205 ms, roots 0.207 ms, finish 0.052 ms, stop_world 0.042 ms, accounting 0.034 ms, resume_world 0.017 ms, verify 0.008 ms
- ('map-api-composed', 8, 'candidate'): allocation_census 30.741 ms, sweep_finalize 26.699 ms, trace 4.505 ms, prepare 2.202 ms, trim 2.201 ms, weak_pages 0.854 ms, roots 0.168 ms, stop_world 0.040 ms, finish 0.039 ms, accounting 0.025 ms, resume_world 0.010 ms, verify 0.005 ms
- ('map-api-entry-control', 1, 'baseline'): allocation_census 2.999 ms, sweep_finalize 2.727 ms, trace 0.270 ms, roots 0.055 ms, resume_world 0.020 ms, prepare 0.017 ms, accounting 0.003 ms, finish 0.003 ms, trim 0.001 ms, stop_world 0.001 ms, verify 0.001 ms, weak_pages 0.001 ms
- ('map-api-entry-control', 1, 'candidate'): allocation_census 2.884 ms, sweep_finalize 2.755 ms, trace 0.271 ms, roots 0.059 ms, prepare 0.019 ms, resume_world 0.017 ms, finish 0.003 ms, accounting 0.002 ms, stop_world 0.001 ms, trim 0.001 ms, weak_pages 0.001 ms, verify 0.000 ms
- ('map-api-entry-control', 8, 'baseline'): allocation_census 2.973 ms, sweep_finalize 2.722 ms, trace 0.309 ms, roots 0.061 ms, prepare 0.018 ms, stop_world 0.006 ms, finish 0.003 ms, accounting 0.002 ms, resume_world 0.001 ms, trim 0.001 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('map-api-entry-control', 8, 'candidate'): allocation_census 3.183 ms, sweep_finalize 2.801 ms, trace 0.288 ms, roots 0.051 ms, prepare 0.016 ms, stop_world 0.006 ms, finish 0.003 ms, accounting 0.002 ms, resume_world 0.001 ms, trim 0.001 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('map-api-filter', 1, 'baseline'): allocation_census 7.818 ms, sweep_finalize 4.677 ms, trace 0.568 ms, roots 0.049 ms, prepare 0.031 ms, resume_world 0.020 ms, finish 0.009 ms, accounting 0.004 ms, stop_world 0.003 ms, trim 0.002 ms, verify 0.001 ms, weak_pages 0.001 ms
- ('map-api-filter', 1, 'candidate'): allocation_census 5.846 ms, sweep_finalize 2.942 ms, trace 0.457 ms, roots 0.039 ms, prepare 0.031 ms, resume_world 0.013 ms, finish 0.007 ms, accounting 0.004 ms, trim 0.002 ms, stop_world 0.001 ms, weak_pages 0.001 ms, verify 0.001 ms
- ('map-api-filter', 8, 'baseline'): allocation_census 7.957 ms, sweep_finalize 4.601 ms, trace 0.604 ms, roots 0.050 ms, prepare 0.030 ms, stop_world 0.009 ms, finish 0.008 ms, accounting 0.004 ms, weak_pages 0.002 ms, resume_world 0.002 ms, trim 0.002 ms, verify 0.001 ms
- ('map-api-filter', 8, 'candidate'): allocation_census 6.182 ms, sweep_finalize 2.855 ms, trace 0.420 ms, roots 0.037 ms, prepare 0.027 ms, stop_world 0.015 ms, finish 0.007 ms, accounting 0.004 ms, resume_world 0.002 ms, trim 0.001 ms, weak_pages 0.001 ms, verify 0.001 ms
- ('map-api-merge', 1, 'baseline'): allocation_census 21.858 ms, sweep_finalize 16.033 ms, trace 5.012 ms, roots 0.083 ms, prepare 0.068 ms, finish 0.021 ms, resume_world 0.018 ms, accounting 0.013 ms, stop_world 0.006 ms, trim 0.005 ms, verify 0.004 ms, weak_pages 0.003 ms
- ('map-api-merge', 1, 'candidate'): allocation_census 16.386 ms, sweep_finalize 10.594 ms, trace 3.729 ms, roots 0.061 ms, prepare 0.056 ms, resume_world 0.017 ms, finish 0.015 ms, accounting 0.009 ms, stop_world 0.005 ms, trim 0.004 ms, weak_pages 0.004 ms, verify 0.003 ms
- ('map-api-merge', 8, 'baseline'): allocation_census 22.730 ms, sweep_finalize 16.649 ms, trace 5.740 ms, roots 0.080 ms, prepare 0.070 ms, stop_world 0.021 ms, finish 0.020 ms, accounting 0.012 ms, resume_world 0.006 ms, trim 0.005 ms, weak_pages 0.003 ms, verify 0.003 ms
- ('map-api-merge', 8, 'candidate'): allocation_census 15.030 ms, sweep_finalize 10.371 ms, trace 3.726 ms, roots 0.058 ms, prepare 0.053 ms, stop_world 0.019 ms, finish 0.015 ms, accounting 0.010 ms, resume_world 0.005 ms, trim 0.004 ms, weak_pages 0.004 ms, verify 0.003 ms
- ('map-api-parallel', 1, 'baseline'): allocation_census 59.218 ms, sweep_finalize 48.401 ms, trace 34.118 ms, trim 6.097 ms, prepare 5.847 ms, weak_pages 4.527 ms, roots 0.571 ms, finish 0.051 ms, accounting 0.032 ms, resume_world 0.032 ms, stop_world 0.011 ms, verify 0.009 ms
- ('map-api-parallel', 1, 'candidate'): allocation_census 40.405 ms, sweep_finalize 36.243 ms, trace 18.066 ms, trim 4.672 ms, prepare 3.871 ms, weak_pages 2.663 ms, roots 0.357 ms, finish 0.047 ms, accounting 0.029 ms, resume_world 0.026 ms, stop_world 0.009 ms, verify 0.007 ms
- ('map-api-parallel', 8, 'baseline'): allocation_census 52.733 ms, trace 30.447 ms, sweep_finalize 19.894 ms, prepare 5.670 ms, weak_pages 4.768 ms, trim 4.640 ms, stop_world 1.740 ms, roots 0.874 ms, finish 0.057 ms, accounting 0.034 ms, resume_world 0.020 ms, verify 0.008 ms
- ('map-api-parallel', 8, 'candidate'): allocation_census 34.841 ms, sweep_finalize 17.623 ms, trace 15.347 ms, trim 9.799 ms, prepare 3.869 ms, weak_pages 2.899 ms, stop_world 0.978 ms, roots 0.474 ms, finish 0.049 ms, accounting 0.031 ms, resume_world 0.015 ms, verify 0.008 ms
- ('map-api-projections', 1, 'baseline'): allocation_census 3.363 ms, sweep_finalize 3.112 ms, trace 0.288 ms, roots 0.037 ms, resume_world 0.018 ms, prepare 0.017 ms, finish 0.003 ms, accounting 0.002 ms, stop_world 0.001 ms, trim 0.001 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('map-api-projections', 8, 'baseline'): allocation_census 3.426 ms, sweep_finalize 3.261 ms, trace 0.292 ms, roots 0.034 ms, prepare 0.016 ms, stop_world 0.006 ms, finish 0.003 ms, accounting 0.002 ms, resume_world 0.001 ms, trim 0.001 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('map-api-values', 1, 'baseline'): allocation_census 15.146 ms, sweep_finalize 9.638 ms, trace 0.935 ms, roots 0.064 ms, prepare 0.049 ms, resume_world 0.015 ms, finish 0.015 ms, accounting 0.009 ms, stop_world 0.004 ms, trim 0.003 ms, weak_pages 0.002 ms, verify 0.002 ms
- ('map-api-values', 1, 'candidate'): allocation_census 8.475 ms, sweep_finalize 6.684 ms, trace 0.565 ms, roots 0.054 ms, prepare 0.039 ms, resume_world 0.023 ms, finish 0.012 ms, accounting 0.006 ms, stop_world 0.003 ms, trim 0.002 ms, weak_pages 0.002 ms, verify 0.001 ms
- ('map-api-values', 8, 'baseline'): allocation_census 12.826 ms, sweep_finalize 10.186 ms, trace 1.018 ms, roots 0.057 ms, prepare 0.045 ms, stop_world 0.014 ms, finish 0.014 ms, accounting 0.009 ms, resume_world 0.004 ms, trim 0.003 ms, weak_pages 0.003 ms, verify 0.002 ms
- ('map-api-values', 8, 'candidate'): allocation_census 8.207 ms, sweep_finalize 6.419 ms, trace 0.524 ms, roots 0.050 ms, prepare 0.038 ms, stop_world 0.014 ms, finish 0.011 ms, accounting 0.006 ms, resume_world 0.003 ms, weak_pages 0.003 ms, trim 0.002 ms, verify 0.001 ms

Trace cardinality: medians of workload totals per fresh process.
Candidates include duplicate, old and immortal pointers; trace counts include remembered parents.
Timing is instrumented wall time, not per-object latency. Separate medians need not conserve counts.

| Case | Workers | Variant | Collections | Candidates | Newly marked | Marked leaves | Traced | Remembered traces | Trace ms |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|
| map-api-composed | 1 | baseline | 21 | 139560 | 117806 | 24093 | 94069 | 356 | 2.228 |
| map-api-composed | 1 | candidate | 19 | 81452 | 77659 | 6171 | 71704 | 216 | 1.508 |
| map-api-composed | 8 | baseline | 21 | 139560 | 117806 | 24093 | 94069 | 356 | 2.333 |
| map-api-composed | 8 | candidate | 19 | 81452 | 77659 | 6171 | 71704 | 216 | 1.466 |
| map-api-entry-control | 1 | baseline | 1 | 14267 | 3473 | 2057 | 1416 | 0 | 0.090 |
| map-api-entry-control | 1 | candidate | 1 | 14267 | 3473 | 2057 | 1416 | 0 | 0.091 |
| map-api-entry-control | 8 | baseline | 1 | 14267 | 3473 | 2057 | 1416 | 0 | 0.089 |
| map-api-entry-control | 8 | candidate | 1 | 14267 | 3473 | 2057 | 1416 | 0 | 0.095 |
| map-api-filter | 1 | baseline | 3 | 30596 | 7629 | 2057 | 5592 | 20 | 0.184 |
| map-api-filter | 1 | candidate | 3 | 23328 | 4066 | 2057 | 2029 | 20 | 0.148 |
| map-api-filter | 8 | baseline | 3 | 30596 | 7629 | 2057 | 5592 | 20 | 0.197 |
| map-api-filter | 8 | candidate | 3 | 23328 | 4066 | 2057 | 2029 | 20 | 0.139 |
| map-api-merge | 1 | baseline | 8 | 176729 | 88599 | 38417 | 50591 | 409 | 1.684 |
| map-api-merge | 1 | candidate | 7 | 138410 | 69554 | 33801 | 36210 | 457 | 1.220 |
| map-api-merge | 8 | baseline | 8 | 176729 | 88599 | 38417 | 50591 | 409 | 1.740 |
| map-api-merge | 8 | candidate | 7 | 138410 | 69554 | 33801 | 36210 | 457 | 1.161 |
| map-api-parallel | 1 | baseline | 21 | 633635 | 499497 | 216989 | 284460 | 1952 | 11.473 |
| map-api-parallel | 1 | candidate | 19 | 261049 | 229169 | 49203 | 181327 | 1361 | 6.008 |
| map-api-parallel | 8 | baseline | 21 | 668896 | 498285 | 195333 | 304081 | 1792 | 9.975 |
| map-api-parallel | 8 | candidate | 19 | 258503 | 228507 | 47155 | 182473 | 1249 | 5.008 |
| map-api-projections | 1 | baseline | 1 | 14557 | 3743 | 2057 | 1686 | 0 | 0.094 |
| map-api-projections | 8 | baseline | 1 | 14557 | 3743 | 2057 | 1686 | 0 | 0.093 |
| map-api-values | 1 | baseline | 6 | 22759 | 15424 | 2057 | 13417 | 50 | 0.311 |
| map-api-values | 1 | candidate | 5 | 9306 | 8121 | 2057 | 6104 | 40 | 0.187 |
| map-api-values | 8 | baseline | 6 | 22759 | 15424 | 2057 | 13417 | 50 | 0.336 |
| map-api-values | 8 | candidate | 5 | 9306 | 8121 | 2057 | 6104 | 40 | 0.173 |

Sweep work distribution (workload collections only):

Small helper jobs contain fewer than 4,096 objects; native thread creation is counted separately.
these counts alone do not measure thread-startup cost.

| Case | Workers | Variant | Collections / parallel | Helper jobs | New native threads | Small helper jobs | Median helper objects | Median largest-list share in parallel collections |
|---|---:|---|---:|---:|---:|---:|---:|---:|
| map-api-composed | 1 | baseline | 63 / 0 | 0 | 0 | 0 | n/a | n/a |
| map-api-composed | 1 | candidate | 57 / 0 | 0 | 0 | 0 | n/a | n/a |
| map-api-composed | 8 | baseline | 63 / 0 | 0 | 0 | 0 | n/a | n/a |
| map-api-composed | 8 | candidate | 57 / 0 | 0 | 0 | 0 | n/a | n/a |
| map-api-entry-control | 1 | baseline | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| map-api-entry-control | 1 | candidate | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| map-api-entry-control | 8 | baseline | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| map-api-entry-control | 8 | candidate | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| map-api-filter | 1 | baseline | 9 / 0 | 0 | 0 | 0 | n/a | n/a |
| map-api-filter | 1 | candidate | 9 / 0 | 0 | 0 | 0 | n/a | n/a |
| map-api-filter | 8 | baseline | 9 / 0 | 0 | 0 | 0 | n/a | n/a |
| map-api-filter | 8 | candidate | 9 / 0 | 0 | 0 | 0 | n/a | n/a |
| map-api-merge | 1 | baseline | 24 / 0 | 0 | 0 | 0 | n/a | n/a |
| map-api-merge | 1 | candidate | 21 / 0 | 0 | 0 | 0 | n/a | n/a |
| map-api-merge | 8 | baseline | 24 / 0 | 0 | 0 | 0 | n/a | n/a |
| map-api-merge | 8 | candidate | 21 / 0 | 0 | 0 | 0 | n/a | n/a |
| map-api-parallel | 1 | baseline | 63 / 0 | 0 | 0 | 0 | n/a | n/a |
| map-api-parallel | 1 | candidate | 57 / 0 | 0 | 0 | 0 | n/a | n/a |
| map-api-parallel | 8 | baseline | 63 / 63 | 438 | 438 | 6 | 11795 | 15.7% |
| map-api-parallel | 8 | candidate | 57 / 57 | 393 | 393 | 13 | 6880 | 15.4% |
| map-api-projections | 1 | baseline | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| map-api-projections | 8 | baseline | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| map-api-values | 1 | baseline | 18 / 0 | 0 | 0 | 0 | n/a | n/a |
| map-api-values | 1 | candidate | 15 / 0 | 0 | 0 | 0 | n/a | n/a |
| map-api-values | 8 | baseline | 18 / 0 | 0 | 0 | 0 | n/a | n/a |
| map-api-values | 8 | candidate | 15 / 0 | 0 | 0 | 0 | n/a | n/a |
