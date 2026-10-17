| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | control | 485.315 [479.880, 550.129] | 88.655 | 53.9 |
| backpressure | 1 | minors32 | 516.406 [471.296, 522.472] | 86.935 | 61.8 |
| backpressure | 1 | no-trim | 478.192 [450.343, 489.372] | 86.838 | 52.4 |
| backpressure | 1 | young64m | 492.783 [467.747, 525.475] | 72.148 | 108.4 |
| backpressure | 2 | control | 420.532 [370.787, 1549.350] | 94.744 | 57.0 |
| backpressure | 2 | minors32 | 417.573 [360.712, 478.112] | 92.968 | 65.7 |
| backpressure | 2 | no-trim | 368.993 [349.845, 425.580] | 82.445 | 55.1 |
| backpressure | 2 | young64m | 339.903 [326.277, 357.162] | 63.220 | 112.7 |
| backpressure | 4 | control | 231.705 [212.061, 248.579] | 63.301 | 66.4 |
| backpressure | 4 | minors32 | 239.271 [232.150, 241.037] | 62.835 | 73.6 |
| backpressure | 4 | no-trim | 260.818 [246.109, 266.540] | 70.726 | 70.3 |
| backpressure | 4 | young64m | 232.867 [226.280, 266.300] | 51.389 | 131.8 |
| backpressure | 8 | control | 195.764 [154.384, 198.209] | 62.453 | 69.3 |
| backpressure | 8 | minors32 | 185.214 [151.149, 194.933] | 55.887 | 72.7 |
| backpressure | 8 | no-trim | 198.832 [152.739, 499.235] | 66.862 | 75.5 |
| backpressure | 8 | young64m | 158.810 [156.186, 199.045] | 45.727 | 121.7 |
| balanced | 1 | control | 556.356 [549.557, 569.361] | 124.819 | 81.4 |
| balanced | 1 | minors32 | 533.190 [520.579, 535.290] | 90.522 | 144.2 |
| balanced | 1 | no-trim | 573.828 [544.714, 631.472] | 128.286 | 81.5 |
| balanced | 1 | young64m | 451.418 [422.689, 490.640] | 59.167 | 124.1 |
| balanced | 2 | control | 394.662 [384.703, 419.223] | 107.206 | 87.5 |
| balanced | 2 | minors32 | 375.433 [372.372, 395.415] | 79.632 | 146.9 |
| balanced | 2 | no-trim | 431.236 [398.572, 1372.649] | 107.910 | 86.6 |
| balanced | 2 | young64m | 331.790 [314.473, 350.424] | 57.561 | 141.7 |
| balanced | 4 | control | 267.226 [253.273, 272.070] | 88.026 | 90.6 |
| balanced | 4 | minors32 | 252.314 [234.282, 289.528] | 65.531 | 150.9 |
| balanced | 4 | no-trim | 248.971 [246.077, 255.576] | 81.669 | 91.4 |
| balanced | 4 | young64m | 206.653 [197.729, 228.373] | 46.836 | 132.9 |
| balanced | 8 | control | 187.886 [182.250, 195.359] | 77.865 | 97.9 |
| balanced | 8 | minors32 | 201.952 [167.760, 233.311] | 66.100 | 150.5 |
| balanced | 8 | no-trim | 181.588 [174.662, 220.801] | 74.128 | 94.2 |
| balanced | 8 | young64m | 150.694 [147.994, 703.169] | 41.283 | 147.9 |
| cpu | 1 | control | 1086.138 [1010.885, 1153.010] | 132.145 | 78.6 |
| cpu | 1 | minors32 | 952.115 [935.638, 975.974] | 91.463 | 139.3 |
| cpu | 1 | no-trim | 1049.472 [952.366, 1133.339] | 129.989 | 78.3 |
| cpu | 1 | young64m | 870.213 [869.603, 883.268] | 67.827 | 123.2 |
| cpu | 2 | control | 612.388 [600.713, 640.867] | 109.341 | 85.4 |
| cpu | 2 | minors32 | 568.834 [564.484, 1983.475] | 83.559 | 142.5 |
| cpu | 2 | no-trim | 630.348 [593.738, 2057.492] | 107.474 | 83.9 |
| cpu | 2 | young64m | 572.083 [520.316, 612.521] | 74.594 | 128.1 |
| cpu | 4 | control | 343.085 [335.272, 432.483] | 79.986 | 90.4 |
| cpu | 4 | minors32 | 323.636 [317.338, 337.677] | 62.452 | 153.7 |
| cpu | 4 | no-trim | 445.027 [348.358, 528.153] | 102.464 | 90.5 |
| cpu | 4 | young64m | 387.159 [302.649, 1382.988] | 61.676 | 139.0 |
| cpu | 8 | control | 357.107 [332.333, 408.866] | 83.922 | 127.2 |
| cpu | 8 | minors32 | 404.774 [371.182, 1093.317] | 72.886 | 169.0 |
| cpu | 8 | no-trim | 352.096 [342.904, 388.891] | 77.714 | 120.6 |
| cpu | 8 | young64m | 389.455 [347.656, 1041.547] | 61.588 | 175.5 |
| fanout | 1 | control | 1546.355 [1476.053, 1752.784] | 286.431 | 81.4 |
| fanout | 1 | minors32 | 1488.676 [1482.460, 1542.273] | 243.480 | 152.3 |
| fanout | 1 | no-trim | 1688.830 [1479.650, 1729.564] | 317.371 | 81.4 |
| fanout | 1 | young64m | 1255.975 [1164.156, 1454.585] | 183.427 | 152.7 |
| fanout | 2 | control | 1135.454 [1130.110, 1241.643] | 261.608 | 100.2 |
| fanout | 2 | minors32 | 1264.923 [1155.911, 3625.362] | 249.797 | 159.5 |
| fanout | 2 | no-trim | 1286.453 [1089.956, 3679.127] | 280.791 | 92.3 |
| fanout | 2 | young64m | 1057.172 [990.275, 1087.654] | 186.386 | 167.9 |
| fanout | 4 | control | 817.405 [772.013, 877.978] | 210.497 | 109.6 |
| fanout | 4 | minors32 | 784.006 [768.385, 879.159] | 181.629 | 160.7 |
| fanout | 4 | no-trim | 849.188 [772.258, 1007.297] | 221.614 | 104.4 |
| fanout | 4 | young64m | 711.094 [710.562, 898.334] | 140.258 | 178.9 |
| fanout | 8 | control | 904.718 [875.188, 1076.113] | 176.459 | 96.1 |
| fanout | 8 | minors32 | 889.052 [820.634, 2432.080] | 169.246 | 171.2 |
| fanout | 8 | no-trim | 958.646 [863.957, 961.057] | 192.552 | 99.3 |
| fanout | 8 | young64m | 904.114 [824.732, 1005.330] | 146.278 | 174.7 |

