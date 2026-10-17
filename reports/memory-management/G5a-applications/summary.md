| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 453.651 [405.172, 465.399] | 70.640 | 57.0 |
| backpressure | 1 | candidate | 445.981 [401.039, 452.976] | 66.204 | 54.9 |
| backpressure | 2 | baseline | 287.464 [283.777, 323.027] | 57.401 | 63.8 |
| backpressure | 2 | candidate | 291.322 [267.283, 311.903] | 54.127 | 65.1 |
| backpressure | 4 | baseline | 173.783 [172.538, 218.479] | 43.871 | 67.8 |
| backpressure | 4 | candidate | 185.682 [171.882, 887.620] | 42.663 | 68.5 |
| backpressure | 8 | baseline | 139.795 [122.122, 913.949] | 43.366 | 80.8 |
| backpressure | 8 | candidate | 125.218 [110.938, 669.774] | 36.999 | 79.8 |
| balanced | 1 | baseline | 435.634 [426.748, 1103.393] | 75.383 | 85.3 |
| balanced | 1 | candidate | 411.899 [405.263, 452.929] | 67.683 | 85.2 |
| balanced | 2 | baseline | 317.210 [278.477, 361.648] | 69.865 | 98.2 |
| balanced | 2 | candidate | 314.897 [284.919, 324.439] | 64.485 | 98.7 |
| balanced | 4 | baseline | 195.442 [184.308, 227.230] | 60.216 | 99.1 |
| balanced | 4 | candidate | 181.176 [167.100, 198.899] | 50.592 | 100.5 |
| balanced | 8 | baseline | 143.767 [132.763, 148.429] | 56.665 | 103.2 |
| balanced | 8 | candidate | 117.433 [114.978, 129.668] | 40.627 | 112.1 |
| cpu | 1 | baseline | 899.568 [819.921, 903.309] | 80.371 | 80.8 |
| cpu | 1 | candidate | 881.326 [807.770, 1061.064] | 75.682 | 79.5 |
| cpu | 2 | baseline | 504.572 [480.846, 1853.846] | 68.805 | 87.5 |
| cpu | 2 | candidate | 487.313 [460.935, 529.984] | 63.329 | 86.2 |
| cpu | 4 | baseline | 285.825 [273.512, 301.921] | 56.421 | 98.4 |
| cpu | 4 | candidate | 279.182 [261.579, 299.693] | 51.167 | 94.4 |
| cpu | 8 | baseline | 337.115 [314.356, 1030.799] | 64.642 | 126.2 |
| cpu | 8 | candidate | 305.855 [266.875, 352.855] | 50.909 | 126.4 |
| fanout | 1 | baseline | 1131.742 [1089.767, 3963.825] | 181.421 | 88.0 |
| fanout | 1 | candidate | 1079.604 [1039.789, 1190.291] | 174.229 | 87.5 |
| fanout | 2 | baseline | 999.784 [898.322, 1017.773] | 189.939 | 108.0 |
| fanout | 2 | candidate | 910.593 [805.662, 975.264] | 168.916 | 106.7 |
| fanout | 4 | baseline | 696.622 [656.598, 738.951] | 152.077 | 128.4 |
| fanout | 4 | candidate | 731.982 [663.166, 746.896] | 144.267 | 135.8 |
| fanout | 8 | baseline | 837.656 [753.623, 3489.913] | 148.878 | 106.3 |
| fanout | 8 | candidate | 785.035 [723.349, 801.881] | 122.521 | 116.2 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| backpressure | 1 | -1.7% | -6.3% | -3.8% | 22 / 22 |
| backpressure | 2 | +1.3% | -5.7% | +2.1% | 22 / 22 |
| backpressure | 4 | +6.8% | -2.8% | +1.1% | 22 / 22 |
| backpressure | 8 | -10.4% | -14.7% | -1.3% | 22 / 22 |
| balanced | 1 | -5.4% | -10.2% | -0.0% | 22 / 22 |
| balanced | 2 | -0.7% | -7.7% | +0.4% | 22 / 22 |
| balanced | 4 | -7.3% | -16.0% | +1.4% | 22 / 22 |
| balanced | 8 | -18.3% | -28.3% | +8.6% | 22 / 22 |
| cpu | 1 | -2.0% | -5.8% | -1.6% | 22 / 22 |
| cpu | 2 | -3.4% | -8.0% | -1.5% | 22 / 22 |
| cpu | 4 | -2.3% | -9.3% | -4.1% | 22 / 22 |
| cpu | 8 | -9.3% | -21.2% | +0.1% | 22 / 22 |
| fanout | 1 | -4.6% | -4.0% | -0.6% | 42 / 42 |
| fanout | 2 | -8.9% | -11.1% | -1.2% | 42 / 42 |
| fanout | 4 | +5.1% | -5.1% | +5.7% | 42 / 42 |
| fanout | 8 | -6.3% | -17.7% | +9.3% | 41 / 41 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 81920 | 0.992 | 4.608 |
| backpressure | 1 | candidate | 81920 | 0.992 | 4.352 |
| backpressure | 2 | baseline | 81920 | 0.496 | 3.712 |
| backpressure | 2 | candidate | 81920 | 0.496 | 3.328 |
| backpressure | 4 | baseline | 81920 | 0.288 | 2.688 |
| backpressure | 4 | candidate | 81920 | 0.304 | 7.424 |
| backpressure | 8 | baseline | 81920 | 0.208 | 8.704 |
| backpressure | 8 | candidate | 81920 | 0.224 | 9.728 |
| balanced | 1 | baseline | 81920 | 15.360 | 61.440 |
| balanced | 1 | candidate | 81920 | 14.336 | 26.624 |
| balanced | 2 | baseline | 81920 | 10.240 | 23.552 |
| balanced | 2 | candidate | 81920 | 10.240 | 21.504 |
| balanced | 4 | baseline | 81920 | 6.144 | 20.480 |
| balanced | 4 | candidate | 81920 | 5.888 | 14.848 |
| balanced | 8 | baseline | 81920 | 4.608 | 15.360 |
| balanced | 8 | candidate | 81920 | 3.712 | 12.800 |
| cpu | 1 | baseline | 81920 | 29.696 | 45.056 |
| cpu | 1 | candidate | 81920 | 30.720 | 53.248 |
| cpu | 2 | baseline | 81920 | 17.408 | 94.208 |
| cpu | 2 | candidate | 81920 | 16.384 | 26.624 |
| cpu | 4 | baseline | 81920 | 9.216 | 21.504 |
| cpu | 4 | candidate | 81920 | 9.216 | 19.456 |
| cpu | 8 | baseline | 81920 | 11.776 | 63.488 |
| cpu | 8 | candidate | 81920 | 10.240 | 21.504 |
| fanout | 1 | baseline | 81920 | 38.912 | 229.376 |
| fanout | 1 | candidate | 81920 | 36.864 | 53.248 |
| fanout | 2 | baseline | 81920 | 31.744 | 53.248 |
| fanout | 2 | candidate | 81920 | 29.696 | 53.248 |
| fanout | 4 | baseline | 81920 | 21.504 | 40.960 |
| fanout | 4 | candidate | 81920 | 22.528 | 43.008 |
| fanout | 8 | baseline | 81920 | 27.648 | 172.032 |
| fanout | 8 | candidate | 81920 | 25.600 | 49.152 |

Diagnostic GC phases summed across measured repetitions (wall time):
