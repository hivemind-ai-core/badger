| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| bytes-survivors | 1 | baseline | 102.115 [95.814, 113.206] | 1.865 | 40.9 |
| bytes-survivors | 1 | diagnostic | 102.031 [102.026, 105.154] | 1.846 | 41.0 |
| bytes-survivors | 1 | diagnostic-allocations | 108.546 [102.546, 109.379] | 2.274 | 44.0 |
| bytes-survivors | 1 | diagnostic-off | 99.507 [99.450, 103.686] | 1.870 | 41.3 |
| bytes-survivors | 2 | baseline | 106.739 [100.833, 107.464] | 1.953 | 39.7 |
| bytes-survivors | 2 | diagnostic | 98.182 [97.928, 99.993] | 1.913 | 39.3 |
| bytes-survivors | 2 | diagnostic-allocations | 109.383 [107.014, 115.042] | 2.298 | 42.3 |
| bytes-survivors | 2 | diagnostic-off | 101.062 [99.466, 111.030] | 1.871 | 39.0 |
| bytes-survivors | 4 | baseline | 108.648 [101.624, 110.198] | 1.875 | 39.0 |
| bytes-survivors | 4 | diagnostic | 100.407 [98.684, 103.122] | 1.962 | 39.5 |
| bytes-survivors | 4 | diagnostic-allocations | 103.620 [102.723, 115.786] | 2.298 | 42.7 |
| bytes-survivors | 4 | diagnostic-off | 99.184 [92.698, 107.470] | 1.852 | 39.1 |
| bytes-survivors | 8 | baseline | 108.548 [104.442, 109.645] | 1.873 | 39.6 |
| bytes-survivors | 8 | diagnostic | 97.304 [96.946, 99.966] | 1.923 | 39.3 |
| bytes-survivors | 8 | diagnostic-allocations | 104.320 [98.404, 106.488] | 2.188 | 42.4 |
| bytes-survivors | 8 | diagnostic-off | 98.393 [96.064, 103.439] | 1.863 | 39.3 |
| bytes-transient | 1 | baseline | 112.004 [108.006, 113.823] | 1.847 | 39.7 |
| bytes-transient | 1 | diagnostic | 99.966 [98.222, 111.888] | 1.815 | 39.7 |
| bytes-transient | 1 | diagnostic-allocations | 107.702 [105.118, 108.011] | 2.141 | 41.8 |
| bytes-transient | 1 | diagnostic-off | 103.762 [100.049, 104.555] | 1.859 | 39.5 |
| bytes-transient | 2 | baseline | 104.813 [102.943, 108.896] | 1.867 | 37.9 |
| bytes-transient | 2 | diagnostic | 100.099 [96.375, 113.466] | 1.884 | 38.1 |
| bytes-transient | 2 | diagnostic-allocations | 103.431 [101.866, 107.786] | 2.224 | 40.2 |
| bytes-transient | 2 | diagnostic-off | 97.934 [95.441, 98.564] | 1.939 | 37.9 |
| bytes-transient | 4 | baseline | 103.887 [100.809, 110.781] | 1.910 | 37.9 |
| bytes-transient | 4 | diagnostic | 98.646 [96.500, 110.463] | 1.930 | 38.0 |
| bytes-transient | 4 | diagnostic-allocations | 99.906 [99.708, 99.907] | 2.183 | 40.2 |
| bytes-transient | 4 | diagnostic-off | 106.376 [95.915, 109.841] | 1.873 | 37.6 |
| bytes-transient | 8 | baseline | 103.340 [98.269, 111.322] | 1.916 | 37.9 |
| bytes-transient | 8 | diagnostic | 97.265 [94.461, 97.697] | 1.828 | 38.2 |
| bytes-transient | 8 | diagnostic-allocations | 104.965 [100.127, 105.111] | 2.180 | 40.0 |
| bytes-transient | 8 | diagnostic-off | 96.804 [95.116, 100.704] | 1.858 | 37.9 |
| messages | 1 | baseline | 28.166 [27.613, 28.453] | 4.913 | 48.3 |
| messages | 1 | diagnostic | 29.580 [27.956, 31.571] | 4.896 | 48.8 |
| messages | 1 | diagnostic-allocations | 57.932 [55.514, 60.130] | 8.834 | 56.5 |
| messages | 1 | diagnostic-off | 28.482 [27.846, 29.519] | 5.010 | 48.7 |
| messages | 2 | baseline | 30.150 [29.991, 31.277] | 5.082 | 57.4 |
| messages | 2 | diagnostic | 28.777 [28.258, 33.206] | 4.622 | 50.3 |
| messages | 2 | diagnostic-allocations | 63.535 [58.872, 72.260] | 9.540 | 81.6 |
| messages | 2 | diagnostic-off | 31.081 [29.916, 32.632] | 4.705 | 60.2 |
| messages | 4 | baseline | 30.394 [29.610, 31.542] | 5.045 | 59.5 |
| messages | 4 | diagnostic | 33.132 [31.438, 35.485] | 5.045 | 64.4 |
| messages | 4 | diagnostic-allocations | 72.523 [70.771, 75.373] | 9.322 | 112.5 |
| messages | 4 | diagnostic-off | 29.539 [29.099, 34.957] | 4.874 | 57.9 |
| messages | 8 | baseline | 30.040 [28.174, 33.602] | 5.175 | 59.1 |
| messages | 8 | diagnostic | 32.301 [30.620, 34.177] | 5.395 | 71.9 |
| messages | 8 | diagnostic-allocations | 72.844 [70.822, 77.606] | 9.348 | 116.2 |
| messages | 8 | diagnostic-off | 35.074 [34.754, 36.088] | 5.331 | 71.6 |
| messages-fanout | 1 | baseline | 43.696 [42.971, 45.981] | 8.755 | 48.6 |
| messages-fanout | 1 | diagnostic | 46.694 [46.584, 59.835] | 7.837 | 48.6 |
| messages-fanout | 1 | diagnostic-allocations | 89.118 [88.371, 89.338] | 14.415 | 56.5 |
| messages-fanout | 1 | diagnostic-off | 44.021 [43.269, 45.422] | 8.539 | 49.1 |
| messages-fanout | 2 | baseline | 45.085 [44.517, 45.303] | 7.645 | 53.9 |
| messages-fanout | 2 | diagnostic | 46.989 [45.494, 48.140] | 8.116 | 54.6 |
| messages-fanout | 2 | diagnostic-allocations | 84.597 [82.831, 84.734] | 14.579 | 59.6 |
| messages-fanout | 2 | diagnostic-off | 48.461 [48.362, 49.520] | 7.723 | 54.5 |
| messages-fanout | 4 | baseline | 37.524 [37.241, 38.469] | 5.738 | 59.1 |
| messages-fanout | 4 | diagnostic | 38.289 [36.750, 40.102] | 6.694 | 61.4 |
| messages-fanout | 4 | diagnostic-allocations | 83.059 [80.596, 83.923] | 12.983 | 86.8 |
| messages-fanout | 4 | diagnostic-off | 37.328 [36.768, 39.390] | 5.585 | 65.4 |
| messages-fanout | 8 | baseline | 39.844 [39.021, 40.761] | 5.471 | 70.4 |
| messages-fanout | 8 | diagnostic | 43.220 [41.571, 43.387] | 5.282 | 73.4 |
| messages-fanout | 8 | diagnostic-allocations | 84.217 [83.055, 86.207] | 12.470 | 117.8 |
| messages-fanout | 8 | diagnostic-off | 40.799 [39.845, 44.374] | 5.594 | 68.0 |
| messages-retained | 1 | baseline | 28.614 [28.401, 28.966] | 4.850 | 48.7 |
| messages-retained | 1 | diagnostic | 28.575 [28.428, 28.654] | 4.886 | 48.6 |
| messages-retained | 1 | diagnostic-allocations | 57.830 [55.764, 58.013] | 8.361 | 56.4 |
| messages-retained | 1 | diagnostic-off | 30.217 [28.840, 31.185] | 4.764 | 48.7 |
| messages-retained | 2 | baseline | 30.423 [29.180, 31.159] | 5.249 | 53.2 |
| messages-retained | 2 | diagnostic | 32.670 [31.864, 33.585] | 5.096 | 58.0 |
| messages-retained | 2 | diagnostic-allocations | 63.851 [62.531, 65.411] | 10.031 | 85.8 |
| messages-retained | 2 | diagnostic-off | 32.362 [31.036, 32.858] | 4.416 | 55.8 |
| messages-retained | 4 | baseline | 31.438 [30.504, 34.586] | 4.907 | 69.8 |
| messages-retained | 4 | diagnostic | 31.502 [31.250, 37.055] | 5.172 | 64.9 |
| messages-retained | 4 | diagnostic-allocations | 69.542 [68.759, 77.264] | 8.409 | 108.7 |
| messages-retained | 4 | diagnostic-off | 32.875 [31.393, 35.735] | 4.175 | 67.3 |
| messages-retained | 8 | baseline | 32.005 [31.029, 35.337] | 5.508 | 74.2 |
| messages-retained | 8 | diagnostic | 34.485 [33.983, 35.462] | 5.030 | 72.1 |
| messages-retained | 8 | diagnostic-allocations | 72.986 [70.686, 73.651] | 9.037 | 117.7 |
| messages-retained | 8 | diagnostic-off | 31.912 [31.072, 33.921] | 4.851 | 70.1 |
| messages-slow | 1 | baseline | 32.143 [31.184, 32.806] | 4.740 | 49.1 |
| messages-slow | 1 | diagnostic | 33.410 [31.657, 34.119] | 4.895 | 48.8 |
| messages-slow | 1 | diagnostic-allocations | 62.895 [62.400, 64.441] | 9.647 | 56.6 |
| messages-slow | 1 | diagnostic-off | 31.853 [31.842, 32.228] | 4.841 | 48.6 |
| messages-slow | 2 | baseline | 36.233 [34.540, 36.836] | 4.824 | 60.0 |
| messages-slow | 2 | diagnostic | 35.208 [33.648, 36.797] | 4.926 | 65.2 |
| messages-slow | 2 | diagnostic-allocations | 78.148 [71.671, 78.409] | 9.481 | 81.7 |
| messages-slow | 2 | diagnostic-off | 36.360 [35.963, 37.119] | 5.081 | 67.8 |
| messages-slow | 4 | baseline | 42.440 [41.311, 48.956] | 4.246 | 50.4 |
| messages-slow | 4 | diagnostic | 40.889 [37.096, 46.312] | 3.958 | 52.0 |
| messages-slow | 4 | diagnostic-allocations | 81.129 [79.124, 83.306] | 7.431 | 71.4 |
| messages-slow | 4 | diagnostic-off | 44.714 [44.682, 46.126] | 4.179 | 51.6 |
| messages-slow | 8 | baseline | 42.639 [36.565, 46.637] | 3.497 | 58.3 |
| messages-slow | 8 | diagnostic | 41.633 [39.656, 46.725] | 3.612 | 56.1 |
| messages-slow | 8 | diagnostic-allocations | 81.142 [76.450, 82.787] | 7.091 | 71.9 |
| messages-slow | 8 | diagnostic-off | 45.408 [44.504, 46.013] | 3.612 | 58.1 |
| snapshots | 1 | baseline | 32.544 [31.514, 32.927] | 9.492 | 41.3 |
| snapshots | 1 | diagnostic | 31.427 [31.199, 32.891] | 9.307 | 41.2 |
| snapshots | 1 | diagnostic-allocations | 41.458 [38.422, 43.633] | 13.022 | 42.7 |
| snapshots | 1 | diagnostic-off | 32.261 [32.065, 33.321] | 9.639 | 41.1 |
| snapshots | 2 | baseline | 35.172 [34.457, 36.136] | 9.492 | 39.2 |
| snapshots | 2 | diagnostic | 33.796 [32.673, 38.344] | 9.563 | 39.5 |
| snapshots | 2 | diagnostic-allocations | 42.671 [41.431, 44.475] | 13.508 | 40.9 |
| snapshots | 2 | diagnostic-off | 34.395 [33.679, 34.739] | 9.485 | 39.3 |
| snapshots | 4 | baseline | 34.123 [33.888, 36.147] | 9.910 | 39.3 |
| snapshots | 4 | diagnostic | 33.641 [33.266, 34.802] | 9.631 | 39.5 |
| snapshots | 4 | diagnostic-allocations | 44.417 [41.025, 45.578] | 12.978 | 41.3 |
| snapshots | 4 | diagnostic-off | 33.120 [31.657, 34.693] | 9.357 | 39.6 |
| snapshots | 8 | baseline | 35.706 [34.666, 36.294] | 10.130 | 39.2 |
| snapshots | 8 | diagnostic | 34.915 [34.713, 37.504] | 9.708 | 39.7 |
| snapshots | 8 | diagnostic-allocations | 41.807 [40.080, 43.193] | 11.086 | 40.9 |
| snapshots | 8 | diagnostic-off | 35.123 [32.812, 37.514] | 9.242 | 39.4 |
| words-transient | 1 | baseline | 117.951 [117.206, 122.626] | 8.007 | 39.8 |
| words-transient | 1 | diagnostic | 123.982 [120.858, 126.949] | 7.957 | 40.1 |
| words-transient | 1 | diagnostic-allocations | 131.079 [125.431, 132.485] | 8.080 | 40.3 |
| words-transient | 1 | diagnostic-off | 121.598 [119.581, 121.957] | 8.108 | 40.1 |
| words-transient | 2 | baseline | 123.778 [121.661, 125.304] | 8.724 | 38.0 |
| words-transient | 2 | diagnostic | 119.518 [118.753, 125.964] | 9.422 | 38.0 |
| words-transient | 2 | diagnostic-allocations | 123.900 [123.162, 127.947] | 9.275 | 38.6 |
| words-transient | 2 | diagnostic-off | 123.469 [122.243, 124.545] | 8.683 | 37.8 |
| words-transient | 4 | baseline | 124.465 [122.011, 126.541] | 8.991 | 37.7 |
| words-transient | 4 | diagnostic | 123.923 [123.288, 126.739] | 8.848 | 37.7 |
| words-transient | 4 | diagnostic-allocations | 124.756 [124.515, 125.272] | 8.692 | 38.6 |
| words-transient | 4 | diagnostic-off | 123.608 [120.300, 125.374] | 8.683 | 39.4 |
| words-transient | 8 | baseline | 118.697 [117.732, 120.967] | 8.611 | 37.7 |
| words-transient | 8 | diagnostic | 122.044 [121.801, 122.080] | 8.781 | 38.1 |
| words-transient | 8 | diagnostic-allocations | 129.600 [127.104, 130.544] | 9.007 | 38.5 |
| words-transient | 8 | diagnostic-off | 124.380 [120.902, 125.490] | 8.265 | 38.1 |

