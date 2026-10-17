| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | pool1024 | 122.113 [112.909, 131.153] | 18.860 | 53.1 |
| backpressure | 1 | pool256 | 123.730 [111.536, 131.358] | 19.845 | 48.7 |
| backpressure | 1 | pool512 | 117.407 [113.233, 185.690] | 18.344 | 51.1 |
| backpressure | 2 | pool1024 | 84.229 [76.347, 108.479] | 17.362 | 58.4 |
| backpressure | 2 | pool256 | 90.883 [77.925, 97.755] | 19.195 | 51.6 |
| backpressure | 2 | pool512 | 86.771 [82.574, 96.990] | 17.866 | 56.6 |
| backpressure | 4 | pool1024 | 57.908 [55.509, 288.590] | 14.953 | 66.2 |
| backpressure | 4 | pool256 | 63.415 [56.257, 175.973] | 16.960 | 61.1 |
| backpressure | 4 | pool512 | 72.734 [51.725, 226.355] | 18.348 | 66.4 |
| backpressure | 8 | pool1024 | 50.314 [45.043, 244.958] | 14.709 | 80.7 |
| backpressure | 8 | pool256 | 47.931 [42.672, 112.510] | 13.247 | 72.1 |
| backpressure | 8 | pool512 | 45.540 [43.688, 62.493] | 13.869 | 79.5 |
| balanced | 1 | pool1024 | 117.570 [111.495, 169.577] | 18.753 | 66.3 |
| balanced | 1 | pool256 | 126.384 [119.097, 131.068] | 21.259 | 64.5 |
| balanced | 1 | pool512 | 123.385 [104.793, 329.124] | 17.786 | 66.8 |
| balanced | 2 | pool1024 | 95.133 [83.388, 131.590] | 18.057 | 71.0 |
| balanced | 2 | pool256 | 92.654 [83.463, 349.054] | 19.542 | 67.2 |
| balanced | 2 | pool512 | 175.782 [81.554, 349.691] | 43.904 | 69.8 |
| balanced | 4 | pool1024 | 61.492 [52.035, 218.031] | 15.394 | 79.7 |
| balanced | 4 | pool256 | 57.166 [53.592, 289.750] | 15.140 | 79.9 |
| balanced | 4 | pool512 | 57.237 [47.211, 63.070] | 12.719 | 76.7 |
| balanced | 8 | pool1024 | 38.814 [38.374, 244.183] | 11.087 | 87.1 |
| balanced | 8 | pool256 | 82.272 [44.546, 276.493] | 16.777 | 81.9 |
| balanced | 8 | pool512 | 49.518 [34.883, 78.462] | 14.445 | 91.1 |
| cpu | 1 | pool1024 | 239.526 [225.375, 265.987] | 18.485 | 65.5 |
| cpu | 1 | pool256 | 279.052 [237.601, 516.265] | 23.947 | 65.3 |
| cpu | 1 | pool512 | 254.130 [223.811, 267.746] | 18.942 | 66.1 |
| cpu | 2 | pool1024 | 143.302 [131.357, 178.087] | 16.253 | 66.9 |
| cpu | 2 | pool256 | 148.049 [133.193, 202.963] | 17.645 | 64.6 |
| cpu | 2 | pool512 | 140.436 [133.948, 518.633] | 17.095 | 67.2 |
| cpu | 4 | pool1024 | 86.240 [73.990, 301.596] | 13.469 | 80.4 |
| cpu | 4 | pool256 | 92.472 [80.540, 140.741] | 15.331 | 76.7 |
| cpu | 4 | pool512 | 87.708 [77.123, 104.035] | 15.130 | 74.7 |
| cpu | 8 | pool1024 | 81.015 [77.496, 117.933] | 14.511 | 90.7 |
| cpu | 8 | pool256 | 99.399 [82.414, 185.670] | 15.691 | 96.0 |
| cpu | 8 | pool512 | 90.406 [60.503, 256.246] | 14.368 | 96.3 |
| fanout | 1 | pool1024 | 341.375 [313.775, 417.070] | 52.095 | 80.0 |
| fanout | 1 | pool256 | 346.884 [300.378, 894.818] | 56.218 | 77.1 |
| fanout | 1 | pool512 | 375.162 [351.657, 948.070] | 53.341 | 79.6 |
| fanout | 2 | pool1024 | 273.116 [258.825, 322.406] | 46.488 | 92.0 |
| fanout | 2 | pool256 | 293.702 [264.904, 433.249] | 58.127 | 80.7 |
| fanout | 2 | pool512 | 274.577 [266.294, 1071.290] | 49.979 | 89.9 |
| fanout | 4 | pool1024 | 219.226 [191.924, 439.419] | 42.093 | 99.7 |
| fanout | 4 | pool256 | 235.385 [201.207, 642.352] | 46.885 | 91.8 |
| fanout | 4 | pool512 | 220.946 [211.475, 289.983] | 44.240 | 101.3 |
| fanout | 8 | pool1024 | 252.307 [205.483, 919.087] | 45.566 | 98.8 |
| fanout | 8 | pool256 | 256.117 [220.833, 431.619] | 43.581 | 90.1 |
| fanout | 8 | pool512 | 235.118 [229.999, 266.760] | 38.789 | 100.5 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | pool1024 | 20480 | 1.088 | 4.864 |
| backpressure | 1 | pool256 | 20480 | 1.088 | 5.632 |
| backpressure | 1 | pool512 | 20480 | 1.088 | 5.888 |
| backpressure | 2 | pool1024 | 20480 | 0.608 | 4.864 |
| backpressure | 2 | pool256 | 20480 | 0.576 | 4.608 |
| backpressure | 2 | pool512 | 20480 | 0.576 | 4.352 |
| backpressure | 4 | pool1024 | 20480 | 0.416 | 12.288 |
| backpressure | 4 | pool256 | 20480 | 0.384 | 5.376 |
| backpressure | 4 | pool512 | 20480 | 0.480 | 9.728 |
| backpressure | 8 | pool1024 | 20480 | 0.272 | 9.728 |
| backpressure | 8 | pool256 | 20480 | 0.256 | 4.352 |
| backpressure | 8 | pool512 | 20480 | 0.248 | 3.712 |
| balanced | 1 | pool1024 | 20480 | 16.384 | 34.816 |
| balanced | 1 | pool256 | 20480 | 17.408 | 21.504 |
| balanced | 1 | pool512 | 20480 | 16.384 | 81.920 |
| balanced | 2 | pool1024 | 20480 | 12.288 | 25.600 |
| balanced | 2 | pool256 | 20480 | 12.288 | 65.536 |
| balanced | 2 | pool512 | 20480 | 20.480 | 61.440 |
| balanced | 4 | pool1024 | 20480 | 7.936 | 38.912 |
| balanced | 4 | pool256 | 20480 | 7.936 | 45.056 |
| balanced | 4 | pool512 | 20480 | 7.168 | 14.336 |
| balanced | 8 | pool1024 | 20480 | 5.376 | 43.008 |
| balanced | 8 | pool256 | 20480 | 8.704 | 61.440 |
| balanced | 8 | pool512 | 20480 | 6.400 | 14.848 |
| cpu | 1 | pool1024 | 20480 | 32.768 | 38.912 |
| cpu | 1 | pool256 | 20480 | 36.864 | 90.112 |
| cpu | 1 | pool512 | 20480 | 34.816 | 43.008 |
| cpu | 2 | pool1024 | 20480 | 19.456 | 26.624 |
| cpu | 2 | pool256 | 20480 | 19.456 | 47.104 |
| cpu | 2 | pool512 | 20480 | 20.480 | 94.208 |
| cpu | 4 | pool1024 | 20480 | 11.776 | 47.104 |
| cpu | 4 | pool256 | 20480 | 12.288 | 34.816 |
| cpu | 4 | pool512 | 20480 | 11.776 | 16.384 |
| cpu | 8 | pool1024 | 20480 | 11.264 | 19.456 |
| cpu | 8 | pool256 | 20480 | 13.312 | 38.912 |
| cpu | 8 | pool512 | 20480 | 12.288 | 47.104 |
| fanout | 1 | pool1024 | 20480 | 45.056 | 73.728 |
| fanout | 1 | pool256 | 20480 | 47.104 | 196.608 |
| fanout | 1 | pool512 | 20480 | 57.344 | 172.032 |
| fanout | 2 | pool1024 | 20480 | 36.864 | 65.536 |
| fanout | 2 | pool256 | 20480 | 40.960 | 77.824 |
| fanout | 2 | pool512 | 20480 | 43.008 | 196.608 |
| fanout | 4 | pool1024 | 20480 | 28.672 | 77.824 |
| fanout | 4 | pool256 | 20480 | 34.816 | 139.264 |
| fanout | 4 | pool512 | 20480 | 30.720 | 63.488 |
| fanout | 8 | pool1024 | 20480 | 34.816 | 163.840 |
| fanout | 8 | pool256 | 20480 | 31.744 | 122.880 |
| fanout | 8 | pool512 | 20480 | 30.720 | 53.248 |

