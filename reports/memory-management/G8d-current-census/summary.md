| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| buffers-capacity-31 | 1 | baseline | 17.743 [17.702, 18.016] | 0.579 | 44.7 |
| buffers-capacity-31 | 1 | candidate | 16.544 [16.222, 18.598] | 0.635 | 44.5 |
| buffers-capacity-31 | 8 | baseline | 17.770 [17.400, 17.971] | 0.594 | 45.1 |
| buffers-capacity-31 | 8 | candidate | 15.462 [15.360, 15.759] | 0.644 | 46.9 |
| buffers-capacity-32 | 1 | baseline | 18.813 [18.080, 21.654] | 0.647 | 42.4 |
| buffers-capacity-32 | 1 | candidate | 17.117 [16.213, 19.039] | 0.613 | 42.2 |
| buffers-capacity-32 | 8 | baseline | 18.050 [17.805, 21.068] | 0.557 | 43.1 |
| buffers-capacity-32 | 8 | candidate | 16.179 [15.885, 16.428] | 0.534 | 43.1 |
| buffers-capacity-33 | 1 | baseline | 26.703 [25.837, 26.803] | 1.739 | 41.1 |
| buffers-capacity-33 | 1 | candidate | 24.292 [24.186, 26.254] | 1.756 | 41.5 |
| buffers-capacity-33 | 8 | baseline | 29.190 [26.971, 30.203] | 1.725 | 41.4 |
| buffers-capacity-33 | 8 | candidate | 23.651 [23.213, 23.920] | 1.784 | 41.3 |
| buffers-capacity-grow | 1 | baseline | 46.943 [46.898, 47.560] | 1.734 | 39.8 |
| buffers-capacity-grow | 1 | candidate | 42.340 [42.118, 42.969] | 1.632 | 39.6 |
| buffers-capacity-grow | 8 | baseline | 46.058 [45.512, 47.398] | 1.714 | 40.8 |
| buffers-capacity-grow | 8 | candidate | 42.936 [42.577, 44.685] | 1.734 | 42.0 |
| buffers-capacity-pointers | 1 | baseline | 22.068 [21.255, 22.174] | 1.986 | 51.2 |
| buffers-capacity-pointers | 1 | candidate | 21.592 [21.573, 21.746] | 1.937 | 49.3 |
| buffers-capacity-pointers | 8 | baseline | 27.534 [27.293, 27.736] | 1.866 | 51.9 |
| buffers-capacity-pointers | 8 | candidate | 27.770 [27.299, 27.852] | 1.926 | 49.8 |
| buffers-grow | 1 | baseline | 31.933 [31.149, 32.040] | 1.920 | 41.8 |
| buffers-grow | 1 | candidate | 29.731 [29.451, 30.232] | 1.750 | 41.2 |
| buffers-grow | 8 | baseline | 31.467 [31.198, 31.812] | 1.807 | 42.0 |
| buffers-grow | 8 | candidate | 29.761 [29.559, 29.994] | 1.803 | 41.7 |
| buffers-pointers | 1 | baseline | 22.872 [22.861, 27.291] | 2.269 | 47.3 |
| buffers-pointers | 1 | candidate | 23.003 [22.470, 24.211] | 2.253 | 47.4 |
| buffers-pointers | 8 | baseline | 28.613 [28.486, 28.699] | 2.360 | 48.0 |
| buffers-pointers | 8 | candidate | 28.595 [28.281, 31.916] | 2.262 | 48.2 |
| bytes-257 | 1 | baseline | 116.883 [106.304, 121.072] | 1.848 | 43.2 |
| bytes-257 | 1 | candidate | 93.731 [88.012, 100.120] | 1.841 | 41.4 |
| bytes-257 | 8 | baseline | 110.621 [105.027, 112.860] | 1.812 | 41.8 |
| bytes-257 | 8 | candidate | 95.065 [91.926, 98.878] | 1.946 | 41.7 |
| bytes-transient | 1 | baseline | 109.414 [106.721, 109.991] | 0.600 | 42.3 |
| bytes-transient | 1 | candidate | 84.268 [83.945, 85.231] | 0.568 | 42.9 |
| bytes-transient | 8 | baseline | 105.977 [98.815, 109.878] | 0.571 | 43.3 |
| bytes-transient | 8 | candidate | 83.847 [78.745, 87.574] | 0.647 | 42.8 |
| records-window | 1 | baseline | 244.063 [243.742, 252.417] | 43.471 | 52.1 |
| records-window | 1 | candidate | 247.975 [244.357, 250.778] | 43.382 | 50.9 |
| records-window | 8 | baseline | 245.899 [245.375, 247.330] | 43.959 | 51.4 |
| records-window | 8 | candidate | 246.140 [244.765, 247.719] | 45.005 | 51.3 |
| stored | 1 | baseline | 0.643 [0.631, 0.646] | 0.000 | 27.0 |
| stored | 1 | candidate | 0.637 [0.625, 0.639] | 0.000 | 26.4 |
| stored | 8 | baseline | 0.655 [0.649, 0.899] | 0.000 | 27.6 |
| stored | 8 | candidate | 1.017 [0.704, 1.288] | 0.000 | 27.3 |
| words-transient | 1 | baseline | 113.465 [112.081, 115.569] | 1.403 | 41.5 |
| words-transient | 1 | candidate | 93.873 [92.742, 97.005] | 1.345 | 40.7 |
| words-transient | 8 | baseline | 115.005 [114.886, 401.193] | 1.404 | 41.4 |
| words-transient | 8 | candidate | 93.020 [90.239, 100.192] | 1.384 | 41.5 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| buffers-capacity-31 | 1 | -6.8% | +9.8% | -0.5% | 1 / 1 |
| buffers-capacity-31 | 8 | -13.0% | +8.5% | +3.9% | 1 / 1 |
| buffers-capacity-32 | 1 | -9.0% | -5.3% | -0.5% | 1 / 1 |
| buffers-capacity-32 | 8 | -10.4% | -4.1% | -0.0% | 1 / 1 |
| buffers-capacity-33 | 1 | -9.0% | +1.0% | +0.9% | 1 / 1 |
| buffers-capacity-33 | 8 | -19.0% | +3.4% | -0.2% | 1 / 1 |
| buffers-capacity-grow | 1 | -9.8% | -5.9% | -0.5% | 5 / 5 |
| buffers-capacity-grow | 8 | -6.8% | +1.2% | +3.0% | 5 / 5 |
| buffers-capacity-pointers | 1 | -2.2% | -2.4% | -3.8% | 1 / 1 |
| buffers-capacity-pointers | 8 | +0.9% | +3.2% | -4.0% | 1 / 1 |
| buffers-grow | 1 | -6.9% | -8.9% | -1.4% | 1 / 1 |
| buffers-grow | 8 | -5.4% | -0.2% | -0.6% | 1 / 1 |
| buffers-pointers | 1 | +0.6% | -0.7% | +0.3% | 1 / 1 |
| buffers-pointers | 8 | -0.1% | -4.1% | +0.5% | 1 / 1 |
| bytes-257 | 1 | -19.8% | -0.4% | -4.1% | 1 / 1 |
| bytes-257 | 8 | -14.1% | +7.4% | -0.4% | 1 / 1 |
| bytes-transient | 1 | -23.0% | -5.3% | +1.4% | 1 / 1 |
| bytes-transient | 8 | -20.9% | +13.2% | -1.2% | 1 / 1 |
| records-window | 1 | +1.6% | -0.2% | -2.3% | 16 / 16 |
| records-window | 8 | +0.1% | +2.4% | -0.3% | 16 / 16 |
| stored | 1 | -1.0% | n/a | -2.1% | 0 / 0 |
| stored | 8 | +55.2% | n/a | -1.3% | 0 / 0 |
| words-transient | 1 | -17.3% | -4.2% | -2.0% | 8 / 8 |
| words-transient | 8 | -19.1% | -1.5% | +0.3% | 8 / 8 |

