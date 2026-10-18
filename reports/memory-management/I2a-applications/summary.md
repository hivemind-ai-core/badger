| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 427.178 [419.696, 446.060] | 53.918 | 59.3 |
| backpressure | 1 | candidate | 412.638 [396.680, 421.658] | 47.583 | 59.3 |
| backpressure | 2 | baseline | 272.413 [261.870, 276.957] | 40.193 | 66.7 |
| backpressure | 2 | candidate | 260.319 [252.116, 262.239] | 35.736 | 66.8 |
| backpressure | 4 | baseline | 157.083 [152.860, 158.824] | 28.367 | 74.2 |
| backpressure | 4 | candidate | 149.187 [143.011, 159.262] | 24.962 | 74.3 |
| backpressure | 8 | baseline | 111.726 [109.685, 120.364] | 23.301 | 73.6 |
| backpressure | 8 | candidate | 106.295 [103.209, 109.055] | 21.794 | 73.9 |
| balanced | 1 | baseline | 421.299 [413.337, 434.685] | 65.679 | 88.4 |
| balanced | 1 | candidate | 417.437 [411.443, 428.126] | 68.180 | 88.2 |
| balanced | 2 | baseline | 296.241 [272.937, 320.240] | 55.813 | 99.9 |
| balanced | 2 | candidate | 272.172 [249.044, 304.914] | 52.411 | 97.6 |
| balanced | 4 | baseline | 160.472 [159.948, 197.127] | 40.417 | 100.4 |
| balanced | 4 | candidate | 152.758 [150.042, 169.306] | 37.612 | 100.6 |
| balanced | 8 | baseline | 115.731 [111.431, 119.815] | 35.064 | 110.0 |
| balanced | 8 | candidate | 109.075 [106.249, 112.871] | 33.871 | 110.6 |
| cpu | 1 | baseline | 865.976 [854.448, 886.901] | 66.117 | 82.6 |
| cpu | 1 | candidate | 856.687 [840.715, 876.199] | 63.488 | 83.7 |
| cpu | 2 | baseline | 485.419 [475.243, 498.800] | 51.744 | 90.3 |
| cpu | 2 | candidate | 472.793 [454.270, 502.680] | 45.065 | 89.1 |
| cpu | 4 | baseline | 263.340 [256.988, 271.805] | 38.106 | 96.3 |
| cpu | 4 | candidate | 253.394 [247.959, 265.339] | 35.497 | 97.2 |
| cpu | 8 | baseline | 330.001 [285.247, 359.504] | 41.158 | 127.4 |
| cpu | 8 | candidate | 288.832 [265.265, 320.926] | 36.119 | 124.5 |
| fanout | 1 | baseline | 1125.361 [1092.013, 1241.562] | 171.306 | 86.9 |
| fanout | 1 | candidate | 1094.110 [1060.806, 1160.373] | 161.776 | 86.9 |
| fanout | 2 | baseline | 879.191 [841.466, 938.832] | 149.825 | 107.3 |
| fanout | 2 | candidate | 783.870 [769.293, 903.178] | 136.187 | 102.5 |
| fanout | 4 | baseline | 694.894 [640.712, 740.164] | 140.323 | 125.6 |
| fanout | 4 | candidate | 670.543 [648.834, 726.629] | 133.188 | 132.1 |
| fanout | 8 | baseline | 786.015 [777.257, 813.211] | 109.206 | 109.7 |
| fanout | 8 | candidate | 760.251 [752.573, 800.188] | 104.380 | 115.4 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| backpressure | 1 | -3.4% | -11.7% | -0.1% | 22 / 21 |
| backpressure | 2 | -4.4% | -11.1% | +0.3% | 22 / 21 |
| backpressure | 4 | -5.0% | -12.0% | +0.1% | 22 / 21 |
| backpressure | 8 | -4.9% | -6.5% | +0.5% | 21 / 21 |
| balanced | 1 | -0.9% | +3.8% | -0.2% | 22 / 21 |
| balanced | 2 | -8.1% | -6.1% | -2.3% | 22 / 21 |
| balanced | 4 | -4.8% | -6.9% | +0.2% | 22 / 21 |
| balanced | 8 | -5.8% | -3.4% | +0.6% | 21 / 21 |
| cpu | 1 | -1.1% | -4.0% | +1.3% | 22 / 21 |
| cpu | 2 | -2.6% | -12.9% | -1.4% | 22 / 21 |
| cpu | 4 | -3.8% | -6.8% | +0.9% | 22 / 21 |
| cpu | 8 | -12.5% | -12.2% | -2.3% | 22 / 21 |
| fanout | 1 | -2.8% | -5.6% | -0.0% | 42 / 40 |
| fanout | 2 | -10.8% | -9.1% | -4.5% | 42 / 40 |
| fanout | 4 | -3.5% | -5.1% | +5.2% | 42 / 40 |
| fanout | 8 | -3.3% | -4.4% | +5.2% | 41 / 40 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| backpressure | 1 | 7 | -3.7% [-6.9%, -2.1%] | 7 / 7 |
| backpressure | 2 | 7 | -4.4% [-7.9%, -3.7%] | 7 / 7 |
| backpressure | 4 | 7 | -5.7% [-8.6%, +3.8%] | 6 / 7 |
| backpressure | 8 | 7 | -5.3% [-9.4%, -2.6%] | 7 / 7 |
| balanced | 1 | 7 | -1.5% [-2.7%, +1.8%] | 5 / 7 |
| balanced | 2 | 7 | -4.8% [-19.0%, +9.0%] | 5 / 7 |
| balanced | 4 | 7 | -4.7% [-23.0%, +3.2%] | 6 / 7 |
| balanced | 8 | 7 | -4.7% [-10.3%, -3.7%] | 7 / 7 |
| cpu | 1 | 7 | -1.5% [-3.5%, +1.9%] | 4 / 7 |
| cpu | 2 | 7 | -0.7% [-8.9%, +3.6%] | 4 / 7 |
| cpu | 4 | 7 | -3.0% [-4.7%, -2.0%] | 7 / 7 |
| cpu | 8 | 7 | -11.7% [-18.1%, +3.8%] | 6 / 7 |
| fanout | 1 | 7 | -3.9% [-7.6%, +6.3%] | 5 / 7 |
| fanout | 2 | 7 | -10.0% [-17.5%, +3.9%] | 6 / 7 |
| fanout | 4 | 7 | -1.9% [-7.5%, +1.3%] | 6 / 7 |
| fanout | 8 | 7 | -2.9% [-4.5%, -0.3%] | 7 / 7 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 114688 | 0.992 | 3.840 |
| backpressure | 1 | candidate | 114688 | 0.960 | 3.712 |
| backpressure | 2 | baseline | 114688 | 0.480 | 2.560 |
| backpressure | 2 | candidate | 114688 | 0.464 | 2.432 |
| backpressure | 4 | baseline | 114688 | 0.256 | 1.600 |
| backpressure | 4 | candidate | 114688 | 0.248 | 1.536 |
| backpressure | 8 | baseline | 114688 | 0.176 | 1.344 |
| backpressure | 8 | candidate | 114688 | 0.168 | 1.280 |
| balanced | 1 | baseline | 114688 | 14.336 | 23.552 |
| balanced | 1 | candidate | 114688 | 13.824 | 29.696 |
| balanced | 2 | baseline | 114688 | 9.728 | 18.432 |
| balanced | 2 | candidate | 114688 | 9.216 | 21.504 |
| balanced | 4 | baseline | 114688 | 5.376 | 12.288 |
| balanced | 4 | candidate | 114688 | 5.120 | 11.776 |
| balanced | 8 | baseline | 114688 | 3.840 | 9.728 |
| balanced | 8 | candidate | 114688 | 3.584 | 9.728 |
| cpu | 1 | baseline | 114688 | 29.696 | 38.912 |
| cpu | 1 | candidate | 114688 | 29.696 | 43.008 |
| cpu | 2 | baseline | 114688 | 16.384 | 24.576 |
| cpu | 2 | candidate | 114688 | 15.872 | 25.600 |
| cpu | 4 | baseline | 114688 | 8.704 | 14.848 |
| cpu | 4 | candidate | 114688 | 8.704 | 14.336 |
| cpu | 8 | baseline | 114688 | 10.752 | 21.504 |
| cpu | 8 | candidate | 114688 | 9.728 | 19.456 |
| fanout | 1 | baseline | 114688 | 38.912 | 55.296 |
| fanout | 1 | candidate | 114688 | 38.912 | 51.200 |
| fanout | 2 | baseline | 114688 | 30.720 | 43.008 |
| fanout | 2 | candidate | 114688 | 27.648 | 40.960 |
| fanout | 4 | baseline | 114688 | 21.504 | 49.152 |
| fanout | 4 | candidate | 114688 | 21.504 | 49.152 |
| fanout | 8 | baseline | 114688 | 26.624 | 38.912 |
| fanout | 8 | candidate | 114688 | 25.600 | 34.816 |

Diagnostic GC phases summed across measured repetitions (wall time):
