| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| parallel-maps | 1 | refill2048 | 204.345 [202.937, 208.007] | 56.188 | 41.4 |
| parallel-maps | 1 | refill256 | 208.590 [206.436, 209.196] | 56.776 | 40.8 |
| parallel-maps | 1 | refill512 | 208.027 [200.797, 223.115] | 54.285 | 40.8 |
| parallel-maps | 8 | refill2048 | 65.261 [64.172, 74.148] | 23.202 | 66.3 |
| parallel-maps | 8 | refill256 | 72.910 [72.498, 75.641] | 24.397 | 68.9 |
| parallel-maps | 8 | refill512 | 68.353 [66.365, 71.310] | 23.251 | 66.6 |
| parallel-snapshots | 1 | refill2048 | 291.095 [286.390, 300.070] | 96.098 | 77.8 |
| parallel-snapshots | 1 | refill256 | 324.949 [310.434, 357.507] | 109.817 | 74.6 |
| parallel-snapshots | 1 | refill512 | 326.649 [306.888, 330.100] | 107.944 | 75.4 |
| parallel-snapshots | 8 | refill2048 | 414.009 [411.654, 417.257] | 121.864 | 136.6 |
| parallel-snapshots | 8 | refill256 | 450.012 [428.464, 457.881] | 126.341 | 124.7 |
| parallel-snapshots | 8 | refill512 | 453.393 [434.964, 455.621] | 125.017 | 124.3 |
| pool-burst-1 | 1 | refill2048 | 64.918 [63.248, 68.025] | 18.358 | 62.1 |
| pool-burst-1 | 1 | refill256 | 78.212 [78.160, 78.519] | 18.950 | 61.9 |
| pool-burst-1 | 1 | refill512 | 83.180 [74.726, 89.237] | 19.784 | 62.9 |
| pool-burst-1 | 8 | refill2048 | 76.984 [67.265, 77.163] | 18.622 | 64.0 |
| pool-burst-1 | 8 | refill256 | 85.125 [81.101, 103.434] | 21.019 | 62.4 |
| pool-burst-1 | 8 | refill512 | 82.716 [79.199, 83.577] | 20.518 | 63.1 |
| pool-burst-16 | 1 | refill2048 | 67.518 [64.534, 69.534] | 19.383 | 62.4 |
| pool-burst-16 | 1 | refill256 | 84.357 [74.545, 98.498] | 19.490 | 62.3 |
| pool-burst-16 | 1 | refill512 | 78.971 [74.956, 84.459] | 19.456 | 62.9 |
| pool-burst-16 | 8 | refill2048 | 64.718 [63.947, 67.447] | 18.408 | 62.7 |
| pool-burst-16 | 8 | refill256 | 83.467 [76.269, 104.006] | 18.121 | 62.7 |
| pool-burst-16 | 8 | refill512 | 86.220 [81.163, 97.746] | 22.664 | 62.9 |
| pool-burst-256 | 1 | refill2048 | 63.708 [62.114, 65.906] | 16.567 | 62.3 |
| pool-burst-256 | 1 | refill256 | 77.261 [76.461, 77.505] | 18.458 | 61.9 |
| pool-burst-256 | 1 | refill512 | 82.906 [79.632, 95.065] | 20.390 | 62.7 |
| pool-burst-256 | 8 | refill2048 | 73.938 [63.678, 73.983] | 17.504 | 62.3 |
| pool-burst-256 | 8 | refill256 | 79.487 [77.743, 83.791] | 19.395 | 62.3 |
| pool-burst-256 | 8 | refill512 | 84.499 [77.590, 88.064] | 21.674 | 63.5 |

Burst checkpoints: pool reservation is distinct from live objects and process peak RSS.
Idle yields without allocating intentionally or forcing a collection.

