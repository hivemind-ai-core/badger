| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 437.641 [420.648, 454.634] | 59.787 | 55.2 |
| backpressure | 2 | baseline | 300.448 [287.666, 313.230] | 53.320 | 63.0 |
| backpressure | 4 | baseline | 173.398 [173.003, 173.792] | 41.102 | 67.3 |
| backpressure | 8 | baseline | 126.589 [125.484, 127.694] | 33.836 | 79.7 |
| balanced | 1 | baseline | 446.656 [433.429, 459.883] | 71.146 | 83.5 |
| balanced | 2 | baseline | 311.153 [297.407, 324.900] | 61.938 | 95.5 |
| balanced | 4 | baseline | 188.341 [181.829, 194.852] | 50.726 | 101.7 |
| balanced | 8 | baseline | 133.913 [129.567, 138.259] | 45.240 | 113.0 |
| cpu | 1 | baseline | 858.555 [855.011, 862.099] | 67.769 | 77.5 |
| cpu | 2 | baseline | 493.766 [484.903, 502.630] | 58.545 | 86.3 |
| cpu | 4 | baseline | 275.201 [269.391, 281.011] | 46.724 | 93.5 |
| cpu | 8 | baseline | 352.884 [352.605, 353.164] | 47.742 | 127.2 |
| fanout | 1 | baseline | 1204.743 [1193.256, 1216.230] | 184.504 | 86.0 |
| fanout | 2 | baseline | 863.814 [797.463, 930.164] | 157.799 | 104.3 |
| fanout | 4 | baseline | 791.183 [769.757, 812.609] | 144.166 | 131.5 |
| fanout | 8 | baseline | 797.793 [787.317, 808.270] | 112.315 | 112.3 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 32768 | 0.992 | 4.352 |
| backpressure | 2 | baseline | 32768 | 0.512 | 3.456 |
| backpressure | 4 | baseline | 32768 | 0.272 | 2.176 |
| backpressure | 8 | baseline | 32768 | 0.184 | 1.856 |
| balanced | 1 | baseline | 32768 | 14.848 | 28.672 |
| balanced | 2 | baseline | 32768 | 10.240 | 20.480 |
| balanced | 4 | baseline | 32768 | 5.888 | 14.848 |
| balanced | 8 | baseline | 32768 | 4.352 | 12.800 |
| cpu | 1 | baseline | 32768 | 29.696 | 38.912 |
| cpu | 2 | baseline | 32768 | 17.408 | 28.672 |
| cpu | 4 | baseline | 32768 | 9.216 | 17.408 |
| cpu | 8 | baseline | 32768 | 12.288 | 21.504 |
| fanout | 1 | baseline | 32768 | 40.960 | 55.296 |
| fanout | 2 | baseline | 32768 | 28.672 | 47.104 |
| fanout | 4 | baseline | 32768 | 25.600 | 45.056 |
| fanout | 8 | baseline | 32768 | 26.624 | 38.912 |

Diagnostic GC phases summed across measured repetitions (wall time):
