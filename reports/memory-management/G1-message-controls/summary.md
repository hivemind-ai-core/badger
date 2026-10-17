| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| messages | 1 | baseline | 27.643 [27.331, 28.825] | 4.530 | 49.0 |
| messages | 1 | diagnostic | 28.003 [26.608, 29.612] | 4.659 | 49.0 |
| messages | 1 | diagnostic-allocations | 57.328 [56.534, 57.479] | 8.227 | 57.2 |
| messages | 1 | diagnostic-off | 27.620 [27.306, 27.802] | 4.525 | 48.8 |
| messages | 2 | baseline | 28.927 [28.806, 31.242] | 4.618 | 53.9 |
| messages | 2 | diagnostic | 29.710 [28.266, 30.630] | 5.101 | 53.8 |
| messages | 2 | diagnostic-allocations | 64.035 [61.235, 66.698] | 10.091 | 84.6 |
| messages | 2 | diagnostic-off | 30.001 [29.710, 30.267] | 4.277 | 49.7 |
| messages | 4 | baseline | 30.536 [28.293, 33.143] | 4.866 | 60.2 |
| messages | 4 | diagnostic | 31.561 [29.810, 33.538] | 4.799 | 66.5 |
| messages | 4 | diagnostic-allocations | 67.264 [66.803, 71.510] | 9.745 | 87.6 |
| messages | 4 | diagnostic-off | 29.823 [29.536, 33.521] | 5.204 | 55.7 |
| messages | 8 | baseline | 33.522 [28.813, 33.761] | 5.627 | 68.3 |
| messages | 8 | diagnostic | 34.157 [34.139, 35.629] | 5.881 | 72.2 |
| messages | 8 | diagnostic-allocations | 70.120 [68.865, 70.219] | 11.049 | 100.2 |
| messages | 8 | diagnostic-off | 29.611 [28.476, 32.451] | 4.762 | 57.9 |
| messages-fanout | 1 | baseline | 42.745 [42.625, 43.059] | 7.552 | 48.9 |
| messages-fanout | 1 | diagnostic | 43.792 [43.586, 45.290] | 7.080 | 49.2 |
| messages-fanout | 1 | diagnostic-allocations | 91.906 [90.656, 92.258] | 15.110 | 56.9 |
| messages-fanout | 1 | diagnostic-off | 43.807 [41.248, 44.393] | 7.042 | 48.9 |
| messages-fanout | 2 | baseline | 46.467 [43.847, 47.947] | 7.552 | 54.8 |
| messages-fanout | 2 | diagnostic | 48.032 [43.836, 49.098] | 7.617 | 53.0 |
| messages-fanout | 2 | diagnostic-allocations | 86.320 [85.151, 87.865] | 13.860 | 60.2 |
| messages-fanout | 2 | diagnostic-off | 47.290 [46.775, 48.719] | 7.433 | 54.8 |
| messages-fanout | 4 | baseline | 36.911 [36.431, 36.997] | 6.165 | 62.8 |
| messages-fanout | 4 | diagnostic | 38.512 [36.532, 44.376] | 5.159 | 59.2 |
| messages-fanout | 4 | diagnostic-allocations | 80.750 [77.452, 83.510] | 13.065 | 92.4 |
| messages-fanout | 4 | diagnostic-off | 35.733 [35.280, 40.319] | 4.974 | 60.2 |
| messages-fanout | 8 | baseline | 38.181 [38.149, 39.435] | 4.632 | 69.1 |
| messages-fanout | 8 | diagnostic | 40.593 [38.661, 42.610] | 4.833 | 72.6 |
| messages-fanout | 8 | diagnostic-allocations | 84.433 [82.740, 85.666] | 11.683 | 108.8 |
| messages-fanout | 8 | diagnostic-off | 42.006 [40.458, 43.535] | 4.965 | 70.6 |
| messages-int | 1 | baseline | 5.338 [5.258, 5.359] | 0.000 | 29.3 |
| messages-int | 1 | diagnostic | 5.195 [5.159, 5.284] | 0.000 | 29.6 |
| messages-int | 1 | diagnostic-allocations | 5.681 [5.605, 5.831] | 0.000 | 30.1 |
| messages-int | 1 | diagnostic-off | 5.109 [5.105, 5.361] | 0.000 | 29.3 |
| messages-int | 2 | baseline | 5.357 [5.218, 10.713] | 0.000 | 29.7 |
| messages-int | 2 | diagnostic | 8.781 [7.618, 9.819] | 0.000 | 33.4 |
| messages-int | 2 | diagnostic-allocations | 9.460 [5.988, 10.073] | 0.000 | 33.7 |
| messages-int | 2 | diagnostic-off | 8.631 [5.433, 9.020] | 0.000 | 34.8 |
| messages-int | 4 | baseline | 9.023 [8.838, 10.993] | 0.000 | 33.1 |
| messages-int | 4 | diagnostic | 10.739 [5.905, 10.857] | 0.000 | 33.2 |
| messages-int | 4 | diagnostic-allocations | 9.506 [9.339, 9.526] | 0.000 | 33.7 |
| messages-int | 4 | diagnostic-off | 8.977 [8.783, 9.236] | 0.000 | 34.9 |
| messages-int | 8 | baseline | 10.071 [9.205, 11.332] | 0.000 | 32.8 |
| messages-int | 8 | diagnostic | 9.285 [5.791, 9.476] | 0.000 | 33.1 |
| messages-int | 8 | diagnostic-allocations | 10.515 [9.957, 10.732] | 0.000 | 33.7 |
| messages-int | 8 | diagnostic-off | 9.248 [9.174, 9.754] | 0.000 | 33.2 |
| messages-int-fanout | 1 | baseline | 6.664 [6.650, 6.755] | 0.000 | 29.4 |
| messages-int-fanout | 1 | diagnostic | 6.677 [6.336, 6.906] | 0.000 | 29.5 |
| messages-int-fanout | 1 | diagnostic-allocations | 7.025 [6.999, 7.173] | 0.000 | 29.7 |
| messages-int-fanout | 1 | diagnostic-off | 6.673 [6.295, 7.297] | 0.000 | 29.4 |
| messages-int-fanout | 2 | baseline | 9.680 [9.032, 9.840] | 0.000 | 30.5 |
| messages-int-fanout | 2 | diagnostic | 10.270 [9.653, 11.240] | 0.000 | 31.3 |
| messages-int-fanout | 2 | diagnostic-allocations | 11.051 [11.048, 12.135] | 0.000 | 32.8 |
| messages-int-fanout | 2 | diagnostic-off | 10.309 [10.032, 11.450] | 0.000 | 31.2 |
| messages-int-fanout | 4 | baseline | 14.360 [13.349, 14.648] | 0.000 | 29.8 |
| messages-int-fanout | 4 | diagnostic | 13.024 [12.119, 13.101] | 0.000 | 30.6 |
| messages-int-fanout | 4 | diagnostic-allocations | 14.896 [13.819, 15.424] | 0.000 | 30.1 |
| messages-int-fanout | 4 | diagnostic-off | 13.265 [12.820, 13.319] | 0.000 | 30.7 |
| messages-int-fanout | 8 | baseline | 12.282 [11.704, 12.809] | 0.000 | 30.9 |
| messages-int-fanout | 8 | diagnostic | 13.274 [12.766, 15.344] | 0.000 | 31.0 |
| messages-int-fanout | 8 | diagnostic-allocations | 13.451 [12.998, 14.250] | 0.000 | 31.5 |
| messages-int-fanout | 8 | diagnostic-off | 13.640 [12.498, 16.271] | 0.000 | 31.0 |
| messages-int-retained | 1 | baseline | 5.379 [5.354, 5.455] | 0.000 | 29.6 |
| messages-int-retained | 1 | diagnostic | 5.377 [5.221, 5.407] | 0.000 | 29.4 |
| messages-int-retained | 1 | diagnostic-allocations | 5.697 [5.681, 5.725] | 0.000 | 29.8 |
| messages-int-retained | 1 | diagnostic-off | 5.274 [5.170, 5.396] | 0.000 | 29.6 |
| messages-int-retained | 2 | baseline | 9.879 [9.060, 10.133] | 0.000 | 34.6 |
| messages-int-retained | 2 | diagnostic | 9.643 [9.167, 11.602] | 0.000 | 34.5 |
| messages-int-retained | 2 | diagnostic-allocations | 9.523 [9.240, 11.666] | 0.000 | 33.8 |
| messages-int-retained | 2 | diagnostic-off | 8.679 [8.676, 9.017] | 0.000 | 33.1 |
| messages-int-retained | 4 | baseline | 9.748 [9.076, 9.869] | 0.000 | 32.9 |
| messages-int-retained | 4 | diagnostic | 10.403 [6.888, 10.432] | 0.000 | 33.1 |
| messages-int-retained | 4 | diagnostic-allocations | 10.135 [9.939, 10.525] | 0.000 | 34.6 |
| messages-int-retained | 4 | diagnostic-off | 9.393 [8.995, 10.519] | 0.000 | 33.0 |
| messages-int-retained | 8 | baseline | 9.039 [8.770, 9.476] | 0.000 | 32.9 |
| messages-int-retained | 8 | diagnostic | 10.143 [9.285, 10.860] | 0.000 | 33.0 |
| messages-int-retained | 8 | diagnostic-allocations | 9.934 [9.871, 10.314] | 0.000 | 33.4 |
| messages-int-retained | 8 | diagnostic-off | 9.013 [8.652, 11.773] | 0.000 | 32.9 |
| messages-int-slow | 1 | baseline | 8.478 [8.403, 8.522] | 0.000 | 29.3 |
| messages-int-slow | 1 | diagnostic | 8.556 [7.916, 9.496] | 0.000 | 29.4 |
| messages-int-slow | 1 | diagnostic-allocations | 8.960 [8.819, 16.717] | 0.000 | 29.7 |
| messages-int-slow | 1 | diagnostic-off | 8.422 [8.019, 8.494] | 0.000 | 29.9 |
| messages-int-slow | 2 | baseline | 10.260 [9.607, 13.170] | 0.000 | 30.0 |
| messages-int-slow | 2 | diagnostic | 10.890 [10.661, 12.458] | 0.000 | 29.7 |
| messages-int-slow | 2 | diagnostic-allocations | 14.152 [10.526, 14.984] | 0.000 | 33.5 |
| messages-int-slow | 2 | diagnostic-off | 11.189 [9.470, 12.453] | 0.000 | 27.8 |
| messages-int-slow | 4 | baseline | 17.524 [16.899, 18.010] | 0.000 | 29.8 |
| messages-int-slow | 4 | diagnostic | 17.376 [17.301, 20.080] | 0.000 | 31.8 |
| messages-int-slow | 4 | diagnostic-allocations | 21.237 [19.532, 21.890] | 0.000 | 31.6 |
| messages-int-slow | 4 | diagnostic-off | 17.535 [17.041, 31.970] | 0.000 | 29.8 |
| messages-int-slow | 8 | baseline | 19.270 [18.950, 19.394] | 0.000 | 30.1 |
| messages-int-slow | 8 | diagnostic | 18.620 [17.772, 19.165] | 0.000 | 31.0 |
| messages-int-slow | 8 | diagnostic-allocations | 20.462 [19.665, 20.649] | 0.000 | 31.4 |
| messages-int-slow | 8 | diagnostic-off | 20.607 [19.298, 20.810] | 0.000 | 30.7 |
| messages-retained | 1 | baseline | 27.750 [27.573, 28.344] | 4.533 | 48.8 |
| messages-retained | 1 | diagnostic | 29.310 [27.775, 30.100] | 4.803 | 49.3 |
| messages-retained | 1 | diagnostic-allocations | 58.020 [56.655, 59.330] | 8.180 | 56.7 |
| messages-retained | 1 | diagnostic-off | 28.142 [27.631, 28.213] | 4.464 | 49.1 |
| messages-retained | 2 | baseline | 30.546 [29.684, 34.320] | 4.975 | 67.8 |
| messages-retained | 2 | diagnostic | 31.773 [31.007, 32.153] | 4.770 | 58.9 |
| messages-retained | 2 | diagnostic-allocations | 61.850 [60.314, 66.932] | 8.774 | 79.9 |
| messages-retained | 2 | diagnostic-off | 32.953 [31.687, 32.979] | 4.865 | 58.4 |
| messages-retained | 4 | baseline | 30.818 [28.582, 30.822] | 4.084 | 69.6 |
| messages-retained | 4 | diagnostic | 34.723 [30.614, 35.093] | 4.641 | 73.9 |
| messages-retained | 4 | diagnostic-allocations | 69.341 [66.136, 70.333] | 8.535 | 89.6 |
| messages-retained | 4 | diagnostic-off | 30.015 [29.596, 34.910] | 4.225 | 68.6 |
| messages-retained | 8 | baseline | 30.797 [30.256, 34.132] | 4.172 | 71.9 |
| messages-retained | 8 | diagnostic | 34.902 [34.101, 36.560] | 6.191 | 80.3 |
| messages-retained | 8 | diagnostic-allocations | 74.379 [69.670, 76.614] | 9.199 | 116.7 |
| messages-retained | 8 | diagnostic-off | 32.662 [31.990, 33.216] | 3.912 | 74.9 |
| messages-slow | 1 | baseline | 31.978 [31.820, 33.045] | 4.998 | 49.1 |
| messages-slow | 1 | diagnostic | 30.651 [30.390, 30.802] | 4.545 | 49.2 |
| messages-slow | 1 | diagnostic-allocations | 62.515 [62.224, 64.444] | 9.419 | 56.9 |
| messages-slow | 1 | diagnostic-off | 32.059 [30.824, 37.279] | 4.601 | 49.1 |
| messages-slow | 2 | baseline | 35.081 [34.896, 35.161] | 5.663 | 66.1 |
| messages-slow | 2 | diagnostic | 39.207 [35.967, 39.818] | 5.257 | 68.2 |
| messages-slow | 2 | diagnostic-allocations | 79.182 [72.489, 80.611] | 9.419 | 71.7 |
| messages-slow | 2 | diagnostic-off | 39.379 [38.715, 41.201] | 5.035 | 68.1 |
| messages-slow | 4 | baseline | 42.227 [41.244, 42.553] | 3.641 | 53.5 |
| messages-slow | 4 | diagnostic | 45.615 [41.046, 50.076] | 3.721 | 54.6 |
| messages-slow | 4 | diagnostic-allocations | 83.233 [82.404, 86.445] | 7.147 | 71.4 |
| messages-slow | 4 | diagnostic-off | 45.103 [43.742, 45.821] | 3.665 | 50.3 |
| messages-slow | 8 | baseline | 45.487 [41.199, 45.619] | 2.943 | 58.5 |
| messages-slow | 8 | diagnostic | 43.995 [41.947, 46.215] | 3.507 | 55.0 |
| messages-slow | 8 | diagnostic-allocations | 85.152 [84.469, 96.450] | 6.382 | 74.1 |
| messages-slow | 8 | diagnostic-off | 43.846 [40.188, 44.769] | 3.333 | 59.8 |

