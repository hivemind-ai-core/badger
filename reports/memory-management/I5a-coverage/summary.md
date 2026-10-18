| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| entries-stored | 1 | baseline | 29.362 [28.751, 30.041] | 1.071 | 47.7 |
| entries-stored | 1 | candidate | 29.734 [29.240, 31.033] | 1.102 | 47.8 |
| entries-stored | 2 | baseline | 35.828 [35.383, 37.413] | 1.073 | 47.8 |
| entries-stored | 2 | candidate | 35.268 [35.095, 38.593] | 1.104 | 48.2 |
| entries-stored | 4 | baseline | 35.503 [35.049, 35.976] | 1.094 | 48.0 |
| entries-stored | 4 | candidate | 36.027 [34.771, 37.813] | 1.099 | 48.1 |
| entries-stored | 8 | baseline | 35.904 [35.096, 39.694] | 1.095 | 47.8 |
| entries-stored | 8 | candidate | 35.673 [35.174, 36.467] | 1.091 | 49.4 |
| parallel-maps | 1 | baseline | 201.035 [195.103, 208.545] | 55.538 | 44.3 |
| parallel-maps | 1 | candidate | 30.351 [30.010, 34.322] | 0.000 | 34.6 |
| parallel-maps | 2 | baseline | 139.943 [136.822, 154.196] | 42.961 | 53.5 |
| parallel-maps | 2 | candidate | 16.489 [15.558, 17.519] | 0.000 | 35.3 |
| parallel-maps | 4 | baseline | 93.027 [86.996, 102.416] | 29.550 | 65.4 |
| parallel-maps | 4 | candidate | 9.235 [8.935, 9.692] | 0.000 | 36.6 |
| parallel-maps | 8 | baseline | 65.667 [60.697, 68.511] | 22.791 | 74.1 |
| parallel-maps | 8 | candidate | 6.899 [5.470, 8.319] | 0.000 | 36.4 |
| persistent-4096-major | 1 | baseline | 99.888 [89.448, 103.568] | 22.432 | 49.6 |
| persistent-4096-major | 1 | candidate | 17.528 [17.339, 18.819] | 0.000 | 32.7 |
| persistent-4096-major | 2 | baseline | 92.833 [90.481, 98.026] | 19.599 | 49.4 |
| persistent-4096-major | 2 | candidate | 17.529 [17.187, 18.124] | 0.000 | 32.8 |
| persistent-4096-major | 4 | baseline | 101.005 [87.473, 106.314] | 22.809 | 49.7 |
| persistent-4096-major | 4 | candidate | 17.365 [17.180, 17.726] | 0.000 | 32.8 |
| persistent-4096-major | 8 | baseline | 96.510 [91.882, 110.997] | 21.439 | 48.9 |
| persistent-4096-major | 8 | candidate | 17.504 [17.301, 18.218] | 0.000 | 33.2 |
| persistent-64-major | 1 | baseline | 53.705 [51.678, 59.666] | 11.407 | 47.1 |
| persistent-64-major | 1 | candidate | 14.193 [13.964, 15.128] | 0.000 | 30.8 |
| persistent-64-major | 2 | baseline | 56.024 [50.754, 57.750] | 12.708 | 47.7 |
| persistent-64-major | 2 | candidate | 14.135 [13.998, 15.652] | 0.000 | 30.8 |
| persistent-64-major | 4 | baseline | 53.338 [49.877, 62.204] | 11.970 | 47.5 |
| persistent-64-major | 4 | candidate | 14.015 [13.841, 14.419] | 0.000 | 31.1 |
| persistent-64-major | 8 | baseline | 55.879 [52.209, 57.870] | 12.448 | 47.6 |
| persistent-64-major | 8 | candidate | 14.222 [14.134, 16.165] | 0.000 | 30.7 |
| records-window | 1 | baseline | 122.700 [120.387, 124.326] | 20.151 | 52.1 |
| records-window | 1 | candidate | 122.124 [120.683, 125.816] | 20.208 | 52.8 |
| records-window | 2 | baseline | 121.258 [119.429, 124.546] | 20.274 | 51.8 |
| records-window | 2 | candidate | 123.774 [119.663, 136.416] | 19.596 | 52.4 |
| records-window | 4 | baseline | 123.936 [120.571, 127.034] | 20.594 | 51.8 |
| records-window | 4 | candidate | 121.866 [119.381, 124.332] | 20.335 | 52.2 |
| records-window | 8 | baseline | 122.508 [120.140, 127.142] | 20.502 | 52.5 |
| records-window | 8 | candidate | 121.960 [120.374, 127.296] | 20.324 | 52.0 |
| scalar-snapshots | 1 | baseline | 12.637 [11.625, 14.807] | 2.583 | 51.5 |
| scalar-snapshots | 1 | candidate | 12.449 [11.562, 13.449] | 2.830 | 51.6 |
| scalar-snapshots | 2 | baseline | 12.445 [12.203, 16.364] | 2.754 | 51.3 |
| scalar-snapshots | 2 | candidate | 12.432 [12.100, 14.806] | 2.699 | 51.4 |
| scalar-snapshots | 4 | baseline | 12.068 [11.908, 13.887] | 2.532 | 51.6 |
| scalar-snapshots | 4 | candidate | 12.382 [11.805, 14.200] | 2.641 | 51.7 |
| scalar-snapshots | 8 | baseline | 12.090 [11.560, 13.318] | 2.495 | 51.6 |
| scalar-snapshots | 8 | candidate | 13.007 [12.044, 13.309] | 2.736 | 51.6 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| entries-stored | 1 | +1.3% | +2.9% | +0.2% | 1 / 1 |
| entries-stored | 2 | -1.6% | +2.9% | +0.9% | 1 / 1 |
| entries-stored | 4 | +1.5% | +0.4% | +0.3% | 1 / 1 |
| entries-stored | 8 | -0.6% | -0.4% | +3.4% | 1 / 1 |
| parallel-maps | 1 | -84.9% | -100.0% | -21.9% | 63 / 0 |
| parallel-maps | 2 | -88.2% | -100.0% | -34.0% | 63 / 0 |
| parallel-maps | 4 | -90.1% | -100.0% | -44.1% | 62 / 0 |
| parallel-maps | 8 | -89.5% | -100.0% | -50.9% | 62 / 0 |
| persistent-4096-major | 1 | -82.5% | -100.0% | -34.0% | 23 / 0 |
| persistent-4096-major | 2 | -81.1% | -100.0% | -33.7% | 23 / 0 |
| persistent-4096-major | 4 | -82.8% | -100.0% | -33.9% | 23 / 0 |
| persistent-4096-major | 8 | -81.9% | -100.0% | -32.2% | 23 / 0 |
| persistent-64-major | 1 | -73.6% | -100.0% | -34.6% | 14 / 0 |
| persistent-64-major | 2 | -74.8% | -100.0% | -35.5% | 14 / 0 |
| persistent-64-major | 4 | -73.7% | -100.0% | -34.6% | 14 / 0 |
| persistent-64-major | 8 | -74.5% | -100.0% | -35.4% | 14 / 0 |
| records-window | 1 | -0.5% | +0.3% | +1.4% | 16 / 16 |
| records-window | 2 | +2.1% | -3.3% | +1.0% | 16 / 16 |
| records-window | 4 | -1.7% | -1.3% | +0.8% | 16 / 16 |
| records-window | 8 | -0.4% | -0.9% | -0.9% | 16 / 16 |
| scalar-snapshots | 1 | -1.5% | +9.6% | +0.1% | 2 / 2 |
| scalar-snapshots | 2 | -0.1% | -2.0% | +0.2% | 2 / 2 |
| scalar-snapshots | 4 | +2.6% | +4.3% | +0.2% | 2 / 2 |
| scalar-snapshots | 8 | +7.6% | +9.7% | +0.1% | 2 / 2 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| entries-stored | 1 | 7 | +1.4% [-1.5%, +4.7%] | 2 / 7 |
| entries-stored | 2 | 7 | -0.6% [-5.7%, +7.7%] | 4 / 7 |
| entries-stored | 4 | 7 | +0.7% [-1.6%, +6.2%] | 3 / 7 |
| entries-stored | 8 | 7 | -1.8% [-8.3%, +3.9%] | 5 / 7 |
| parallel-maps | 1 | 7 | -84.8% [-85.5%, -83.5%] | 7 / 7 |
| parallel-maps | 2 | 7 | -88.3% [-88.6%, -88.1%] | 7 / 7 |
| parallel-maps | 4 | 7 | -90.3% [-90.8%, -89.4%] | 7 / 7 |
| parallel-maps | 8 | 7 | -89.6% [-91.9%, -87.2%] | 7 / 7 |
| persistent-4096-major | 1 | 7 | -81.3% [-83.2%, -80.4%] | 7 / 7 |
| persistent-4096-major | 2 | 7 | -81.1% [-82.0%, -80.1%] | 7 / 7 |
| persistent-4096-major | 4 | 7 | -82.6% [-83.7%, -80.3%] | 7 / 7 |
| persistent-4096-major | 8 | 7 | -82.0% [-83.8%, -81.0%] | 7 / 7 |
| persistent-64-major | 1 | 7 | -73.6% [-76.6%, -70.7%] | 7 / 7 |
| persistent-64-major | 2 | 7 | -75.0% [-75.3%, -69.6%] | 7 / 7 |
| persistent-64-major | 4 | 7 | -73.7% [-77.7%, -72.0%] | 7 / 7 |
| persistent-64-major | 8 | 7 | -74.2% [-75.6%, -71.8%] | 7 / 7 |
| records-window | 1 | 7 | +0.9% [-2.0%, +2.2%] | 3 / 7 |
| records-window | 2 | 7 | +2.0% [-3.9%, +11.9%] | 3 / 7 |
| records-window | 4 | 7 | -1.2% [-3.7%, +0.8%] | 5 / 7 |
| records-window | 8 | 7 | +0.0% [-4.8%, +5.2%] | 3 / 7 |
| scalar-snapshots | 1 | 7 | +5.6% [-15.9%, +14.0%] | 3 / 7 |
| scalar-snapshots | 2 | 7 | -0.8% [-24.0%, +19.0%] | 4 / 7 |
| scalar-snapshots | 4 | 7 | +2.6% [-12.9%, +18.6%] | 2 / 7 |
| scalar-snapshots | 8 | 7 | +2.5% [-2.2%, +15.1%] | 1 / 7 |

Diagnostic GC phases summed across measured repetitions (wall time):