Diagnostic GC phases summed across measured repetitions (wall time):

- ('buffers-capacity-31', 1, 'baseline'): allocation_census 1.091 ms, sweep_finalize 0.524 ms, roots 0.023 ms, trace 0.017 ms, prepare 0.014 ms, resume_world 0.010 ms, finish 0.002 ms, accounting 0.001 ms, stop_world 0.001 ms, trim 0.001 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('buffers-capacity-31', 1, 'candidate'): allocation_census 1.234 ms, sweep_finalize 0.551 ms, roots 0.026 ms, trace 0.019 ms, prepare 0.018 ms, resume_world 0.012 ms, finish 0.003 ms, accounting 0.002 ms, stop_world 0.001 ms, weak_pages 0.001 ms, trim 0.000 ms, verify 0.000 ms
- ('buffers-capacity-31', 8, 'baseline'): allocation_census 1.097 ms, sweep_finalize 0.529 ms, roots 0.020 ms, trace 0.019 ms, prepare 0.014 ms, stop_world 0.006 ms, finish 0.003 ms, accounting 0.001 ms, resume_world 0.001 ms, trim 0.001 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('buffers-capacity-31', 8, 'candidate'): allocation_census 1.223 ms, sweep_finalize 0.569 ms, trace 0.024 ms, roots 0.020 ms, prepare 0.014 ms, stop_world 0.006 ms, finish 0.003 ms, accounting 0.002 ms, resume_world 0.001 ms, weak_pages 0.001 ms, verify 0.000 ms, trim 0.000 ms
- ('buffers-capacity-32', 1, 'baseline'): allocation_census 1.220 ms, sweep_finalize 0.577 ms, roots 0.023 ms, trace 0.020 ms, prepare 0.017 ms, resume_world 0.010 ms, finish 0.003 ms, accounting 0.001 ms, trim 0.001 ms, stop_world 0.001 ms, verify 0.001 ms, weak_pages 0.001 ms
- ('buffers-capacity-32', 1, 'candidate'): allocation_census 1.220 ms, sweep_finalize 0.672 ms, roots 0.027 ms, trace 0.019 ms, prepare 0.019 ms, resume_world 0.011 ms, finish 0.003 ms, accounting 0.002 ms, stop_world 0.001 ms, trim 0.001 ms, verify 0.000 ms, weak_pages 0.000 ms
- ('buffers-capacity-32', 8, 'baseline'): allocation_census 1.077 ms, sweep_finalize 0.492 ms, trace 0.024 ms, roots 0.024 ms, prepare 0.014 ms, stop_world 0.008 ms, finish 0.003 ms, accounting 0.001 ms, trim 0.001 ms, resume_world 0.001 ms, weak_pages 0.001 ms, verify 0.000 ms
- ('buffers-capacity-32', 8, 'candidate'): allocation_census 1.058 ms, sweep_finalize 0.499 ms, roots 0.026 ms, trace 0.021 ms, prepare 0.014 ms, stop_world 0.009 ms, finish 0.002 ms, accounting 0.001 ms, resume_world 0.001 ms, trim 0.001 ms, weak_pages 0.001 ms, verify 0.000 ms
- ('buffers-capacity-33', 1, 'baseline'): sweep_finalize 4.453 ms, allocation_census 0.582 ms, prepare 0.027 ms, roots 0.023 ms, trace 0.019 ms, resume_world 0.013 ms, finish 0.003 ms, accounting 0.002 ms, stop_world 0.001 ms, trim 0.001 ms, weak_pages 0.001 ms, verify 0.000 ms
- ('buffers-capacity-33', 1, 'candidate'): sweep_finalize 4.424 ms, allocation_census 0.612 ms, roots 0.024 ms, prepare 0.022 ms, trace 0.018 ms, resume_world 0.013 ms, finish 0.003 ms, accounting 0.002 ms, stop_world 0.001 ms, trim 0.001 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('buffers-capacity-33', 8, 'baseline'): sweep_finalize 4.426 ms, allocation_census 0.582 ms, trace 0.028 ms, prepare 0.015 ms, roots 0.015 ms, stop_world 0.006 ms, finish 0.003 ms, accounting 0.002 ms, resume_world 0.001 ms, trim 0.001 ms, weak_pages 0.001 ms, verify 0.000 ms
- ('buffers-capacity-33', 8, 'candidate'): sweep_finalize 4.594 ms, allocation_census 0.628 ms, trace 0.029 ms, roots 0.028 ms, prepare 0.023 ms, stop_world 0.009 ms, finish 0.003 ms, accounting 0.002 ms, resume_world 0.001 ms, trim 0.001 ms, verify 0.000 ms, weak_pages 0.000 ms
- ('buffers-capacity-grow', 1, 'baseline'): sweep_finalize 4.109 ms, allocation_census 1.031 ms, trace 0.052 ms, roots 0.047 ms, prepare 0.034 ms, finish 0.010 ms, resume_world 0.010 ms, accounting 0.006 ms, stop_world 0.002 ms, trim 0.002 ms, weak_pages 0.002 ms, verify 0.002 ms
- ('buffers-capacity-grow', 1, 'candidate'): sweep_finalize 3.818 ms, allocation_census 0.922 ms, trace 0.049 ms, roots 0.046 ms, prepare 0.032 ms, resume_world 0.011 ms, finish 0.010 ms, accounting 0.007 ms, trim 0.002 ms, stop_world 0.002 ms, verify 0.001 ms, weak_pages 0.001 ms
- ('buffers-capacity-grow', 8, 'baseline'): sweep_finalize 3.891 ms, allocation_census 0.923 ms, trace 0.050 ms, roots 0.042 ms, prepare 0.031 ms, finish 0.011 ms, stop_world 0.011 ms, accounting 0.007 ms, resume_world 0.002 ms, verify 0.002 ms, weak_pages 0.002 ms, trim 0.002 ms
- ('buffers-capacity-grow', 8, 'candidate'): sweep_finalize 3.887 ms, allocation_census 0.906 ms, trace 0.048 ms, roots 0.042 ms, prepare 0.028 ms, finish 0.012 ms, stop_world 0.011 ms, accounting 0.006 ms, resume_world 0.003 ms, trim 0.003 ms, verify 0.002 ms, weak_pages 0.001 ms
- ('buffers-capacity-pointers', 1, 'baseline'): allocation_census 3.380 ms, sweep_finalize 2.330 ms, trace 0.038 ms, resume_world 0.027 ms, roots 0.024 ms, prepare 0.018 ms, finish 0.004 ms, accounting 0.002 ms, stop_world 0.001 ms, trim 0.001 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('buffers-capacity-pointers', 1, 'candidate'): allocation_census 3.353 ms, sweep_finalize 2.312 ms, trace 0.037 ms, resume_world 0.029 ms, roots 0.026 ms, prepare 0.021 ms, finish 0.004 ms, accounting 0.002 ms, stop_world 0.001 ms, trim 0.001 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('buffers-capacity-pointers', 8, 'baseline'): allocation_census 3.213 ms, sweep_finalize 2.253 ms, trace 0.039 ms, prepare 0.017 ms, roots 0.016 ms, stop_world 0.006 ms, finish 0.004 ms, accounting 0.002 ms, resume_world 0.002 ms, trim 0.001 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('buffers-capacity-pointers', 8, 'candidate'): allocation_census 3.292 ms, sweep_finalize 2.460 ms, trace 0.038 ms, prepare 0.019 ms, roots 0.019 ms, stop_world 0.009 ms, finish 0.004 ms, accounting 0.002 ms, resume_world 0.002 ms, trim 0.001 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('buffers-grow', 1, 'baseline'): sweep_finalize 5.486 ms, allocation_census 0.650 ms, roots 0.024 ms, trace 0.022 ms, prepare 0.017 ms, resume_world 0.013 ms, finish 0.004 ms, accounting 0.002 ms, stop_world 0.001 ms, trim 0.001 ms, weak_pages 0.001 ms, verify 0.000 ms
- ('buffers-grow', 1, 'candidate'): sweep_finalize 4.552 ms, allocation_census 0.608 ms, roots 0.023 ms, trace 0.018 ms, prepare 0.015 ms, resume_world 0.011 ms, finish 0.003 ms, accounting 0.002 ms, stop_world 0.001 ms, trim 0.001 ms, weak_pages 0.001 ms, verify 0.001 ms
- ('buffers-grow', 8, 'baseline'): sweep_finalize 4.638 ms, allocation_census 0.619 ms, roots 0.015 ms, trace 0.015 ms, prepare 0.014 ms, stop_world 0.005 ms, finish 0.003 ms, accounting 0.001 ms, trim 0.001 ms, resume_world 0.001 ms, verify 0.000 ms, weak_pages 0.000 ms
- ('buffers-grow', 8, 'candidate'): sweep_finalize 4.629 ms, allocation_census 0.629 ms, trace 0.015 ms, prepare 0.015 ms, roots 0.015 ms, stop_world 0.007 ms, finish 0.003 ms, accounting 0.001 ms, trim 0.001 ms, resume_world 0.001 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('buffers-pointers', 1, 'baseline'): sweep_finalize 3.516 ms, allocation_census 3.131 ms, trace 0.032 ms, resume_world 0.026 ms, roots 0.025 ms, prepare 0.018 ms, finish 0.004 ms, accounting 0.003 ms, stop_world 0.001 ms, trim 0.001 ms, weak_pages 0.001 ms, verify 0.001 ms
- ('buffers-pointers', 1, 'candidate'): sweep_finalize 3.706 ms, allocation_census 3.255 ms, roots 0.034 ms, trace 0.033 ms, resume_world 0.028 ms, prepare 0.021 ms, finish 0.005 ms, accounting 0.003 ms, stop_world 0.001 ms, verify 0.001 ms, trim 0.001 ms, weak_pages 0.001 ms
- ('buffers-pointers', 8, 'baseline'): sweep_finalize 3.649 ms, allocation_census 3.231 ms, trace 0.036 ms, roots 0.016 ms, prepare 0.016 ms, stop_world 0.006 ms, finish 0.004 ms, accounting 0.003 ms, resume_world 0.002 ms, verify 0.001 ms, trim 0.001 ms, weak_pages 0.001 ms
- ('buffers-pointers', 8, 'candidate'): sweep_finalize 3.511 ms, allocation_census 3.131 ms, trace 0.034 ms, roots 0.015 ms, prepare 0.014 ms, stop_world 0.006 ms, finish 0.004 ms, accounting 0.002 ms, resume_world 0.001 ms, trim 0.001 ms, verify 0.001 ms, weak_pages 0.000 ms
- ('bytes-257', 1, 'baseline'): sweep_finalize 4.810 ms, allocation_census 0.636 ms, roots 0.028 ms, trace 0.018 ms, prepare 0.017 ms, resume_world 0.015 ms, finish 0.003 ms, accounting 0.002 ms, stop_world 0.001 ms, trim 0.000 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('bytes-257', 1, 'candidate'): sweep_finalize 4.675 ms, allocation_census 0.637 ms, roots 0.031 ms, trace 0.017 ms, prepare 0.017 ms, resume_world 0.012 ms, finish 0.003 ms, accounting 0.002 ms, stop_world 0.001 ms, trim 0.001 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('bytes-257', 8, 'baseline'): sweep_finalize 4.714 ms, allocation_census 0.631 ms, trace 0.017 ms, roots 0.016 ms, prepare 0.016 ms, stop_world 0.006 ms, finish 0.003 ms, accounting 0.002 ms, resume_world 0.001 ms, weak_pages 0.001 ms, trim 0.000 ms, verify 0.000 ms
- ('bytes-257', 8, 'candidate'): sweep_finalize 4.861 ms, allocation_census 0.697 ms, roots 0.022 ms, trace 0.019 ms, prepare 0.016 ms, stop_world 0.007 ms, finish 0.003 ms, accounting 0.002 ms, resume_world 0.001 ms, trim 0.001 ms, weak_pages 0.001 ms, verify 0.001 ms
- ('bytes-transient', 1, 'baseline'): allocation_census 1.159 ms, sweep_finalize 0.514 ms, roots 0.024 ms, prepare 0.017 ms, trace 0.014 ms, resume_world 0.011 ms, finish 0.003 ms, accounting 0.001 ms, stop_world 0.001 ms, verify 0.001 ms, trim 0.001 ms, weak_pages 0.000 ms
- ('bytes-transient', 1, 'candidate'): allocation_census 1.080 ms, sweep_finalize 0.509 ms, roots 0.026 ms, prepare 0.015 ms, trace 0.013 ms, resume_world 0.010 ms, finish 0.003 ms, accounting 0.001 ms, stop_world 0.001 ms, trim 0.001 ms, weak_pages 0.001 ms, verify 0.000 ms
- ('bytes-transient', 8, 'baseline'): allocation_census 1.098 ms, sweep_finalize 0.566 ms, roots 0.016 ms, prepare 0.016 ms, trace 0.015 ms, stop_world 0.006 ms, finish 0.003 ms, accounting 0.001 ms, resume_world 0.001 ms, trim 0.001 ms, weak_pages 0.000 ms, verify 0.000 ms
- ('bytes-transient', 8, 'candidate'): allocation_census 1.259 ms, sweep_finalize 0.551 ms, trace 0.021 ms, roots 0.018 ms, prepare 0.015 ms, stop_world 0.007 ms, finish 0.004 ms, resume_world 0.003 ms, accounting 0.002 ms, weak_pages 0.001 ms, verify 0.001 ms, trim 0.001 ms
- ('records-window', 1, 'baseline'): allocation_census 63.435 ms, sweep_finalize 58.675 ms, trim 4.062 ms, prepare 2.407 ms, weak_pages 1.956 ms, trace 1.151 ms, roots 0.145 ms, finish 0.052 ms, resume_world 0.047 ms, accounting 0.037 ms, verify 0.009 ms, stop_world 0.008 ms
- ('records-window', 1, 'candidate'): allocation_census 62.989 ms, sweep_finalize 57.029 ms, trim 4.093 ms, prepare 2.574 ms, weak_pages 2.016 ms, trace 1.154 ms, roots 0.157 ms, finish 0.054 ms, resume_world 0.043 ms, accounting 0.033 ms, stop_world 0.009 ms, verify 0.007 ms
- ('records-window', 8, 'baseline'): allocation_census 64.160 ms, sweep_finalize 57.694 ms, trim 4.297 ms, prepare 2.511 ms, weak_pages 2.070 ms, trace 1.163 ms, roots 0.128 ms, finish 0.058 ms, accounting 0.035 ms, stop_world 0.032 ms, resume_world 0.016 ms, verify 0.010 ms
- ('records-window', 8, 'candidate'): allocation_census 67.111 ms, sweep_finalize 57.245 ms, trim 4.918 ms, prepare 2.788 ms, weak_pages 1.917 ms, trace 1.223 ms, roots 0.144 ms, finish 0.056 ms, accounting 0.035 ms, stop_world 0.032 ms, resume_world 0.017 ms, verify 0.008 ms
- ('words-transient', 1, 'baseline'): sweep_finalize 3.051 ms, allocation_census 0.764 ms, trace 0.064 ms, roots 0.062 ms, prepare 0.049 ms, finish 0.016 ms, resume_world 0.015 ms, accounting 0.011 ms, stop_world 0.003 ms, trim 0.003 ms, weak_pages 0.002 ms, verify 0.002 ms
- ('words-transient', 1, 'candidate'): sweep_finalize 3.023 ms, allocation_census 0.682 ms, roots 0.064 ms, trace 0.059 ms, prepare 0.045 ms, resume_world 0.016 ms, finish 0.014 ms, accounting 0.009 ms, stop_world 0.003 ms, trim 0.003 ms, verify 0.002 ms, weak_pages 0.002 ms
- ('words-transient', 8, 'baseline'): sweep_finalize 3.161 ms, allocation_census 0.741 ms, roots 0.072 ms, trace 0.071 ms, prepare 0.054 ms, stop_world 0.016 ms, finish 0.016 ms, accounting 0.011 ms, trim 0.004 ms, verify 0.004 ms, resume_world 0.003 ms, weak_pages 0.003 ms
- ('words-transient', 8, 'candidate'): sweep_finalize 3.098 ms, allocation_census 0.686 ms, roots 0.069 ms, trace 0.065 ms, prepare 0.047 ms, finish 0.017 ms, stop_world 0.017 ms, accounting 0.010 ms, resume_world 0.004 ms, trim 0.003 ms, weak_pages 0.002 ms, verify 0.002 ms

