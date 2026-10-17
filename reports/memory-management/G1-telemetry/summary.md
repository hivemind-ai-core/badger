| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 106.801 [105.184, 109.850] | 17.408 | 47.9 |
| backpressure | 1 | diagnostic | 104.626 [103.480, 250.795] | 16.007 | 48.2 |
| backpressure | 1 | diagnostic-allocations | 135.205 [128.497, 135.465] | 24.567 | 51.5 |
| backpressure | 1 | diagnostic-off | 110.933 [106.948, 112.335] | 18.017 | 47.9 |
| backpressure | 2 | baseline | 77.025 [73.913, 264.402] | 15.860 | 51.0 |
| backpressure | 2 | diagnostic | 79.679 [71.958, 90.931] | 16.956 | 50.8 |
| backpressure | 2 | diagnostic-allocations | 95.312 [93.342, 95.948] | 21.819 | 55.0 |
| backpressure | 2 | diagnostic-off | 80.880 [75.733, 92.064] | 16.901 | 50.7 |
| backpressure | 4 | baseline | 50.370 [47.532, 60.571] | 13.262 | 60.9 |
| backpressure | 4 | diagnostic | 48.742 [47.173, 48.777] | 12.067 | 56.1 |
| backpressure | 4 | diagnostic-allocations | 70.979 [64.485, 418.210] | 24.632 | 65.1 |
| backpressure | 4 | diagnostic-off | 49.831 [48.413, 59.786] | 12.848 | 58.2 |
| backpressure | 8 | baseline | 38.524 [34.214, 39.879] | 10.121 | 71.8 |
| backpressure | 8 | diagnostic | 39.915 [39.254, 43.133] | 12.134 | 66.5 |
| backpressure | 8 | diagnostic-allocations | 59.383 [47.265, 65.804] | 22.462 | 74.3 |
| backpressure | 8 | diagnostic-off | 36.879 [34.084, 39.869] | 11.382 | 71.2 |
| balanced | 1 | baseline | 113.225 [107.757, 120.447] | 19.515 | 64.1 |
| balanced | 1 | diagnostic | 104.844 [102.669, 110.947] | 17.736 | 64.5 |
| balanced | 1 | diagnostic-allocations | 140.426 [129.334, 150.934] | 24.868 | 68.4 |
| balanced | 1 | diagnostic-off | 108.902 [102.989, 114.582] | 18.404 | 64.6 |
| balanced | 2 | baseline | 80.876 [75.229, 88.983] | 17.627 | 65.9 |
| balanced | 2 | diagnostic | 84.207 [82.385, 106.875] | 17.265 | 68.1 |
| balanced | 2 | diagnostic-allocations | 105.120 [95.268, 108.507] | 24.950 | 73.9 |
| balanced | 2 | diagnostic-off | 80.991 [77.478, 316.304] | 16.721 | 67.2 |
| balanced | 4 | baseline | 63.595 [47.745, 67.431] | 13.990 | 79.6 |
| balanced | 4 | diagnostic | 56.900 [51.050, 68.982] | 13.756 | 76.6 |
| balanced | 4 | diagnostic-allocations | 74.640 [67.210, 300.444] | 22.820 | 83.3 |
| balanced | 4 | diagnostic-off | 53.641 [49.477, 53.804] | 13.199 | 75.0 |
| balanced | 8 | baseline | 37.107 [33.798, 48.499] | 10.481 | 83.1 |
| balanced | 8 | diagnostic | 34.852 [34.637, 36.445] | 9.838 | 83.9 |
| balanced | 8 | diagnostic-allocations | 48.838 [48.124, 275.731] | 16.430 | 98.5 |
| balanced | 8 | diagnostic-off | 39.238 [34.478, 40.820] | 11.598 | 85.9 |
| cpu | 1 | baseline | 209.037 [207.552, 229.540] | 18.960 | 64.8 |
| cpu | 1 | diagnostic | 223.034 [213.494, 224.935] | 19.418 | 65.1 |
| cpu | 1 | diagnostic-allocations | 253.612 [243.082, 260.341] | 27.258 | 68.9 |
| cpu | 1 | diagnostic-off | 209.231 [208.787, 214.168] | 16.282 | 64.8 |
| cpu | 2 | baseline | 129.829 [126.751, 139.570] | 17.355 | 65.5 |
| cpu | 2 | diagnostic | 129.350 [124.649, 133.575] | 15.959 | 66.9 |
| cpu | 2 | diagnostic-allocations | 160.253 [145.081, 554.228] | 23.730 | 75.4 |
| cpu | 2 | diagnostic-off | 133.638 [126.768, 136.394] | 16.106 | 67.3 |
| cpu | 4 | baseline | 77.503 [72.442, 93.784] | 12.457 | 74.8 |
| cpu | 4 | diagnostic | 76.210 [72.847, 77.269] | 13.276 | 79.6 |
| cpu | 4 | diagnostic-allocations | 94.805 [87.515, 102.545] | 20.195 | 87.8 |
| cpu | 4 | diagnostic-off | 73.600 [72.954, 77.176] | 12.302 | 79.7 |
| cpu | 8 | baseline | 87.726 [82.796, 88.533] | 13.833 | 94.8 |
| cpu | 8 | diagnostic | 80.358 [79.484, 256.814] | 13.308 | 96.0 |
| cpu | 8 | diagnostic-allocations | 99.805 [96.965, 102.082] | 19.362 | 100.2 |
| cpu | 8 | diagnostic-off | 90.553 [82.479, 93.414] | 14.328 | 105.7 |
| fanout | 1 | baseline | 280.328 [273.613, 809.694] | 51.453 | 76.8 |
| fanout | 1 | diagnostic | 313.606 [287.202, 373.568] | 57.355 | 76.8 |
| fanout | 1 | diagnostic-allocations | 355.133 [331.248, 366.909] | 67.654 | 82.7 |
| fanout | 1 | diagnostic-off | 294.086 [277.925, 311.344] | 51.339 | 76.8 |
| fanout | 2 | baseline | 265.491 [223.353, 990.564] | 53.404 | 81.4 |
| fanout | 2 | diagnostic | 217.698 [217.647, 248.401] | 47.008 | 89.3 |
| fanout | 2 | diagnostic-allocations | 289.645 [273.744, 292.489] | 61.903 | 92.9 |
| fanout | 2 | diagnostic-off | 236.326 [215.511, 245.728] | 47.221 | 85.2 |
| fanout | 4 | baseline | 192.603 [186.291, 197.012] | 38.374 | 96.1 |
| fanout | 4 | diagnostic | 188.066 [180.010, 228.295] | 36.876 | 95.9 |
| fanout | 4 | diagnostic-allocations | 226.372 [217.759, 264.414] | 59.846 | 105.8 |
| fanout | 4 | diagnostic-off | 196.154 [168.444, 224.179] | 38.547 | 88.9 |
| fanout | 8 | baseline | 215.914 [197.802, 257.181] | 40.334 | 93.2 |
| fanout | 8 | diagnostic | 211.178 [207.581, 248.506] | 39.587 | 93.0 |
| fanout | 8 | diagnostic-allocations | 237.766 [228.948, 244.428] | 52.505 | 104.8 |
| fanout | 8 | diagnostic-off | 206.182 [199.110, 255.370] | 35.201 | 97.1 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 12288 | 0.928 | 4.864 |
| backpressure | 1 | diagnostic | 12288 | 1.024 | 8.704 |
| backpressure | 1 | diagnostic-allocations | 12288 | 1.152 | 6.400 |
| backpressure | 1 | diagnostic-off | 12288 | 0.928 | 5.120 |
| backpressure | 2 | baseline | 12288 | 0.608 | 9.728 |
| backpressure | 2 | diagnostic | 12288 | 0.512 | 4.352 |
| backpressure | 2 | diagnostic-allocations | 12288 | 0.608 | 5.120 |
| backpressure | 2 | diagnostic-off | 12288 | 0.512 | 4.352 |
| backpressure | 4 | baseline | 12288 | 0.304 | 3.456 |
| backpressure | 4 | diagnostic | 12288 | 0.272 | 2.560 |
| backpressure | 4 | diagnostic-allocations | 12288 | 0.448 | 21.504 |
| backpressure | 4 | diagnostic-off | 12288 | 0.304 | 3.072 |
| backpressure | 8 | baseline | 12288 | 0.192 | 2.048 |
| backpressure | 8 | diagnostic | 12288 | 0.208 | 2.304 |
| backpressure | 8 | diagnostic-allocations | 12288 | 0.272 | 4.864 |
| backpressure | 8 | diagnostic-off | 12288 | 0.192 | 2.304 |
| balanced | 1 | baseline | 12288 | 14.848 | 19.456 |
| balanced | 1 | diagnostic | 12288 | 14.336 | 18.432 |
| balanced | 1 | diagnostic-allocations | 12288 | 18.432 | 24.576 |
| balanced | 1 | diagnostic-off | 12288 | 13.824 | 18.432 |
| balanced | 2 | baseline | 12288 | 10.240 | 19.456 |
| balanced | 2 | diagnostic | 12288 | 11.776 | 21.504 |
| balanced | 2 | diagnostic-allocations | 12288 | 13.312 | 17.408 |
| balanced | 2 | diagnostic-off | 12288 | 11.776 | 59.392 |
| balanced | 4 | baseline | 12288 | 6.912 | 20.480 |
| balanced | 4 | diagnostic | 12288 | 6.912 | 16.384 |
| balanced | 4 | diagnostic-allocations | 12288 | 10.752 | 47.104 |
| balanced | 4 | diagnostic-off | 12288 | 6.656 | 9.728 |
| balanced | 8 | baseline | 12288 | 4.608 | 8.704 |
| balanced | 8 | diagnostic | 12288 | 4.608 | 5.376 |
| balanced | 8 | diagnostic-allocations | 12288 | 6.656 | 49.152 |
| balanced | 8 | diagnostic-off | 12288 | 4.864 | 7.424 |
| cpu | 1 | baseline | 12288 | 29.696 | 34.816 |
| cpu | 1 | diagnostic | 12288 | 30.720 | 34.816 |
| cpu | 1 | diagnostic-allocations | 12288 | 34.816 | 40.960 |
| cpu | 1 | diagnostic-off | 12288 | 28.672 | 34.816 |
| cpu | 2 | baseline | 12288 | 18.432 | 22.528 |
| cpu | 2 | diagnostic | 12288 | 17.408 | 21.504 |
| cpu | 2 | diagnostic-allocations | 12288 | 21.504 | 86.016 |
| cpu | 2 | diagnostic-off | 12288 | 17.408 | 27.648 |
| cpu | 4 | baseline | 12288 | 10.240 | 14.336 |
| cpu | 4 | diagnostic | 12288 | 9.728 | 12.800 |
| cpu | 4 | diagnostic-allocations | 12288 | 12.288 | 17.408 |
| cpu | 4 | diagnostic-off | 12288 | 10.240 | 12.288 |
| cpu | 8 | baseline | 12288 | 11.776 | 14.336 |
| cpu | 8 | diagnostic | 12288 | 11.776 | 40.960 |
| cpu | 8 | diagnostic-allocations | 12288 | 13.312 | 17.408 |
| cpu | 8 | diagnostic-off | 12288 | 11.776 | 14.848 |
| fanout | 1 | baseline | 12288 | 40.960 | 163.840 |
| fanout | 1 | diagnostic | 12288 | 40.960 | 69.632 |
| fanout | 1 | diagnostic-allocations | 12288 | 45.056 | 77.824 |
| fanout | 1 | diagnostic-off | 12288 | 38.912 | 59.392 |
| fanout | 2 | baseline | 12288 | 36.864 | 188.416 |
| fanout | 2 | diagnostic | 12288 | 28.672 | 47.104 |
| fanout | 2 | diagnostic-allocations | 12288 | 36.864 | 57.344 |
| fanout | 2 | diagnostic-off | 12288 | 29.696 | 49.152 |
| fanout | 4 | baseline | 12288 | 23.552 | 36.864 |
| fanout | 4 | diagnostic | 12288 | 23.552 | 43.008 |
| fanout | 4 | diagnostic-allocations | 12288 | 30.720 | 55.296 |
| fanout | 4 | diagnostic-off | 12288 | 23.552 | 43.008 |
| fanout | 8 | baseline | 12288 | 28.672 | 51.200 |
| fanout | 8 | diagnostic | 12288 | 28.672 | 45.056 |
| fanout | 8 | diagnostic-allocations | 12288 | 30.720 | 55.296 |
| fanout | 8 | diagnostic-off | 12288 | 27.648 | 47.104 |

