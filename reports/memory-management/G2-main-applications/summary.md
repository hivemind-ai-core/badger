| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 473.853 [446.186, 642.774] | 85.018 | 52.8 |
| backpressure | 1 | candidate | 450.080 [443.725, 474.329] | 71.398 | 56.4 |
| backpressure | 2 | baseline | 340.542 [306.682, 476.989] | 79.455 | 59.1 |
| backpressure | 2 | candidate | 321.878 [301.758, 372.089] | 67.897 | 63.5 |
| backpressure | 4 | baseline | 199.993 [186.276, 215.610] | 53.246 | 66.2 |
| backpressure | 4 | candidate | 193.946 [183.174, 273.053] | 49.511 | 67.9 |
| backpressure | 8 | baseline | 142.479 [136.701, 278.665] | 45.670 | 70.9 |
| backpressure | 8 | candidate | 145.405 [128.576, 161.240] | 45.832 | 80.3 |
| balanced | 1 | baseline | 489.420 [461.667, 547.099] | 97.228 | 82.8 |
| balanced | 1 | candidate | 460.216 [448.068, 1130.015] | 82.301 | 85.1 |
| balanced | 2 | baseline | 387.425 [324.824, 481.420] | 92.285 | 92.0 |
| balanced | 2 | candidate | 334.907 [327.201, 1109.049] | 79.027 | 101.4 |
| balanced | 4 | baseline | 209.730 [201.514, 1023.061] | 68.053 | 90.8 |
| balanced | 4 | candidate | 194.421 [189.894, 210.150] | 60.994 | 96.4 |
| balanced | 8 | baseline | 157.641 [142.334, 281.331] | 61.637 | 93.7 |
| balanced | 8 | candidate | 149.351 [141.521, 477.297] | 58.325 | 103.5 |
| cpu | 1 | baseline | 923.107 [885.771, 1178.962] | 94.814 | 79.1 |
| cpu | 1 | candidate | 927.922 [876.240, 940.789] | 83.559 | 80.8 |
| cpu | 2 | baseline | 520.494 [507.987, 530.866] | 78.381 | 87.2 |
| cpu | 2 | candidate | 543.781 [506.535, 571.080] | 73.090 | 89.4 |
| cpu | 4 | baseline | 298.083 [284.279, 439.253] | 61.277 | 90.8 |
| cpu | 4 | candidate | 301.096 [281.877, 407.751] | 63.566 | 99.8 |
| cpu | 8 | baseline | 330.641 [311.697, 644.294] | 64.594 | 123.8 |
| cpu | 8 | candidate | 333.207 [317.303, 1116.140] | 64.925 | 128.3 |
| fanout | 1 | baseline | 1186.090 [1155.494, 1309.066] | 220.077 | 83.2 |
| fanout | 1 | candidate | 1291.536 [1187.824, 3774.455] | 205.448 | 88.0 |
| fanout | 2 | baseline | 1048.790 [992.975, 1252.690] | 218.686 | 99.1 |
| fanout | 2 | candidate | 1039.878 [890.303, 3519.047] | 196.537 | 109.4 |
| fanout | 4 | baseline | 768.336 [720.879, 789.619] | 170.889 | 110.7 |
| fanout | 4 | candidate | 744.663 [674.314, 812.078] | 160.474 | 127.1 |
| fanout | 8 | baseline | 849.348 [836.175, 1148.840] | 158.934 | 96.3 |
| fanout | 8 | candidate | 818.753 [764.217, 888.937] | 143.280 | 105.8 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| backpressure | 1 | -5.0% | -16.0% | +6.8% | 22 / 22 |
| backpressure | 2 | -5.5% | -14.5% | +7.4% | 22 / 22 |
| backpressure | 4 | -3.0% | -7.0% | +2.7% | 22 / 22 |
| backpressure | 8 | +2.1% | +0.4% | +13.2% | 22 / 22 |
| balanced | 1 | -6.0% | -15.4% | +2.8% | 22 / 22 |
| balanced | 2 | -13.6% | -14.4% | +10.3% | 22 / 22 |
| balanced | 4 | -7.3% | -10.4% | +6.2% | 22 / 22 |
| balanced | 8 | -5.3% | -5.4% | +10.4% | 22 / 22 |
| cpu | 1 | +0.5% | -11.9% | +2.2% | 22 / 22 |
| cpu | 2 | +4.5% | -6.8% | +2.5% | 22 / 22 |
| cpu | 4 | +1.0% | +3.7% | +9.9% | 22 / 22 |
| cpu | 8 | +0.8% | +0.5% | +3.6% | 22 / 22 |
| fanout | 1 | +8.9% | -6.6% | +5.7% | 42 / 42 |
| fanout | 2 | -0.8% | -10.1% | +10.4% | 42 / 42 |
| fanout | 4 | -3.1% | -6.1% | +14.8% | 42 / 42 |
| fanout | 8 | -3.6% | -9.8% | +9.9% | 41 / 41 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 114688 | 1.024 | 6.656 |
| backpressure | 1 | candidate | 114688 | 0.992 | 5.120 |
| backpressure | 2 | baseline | 114688 | 0.576 | 5.120 |
| backpressure | 2 | candidate | 114688 | 0.544 | 4.352 |
| backpressure | 4 | baseline | 114688 | 0.288 | 3.200 |
| backpressure | 4 | candidate | 114688 | 0.304 | 3.456 |
| backpressure | 8 | baseline | 114688 | 0.200 | 3.456 |
| backpressure | 8 | candidate | 114688 | 0.192 | 2.432 |
| balanced | 1 | baseline | 114688 | 16.384 | 32.768 |
| balanced | 1 | candidate | 114688 | 15.872 | 51.200 |
| balanced | 2 | baseline | 114688 | 12.800 | 29.696 |
| balanced | 2 | candidate | 114688 | 11.776 | 53.248 |
| balanced | 4 | baseline | 114688 | 6.912 | 57.344 |
| balanced | 4 | candidate | 114688 | 6.400 | 19.456 |
| balanced | 8 | baseline | 114688 | 4.864 | 20.480 |
| balanced | 8 | candidate | 114688 | 4.608 | 36.864 |
| cpu | 1 | baseline | 114688 | 31.744 | 59.392 |
| cpu | 1 | candidate | 114688 | 31.744 | 45.056 |
| cpu | 2 | baseline | 114688 | 17.408 | 30.720 |
| cpu | 2 | candidate | 114688 | 17.408 | 31.744 |
| cpu | 4 | baseline | 114688 | 10.240 | 25.600 |
| cpu | 4 | candidate | 114688 | 9.728 | 30.720 |
| cpu | 8 | baseline | 114688 | 10.752 | 51.200 |
| cpu | 8 | candidate | 114688 | 11.776 | 61.440 |
| fanout | 1 | baseline | 114688 | 40.960 | 59.392 |
| fanout | 1 | candidate | 114688 | 45.056 | 180.224 |
| fanout | 2 | baseline | 114688 | 34.816 | 77.824 |
| fanout | 2 | candidate | 114688 | 36.864 | 180.224 |
| fanout | 4 | baseline | 114688 | 24.576 | 47.104 |
| fanout | 4 | candidate | 114688 | 23.552 | 47.104 |
| fanout | 8 | baseline | 114688 | 28.672 | 57.344 |
| fanout | 8 | candidate | 114688 | 26.624 | 45.056 |

Diagnostic GC phases summed across measured repetitions (wall time):