Trace cardinality: medians of workload totals per fresh process.
Candidates include duplicate, old and immortal pointers; trace counts include remembered parents.
Timing is instrumented wall time, not per-object latency. Separate medians need not conserve counts.

| Case | Workers | Variant | Collections | Candidates | Newly marked | Marked leaves | Traced | Remembered traces | Trace ms |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|
| buffers-capacity-31 | 1 | baseline | 1 | 153 | 151 | 9 | 142 | 0 | 0.005 |
| buffers-capacity-31 | 1 | candidate | 1 | 153 | 151 | 9 | 142 | 0 | 0.006 |
| buffers-capacity-31 | 8 | baseline | 1 | 153 | 151 | 9 | 142 | 0 | 0.006 |
| buffers-capacity-31 | 8 | candidate | 1 | 153 | 151 | 9 | 142 | 0 | 0.007 |
| buffers-capacity-32 | 1 | baseline | 1 | 153 | 151 | 9 | 142 | 0 | 0.006 |
| buffers-capacity-32 | 1 | candidate | 1 | 153 | 151 | 9 | 142 | 0 | 0.006 |
| buffers-capacity-32 | 8 | baseline | 1 | 153 | 151 | 9 | 142 | 0 | 0.006 |
| buffers-capacity-32 | 8 | candidate | 1 | 153 | 151 | 9 | 142 | 0 | 0.007 |
| buffers-capacity-33 | 1 | baseline | 1 | 153 | 151 | 9 | 142 | 0 | 0.006 |
| buffers-capacity-33 | 1 | candidate | 1 | 153 | 151 | 9 | 142 | 0 | 0.006 |
| buffers-capacity-33 | 8 | baseline | 1 | 153 | 151 | 9 | 142 | 0 | 0.009 |
| buffers-capacity-33 | 8 | candidate | 1 | 153 | 151 | 9 | 142 | 0 | 0.010 |
| buffers-capacity-grow | 1 | baseline | 5 | 433 | 407 | 9 | 654 | 256 | 0.016 |
| buffers-capacity-grow | 1 | candidate | 5 | 433 | 407 | 9 | 654 | 256 | 0.017 |
| buffers-capacity-grow | 8 | baseline | 5 | 433 | 407 | 9 | 654 | 256 | 0.016 |
| buffers-capacity-grow | 8 | candidate | 5 | 433 | 407 | 9 | 654 | 256 | 0.016 |
| buffers-capacity-pointers | 1 | baseline | 1 | 1305 | 1303 | 1033 | 270 | 0 | 0.013 |
| buffers-capacity-pointers | 1 | candidate | 1 | 1305 | 1303 | 1033 | 270 | 0 | 0.012 |
| buffers-capacity-pointers | 8 | baseline | 1 | 1305 | 1303 | 1033 | 270 | 0 | 0.013 |
| buffers-capacity-pointers | 8 | candidate | 1 | 1305 | 1303 | 1033 | 270 | 0 | 0.013 |
| buffers-grow | 1 | baseline | 1 | 153 | 151 | 9 | 142 | 0 | 0.007 |
| buffers-grow | 1 | candidate | 1 | 153 | 151 | 9 | 142 | 0 | 0.007 |
| buffers-grow | 8 | baseline | 1 | 153 | 151 | 9 | 142 | 0 | 0.005 |
| buffers-grow | 8 | candidate | 1 | 153 | 151 | 9 | 142 | 0 | 0.005 |
| buffers-pointers | 1 | baseline | 1 | 1312 | 1309 | 1038 | 271 | 0 | 0.011 |
| buffers-pointers | 1 | candidate | 1 | 1312 | 1309 | 1038 | 271 | 0 | 0.011 |
| buffers-pointers | 8 | baseline | 1 | 1312 | 1309 | 1038 | 271 | 0 | 0.011 |
| buffers-pointers | 8 | candidate | 1 | 1312 | 1309 | 1038 | 271 | 0 | 0.011 |
| bytes-257 | 1 | baseline | 1 | 89 | 87 | 9 | 78 | 0 | 0.006 |
| bytes-257 | 1 | candidate | 1 | 89 | 87 | 9 | 78 | 0 | 0.006 |
| bytes-257 | 8 | baseline | 1 | 89 | 87 | 9 | 78 | 0 | 0.006 |
| bytes-257 | 8 | candidate | 1 | 89 | 87 | 9 | 78 | 0 | 0.006 |
| bytes-transient | 1 | baseline | 1 | 89 | 87 | 9 | 78 | 0 | 0.005 |
| bytes-transient | 1 | candidate | 1 | 89 | 87 | 9 | 78 | 0 | 0.004 |
| bytes-transient | 8 | baseline | 1 | 89 | 87 | 9 | 78 | 0 | 0.005 |
| bytes-transient | 8 | candidate | 1 | 89 | 87 | 9 | 78 | 0 | 0.005 |
| records-window | 1 | baseline | 16 | 51217 | 24198 | 20 | 24202 | 24 | 0.375 |
| records-window | 1 | candidate | 16 | 51217 | 24198 | 20 | 24202 | 24 | 0.384 |
| records-window | 8 | baseline | 16 | 51217 | 24198 | 20 | 24202 | 24 | 0.386 |
| records-window | 8 | candidate | 16 | 51217 | 24198 | 20 | 24202 | 24 | 0.405 |
| words-transient | 1 | baseline | 8 | 355 | 311 | 9 | 526 | 224 | 0.021 |
| words-transient | 1 | candidate | 8 | 355 | 311 | 9 | 526 | 224 | 0.019 |
| words-transient | 8 | baseline | 8 | 355 | 311 | 9 | 526 | 224 | 0.021 |
| words-transient | 8 | candidate | 8 | 355 | 311 | 9 | 526 | 224 | 0.022 |

