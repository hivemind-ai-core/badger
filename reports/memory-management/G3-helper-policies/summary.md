| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | sweep-reuse | 443.529 [440.513, 462.592] | 70.914 | 58.3 |
| backpressure | 1 | sweep-scoped | 445.591 [437.403, 475.618] | 71.007 | 58.2 |
| backpressure | 2 | sweep-reuse | 304.222 [294.502, 323.110] | 58.896 | 62.4 |
| backpressure | 2 | sweep-scoped | 305.599 [297.701, 308.742] | 62.593 | 63.8 |
| backpressure | 4 | sweep-reuse | 190.799 [184.324, 201.040] | 49.875 | 70.7 |
| backpressure | 4 | sweep-scoped | 195.331 [187.571, 203.121] | 50.144 | 72.0 |
| backpressure | 8 | sweep-reuse | 132.876 [132.119, 140.170] | 39.060 | 81.6 |
| backpressure | 8 | sweep-scoped | 133.439 [128.904, 176.129] | 42.864 | 81.5 |
| balanced | 1 | sweep-reuse | 464.699 [449.635, 481.751] | 86.062 | 85.7 |
| balanced | 1 | sweep-scoped | 456.673 [447.567, 482.000] | 82.780 | 85.6 |
| balanced | 2 | sweep-reuse | 359.554 [302.850, 378.379] | 79.380 | 102.8 |
| balanced | 2 | sweep-scoped | 360.027 [331.278, 370.003] | 79.023 | 108.6 |
| balanced | 4 | sweep-reuse | 197.298 [191.723, 211.011] | 62.112 | 101.6 |
| balanced | 4 | sweep-scoped | 196.090 [193.732, 211.224] | 62.643 | 98.2 |
| balanced | 8 | sweep-reuse | 142.093 [136.831, 147.178] | 55.394 | 102.2 |
| balanced | 8 | sweep-scoped | 143.181 [136.852, 155.615] | 58.767 | 101.4 |
| cpu | 1 | sweep-reuse | 875.540 [868.106, 912.414] | 80.541 | 81.0 |
| cpu | 1 | sweep-scoped | 879.775 [861.013, 908.773] | 81.530 | 81.1 |
| cpu | 2 | sweep-reuse | 503.594 [493.588, 560.730] | 71.546 | 88.8 |
| cpu | 2 | sweep-scoped | 528.713 [504.018, 579.469] | 72.850 | 91.4 |
| cpu | 4 | sweep-reuse | 287.421 [285.388, 302.892] | 60.177 | 96.6 |
| cpu | 4 | sweep-scoped | 293.476 [285.766, 318.756] | 62.011 | 99.0 |
| cpu | 8 | sweep-reuse | 318.278 [311.303, 334.422] | 58.724 | 128.9 |
| cpu | 8 | sweep-scoped | 325.372 [307.158, 338.175] | 62.990 | 127.8 |
| fanout | 1 | sweep-reuse | 1175.191 [1150.567, 1262.042] | 193.224 | 88.5 |
| fanout | 1 | sweep-scoped | 1239.367 [1156.914, 1268.124] | 202.264 | 88.6 |
| fanout | 2 | sweep-reuse | 1070.832 [922.847, 1085.965] | 194.040 | 112.4 |
| fanout | 2 | sweep-scoped | 1078.448 [927.626, 1101.817] | 206.455 | 111.0 |
| fanout | 4 | sweep-reuse | 728.286 [688.375, 737.073] | 156.449 | 129.1 |
| fanout | 4 | sweep-scoped | 720.529 [687.874, 721.965] | 160.132 | 122.3 |
| fanout | 8 | sweep-reuse | 780.093 [773.193, 791.823] | 138.777 | 108.9 |
| fanout | 8 | sweep-scoped | 774.996 [768.735, 804.198] | 143.449 | 104.4 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | sweep-reuse | 81920 | 0.992 | 4.864 |
| backpressure | 1 | sweep-scoped | 81920 | 0.992 | 5.120 |
| backpressure | 2 | sweep-reuse | 81920 | 0.512 | 3.712 |
| backpressure | 2 | sweep-scoped | 81920 | 0.496 | 3.840 |
| backpressure | 4 | sweep-reuse | 81920 | 0.288 | 2.816 |
| backpressure | 4 | sweep-scoped | 81920 | 0.288 | 3.072 |
| backpressure | 8 | sweep-reuse | 81920 | 0.184 | 2.176 |
| backpressure | 8 | sweep-scoped | 81920 | 0.184 | 2.560 |
| balanced | 1 | sweep-reuse | 81920 | 15.360 | 32.768 |
| balanced | 1 | sweep-scoped | 81920 | 15.360 | 30.720 |
| balanced | 2 | sweep-reuse | 81920 | 11.264 | 26.624 |
| balanced | 2 | sweep-scoped | 81920 | 11.264 | 26.624 |
| balanced | 4 | sweep-reuse | 81920 | 6.144 | 18.432 |
| balanced | 4 | sweep-scoped | 81920 | 6.144 | 18.432 |
| balanced | 8 | sweep-reuse | 81920 | 4.352 | 15.872 |
| balanced | 8 | sweep-scoped | 81920 | 4.352 | 18.432 |
| cpu | 1 | sweep-reuse | 81920 | 29.696 | 43.008 |
| cpu | 1 | sweep-scoped | 81920 | 29.696 | 45.056 |
| cpu | 2 | sweep-reuse | 81920 | 17.408 | 29.696 |
| cpu | 2 | sweep-scoped | 81920 | 17.408 | 29.696 |
| cpu | 4 | sweep-reuse | 81920 | 9.216 | 21.504 |
| cpu | 4 | sweep-scoped | 81920 | 9.728 | 21.504 |
| cpu | 8 | sweep-reuse | 81920 | 10.752 | 23.552 |
| cpu | 8 | sweep-scoped | 81920 | 10.752 | 23.552 |
| fanout | 1 | sweep-reuse | 81920 | 38.912 | 59.392 |
| fanout | 1 | sweep-scoped | 81920 | 40.960 | 61.440 |
| fanout | 2 | sweep-reuse | 81920 | 34.816 | 55.296 |
| fanout | 2 | sweep-scoped | 81920 | 34.816 | 59.392 |
| fanout | 4 | sweep-reuse | 81920 | 22.528 | 45.056 |
| fanout | 4 | sweep-scoped | 81920 | 22.528 | 45.056 |
| fanout | 8 | sweep-reuse | 81920 | 25.600 | 40.960 |
| fanout | 8 | sweep-scoped | 81920 | 25.600 | 40.960 |