Diagnostic GC phases summed across measured repetitions (wall time):

- ('backpressure', 1, 'diagnostic'): sweep_finalize 63.673 ms, trace 3.709 ms, roots 0.284 ms, prepare 0.044 ms, resume_world 0.037 ms, finish 0.029 ms, accounting 0.023 ms, allocation_census 0.011 ms, weak_pages 0.004 ms, trim 0.004 ms, verify 0.003 ms, stop_world 0.003 ms
- ('backpressure', 1, 'diagnostic-allocations'): sweep_finalize 46.660 ms, allocation_census 21.694 ms, trace 2.669 ms, roots 0.400 ms, prepare 0.037 ms, resume_world 0.035 ms, finish 0.026 ms, accounting 0.022 ms, trim 0.004 ms, verify 0.003 ms, weak_pages 0.003 ms, stop_world 0.002 ms
- ('backpressure', 2, 'diagnostic'): sweep_finalize 50.444 ms, trace 2.609 ms, roots 0.220 ms, stop_world 0.109 ms, prepare 0.048 ms, finish 0.031 ms, accounting 0.023 ms, resume_world 0.009 ms, allocation_census 0.006 ms, trim 0.004 ms, verify 0.003 ms, weak_pages 0.003 ms
- ('backpressure', 2, 'diagnostic-allocations'): sweep_finalize 39.091 ms, allocation_census 21.470 ms, trace 2.374 ms, roots 0.186 ms, prepare 0.045 ms, finish 0.045 ms, stop_world 0.020 ms, accounting 0.019 ms, resume_world 0.007 ms, trim 0.004 ms, verify 0.003 ms, weak_pages 0.003 ms
- ('backpressure', 4, 'diagnostic'): sweep_finalize 34.166 ms, trace 2.494 ms, roots 0.198 ms, stop_world 0.139 ms, prepare 0.051 ms, finish 0.043 ms, accounting 0.017 ms, allocation_census 0.007 ms, resume_world 0.006 ms, trim 0.004 ms, verify 0.003 ms, weak_pages 0.003 ms
- ('backpressure', 4, 'diagnostic-allocations'): sweep_finalize 79.566 ms, allocation_census 56.710 ms, stop_world 7.524 ms, trace 4.866 ms, roots 0.420 ms, prepare 0.082 ms, finish 0.066 ms, accounting 0.027 ms, resume_world 0.013 ms, weak_pages 0.005 ms, trim 0.005 ms, verify 0.004 ms
- ('backpressure', 8, 'diagnostic'): sweep_finalize 32.612 ms, trace 2.381 ms, roots 0.219 ms, prepare 0.068 ms, finish 0.048 ms, stop_world 0.030 ms, accounting 0.018 ms, allocation_census 0.010 ms, resume_world 0.006 ms, trim 0.004 ms, weak_pages 0.003 ms, verify 0.003 ms
- ('backpressure', 8, 'diagnostic-allocations'): sweep_finalize 36.450 ms, allocation_census 25.928 ms, trace 2.703 ms, roots 0.216 ms, prepare 0.076 ms, finish 0.074 ms, stop_world 0.051 ms, accounting 0.027 ms, resume_world 0.010 ms, weak_pages 0.004 ms, trim 0.003 ms, verify 0.003 ms
- ('balanced', 1, 'diagnostic'): sweep_finalize 39.619 ms, trace 13.131 ms, roots 0.174 ms, prepare 0.032 ms, resume_world 0.027 ms, finish 0.021 ms, accounting 0.016 ms, allocation_census 0.009 ms, weak_pages 0.004 ms, trim 0.004 ms, verify 0.003 ms, stop_world 0.002 ms
- ('balanced', 1, 'diagnostic-allocations'): sweep_finalize 40.727 ms, allocation_census 20.577 ms, trace 12.800 ms, roots 0.175 ms, prepare 0.040 ms, resume_world 0.038 ms, finish 0.026 ms, accounting 0.017 ms, weak_pages 0.004 ms, trim 0.003 ms, verify 0.003 ms, stop_world 0.002 ms
- ('balanced', 2, 'diagnostic'): sweep_finalize 38.921 ms, trace 13.160 ms, roots 0.225 ms, prepare 0.048 ms, finish 0.040 ms, accounting 0.020 ms, stop_world 0.018 ms, allocation_census 0.010 ms, resume_world 0.007 ms, weak_pages 0.006 ms, verify 0.004 ms, trim 0.003 ms
- ('balanced', 2, 'diagnostic-allocations'): sweep_finalize 38.568 ms, allocation_census 23.820 ms, trace 13.366 ms, roots 0.245 ms, stop_world 0.079 ms, prepare 0.044 ms, finish 0.038 ms, accounting 0.023 ms, resume_world 0.009 ms, weak_pages 0.005 ms, trim 0.004 ms, verify 0.003 ms
- ('balanced', 4, 'diagnostic'): sweep_finalize 28.401 ms, trace 12.688 ms, roots 0.243 ms, stop_world 0.192 ms, prepare 0.064 ms, finish 0.047 ms, accounting 0.019 ms, allocation_census 0.010 ms, resume_world 0.008 ms, trim 0.006 ms, weak_pages 0.004 ms, verify 0.003 ms
- ('balanced', 4, 'diagnostic-allocations'): sweep_finalize 43.131 ms, allocation_census 37.087 ms, trace 19.235 ms, stop_world 0.882 ms, roots 0.392 ms, prepare 0.069 ms, finish 0.059 ms, accounting 0.031 ms, resume_world 0.011 ms, weak_pages 0.008 ms, trim 0.005 ms, verify 0.004 ms
- ('balanced', 8, 'diagnostic'): sweep_finalize 18.946 ms, trace 10.329 ms, roots 0.223 ms, stop_world 0.101 ms, prepare 0.049 ms, finish 0.034 ms, accounting 0.015 ms, allocation_census 0.011 ms, resume_world 0.005 ms, trim 0.004 ms, weak_pages 0.003 ms, verify 0.003 ms
- ('balanced', 8, 'diagnostic-allocations'): sweep_finalize 43.879 ms, allocation_census 37.726 ms, trace 19.061 ms, stop_world 4.014 ms, roots 0.355 ms, prepare 0.070 ms, finish 0.043 ms, accounting 0.021 ms, resume_world 0.010 ms, weak_pages 0.006 ms, trim 0.004 ms, verify 0.004 ms
- ('cpu', 1, 'diagnostic'): sweep_finalize 42.196 ms, trace 13.390 ms, roots 0.241 ms, resume_world 0.037 ms, prepare 0.034 ms, finish 0.027 ms, accounting 0.019 ms, allocation_census 0.013 ms, weak_pages 0.006 ms, trim 0.004 ms, verify 0.003 ms, stop_world 0.003 ms
- ('cpu', 1, 'diagnostic-allocations'): sweep_finalize 40.850 ms, allocation_census 22.638 ms, trace 13.486 ms, roots 0.278 ms, resume_world 0.049 ms, prepare 0.036 ms, finish 0.027 ms, accounting 0.019 ms, trim 0.004 ms, verify 0.004 ms, weak_pages 0.004 ms, stop_world 0.003 ms
- ('cpu', 2, 'diagnostic'): sweep_finalize 33.555 ms, trace 12.568 ms, roots 0.254 ms, prepare 0.042 ms, stop_world 0.030 ms, finish 0.024 ms, accounting 0.016 ms, allocation_census 0.009 ms, resume_world 0.005 ms, weak_pages 0.004 ms, trim 0.004 ms, verify 0.003 ms
- ('cpu', 2, 'diagnostic-allocations'): sweep_finalize 62.523 ms, allocation_census 44.976 ms, trace 16.162 ms, stop_world 8.848 ms, roots 0.488 ms, prepare 0.093 ms, finish 0.032 ms, accounting 0.022 ms, resume_world 0.009 ms, weak_pages 0.007 ms, trim 0.004 ms, verify 0.004 ms
- ('cpu', 4, 'diagnostic'): sweep_finalize 27.139 ms, trace 11.634 ms, roots 0.277 ms, prepare 0.048 ms, stop_world 0.037 ms, finish 0.033 ms, accounting 0.018 ms, allocation_census 0.010 ms, resume_world 0.006 ms, weak_pages 0.004 ms, trim 0.004 ms, verify 0.003 ms
- ('cpu', 4, 'diagnostic-allocations'): sweep_finalize 25.867 ms, allocation_census 23.780 ms, trace 12.263 ms, stop_world 0.630 ms, roots 0.243 ms, prepare 0.055 ms, finish 0.040 ms, accounting 0.019 ms, resume_world 0.008 ms, weak_pages 0.004 ms, trim 0.004 ms, verify 0.003 ms
- ('cpu', 8, 'diagnostic'): sweep_finalize 45.181 ms, trace 14.983 ms, stop_world 8.012 ms, roots 0.384 ms, prepare 0.113 ms, finish 0.058 ms, allocation_census 0.030 ms, accounting 0.020 ms, resume_world 0.010 ms, weak_pages 0.006 ms, trim 0.004 ms, verify 0.004 ms
- ('cpu', 8, 'diagnostic-allocations'): sweep_finalize 27.423 ms, allocation_census 20.658 ms, trace 10.576 ms, roots 0.216 ms, finish 0.055 ms, stop_world 0.043 ms, prepare 0.041 ms, accounting 0.019 ms, resume_world 0.011 ms, trim 0.004 ms, weak_pages 0.004 ms, verify 0.003 ms
- ('fanout', 1, 'diagnostic'): sweep_finalize 124.590 ms, trace 28.936 ms, prepare 3.927 ms, weak_pages 3.782 ms, trim 2.401 ms, roots 2.226 ms, finish 0.050 ms, resume_world 0.048 ms, accounting 0.036 ms, allocation_census 0.027 ms, verify 0.008 ms, stop_world 0.004 ms
- ('fanout', 1, 'diagnostic-allocations'): sweep_finalize 112.434 ms, allocation_census 49.356 ms, trace 26.331 ms, prepare 3.816 ms, weak_pages 3.094 ms, trim 2.252 ms, roots 1.842 ms, finish 0.050 ms, resume_world 0.044 ms, accounting 0.034 ms, verify 0.006 ms, stop_world 0.003 ms
- ('fanout', 2, 'diagnostic'): sweep_finalize 105.391 ms, trace 26.334 ms, prepare 3.823 ms, weak_pages 3.152 ms, roots 2.045 ms, trim 1.193 ms, stop_world 0.075 ms, finish 0.045 ms, accounting 0.038 ms, allocation_census 0.024 ms, resume_world 0.015 ms, verify 0.008 ms
- ('fanout', 2, 'diagnostic-allocations'): sweep_finalize 97.128 ms, allocation_census 51.930 ms, trace 27.561 ms, prepare 4.169 ms, weak_pages 3.360 ms, roots 1.949 ms, trim 1.237 ms, stop_world 0.044 ms, finish 0.043 ms, accounting 0.038 ms, resume_world 0.017 ms, verify 0.006 ms
- ('fanout', 4, 'diagnostic'): sweep_finalize 81.250 ms, trace 25.264 ms, prepare 3.896 ms, weak_pages 3.408 ms, roots 1.740 ms, trim 1.120 ms, finish 0.046 ms, stop_world 0.037 ms, accounting 0.034 ms, allocation_census 0.020 ms, resume_world 0.016 ms, verify 0.006 ms
- ('fanout', 4, 'diagnostic-allocations'): sweep_finalize 80.211 ms, allocation_census 58.425 ms, trace 27.006 ms, weak_pages 3.937 ms, prepare 3.927 ms, roots 1.958 ms, trim 1.316 ms, stop_world 0.418 ms, finish 0.045 ms, accounting 0.040 ms, resume_world 0.017 ms, verify 0.007 ms
- ('fanout', 8, 'diagnostic'): sweep_finalize 77.589 ms, trace 31.037 ms, weak_pages 3.945 ms, prepare 3.899 ms, roots 2.293 ms, trim 1.452 ms, finish 0.063 ms, stop_world 0.061 ms, allocation_census 0.040 ms, accounting 0.037 ms, resume_world 0.018 ms, verify 0.007 ms
- ('fanout', 8, 'diagnostic-allocations'): sweep_finalize 69.974 ms, allocation_census 56.150 ms, trace 25.868 ms, prepare 3.660 ms, weak_pages 3.572 ms, roots 1.767 ms, trim 1.306 ms, stop_world 0.243 ms, finish 0.059 ms, accounting 0.036 ms, resume_world 0.018 ms, verify 0.006 ms
