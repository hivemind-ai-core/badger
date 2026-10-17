| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 442.222 [440.224, 444.220] | 60.281 | 59.6 |
| backpressure | 2 | baseline | 277.473 [276.878, 278.069] | 43.533 | 67.0 |
| backpressure | 4 | baseline | 152.445 [151.391, 153.498] | 29.423 | 72.9 |
| backpressure | 8 | baseline | 107.386 [107.077, 107.694] | 23.377 | 78.6 |
| balanced | 1 | baseline | 436.589 [433.704, 439.473] | 72.165 | 88.6 |
| balanced | 2 | baseline | 312.351 [304.168, 320.534] | 66.898 | 100.7 |
| balanced | 4 | baseline | 174.064 [159.500, 188.627] | 47.797 | 108.8 |
| balanced | 8 | baseline | 113.712 [111.813, 115.611] | 35.731 | 108.0 |
| cpu | 1 | baseline | 880.291 [878.703, 881.878] | 74.761 | 82.6 |
| cpu | 2 | baseline | 504.855 [491.887, 517.824] | 56.725 | 91.0 |
| cpu | 4 | baseline | 262.672 [259.513, 265.830] | 41.056 | 99.2 |
| cpu | 8 | baseline | 313.173 [310.505, 315.840] | 40.413 | 129.0 |
| fanout | 1 | baseline | 1187.137 [1132.464, 1241.810] | 183.953 | 86.4 |
| fanout | 2 | baseline | 916.819 [832.396, 1001.243] | 162.428 | 107.6 |
| fanout | 4 | baseline | 724.130 [707.336, 740.924] | 160.991 | 133.0 |
| fanout | 8 | baseline | 781.924 [767.637, 796.211] | 116.108 | 106.2 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 32768 | 0.992 | 4.864 |
| backpressure | 2 | baseline | 32768 | 0.480 | 2.688 |
| backpressure | 4 | baseline | 32768 | 0.248 | 1.536 |
| backpressure | 8 | baseline | 32768 | 0.160 | 1.216 |
| balanced | 1 | baseline | 32768 | 14.336 | 24.576 |
| balanced | 2 | baseline | 32768 | 10.240 | 30.720 |
| balanced | 4 | baseline | 32768 | 5.376 | 26.624 |
| balanced | 8 | baseline | 32768 | 3.712 | 9.216 |
| cpu | 1 | baseline | 32768 | 29.696 | 43.008 |
| cpu | 2 | baseline | 32768 | 17.408 | 26.624 |
| cpu | 4 | baseline | 32768 | 8.704 | 15.360 |
| cpu | 8 | baseline | 32768 | 10.752 | 19.456 |
| fanout | 1 | baseline | 32768 | 40.960 | 55.296 |
| fanout | 2 | baseline | 32768 | 30.720 | 49.152 |
| fanout | 4 | baseline | 32768 | 22.528 | 63.488 |
| fanout | 8 | baseline | 32768 | 26.624 | 45.056 |

Diagnostic GC phases summed across measured repetitions (wall time):