Diagnostic GC phases summed across measured repetitions (wall time):

- ('backpressure', 1, 'sweep-reuse'): sweep_finalize 312.017 ms, trace 20.285 ms, prepare 7.321 ms, weak_pages 5.534 ms, trim 1.817 ms, roots 1.042 ms, allocation_census 0.262 ms, finish 0.107 ms, accounting 0.095 ms, resume_world 0.088 ms, verify 0.016 ms, stop_world 0.013 ms
- ('backpressure', 1, 'sweep-scoped'): sweep_finalize 316.749 ms, trace 21.150 ms, prepare 7.281 ms, weak_pages 5.600 ms, trim 2.021 ms, roots 1.113 ms, allocation_census 0.284 ms, finish 0.128 ms, accounting 0.105 ms, resume_world 0.087 ms, verify 0.017 ms, stop_world 0.015 ms
- ('backpressure', 2, 'sweep-reuse'): sweep_finalize 260.519 ms, trace 20.912 ms, prepare 7.343 ms, weak_pages 5.782 ms, trim 2.004 ms, roots 1.109 ms, allocation_census 0.407 ms, stop_world 0.109 ms, finish 0.089 ms, accounting 0.089 ms, resume_world 0.051 ms, verify 0.016 ms
- ('backpressure', 2, 'sweep-scoped'): sweep_finalize 274.360 ms, trace 20.895 ms, prepare 7.469 ms, weak_pages 6.279 ms, trim 1.912 ms, roots 1.160 ms, stop_world 0.531 ms, allocation_census 0.418 ms, accounting 0.087 ms, finish 0.084 ms, resume_world 0.046 ms, verify 0.018 ms
- ('backpressure', 4, 'sweep-reuse'): sweep_finalize 208.636 ms, trace 21.469 ms, prepare 7.564 ms, weak_pages 6.899 ms, trim 1.816 ms, roots 1.223 ms, allocation_census 0.459 ms, stop_world 0.230 ms, finish 0.083 ms, accounting 0.082 ms, resume_world 0.040 ms, verify 0.018 ms
- ('backpressure', 4, 'sweep-scoped'): sweep_finalize 216.782 ms, trace 22.050 ms, prepare 7.801 ms, weak_pages 6.900 ms, trim 1.968 ms, roots 1.233 ms, stop_world 0.748 ms, allocation_census 0.458 ms, finish 0.084 ms, accounting 0.081 ms, resume_world 0.044 ms, verify 0.018 ms
- ('backpressure', 8, 'sweep-reuse'): sweep_finalize 159.289 ms, trace 21.288 ms, prepare 6.908 ms, weak_pages 6.037 ms, trim 1.895 ms, roots 1.227 ms, allocation_census 0.373 ms, stop_world 0.328 ms, accounting 0.153 ms, finish 0.084 ms, resume_world 0.048 ms, verify 0.016 ms
- ('backpressure', 8, 'sweep-scoped'): sweep_finalize 187.888 ms, trace 23.902 ms, prepare 8.082 ms, weak_pages 6.899 ms, trim 1.950 ms, roots 1.364 ms, allocation_census 0.390 ms, stop_world 0.187 ms, finish 0.080 ms, accounting 0.078 ms, resume_world 0.047 ms, verify 0.018 ms
- ('balanced', 1, 'sweep-reuse'): sweep_finalize 313.669 ms, trace 75.126 ms, prepare 12.830 ms, weak_pages 11.952 ms, trim 6.331 ms, roots 1.457 ms, allocation_census 0.478 ms, finish 0.114 ms, accounting 0.095 ms, resume_world 0.091 ms, verify 0.018 ms, stop_world 0.011 ms
- ('balanced', 1, 'sweep-scoped'): sweep_finalize 310.701 ms, trace 71.791 ms, prepare 14.032 ms, weak_pages 11.509 ms, trim 6.517 ms, roots 1.311 ms, allocation_census 0.459 ms, finish 0.113 ms, accounting 0.103 ms, resume_world 0.090 ms, verify 0.018 ms, stop_world 0.009 ms
- ('balanced', 2, 'sweep-reuse'): sweep_finalize 283.073 ms, trace 81.332 ms, prepare 15.248 ms, weak_pages 12.875 ms, trim 4.083 ms, roots 1.452 ms, allocation_census 0.532 ms, stop_world 0.202 ms, accounting 0.092 ms, finish 0.088 ms, resume_world 0.048 ms, verify 0.020 ms
- ('balanced', 2, 'sweep-scoped'): sweep_finalize 284.359 ms, trace 79.236 ms, prepare 12.912 ms, weak_pages 11.596 ms, trim 3.781 ms, roots 1.418 ms, allocation_census 0.452 ms, stop_world 0.443 ms, finish 0.089 ms, accounting 0.088 ms, resume_world 0.047 ms, verify 0.018 ms
- ('balanced', 4, 'sweep-reuse'): sweep_finalize 202.006 ms, trace 78.635 ms, prepare 12.606 ms, weak_pages 11.262 ms, trim 3.977 ms, roots 1.454 ms, stop_world 0.635 ms, allocation_census 0.585 ms, finish 0.096 ms, accounting 0.082 ms, resume_world 0.041 ms, verify 0.019 ms
- ('balanced', 4, 'sweep-scoped'): sweep_finalize 206.475 ms, trace 74.200 ms, prepare 12.454 ms, weak_pages 11.050 ms, trim 3.740 ms, roots 1.511 ms, stop_world 0.818 ms, allocation_census 0.486 ms, finish 0.083 ms, accounting 0.080 ms, resume_world 0.041 ms, verify 0.017 ms
- ('balanced', 8, 'sweep-reuse'): sweep_finalize 167.712 ms, trace 72.859 ms, weak_pages 13.167 ms, prepare 12.109 ms, trim 4.351 ms, roots 1.399 ms, allocation_census 0.492 ms, stop_world 0.299 ms, finish 0.086 ms, accounting 0.082 ms, resume_world 0.048 ms, verify 0.018 ms
- ('balanced', 8, 'sweep-scoped'): sweep_finalize 184.850 ms, trace 75.792 ms, weak_pages 12.763 ms, prepare 12.078 ms, trim 4.047 ms, roots 1.443 ms, allocation_census 0.634 ms, stop_world 0.349 ms, finish 0.084 ms, accounting 0.082 ms, resume_world 0.046 ms, verify 0.021 ms
- ('cpu', 1, 'sweep-reuse'): sweep_finalize 307.546 ms, trace 64.879 ms, prepare 11.965 ms, weak_pages 10.562 ms, trim 5.971 ms, roots 1.619 ms, allocation_census 0.441 ms, finish 0.114 ms, accounting 0.095 ms, resume_world 0.078 ms, verify 0.017 ms, stop_world 0.012 ms
- ('cpu', 1, 'sweep-scoped'): sweep_finalize 313.309 ms, trace 65.242 ms, prepare 12.214 ms, weak_pages 10.508 ms, trim 5.111 ms, roots 1.685 ms, allocation_census 0.446 ms, finish 0.122 ms, accounting 0.110 ms, resume_world 0.107 ms, verify 0.017 ms, stop_world 0.013 ms
- ('cpu', 2, 'sweep-reuse'): sweep_finalize 261.964 ms, trace 67.695 ms, prepare 12.820 ms, weak_pages 11.873 ms, trim 2.888 ms, roots 1.643 ms, allocation_census 0.539 ms, stop_world 0.421 ms, accounting 0.088 ms, finish 0.085 ms, resume_world 0.043 ms, verify 0.026 ms
- ('cpu', 2, 'sweep-scoped'): sweep_finalize 268.006 ms, trace 73.548 ms, prepare 12.195 ms, weak_pages 11.823 ms, trim 3.249 ms, roots 1.735 ms, allocation_census 0.548 ms, stop_world 0.194 ms, accounting 0.087 ms, finish 0.084 ms, resume_world 0.045 ms, verify 0.037 ms
- ('cpu', 4, 'sweep-reuse'): sweep_finalize 203.249 ms, trace 64.453 ms, prepare 11.417 ms, weak_pages 10.787 ms, trim 3.457 ms, roots 1.585 ms, stop_world 0.579 ms, allocation_census 0.475 ms, finish 0.086 ms, accounting 0.081 ms, resume_world 0.043 ms, verify 0.018 ms
- ('cpu', 4, 'sweep-scoped'): sweep_finalize 214.899 ms, trace 67.759 ms, prepare 11.572 ms, weak_pages 10.680 ms, trim 3.106 ms, roots 1.577 ms, allocation_census 0.508 ms, stop_world 0.505 ms, accounting 0.089 ms, finish 0.083 ms, resume_world 0.042 ms, verify 0.018 ms
- ('cpu', 8, 'sweep-reuse'): sweep_finalize 192.858 ms, trace 68.479 ms, prepare 12.470 ms, weak_pages 11.740 ms, trim 3.319 ms, roots 1.486 ms, allocation_census 0.504 ms, stop_world 0.327 ms, finish 0.088 ms, accounting 0.081 ms, resume_world 0.042 ms, verify 0.019 ms
- ('cpu', 8, 'sweep-scoped'): sweep_finalize 206.281 ms, trace 67.998 ms, trim 19.257 ms, prepare 13.172 ms, weak_pages 11.483 ms, roots 1.474 ms, allocation_census 0.512 ms, stop_world 0.356 ms, finish 0.083 ms, accounting 0.078 ms, resume_world 0.043 ms, verify 0.018 ms
- ('fanout', 1, 'sweep-reuse'): sweep_finalize 718.235 ms, trace 161.068 ms, prepare 29.030 ms, weak_pages 25.829 ms, trim 15.341 ms, roots 11.001 ms, allocation_census 0.501 ms, finish 0.225 ms, accounting 0.191 ms, resume_world 0.144 ms, verify 0.034 ms, stop_world 0.019 ms
- ('fanout', 1, 'sweep-scoped'): sweep_finalize 745.885 ms, trace 167.693 ms, prepare 28.854 ms, weak_pages 25.714 ms, trim 15.354 ms, roots 11.493 ms, allocation_census 0.534 ms, finish 0.267 ms, accounting 0.219 ms, resume_world 0.166 ms, verify 0.035 ms, stop_world 0.016 ms
- ('fanout', 2, 'sweep-reuse'): sweep_finalize 686.674 ms, trace 190.799 ms, trim 36.297 ms, prepare 31.100 ms, weak_pages 28.760 ms, roots 11.164 ms, allocation_census 0.667 ms, finish 0.195 ms, accounting 0.194 ms, stop_world 0.154 ms, resume_world 0.120 ms, verify 0.036 ms
- ('fanout', 2, 'sweep-scoped'): sweep_finalize 720.280 ms, trace 191.966 ms, trim 32.132 ms, prepare 31.365 ms, weak_pages 28.312 ms, roots 11.159 ms, allocation_census 0.624 ms, accounting 0.191 ms, finish 0.173 ms, stop_world 0.144 ms, resume_world 0.121 ms, verify 0.036 ms
- ('fanout', 4, 'sweep-reuse'): sweep_finalize 509.725 ms, trace 183.798 ms, prepare 31.714 ms, weak_pages 30.451 ms, roots 10.685 ms, trim 9.020 ms, allocation_census 0.664 ms, stop_world 0.262 ms, accounting 0.172 ms, finish 0.167 ms, resume_world 0.097 ms, verify 0.034 ms
- ('fanout', 4, 'sweep-scoped'): sweep_finalize 514.973 ms, trace 192.684 ms, weak_pages 29.624 ms, prepare 29.396 ms, roots 10.562 ms, trim 9.284 ms, stop_world 0.795 ms, allocation_census 0.632 ms, accounting 0.187 ms, finish 0.162 ms, resume_world 0.102 ms, verify 0.037 ms
- ('fanout', 8, 'sweep-reuse'): sweep_finalize 404.508 ms, trace 202.535 ms, prepare 29.658 ms, weak_pages 27.765 ms, roots 11.045 ms, trim 10.568 ms, allocation_census 0.765 ms, stop_world 0.442 ms, accounting 0.173 ms, finish 0.171 ms, resume_world 0.097 ms, verify 0.034 ms
- ('fanout', 8, 'sweep-scoped'): sweep_finalize 426.372 ms, trace 197.149 ms, prepare 30.384 ms, weak_pages 29.339 ms, roots 12.077 ms, trim 8.695 ms, allocation_census 0.762 ms, stop_world 0.473 ms, accounting 0.164 ms, finish 0.149 ms, resume_world 0.088 ms, verify 0.045 ms