Diagnostic GC phases summed across measured repetitions (wall time):

- ('messages', 1, 'diagnostic'): sweep_finalize 13.762 ms, roots 0.054 ms, trace 0.049 ms, resume_world 0.019 ms, prepare 0.018 ms, finish 0.012 ms, accounting 0.008 ms, allocation_census 0.003 ms, verify 0.002 ms, trim 0.002 ms, stop_world 0.002 ms, weak_pages 0.001 ms
- ('messages', 1, 'diagnostic-allocations'): sweep_finalize 14.102 ms, allocation_census 11.035 ms, roots 0.057 ms, trace 0.055 ms, resume_world 0.030 ms, prepare 0.023 ms, finish 0.013 ms, accounting 0.008 ms, trim 0.002 ms, stop_world 0.002 ms, weak_pages 0.001 ms, verify 0.001 ms
- ('messages', 2, 'diagnostic'): sweep_finalize 14.617 ms, roots 0.050 ms, trace 0.035 ms, prepare 0.015 ms, finish 0.013 ms, stop_world 0.009 ms, accounting 0.009 ms, allocation_census 0.003 ms, resume_world 0.003 ms, trim 0.002 ms, verify 0.001 ms, weak_pages 0.001 ms
- ('messages', 2, 'diagnostic-allocations'): sweep_finalize 19.368 ms, allocation_census 13.624 ms, roots 0.052 ms, trace 0.042 ms, prepare 0.018 ms, finish 0.016 ms, accounting 0.012 ms, stop_world 0.009 ms, resume_world 0.004 ms, trim 0.002 ms, verify 0.002 ms, weak_pages 0.001 ms
- ('messages', 4, 'diagnostic'): sweep_finalize 14.419 ms, roots 0.046 ms, trace 0.036 ms, prepare 0.019 ms, finish 0.016 ms, stop_world 0.011 ms, accounting 0.011 ms, allocation_census 0.004 ms, resume_world 0.004 ms, trim 0.002 ms, verify 0.002 ms, weak_pages 0.001 ms
- ('messages', 4, 'diagnostic-allocations'): sweep_finalize 15.508 ms, allocation_census 12.735 ms, roots 0.056 ms, trace 0.044 ms, prepare 0.021 ms, finish 0.017 ms, accounting 0.012 ms, stop_world 0.011 ms, resume_world 0.004 ms, trim 0.002 ms, verify 0.002 ms, weak_pages 0.001 ms
- ('messages', 8, 'diagnostic'): sweep_finalize 17.011 ms, roots 0.074 ms, trace 0.068 ms, finish 0.052 ms, prepare 0.025 ms, accounting 0.011 ms, stop_world 0.011 ms, allocation_census 0.006 ms, resume_world 0.003 ms, verify 0.002 ms, trim 0.002 ms, weak_pages 0.002 ms
- ('messages', 8, 'diagnostic-allocations'): sweep_finalize 20.164 ms, allocation_census 11.189 ms, roots 0.048 ms, trace 0.045 ms, finish 0.019 ms, prepare 0.018 ms, accounting 0.012 ms, stop_world 0.010 ms, resume_world 0.004 ms, trim 0.002 ms, verify 0.002 ms, weak_pages 0.001 ms
- ('messages-fanout', 1, 'diagnostic'): sweep_finalize 21.365 ms, trace 0.083 ms, roots 0.061 ms, resume_world 0.032 ms, prepare 0.027 ms, finish 0.021 ms, accounting 0.013 ms, allocation_census 0.006 ms, trim 0.002 ms, stop_world 0.002 ms, verify 0.002 ms, weak_pages 0.001 ms
- ('messages-fanout', 1, 'diagnostic-allocations'): sweep_finalize 23.214 ms, allocation_census 19.989 ms, trace 0.087 ms, roots 0.071 ms, resume_world 0.037 ms, prepare 0.031 ms, finish 0.025 ms, accounting 0.015 ms, trim 0.003 ms, verify 0.003 ms, stop_world 0.003 ms, weak_pages 0.002 ms
- ('messages-fanout', 2, 'diagnostic'): sweep_finalize 22.785 ms, trace 0.089 ms, roots 0.066 ms, prepare 0.030 ms, finish 0.021 ms, accounting 0.017 ms, stop_world 0.015 ms, allocation_census 0.006 ms, resume_world 0.005 ms, trim 0.003 ms, verify 0.003 ms, weak_pages 0.003 ms
- ('messages-fanout', 2, 'diagnostic-allocations'): sweep_finalize 21.921 ms, allocation_census 19.027 ms, trace 0.123 ms, roots 0.073 ms, prepare 0.032 ms, finish 0.024 ms, accounting 0.020 ms, stop_world 0.013 ms, resume_world 0.006 ms, trim 0.003 ms, weak_pages 0.003 ms, verify 0.003 ms
- ('messages-fanout', 4, 'diagnostic'): sweep_finalize 15.905 ms, trace 0.108 ms, stop_world 0.074 ms, roots 0.072 ms, prepare 0.034 ms, finish 0.023 ms, accounting 0.017 ms, allocation_census 0.007 ms, resume_world 0.005 ms, trim 0.004 ms, verify 0.003 ms, weak_pages 0.003 ms
- ('messages-fanout', 4, 'diagnostic-allocations'): allocation_census 20.043 ms, sweep_finalize 19.120 ms, stop_world 0.182 ms, trace 0.089 ms, roots 0.071 ms, prepare 0.036 ms, finish 0.024 ms, accounting 0.020 ms, resume_world 0.006 ms, trim 0.003 ms, verify 0.003 ms, weak_pages 0.002 ms
- ('messages-fanout', 8, 'diagnostic'): sweep_finalize 14.513 ms, trace 0.082 ms, roots 0.066 ms, stop_world 0.057 ms, prepare 0.030 ms, finish 0.024 ms, accounting 0.016 ms, allocation_census 0.013 ms, resume_world 0.005 ms, trim 0.003 ms, verify 0.003 ms, weak_pages 0.002 ms
- ('messages-fanout', 8, 'diagnostic-allocations'): allocation_census 19.121 ms, sweep_finalize 16.375 ms, trace 0.085 ms, roots 0.072 ms, prepare 0.033 ms, finish 0.024 ms, accounting 0.018 ms, stop_world 0.011 ms, resume_world 0.004 ms, trim 0.004 ms, verify 0.003 ms, weak_pages 0.003 ms
- ('messages-retained', 1, 'diagnostic'): sweep_finalize 14.051 ms, trace 0.076 ms, roots 0.049 ms, resume_world 0.029 ms, prepare 0.024 ms, finish 0.014 ms, accounting 0.008 ms, allocation_census 0.003 ms, trim 0.002 ms, stop_world 0.002 ms, weak_pages 0.001 ms, verify 0.001 ms
- ('messages-retained', 1, 'diagnostic-allocations'): sweep_finalize 13.505 ms, allocation_census 11.030 ms, trace 0.103 ms, roots 0.055 ms, resume_world 0.033 ms, prepare 0.029 ms, finish 0.014 ms, accounting 0.008 ms, trim 0.002 ms, stop_world 0.002 ms, verify 0.001 ms, weak_pages 0.001 ms
- ('messages-retained', 2, 'diagnostic'): sweep_finalize 14.040 ms, trace 0.078 ms, stop_world 0.075 ms, roots 0.050 ms, prepare 0.024 ms, finish 0.015 ms, accounting 0.010 ms, resume_world 0.004 ms, allocation_census 0.004 ms, trim 0.002 ms, weak_pages 0.001 ms, verify 0.001 ms
- ('messages-retained', 2, 'diagnostic-allocations'): sweep_finalize 14.694 ms, allocation_census 11.238 ms, trace 0.099 ms, roots 0.049 ms, prepare 0.030 ms, finish 0.016 ms, accounting 0.012 ms, stop_world 0.011 ms, resume_world 0.005 ms, trim 0.002 ms, verify 0.001 ms, weak_pages 0.001 ms
- ('messages-retained', 4, 'diagnostic'): sweep_finalize 14.531 ms, trace 0.092 ms, roots 0.049 ms, prepare 0.034 ms, finish 0.016 ms, accounting 0.010 ms, stop_world 0.010 ms, resume_world 0.004 ms, allocation_census 0.004 ms, trim 0.002 ms, verify 0.002 ms, weak_pages 0.001 ms
- ('messages-retained', 4, 'diagnostic-allocations'): sweep_finalize 15.231 ms, allocation_census 11.752 ms, trace 0.102 ms, roots 0.059 ms, prepare 0.036 ms, finish 0.018 ms, accounting 0.013 ms, stop_world 0.011 ms, resume_world 0.005 ms, trim 0.002 ms, verify 0.002 ms, weak_pages 0.001 ms
- ('messages-retained', 8, 'diagnostic'): sweep_finalize 18.225 ms, trace 0.094 ms, roots 0.051 ms, prepare 0.029 ms, finish 0.021 ms, accounting 0.011 ms, stop_world 0.005 ms, allocation_census 0.005 ms, resume_world 0.004 ms, trim 0.002 ms, weak_pages 0.002 ms, verify 0.001 ms
- ('messages-retained', 8, 'diagnostic-allocations'): sweep_finalize 13.439 ms, allocation_census 12.912 ms, stop_world 0.654 ms, trace 0.109 ms, roots 0.054 ms, prepare 0.033 ms, finish 0.016 ms, accounting 0.011 ms, resume_world 0.004 ms, trim 0.002 ms, verify 0.002 ms, weak_pages 0.001 ms
- ('messages-slow', 1, 'diagnostic'): sweep_finalize 13.483 ms, roots 0.048 ms, resume_world 0.023 ms, prepare 0.016 ms, finish 0.013 ms, trace 0.013 ms, accounting 0.008 ms, allocation_census 0.002 ms, stop_world 0.002 ms, trim 0.002 ms, verify 0.002 ms, weak_pages 0.001 ms
- ('messages-slow', 1, 'diagnostic-allocations'): sweep_finalize 15.983 ms, allocation_census 11.733 ms, roots 0.058 ms, resume_world 0.033 ms, prepare 0.018 ms, trace 0.016 ms, finish 0.016 ms, accounting 0.010 ms, stop_world 0.002 ms, trim 0.002 ms, verify 0.002 ms, weak_pages 0.001 ms
- ('messages-slow', 2, 'diagnostic'): sweep_finalize 17.122 ms, roots 0.052 ms, prepare 0.021 ms, trace 0.016 ms, finish 0.016 ms, accounting 0.010 ms, stop_world 0.010 ms, allocation_census 0.003 ms, resume_world 0.003 ms, trim 0.002 ms, verify 0.001 ms, weak_pages 0.001 ms
- ('messages-slow', 2, 'diagnostic-allocations'): sweep_finalize 17.202 ms, allocation_census 11.743 ms, roots 0.062 ms, prepare 0.022 ms, trace 0.016 ms, finish 0.014 ms, accounting 0.011 ms, stop_world 0.010 ms, resume_world 0.004 ms, trim 0.002 ms, verify 0.002 ms, weak_pages 0.001 ms
- ('messages-slow', 4, 'diagnostic'): sweep_finalize 11.051 ms, roots 0.044 ms, prepare 0.015 ms, finish 0.014 ms, trace 0.013 ms, accounting 0.010 ms, stop_world 0.008 ms, allocation_census 0.003 ms, resume_world 0.002 ms, trim 0.002 ms, verify 0.002 ms, weak_pages 0.001 ms
- ('messages-slow', 4, 'diagnostic-allocations'): sweep_finalize 10.803 ms, allocation_census 10.420 ms, roots 0.046 ms, prepare 0.019 ms, trace 0.018 ms, finish 0.015 ms, accounting 0.010 ms, stop_world 0.009 ms, resume_world 0.003 ms, trim 0.002 ms, verify 0.001 ms, weak_pages 0.001 ms
- ('messages-slow', 8, 'diagnostic'): sweep_finalize 10.072 ms, roots 0.052 ms, prepare 0.018 ms, finish 0.015 ms, trace 0.015 ms, accounting 0.010 ms, allocation_census 0.007 ms, stop_world 0.007 ms, resume_world 0.003 ms, trim 0.002 ms, verify 0.002 ms, weak_pages 0.001 ms
- ('messages-slow', 8, 'diagnostic-allocations'): allocation_census 11.078 ms, sweep_finalize 8.585 ms, roots 0.053 ms, prepare 0.021 ms, trace 0.015 ms, finish 0.014 ms, accounting 0.010 ms, stop_world 0.006 ms, resume_world 0.003 ms, trim 0.002 ms, verify 0.002 ms, weak_pages 0.001 ms