Diagnostic GC phases summed across measured repetitions (wall time):

- ('backpressure', 1, 'pool1024'): sweep_finalize 91.780 ms, trace 4.362 ms, roots 0.286 ms, resume_world 0.068 ms, prepare 0.067 ms, finish 0.044 ms, accounting 0.029 ms, allocation_census 0.017 ms, trim 0.007 ms, verify 0.005 ms, weak_pages 0.005 ms, stop_world 0.004 ms
- ('backpressure', 1, 'pool256'): sweep_finalize 92.862 ms, trace 4.752 ms, roots 0.372 ms, prepare 0.084 ms, resume_world 0.048 ms, finish 0.045 ms, accounting 0.034 ms, allocation_census 0.019 ms, trim 0.005 ms, weak_pages 0.005 ms, verify 0.005 ms, stop_world 0.005 ms
- ('backpressure', 1, 'pool512'): sweep_finalize 90.899 ms, trace 4.934 ms, roots 0.340 ms, prepare 0.074 ms, resume_world 0.059 ms, finish 0.050 ms, accounting 0.031 ms, allocation_census 0.018 ms, trim 0.007 ms, weak_pages 0.006 ms, verify 0.005 ms, stop_world 0.004 ms
- ('backpressure', 2, 'pool1024'): sweep_finalize 86.604 ms, trace 5.006 ms, roots 0.367 ms, prepare 0.104 ms, finish 0.061 ms, stop_world 0.047 ms, accounting 0.038 ms, allocation_census 0.018 ms, resume_world 0.015 ms, trim 0.007 ms, weak_pages 0.006 ms, verify 0.005 ms
- ('backpressure', 2, 'pool256'): sweep_finalize 85.541 ms, trace 4.585 ms, roots 0.368 ms, prepare 0.113 ms, finish 0.068 ms, stop_world 0.044 ms, accounting 0.037 ms, allocation_census 0.021 ms, resume_world 0.017 ms, trim 0.008 ms, weak_pages 0.006 ms, verify 0.005 ms
- ('backpressure', 2, 'pool512'): sweep_finalize 84.433 ms, trace 4.663 ms, roots 0.353 ms, prepare 0.102 ms, finish 0.060 ms, accounting 0.038 ms, stop_world 0.031 ms, allocation_census 0.019 ms, resume_world 0.016 ms, trim 0.007 ms, weak_pages 0.006 ms, verify 0.005 ms
- ('backpressure', 4, 'pool1024'): sweep_finalize 121.666 ms, stop_world 11.828 ms, trace 7.535 ms, roots 3.587 ms, prepare 0.137 ms, finish 0.124 ms, accounting 0.043 ms, allocation_census 0.029 ms, resume_world 0.019 ms, weak_pages 0.008 ms, trim 0.007 ms, verify 0.007 ms
- ('backpressure', 4, 'pool256'): sweep_finalize 81.764 ms, trace 5.599 ms, roots 0.472 ms, stop_world 0.287 ms, prepare 0.157 ms, finish 0.086 ms, accounting 0.035 ms, allocation_census 0.017 ms, resume_world 0.015 ms, trim 0.007 ms, weak_pages 0.006 ms, verify 0.005 ms
- ('backpressure', 4, 'pool512'): sweep_finalize 121.167 ms, trace 7.271 ms, stop_world 6.414 ms, roots 0.583 ms, prepare 0.173 ms, finish 0.102 ms, accounting 0.042 ms, allocation_census 0.024 ms, resume_world 0.022 ms, weak_pages 0.008 ms, trim 0.007 ms, verify 0.006 ms
- ('backpressure', 8, 'pool1024'): sweep_finalize 93.442 ms, trace 7.482 ms, stop_world 6.310 ms, roots 0.727 ms, prepare 0.187 ms, finish 0.112 ms, accounting 0.040 ms, allocation_census 0.036 ms, resume_world 0.020 ms, weak_pages 0.008 ms, trim 0.007 ms, verify 0.006 ms
- ('backpressure', 8, 'pool256'): sweep_finalize 71.503 ms, trace 5.961 ms, roots 0.459 ms, stop_world 0.167 ms, prepare 0.132 ms, finish 0.115 ms, accounting 0.034 ms, allocation_census 0.027 ms, resume_world 0.016 ms, trim 0.007 ms, weak_pages 0.007 ms, verify 0.006 ms
- ('backpressure', 8, 'pool512'): sweep_finalize 68.064 ms, trace 5.562 ms, stop_world 0.491 ms, roots 0.437 ms, prepare 0.142 ms, finish 0.084 ms, accounting 0.040 ms, allocation_census 0.030 ms, resume_world 0.019 ms, trim 0.008 ms, weak_pages 0.007 ms, verify 0.005 ms
- ('balanced', 1, 'pool1024'): sweep_finalize 73.158 ms, trace 23.143 ms, roots 0.334 ms, prepare 0.070 ms, resume_world 0.056 ms, allocation_census 0.049 ms, finish 0.049 ms, accounting 0.033 ms, weak_pages 0.009 ms, trim 0.006 ms, verify 0.006 ms, stop_world 0.004 ms
- ('balanced', 1, 'pool256'): sweep_finalize 80.927 ms, trace 23.286 ms, roots 0.353 ms, prepare 0.087 ms, resume_world 0.052 ms, finish 0.049 ms, accounting 0.032 ms, allocation_census 0.025 ms, weak_pages 0.008 ms, trim 0.007 ms, verify 0.005 ms, stop_world 0.003 ms
- ('balanced', 1, 'pool512'): sweep_finalize 79.290 ms, trace 30.419 ms, roots 0.438 ms, prepare 0.081 ms, resume_world 0.055 ms, finish 0.053 ms, allocation_census 0.031 ms, accounting 0.031 ms, weak_pages 0.011 ms, verify 0.006 ms, trim 0.006 ms, stop_world 0.004 ms
- ('balanced', 2, 'pool1024'): sweep_finalize 67.191 ms, trace 28.427 ms, roots 0.436 ms, prepare 0.094 ms, finish 0.063 ms, accounting 0.038 ms, stop_world 0.038 ms, allocation_census 0.035 ms, resume_world 0.017 ms, weak_pages 0.014 ms, trim 0.007 ms, verify 0.006 ms
- ('balanced', 2, 'pool256'): sweep_finalize 94.264 ms, trace 30.053 ms, stop_world 2.916 ms, roots 0.571 ms, prepare 0.108 ms, finish 0.067 ms, accounting 0.041 ms, allocation_census 0.034 ms, resume_world 0.017 ms, weak_pages 0.012 ms, trim 0.007 ms, verify 0.006 ms
- ('balanced', 2, 'pool512'): sweep_finalize 141.493 ms, trace 41.141 ms, stop_world 18.396 ms, roots 0.944 ms, prepare 0.173 ms, finish 0.086 ms, allocation_census 0.044 ms, accounting 0.043 ms, resume_world 0.019 ms, weak_pages 0.017 ms, verify 0.008 ms, trim 0.007 ms
- ('balanced', 4, 'pool1024'): sweep_finalize 66.841 ms, trace 32.030 ms, stop_world 5.538 ms, roots 0.588 ms, prepare 0.139 ms, finish 0.071 ms, accounting 0.035 ms, allocation_census 0.035 ms, resume_world 0.017 ms, weak_pages 0.012 ms, trim 0.008 ms, verify 0.007 ms
- ('balanced', 4, 'pool256'): sweep_finalize 72.021 ms, trace 33.118 ms, stop_world 12.282 ms, roots 0.913 ms, prepare 0.154 ms, finish 0.078 ms, accounting 0.035 ms, allocation_census 0.031 ms, resume_world 0.014 ms, weak_pages 0.013 ms, trim 0.007 ms, verify 0.006 ms
- ('balanced', 4, 'pool512'): sweep_finalize 41.997 ms, trace 22.504 ms, roots 0.379 ms, stop_world 0.134 ms, prepare 0.099 ms, finish 0.057 ms, accounting 0.030 ms, allocation_census 0.025 ms, resume_world 0.011 ms, weak_pages 0.009 ms, trim 0.007 ms, verify 0.006 ms
- ('balanced', 8, 'pool1024'): sweep_finalize 60.644 ms, trace 32.053 ms, stop_world 6.504 ms, roots 0.638 ms, prepare 0.155 ms, finish 0.066 ms, allocation_census 0.051 ms, accounting 0.035 ms, resume_world 0.016 ms, weak_pages 0.012 ms, trim 0.007 ms, verify 0.006 ms
- ('balanced', 8, 'pool256'): sweep_finalize 77.729 ms, trace 30.424 ms, stop_world 8.864 ms, roots 0.659 ms, prepare 0.142 ms, finish 0.113 ms, allocation_census 0.053 ms, accounting 0.039 ms, resume_world 0.019 ms, weak_pages 0.012 ms, trim 0.007 ms, verify 0.007 ms
- ('balanced', 8, 'pool512'): sweep_finalize 44.691 ms, trace 27.934 ms, roots 0.484 ms, stop_world 0.469 ms, prepare 0.137 ms, finish 0.071 ms, allocation_census 0.043 ms, accounting 0.035 ms, resume_world 0.014 ms, weak_pages 0.012 ms, trim 0.007 ms, verify 0.007 ms
- ('cpu', 1, 'pool1024'): sweep_finalize 70.391 ms, trace 22.496 ms, roots 0.434 ms, prepare 0.068 ms, resume_world 0.048 ms, finish 0.046 ms, accounting 0.028 ms, allocation_census 0.025 ms, weak_pages 0.008 ms, trim 0.007 ms, verify 0.005 ms, stop_world 0.005 ms
- ('cpu', 1, 'pool256'): sweep_finalize 99.015 ms, trace 31.710 ms, roots 0.820 ms, prepare 0.089 ms, resume_world 0.065 ms, finish 0.060 ms, allocation_census 0.041 ms, accounting 0.040 ms, weak_pages 0.013 ms, verify 0.007 ms, trim 0.007 ms, stop_world 0.005 ms
- ('cpu', 1, 'pool512'): sweep_finalize 71.400 ms, trace 24.026 ms, roots 0.458 ms, prepare 0.070 ms, resume_world 0.055 ms, finish 0.041 ms, accounting 0.028 ms, allocation_census 0.024 ms, weak_pages 0.008 ms, trim 0.006 ms, verify 0.005 ms, stop_world 0.004 ms
- ('cpu', 2, 'pool1024'): sweep_finalize 56.962 ms, trace 24.611 ms, roots 0.451 ms, stop_world 0.311 ms, prepare 0.081 ms, finish 0.054 ms, accounting 0.039 ms, allocation_census 0.024 ms, resume_world 0.013 ms, weak_pages 0.010 ms, trim 0.007 ms, verify 0.006 ms
- ('cpu', 2, 'pool256'): sweep_finalize 74.512 ms, trace 26.752 ms, stop_world 2.623 ms, roots 0.658 ms, prepare 0.113 ms, finish 0.045 ms, accounting 0.040 ms, allocation_census 0.026 ms, resume_world 0.014 ms, weak_pages 0.009 ms, trim 0.007 ms, verify 0.006 ms
- ('cpu', 2, 'pool512'): sweep_finalize 80.981 ms, trace 35.632 ms, stop_world 6.711 ms, roots 0.561 ms, prepare 0.132 ms, finish 0.064 ms, accounting 0.041 ms, allocation_census 0.031 ms, resume_world 0.014 ms, weak_pages 0.013 ms, trim 0.007 ms, verify 0.006 ms
- ('cpu', 4, 'pool1024'): sweep_finalize 59.339 ms, trace 29.351 ms, stop_world 7.584 ms, roots 0.649 ms, prepare 0.127 ms, finish 0.063 ms, accounting 0.034 ms, allocation_census 0.029 ms, resume_world 0.015 ms, weak_pages 0.011 ms, trim 0.007 ms, verify 0.007 ms
- ('cpu', 4, 'pool256'): sweep_finalize 54.032 ms, trace 24.862 ms, stop_world 1.316 ms, roots 0.567 ms, prepare 0.115 ms, finish 0.070 ms, accounting 0.033 ms, allocation_census 0.027 ms, resume_world 0.013 ms, weak_pages 0.010 ms, trim 0.007 ms, verify 0.007 ms
- ('cpu', 4, 'pool512'): sweep_finalize 45.646 ms, trace 25.361 ms, roots 0.490 ms, stop_world 0.282 ms, prepare 0.101 ms, finish 0.060 ms, accounting 0.034 ms, allocation_census 0.028 ms, resume_world 0.012 ms, weak_pages 0.011 ms, trim 0.007 ms, verify 0.006 ms
- ('cpu', 8, 'pool1024'): sweep_finalize 43.968 ms, trace 25.352 ms, roots 0.519 ms, stop_world 0.149 ms, prepare 0.128 ms, finish 0.068 ms, allocation_census 0.039 ms, accounting 0.035 ms, resume_world 0.013 ms, weak_pages 0.010 ms, trim 0.007 ms, verify 0.006 ms
- ('cpu', 8, 'pool256'): sweep_finalize 62.756 ms, trace 24.809 ms, roots 0.614 ms, prepare 0.212 ms, stop_world 0.132 ms, finish 0.086 ms, allocation_census 0.041 ms, accounting 0.036 ms, resume_world 0.017 ms, weak_pages 0.010 ms, trim 0.008 ms, verify 0.006 ms
- ('cpu', 8, 'pool512'): sweep_finalize 67.928 ms, trace 29.854 ms, stop_world 9.863 ms, roots 0.599 ms, prepare 0.139 ms, finish 0.082 ms, allocation_census 0.050 ms, accounting 0.036 ms, resume_world 0.017 ms, weak_pages 0.012 ms, trim 0.007 ms, verify 0.007 ms
- ('fanout', 1, 'pool1024'): sweep_finalize 185.054 ms, trace 49.905 ms, prepare 7.045 ms, weak_pages 6.435 ms, roots 4.252 ms, trim 3.461 ms, finish 0.098 ms, resume_world 0.078 ms, accounting 0.058 ms, allocation_census 0.049 ms, verify 0.011 ms, stop_world 0.009 ms
- ('fanout', 1, 'pool256'): sweep_finalize 249.525 ms, trace 56.735 ms, prepare 8.013 ms, weak_pages 6.461 ms, roots 5.857 ms, trim 4.627 ms, finish 0.104 ms, resume_world 0.080 ms, accounting 0.069 ms, allocation_census 0.061 ms, verify 0.026 ms, stop_world 0.009 ms
- ('fanout', 1, 'pool512'): sweep_finalize 239.070 ms, trace 69.066 ms, roots 8.522 ms, prepare 8.306 ms, weak_pages 7.493 ms, trim 3.902 ms, finish 0.112 ms, resume_world 0.093 ms, allocation_census 0.086 ms, accounting 0.069 ms, verify 0.013 ms, stop_world 0.012 ms
- ('fanout', 2, 'pool1024'): sweep_finalize 171.948 ms, trace 51.321 ms, prepare 7.546 ms, weak_pages 6.827 ms, roots 3.581 ms, trim 2.284 ms, finish 0.089 ms, accounting 0.065 ms, allocation_census 0.055 ms, stop_world 0.053 ms, resume_world 0.033 ms, verify 0.011 ms
- ('fanout', 2, 'pool256'): sweep_finalize 226.169 ms, trace 65.479 ms, prepare 8.777 ms, weak_pages 8.132 ms, roots 5.275 ms, trim 2.944 ms, finish 0.108 ms, accounting 0.076 ms, allocation_census 0.071 ms, stop_world 0.068 ms, resume_world 0.042 ms, verify 0.013 ms
- ('fanout', 2, 'pool512'): sweep_finalize 246.695 ms, trace 87.491 ms, prepare 9.017 ms, roots 8.719 ms, weak_pages 7.275 ms, stop_world 4.758 ms, trim 2.537 ms, finish 0.102 ms, allocation_census 0.076 ms, accounting 0.070 ms, resume_world 0.037 ms, verify 0.014 ms
- ('fanout', 4, 'pool1024'): sweep_finalize 153.438 ms, trace 58.446 ms, prepare 9.572 ms, weak_pages 7.951 ms, roots 4.754 ms, trim 2.682 ms, stop_world 0.314 ms, finish 0.127 ms, accounting 0.075 ms, allocation_census 0.059 ms, resume_world 0.034 ms, verify 0.013 ms
- ('fanout', 4, 'pool256'): sweep_finalize 212.359 ms, trace 63.878 ms, weak_pages 9.563 ms, prepare 8.395 ms, roots 6.308 ms, stop_world 5.013 ms, trim 2.789 ms, finish 0.106 ms, accounting 0.082 ms, allocation_census 0.067 ms, resume_world 0.044 ms, verify 0.015 ms
- ('fanout', 4, 'pool512'): sweep_finalize 150.675 ms, trace 54.156 ms, prepare 7.003 ms, weak_pages 6.930 ms, roots 4.010 ms, trim 2.288 ms, stop_world 0.152 ms, finish 0.126 ms, accounting 0.079 ms, allocation_census 0.060 ms, resume_world 0.037 ms, verify 0.013 ms
- ('fanout', 8, 'pool1024'): sweep_finalize 194.202 ms, trace 84.362 ms, stop_world 14.237 ms, prepare 10.916 ms, weak_pages 9.693 ms, roots 8.486 ms, trim 2.793 ms, finish 0.153 ms, allocation_census 0.107 ms, accounting 0.076 ms, resume_world 0.034 ms, verify 0.014 ms
- ('fanout', 8, 'pool256'): sweep_finalize 167.725 ms, trace 60.702 ms, prepare 8.294 ms, weak_pages 7.226 ms, roots 4.728 ms, trim 3.164 ms, stop_world 2.940 ms, finish 0.113 ms, allocation_census 0.093 ms, accounting 0.070 ms, resume_world 0.037 ms, verify 0.013 ms
- ('fanout', 8, 'pool512'): sweep_finalize 121.926 ms, trace 56.846 ms, prepare 7.297 ms, weak_pages 7.140 ms, roots 4.204 ms, trim 2.535 ms, stop_world 0.294 ms, finish 0.122 ms, allocation_census 0.074 ms, accounting 0.068 ms, resume_world 0.028 ms, verify 0.015 ms