Sweep work distribution (workload collections only):

Small helper jobs contain fewer than 4,096 objects; native thread creation is counted separately.
these counts alone do not measure thread-startup cost.

| Case | Workers | Variant | Collections / parallel | Helper jobs | New native threads | Small helper jobs | Median helper objects | Median largest-list share in parallel collections |
|---|---:|---|---:|---:|---:|---:|---:|---:|
| backpressure | 1 | sweep-reuse | 110 / 0 | 0 | 0 | 0 | n/a | n/a |
| backpressure | 1 | sweep-scoped | 110 / 0 | 0 | 0 | 0 | n/a | n/a |
| backpressure | 2 | sweep-reuse | 110 / 110 | 110 | 5 | 0 | 67156 | 52.0% |
| backpressure | 2 | sweep-scoped | 110 / 110 | 110 | 110 | 0 | 67519 | 51.6% |
| backpressure | 4 | sweep-reuse | 110 / 110 | 330 | 15 | 2 | 34630 | 27.6% |
| backpressure | 4 | sweep-scoped | 110 / 110 | 330 | 330 | 0 | 33903 | 27.6% |
| backpressure | 8 | sweep-reuse | 110 / 110 | 770 | 35 | 0 | 17260 | 14.5% |
| backpressure | 8 | sweep-scoped | 110 / 110 | 770 | 770 | 0 | 17191 | 14.2% |
| balanced | 1 | sweep-reuse | 110 / 0 | 0 | 0 | 0 | n/a | n/a |
| balanced | 1 | sweep-scoped | 110 / 0 | 0 | 0 | 0 | n/a | n/a |
| balanced | 2 | sweep-reuse | 110 / 110 | 110 | 5 | 1 | 68010 | 55.0% |
| balanced | 2 | sweep-scoped | 110 / 109 | 109 | 109 | 0 | 67692 | 55.4% |
| balanced | 4 | sweep-reuse | 110 / 110 | 330 | 15 | 2 | 33782 | 27.7% |
| balanced | 4 | sweep-scoped | 110 / 110 | 330 | 330 | 2 | 33182 | 27.6% |
| balanced | 8 | sweep-reuse | 110 / 110 | 770 | 35 | 0 | 17086 | 14.7% |
| balanced | 8 | sweep-scoped | 110 / 110 | 770 | 770 | 0 | 17200 | 14.4% |
| cpu | 1 | sweep-reuse | 110 / 0 | 0 | 0 | 0 | n/a | n/a |
| cpu | 1 | sweep-scoped | 110 / 0 | 0 | 0 | 0 | n/a | n/a |
| cpu | 2 | sweep-reuse | 110 / 110 | 110 | 5 | 0 | 68142 | 52.3% |
| cpu | 2 | sweep-scoped | 110 / 110 | 110 | 110 | 0 | 68166 | 52.2% |
| cpu | 4 | sweep-reuse | 110 / 110 | 330 | 15 | 0 | 33940 | 28.7% |
| cpu | 4 | sweep-scoped | 110 / 110 | 330 | 330 | 0 | 33980 | 28.7% |
| cpu | 8 | sweep-reuse | 110 / 110 | 770 | 35 | 0 | 13874 | 24.3% |
| cpu | 8 | sweep-scoped | 110 / 110 | 770 | 770 | 0 | 15004 | 24.0% |
| fanout | 1 | sweep-reuse | 210 / 0 | 0 | 0 | 0 | n/a | n/a |
| fanout | 1 | sweep-scoped | 210 / 0 | 0 | 0 | 0 | n/a | n/a |
| fanout | 2 | sweep-reuse | 210 / 210 | 210 | 5 | 0 | 82122 | 63.1% |
| fanout | 2 | sweep-scoped | 210 / 210 | 210 | 210 | 0 | 81876 | 63.5% |
| fanout | 4 | sweep-reuse | 210 / 210 | 630 | 15 | 5 | 40388 | 27.1% |
| fanout | 4 | sweep-scoped | 210 / 210 | 630 | 630 | 9 | 40508 | 27.0% |
| fanout | 8 | sweep-reuse | 206 / 206 | 1442 | 35 | 0 | 20341 | 13.8% |
| fanout | 8 | sweep-scoped | 205 / 205 | 1435 | 1435 | 0 | 20413 | 13.7% |