Diagnostic GC phases summed across measured repetitions (wall time):

- ('bytes-survivors', 1, 'diagnostic'): sweep_finalize 4.783 ms, trace 0.770 ms, resume_world 0.033 ms, roots 0.030 ms, prepare 0.008 ms, finish 0.005 ms, accounting 0.003 ms, allocation_census 0.002 ms, verify 0.001 ms, stop_world 0.001 ms, trim 0.001 ms, weak_pages 0.000 ms
- ('bytes-survivors', 1, 'diagnostic-allocations'): sweep_finalize 4.778 ms, allocation_census 1.171 ms, trace 0.694 ms, resume_world 0.041 ms, roots 0.040 ms, prepare 0.008 ms, finish 0.005 ms, accounting 0.003 ms, verify 0.001 ms, trim 0.001 ms, stop_world 0.000 ms, weak_pages 0.000 ms
- ('bytes-survivors', 2, 'diagnostic'): sweep_finalize 4.879 ms, trace 0.763 ms, roots 0.019 ms, prepare 0.007 ms, stop_world 0.006 ms, finish 0.004 ms, accounting 0.003 ms, allocation_census 0.001 ms, resume_world 0.001 ms, trim 0.001 ms, verify 0.001 ms, weak_pages 0.000 ms
- ('bytes-survivors', 2, 'diagnostic-allocations'): sweep_finalize 6.281 ms, allocation_census 1.120 ms, trace 0.691 ms, roots 0.020 ms, prepare 0.011 ms, finish 0.010 ms, stop_world 0.009 ms, accounting 0.006 ms, resume_world 0.003 ms, trim 0.001 ms, verify 0.001 ms, weak_pages 0.000 ms
- ('bytes-survivors', 4, 'diagnostic'): sweep_finalize 7.000 ms, trace 0.769 ms, roots 0.021 ms, prepare 0.008 ms, stop_world 0.006 ms, finish 0.005 ms, accounting 0.004 ms, resume_world 0.002 ms, allocation_census 0.001 ms, trim 0.001 ms, verify 0.000 ms, weak_pages 0.000 ms
- ('bytes-survivors', 4, 'diagnostic-allocations'): sweep_finalize 5.490 ms, allocation_census 1.090 ms, trace 0.683 ms, roots 0.026 ms, prepare 0.008 ms, finish 0.007 ms, stop_world 0.006 ms, accounting 0.004 ms, resume_world 0.002 ms, trim 0.001 ms, verify 0.001 ms, weak_pages 0.000 ms
- ('bytes-survivors', 8, 'diagnostic'): sweep_finalize 4.868 ms, trace 0.760 ms, roots 0.030 ms, prepare 0.008 ms, stop_world 0.006 ms, finish 0.004 ms, accounting 0.003 ms, resume_world 0.002 ms, allocation_census 0.001 ms, trim 0.001 ms, verify 0.000 ms, weak_pages 0.000 ms
- ('bytes-survivors', 8, 'diagnostic-allocations'): sweep_finalize 4.742 ms, allocation_census 1.102 ms, trace 0.671 ms, roots 0.030 ms, prepare 0.008 ms, stop_world 0.007 ms, finish 0.005 ms, accounting 0.003 ms, resume_world 0.001 ms, verify 0.001 ms, trim 0.001 ms, weak_pages 0.000 ms
- ('bytes-transient', 1, 'diagnostic'): sweep_finalize 5.395 ms, roots 0.038 ms, resume_world 0.035 ms, trace 0.013 ms, prepare 0.010 ms, finish 0.005 ms, accounting 0.004 ms, allocation_census 0.001 ms, trim 0.001 ms, verify 0.000 ms, stop_world 0.000 ms, weak_pages 0.000 ms
- ('bytes-transient', 1, 'diagnostic-allocations'): sweep_finalize 5.457 ms, allocation_census 0.931 ms, resume_world 0.032 ms, roots 0.030 ms, trace 0.013 ms, prepare 0.007 ms, finish 0.005 ms, accounting 0.004 ms, trim 0.001 ms, verify 0.001 ms, stop_world 0.000 ms, weak_pages 0.000 ms
- ('bytes-transient', 2, 'diagnostic'): sweep_finalize 5.662 ms, roots 0.022 ms, trace 0.014 ms, prepare 0.007 ms, stop_world 0.006 ms, finish 0.004 ms, accounting 0.004 ms, resume_world 0.001 ms, allocation_census 0.001 ms, trim 0.001 ms, verify 0.000 ms, weak_pages 0.000 ms
- ('bytes-transient', 2, 'diagnostic-allocations'): sweep_finalize 7.314 ms, allocation_census 0.923 ms, roots 0.022 ms, trace 0.013 ms, prepare 0.008 ms, stop_world 0.006 ms, finish 0.006 ms, accounting 0.004 ms, resume_world 0.002 ms, trim 0.001 ms, verify 0.001 ms, weak_pages 0.000 ms
- ('bytes-transient', 4, 'diagnostic'): sweep_finalize 5.677 ms, roots 0.020 ms, trace 0.015 ms, prepare 0.008 ms, stop_world 0.007 ms, finish 0.005 ms, accounting 0.004 ms, resume_world 0.002 ms, allocation_census 0.001 ms, trim 0.001 ms, verify 0.001 ms, weak_pages 0.000 ms
- ('bytes-transient', 4, 'diagnostic-allocations'): sweep_finalize 5.843 ms, allocation_census 0.907 ms, roots 0.020 ms, trace 0.013 ms, prepare 0.008 ms, stop_world 0.006 ms, accounting 0.006 ms, finish 0.005 ms, resume_world 0.002 ms, trim 0.001 ms, verify 0.001 ms, weak_pages 0.000 ms
- ('bytes-transient', 8, 'diagnostic'): sweep_finalize 5.633 ms, roots 0.030 ms, trace 0.013 ms, prepare 0.008 ms, finish 0.005 ms, stop_world 0.005 ms, accounting 0.003 ms, resume_world 0.001 ms, allocation_census 0.001 ms, verify 0.001 ms, trim 0.001 ms, weak_pages 0.000 ms
- ('bytes-transient', 8, 'diagnostic-allocations'): sweep_finalize 6.416 ms, allocation_census 0.890 ms, roots 0.030 ms, trace 0.015 ms, prepare 0.008 ms, stop_world 0.006 ms, finish 0.005 ms, accounting 0.004 ms, resume_world 0.002 ms, trim 0.001 ms, verify 0.001 ms, weak_pages 0.000 ms
- ('messages', 1, 'diagnostic'): sweep_finalize 14.386 ms, roots 0.065 ms, trace 0.051 ms, resume_world 0.032 ms, prepare 0.018 ms, finish 0.014 ms, accounting 0.010 ms, allocation_census 0.004 ms, trim 0.002 ms, stop_world 0.002 ms, verify 0.001 ms, weak_pages 0.001 ms
- ('messages', 1, 'diagnostic-allocations'): sweep_finalize 14.641 ms, allocation_census 11.612 ms, roots 0.058 ms, trace 0.049 ms, resume_world 0.041 ms, prepare 0.025 ms, finish 0.017 ms, accounting 0.011 ms, trim 0.002 ms, verify 0.001 ms, stop_world 0.001 ms, weak_pages 0.001 ms
- ('messages', 2, 'diagnostic'): sweep_finalize 13.812 ms, roots 0.053 ms, trace 0.038 ms, finish 0.023 ms, prepare 0.020 ms, accounting 0.012 ms, stop_world 0.011 ms, resume_world 0.005 ms, allocation_census 0.004 ms, trim 0.002 ms, verify 0.002 ms, weak_pages 0.001 ms
- ('messages', 2, 'diagnostic-allocations'): sweep_finalize 16.526 ms, allocation_census 11.370 ms, roots 0.057 ms, trace 0.040 ms, prepare 0.019 ms, finish 0.014 ms, accounting 0.012 ms, stop_world 0.011 ms, resume_world 0.005 ms, trim 0.002 ms, verify 0.002 ms, weak_pages 0.001 ms
- ('messages', 4, 'diagnostic'): sweep_finalize 16.733 ms, roots 0.051 ms, trace 0.039 ms, finish 0.020 ms, prepare 0.019 ms, accounting 0.013 ms, stop_world 0.011 ms, resume_world 0.005 ms, allocation_census 0.005 ms, trim 0.003 ms, verify 0.001 ms, weak_pages 0.001 ms
- ('messages', 4, 'diagnostic-allocations'): sweep_finalize 16.723 ms, allocation_census 11.387 ms, roots 0.061 ms, trace 0.049 ms, prepare 0.022 ms, finish 0.017 ms, accounting 0.012 ms, stop_world 0.011 ms, resume_world 0.005 ms, trim 0.002 ms, weak_pages 0.002 ms, verify 0.002 ms
- ('messages', 8, 'diagnostic'): sweep_finalize 15.840 ms, roots 0.051 ms, trace 0.040 ms, prepare 0.020 ms, finish 0.017 ms, accounting 0.012 ms, stop_world 0.012 ms, allocation_census 0.004 ms, resume_world 0.004 ms, trim 0.002 ms, verify 0.001 ms, weak_pages 0.001 ms
- ('messages', 8, 'diagnostic-allocations'): sweep_finalize 16.856 ms, allocation_census 11.136 ms, roots 0.054 ms, trace 0.048 ms, prepare 0.025 ms, finish 0.019 ms, accounting 0.014 ms, stop_world 0.012 ms, resume_world 0.004 ms, trim 0.002 ms, weak_pages 0.002 ms, verify 0.002 ms
- ('messages-fanout', 1, 'diagnostic'): sweep_finalize 24.586 ms, trace 0.090 ms, roots 0.076 ms, resume_world 0.039 ms, prepare 0.033 ms, finish 0.025 ms, accounting 0.020 ms, allocation_census 0.018 ms, trim 0.004 ms, stop_world 0.003 ms, weak_pages 0.002 ms, verify 0.002 ms
- ('messages-fanout', 1, 'diagnostic-allocations'): sweep_finalize 23.403 ms, allocation_census 19.515 ms, trace 0.096 ms, roots 0.075 ms, prepare 0.043 ms, resume_world 0.040 ms, finish 0.035 ms, accounting 0.018 ms, trim 0.003 ms, weak_pages 0.003 ms, stop_world 0.003 ms, verify 0.003 ms
- ('messages-fanout', 2, 'diagnostic'): sweep_finalize 23.513 ms, trace 0.100 ms, roots 0.097 ms, prepare 0.036 ms, finish 0.025 ms, accounting 0.019 ms, stop_world 0.017 ms, allocation_census 0.006 ms, resume_world 0.005 ms, trim 0.003 ms, weak_pages 0.003 ms, verify 0.003 ms
- ('messages-fanout', 2, 'diagnostic-allocations'): sweep_finalize 24.767 ms, allocation_census 19.486 ms, trace 0.099 ms, roots 0.079 ms, stop_world 0.037 ms, prepare 0.032 ms, finish 0.027 ms, accounting 0.023 ms, resume_world 0.007 ms, trim 0.003 ms, verify 0.003 ms, weak_pages 0.003 ms
- ('messages-fanout', 4, 'diagnostic'): sweep_finalize 18.036 ms, stop_world 1.242 ms, trace 0.093 ms, roots 0.076 ms, prepare 0.036 ms, finish 0.025 ms, accounting 0.020 ms, allocation_census 0.007 ms, resume_world 0.006 ms, trim 0.004 ms, verify 0.003 ms, weak_pages 0.002 ms
- ('messages-fanout', 4, 'diagnostic-allocations'): allocation_census 20.048 ms, sweep_finalize 18.719 ms, trace 0.121 ms, roots 0.077 ms, prepare 0.042 ms, finish 0.026 ms, accounting 0.021 ms, stop_world 0.017 ms, resume_world 0.007 ms, trim 0.004 ms, weak_pages 0.003 ms, verify 0.003 ms
- ('messages-fanout', 8, 'diagnostic'): sweep_finalize 15.300 ms, stop_world 0.136 ms, trace 0.103 ms, roots 0.082 ms, prepare 0.039 ms, finish 0.025 ms, accounting 0.019 ms, allocation_census 0.016 ms, resume_world 0.005 ms, trim 0.003 ms, verify 0.003 ms, weak_pages 0.003 ms
- ('messages-fanout', 8, 'diagnostic-allocations'): allocation_census 20.353 ms, sweep_finalize 16.623 ms, roots 0.149 ms, stop_world 0.145 ms, trace 0.099 ms, prepare 0.037 ms, finish 0.026 ms, accounting 0.021 ms, resume_world 0.006 ms, trim 0.004 ms, weak_pages 0.003 ms, verify 0.002 ms
- ('messages-retained', 1, 'diagnostic'): sweep_finalize 14.570 ms, trace 0.086 ms, roots 0.058 ms, resume_world 0.035 ms, prepare 0.028 ms, finish 0.014 ms, accounting 0.010 ms, allocation_census 0.003 ms, trim 0.002 ms, stop_world 0.002 ms, verify 0.001 ms, weak_pages 0.001 ms
- ('messages-retained', 1, 'diagnostic-allocations'): sweep_finalize 14.082 ms, allocation_census 11.166 ms, trace 0.092 ms, roots 0.057 ms, resume_world 0.040 ms, prepare 0.028 ms, finish 0.018 ms, accounting 0.011 ms, trim 0.002 ms, verify 0.002 ms, weak_pages 0.002 ms, stop_world 0.002 ms
- ('messages-retained', 2, 'diagnostic'): sweep_finalize 17.993 ms, stop_world 0.085 ms, trace 0.084 ms, roots 0.053 ms, prepare 0.028 ms, finish 0.019 ms, accounting 0.013 ms, resume_world 0.005 ms, allocation_census 0.004 ms, trim 0.003 ms, verify 0.002 ms, weak_pages 0.001 ms
- ('messages-retained', 2, 'diagnostic-allocations'): sweep_finalize 17.060 ms, allocation_census 11.400 ms, trace 0.082 ms, roots 0.054 ms, prepare 0.040 ms, finish 0.017 ms, accounting 0.014 ms, stop_world 0.012 ms, resume_world 0.006 ms, trim 0.002 ms, verify 0.002 ms, weak_pages 0.001 ms
- ('messages-retained', 4, 'diagnostic'): sweep_finalize 14.828 ms, trace 0.082 ms, roots 0.053 ms, prepare 0.026 ms, finish 0.021 ms, accounting 0.014 ms, stop_world 0.010 ms, resume_world 0.006 ms, allocation_census 0.004 ms, trim 0.002 ms, weak_pages 0.002 ms, verify 0.002 ms
- ('messages-retained', 4, 'diagnostic-allocations'): sweep_finalize 14.285 ms, allocation_census 11.750 ms, trace 0.128 ms, roots 0.057 ms, prepare 0.038 ms, finish 0.019 ms, accounting 0.013 ms, stop_world 0.012 ms, resume_world 0.005 ms, trim 0.002 ms, verify 0.002 ms, weak_pages 0.001 ms
- ('messages-retained', 8, 'diagnostic'): sweep_finalize 14.650 ms, trace 0.130 ms, roots 0.078 ms, prepare 0.038 ms, finish 0.017 ms, accounting 0.012 ms, stop_world 0.012 ms, allocation_census 0.007 ms, resume_world 0.005 ms, verify 0.002 ms, weak_pages 0.002 ms, trim 0.002 ms
- ('messages-retained', 8, 'diagnostic-allocations'): sweep_finalize 14.777 ms, allocation_census 12.399 ms, trace 0.108 ms, roots 0.056 ms, prepare 0.034 ms, finish 0.017 ms, accounting 0.013 ms, stop_world 0.011 ms, resume_world 0.005 ms, trim 0.002 ms, verify 0.002 ms, weak_pages 0.001 ms
- ('messages-slow', 1, 'diagnostic'): sweep_finalize 15.281 ms, roots 0.056 ms, resume_world 0.041 ms, prepare 0.019 ms, trace 0.016 ms, finish 0.016 ms, accounting 0.009 ms, allocation_census 0.003 ms, trim 0.003 ms, stop_world 0.002 ms, verify 0.001 ms, weak_pages 0.001 ms
- ('messages-slow', 1, 'diagnostic-allocations'): sweep_finalize 16.033 ms, allocation_census 11.794 ms, roots 0.060 ms, resume_world 0.054 ms, prepare 0.025 ms, finish 0.018 ms, trace 0.016 ms, accounting 0.012 ms, trim 0.003 ms, stop_world 0.002 ms, verify 0.002 ms, weak_pages 0.002 ms
- ('messages-slow', 2, 'diagnostic'): sweep_finalize 14.724 ms, roots 0.057 ms, prepare 0.024 ms, trace 0.018 ms, finish 0.017 ms, accounting 0.012 ms, stop_world 0.012 ms, allocation_census 0.004 ms, resume_world 0.004 ms, trim 0.002 ms, weak_pages 0.002 ms, verify 0.001 ms
- ('messages-slow', 2, 'diagnostic-allocations'): sweep_finalize 14.990 ms, allocation_census 12.544 ms, roots 0.057 ms, prepare 0.019 ms, trace 0.018 ms, finish 0.018 ms, accounting 0.014 ms, stop_world 0.010 ms, resume_world 0.005 ms, trim 0.002 ms, verify 0.001 ms, weak_pages 0.001 ms
- ('messages-slow', 4, 'diagnostic'): sweep_finalize 11.866 ms, roots 0.050 ms, prepare 0.028 ms, trace 0.016 ms, finish 0.014 ms, accounting 0.011 ms, stop_world 0.010 ms, allocation_census 0.003 ms, resume_world 0.003 ms, trim 0.002 ms, weak_pages 0.001 ms, verify 0.001 ms
- ('messages-slow', 4, 'diagnostic-allocations'): allocation_census 11.361 ms, sweep_finalize 10.855 ms, roots 0.050 ms, prepare 0.021 ms, trace 0.016 ms, finish 0.015 ms, accounting 0.013 ms, stop_world 0.009 ms, resume_world 0.003 ms, trim 0.002 ms, verify 0.002 ms, weak_pages 0.001 ms
- ('messages-slow', 8, 'diagnostic'): sweep_finalize 11.307 ms, roots 0.048 ms, prepare 0.024 ms, trace 0.016 ms, finish 0.014 ms, accounting 0.011 ms, stop_world 0.010 ms, allocation_census 0.009 ms, resume_world 0.003 ms, trim 0.002 ms, verify 0.002 ms, weak_pages 0.001 ms
- ('messages-slow', 8, 'diagnostic-allocations'): allocation_census 11.018 ms, sweep_finalize 9.923 ms, stop_world 0.121 ms, roots 0.049 ms, prepare 0.018 ms, trace 0.016 ms, finish 0.014 ms, accounting 0.011 ms, resume_world 0.003 ms, trim 0.003 ms, verify 0.002 ms, weak_pages 0.001 ms
- ('snapshots', 1, 'diagnostic'): sweep_finalize 25.199 ms, trace 2.170 ms, roots 0.056 ms, prepare 0.035 ms, resume_world 0.032 ms, finish 0.017 ms, accounting 0.011 ms, allocation_census 0.006 ms, trim 0.003 ms, weak_pages 0.002 ms, stop_world 0.002 ms, verify 0.002 ms
- ('snapshots', 1, 'diagnostic-allocations'): sweep_finalize 23.536 ms, allocation_census 11.244 ms, trace 2.545 ms, roots 0.051 ms, resume_world 0.032 ms, prepare 0.032 ms, finish 0.021 ms, accounting 0.012 ms, weak_pages 0.002 ms, stop_world 0.002 ms, trim 0.002 ms, verify 0.002 ms
- ('snapshots', 2, 'diagnostic'): sweep_finalize 25.586 ms, trace 2.390 ms, roots 0.053 ms, prepare 0.028 ms, finish 0.015 ms, accounting 0.011 ms, stop_world 0.011 ms, allocation_census 0.006 ms, resume_world 0.006 ms, trim 0.003 ms, weak_pages 0.002 ms, verify 0.002 ms
- ('snapshots', 2, 'diagnostic-allocations'): sweep_finalize 24.851 ms, allocation_census 11.337 ms, trace 2.946 ms, roots 0.049 ms, prepare 0.029 ms, finish 0.023 ms, accounting 0.015 ms, stop_world 0.012 ms, resume_world 0.009 ms, trim 0.003 ms, weak_pages 0.002 ms, verify 0.002 ms
- ('snapshots', 4, 'diagnostic'): sweep_finalize 25.854 ms, trace 2.262 ms, roots 0.048 ms, prepare 0.043 ms, finish 0.019 ms, stop_world 0.012 ms, accounting 0.011 ms, resume_world 0.008 ms, allocation_census 0.006 ms, trim 0.003 ms, weak_pages 0.002 ms, verify 0.002 ms
- ('snapshots', 4, 'diagnostic-allocations'): sweep_finalize 23.288 ms, allocation_census 11.229 ms, trace 2.500 ms, roots 0.050 ms, prepare 0.027 ms, finish 0.020 ms, accounting 0.014 ms, stop_world 0.013 ms, resume_world 0.007 ms, trim 0.003 ms, weak_pages 0.003 ms, verify 0.002 ms
- ('snapshots', 8, 'diagnostic'): sweep_finalize 27.888 ms, trace 2.582 ms, roots 0.054 ms, prepare 0.029 ms, finish 0.019 ms, accounting 0.012 ms, stop_world 0.011 ms, allocation_census 0.006 ms, resume_world 0.006 ms, trim 0.003 ms, verify 0.002 ms, weak_pages 0.002 ms
- ('snapshots', 8, 'diagnostic-allocations'): sweep_finalize 20.793 ms, allocation_census 11.129 ms, trace 2.141 ms, roots 0.050 ms, prepare 0.029 ms, finish 0.019 ms, accounting 0.012 ms, stop_world 0.011 ms, resume_world 0.007 ms, trim 0.003 ms, weak_pages 0.002 ms, verify 0.002 ms
- ('words-transient', 1, 'diagnostic'): sweep_finalize 23.544 ms, roots 0.095 ms, trace 0.064 ms, prepare 0.062 ms, resume_world 0.033 ms, finish 0.027 ms, accounting 0.017 ms, allocation_census 0.010 ms, stop_world 0.005 ms, trim 0.005 ms, verify 0.004 ms, weak_pages 0.003 ms
- ('words-transient', 1, 'diagnostic-allocations'): sweep_finalize 22.920 ms, allocation_census 1.353 ms, roots 0.091 ms, trace 0.067 ms, prepare 0.061 ms, finish 0.039 ms, resume_world 0.028 ms, accounting 0.018 ms, trim 0.005 ms, stop_world 0.005 ms, verify 0.004 ms, weak_pages 0.003 ms
- ('words-transient', 2, 'diagnostic'): sweep_finalize 27.975 ms, roots 0.094 ms, prepare 0.060 ms, trace 0.059 ms, finish 0.036 ms, accounting 0.026 ms, stop_world 0.022 ms, resume_world 0.014 ms, allocation_census 0.010 ms, trim 0.005 ms, verify 0.004 ms, weak_pages 0.003 ms
- ('words-transient', 2, 'diagnostic-allocations'): sweep_finalize 26.391 ms, allocation_census 1.388 ms, roots 0.090 ms, prepare 0.064 ms, trace 0.057 ms, finish 0.029 ms, stop_world 0.021 ms, accounting 0.019 ms, resume_world 0.010 ms, trim 0.005 ms, verify 0.004 ms, weak_pages 0.003 ms
- ('words-transient', 4, 'diagnostic'): sweep_finalize 25.364 ms, roots 0.088 ms, trace 0.059 ms, prepare 0.057 ms, finish 0.031 ms, stop_world 0.026 ms, accounting 0.019 ms, allocation_census 0.011 ms, resume_world 0.008 ms, trim 0.005 ms, verify 0.004 ms, weak_pages 0.004 ms
- ('words-transient', 4, 'diagnostic-allocations'): sweep_finalize 24.060 ms, allocation_census 1.305 ms, roots 0.083 ms, trace 0.056 ms, prepare 0.054 ms, finish 0.029 ms, stop_world 0.019 ms, accounting 0.018 ms, resume_world 0.008 ms, trim 0.005 ms, verify 0.004 ms, weak_pages 0.003 ms
- ('words-transient', 8, 'diagnostic'): sweep_finalize 25.813 ms, roots 0.101 ms, prepare 0.064 ms, trace 0.061 ms, finish 0.031 ms, stop_world 0.029 ms, accounting 0.020 ms, allocation_census 0.009 ms, resume_world 0.009 ms, trim 0.005 ms, verify 0.004 ms, weak_pages 0.003 ms
- ('words-transient', 8, 'diagnostic-allocations'): sweep_finalize 25.012 ms, allocation_census 1.349 ms, roots 0.087 ms, accounting 0.063 ms, prepare 0.058 ms, trace 0.057 ms, finish 0.032 ms, stop_world 0.019 ms, resume_world 0.011 ms, trim 0.006 ms, weak_pages 0.004 ms, verify 0.004 ms