| Case | Workers | Variant | Stage | Pool MiB median [min, max] | Managed live MiB median | Collection count median |
|---|---:|---|---|---:|---:|---:|
| pool-burst-1 | 1 | refill2048 | after churn | 18.25 [18.25, 18.25] | 0.02 | 15 |
| pool-burst-1 | 1 | refill2048 | burst | 18.25 [18.25, 18.25] | 15.55 | 1 |
| pool-burst-1 | 1 | refill2048 | idle | 18.25 [18.25, 18.25] | 0.02 | 15 |
| pool-burst-1 | 1 | refill256 | after churn | 16.50 [16.50, 16.50] | 0.02 | 15 |
| pool-burst-1 | 1 | refill256 | burst | 18.25 [18.25, 18.25] | 15.55 | 1 |
| pool-burst-1 | 1 | refill256 | idle | 16.50 [16.50, 16.50] | 0.02 | 15 |
| pool-burst-1 | 1 | refill512 | after churn | 16.75 [16.75, 16.75] | 0.02 | 15 |
| pool-burst-1 | 1 | refill512 | burst | 18.25 [18.25, 18.25] | 15.55 | 1 |
| pool-burst-1 | 1 | refill512 | idle | 16.75 [16.75, 16.75] | 0.02 | 15 |
| pool-burst-1 | 8 | refill2048 | after churn | 18.25 [18.25, 18.25] | 0.02 | 15 |
| pool-burst-1 | 8 | refill2048 | burst | 18.25 [18.25, 18.25] | 15.55 | 1 |
| pool-burst-1 | 8 | refill2048 | idle | 18.25 [18.25, 18.25] | 0.02 | 15 |
| pool-burst-1 | 8 | refill256 | after churn | 16.50 [16.50, 16.50] | 0.02 | 15 |
| pool-burst-1 | 8 | refill256 | burst | 18.25 [18.25, 18.25] | 15.55 | 1 |
| pool-burst-1 | 8 | refill256 | idle | 16.50 [16.50, 16.50] | 0.02 | 15 |
| pool-burst-1 | 8 | refill512 | after churn | 16.75 [16.75, 16.75] | 0.02 | 15 |
| pool-burst-1 | 8 | refill512 | burst | 18.25 [18.25, 18.25] | 15.55 | 1 |
| pool-burst-1 | 8 | refill512 | idle | 16.75 [16.75, 16.75] | 0.02 | 15 |
| pool-burst-16 | 1 | refill2048 | after churn | 34.25 [34.25, 34.25] | 0.02 | 15 |
| pool-burst-16 | 1 | refill2048 | burst | 18.25 [18.25, 18.25] | 15.55 | 1 |
| pool-burst-16 | 1 | refill2048 | idle | 34.25 [34.25, 34.25] | 0.02 | 15 |
| pool-burst-16 | 1 | refill256 | after churn | 20.25 [20.25, 20.25] | 0.02 | 15 |
| pool-burst-16 | 1 | refill256 | burst | 18.25 [18.25, 18.25] | 15.55 | 1 |
| pool-burst-16 | 1 | refill256 | idle | 20.25 [20.25, 20.25] | 0.02 | 15 |
| pool-burst-16 | 1 | refill512 | after churn | 24.25 [24.25, 24.25] | 0.02 | 15 |
| pool-burst-16 | 1 | refill512 | burst | 18.25 [18.25, 18.25] | 15.55 | 1 |
| pool-burst-16 | 1 | refill512 | idle | 24.25 [24.25, 24.25] | 0.02 | 15 |
| pool-burst-16 | 8 | refill2048 | after churn | 34.25 [34.25, 34.25] | 0.02 | 15 |
| pool-burst-16 | 8 | refill2048 | burst | 18.25 [18.25, 18.25] | 15.55 | 1 |
| pool-burst-16 | 8 | refill2048 | idle | 34.25 [34.25, 34.25] | 0.02 | 15 |
| pool-burst-16 | 8 | refill256 | after churn | 20.25 [20.25, 20.25] | 0.02 | 15 |
| pool-burst-16 | 8 | refill256 | burst | 18.25 [18.25, 18.25] | 15.55 | 1 |
| pool-burst-16 | 8 | refill256 | idle | 20.25 [20.25, 20.25] | 0.02 | 15 |
| pool-burst-16 | 8 | refill512 | after churn | 24.25 [24.25, 24.25] | 0.02 | 15 |
| pool-burst-16 | 8 | refill512 | burst | 18.25 [18.25, 18.25] | 15.55 | 1 |
| pool-burst-16 | 8 | refill512 | idle | 24.25 [24.25, 24.25] | 0.02 | 15 |
| pool-burst-256 | 1 | refill2048 | after churn | 34.25 [34.25, 34.25] | 0.09 | 15 |
| pool-burst-256 | 1 | refill2048 | burst | 18.25 [18.25, 18.25] | 15.55 | 1 |
| pool-burst-256 | 1 | refill2048 | idle | 34.25 [34.25, 34.25] | 0.09 | 15 |
| pool-burst-256 | 1 | refill256 | after churn | 34.25 [34.25, 34.25] | 0.09 | 15 |
| pool-burst-256 | 1 | refill256 | burst | 18.25 [18.25, 18.25] | 15.55 | 1 |
| pool-burst-256 | 1 | refill256 | idle | 34.25 [34.25, 34.25] | 0.09 | 15 |
| pool-burst-256 | 1 | refill512 | after churn | 34.25 [34.25, 34.25] | 0.09 | 15 |
| pool-burst-256 | 1 | refill512 | burst | 18.25 [18.25, 18.25] | 15.55 | 1 |
| pool-burst-256 | 1 | refill512 | idle | 34.25 [34.25, 34.25] | 0.09 | 15 |
| pool-burst-256 | 8 | refill2048 | after churn | 34.25 [34.25, 34.25] | 0.09 | 15 |
| pool-burst-256 | 8 | refill2048 | burst | 18.25 [18.25, 18.25] | 15.55 | 1 |
| pool-burst-256 | 8 | refill2048 | idle | 34.25 [34.25, 34.25] | 0.09 | 15 |
| pool-burst-256 | 8 | refill256 | after churn | 34.25 [34.25, 34.25] | 0.09 | 15 |
| pool-burst-256 | 8 | refill256 | burst | 18.25 [18.25, 18.25] | 15.55 | 1 |
| pool-burst-256 | 8 | refill256 | idle | 34.25 [34.25, 34.25] | 0.09 | 15 |
| pool-burst-256 | 8 | refill512 | after churn | 34.25 [34.25, 34.25] | 0.09 | 15 |
| pool-burst-256 | 8 | refill512 | burst | 18.25 [18.25, 18.25] | 15.55 | 1 |
| pool-burst-256 | 8 | refill512 | idle | 34.25 [34.25, 34.25] | 0.09 | 15 |

