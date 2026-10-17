| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 430.395 [399.801, 463.405] | 57.176 | 58.8 |
| backpressure | 1 | candidate | 432.746 [411.803, 493.817] | 55.234 | 58.7 |
| backpressure | 2 | baseline | 272.433 [260.865, 279.375] | 43.176 | 66.1 |
| backpressure | 2 | candidate | 264.321 [255.652, 280.643] | 41.251 | 66.4 |
| backpressure | 4 | baseline | 161.235 [151.267, 169.815] | 30.641 | 75.4 |
| backpressure | 4 | candidate | 155.880 [151.335, 175.969] | 29.173 | 70.9 |
| backpressure | 8 | baseline | 107.086 [101.935, 111.534] | 21.937 | 73.4 |
| backpressure | 8 | candidate | 108.797 [95.519, 112.108] | 22.770 | 73.2 |
| balanced | 1 | baseline | 436.745 [386.438, 492.540] | 71.165 | 87.9 |
| balanced | 1 | candidate | 432.339 [391.991, 487.325] | 69.676 | 87.8 |
| balanced | 2 | baseline | 304.655 [270.922, 1139.200] | 61.712 | 96.9 |
| balanced | 2 | candidate | 315.655 [272.553, 1106.513] | 55.356 | 100.3 |
| balanced | 4 | baseline | 168.429 [151.767, 741.252] | 42.098 | 106.8 |
| balanced | 4 | candidate | 165.426 [157.699, 721.186] | 41.305 | 101.1 |
| balanced | 8 | baseline | 111.376 [103.049, 113.061] | 33.934 | 110.1 |
| balanced | 8 | candidate | 111.489 [103.919, 127.481] | 34.164 | 108.2 |
| cpu | 1 | baseline | 852.362 [792.528, 924.505] | 69.393 | 81.6 |
| cpu | 1 | candidate | 842.937 [806.135, 937.348] | 69.472 | 81.7 |
| cpu | 2 | baseline | 476.944 [468.734, 499.169] | 53.439 | 91.1 |
| cpu | 2 | candidate | 491.381 [475.427, 501.718] | 54.807 | 90.8 |
| cpu | 4 | baseline | 260.667 [255.383, 303.042] | 39.381 | 99.9 |
| cpu | 4 | candidate | 265.302 [256.862, 314.289] | 40.137 | 100.6 |
| cpu | 8 | baseline | 322.224 [301.375, 338.089] | 44.807 | 124.5 |
| cpu | 8 | candidate | 312.648 [275.106, 376.420] | 41.029 | 127.0 |
| fanout | 1 | baseline | 1148.051 [1067.607, 1382.627] | 178.439 | 85.9 |
| fanout | 1 | candidate | 1124.370 [1072.968, 1434.507] | 178.151 | 86.0 |
| fanout | 2 | baseline | 888.164 [772.692, 1033.774] | 154.381 | 109.0 |
| fanout | 2 | candidate | 882.640 [792.820, 987.728] | 158.613 | 108.7 |
| fanout | 4 | baseline | 705.682 [616.945, 952.468] | 148.694 | 131.1 |
| fanout | 4 | candidate | 693.785 [631.544, 896.629] | 151.311 | 133.7 |
| fanout | 8 | baseline | 747.026 [714.923, 2971.892] | 111.968 | 116.8 |
| fanout | 8 | candidate | 756.717 [723.165, 1214.084] | 119.350 | 112.8 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| backpressure | 1 | +0.5% | -3.4% | -0.2% | 22 / 22 |
| backpressure | 2 | -3.0% | -4.5% | +0.4% | 22 / 22 |
| backpressure | 4 | -3.3% | -4.8% | -5.9% | 22 / 22 |
| backpressure | 8 | +1.6% | +3.8% | -0.3% | 21 / 21 |
| balanced | 1 | -1.0% | -2.1% | -0.1% | 22 / 22 |
| balanced | 2 | +3.6% | -10.3% | +3.5% | 22 / 22 |
| balanced | 4 | -1.8% | -1.9% | -5.4% | 22 / 22 |
| balanced | 8 | +0.1% | +0.7% | -1.8% | 21 / 21 |
| cpu | 1 | -1.1% | +0.1% | +0.1% | 22 / 22 |
| cpu | 2 | +3.0% | +2.6% | -0.3% | 22 / 22 |
| cpu | 4 | +1.8% | +1.9% | +0.7% | 22 / 22 |
| cpu | 8 | -3.0% | -8.4% | +2.0% | 22 / 22 |
| fanout | 1 | -2.1% | -0.2% | +0.1% | 42 / 42 |
| fanout | 2 | -0.6% | +2.7% | -0.2% | 42 / 42 |
| fanout | 4 | -1.7% | +1.8% | +2.0% | 42 / 42 |
| fanout | 8 | +1.3% | +6.6% | -3.4% | 41 / 41 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| backpressure | 1 | 7 | +1.8% [-5.1%, +6.6%] | 2 / 7 |
| backpressure | 2 | 7 | -2.0% [-4.9%, +2.3%] | 5 / 7 |
| backpressure | 4 | 7 | -1.6% [-8.4%, +10.7%] | 4 / 7 |
| backpressure | 8 | 7 | -0.0% [-8.8%, +4.3%] | 4 / 7 |
| balanced | 1 | 7 | -1.0% [-3.7%, +1.4%] | 5 / 7 |
| balanced | 2 | 7 | -2.9% [-14.9%, +14.9%] | 4 / 7 |
| balanced | 4 | 7 | -1.0% [-9.4%, +9.3%] | 4 / 7 |
| balanced | 8 | 7 | +2.2% [-5.8%, +14.5%] | 3 / 7 |
| cpu | 1 | 7 | +0.0% [-4.9%, +8.9%] | 3 / 7 |
| cpu | 2 | 7 | +1.9% [-2.9%, +6.9%] | 2 / 7 |
| cpu | 4 | 7 | +1.6% [-0.2%, +5.0%] | 1 / 7 |
| cpu | 8 | 7 | +1.2% [-17.4%, +16.8%] | 3 / 7 |
| fanout | 1 | 7 | +0.5% [-15.6%, +3.8%] | 2 / 7 |
| fanout | 2 | 7 | +1.8% [-14.6%, +11.8%] | 3 / 7 |
| fanout | 4 | 7 | +0.1% [-10.5%, +10.9%] | 3 / 7 |
| fanout | 8 | 7 | -1.0% [-59.1%, +5.9%] | 4 / 7 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 114688 | 0.992 | 3.968 |
| backpressure | 1 | candidate | 114688 | 0.992 | 4.352 |
| backpressure | 2 | baseline | 114688 | 0.480 | 2.688 |
| backpressure | 2 | candidate | 114688 | 0.464 | 2.688 |
| backpressure | 4 | baseline | 114688 | 0.256 | 1.728 |
| backpressure | 4 | candidate | 114688 | 0.256 | 1.664 |
| backpressure | 8 | baseline | 114688 | 0.160 | 1.280 |
| backpressure | 8 | candidate | 114688 | 0.160 | 1.280 |
| balanced | 1 | baseline | 114688 | 14.336 | 26.624 |
| balanced | 1 | candidate | 114688 | 14.336 | 24.576 |
| balanced | 2 | baseline | 114688 | 10.752 | 47.104 |
| balanced | 2 | candidate | 114688 | 10.240 | 53.248 |
| balanced | 4 | baseline | 114688 | 5.376 | 36.864 |
| balanced | 4 | candidate | 114688 | 5.632 | 32.768 |
| balanced | 8 | baseline | 114688 | 3.584 | 9.216 |
| balanced | 8 | candidate | 114688 | 3.712 | 9.728 |
| cpu | 1 | baseline | 114688 | 28.672 | 38.912 |
| cpu | 1 | candidate | 114688 | 28.672 | 40.960 |
| cpu | 2 | baseline | 114688 | 16.384 | 24.576 |
| cpu | 2 | candidate | 114688 | 16.384 | 26.624 |
| cpu | 4 | baseline | 114688 | 8.704 | 14.848 |
| cpu | 4 | candidate | 114688 | 9.216 | 17.408 |
| cpu | 8 | baseline | 114688 | 10.752 | 27.648 |
| cpu | 8 | candidate | 114688 | 10.752 | 19.456 |
| fanout | 1 | baseline | 114688 | 40.960 | 59.392 |
| fanout | 1 | candidate | 114688 | 38.912 | 61.440 |
| fanout | 2 | baseline | 114688 | 30.720 | 53.248 |
| fanout | 2 | candidate | 114688 | 30.720 | 45.056 |
| fanout | 4 | baseline | 114688 | 23.552 | 59.392 |
| fanout | 4 | candidate | 114688 | 23.552 | 55.296 |
| fanout | 8 | baseline | 114688 | 25.600 | 118.784 |
| fanout | 8 | candidate | 114688 | 25.600 | 63.488 |

Diagnostic GC phases summed across measured repetitions (wall time):
