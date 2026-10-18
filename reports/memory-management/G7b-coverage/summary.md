| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| buffers-pointers | 1 | baseline | 18.285 [17.680, 19.365] | 1.386 | 48.4 |
| buffers-pointers | 1 | candidate | 18.127 [17.555, 19.288] | 1.301 | 48.6 |
| buffers-pointers | 2 | baseline | 23.918 [23.434, 24.276] | 1.378 | 48.6 |
| buffers-pointers | 2 | candidate | 24.549 [23.461, 26.499] | 1.377 | 47.9 |
| buffers-pointers | 4 | baseline | 24.187 [23.682, 26.695] | 1.433 | 48.3 |
| buffers-pointers | 4 | candidate | 24.025 [23.629, 28.058] | 1.446 | 48.0 |
| buffers-pointers | 8 | baseline | 24.194 [23.900, 25.594] | 1.327 | 48.5 |
| buffers-pointers | 8 | candidate | 23.848 [23.280, 23.945] | 1.361 | 48.1 |
| parallel-snapshots | 1 | baseline | 331.372 [322.005, 338.435] | 113.496 | 81.4 |
| parallel-snapshots | 1 | candidate | 321.054 [316.529, 340.212] | 107.909 | 81.1 |
| parallel-snapshots | 2 | baseline | 239.220 [229.177, 242.914] | 84.278 | 79.7 |
| parallel-snapshots | 2 | candidate | 234.828 [226.163, 245.115] | 80.909 | 78.0 |
| parallel-snapshots | 4 | baseline | 236.040 [228.695, 239.624] | 96.144 | 98.7 |
| parallel-snapshots | 4 | candidate | 233.385 [226.086, 238.116] | 93.424 | 98.8 |
| parallel-snapshots | 8 | baseline | 410.848 [398.947, 430.189] | 111.012 | 135.8 |
| parallel-snapshots | 8 | candidate | 414.383 [407.525, 416.927] | 112.931 | 136.6 |
| persistent-64-major | 1 | baseline | 14.811 [14.209, 16.577] | 0.000 | 30.4 |
| persistent-64-major | 1 | candidate | 14.457 [14.255, 15.212] | 0.000 | 30.3 |
| persistent-64-major | 2 | baseline | 14.284 [14.000, 15.504] | 0.000 | 30.3 |
| persistent-64-major | 2 | candidate | 14.262 [14.150, 17.672] | 0.000 | 30.2 |
| persistent-64-major | 4 | baseline | 14.551 [14.247, 14.936] | 0.000 | 30.9 |
| persistent-64-major | 4 | candidate | 15.059 [14.459, 18.904] | 0.000 | 30.2 |
| persistent-64-major | 8 | baseline | 14.331 [14.210, 14.675] | 0.000 | 30.2 |
| persistent-64-major | 8 | candidate | 14.359 [14.240, 15.678] | 0.000 | 30.1 |
| records-window | 1 | baseline | 129.874 [127.262, 132.158] | 23.300 | 52.2 |
| records-window | 1 | candidate | 130.137 [126.777, 131.396] | 22.505 | 51.4 |
| records-window | 2 | baseline | 130.725 [126.673, 136.800] | 23.454 | 51.7 |
| records-window | 2 | candidate | 127.012 [123.179, 132.181] | 22.138 | 52.0 |
| records-window | 4 | baseline | 128.170 [125.695, 131.746] | 22.184 | 51.7 |
| records-window | 4 | candidate | 127.102 [122.528, 130.753] | 21.871 | 51.3 |
| records-window | 8 | baseline | 129.540 [126.595, 133.009] | 23.026 | 51.6 |
| records-window | 8 | candidate | 126.200 [124.563, 133.865] | 21.846 | 51.8 |
| scalar-snapshots | 1 | baseline | 13.669 [12.511, 15.197] | 3.221 | 51.4 |
| scalar-snapshots | 1 | candidate | 13.007 [12.369, 16.451] | 2.703 | 51.0 |
| scalar-snapshots | 2 | baseline | 14.539 [12.833, 16.083] | 3.379 | 51.1 |
| scalar-snapshots | 2 | candidate | 13.448 [12.219, 14.745] | 2.822 | 51.0 |
| scalar-snapshots | 4 | baseline | 14.564 [13.436, 15.758] | 3.278 | 51.3 |
| scalar-snapshots | 4 | candidate | 13.624 [11.660, 14.401] | 2.773 | 51.0 |
| scalar-snapshots | 8 | baseline | 14.196 [12.306, 14.576] | 3.014 | 51.2 |
| scalar-snapshots | 8 | candidate | 14.752 [12.649, 17.371] | 2.848 | 51.1 |
| snapshots-major | 1 | baseline | 105.373 [103.783, 109.910] | 27.076 | 49.2 |
| snapshots-major | 1 | candidate | 107.572 [102.352, 112.648] | 27.465 | 48.9 |
| snapshots-major | 2 | baseline | 111.035 [105.902, 113.822] | 28.516 | 49.7 |
| snapshots-major | 2 | candidate | 112.790 [106.312, 125.821] | 28.507 | 49.0 |
| snapshots-major | 4 | baseline | 107.042 [106.113, 113.568] | 27.587 | 49.6 |
| snapshots-major | 4 | candidate | 111.603 [105.562, 124.864] | 27.935 | 48.9 |
| snapshots-major | 8 | baseline | 109.119 [106.135, 112.546] | 27.574 | 49.5 |
| snapshots-major | 8 | candidate | 108.074 [105.169, 117.267] | 26.253 | 49.4 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| buffers-pointers | 1 | -0.9% | -6.1% | +0.3% | 1 / 1 |
| buffers-pointers | 2 | +2.6% | -0.0% | -1.3% | 1 / 1 |
| buffers-pointers | 4 | -0.7% | +0.9% | -0.7% | 1 / 1 |
| buffers-pointers | 8 | -1.4% | +2.6% | -0.8% | 1 / 1 |
| parallel-snapshots | 1 | -3.1% | -4.9% | -0.3% | 37 / 37 |
| parallel-snapshots | 2 | -1.8% | -4.0% | -2.2% | 37 / 37 |
| parallel-snapshots | 4 | -1.1% | -2.8% | +0.1% | 36 / 36 |
| parallel-snapshots | 8 | +0.9% | +1.7% | +0.6% | 32 / 33 |
| persistent-64-major | 1 | -2.4% | n/a | -0.3% | 0 / 0 |
| persistent-64-major | 2 | -0.2% | n/a | -0.5% | 0 / 0 |
| persistent-64-major | 4 | +3.5% | n/a | -2.0% | 0 / 0 |
| persistent-64-major | 8 | +0.2% | n/a | -0.5% | 0 / 0 |
| records-window | 1 | +0.2% | -3.4% | -1.5% | 16 / 16 |
| records-window | 2 | -2.8% | -5.6% | +0.6% | 16 / 16 |
| records-window | 4 | -0.8% | -1.4% | -0.7% | 16 / 16 |
| records-window | 8 | -2.6% | -5.1% | +0.2% | 16 / 16 |
| scalar-snapshots | 1 | -4.8% | -16.1% | -0.9% | 2 / 2 |
| scalar-snapshots | 2 | -7.5% | -16.5% | -0.1% | 2 / 2 |
| scalar-snapshots | 4 | -6.5% | -15.4% | -0.6% | 2 / 2 |
| scalar-snapshots | 8 | +3.9% | -5.5% | -0.3% | 2 / 2 |
| snapshots-major | 1 | +2.1% | +1.4% | -0.7% | 17 / 17 |
| snapshots-major | 2 | +1.6% | -0.0% | -1.3% | 17 / 17 |
| snapshots-major | 4 | +4.3% | +1.3% | -1.6% | 17 / 17 |
| snapshots-major | 8 | -1.0% | -4.8% | -0.2% | 17 / 17 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| buffers-pointers | 1 | 7 | -0.6% [-4.6%, +6.7%] | 5 / 7 |
| buffers-pointers | 2 | 7 | +4.0% [-3.4%, +10.8%] | 2 / 7 |
| buffers-pointers | 4 | 7 | -0.7% [-2.3%, +15.4%] | 4 / 7 |
| buffers-pointers | 8 | 7 | -1.6% [-6.8%, -0.2%] | 7 / 7 |
| parallel-snapshots | 1 | 7 | -3.1% [-5.1%, +5.2%] | 5 / 7 |
| parallel-snapshots | 2 | 7 | -3.0% [-5.5%, +6.1%] | 4 / 7 |
| parallel-snapshots | 4 | 7 | -1.0% [-3.6%, +2.2%] | 6 / 7 |
| parallel-snapshots | 8 | 7 | -0.7% [-4.0%, +4.2%] | 4 / 7 |
| persistent-64-major | 1 | 7 | -2.4% [-14.0%, +1.1%] | 5 / 7 |
| persistent-64-major | 2 | 7 | -0.5% [-8.4%, +26.2%] | 4 / 7 |
| persistent-64-major | 4 | 7 | +3.5% [+1.1%, +26.6%] | 0 / 7 |
| persistent-64-major | 8 | 7 | +0.4% [-3.0%, +6.9%] | 2 / 7 |
| records-window | 1 | 7 | -0.9% [-4.1%, +2.0%] | 5 / 7 |
| records-window | 2 | 7 | -2.3% [-10.0%, +0.6%] | 6 / 7 |
| records-window | 4 | 7 | -0.8% [-5.2%, +4.0%] | 5 / 7 |
| records-window | 8 | 7 | -2.0% [-4.4%, +0.6%] | 6 / 7 |
| scalar-snapshots | 1 | 7 | -1.1% [-11.1%, +25.0%] | 4 / 7 |
| scalar-snapshots | 2 | 7 | -8.6% [-16.0%, +11.2%] | 5 / 7 |
| scalar-snapshots | 4 | 7 | -7.1% [-26.0%, +2.5%] | 4 / 7 |
| scalar-snapshots | 8 | 7 | +2.9% [-7.8%, +41.2%] | 3 / 7 |
| snapshots-major | 1 | 7 | +2.1% [-6.9%, +4.9%] | 2 / 7 |
| snapshots-major | 2 | 7 | +3.7% [-6.2%, +14.7%] | 3 / 7 |
| snapshots-major | 4 | 7 | +4.0% [-5.2%, +17.5%] | 2 / 7 |
| snapshots-major | 8 | 7 | -1.9% [-3.3%, +6.6%] | 4 / 7 |

Diagnostic GC phases summed across measured repetitions (wall time):