Diagnostic GC phases summed across measured repetitions (wall time):

- ('parallel-maps', 1, 'refill2048'): sweep_finalize 154.714 ms, prepare 3.584 ms, weak_pages 2.411 ms, trace 1.847 ms, trim 0.664 ms, roots 0.588 ms, allocation_census 0.438 ms, finish 0.127 ms, accounting 0.083 ms, resume_world 0.058 ms, verify 0.020 ms, stop_world 0.017 ms
- ('parallel-maps', 1, 'refill256'): sweep_finalize 149.240 ms, trim 4.873 ms, prepare 3.686 ms, weak_pages 2.191 ms, trace 1.845 ms, roots 0.654 ms, allocation_census 0.502 ms, finish 0.120 ms, accounting 0.081 ms, resume_world 0.056 ms, verify 0.019 ms, stop_world 0.016 ms
- ('parallel-maps', 1, 'refill512'): sweep_finalize 155.517 ms, prepare 3.875 ms, trim 3.760 ms, weak_pages 2.270 ms, trace 1.948 ms, roots 0.672 ms, allocation_census 0.385 ms, finish 0.128 ms, accounting 0.085 ms, resume_world 0.056 ms, verify 0.021 ms, stop_world 0.018 ms
- ('parallel-maps', 8, 'refill2048'): sweep_finalize 53.915 ms, trim 7.547 ms, trace 3.929 ms, prepare 3.147 ms, weak_pages 1.402 ms, roots 1.144 ms, stop_world 1.027 ms, allocation_census 0.434 ms, finish 0.142 ms, accounting 0.102 ms, resume_world 0.029 ms, verify 0.027 ms
- ('parallel-maps', 8, 'refill256'): sweep_finalize 53.047 ms, trim 7.483 ms, trace 3.839 ms, prepare 2.813 ms, weak_pages 1.337 ms, roots 0.983 ms, stop_world 0.849 ms, allocation_census 0.444 ms, finish 0.145 ms, accounting 0.086 ms, resume_world 0.029 ms, verify 0.017 ms
- ('parallel-maps', 8, 'refill512'): sweep_finalize 48.750 ms, trim 8.061 ms, trace 3.929 ms, prepare 2.850 ms, weak_pages 1.507 ms, roots 0.913 ms, stop_world 0.778 ms, allocation_census 0.391 ms, finish 0.122 ms, accounting 0.090 ms, resume_world 0.029 ms, verify 0.019 ms
- ('parallel-snapshots', 1, 'refill2048'): sweep_finalize 181.618 ms, trace 67.791 ms, prepare 11.336 ms, weak_pages 9.044 ms, trim 6.774 ms, roots 0.601 ms, allocation_census 0.436 ms, finish 0.093 ms, accounting 0.067 ms, resume_world 0.047 ms, stop_world 0.025 ms, verify 0.019 ms
- ('parallel-snapshots', 1, 'refill256'): sweep_finalize 199.153 ms, trace 72.105 ms, trim 36.628 ms, prepare 12.007 ms, weak_pages 10.395 ms, roots 0.651 ms, allocation_census 0.464 ms, finish 0.102 ms, accounting 0.071 ms, resume_world 0.056 ms, verify 0.020 ms, stop_world 0.019 ms
- ('parallel-snapshots', 1, 'refill512'): sweep_finalize 187.605 ms, trace 74.755 ms, trim 30.656 ms, prepare 12.469 ms, weak_pages 10.171 ms, roots 0.620 ms, allocation_census 0.443 ms, finish 0.096 ms, accounting 0.070 ms, resume_world 0.048 ms, verify 0.020 ms, stop_world 0.019 ms
- ('parallel-snapshots', 8, 'refill2048'): trace 185.904 ms, trim 71.855 ms, sweep_finalize 70.075 ms, prepare 17.331 ms, weak_pages 15.839 ms, stop_world 1.108 ms, roots 0.708 ms, allocation_census 0.537 ms, finish 0.095 ms, accounting 0.075 ms, resume_world 0.040 ms, verify 0.019 ms
- ('parallel-snapshots', 8, 'refill256'): trace 199.265 ms, trim 70.921 ms, sweep_finalize 67.396 ms, prepare 17.788 ms, weak_pages 16.309 ms, roots 0.669 ms, stop_world 0.599 ms, allocation_census 0.563 ms, finish 0.112 ms, accounting 0.079 ms, resume_world 0.043 ms, verify 0.019 ms
- ('parallel-snapshots', 8, 'refill512'): trace 202.795 ms, sweep_finalize 68.470 ms, trim 68.362 ms, prepare 17.298 ms, weak_pages 16.309 ms, stop_world 3.151 ms, roots 0.727 ms, allocation_census 0.575 ms, finish 0.103 ms, accounting 0.080 ms, resume_world 0.053 ms, verify 0.019 ms
- ('pool-burst-1', 1, 'refill2048'): sweep_finalize 38.611 ms, trim 7.696 ms, trace 2.734 ms, prepare 2.422 ms, weak_pages 2.169 ms, roots 0.123 ms, allocation_census 0.088 ms, finish 0.039 ms, resume_world 0.027 ms, accounting 0.024 ms, stop_world 0.009 ms, verify 0.006 ms
- ('pool-burst-1', 1, 'refill256'): sweep_finalize 40.852 ms, trim 6.521 ms, trace 3.202 ms, prepare 2.703 ms, weak_pages 2.103 ms, roots 0.116 ms, allocation_census 0.082 ms, finish 0.038 ms, resume_world 0.030 ms, accounting 0.025 ms, stop_world 0.007 ms, verify 0.005 ms
- ('pool-burst-1', 1, 'refill512'): sweep_finalize 44.998 ms, trim 8.343 ms, trace 3.197 ms, prepare 2.741 ms, weak_pages 2.040 ms, roots 0.131 ms, allocation_census 0.087 ms, finish 0.039 ms, resume_world 0.029 ms, accounting 0.028 ms, stop_world 0.007 ms, verify 0.006 ms
- ('pool-burst-1', 8, 'refill2048'): sweep_finalize 40.055 ms, trim 12.886 ms, trace 2.982 ms, prepare 2.578 ms, weak_pages 2.329 ms, roots 0.128 ms, allocation_census 0.094 ms, finish 0.038 ms, stop_world 0.035 ms, accounting 0.024 ms, resume_world 0.018 ms, verify 0.006 ms
- ('pool-burst-1', 8, 'refill256'): sweep_finalize 51.541 ms, trim 16.331 ms, trace 3.134 ms, prepare 2.850 ms, weak_pages 2.217 ms, roots 0.163 ms, allocation_census 0.098 ms, finish 0.042 ms, stop_world 0.030 ms, accounting 0.029 ms, resume_world 0.023 ms, verify 0.006 ms
- ('pool-burst-1', 8, 'refill512'): sweep_finalize 44.194 ms, trim 7.084 ms, trace 3.083 ms, prepare 2.532 ms, weak_pages 1.819 ms, roots 0.128 ms, allocation_census 0.089 ms, finish 0.037 ms, stop_world 0.030 ms, accounting 0.026 ms, resume_world 0.019 ms, verify 0.006 ms
- ('pool-burst-16', 1, 'refill2048'): sweep_finalize 40.471 ms, trim 7.193 ms, trace 2.688 ms, prepare 2.503 ms, weak_pages 2.194 ms, roots 0.121 ms, allocation_census 0.091 ms, finish 0.038 ms, resume_world 0.026 ms, accounting 0.024 ms, stop_world 0.010 ms, verify 0.006 ms
- ('pool-burst-16', 1, 'refill256'): sweep_finalize 44.320 ms, trim 7.226 ms, trace 3.078 ms, prepare 2.784 ms, weak_pages 2.384 ms, roots 0.139 ms, allocation_census 0.088 ms, finish 0.038 ms, resume_world 0.030 ms, accounting 0.026 ms, stop_world 0.006 ms, verify 0.006 ms
- ('pool-burst-16', 1, 'refill512'): sweep_finalize 43.264 ms, trim 7.708 ms, trace 3.263 ms, prepare 2.539 ms, weak_pages 1.802 ms, roots 0.129 ms, allocation_census 0.084 ms, finish 0.039 ms, resume_world 0.030 ms, accounting 0.027 ms, stop_world 0.007 ms, verify 0.006 ms
- ('pool-burst-16', 8, 'refill2048'): sweep_finalize 38.495 ms, trim 8.114 ms, trace 2.743 ms, prepare 2.353 ms, weak_pages 2.101 ms, roots 0.117 ms, allocation_census 0.083 ms, finish 0.037 ms, stop_world 0.032 ms, accounting 0.025 ms, resume_world 0.016 ms, verify 0.006 ms
- ('pool-burst-16', 8, 'refill256'): sweep_finalize 41.143 ms, trim 8.658 ms, trace 3.113 ms, prepare 2.696 ms, weak_pages 2.020 ms, roots 0.118 ms, allocation_census 0.083 ms, finish 0.035 ms, stop_world 0.027 ms, accounting 0.026 ms, resume_world 0.018 ms, verify 0.006 ms
- ('pool-burst-16', 8, 'refill512'): sweep_finalize 51.280 ms, trim 13.659 ms, trace 3.530 ms, prepare 2.962 ms, weak_pages 2.391 ms, roots 0.142 ms, allocation_census 0.104 ms, finish 0.045 ms, accounting 0.031 ms, stop_world 0.029 ms, resume_world 0.022 ms, verify 0.007 ms
- ('pool-burst-256', 1, 'refill2048'): sweep_finalize 35.126 ms, trim 8.014 ms, trace 2.772 ms, prepare 2.485 ms, weak_pages 2.138 ms, roots 0.121 ms, allocation_census 0.088 ms, finish 0.039 ms, resume_world 0.025 ms, accounting 0.023 ms, stop_world 0.009 ms, verify 0.006 ms
- ('pool-burst-256', 1, 'refill256'): sweep_finalize 41.565 ms, trim 6.115 ms, trace 3.086 ms, prepare 2.565 ms, weak_pages 1.749 ms, roots 0.121 ms, allocation_census 0.094 ms, finish 0.036 ms, resume_world 0.028 ms, accounting 0.023 ms, stop_world 0.006 ms, verify 0.005 ms
- ('pool-burst-256', 1, 'refill512'): sweep_finalize 44.523 ms, trim 9.403 ms, trace 3.413 ms, prepare 2.447 ms, weak_pages 2.015 ms, roots 0.131 ms, allocation_census 0.097 ms, finish 0.039 ms, resume_world 0.036 ms, accounting 0.026 ms, stop_world 0.006 ms, verify 0.006 ms
- ('pool-burst-256', 8, 'refill2048'): sweep_finalize 38.539 ms, trim 9.516 ms, trace 2.717 ms, prepare 2.496 ms, weak_pages 2.263 ms, roots 0.124 ms, allocation_census 0.090 ms, finish 0.038 ms, stop_world 0.033 ms, accounting 0.023 ms, resume_world 0.017 ms, verify 0.006 ms
- ('pool-burst-256', 8, 'refill256'): sweep_finalize 41.960 ms, trim 7.007 ms, trace 3.428 ms, prepare 2.698 ms, weak_pages 1.895 ms, roots 0.130 ms, allocation_census 0.087 ms, finish 0.039 ms, stop_world 0.029 ms, accounting 0.024 ms, resume_world 0.018 ms, verify 0.005 ms
- ('pool-burst-256', 8, 'refill512'): sweep_finalize 42.901 ms, trim 10.196 ms, trace 3.061 ms, prepare 2.564 ms, weak_pages 2.036 ms, roots 0.134 ms, allocation_census 0.083 ms, finish 0.040 ms, stop_world 0.029 ms, accounting 0.025 ms, resume_world 0.018 ms, verify 0.006 ms

