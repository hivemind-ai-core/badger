| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 453.559 [414.125, 577.675] | 60.560 | 59.1 |
| backpressure | 1 | candidate | 439.497 [388.621, 587.740] | 55.102 | 59.1 |
| backpressure | 2 | baseline | 289.531 [261.551, 451.256] | 45.090 | 68.4 |
| backpressure | 2 | candidate | 275.182 [253.356, 396.598] | 40.075 | 66.5 |
| backpressure | 4 | baseline | 161.595 [150.036, 185.947] | 29.773 | 73.9 |
| backpressure | 4 | candidate | 152.492 [140.535, 176.838] | 27.259 | 72.1 |
| backpressure | 8 | baseline | 113.039 [96.485, 142.301] | 24.339 | 76.2 |
| backpressure | 8 | candidate | 110.261 [96.113, 145.998] | 23.147 | 75.7 |
| balanced | 1 | baseline | 454.188 [404.792, 613.169] | 73.219 | 88.8 |
| balanced | 1 | candidate | 434.857 [395.944, 608.733] | 75.204 | 88.3 |
| balanced | 2 | baseline | 289.038 [258.786, 505.012] | 59.712 | 99.4 |
| balanced | 2 | candidate | 274.724 [246.019, 430.759] | 60.713 | 96.7 |
| balanced | 4 | baseline | 171.850 [157.560, 830.091] | 45.240 | 101.2 |
| balanced | 4 | candidate | 163.957 [145.243, 701.928] | 39.441 | 102.8 |
| balanced | 8 | baseline | 118.610 [103.584, 149.872] | 37.394 | 110.8 |
| balanced | 8 | candidate | 120.934 [100.908, 131.423] | 41.014 | 110.0 |
| cpu | 1 | baseline | 914.550 [840.653, 1786.215] | 78.169 | 82.2 |
| cpu | 1 | candidate | 892.820 [809.677, 1796.755] | 75.543 | 83.5 |
| cpu | 2 | baseline | 497.140 [457.020, 525.487] | 59.467 | 90.5 |
| cpu | 2 | candidate | 470.706 [457.790, 524.676] | 49.344 | 89.9 |
| cpu | 4 | baseline | 273.636 [258.541, 298.244] | 42.726 | 99.2 |
| cpu | 4 | candidate | 254.925 [244.445, 283.730] | 39.619 | 97.6 |
| cpu | 8 | baseline | 330.083 [288.033, 438.839] | 54.488 | 129.1 |
| cpu | 8 | candidate | 293.294 [236.987, 371.248] | 45.151 | 125.1 |
| fanout | 1 | baseline | 1348.140 [1087.571, 3353.903] | 202.446 | 86.5 |
| fanout | 1 | candidate | 1280.591 [1073.422, 1498.642] | 182.476 | 86.7 |
| fanout | 2 | baseline | 994.806 [824.261, 1108.231] | 172.823 | 103.8 |
| fanout | 2 | candidate | 868.288 [791.076, 1030.063] | 149.800 | 104.6 |
| fanout | 4 | baseline | 706.178 [675.394, 791.843] | 146.076 | 128.6 |
| fanout | 4 | candidate | 751.534 [662.962, 757.737] | 148.505 | 135.0 |
| fanout | 8 | baseline | 807.563 [746.612, 897.783] | 129.433 | 107.4 |
| fanout | 8 | candidate | 783.008 [695.189, 886.611] | 111.784 | 112.9 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| backpressure | 1 | -3.1% | -9.0% | +0.0% | 22 / 21 |
| backpressure | 2 | -5.0% | -11.1% | -2.8% | 22 / 21 |
| backpressure | 4 | -5.6% | -8.4% | -2.5% | 22 / 21 |
| backpressure | 8 | -2.5% | -4.9% | -0.6% | 21 / 21 |
| balanced | 1 | -4.3% | +2.7% | -0.5% | 22 / 21 |
| balanced | 2 | -5.0% | +1.7% | -2.7% | 22 / 21 |
| balanced | 4 | -4.6% | -12.8% | +1.5% | 22 / 21 |
| balanced | 8 | +2.0% | +9.7% | -0.8% | 21 / 21 |
| cpu | 1 | -2.4% | -3.4% | +1.6% | 22 / 21 |
| cpu | 2 | -5.3% | -17.0% | -0.7% | 22 / 21 |
| cpu | 4 | -6.8% | -7.3% | -1.6% | 22 / 21 |
| cpu | 8 | -11.1% | -17.1% | -3.1% | 22 / 21 |
| fanout | 1 | -5.0% | -9.9% | +0.2% | 42 / 40 |
| fanout | 2 | -12.7% | -13.3% | +0.9% | 42 / 40 |
| fanout | 4 | +6.4% | +1.7% | +5.0% | 42 / 40 |
| fanout | 8 | -3.0% | -13.6% | +5.2% | 41 / 40 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| backpressure | 1 | 7 | -3.1% [-7.2%, +1.7%] | 6 / 7 |
| backpressure | 2 | 7 | -3.7% [-12.1%, -1.4%] | 7 / 7 |
| backpressure | 4 | 7 | -6.2% [-12.6%, -1.5%] | 7 / 7 |
| backpressure | 8 | 7 | -2.5% [-8.4%, +4.7%] | 5 / 7 |
| balanced | 1 | 7 | -3.1% [-8.4%, +0.4%] | 6 / 7 |
| balanced | 2 | 7 | -6.6% [-18.5%, +12.6%] | 6 / 7 |
| balanced | 4 | 7 | -8.2% [-15.4%, -1.3%] | 7 / 7 |
| balanced | 8 | 7 | -4.1% [-19.3%, +23.4%] | 6 / 7 |
| cpu | 1 | 7 | -2.6% [-5.0%, +2.2%] | 5 / 7 |
| cpu | 2 | 7 | -5.0% [-7.5%, +0.8%] | 5 / 7 |
| cpu | 4 | 7 | -4.9% [-8.3%, -3.4%] | 7 / 7 |
| cpu | 8 | 7 | -15.4% [-28.0%, +1.8%] | 6 / 7 |
| fanout | 1 | 7 | -4.0% [-55.3%, -1.2%] | 7 / 7 |
| fanout | 2 | 7 | -7.8% [-13.6%, +3.3%] | 6 / 7 |
| fanout | 4 | 7 | -2.5% [-5.4%, +7.4%] | 5 / 7 |
| fanout | 8 | 7 | -4.8% [-10.6%, -1.2%] | 7 / 7 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 114688 | 1.088 | 5.120 |
| backpressure | 1 | candidate | 114688 | 1.024 | 4.608 |
| backpressure | 2 | baseline | 114688 | 0.512 | 3.584 |
| backpressure | 2 | candidate | 114688 | 0.496 | 3.456 |
| backpressure | 4 | baseline | 114688 | 0.272 | 1.792 |
| backpressure | 4 | candidate | 114688 | 0.248 | 1.728 |
| backpressure | 8 | baseline | 114688 | 0.176 | 1.536 |
| backpressure | 8 | candidate | 114688 | 0.168 | 1.664 |
| balanced | 1 | baseline | 114688 | 15.360 | 27.648 |
| balanced | 1 | candidate | 114688 | 14.336 | 31.744 |
| balanced | 2 | baseline | 114688 | 10.240 | 26.624 |
| balanced | 2 | candidate | 114688 | 9.216 | 26.624 |
| balanced | 4 | baseline | 114688 | 5.888 | 38.912 |
| balanced | 4 | candidate | 114688 | 5.376 | 34.816 |
| balanced | 8 | baseline | 114688 | 3.712 | 11.264 |
| balanced | 8 | candidate | 114688 | 3.584 | 13.312 |
| cpu | 1 | baseline | 114688 | 30.720 | 77.824 |
| cpu | 1 | candidate | 114688 | 30.720 | 81.920 |
| cpu | 2 | baseline | 114688 | 17.408 | 25.600 |
| cpu | 2 | candidate | 114688 | 15.872 | 26.624 |
| cpu | 4 | baseline | 114688 | 9.216 | 15.360 |
| cpu | 4 | candidate | 114688 | 8.704 | 16.384 |
| cpu | 8 | baseline | 114688 | 11.776 | 28.672 |
| cpu | 8 | candidate | 114688 | 9.728 | 20.480 |
| fanout | 1 | baseline | 114688 | 47.104 | 139.264 |
| fanout | 1 | candidate | 114688 | 43.008 | 69.632 |
| fanout | 2 | baseline | 114688 | 31.744 | 53.248 |
| fanout | 2 | candidate | 114688 | 29.696 | 51.200 |
| fanout | 4 | baseline | 114688 | 23.552 | 51.200 |
| fanout | 4 | candidate | 114688 | 22.528 | 55.296 |
| fanout | 8 | baseline | 114688 | 27.648 | 45.056 |
| fanout | 8 | candidate | 114688 | 26.624 | 43.008 |

Diagnostic GC phases summed across measured repetitions (wall time):
