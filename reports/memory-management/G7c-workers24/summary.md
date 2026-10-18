| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| buffers-pointers | 2 | baseline | 23.932 [23.772, 25.228] | 1.369 | 50.4 |
| buffers-pointers | 2 | candidate | 23.718 [22.899, 24.388] | 1.353 | 47.8 |
| buffers-pointers | 4 | baseline | 23.945 [23.811, 25.486] | 1.404 | 47.6 |
| buffers-pointers | 4 | candidate | 23.407 [23.242, 24.073] | 1.335 | 47.6 |
| parallel-snapshots | 2 | baseline | 239.112 [231.481, 251.823] | 88.612 | 77.8 |
| parallel-snapshots | 2 | candidate | 232.820 [230.405, 239.174] | 85.071 | 76.8 |
| parallel-snapshots | 4 | baseline | 238.056 [236.256, 240.089] | 96.585 | 101.1 |
| parallel-snapshots | 4 | candidate | 235.675 [230.458, 246.617] | 93.577 | 98.3 |
| persistent-64-major | 2 | baseline | 14.618 [14.187, 15.347] | 0.000 | 29.8 |
| persistent-64-major | 2 | candidate | 14.408 [14.160, 14.649] | 0.000 | 30.4 |
| persistent-64-major | 4 | baseline | 14.933 [14.183, 16.263] | 0.000 | 31.8 |
| persistent-64-major | 4 | candidate | 14.560 [14.314, 15.360] | 0.000 | 30.2 |
| records-window | 2 | baseline | 129.277 [128.210, 133.174] | 23.267 | 50.9 |
| records-window | 2 | candidate | 127.146 [126.015, 130.565] | 22.293 | 50.8 |
| records-window | 4 | baseline | 127.452 [126.295, 131.107] | 23.089 | 51.0 |
| records-window | 4 | candidate | 124.488 [123.694, 126.074] | 22.092 | 51.1 |
| scalar-snapshots | 2 | baseline | 14.252 [13.042, 15.662] | 3.487 | 50.7 |
| scalar-snapshots | 2 | candidate | 14.370 [13.342, 15.589] | 3.012 | 50.8 |
| scalar-snapshots | 4 | baseline | 13.886 [12.431, 14.811] | 3.141 | 50.6 |
| scalar-snapshots | 4 | candidate | 13.629 [13.119, 14.742] | 2.933 | 50.6 |
| snapshots-major | 2 | baseline | 106.967 [104.552, 127.833] | 27.326 | 48.8 |
| snapshots-major | 2 | candidate | 105.219 [102.618, 114.077] | 26.431 | 48.9 |
| snapshots-major | 4 | baseline | 109.903 [105.477, 117.888] | 27.621 | 49.0 |
| snapshots-major | 4 | candidate | 106.569 [105.306, 108.033] | 26.614 | 49.3 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| buffers-pointers | 2 | -0.9% | -1.2% | -5.2% | 1 / 1 |
| buffers-pointers | 4 | -2.2% | -4.9% | -0.0% | 1 / 1 |
| parallel-snapshots | 2 | -2.6% | -4.0% | -1.3% | 37 / 37 |
| parallel-snapshots | 4 | -1.0% | -3.1% | -2.8% | 36 / 36 |
| persistent-64-major | 2 | -1.4% | n/a | +2.0% | 0 / 0 |
| persistent-64-major | 4 | -2.5% | n/a | -5.0% | 0 / 0 |
| records-window | 2 | -1.6% | -4.2% | -0.2% | 16 / 16 |
| records-window | 4 | -2.3% | -4.3% | +0.2% | 16 / 16 |
| scalar-snapshots | 2 | +0.8% | -13.6% | +0.2% | 2 / 2 |
| scalar-snapshots | 4 | -1.8% | -6.6% | -0.1% | 2 / 2 |
| snapshots-major | 2 | -1.6% | -3.3% | +0.2% | 17 / 17 |
| snapshots-major | 4 | -3.0% | -3.6% | +0.5% | 17 / 17 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| buffers-pointers | 2 | 5 | -2.2% [-3.7%, -0.1%] | 5 / 5 |
| buffers-pointers | 4 | 5 | -2.4% [-5.5%, -1.1%] | 5 / 5 |
| parallel-snapshots | 2 | 5 | -2.7% [-7.5%, -0.5%] | 5 / 5 |
| parallel-snapshots | 4 | 5 | -1.6% [-3.0%, +3.4%] | 4 / 5 |
| persistent-64-major | 2 | 5 | -2.8% [-6.1%, +3.3%] | 4 / 5 |
| persistent-64-major | 4 | 5 | -3.5% [-10.3%, +3.2%] | 3 / 5 |
| records-window | 2 | 5 | -0.8% [-5.4%, -0.1%] | 5 / 5 |
| records-window | 4 | 5 | -2.3% [-5.2%, -1.0%] | 5 / 5 |
| scalar-snapshots | 2 | 5 | +1.1% [-6.4%, +7.1%] | 2 / 5 |
| scalar-snapshots | 4 | 5 | -1.8% [-11.1%, +18.6%] | 3 / 5 |
| snapshots-major | 2 | 5 | +0.6% [-18.1%, +5.1%] | 2 / 5 |
| snapshots-major | 4 | 5 | -4.2% [-9.4%, +1.0%] | 4 / 5 |

Diagnostic GC phases summed across measured repetitions (wall time):
