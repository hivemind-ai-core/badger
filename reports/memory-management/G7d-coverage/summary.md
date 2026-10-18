| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| buffers-pointers | 1 | baseline | 17.812 [17.714, 18.264] | 1.420 | 48.5 |
| buffers-pointers | 1 | candidate | 18.499 [17.509, 24.250] | 1.356 | 47.5 |
| buffers-pointers | 2 | baseline | 23.986 [23.585, 25.246] | 1.399 | 48.4 |
| buffers-pointers | 2 | candidate | 24.366 [24.281, 25.313] | 1.321 | 48.6 |
| buffers-pointers | 4 | baseline | 24.008 [23.681, 24.991] | 1.375 | 48.2 |
| buffers-pointers | 4 | candidate | 24.494 [24.213, 25.590] | 1.335 | 49.4 |
| buffers-pointers | 8 | baseline | 24.150 [23.923, 25.754] | 1.385 | 48.0 |
| buffers-pointers | 8 | candidate | 25.283 [23.713, 26.029] | 1.323 | 47.9 |
| parallel-snapshots | 1 | baseline | 325.067 [317.998, 332.863] | 112.705 | 81.1 |
| parallel-snapshots | 1 | candidate | 279.433 [271.422, 282.023] | 78.263 | 80.1 |
| parallel-snapshots | 2 | baseline | 239.974 [229.311, 245.856] | 86.312 | 78.2 |
| parallel-snapshots | 2 | candidate | 217.736 [207.902, 224.894] | 69.185 | 77.9 |
| parallel-snapshots | 4 | baseline | 237.217 [229.683, 245.540] | 93.928 | 100.7 |
| parallel-snapshots | 4 | candidate | 233.272 [221.644, 239.699] | 89.690 | 99.7 |
| parallel-snapshots | 8 | baseline | 415.566 [411.026, 430.803] | 118.969 | 137.6 |
| parallel-snapshots | 8 | candidate | 421.032 [405.835, 430.142] | 116.852 | 138.4 |
| persistent-64-major | 1 | baseline | 14.584 [14.200, 15.172] | 0.000 | 29.9 |
| persistent-64-major | 1 | candidate | 14.481 [14.126, 15.249] | 0.000 | 30.2 |
| persistent-64-major | 2 | baseline | 14.553 [14.375, 15.530] | 0.000 | 29.8 |
| persistent-64-major | 2 | candidate | 14.919 [14.307, 15.345] | 0.000 | 29.8 |
| persistent-64-major | 4 | baseline | 14.577 [14.260, 15.641] | 0.000 | 29.9 |
| persistent-64-major | 4 | candidate | 14.363 [14.230, 16.439] | 0.000 | 30.0 |
| persistent-64-major | 8 | baseline | 14.433 [14.245, 16.809] | 0.000 | 30.3 |
| persistent-64-major | 8 | candidate | 15.107 [14.286, 15.867] | 0.000 | 30.1 |
| records-window | 1 | baseline | 130.621 [126.417, 142.485] | 22.901 | 51.5 |
| records-window | 1 | candidate | 127.203 [124.083, 133.577] | 20.893 | 51.7 |
| records-window | 2 | baseline | 128.806 [125.835, 137.882] | 22.599 | 51.0 |
| records-window | 2 | candidate | 125.874 [124.065, 127.301] | 20.636 | 50.9 |
| records-window | 4 | baseline | 127.999 [126.810, 132.154] | 23.019 | 50.8 |
| records-window | 4 | candidate | 125.569 [123.188, 128.382] | 20.771 | 51.6 |
| records-window | 8 | baseline | 128.610 [127.585, 133.374] | 23.350 | 51.5 |
| records-window | 8 | candidate | 127.107 [122.901, 129.578] | 20.790 | 52.0 |
| scalar-snapshots | 1 | baseline | 15.009 [13.634, 15.788] | 3.589 | 50.8 |
| scalar-snapshots | 1 | candidate | 12.530 [11.658, 13.885] | 1.845 | 50.7 |
| scalar-snapshots | 2 | baseline | 13.741 [13.016, 15.342] | 3.271 | 50.8 |
| scalar-snapshots | 2 | candidate | 12.098 [11.027, 13.745] | 1.719 | 50.6 |
| scalar-snapshots | 4 | baseline | 13.582 [12.825, 23.366] | 2.948 | 50.8 |
| scalar-snapshots | 4 | candidate | 12.731 [11.509, 14.283] | 1.770 | 51.0 |
| scalar-snapshots | 8 | baseline | 14.529 [12.643, 16.144] | 3.314 | 50.7 |
| scalar-snapshots | 8 | candidate | 12.961 [11.558, 15.342] | 1.679 | 51.0 |
| snapshots-major | 1 | baseline | 107.373 [103.869, 129.083] | 28.034 | 48.5 |
| snapshots-major | 1 | candidate | 89.435 [87.838, 92.330] | 15.167 | 48.5 |
| snapshots-major | 2 | baseline | 109.499 [105.110, 113.848] | 28.303 | 48.7 |
| snapshots-major | 2 | candidate | 97.713 [91.915, 101.903] | 15.358 | 48.7 |
| snapshots-major | 4 | baseline | 110.054 [106.341, 116.126] | 28.048 | 48.8 |
| snapshots-major | 4 | candidate | 97.561 [92.361, 106.881] | 16.011 | 48.7 |
| snapshots-major | 8 | baseline | 112.228 [106.950, 117.483] | 28.209 | 48.9 |
| snapshots-major | 8 | candidate | 96.537 [92.102, 101.265] | 15.051 | 48.8 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| buffers-pointers | 1 | +3.9% | -4.6% | -2.0% | 1 / 1 |
| buffers-pointers | 2 | +1.6% | -5.6% | +0.3% | 1 / 1 |
| buffers-pointers | 4 | +2.0% | -2.9% | +2.4% | 1 / 1 |
| buffers-pointers | 8 | +4.7% | -4.5% | -0.0% | 1 / 1 |
| parallel-snapshots | 1 | -14.0% | -30.6% | -1.3% | 37 / 37 |
| parallel-snapshots | 2 | -9.3% | -19.8% | -0.5% | 37 / 37 |
| parallel-snapshots | 4 | -1.7% | -4.5% | -1.0% | 36 / 36 |
| parallel-snapshots | 8 | +1.3% | -1.8% | +0.5% | 33 / 33 |
| persistent-64-major | 1 | -0.7% | n/a | +1.1% | 0 / 0 |
| persistent-64-major | 2 | +2.5% | n/a | +0.3% | 0 / 0 |
| persistent-64-major | 4 | -1.5% | n/a | +0.4% | 0 / 0 |
| persistent-64-major | 8 | +4.7% | n/a | -0.4% | 0 / 0 |
| records-window | 1 | -2.6% | -8.8% | +0.4% | 16 / 16 |
| records-window | 2 | -2.3% | -8.7% | -0.2% | 16 / 16 |
| records-window | 4 | -1.9% | -9.8% | +1.5% | 16 / 16 |
| records-window | 8 | -1.2% | -11.0% | +1.1% | 16 / 16 |
| scalar-snapshots | 1 | -16.5% | -48.6% | -0.2% | 2 / 2 |
| scalar-snapshots | 2 | -12.0% | -47.4% | -0.4% | 2 / 2 |
| scalar-snapshots | 4 | -6.3% | -40.0% | +0.6% | 2 / 2 |
| scalar-snapshots | 8 | -10.8% | -49.3% | +0.6% | 2 / 2 |
| snapshots-major | 1 | -16.7% | -45.9% | -0.1% | 17 / 17 |
| snapshots-major | 2 | -10.8% | -45.7% | +0.0% | 17 / 17 |
| snapshots-major | 4 | -11.4% | -42.9% | -0.0% | 17 / 17 |
| snapshots-major | 8 | -14.0% | -46.6% | -0.4% | 17 / 17 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| buffers-pointers | 1 | 7 | +3.0% [-1.5%, +34.8%] | 1 / 7 |
| buffers-pointers | 2 | 7 | +2.1% [+0.1%, +3.3%] | 0 / 7 |
| buffers-pointers | 4 | 7 | +1.5% [-0.7%, +7.1%] | 1 / 7 |
| buffers-pointers | 8 | 7 | +4.0% [-1.8%, +8.8%] | 3 / 7 |
| parallel-snapshots | 1 | 7 | -15.9% [-16.5%, -12.1%] | 7 / 7 |
| parallel-snapshots | 2 | 7 | -9.3% [-14.7%, -2.4%] | 7 / 7 |
| parallel-snapshots | 4 | 7 | -1.4% [-9.7%, +2.6%] | 5 / 7 |
| parallel-snapshots | 8 | 7 | +1.5% [-5.8%, +2.7%] | 1 / 7 |
| persistent-64-major | 1 | 7 | -0.1% [-3.8%, +2.8%] | 4 / 7 |
| persistent-64-major | 2 | 7 | +0.1% [-1.2%, +3.8%] | 3 / 7 |
| persistent-64-major | 4 | 7 | -1.5% [-3.3%, +5.1%] | 4 / 7 |
| persistent-64-major | 8 | 7 | +0.6% [-7.8%, +8.6%] | 3 / 7 |
| records-window | 1 | 7 | -0.4% [-10.7%, +2.0%] | 4 / 7 |
| records-window | 2 | 7 | -2.8% [-8.8%, +0.5%] | 6 / 7 |
| records-window | 4 | 7 | -1.5% [-4.8%, +1.2%] | 6 / 7 |
| records-window | 8 | 7 | -1.9% [-6.4%, +0.8%] | 6 / 7 |
| scalar-snapshots | 1 | 7 | -13.6% [-21.8%, +0.1%] | 6 / 7 |
| scalar-snapshots | 2 | 7 | -10.4% [-24.3%, -0.5%] | 7 / 7 |
| scalar-snapshots | 4 | 7 | -6.2% [-38.9%, +3.8%] | 6 / 7 |
| scalar-snapshots | 8 | 7 | -12.1% [-28.4%, +10.4%] | 5 / 7 |
| snapshots-major | 1 | 7 | -16.5% [-28.5%, -14.6%] | 7 / 7 |
| snapshots-major | 2 | 7 | -11.8% [-13.9%, -9.9%] | 7 / 7 |
| snapshots-major | 4 | 7 | -10.0% [-17.3%, -8.0%] | 7 / 7 |
| snapshots-major | 8 | 7 | -14.4% [-16.8%, -9.2%] | 7 / 7 |

Diagnostic GC phases summed across measured repetitions (wall time):
