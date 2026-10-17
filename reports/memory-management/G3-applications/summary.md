| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 448.989 [424.348, 521.292] | 69.152 | 56.8 |
| backpressure | 1 | candidate | 440.833 [430.842, 444.256] | 69.390 | 57.1 |
| backpressure | 2 | baseline | 298.517 [291.159, 563.512] | 62.894 | 63.7 |
| backpressure | 2 | candidate | 299.721 [291.010, 329.059] | 55.928 | 63.4 |
| backpressure | 4 | baseline | 187.327 [177.585, 1102.527] | 48.838 | 68.1 |
| backpressure | 4 | candidate | 183.891 [177.781, 216.125] | 44.966 | 68.0 |
| backpressure | 8 | baseline | 133.972 [125.920, 139.607] | 41.499 | 78.7 |
| backpressure | 8 | candidate | 129.662 [128.608, 141.275] | 38.856 | 78.9 |
| balanced | 1 | baseline | 442.627 [432.655, 1088.329] | 76.720 | 85.3 |
| balanced | 1 | candidate | 448.599 [444.564, 498.538] | 81.507 | 85.4 |
| balanced | 2 | baseline | 317.693 [290.279, 325.211] | 73.019 | 95.3 |
| balanced | 2 | candidate | 321.320 [300.444, 365.748] | 77.484 | 97.3 |
| balanced | 4 | baseline | 189.923 [186.369, 206.315] | 59.310 | 99.4 |
| balanced | 4 | candidate | 189.941 [185.336, 220.708] | 60.556 | 97.7 |
| balanced | 8 | baseline | 140.238 [135.516, 158.522] | 54.425 | 105.2 |
| balanced | 8 | candidate | 140.716 [132.553, 156.425] | 53.516 | 99.3 |
| cpu | 1 | baseline | 908.972 [856.624, 1510.554] | 80.521 | 80.7 |
| cpu | 1 | candidate | 879.602 [858.485, 2138.133] | 80.943 | 81.0 |
| cpu | 2 | baseline | 517.797 [489.283, 529.478] | 69.890 | 89.4 |
| cpu | 2 | candidate | 509.094 [481.198, 543.265] | 69.429 | 89.0 |
| cpu | 4 | baseline | 286.574 [281.186, 1460.360] | 59.542 | 99.6 |
| cpu | 4 | candidate | 291.033 [280.877, 309.892] | 61.062 | 98.3 |
| cpu | 8 | baseline | 325.773 [305.247, 1088.670] | 60.471 | 126.9 |
| cpu | 8 | candidate | 339.265 [286.128, 665.220] | 62.448 | 124.5 |
| fanout | 1 | baseline | 1152.482 [1121.417, 1382.185] | 191.501 | 88.2 |
| fanout | 1 | candidate | 1160.155 [1153.526, 1302.600] | 192.969 | 88.1 |
| fanout | 2 | baseline | 963.808 [899.893, 1041.284] | 194.653 | 107.4 |
| fanout | 2 | candidate | 1006.299 [870.369, 1214.364] | 193.282 | 106.1 |
| fanout | 4 | baseline | 723.399 [706.135, 811.534] | 165.871 | 125.0 |
| fanout | 4 | candidate | 693.263 [666.457, 825.283] | 150.345 | 122.3 |
| fanout | 8 | baseline | 794.958 [764.218, 968.130] | 144.011 | 108.3 |
| fanout | 8 | candidate | 836.823 [776.431, 881.438] | 144.881 | 116.8 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| backpressure | 1 | -1.8% | +0.3% | +0.4% | 22 / 22 |
| backpressure | 2 | +0.4% | -11.1% | -0.5% | 22 / 22 |
| backpressure | 4 | -1.8% | -7.9% | -0.1% | 22 / 22 |
| backpressure | 8 | -3.2% | -6.4% | +0.3% | 22 / 22 |
| balanced | 1 | +1.3% | +6.2% | +0.1% | 22 / 22 |
| balanced | 2 | +1.1% | +6.1% | +2.1% | 22 / 22 |
| balanced | 4 | +0.0% | +2.1% | -1.7% | 22 / 22 |
| balanced | 8 | +0.3% | -1.7% | -5.6% | 22 / 22 |
| cpu | 1 | -3.2% | +0.5% | +0.4% | 22 / 22 |
| cpu | 2 | -1.7% | -0.7% | -0.4% | 22 / 22 |
| cpu | 4 | +1.6% | +2.6% | -1.3% | 22 / 22 |
| cpu | 8 | +4.1% | +3.3% | -1.9% | 22 / 22 |
| fanout | 1 | +0.7% | +0.8% | -0.1% | 42 / 42 |
| fanout | 2 | +4.4% | -0.7% | -1.2% | 42 / 42 |
| fanout | 4 | -4.2% | -9.4% | -2.2% | 42 / 42 |
| fanout | 8 | +5.3% | +0.6% | +7.8% | 41 / 41 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 114688 | 0.992 | 5.120 |
| backpressure | 1 | candidate | 114688 | 0.960 | 4.864 |
| backpressure | 2 | baseline | 114688 | 0.544 | 4.864 |
| backpressure | 2 | candidate | 114688 | 0.512 | 3.712 |
| backpressure | 4 | baseline | 114688 | 0.304 | 7.168 |
| backpressure | 4 | candidate | 114688 | 0.288 | 2.816 |
| backpressure | 8 | baseline | 114688 | 0.176 | 2.304 |
| backpressure | 8 | candidate | 114688 | 0.176 | 2.176 |
| balanced | 1 | baseline | 114688 | 15.360 | 59.392 |
| balanced | 1 | candidate | 114688 | 15.360 | 30.720 |
| balanced | 2 | baseline | 114688 | 10.240 | 23.552 |
| balanced | 2 | candidate | 114688 | 10.752 | 24.576 |
| balanced | 4 | baseline | 114688 | 6.144 | 18.432 |
| balanced | 4 | candidate | 114688 | 6.144 | 19.456 |
| balanced | 8 | baseline | 114688 | 4.352 | 17.408 |
| balanced | 8 | candidate | 114688 | 4.352 | 16.384 |
| cpu | 1 | baseline | 114688 | 30.720 | 77.824 |
| cpu | 1 | candidate | 114688 | 30.720 | 114.688 |
| cpu | 2 | baseline | 114688 | 17.408 | 31.744 |
| cpu | 2 | candidate | 114688 | 17.408 | 29.696 |
| cpu | 4 | baseline | 114688 | 9.728 | 73.728 |
| cpu | 4 | candidate | 114688 | 9.216 | 21.504 |
| cpu | 8 | baseline | 114688 | 11.264 | 53.248 |
| cpu | 8 | candidate | 114688 | 11.264 | 40.960 |
| fanout | 1 | baseline | 114688 | 38.912 | 61.440 |
| fanout | 1 | candidate | 114688 | 38.912 | 59.392 |
| fanout | 2 | baseline | 114688 | 31.744 | 61.440 |
| fanout | 2 | candidate | 114688 | 32.768 | 69.632 |
| fanout | 4 | baseline | 114688 | 23.552 | 53.248 |
| fanout | 4 | candidate | 114688 | 22.528 | 43.008 |
| fanout | 8 | baseline | 114688 | 26.624 | 45.056 |
| fanout | 8 | candidate | 114688 | 26.624 | 43.008 |

Diagnostic GC phases summed across measured repetitions (wall time):