Sweep work distribution (workload collections only):

Small helper jobs contain fewer than 4,096 objects; native thread creation is counted separately.
these counts alone do not measure thread-startup cost.

| Case | Workers | Variant | Collections / parallel | Helper jobs | New native threads | Small helper jobs | Median helper objects | Median largest-list share in parallel collections |
|---|---:|---|---:|---:|---:|---:|---:|---:|
| parallel-maps | 1 | refill2048 | 189 / 0 | 0 | 0 | 0 | n/a | n/a |
| parallel-maps | 1 | refill256 | 189 / 0 | 0 | 0 | 0 | n/a | n/a |
| parallel-maps | 1 | refill512 | 189 / 0 | 0 | 0 | 0 | n/a | n/a |
| parallel-maps | 8 | refill2048 | 186 / 186 | 1218 | 1218 | 29 | 7188 | 15.7% |
| parallel-maps | 8 | refill256 | 186 / 184 | 1215 | 1215 | 30 | 7133 | 15.8% |
| parallel-maps | 8 | refill512 | 186 / 186 | 1223 | 1223 | 25 | 7111 | 15.1% |
| parallel-snapshots | 1 | refill2048 | 114 / 0 | 0 | 0 | 0 | n/a | n/a |
| parallel-snapshots | 1 | refill256 | 114 / 0 | 0 | 0 | 0 | n/a | n/a |
| parallel-snapshots | 1 | refill512 | 114 / 0 | 0 | 0 | 0 | n/a | n/a |
| parallel-snapshots | 8 | refill2048 | 102 / 102 | 671 | 671 | 13 | 10366 | 16.3% |
| parallel-snapshots | 8 | refill256 | 100 / 100 | 689 | 689 | 2 | 10177 | 15.5% |
| parallel-snapshots | 8 | refill512 | 101 / 101 | 687 | 687 | 3 | 10206 | 15.7% |
| pool-burst-1 | 1 | refill2048 | 45 / 0 | 0 | 0 | 0 | n/a | n/a |
| pool-burst-1 | 1 | refill256 | 45 / 0 | 0 | 0 | 0 | n/a | n/a |
| pool-burst-1 | 1 | refill512 | 45 / 0 | 0 | 0 | 0 | n/a | n/a |
| pool-burst-1 | 8 | refill2048 | 45 / 0 | 0 | 0 | 0 | n/a | n/a |
| pool-burst-1 | 8 | refill256 | 45 / 0 | 0 | 0 | 0 | n/a | n/a |
| pool-burst-1 | 8 | refill512 | 45 / 0 | 0 | 0 | 0 | n/a | n/a |
| pool-burst-16 | 1 | refill2048 | 45 / 0 | 0 | 0 | 0 | n/a | n/a |
| pool-burst-16 | 1 | refill256 | 45 / 0 | 0 | 0 | 0 | n/a | n/a |
| pool-burst-16 | 1 | refill512 | 45 / 0 | 0 | 0 | 0 | n/a | n/a |
| pool-burst-16 | 8 | refill2048 | 45 / 0 | 0 | 0 | 0 | n/a | n/a |
| pool-burst-16 | 8 | refill256 | 45 / 0 | 0 | 0 | 0 | n/a | n/a |
| pool-burst-16 | 8 | refill512 | 45 / 0 | 0 | 0 | 0 | n/a | n/a |
| pool-burst-256 | 1 | refill2048 | 45 / 0 | 0 | 0 | 0 | n/a | n/a |
| pool-burst-256 | 1 | refill256 | 45 / 0 | 0 | 0 | 0 | n/a | n/a |
| pool-burst-256 | 1 | refill512 | 45 / 0 | 0 | 0 | 0 | n/a | n/a |
| pool-burst-256 | 8 | refill2048 | 45 / 0 | 0 | 0 | 0 | n/a | n/a |
| pool-burst-256 | 8 | refill256 | 45 / 0 | 0 | 0 | 0 | n/a | n/a |
| pool-burst-256 | 8 | refill512 | 45 / 0 | 0 | 0 | 0 | n/a | n/a |
