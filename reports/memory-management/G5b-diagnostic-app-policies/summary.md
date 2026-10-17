| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | inline0 | 427.254 [423.657, 430.406] | 63.744 | 57.3 |
| backpressure | 1 | inline128 | 440.754 [433.966, 442.639] | 64.440 | 57.1 |
| backpressure | 1 | inline256 | 424.681 [414.392, 426.251] | 59.004 | 59.3 |
| backpressure | 1 | inline256-pool256 | 426.856 [422.222, 436.066] | 72.835 | 52.9 |
| backpressure | 8 | inline0 | 121.838 [121.571, 122.516] | 33.607 | 80.1 |
| backpressure | 8 | inline128 | 122.080 [120.772, 124.969] | 33.246 | 80.5 |
| backpressure | 8 | inline256 | 108.631 [104.647, 109.990] | 23.629 | 73.5 |
| backpressure | 8 | inline256-pool256 | 115.088 [114.280, 118.339] | 27.192 | 71.6 |
| balanced | 1 | inline0 | 439.774 [426.342, 444.394] | 74.611 | 84.3 |
| balanced | 1 | inline128 | 429.378 [429.072, 436.163] | 73.154 | 84.0 |
| balanced | 1 | inline256 | 422.005 [414.232, 431.285] | 72.085 | 87.9 |
| balanced | 1 | inline256-pool256 | 432.746 [427.781, 433.567] | 87.272 | 81.0 |
| balanced | 8 | inline0 | 125.599 [122.255, 127.980] | 41.367 | 107.2 |
| balanced | 8 | inline128 | 125.684 [122.424, 126.183] | 40.506 | 109.2 |
| balanced | 8 | inline256 | 117.517 [108.511, 118.664] | 36.738 | 109.4 |
| balanced | 8 | inline256-pool256 | 123.659 [121.578, 123.726] | 39.573 | 95.9 |
| cpu | 1 | inline0 | 849.464 [844.534, 858.999] | 71.302 | 78.3 |
| cpu | 1 | inline128 | 845.387 [838.492, 851.572] | 71.785 | 78.5 |
| cpu | 1 | inline256 | 844.967 [838.591, 845.796] | 71.724 | 82.1 |
| cpu | 1 | inline256-pool256 | 836.364 [835.216, 847.238] | 80.817 | 75.7 |
| cpu | 8 | inline0 | 331.768 [317.532, 332.164] | 48.087 | 128.6 |
| cpu | 8 | inline128 | 308.364 [300.661, 321.658] | 49.183 | 128.5 |
| cpu | 8 | inline256 | 314.475 [301.457, 333.769] | 41.113 | 126.6 |
| cpu | 8 | inline256-pool256 | 322.651 [313.883, 323.758] | 48.124 | 120.2 |
| fanout | 1 | inline0 | 1193.927 [1192.226, 1194.326] | 186.847 | 86.5 |
| fanout | 1 | inline128 | 1178.830 [1151.067, 1195.084] | 186.227 | 86.6 |
| fanout | 1 | inline256 | 1189.282 [1158.184, 1691.346] | 187.426 | 86.4 |
| fanout | 1 | inline256-pool256 | 1169.165 [1104.803, 1177.434] | 207.350 | 81.9 |
| fanout | 8 | inline0 | 763.439 [748.765, 782.813] | 121.512 | 105.4 |
| fanout | 8 | inline128 | 760.409 [757.736, 803.833] | 118.555 | 111.4 |
| fanout | 8 | inline256 | 766.399 [762.283, 767.140] | 111.416 | 110.8 |
| fanout | 8 | inline256-pool256 | 798.466 [784.310, 805.089] | 124.635 | 98.6 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | inline0 | 49152 | 0.960 | 4.352 |
| backpressure | 1 | inline128 | 49152 | 0.992 | 4.352 |
| backpressure | 1 | inline256 | 49152 | 0.960 | 3.968 |
| backpressure | 1 | inline256-pool256 | 49152 | 0.928 | 4.864 |
| backpressure | 8 | inline0 | 49152 | 0.176 | 1.856 |
| backpressure | 8 | inline128 | 49152 | 0.176 | 1.728 |
| backpressure | 8 | inline256 | 49152 | 0.168 | 1.280 |
| backpressure | 8 | inline256-pool256 | 49152 | 0.176 | 1.472 |
| balanced | 1 | inline0 | 49152 | 15.360 | 26.624 |
| balanced | 1 | inline128 | 49152 | 14.848 | 26.624 |
| balanced | 1 | inline256 | 49152 | 14.336 | 24.576 |
| balanced | 1 | inline256-pool256 | 49152 | 14.848 | 26.624 |
| balanced | 8 | inline0 | 49152 | 3.968 | 11.776 |
| balanced | 8 | inline128 | 49152 | 3.968 | 11.776 |
| balanced | 8 | inline256 | 49152 | 3.712 | 9.216 |
| balanced | 8 | inline256-pool256 | 49152 | 4.096 | 9.728 |
| cpu | 1 | inline0 | 49152 | 29.696 | 38.912 |
| cpu | 1 | inline128 | 49152 | 29.696 | 38.912 |
| cpu | 1 | inline256 | 49152 | 28.672 | 38.912 |
| cpu | 1 | inline256-pool256 | 49152 | 28.672 | 40.960 |
| cpu | 8 | inline0 | 49152 | 10.752 | 18.432 |
| cpu | 8 | inline128 | 49152 | 10.240 | 22.528 |
| cpu | 8 | inline256 | 49152 | 10.240 | 26.624 |
| cpu | 8 | inline256-pool256 | 49152 | 10.752 | 19.456 |
| fanout | 1 | inline0 | 49152 | 40.960 | 55.296 |
| fanout | 1 | inline128 | 49152 | 40.960 | 55.296 |
| fanout | 1 | inline256 | 49152 | 43.008 | 90.112 |
| fanout | 1 | inline256-pool256 | 49152 | 38.912 | 53.248 |
| fanout | 8 | inline0 | 49152 | 25.600 | 45.056 |
| fanout | 8 | inline128 | 49152 | 25.600 | 45.056 |
| fanout | 8 | inline256 | 49152 | 25.600 | 45.056 |
| fanout | 8 | inline256-pool256 | 49152 | 26.624 | 34.816 |

