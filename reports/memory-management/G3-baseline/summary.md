| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 450.115 [444.196, 456.455] | 70.637 | 56.6 |
| backpressure | 2 | baseline | 311.197 [301.920, 317.413] | 61.970 | 64.0 |
| backpressure | 4 | baseline | 180.889 [176.032, 195.536] | 45.897 | 71.2 |
| backpressure | 8 | baseline | 131.145 [127.778, 144.571] | 43.724 | 78.5 |
| balanced | 1 | baseline | 451.467 [448.220, 463.783] | 81.037 | 85.1 |
| balanced | 2 | baseline | 367.880 [355.581, 375.119] | 81.359 | 99.2 |
| balanced | 4 | baseline | 191.794 [191.642, 194.609] | 59.745 | 99.5 |
| balanced | 8 | baseline | 150.524 [135.914, 152.177] | 59.423 | 102.9 |
| cpu | 1 | baseline | 889.239 [870.982, 919.402] | 80.177 | 80.7 |
| cpu | 2 | baseline | 513.150 [493.230, 519.642] | 70.507 | 88.6 |
| cpu | 4 | baseline | 284.899 [284.803, 285.507] | 60.286 | 99.7 |
| cpu | 8 | baseline | 324.358 [315.811, 334.481] | 64.972 | 124.2 |
| fanout | 1 | baseline | 1170.893 [1165.247, 1227.891] | 189.003 | 87.9 |
| fanout | 2 | baseline | 879.232 [864.497, 1027.219] | 175.610 | 104.0 |
| fanout | 4 | baseline | 725.366 [677.899, 743.303] | 154.764 | 127.1 |
| fanout | 8 | baseline | 799.860 [788.435, 818.508] | 139.660 | 103.0 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 49152 | 0.992 | 4.864 |
| backpressure | 2 | baseline | 49152 | 0.512 | 3.840 |
| backpressure | 4 | baseline | 49152 | 0.272 | 2.560 |
| backpressure | 8 | baseline | 49152 | 0.176 | 2.560 |
| balanced | 1 | baseline | 49152 | 14.848 | 29.696 |
| balanced | 2 | baseline | 49152 | 11.264 | 27.648 |
| balanced | 4 | baseline | 49152 | 6.144 | 18.432 |
| balanced | 8 | baseline | 49152 | 4.352 | 16.384 |
| cpu | 1 | baseline | 49152 | 29.696 | 55.296 |
| cpu | 2 | baseline | 49152 | 17.408 | 29.696 |
| cpu | 4 | baseline | 49152 | 9.216 | 21.504 |
| cpu | 8 | baseline | 49152 | 10.752 | 36.864 |
| fanout | 1 | baseline | 49152 | 40.960 | 59.392 |
| fanout | 2 | baseline | 49152 | 30.720 | 73.728 |
| fanout | 4 | baseline | 49152 | 22.528 | 65.536 |
| fanout | 8 | baseline | 49152 | 25.600 | 45.056 |

Diagnostic GC phases summed across measured repetitions (wall time):