Diagnostic GC phases summed across measured repetitions (wall time):

- ('backpressure', 1, 'control'): sweep_finalize 232.876 ms, trace 15.570 ms, weak_pages 5.085 ms, prepare 5.020 ms, trim 1.308 ms, roots 0.812 ms, finish 0.101 ms, accounting 0.074 ms, resume_world 0.065 ms, verify 0.012 ms, stop_world 0.009 ms
- ('backpressure', 1, 'minors32'): sweep_finalize 235.167 ms, trace 14.226 ms, roots 0.776 ms, prepare 0.154 ms, finish 0.106 ms, accounting 0.077 ms, resume_world 0.051 ms, trim 0.014 ms, weak_pages 0.013 ms, verify 0.010 ms, stop_world 0.010 ms
- ('backpressure', 1, 'no-trim'): sweep_finalize 220.829 ms, trace 14.078 ms, weak_pages 5.137 ms, prepare 5.123 ms, roots 0.749 ms, finish 0.104 ms, accounting 0.075 ms, resume_world 0.057 ms, trim 0.015 ms, verify 0.013 ms, stop_world 0.009 ms
- ('backpressure', 1, 'young64m'): sweep_finalize 211.415 ms, trace 5.589 ms, roots 0.230 ms, prepare 0.049 ms, finish 0.033 ms, resume_world 0.032 ms, accounting 0.024 ms, weak_pages 0.005 ms, trim 0.004 ms, verify 0.003 ms, stop_world 0.002 ms
- ('backpressure', 2, 'control'): sweep_finalize 393.385 ms, stop_world 24.823 ms, trace 22.978 ms, weak_pages 8.384 ms, prepare 6.688 ms, trim 1.904 ms, roots 1.198 ms, finish 0.170 ms, accounting 0.101 ms, resume_world 0.043 ms, verify 0.014 ms
- ('backpressure', 2, 'minors32'): sweep_finalize 256.399 ms, trace 15.314 ms, roots 0.841 ms, stop_world 0.432 ms, prepare 0.226 ms, finish 0.114 ms, accounting 0.086 ms, resume_world 0.037 ms, trim 0.017 ms, weak_pages 0.014 ms, verify 0.011 ms
- ('backpressure', 2, 'no-trim'): sweep_finalize 225.962 ms, trace 14.835 ms, weak_pages 5.915 ms, prepare 5.534 ms, roots 0.792 ms, finish 0.108 ms, accounting 0.083 ms, stop_world 0.057 ms, resume_world 0.032 ms, trim 0.017 ms, verify 0.012 ms
- ('backpressure', 2, 'young64m'): sweep_finalize 182.135 ms, trace 5.127 ms, roots 0.218 ms, prepare 0.067 ms, finish 0.036 ms, accounting 0.029 ms, stop_world 0.018 ms, resume_world 0.013 ms, trim 0.005 ms, weak_pages 0.004 ms, verify 0.003 ms
- ('backpressure', 4, 'control'): sweep_finalize 162.549 ms, trace 14.692 ms, weak_pages 6.194 ms, prepare 5.352 ms, trim 0.985 ms, roots 0.822 ms, stop_world 0.424 ms, finish 0.148 ms, accounting 0.076 ms, resume_world 0.027 ms, verify 0.012 ms
- ('backpressure', 4, 'minors32'): sweep_finalize 172.724 ms, trace 14.668 ms, roots 0.850 ms, stop_world 0.376 ms, prepare 0.260 ms, finish 0.114 ms, accounting 0.078 ms, resume_world 0.031 ms, trim 0.016 ms, weak_pages 0.014 ms, verify 0.012 ms
- ('backpressure', 4, 'no-trim'): sweep_finalize 178.297 ms, trace 15.759 ms, weak_pages 5.963 ms, prepare 5.490 ms, roots 0.899 ms, stop_world 0.221 ms, finish 0.115 ms, accounting 0.081 ms, resume_world 0.031 ms, trim 0.017 ms, verify 0.013 ms
- ('backpressure', 4, 'young64m'): sweep_finalize 145.008 ms, trace 6.093 ms, roots 0.304 ms, stop_world 0.073 ms, finish 0.073 ms, prepare 0.071 ms, accounting 0.027 ms, resume_world 0.012 ms, trim 0.005 ms, weak_pages 0.004 ms, verify 0.003 ms
- ('backpressure', 8, 'control'): sweep_finalize 139.313 ms, trace 16.860 ms, weak_pages 7.495 ms, prepare 6.040 ms, trim 1.429 ms, roots 0.957 ms, finish 0.137 ms, stop_world 0.127 ms, accounting 0.072 ms, resume_world 0.029 ms, verify 0.014 ms
- ('backpressure', 8, 'minors32'): sweep_finalize 143.744 ms, trace 15.755 ms, roots 0.908 ms, stop_world 0.360 ms, prepare 0.278 ms, finish 0.187 ms, accounting 0.075 ms, resume_world 0.029 ms, trim 0.016 ms, weak_pages 0.015 ms, verify 0.012 ms
- ('backpressure', 8, 'no-trim'): sweep_finalize 195.106 ms, trace 20.330 ms, weak_pages 7.715 ms, stop_world 7.259 ms, prepare 6.692 ms, roots 1.129 ms, finish 0.194 ms, accounting 0.087 ms, resume_world 0.038 ms, trim 0.018 ms, verify 0.014 ms
- ('backpressure', 8, 'young64m'): sweep_finalize 131.935 ms, trace 5.899 ms, roots 0.253 ms, finish 0.158 ms, prepare 0.069 ms, stop_world 0.035 ms, accounting 0.025 ms, resume_world 0.013 ms, trim 0.005 ms, weak_pages 0.004 ms, verify 0.003 ms
- ('balanced', 1, 'control'): sweep_finalize 274.902 ms, trace 69.240 ms, weak_pages 9.910 ms, prepare 9.792 ms, trim 4.562 ms, roots 1.151 ms, finish 0.098 ms, accounting 0.062 ms, resume_world 0.061 ms, verify 0.013 ms, stop_world 0.008 ms
- ('balanced', 1, 'minors32'): sweep_finalize 195.708 ms, trace 73.589 ms, roots 0.751 ms, prepare 0.163 ms, finish 0.098 ms, accounting 0.066 ms, resume_world 0.049 ms, weak_pages 0.018 ms, trim 0.014 ms, verify 0.011 ms, stop_world 0.005 ms
- ('balanced', 1, 'no-trim'): sweep_finalize 284.802 ms, trace 74.665 ms, weak_pages 10.111 ms, prepare 9.122 ms, roots 0.908 ms, finish 0.106 ms, accounting 0.084 ms, resume_world 0.058 ms, trim 0.016 ms, verify 0.014 ms, stop_world 0.008 ms
- ('balanced', 1, 'young64m'): sweep_finalize 164.583 ms, trace 16.220 ms, roots 0.243 ms, prepare 0.044 ms, finish 0.029 ms, resume_world 0.026 ms, accounting 0.022 ms, weak_pages 0.006 ms, trim 0.004 ms, verify 0.003 ms, stop_world 0.002 ms
- ('balanced', 2, 'control'): sweep_finalize 227.522 ms, trace 70.518 ms, weak_pages 13.417 ms, prepare 9.098 ms, trim 2.400 ms, roots 1.072 ms, stop_world 0.288 ms, finish 0.126 ms, accounting 0.077 ms, resume_world 0.029 ms, verify 0.014 ms
- ('balanced', 2, 'minors32'): sweep_finalize 165.896 ms, trace 71.631 ms, roots 0.866 ms, stop_world 0.310 ms, prepare 0.188 ms, finish 0.093 ms, accounting 0.065 ms, resume_world 0.024 ms, weak_pages 0.018 ms, trim 0.016 ms, verify 0.012 ms
- ('balanced', 2, 'no-trim'): sweep_finalize 390.821 ms, trace 110.241 ms, stop_world 14.142 ms, weak_pages 12.046 ms, prepare 10.370 ms, roots 1.379 ms, finish 0.136 ms, accounting 0.098 ms, resume_world 0.043 ms, trim 0.019 ms, verify 0.015 ms
- ('balanced', 2, 'young64m'): sweep_finalize 159.911 ms, trace 13.527 ms, roots 0.212 ms, stop_world 0.102 ms, prepare 0.042 ms, finish 0.030 ms, accounting 0.023 ms, resume_world 0.009 ms, trim 0.004 ms, weak_pages 0.004 ms, verify 0.002 ms
- ('balanced', 4, 'control'): sweep_finalize 173.032 ms, trace 67.807 ms, weak_pages 10.932 ms, prepare 8.784 ms, trim 2.488 ms, roots 0.977 ms, stop_world 0.454 ms, finish 0.121 ms, accounting 0.072 ms, verify 0.036 ms, resume_world 0.025 ms
- ('balanced', 4, 'minors32'): sweep_finalize 119.985 ms, trace 78.252 ms, roots 1.004 ms, prepare 0.250 ms, stop_world 0.215 ms, finish 0.110 ms, accounting 0.069 ms, weak_pages 0.022 ms, resume_world 0.022 ms, trim 0.015 ms, verify 0.012 ms
- ('balanced', 4, 'no-trim'): sweep_finalize 158.022 ms, trace 67.841 ms, weak_pages 10.101 ms, prepare 8.622 ms, roots 0.992 ms, finish 0.124 ms, stop_world 0.083 ms, accounting 0.069 ms, resume_world 0.027 ms, trim 0.015 ms, verify 0.013 ms
- ('balanced', 4, 'young64m'): sweep_finalize 124.019 ms, trace 16.864 ms, roots 0.309 ms, finish 0.098 ms, prepare 0.071 ms, stop_world 0.054 ms, accounting 0.026 ms, resume_world 0.009 ms, weak_pages 0.006 ms, trim 0.005 ms, verify 0.003 ms
- ('balanced', 8, 'control'): sweep_finalize 137.915 ms, trace 69.450 ms, weak_pages 10.797 ms, prepare 8.540 ms, trim 2.638 ms, roots 1.029 ms, stop_world 0.370 ms, finish 0.120 ms, accounting 0.065 ms, resume_world 0.027 ms, verify 0.013 ms
- ('balanced', 8, 'minors32'): sweep_finalize 104.450 ms, trace 85.503 ms, roots 1.152 ms, stop_world 0.526 ms, prepare 0.351 ms, finish 0.170 ms, accounting 0.077 ms, weak_pages 0.028 ms, resume_world 0.026 ms, trim 0.015 ms, verify 0.013 ms
- ('balanced', 8, 'no-trim'): sweep_finalize 139.675 ms, trace 67.907 ms, weak_pages 10.217 ms, prepare 8.734 ms, roots 1.045 ms, stop_world 0.263 ms, finish 0.155 ms, accounting 0.069 ms, resume_world 0.031 ms, trim 0.015 ms, verify 0.014 ms
- ('balanced', 8, 'young64m'): sweep_finalize 178.800 ms, trace 21.644 ms, stop_world 6.898 ms, roots 0.362 ms, finish 0.124 ms, prepare 0.094 ms, accounting 0.024 ms, resume_world 0.014 ms, weak_pages 0.006 ms, trim 0.005 ms, verify 0.003 ms
- ('cpu', 1, 'control'): sweep_finalize 303.064 ms, trace 73.191 ms, weak_pages 9.776 ms, prepare 8.988 ms, trim 3.591 ms, roots 1.234 ms, finish 0.112 ms, accounting 0.079 ms, resume_world 0.068 ms, verify 0.015 ms, stop_world 0.012 ms
- ('cpu', 1, 'minors32'): sweep_finalize 199.344 ms, trace 69.401 ms, roots 1.012 ms, prepare 0.156 ms, finish 0.095 ms, accounting 0.062 ms, resume_world 0.052 ms, weak_pages 0.018 ms, trim 0.014 ms, verify 0.012 ms, stop_world 0.010 ms
- ('cpu', 1, 'no-trim'): sweep_finalize 291.628 ms, trace 72.874 ms, weak_pages 9.346 ms, prepare 8.996 ms, roots 1.250 ms, finish 0.105 ms, accounting 0.073 ms, resume_world 0.055 ms, trim 0.021 ms, verify 0.014 ms, stop_world 0.012 ms
- ('cpu', 1, 'young64m'): sweep_finalize 187.729 ms, trace 16.468 ms, roots 0.281 ms, prepare 0.048 ms, finish 0.035 ms, resume_world 0.028 ms, accounting 0.024 ms, trim 0.005 ms, weak_pages 0.005 ms, verify 0.003 ms, stop_world 0.003 ms
- ('cpu', 2, 'control'): sweep_finalize 237.074 ms, trace 67.828 ms, weak_pages 10.182 ms, prepare 8.645 ms, trim 1.887 ms, roots 1.305 ms, finish 0.111 ms, stop_world 0.104 ms, accounting 0.082 ms, resume_world 0.033 ms, verify 0.013 ms
- ('cpu', 2, 'minors32'): sweep_finalize 277.137 ms, trace 98.396 ms, stop_world 31.038 ms, roots 1.461 ms, prepare 0.261 ms, finish 0.112 ms, accounting 0.088 ms, resume_world 0.028 ms, weak_pages 0.026 ms, trim 0.017 ms, verify 0.014 ms
- ('cpu', 2, 'no-trim'): sweep_finalize 370.610 ms, trace 97.210 ms, stop_world 40.313 ms, weak_pages 14.613 ms, prepare 9.675 ms, roots 1.653 ms, finish 0.124 ms, accounting 0.091 ms, resume_world 0.034 ms, trim 0.019 ms, verify 0.016 ms
- ('cpu', 2, 'young64m'): sweep_finalize 200.607 ms, trace 16.729 ms, roots 0.311 ms, finish 0.053 ms, prepare 0.052 ms, stop_world 0.032 ms, accounting 0.031 ms, resume_world 0.013 ms, trim 0.006 ms, weak_pages 0.006 ms, verify 0.003 ms
- ('cpu', 4, 'control'): sweep_finalize 170.925 ms, trace 59.880 ms, weak_pages 8.911 ms, prepare 7.621 ms, trim 1.796 ms, roots 1.177 ms, stop_world 0.171 ms, finish 0.152 ms, accounting 0.078 ms, resume_world 0.031 ms, verify 0.013 ms
- ('cpu', 4, 'minors32'): sweep_finalize 120.330 ms, trace 61.438 ms, roots 1.063 ms, stop_world 0.306 ms, prepare 0.215 ms, finish 0.141 ms, accounting 0.074 ms, resume_world 0.025 ms, weak_pages 0.020 ms, trim 0.016 ms, verify 0.011 ms
- ('cpu', 4, 'no-trim'): sweep_finalize 205.905 ms, trace 76.221 ms, weak_pages 12.582 ms, prepare 10.003 ms, roots 1.410 ms, stop_world 0.574 ms, finish 0.180 ms, accounting 0.093 ms, resume_world 0.037 ms, trim 0.020 ms, verify 0.017 ms
- ('cpu', 4, 'young64m'): sweep_finalize 247.083 ms, trace 21.394 ms, stop_world 16.912 ms, roots 0.448 ms, prepare 0.097 ms, finish 0.086 ms, accounting 0.032 ms, resume_world 0.014 ms, weak_pages 0.007 ms, trim 0.005 ms, verify 0.004 ms
- ('cpu', 8, 'control'): sweep_finalize 162.969 ms, trace 62.027 ms, weak_pages 9.872 ms, prepare 8.452 ms, trim 1.821 ms, roots 1.058 ms, stop_world 0.285 ms, finish 0.132 ms, accounting 0.070 ms, resume_world 0.031 ms, verify 0.013 ms
- ('cpu', 8, 'minors32'): sweep_finalize 207.861 ms, trace 93.616 ms, stop_world 66.512 ms, roots 1.501 ms, prepare 0.346 ms, finish 0.210 ms, accounting 0.091 ms, resume_world 0.041 ms, weak_pages 0.029 ms, trim 0.017 ms, verify 0.014 ms
- ('cpu', 8, 'no-trim'): sweep_finalize 167.593 ms, trace 61.785 ms, weak_pages 10.065 ms, prepare 8.719 ms, roots 1.035 ms, finish 0.182 ms, stop_world 0.178 ms, accounting 0.074 ms, resume_world 0.037 ms, trim 0.018 ms, verify 0.014 ms
- ('cpu', 8, 'young64m'): sweep_finalize 213.345 ms, trace 20.116 ms, stop_world 9.235 ms, roots 0.415 ms, prepare 0.106 ms, finish 0.087 ms, accounting 0.030 ms, resume_world 0.020 ms, weak_pages 0.008 ms, trim 0.006 ms, verify 0.004 ms
- ('fanout', 1, 'control'): sweep_finalize 665.948 ms, trace 145.186 ms, weak_pages 29.590 ms, prepare 26.042 ms, trim 13.532 ms, roots 9.872 ms, finish 0.195 ms, accounting 0.137 ms, resume_world 0.094 ms, verify 0.024 ms, stop_world 0.015 ms
- ('fanout', 1, 'minors32'): sweep_finalize 552.614 ms, trace 138.099 ms, weak_pages 12.029 ms, prepare 10.743 ms, roots 9.624 ms, trim 3.791 ms, finish 0.182 ms, accounting 0.125 ms, resume_world 0.079 ms, verify 0.023 ms, stop_world 0.014 ms
- ('fanout', 1, 'no-trim'): sweep_finalize 703.990 ms, trace 151.993 ms, weak_pages 32.276 ms, prepare 27.529 ms, roots 10.334 ms, finish 0.212 ms, accounting 0.146 ms, resume_world 0.100 ms, trim 0.031 ms, verify 0.026 ms, stop_world 0.017 ms
- ('fanout', 1, 'young64m'): sweep_finalize 466.324 ms, trace 43.481 ms, weak_pages 17.107 ms, prepare 12.590 ms, trim 3.535 ms, roots 2.935 ms, finish 0.061 ms, accounting 0.046 ms, resume_world 0.042 ms, verify 0.007 ms, stop_world 0.004 ms
- ('fanout', 2, 'control'): sweep_finalize 570.486 ms, trace 133.734 ms, weak_pages 31.766 ms, prepare 25.090 ms, trim 24.647 ms, roots 8.026 ms, stop_world 0.557 ms, finish 0.188 ms, accounting 0.156 ms, resume_world 0.078 ms, verify 0.028 ms
- ('fanout', 2, 'minors32'): sweep_finalize 801.566 ms, trace 199.805 ms, stop_world 24.622 ms, weak_pages 17.641 ms, roots 15.570 ms, prepare 11.718 ms, trim 3.212 ms, finish 0.236 ms, accounting 0.182 ms, resume_world 0.092 ms, verify 0.027 ms
- ('fanout', 2, 'no-trim'): sweep_finalize 891.556 ms, trace 190.261 ms, weak_pages 43.074 ms, prepare 35.288 ms, roots 18.408 ms, stop_world 15.932 ms, finish 0.228 ms, accounting 0.179 ms, resume_world 0.089 ms, trim 0.033 ms, verify 0.030 ms
- ('fanout', 2, 'young64m'): sweep_finalize 480.975 ms, trace 46.238 ms, weak_pages 16.609 ms, prepare 14.138 ms, trim 3.056 ms, roots 2.215 ms, stop_world 0.133 ms, finish 0.067 ms, accounting 0.052 ms, resume_world 0.027 ms, verify 0.008 ms
- ('fanout', 4, 'control'): sweep_finalize 424.729 ms, trace 128.884 ms, weak_pages 30.550 ms, prepare 24.135 ms, roots 7.492 ms, trim 6.345 ms, finish 0.196 ms, accounting 0.156 ms, stop_world 0.109 ms, resume_world 0.073 ms, verify 0.026 ms
- ('fanout', 4, 'minors32'): sweep_finalize 381.240 ms, trace 135.393 ms, weak_pages 11.478 ms, prepare 9.316 ms, roots 8.216 ms, trim 1.692 ms, stop_world 0.524 ms, finish 0.192 ms, accounting 0.158 ms, resume_world 0.071 ms, verify 0.023 ms
- ('fanout', 4, 'no-trim'): sweep_finalize 460.314 ms, trace 142.480 ms, weak_pages 31.020 ms, prepare 25.467 ms, roots 8.600 ms, finish 0.208 ms, stop_world 0.170 ms, accounting 0.157 ms, resume_world 0.075 ms, verify 0.033 ms, trim 0.033 ms
- ('fanout', 4, 'young64m'): sweep_finalize 367.527 ms, trace 46.924 ms, weak_pages 15.932 ms, prepare 12.711 ms, roots 2.544 ms, trim 1.791 ms, stop_world 0.110 ms, finish 0.072 ms, accounting 0.052 ms, resume_world 0.029 ms, verify 0.007 ms
- ('fanout', 8, 'control'): sweep_finalize 352.561 ms, trace 151.441 ms, weak_pages 24.434 ms, prepare 20.727 ms, roots 8.924 ms, trim 6.184 ms, stop_world 0.535 ms, finish 0.195 ms, accounting 0.145 ms, resume_world 0.074 ms, verify 0.026 ms
- ('fanout', 8, 'minors32'): sweep_finalize 435.243 ms, trace 164.245 ms, weak_pages 12.532 ms, stop_world 11.999 ms, prepare 10.731 ms, roots 10.423 ms, trim 1.812 ms, finish 0.206 ms, accounting 0.164 ms, resume_world 0.075 ms, verify 0.026 ms
- ('fanout', 8, 'no-trim'): sweep_finalize 364.087 ms, trace 148.882 ms, weak_pages 26.347 ms, prepare 20.066 ms, roots 8.790 ms, stop_world 0.227 ms, finish 0.198 ms, accounting 0.143 ms, resume_world 0.072 ms, trim 0.031 ms, verify 0.027 ms
- ('fanout', 8, 'young64m'): sweep_finalize 347.240 ms, trace 56.098 ms, weak_pages 16.291 ms, prepare 13.018 ms, roots 3.622 ms, trim 1.593 ms, finish 0.115 ms, stop_world 0.113 ms, accounting 0.055 ms, resume_world 0.027 ms, verify 0.009 ms