Sweep work distribution (workload collections only):

Small helper jobs contain fewer than 4,096 objects; native thread creation is counted separately.
these counts alone do not measure thread-startup cost.

| Case | Workers | Variant | Collections / parallel | Helper jobs | New native threads | Small helper jobs | Median helper objects | Median largest-list share in parallel collections |
|---|---:|---|---:|---:|---:|---:|---:|---:|
| buffers-capacity-31 | 1 | baseline | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-31 | 1 | candidate | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-31 | 8 | baseline | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-31 | 8 | candidate | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-32 | 1 | baseline | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-32 | 1 | candidate | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-32 | 8 | baseline | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-32 | 8 | candidate | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-33 | 1 | baseline | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-33 | 1 | candidate | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-33 | 8 | baseline | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-33 | 8 | candidate | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-grow | 1 | baseline | 15 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-grow | 1 | candidate | 15 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-grow | 8 | baseline | 15 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-grow | 8 | candidate | 15 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-pointers | 1 | baseline | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-pointers | 1 | candidate | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-pointers | 8 | baseline | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-capacity-pointers | 8 | candidate | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-grow | 1 | baseline | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-grow | 1 | candidate | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-grow | 8 | baseline | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-grow | 8 | candidate | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-pointers | 1 | baseline | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-pointers | 1 | candidate | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-pointers | 8 | baseline | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| buffers-pointers | 8 | candidate | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| bytes-257 | 1 | baseline | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| bytes-257 | 1 | candidate | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| bytes-257 | 8 | baseline | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| bytes-257 | 8 | candidate | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| bytes-transient | 1 | baseline | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| bytes-transient | 1 | candidate | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| bytes-transient | 8 | baseline | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| bytes-transient | 8 | candidate | 3 / 0 | 0 | 0 | 0 | n/a | n/a |
| records-window | 1 | baseline | 48 / 0 | 0 | 0 | 0 | n/a | n/a |
| records-window | 1 | candidate | 48 / 0 | 0 | 0 | 0 | n/a | n/a |
| records-window | 8 | baseline | 48 / 0 | 0 | 0 | 0 | n/a | n/a |
| records-window | 8 | candidate | 48 / 0 | 0 | 0 | 0 | n/a | n/a |
| words-transient | 1 | baseline | 24 / 0 | 0 | 0 | 0 | n/a | n/a |
| words-transient | 1 | candidate | 24 / 0 | 0 | 0 | 0 | n/a | n/a |
| words-transient | 8 | baseline | 24 / 0 | 0 | 0 | 0 | n/a | n/a |
| words-transient | 8 | candidate | 24 / 0 | 0 | 0 | 0 | n/a | n/a |
