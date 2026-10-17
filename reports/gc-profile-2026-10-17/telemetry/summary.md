| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 468.067 [453.253, 469.132] | 80.139 | 52.3 |
| backpressure | 1 | diagnostic | 476.823 [470.929, 487.109] | 83.071 | 55.1 |
| backpressure | 1 | diagnostic-off | 470.673 [466.937, 477.112] | 80.996 | 52.6 |
| backpressure | 2 | baseline | 362.701 [351.242, 367.601] | 77.587 | 54.3 |
| backpressure | 2 | diagnostic | 367.459 [356.322, 375.779] | 83.223 | 56.1 |
| backpressure | 2 | diagnostic-off | 367.173 [363.438, 397.033] | 84.682 | 55.1 |
| backpressure | 4 | baseline | 241.537 [229.309, 244.903] | 60.373 | 70.2 |
| backpressure | 4 | diagnostic | 231.234 [222.075, 235.981] | 60.162 | 66.4 |
| backpressure | 4 | diagnostic-off | 229.219 [225.630, 235.063] | 63.100 | 66.2 |
| backpressure | 8 | baseline | 164.623 [161.612, 168.978] | 51.357 | 74.6 |
| backpressure | 8 | diagnostic | 168.240 [163.886, 198.600] | 53.585 | 76.4 |
| backpressure | 8 | diagnostic-off | 168.465 [161.383, 169.201] | 51.776 | 76.0 |
| balanced | 1 | baseline | 545.296 [528.884, 551.712] | 118.572 | 81.2 |
| balanced | 1 | diagnostic | 552.115 [546.385, 557.224] | 119.085 | 81.4 |
| balanced | 1 | diagnostic-off | 558.786 [557.101, 564.383] | 116.887 | 81.5 |
| balanced | 2 | baseline | 376.063 [372.900, 377.467] | 98.326 | 85.2 |
| balanced | 2 | diagnostic | 415.430 [389.288, 436.713] | 104.335 | 91.2 |
| balanced | 2 | diagnostic-off | 388.595 [388.174, 389.540] | 102.532 | 86.8 |
| balanced | 4 | baseline | 257.740 [256.455, 265.574] | 79.820 | 96.3 |
| balanced | 4 | diagnostic | 249.662 [246.602, 253.358] | 84.011 | 93.5 |
| balanced | 4 | diagnostic-off | 249.553 [240.829, 262.563] | 83.128 | 95.4 |
| balanced | 8 | baseline | 180.586 [177.297, 181.194] | 68.802 | 99.9 |
| balanced | 8 | diagnostic | 189.402 [179.543, 194.949] | 74.561 | 96.3 |
| balanced | 8 | diagnostic-off | 183.898 [181.595, 184.827] | 69.837 | 101.2 |
| cpu | 1 | baseline | 984.545 [978.860, 986.807] | 117.968 | 78.1 |
| cpu | 1 | diagnostic | 994.256 [988.016, 997.935] | 118.899 | 78.2 |
| cpu | 1 | diagnostic-off | 996.937 [980.664, 1004.498] | 119.903 | 78.1 |
| cpu | 2 | baseline | 593.066 [576.380, 600.993] | 98.596 | 84.6 |
| cpu | 2 | diagnostic | 616.323 [570.765, 633.746] | 102.818 | 85.3 |
| cpu | 2 | diagnostic-off | 619.450 [610.119, 626.318] | 104.431 | 85.0 |
| cpu | 4 | baseline | 340.431 [332.412, 340.641] | 75.575 | 89.4 |
| cpu | 4 | diagnostic | 343.399 [335.173, 353.479] | 78.648 | 95.6 |
| cpu | 4 | diagnostic-off | 340.624 [335.606, 343.214] | 76.862 | 90.4 |
| cpu | 8 | baseline | 358.881 [334.168, 365.380] | 78.426 | 127.0 |
| cpu | 8 | diagnostic | 386.527 [368.128, 403.076] | 83.093 | 138.3 |
| cpu | 8 | diagnostic-off | 372.070 [367.547, 375.789] | 82.402 | 125.4 |
| fanout | 1 | baseline | 1456.195 [1398.323, 1480.921] | 284.832 | 80.8 |
| fanout | 1 | diagnostic | 1542.612 [1538.945, 1542.872] | 292.003 | 81.4 |
| fanout | 1 | diagnostic-off | 1562.862 [1520.229, 1607.725] | 289.615 | 81.1 |
| fanout | 2 | baseline | 1157.437 [1156.472, 1169.021] | 250.544 | 103.2 |
| fanout | 2 | diagnostic | 1177.631 [1153.587, 1216.325] | 259.422 | 99.5 |
| fanout | 2 | diagnostic-off | 1180.526 [1133.944, 1307.669] | 257.287 | 105.5 |
| fanout | 4 | baseline | 776.263 [751.642, 797.926] | 191.888 | 106.3 |
| fanout | 4 | diagnostic | 850.249 [833.890, 854.471] | 213.174 | 118.1 |
| fanout | 4 | diagnostic-off | 801.667 [800.985, 817.599] | 203.578 | 107.5 |
| fanout | 8 | baseline | 836.042 [832.842, 905.754] | 160.935 | 101.3 |
| fanout | 8 | diagnostic | 871.994 [859.778, 940.353] | 173.818 | 97.9 |
| fanout | 8 | diagnostic-off | 881.263 [832.310, 887.814] | 176.807 | 100.0 |

Diagnostic GC phases summed across measured repetitions (wall time):

