| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 415.316 [388.702, 451.660] | 54.650 | 55.0 |
| backpressure | 1 | candidate | 415.573 [385.665, 451.666] | 52.654 | 58.6 |
| backpressure | 4 | baseline | 177.980 [156.692, 209.217] | 40.654 | 68.5 |
| backpressure | 4 | candidate | 156.381 [140.905, 341.482] | 29.947 | 73.3 |
| backpressure | 8 | baseline | 119.040 [113.372, 137.400] | 32.620 | 79.8 |
| backpressure | 8 | candidate | 107.568 [98.309, 123.298] | 22.280 | 73.3 |
| balanced | 1 | baseline | 420.677 [385.332, 454.548] | 69.467 | 83.8 |
| balanced | 1 | candidate | 421.431 [381.187, 433.311] | 67.396 | 87.9 |
| balanced | 4 | baseline | 172.325 [158.540, 225.172] | 47.595 | 97.0 |
| balanced | 4 | candidate | 161.995 [149.201, 350.325] | 40.022 | 98.5 |
| balanced | 8 | baseline | 120.749 [108.647, 154.464] | 40.013 | 109.6 |
| balanced | 8 | candidate | 108.306 [100.125, 183.501] | 33.890 | 109.0 |
| cpu | 1 | baseline | 840.565 [784.294, 972.202] | 70.166 | 77.5 |
| cpu | 1 | candidate | 830.147 [785.212, 954.354] | 65.730 | 81.7 |
| cpu | 4 | baseline | 273.385 [253.415, 388.070] | 46.952 | 92.4 |
| cpu | 4 | candidate | 266.584 [245.411, 324.585] | 46.278 | 98.6 |
| cpu | 8 | baseline | 307.341 [290.656, 335.569] | 46.202 | 125.8 |
| cpu | 8 | candidate | 323.552 [276.533, 350.083] | 41.143 | 128.4 |
| fanout | 1 | baseline | 1140.231 [1010.849, 1417.487] | 178.974 | 85.9 |
| fanout | 1 | candidate | 1113.951 [1025.009, 1604.108] | 175.328 | 86.1 |
| fanout | 4 | baseline | 700.544 [651.043, 720.856] | 130.358 | 127.8 |
| fanout | 4 | candidate | 655.411 [608.925, 800.181] | 135.299 | 130.9 |
| fanout | 8 | baseline | 752.908 [719.482, 1187.216] | 111.886 | 108.4 |
| fanout | 8 | candidate | 758.607 [704.474, 2029.594] | 109.569 | 112.8 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| backpressure | 1 | +0.1% | -3.7% | +6.5% | 22 / 22 |
| backpressure | 4 | -12.1% | -26.3% | +7.0% | 22 / 22 |
| backpressure | 8 | -9.6% | -31.7% | -8.1% | 21 / 21 |
| balanced | 1 | +0.2% | -3.0% | +4.9% | 22 / 22 |
| balanced | 4 | -6.0% | -15.9% | +1.6% | 22 / 22 |
| balanced | 8 | -10.3% | -15.3% | -0.6% | 21 / 21 |
| cpu | 1 | -1.2% | -6.3% | +5.4% | 22 / 22 |
| cpu | 4 | -2.5% | -1.4% | +6.7% | 22 / 22 |
| cpu | 8 | +5.3% | -10.9% | +2.1% | 22 / 22 |
| fanout | 1 | -2.3% | -2.0% | +0.2% | 42 / 42 |
| fanout | 4 | -6.4% | +3.8% | +2.4% | 42 / 42 |
| fanout | 8 | +0.8% | -2.1% | +4.0% | 41 / 41 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| backpressure | 1 | 9 | -0.9% [-5.6%, +8.8%] | 7 / 9 |
| backpressure | 4 | 9 | -10.1% [-24.2%, +63.2%] | 8 / 9 |
| backpressure | 8 | 9 | -10.6% [-14.3%, -3.8%] | 9 / 9 |
| balanced | 1 | 9 | -2.6% [-7.5%, +4.3%] | 5 / 9 |
| balanced | 4 | 9 | -7.5% [-13.4%, +55.6%] | 6 / 9 |
| balanced | 8 | 9 | -8.4% [-13.3%, +18.8%] | 8 / 9 |
| cpu | 1 | 9 | -1.3% [-3.8%, +11.6%] | 6 / 9 |
| cpu | 4 | 9 | -3.2% [-16.4%, +4.4%] | 7 / 9 |
| cpu | 8 | 9 | +4.7% [-5.2%, +11.7%] | 3 / 9 |
| fanout | 1 | 9 | +1.4% [-10.1%, +13.2%] | 3 / 9 |
| fanout | 4 | 9 | -7.2% [-13.5%, +13.4%] | 7 / 9 |
| fanout | 8 | 9 | -1.4% [-21.4%, +71.0%] | 5 / 9 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 147456 | 0.960 | 4.096 |
| backpressure | 1 | candidate | 147456 | 0.960 | 3.840 |
| backpressure | 4 | baseline | 147456 | 0.272 | 2.432 |
| backpressure | 4 | candidate | 147456 | 0.272 | 2.688 |
| backpressure | 8 | baseline | 147456 | 0.168 | 1.792 |
| backpressure | 8 | candidate | 147456 | 0.168 | 1.344 |
| balanced | 1 | baseline | 147456 | 14.336 | 26.624 |
| balanced | 1 | candidate | 147456 | 13.824 | 23.552 |
| balanced | 4 | baseline | 147456 | 5.632 | 14.848 |
| balanced | 4 | candidate | 147456 | 5.376 | 19.456 |
| balanced | 8 | baseline | 147456 | 3.712 | 12.800 |
| balanced | 8 | candidate | 147456 | 3.584 | 13.312 |
| cpu | 1 | baseline | 147456 | 28.672 | 40.960 |
| cpu | 1 | candidate | 147456 | 28.672 | 43.008 |
| cpu | 4 | baseline | 147456 | 9.216 | 21.504 |
| cpu | 4 | candidate | 147456 | 8.704 | 20.480 |
| cpu | 8 | baseline | 147456 | 10.240 | 19.456 |
| cpu | 8 | candidate | 147456 | 10.752 | 18.432 |
| fanout | 1 | baseline | 147456 | 38.912 | 61.440 |
| fanout | 1 | candidate | 147456 | 38.912 | 63.488 |
| fanout | 4 | baseline | 147456 | 22.528 | 40.960 |
| fanout | 4 | candidate | 147456 | 21.504 | 45.056 |
| fanout | 8 | baseline | 147456 | 25.600 | 55.296 |
| fanout | 8 | candidate | 147456 | 25.600 | 118.784 |

Diagnostic GC phases summed across measured repetitions (wall time):
