| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 483.560 [456.412, 583.705] | 83.449 | 53.1 |
| backpressure | 1 | candidate | 498.950 [484.172, 537.732] | 77.857 | 57.5 |
| backpressure | 2 | baseline | 371.240 [349.652, 420.410] | 78.190 | 55.3 |
| backpressure | 2 | candidate | 368.636 [350.478, 421.474] | 74.415 | 61.9 |
| backpressure | 4 | baseline | 228.367 [216.172, 287.699] | 58.978 | 69.2 |
| backpressure | 4 | candidate | 215.912 [210.609, 295.906] | 54.930 | 70.9 |
| backpressure | 8 | baseline | 178.911 [165.848, 208.297] | 55.429 | 76.2 |
| backpressure | 8 | candidate | 166.244 [158.024, 239.216] | 49.189 | 83.6 |
| balanced | 1 | baseline | 558.783 [547.782, 669.321] | 122.224 | 81.5 |
| balanced | 1 | candidate | 550.937 [543.468, 626.453] | 103.487 | 86.8 |
| balanced | 2 | baseline | 391.242 [382.244, 484.100] | 102.215 | 87.5 |
| balanced | 2 | candidate | 411.950 [383.682, 444.257] | 94.204 | 99.6 |
| balanced | 4 | baseline | 255.141 [248.537, 336.669] | 80.982 | 90.5 |
| balanced | 4 | candidate | 249.445 [225.826, 321.478] | 75.025 | 98.7 |
| balanced | 8 | baseline | 190.305 [177.096, 227.873] | 72.202 | 94.5 |
| balanced | 8 | candidate | 180.416 [168.537, 205.848] | 70.222 | 105.0 |
| cpu | 1 | baseline | 1024.837 [1007.705, 1109.281] | 120.475 | 78.0 |
| cpu | 1 | candidate | 1031.145 [1004.948, 1177.058] | 107.490 | 80.7 |
| cpu | 2 | baseline | 598.981 [588.431, 704.504] | 97.952 | 84.8 |
| cpu | 2 | candidate | 611.309 [580.782, 702.602] | 93.821 | 89.9 |
| cpu | 4 | baseline | 346.559 [337.733, 453.598] | 78.390 | 93.6 |
| cpu | 4 | candidate | 348.330 [337.883, 422.726] | 74.325 | 97.6 |
| cpu | 8 | baseline | 420.431 [360.371, 459.185] | 84.891 | 123.4 |
| cpu | 8 | candidate | 382.902 [340.598, 413.470] | 72.838 | 130.7 |
| fanout | 1 | baseline | 1430.231 [1377.795, 1592.111] | 283.241 | 81.3 |
| fanout | 1 | candidate | 1403.546 [1351.367, 1647.650] | 250.562 | 86.8 |
| fanout | 2 | baseline | 1164.335 [1096.433, 1332.440] | 250.065 | 98.4 |
| fanout | 2 | candidate | 1152.707 [1027.780, 1235.131] | 233.352 | 107.1 |
| fanout | 4 | baseline | 842.114 [793.188, 974.987] | 201.332 | 111.5 |
| fanout | 4 | candidate | 778.360 [739.837, 902.514] | 182.574 | 121.9 |
| fanout | 8 | baseline | 930.388 [894.944, 1081.306] | 177.417 | 99.5 |
| fanout | 8 | candidate | 883.210 [861.527, 1126.253] | 157.158 | 110.7 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| backpressure | 1 | +3.2% | -6.7% | +8.2% | 24 / 24 |
| backpressure | 2 | -0.7% | -4.8% | +11.9% | 24 / 24 |
| backpressure | 4 | -5.5% | -6.9% | +2.4% | 24 / 24 |
| backpressure | 8 | -7.1% | -11.3% | +9.7% | 24 / 24 |
| balanced | 1 | -1.4% | -15.3% | +6.4% | 24 / 24 |
| balanced | 2 | +5.3% | -7.8% | +13.9% | 24 / 24 |
| balanced | 4 | -2.2% | -7.4% | +9.1% | 24 / 24 |
| balanced | 8 | -5.2% | -2.7% | +11.1% | 24 / 24 |
| cpu | 1 | +0.6% | -10.8% | +3.4% | 25 / 25 |
| cpu | 2 | +2.1% | -4.2% | +6.0% | 25 / 25 |
| cpu | 4 | +0.5% | -5.2% | +4.3% | 25 / 25 |
| cpu | 8 | -8.9% | -14.2% | +5.9% | 24 / 24 |
| fanout | 1 | -1.9% | -11.5% | +6.7% | 45 / 45 |
| fanout | 2 | -1.0% | -6.7% | +8.8% | 45 / 45 |
| fanout | 4 | -7.6% | -9.3% | +9.3% | 45 / 45 |
| fanout | 8 | -5.1% | -11.4% | +11.2% | 44 / 44 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 114688 | 1.088 | 5.888 |
| backpressure | 1 | candidate | 114688 | 1.152 | 5.632 |
| backpressure | 2 | baseline | 114688 | 0.608 | 4.864 |
| backpressure | 2 | candidate | 114688 | 0.640 | 4.608 |
| backpressure | 4 | baseline | 114688 | 0.352 | 3.456 |
| backpressure | 4 | candidate | 114688 | 0.336 | 3.200 |
| backpressure | 8 | baseline | 114688 | 0.240 | 2.944 |
| backpressure | 8 | candidate | 114688 | 0.240 | 3.072 |
| balanced | 1 | baseline | 114688 | 19.456 | 38.912 |
| balanced | 1 | candidate | 114688 | 19.456 | 36.864 |
| balanced | 2 | baseline | 114688 | 13.312 | 29.696 |
| balanced | 2 | candidate | 114688 | 13.312 | 31.744 |
| balanced | 4 | baseline | 114688 | 8.704 | 28.672 |
| balanced | 4 | candidate | 114688 | 7.936 | 26.624 |
| balanced | 8 | baseline | 114688 | 5.888 | 20.480 |
| balanced | 8 | candidate | 114688 | 5.632 | 19.456 |
| cpu | 1 | baseline | 114688 | 34.816 | 51.200 |
| cpu | 1 | candidate | 114688 | 34.816 | 51.200 |
| cpu | 2 | baseline | 114688 | 20.480 | 38.912 |
| cpu | 2 | candidate | 114688 | 20.480 | 38.912 |
| cpu | 4 | baseline | 114688 | 11.264 | 26.624 |
| cpu | 4 | candidate | 114688 | 11.776 | 27.648 |
| cpu | 8 | baseline | 114688 | 13.824 | 29.696 |
| cpu | 8 | candidate | 114688 | 12.288 | 27.648 |
| fanout | 1 | baseline | 114688 | 49.152 | 73.728 |
| fanout | 1 | candidate | 114688 | 49.152 | 73.728 |
| fanout | 2 | baseline | 114688 | 38.912 | 65.536 |
| fanout | 2 | candidate | 114688 | 38.912 | 61.440 |
| fanout | 4 | baseline | 114688 | 27.648 | 47.104 |
| fanout | 4 | candidate | 114688 | 24.576 | 49.152 |
| fanout | 8 | baseline | 114688 | 30.720 | 51.200 |
| fanout | 8 | candidate | 114688 | 29.696 | 59.392 |

Diagnostic GC phases summed across measured repetitions (wall time):