- ('backpressure', 1, 'diagnostic'): sweep_finalize 214.717 ms, trace 14.142 ms, weak_pages 5.253 ms, prepare 5.193 ms, trim 1.171 ms, roots 0.747 ms, finish 0.089 ms, accounting 0.062 ms, resume_world 0.045 ms, verify 0.011 ms, stop_world 0.009 ms
- ('backpressure', 2, 'diagnostic'): sweep_finalize 216.148 ms, trace 15.258 ms, weak_pages 6.748 ms, prepare 6.294 ms, trim 1.042 ms, roots 0.859 ms, stop_world 0.307 ms, finish 0.103 ms, accounting 0.080 ms, resume_world 0.030 ms, verify 0.012 ms
- ('backpressure', 4, 'diagnostic'): sweep_finalize 156.832 ms, trace 14.646 ms, weak_pages 5.577 ms, prepare 5.076 ms, trim 1.062 ms, roots 0.857 ms, finish 0.145 ms, stop_world 0.115 ms, accounting 0.077 ms, resume_world 0.027 ms, verify 0.012 ms
- ('backpressure', 8, 'diagnostic'): sweep_finalize 142.618 ms, trace 17.366 ms, weak_pages 6.896 ms, prepare 5.159 ms, roots 1.123 ms, trim 1.054 ms, stop_world 0.132 ms, finish 0.131 ms, accounting 0.073 ms, resume_world 0.034 ms, verify 0.014 ms
- ('balanced', 1, 'diagnostic'): sweep_finalize 263.530 ms, trace 66.639 ms, weak_pages 9.190 ms, prepare 8.812 ms, trim 4.245 ms, roots 0.866 ms, finish 0.102 ms, accounting 0.068 ms, resume_world 0.054 ms, verify 0.012 ms, stop_world 0.008 ms
- ('balanced', 2, 'diagnostic'): sweep_finalize 225.364 ms, trace 64.406 ms, weak_pages 9.611 ms, prepare 8.691 ms, trim 2.317 ms, roots 0.985 ms, stop_world 0.358 ms, finish 0.136 ms, accounting 0.075 ms, resume_world 0.026 ms, verify 0.012 ms
- ('balanced', 4, 'diagnostic'): sweep_finalize 160.978 ms, trace 66.032 ms, weak_pages 11.182 ms, prepare 8.847 ms, trim 2.504 ms, roots 1.038 ms, stop_world 0.180 ms, finish 0.140 ms, accounting 0.066 ms, resume_world 0.027 ms, verify 0.018 ms
- ('balanced', 8, 'diagnostic'): sweep_finalize 134.135 ms, trace 64.708 ms, weak_pages 10.092 ms, prepare 8.329 ms, trim 2.435 ms, roots 1.025 ms, stop_world 0.258 ms, finish 0.113 ms, accounting 0.067 ms, resume_world 0.029 ms, verify 0.014 ms
- ('cpu', 1, 'diagnostic'): sweep_finalize 263.136 ms, trace 64.612 ms, prepare 10.338 ms, weak_pages 8.610 ms, trim 3.345 ms, roots 1.223 ms, finish 0.100 ms, accounting 0.066 ms, resume_world 0.052 ms, verify 0.013 ms, stop_world 0.012 ms
- ('cpu', 2, 'diagnostic'): sweep_finalize 226.063 ms, trace 60.829 ms, weak_pages 8.471 ms, prepare 7.964 ms, trim 2.041 ms, roots 1.203 ms, stop_world 0.225 ms, finish 0.105 ms, accounting 0.085 ms, resume_world 0.028 ms, verify 0.013 ms
- ('cpu', 4, 'diagnostic'): sweep_finalize 160.076 ms, trace 53.215 ms, weak_pages 8.880 ms, prepare 7.556 ms, trim 2.075 ms, roots 1.104 ms, stop_world 0.377 ms, finish 0.136 ms, accounting 0.070 ms, resume_world 0.026 ms, verify 0.019 ms
- ('cpu', 8, 'diagnostic'): sweep_finalize 165.312 ms, trace 59.273 ms, weak_pages 9.303 ms, prepare 8.325 ms, trim 2.108 ms, roots 0.995 ms, stop_world 0.178 ms, finish 0.116 ms, accounting 0.076 ms, resume_world 0.032 ms, verify 0.013 ms
- ('fanout', 1, 'diagnostic'): sweep_finalize 648.970 ms, trace 143.349 ms, weak_pages 28.439 ms, prepare 26.962 ms, trim 12.861 ms, roots 9.984 ms, finish 0.192 ms, accounting 0.135 ms, resume_world 0.093 ms, verify 0.027 ms, stop_world 0.015 ms
- ('fanout', 2, 'diagnostic'): sweep_finalize 572.545 ms, trace 131.019 ms, weak_pages 29.450 ms, prepare 25.623 ms, trim 8.473 ms, roots 7.446 ms, stop_world 0.683 ms, finish 0.211 ms, accounting 0.160 ms, resume_world 0.076 ms, verify 0.025 ms
- ('fanout', 4, 'diagnostic'): sweep_finalize 418.944 ms, trace 134.485 ms, trim 38.182 ms, weak_pages 31.332 ms, prepare 24.939 ms, roots 7.641 ms, stop_world 0.530 ms, finish 0.194 ms, accounting 0.152 ms, resume_world 0.068 ms, verify 0.027 ms
- ('fanout', 8, 'diagnostic'): sweep_finalize 332.212 ms, trace 142.090 ms, weak_pages 24.642 ms, prepare 19.328 ms, roots 8.170 ms, trim 5.860 ms, finish 0.226 ms, stop_world 0.224 ms, accounting 0.144 ms, resume_world 0.072 ms, verify 0.026 ms
