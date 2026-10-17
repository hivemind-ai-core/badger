| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 432.065 [431.070, 433.060] | 60.492 | 57.2 |
| backpressure | 2 | baseline | 290.779 [290.411, 291.147] | 56.268 | 61.6 |
| backpressure | 4 | baseline | 179.987 [177.813, 182.162] | 43.568 | 70.5 |
| backpressure | 8 | baseline | 129.144 [127.753, 130.534] | 38.061 | 80.5 |
| balanced | 1 | baseline | 447.414 [445.533, 449.295] | 74.009 | 84.9 |
| balanced | 2 | baseline | 322.782 [310.500, 335.064] | 68.226 | 97.8 |
| balanced | 4 | baseline | 189.511 [189.282, 189.740] | 56.974 | 95.6 |
| balanced | 8 | baseline | 145.117 [140.895, 149.338] | 54.060 | 103.0 |
| cpu | 1 | baseline | 862.459 [857.237, 867.682] | 70.692 | 80.7 |
| cpu | 2 | baseline | 520.253 [489.648, 550.858] | 64.933 | 90.0 |
| cpu | 4 | baseline | 281.739 [280.748, 282.731] | 54.084 | 99.4 |
| cpu | 8 | baseline | 319.835 [318.849, 320.820] | 55.067 | 124.7 |
| fanout | 1 | baseline | 1107.896 [1087.402, 1128.391] | 179.197 | 88.2 |
| fanout | 2 | baseline | 901.956 [857.080, 946.833] | 170.030 | 108.5 |
| fanout | 4 | baseline | 715.459 [680.600, 750.318] | 147.926 | 134.9 |
| fanout | 8 | baseline | 792.613 [785.442, 799.785] | 134.701 | 114.2 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 32768 | 0.992 | 4.352 |
| backpressure | 2 | baseline | 32768 | 0.496 | 3.456 |
| backpressure | 4 | baseline | 32768 | 0.272 | 2.304 |
| backpressure | 8 | baseline | 32768 | 0.176 | 1.856 |
| balanced | 1 | baseline | 32768 | 14.848 | 29.696 |
| balanced | 2 | baseline | 32768 | 10.240 | 25.600 |
| balanced | 4 | baseline | 32768 | 6.144 | 17.408 |
| balanced | 8 | baseline | 32768 | 4.608 | 15.360 |
| cpu | 1 | baseline | 32768 | 29.696 | 43.008 |
| cpu | 2 | baseline | 32768 | 17.408 | 28.672 |
| cpu | 4 | baseline | 32768 | 9.728 | 20.480 |
| cpu | 8 | baseline | 32768 | 10.240 | 24.576 |
| fanout | 1 | baseline | 32768 | 36.864 | 53.248 |
| fanout | 2 | baseline | 32768 | 29.696 | 53.248 |
| fanout | 4 | baseline | 32768 | 22.528 | 43.008 |
| fanout | 8 | baseline | 32768 | 25.600 | 43.008 |

Diagnostic GC phases summed across measured repetitions (wall time):
