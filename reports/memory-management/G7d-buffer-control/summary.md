| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| buffers-pointers | 1 | baseline | 18.022 [17.799, 19.190] | 1.382 | 48.2 |
| buffers-pointers | 1 | candidate | 18.099 [17.705, 19.715] | 1.395 | 47.9 |
| buffers-pointers | 8 | baseline | 24.419 [23.544, 26.506] | 1.373 | 48.0 |
| buffers-pointers | 8 | candidate | 24.073 [23.773, 26.727] | 1.368 | 47.9 |
| persistent-64-major | 1 | baseline | 14.285 [14.141, 15.866] | 0.000 | 29.9 |
| persistent-64-major | 1 | candidate | 14.257 [14.086, 15.112] | 0.000 | 29.9 |
| persistent-64-major | 8 | baseline | 14.453 [14.118, 16.467] | 0.000 | 30.0 |
| persistent-64-major | 8 | candidate | 14.237 [14.085, 15.188] | 0.000 | 30.0 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| buffers-pointers | 1 | +0.4% | +0.9% | -0.8% | 1 / 1 |
| buffers-pointers | 8 | -1.4% | -0.3% | -0.1% | 1 / 1 |
| persistent-64-major | 1 | -0.2% | n/a | +0.1% | 0 / 0 |
| persistent-64-major | 8 | -1.5% | n/a | +0.0% | 0 / 0 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| buffers-pointers | 1 | 15 | -0.2% [-7.5%, +8.4%] | 9 / 15 |
| buffers-pointers | 8 | 15 | +0.1% [-9.5%, +5.4%] | 7 / 15 |
| persistent-64-major | 1 | 15 | -0.6% [-11.2%, +6.9%] | 9 / 15 |
| persistent-64-major | 8 | 15 | -1.7% [-11.1%, +4.2%] | 9 / 15 |

Diagnostic GC phases summed across measured repetitions (wall time):
