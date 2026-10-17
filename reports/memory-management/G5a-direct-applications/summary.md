| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 426.021 [420.894, 437.240] | 64.440 | 56.3 |
| backpressure | 1 | candidate | 411.720 [407.734, 417.674] | 56.900 | 55.1 |
| backpressure | 2 | baseline | 289.542 [287.551, 302.469] | 58.047 | 63.8 |
| backpressure | 2 | candidate | 274.803 [270.906, 284.136] | 49.774 | 63.0 |
| backpressure | 4 | baseline | 179.337 [173.951, 191.947] | 45.004 | 71.4 |
| backpressure | 4 | candidate | 165.795 [161.871, 178.952] | 37.794 | 68.9 |
| backpressure | 8 | baseline | 132.110 [127.687, 146.987] | 40.030 | 82.3 |
| backpressure | 8 | candidate | 116.770 [114.261, 119.809] | 31.179 | 79.6 |
| balanced | 1 | baseline | 433.581 [427.046, 441.298] | 75.003 | 85.2 |
| balanced | 1 | candidate | 413.924 [407.679, 427.942] | 67.864 | 83.7 |
| balanced | 2 | baseline | 317.288 [285.368, 359.480] | 69.770 | 95.4 |
| balanced | 2 | candidate | 277.365 [266.694, 289.252] | 56.908 | 94.8 |
| balanced | 4 | baseline | 187.668 [181.023, 200.651] | 57.623 | 99.3 |
| balanced | 4 | candidate | 171.930 [168.580, 183.769] | 46.016 | 96.9 |
| balanced | 8 | baseline | 135.872 [133.990, 139.524] | 52.382 | 103.2 |
| balanced | 8 | candidate | 119.450 [118.483, 122.184] | 39.592 | 110.7 |
| cpu | 1 | baseline | 848.308 [840.680, 853.063] | 72.406 | 80.8 |
| cpu | 1 | candidate | 823.735 [820.354, 830.825] | 63.523 | 77.6 |
| cpu | 2 | baseline | 494.012 [477.744, 525.078] | 67.931 | 89.7 |
| cpu | 2 | candidate | 480.991 [460.916, 501.480] | 54.159 | 86.1 |
| cpu | 4 | baseline | 283.169 [281.664, 284.105] | 56.851 | 98.8 |
| cpu | 4 | candidate | 265.406 [256.538, 269.641] | 44.257 | 94.0 |
| cpu | 8 | baseline | 324.459 [301.397, 341.974] | 56.244 | 125.8 |
| cpu | 8 | candidate | 321.142 [300.152, 332.766] | 45.435 | 126.8 |
| fanout | 1 | baseline | 1113.658 [1089.352, 1189.291] | 182.278 | 88.1 |
| fanout | 1 | candidate | 1068.482 [1051.814, 1112.614] | 166.279 | 86.0 |
| fanout | 2 | baseline | 937.677 [862.638, 973.403] | 178.849 | 112.3 |
| fanout | 2 | candidate | 904.407 [859.813, 955.280] | 154.730 | 106.3 |
| fanout | 4 | baseline | 682.097 [657.087, 717.373] | 147.018 | 123.5 |
| fanout | 4 | candidate | 659.619 [627.912, 680.699] | 125.349 | 125.8 |
| fanout | 8 | baseline | 767.229 [757.818, 784.093] | 130.461 | 103.9 |
| fanout | 8 | candidate | 747.746 [729.170, 762.215] | 109.013 | 108.7 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| backpressure | 1 | -3.4% | -11.7% | -2.2% | 22 / 22 |
| backpressure | 2 | -5.1% | -14.3% | -1.3% | 22 / 22 |
| backpressure | 4 | -7.6% | -16.0% | -3.6% | 22 / 22 |
| backpressure | 8 | -11.6% | -22.1% | -3.3% | 22 / 21 |
| balanced | 1 | -4.5% | -9.5% | -1.7% | 22 / 22 |
| balanced | 2 | -12.6% | -18.4% | -0.6% | 22 / 22 |
| balanced | 4 | -8.4% | -20.1% | -2.5% | 22 / 22 |
| balanced | 8 | -12.1% | -24.4% | +7.3% | 22 / 21 |
| cpu | 1 | -2.9% | -12.3% | -4.0% | 22 / 22 |
| cpu | 2 | -2.6% | -20.3% | -4.0% | 22 / 22 |
| cpu | 4 | -6.3% | -22.2% | -4.8% | 22 / 22 |
| cpu | 8 | -1.0% | -19.2% | +0.8% | 22 / 22 |
| fanout | 1 | -4.1% | -8.8% | -2.3% | 42 / 42 |
| fanout | 2 | -3.5% | -13.5% | -5.3% | 42 / 42 |
| fanout | 4 | -3.3% | -14.7% | +1.9% | 42 / 42 |
| fanout | 8 | -2.5% | -16.4% | +4.6% | 41 / 41 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 114688 | 0.960 | 4.352 |
| backpressure | 1 | candidate | 114688 | 0.960 | 3.840 |
| backpressure | 2 | baseline | 114688 | 0.496 | 3.456 |
| backpressure | 2 | candidate | 114688 | 0.480 | 3.072 |
| backpressure | 4 | baseline | 114688 | 0.272 | 2.432 |
| backpressure | 4 | candidate | 114688 | 0.256 | 2.048 |
| backpressure | 8 | baseline | 114688 | 0.184 | 2.048 |
| backpressure | 8 | candidate | 114688 | 0.168 | 1.664 |
| balanced | 1 | baseline | 114688 | 14.848 | 28.672 |
| balanced | 1 | candidate | 114688 | 14.336 | 26.624 |
| balanced | 2 | baseline | 114688 | 10.240 | 23.552 |
| balanced | 2 | candidate | 114688 | 9.216 | 18.432 |
| balanced | 4 | baseline | 114688 | 6.144 | 17.408 |
| balanced | 4 | candidate | 114688 | 5.632 | 14.336 |
| balanced | 8 | baseline | 114688 | 4.352 | 15.360 |
| balanced | 8 | candidate | 114688 | 3.840 | 11.264 |
| cpu | 1 | baseline | 114688 | 28.672 | 43.008 |
| cpu | 1 | candidate | 114688 | 28.672 | 36.864 |
| cpu | 2 | baseline | 114688 | 16.384 | 29.696 |
| cpu | 2 | candidate | 114688 | 15.872 | 28.672 |
| cpu | 4 | baseline | 114688 | 9.216 | 20.480 |
| cpu | 4 | candidate | 114688 | 8.704 | 16.384 |
| cpu | 8 | baseline | 114688 | 10.752 | 22.528 |
| cpu | 8 | candidate | 114688 | 10.752 | 20.480 |
| fanout | 1 | baseline | 114688 | 38.912 | 55.296 |
| fanout | 1 | candidate | 114688 | 36.864 | 51.200 |
| fanout | 2 | baseline | 114688 | 30.720 | 51.200 |
| fanout | 2 | candidate | 114688 | 30.720 | 47.104 |
| fanout | 4 | baseline | 114688 | 21.504 | 38.912 |
| fanout | 4 | candidate | 114688 | 21.504 | 36.864 |
| fanout | 8 | baseline | 114688 | 25.600 | 40.960 |
| fanout | 8 | candidate | 114688 | 24.576 | 36.864 |

Diagnostic GC phases summed across measured repetitions (wall time):