Diagnostic GC phases summed across measured repetitions (wall time):

- ('backpressure', 1, 'inline0'): sweep_finalize 164.440 ms, trace 11.703 ms, prepare 4.732 ms, weak_pages 3.814 ms, trim 1.169 ms, roots 0.602 ms, allocation_census 0.141 ms, finish 0.058 ms, accounting 0.048 ms, resume_world 0.042 ms, verify 0.009 ms, stop_world 0.006 ms
- ('backpressure', 1, 'inline128'): sweep_finalize 167.700 ms, trace 12.154 ms, prepare 4.964 ms, weak_pages 4.330 ms, trim 1.206 ms, roots 0.637 ms, allocation_census 0.153 ms, finish 0.068 ms, accounting 0.052 ms, resume_world 0.047 ms, verify 0.010 ms, stop_world 0.007 ms
- ('backpressure', 1, 'inline256'): sweep_finalize 151.766 ms, trace 11.837 ms, prepare 4.896 ms, weak_pages 4.260 ms, trim 1.058 ms, roots 0.600 ms, allocation_census 0.138 ms, finish 0.061 ms, accounting 0.049 ms, resume_world 0.039 ms, verify 0.010 ms, stop_world 0.005 ms
- ('backpressure', 1, 'inline256-pool256'): sweep_finalize 187.107 ms, trace 11.998 ms, prepare 5.165 ms, weak_pages 4.166 ms, trim 1.220 ms, roots 0.789 ms, allocation_census 0.217 ms, finish 0.069 ms, accounting 0.052 ms, resume_world 0.045 ms, verify 0.010 ms, stop_world 0.006 ms
- ('backpressure', 8, 'inline0'): sweep_finalize 76.443 ms, trace 12.561 ms, prepare 4.249 ms, weak_pages 3.589 ms, trim 1.110 ms, roots 0.703 ms, stop_world 0.273 ms, allocation_census 0.206 ms, finish 0.057 ms, accounting 0.054 ms, resume_world 0.022 ms, verify 0.010 ms
- ('backpressure', 8, 'inline128'): sweep_finalize 76.186 ms, trace 12.399 ms, prepare 4.277 ms, weak_pages 3.766 ms, trim 1.092 ms, roots 0.760 ms, allocation_census 0.200 ms, stop_world 0.191 ms, finish 0.056 ms, accounting 0.054 ms, resume_world 0.021 ms, verify 0.009 ms
- ('backpressure', 8, 'inline256'): sweep_finalize 46.888 ms, trace 12.486 ms, prepare 4.451 ms, weak_pages 4.169 ms, trim 1.233 ms, roots 0.715 ms, stop_world 0.179 ms, allocation_census 0.169 ms, finish 0.056 ms, accounting 0.048 ms, resume_world 0.019 ms, verify 0.010 ms
- ('backpressure', 8, 'inline256-pool256'): sweep_finalize 58.342 ms, trace 12.370 ms, prepare 4.222 ms, weak_pages 3.805 ms, trim 1.154 ms, roots 0.732 ms, allocation_census 0.207 ms, stop_world 0.130 ms, finish 0.056 ms, accounting 0.047 ms, resume_world 0.024 ms, verify 0.009 ms
- ('balanced', 1, 'inline0'): sweep_finalize 168.932 ms, trace 31.430 ms, prepare 8.545 ms, weak_pages 7.418 ms, trim 3.314 ms, roots 0.811 ms, allocation_census 0.274 ms, finish 0.064 ms, accounting 0.052 ms, resume_world 0.049 ms, verify 0.010 ms, stop_world 0.005 ms
- ('balanced', 1, 'inline128'): sweep_finalize 166.245 ms, trace 30.276 ms, prepare 8.358 ms, weak_pages 7.107 ms, trim 3.265 ms, roots 0.792 ms, allocation_census 0.232 ms, finish 0.065 ms, accounting 0.051 ms, resume_world 0.044 ms, verify 0.010 ms, stop_world 0.006 ms
- ('balanced', 1, 'inline256'): sweep_finalize 159.598 ms, trace 32.396 ms, prepare 10.086 ms, weak_pages 9.098 ms, trim 2.876 ms, roots 0.702 ms, allocation_census 0.149 ms, finish 0.061 ms, accounting 0.045 ms, resume_world 0.040 ms, verify 0.011 ms, stop_world 0.005 ms
- ('balanced', 1, 'inline256-pool256'): sweep_finalize 193.660 ms, trace 37.000 ms, prepare 9.368 ms, weak_pages 8.179 ms, trim 3.511 ms, roots 0.825 ms, allocation_census 0.276 ms, finish 0.065 ms, accounting 0.058 ms, resume_world 0.039 ms, verify 0.011 ms, stop_world 0.006 ms
- ('balanced', 8, 'inline0'): sweep_finalize 74.843 ms, trace 31.402 ms, prepare 7.462 ms, weak_pages 6.739 ms, trim 2.480 ms, roots 0.820 ms, allocation_census 0.269 ms, stop_world 0.226 ms, finish 0.060 ms, accounting 0.054 ms, resume_world 0.024 ms, verify 0.011 ms
- ('balanced', 8, 'inline128'): sweep_finalize 72.984 ms, trace 30.952 ms, prepare 7.588 ms, weak_pages 7.026 ms, trim 2.733 ms, roots 0.768 ms, allocation_census 0.257 ms, stop_world 0.238 ms, finish 0.055 ms, accounting 0.051 ms, resume_world 0.024 ms, verify 0.010 ms
- ('balanced', 8, 'inline256'): sweep_finalize 50.791 ms, trace 34.777 ms, prepare 9.422 ms, weak_pages 8.891 ms, trim 2.552 ms, roots 0.784 ms, stop_world 0.376 ms, allocation_census 0.182 ms, finish 0.055 ms, accounting 0.049 ms, resume_world 0.022 ms, verify 0.009 ms
- ('balanced', 8, 'inline256-pool256'): sweep_finalize 61.230 ms, trace 36.626 ms, prepare 8.588 ms, weak_pages 8.088 ms, trim 2.620 ms, roots 0.845 ms, allocation_census 0.279 ms, stop_world 0.134 ms, finish 0.058 ms, accounting 0.047 ms, resume_world 0.025 ms, verify 0.011 ms
- ('cpu', 1, 'inline0'): sweep_finalize 166.696 ms, trace 26.350 ms, prepare 7.724 ms, weak_pages 6.674 ms, trim 2.727 ms, roots 0.977 ms, allocation_census 0.250 ms, finish 0.062 ms, accounting 0.048 ms, resume_world 0.040 ms, verify 0.011 ms, stop_world 0.006 ms
- ('cpu', 1, 'inline128'): sweep_finalize 167.581 ms, trace 26.176 ms, prepare 7.770 ms, weak_pages 6.759 ms, trim 2.670 ms, roots 0.940 ms, allocation_census 0.255 ms, finish 0.066 ms, accounting 0.051 ms, resume_world 0.042 ms, verify 0.010 ms, stop_world 0.007 ms
- ('cpu', 1, 'inline256'): sweep_finalize 166.054 ms, trace 29.238 ms, prepare 9.600 ms, weak_pages 9.009 ms, trim 2.332 ms, roots 0.874 ms, allocation_census 0.158 ms, finish 0.057 ms, accounting 0.045 ms, resume_world 0.039 ms, verify 0.011 ms, stop_world 0.006 ms
- ('cpu', 1, 'inline256-pool256'): sweep_finalize 184.387 ms, trace 32.860 ms, prepare 8.655 ms, weak_pages 7.605 ms, trim 2.819 ms, roots 1.010 ms, allocation_census 0.467 ms, finish 0.068 ms, accounting 0.050 ms, resume_world 0.042 ms, verify 0.010 ms, stop_world 0.007 ms
- ('cpu', 8, 'inline0'): sweep_finalize 96.353 ms, trace 28.333 ms, prepare 7.387 ms, weak_pages 6.882 ms, trim 2.004 ms, roots 0.865 ms, allocation_census 0.274 ms, stop_world 0.159 ms, finish 0.063 ms, accounting 0.058 ms, resume_world 0.027 ms, verify 0.012 ms
- ('cpu', 8, 'inline128'): sweep_finalize 98.095 ms, trace 28.440 ms, prepare 7.746 ms, weak_pages 6.904 ms, trim 2.697 ms, roots 0.918 ms, stop_world 0.416 ms, allocation_census 0.276 ms, finish 0.061 ms, accounting 0.059 ms, resume_world 0.028 ms, verify 0.011 ms
- ('cpu', 8, 'inline256'): sweep_finalize 72.163 ms, trace 30.737 ms, trim 12.629 ms, prepare 9.358 ms, weak_pages 8.804 ms, roots 0.897 ms, allocation_census 0.215 ms, stop_world 0.186 ms, finish 0.062 ms, accounting 0.055 ms, resume_world 0.025 ms, verify 0.012 ms
- ('cpu', 8, 'inline256-pool256'): sweep_finalize 91.840 ms, trace 32.495 ms, prepare 8.280 ms, weak_pages 7.721 ms, trim 6.584 ms, roots 0.870 ms, stop_world 0.366 ms, allocation_census 0.281 ms, finish 0.058 ms, accounting 0.054 ms, resume_world 0.029 ms, verify 0.011 ms
- ('fanout', 1, 'inline0'): sweep_finalize 421.465 ms, trace 82.054 ms, prepare 18.611 ms, weak_pages 16.091 ms, trim 9.013 ms, roots 7.551 ms, allocation_census 0.289 ms, finish 0.136 ms, accounting 0.106 ms, resume_world 0.083 ms, verify 0.019 ms, stop_world 0.009 ms
- ('fanout', 1, 'inline128'): sweep_finalize 423.248 ms, trace 78.271 ms, prepare 18.675 ms, weak_pages 17.270 ms, trim 9.209 ms, roots 7.042 ms, allocation_census 0.298 ms, finish 0.143 ms, accounting 0.106 ms, resume_world 0.082 ms, verify 0.021 ms, stop_world 0.010 ms
- ('fanout', 1, 'inline256'): sweep_finalize 447.198 ms, trace 97.099 ms, prepare 21.477 ms, weak_pages 19.554 ms, roots 7.920 ms, trim 7.100 ms, allocation_census 0.311 ms, finish 0.154 ms, accounting 0.121 ms, resume_world 0.097 ms, verify 0.022 ms, stop_world 0.012 ms
- ('fanout', 1, 'inline256-pool256'): sweep_finalize 461.550 ms, trace 89.289 ms, prepare 19.711 ms, weak_pages 17.940 ms, trim 9.135 ms, roots 7.030 ms, allocation_census 0.288 ms, finish 0.131 ms, accounting 0.115 ms, resume_world 0.078 ms, verify 0.019 ms, stop_world 0.008 ms
- ('fanout', 8, 'inline0'): sweep_finalize 213.953 ms, trace 74.804 ms, trim 26.135 ms, prepare 18.595 ms, weak_pages 17.874 ms, roots 6.276 ms, allocation_census 0.435 ms, stop_world 0.220 ms, finish 0.126 ms, accounting 0.115 ms, resume_world 0.058 ms, verify 0.020 ms
- ('fanout', 8, 'inline128'): sweep_finalize 215.136 ms, trace 75.369 ms, prepare 17.895 ms, trim 16.333 ms, weak_pages 16.220 ms, roots 6.136 ms, allocation_census 0.452 ms, stop_world 0.399 ms, accounting 0.133 ms, finish 0.126 ms, resume_world 0.057 ms, verify 0.019 ms
- ('fanout', 8, 'inline256'): sweep_finalize 189.124 ms, trace 88.408 ms, prepare 20.031 ms, weak_pages 18.408 ms, trim 18.030 ms, roots 6.071 ms, stop_world 0.526 ms, allocation_census 0.400 ms, finish 0.123 ms, accounting 0.112 ms, resume_world 0.056 ms, verify 0.021 ms
- ('fanout', 8, 'inline256-pool256'): sweep_finalize 230.003 ms, trace 94.225 ms, prepare 18.704 ms, weak_pages 17.664 ms, roots 6.326 ms, trim 5.671 ms, allocation_census 0.478 ms, stop_world 0.335 ms, finish 0.119 ms, accounting 0.110 ms, resume_world 0.065 ms, verify 0.020 ms

