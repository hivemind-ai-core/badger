| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| buffers-pointers | 1 | baseline | 18.134 [17.875, 19.234] | 1.325 | 48.5 |
| buffers-pointers | 1 | candidate | 17.752 [17.446, 18.043] | 1.357 | 48.4 |
| buffers-pointers | 8 | baseline | 24.415 [23.786, 25.574] | 1.442 | 48.4 |
| buffers-pointers | 8 | candidate | 24.579 [23.412, 26.551] | 1.345 | 48.3 |
| parallel-snapshots | 1 | baseline | 323.835 [320.324, 345.652] | 113.186 | 79.5 |
| parallel-snapshots | 1 | candidate | 306.894 [306.341, 310.214] | 105.475 | 80.8 |
| parallel-snapshots | 8 | baseline | 412.485 [402.980, 427.542] | 109.995 | 134.4 |
| parallel-snapshots | 8 | candidate | 419.640 [417.341, 428.784] | 113.749 | 137.7 |
| persistent-64-major | 1 | baseline | 14.685 [14.509, 18.022] | 0.000 | 29.9 |
| persistent-64-major | 1 | candidate | 14.863 [14.535, 16.491] | 0.000 | 31.2 |
| persistent-64-major | 8 | baseline | 14.604 [14.294, 15.080] | 0.000 | 30.2 |
| persistent-64-major | 8 | candidate | 14.478 [14.445, 15.727] | 0.000 | 30.8 |
| records-window | 1 | baseline | 130.912 [129.705, 143.389] | 23.258 | 51.3 |
| records-window | 1 | candidate | 129.051 [128.430, 131.316] | 22.369 | 51.7 |
| records-window | 8 | baseline | 128.361 [127.001, 137.317] | 23.294 | 50.8 |
| records-window | 8 | candidate | 129.097 [125.555, 130.335] | 23.102 | 53.0 |
| scalar-snapshots | 1 | baseline | 15.011 [12.430, 15.752] | 3.697 | 50.9 |
| scalar-snapshots | 1 | candidate | 13.557 [12.659, 14.371] | 2.819 | 51.2 |
| scalar-snapshots | 8 | baseline | 14.925 [12.934, 16.362] | 3.431 | 50.8 |
| scalar-snapshots | 8 | candidate | 14.255 [11.662, 14.789] | 3.259 | 50.6 |
| snapshots-major | 1 | baseline | 106.759 [106.291, 110.365] | 27.442 | 48.6 |
| snapshots-major | 1 | candidate | 99.658 [97.882, 107.490] | 25.897 | 48.5 |
| snapshots-major | 8 | baseline | 108.970 [103.054, 129.989] | 28.515 | 48.7 |
| snapshots-major | 8 | candidate | 106.054 [101.880, 108.848] | 26.847 | 49.0 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| buffers-pointers | 1 | -2.1% | +2.4% | -0.2% | 1 / 1 |
| buffers-pointers | 8 | +0.7% | -6.7% | -0.2% | 1 / 1 |
| parallel-snapshots | 1 | -5.2% | -6.8% | +1.6% | 37 / 37 |
| parallel-snapshots | 8 | +1.7% | +3.4% | +2.4% | 32 / 32 |
| persistent-64-major | 1 | +1.2% | n/a | +4.4% | 0 / 0 |
| persistent-64-major | 8 | -0.9% | n/a | +2.0% | 0 / 0 |
| records-window | 1 | -1.4% | -3.8% | +0.7% | 16 / 16 |
| records-window | 8 | +0.6% | -0.8% | +4.3% | 16 / 16 |
| scalar-snapshots | 1 | -9.7% | -23.7% | +0.6% | 2 / 2 |
| scalar-snapshots | 8 | -4.5% | -5.0% | -0.3% | 2 / 2 |
| snapshots-major | 1 | -6.7% | -5.6% | -0.1% | 17 / 17 |
| snapshots-major | 8 | -2.7% | -5.9% | +0.6% | 17 / 17 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| buffers-pointers | 1 | 5 | -1.6% [-9.3%, -0.5%] | 5 / 5 |
| buffers-pointers | 8 | 5 | +2.9% [-7.9%, +8.8%] | 2 / 5 |
| parallel-snapshots | 1 | 5 | -5.2% [-11.4%, -3.3%] | 5 / 5 |
| parallel-snapshots | 8 | 5 | +3.4% [-2.4%, +4.0%] | 1 / 5 |
| persistent-64-major | 1 | 5 | +1.5% [-13.3%, +12.3%] | 2 / 5 |
| persistent-64-major | 8 | 5 | -1.1% [-2.7%, +8.8%] | 3 / 5 |
| records-window | 1 | 5 | -0.8% [-10.0%, +1.2%] | 4 / 5 |
| records-window | 8 | 5 | -0.7% [-6.0%, +2.4%] | 3 / 5 |
| scalar-snapshots | 1 | 5 | -8.9% [-15.3%, +9.1%] | 4 / 5 |
| scalar-snapshots | 8 | 5 | -1.1% [-25.5%, +7.5%] | 4 / 5 |
| snapshots-major | 1 | 5 | -7.9% [-10.4%, +0.9%] | 4 / 5 |
| snapshots-major | 8 | 5 | -1.8% [-19.1%, -1.1%] | 5 / 5 |

Diagnostic GC phases summed across measured repetitions (wall time):