Sweep work distribution (workload collections only):

Small helper jobs contain fewer than 4,096 objects; native thread creation is counted separately.
these counts alone do not measure thread-startup cost.

| Case | Workers | Variant | Collections / parallel | Helper jobs | New native threads | Small helper jobs | Median helper objects | Median largest-list share in parallel collections |
|---|---:|---|---:|---:|---:|---:|---:|---:|
| backpressure | 1 | inline0 | 66 / 0 | 0 | 0 | 0 | n/a | n/a |
| backpressure | 1 | inline128 | 66 / 0 | 0 | 0 | 0 | n/a | n/a |
| backpressure | 1 | inline256 | 66 / 0 | 0 | 0 | 0 | n/a | n/a |
| backpressure | 1 | inline256-pool256 | 66 / 0 | 0 | 0 | 0 | n/a | n/a |
| backpressure | 8 | inline0 | 63 / 63 | 441 | 441 | 0 | 17628 | 14.4% |
| backpressure | 8 | inline128 | 64 / 64 | 448 | 448 | 0 | 17351 | 14.5% |
| backpressure | 8 | inline256 | 63 / 63 | 441 | 441 | 0 | 17132 | 14.3% |
| backpressure | 8 | inline256-pool256 | 63 / 63 | 441 | 441 | 0 | 17421 | 14.1% |
| balanced | 1 | inline0 | 66 / 0 | 0 | 0 | 0 | n/a | n/a |
| balanced | 1 | inline128 | 66 / 0 | 0 | 0 | 0 | n/a | n/a |
| balanced | 1 | inline256 | 66 / 0 | 0 | 0 | 0 | n/a | n/a |
| balanced | 1 | inline256-pool256 | 66 / 0 | 0 | 0 | 0 | n/a | n/a |
| balanced | 8 | inline0 | 64 / 64 | 448 | 448 | 0 | 17024 | 14.5% |
| balanced | 8 | inline128 | 63 / 63 | 441 | 441 | 0 | 17152 | 14.5% |
| balanced | 8 | inline256 | 63 / 63 | 441 | 441 | 0 | 17207 | 14.5% |
| balanced | 8 | inline256-pool256 | 63 / 63 | 441 | 441 | 0 | 17013 | 14.5% |
| cpu | 1 | inline0 | 66 / 0 | 0 | 0 | 0 | n/a | n/a |
| cpu | 1 | inline128 | 66 / 0 | 0 | 0 | 0 | n/a | n/a |
| cpu | 1 | inline256 | 66 / 0 | 0 | 0 | 0 | n/a | n/a |
| cpu | 1 | inline256-pool256 | 66 / 0 | 0 | 0 | 0 | n/a | n/a |
| cpu | 8 | inline0 | 66 / 66 | 462 | 462 | 0 | 13436 | 24.3% |
| cpu | 8 | inline128 | 66 / 66 | 462 | 462 | 0 | 14490 | 23.1% |
| cpu | 8 | inline256 | 66 / 66 | 462 | 462 | 0 | 15114 | 23.3% |
| cpu | 8 | inline256-pool256 | 66 / 66 | 462 | 462 | 0 | 13768 | 26.0% |
| fanout | 1 | inline0 | 126 / 0 | 0 | 0 | 0 | n/a | n/a |
| fanout | 1 | inline128 | 126 / 0 | 0 | 0 | 0 | n/a | n/a |
| fanout | 1 | inline256 | 126 / 0 | 0 | 0 | 0 | n/a | n/a |
| fanout | 1 | inline256-pool256 | 126 / 0 | 0 | 0 | 0 | n/a | n/a |
| fanout | 8 | inline0 | 123 / 123 | 861 | 861 | 0 | 20411 | 13.8% |
| fanout | 8 | inline128 | 123 / 123 | 861 | 861 | 0 | 20471 | 13.8% |
| fanout | 8 | inline256 | 123 / 123 | 861 | 861 | 5 | 20577 | 13.7% |
| fanout | 8 | inline256-pool256 | 123 / 123 | 861 | 861 | 0 | 20488 | 13.8% |
