| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| lookup-list-misses | 1 | baseline | 7.274 [7.231, 9.372] | 0.000 | 28.9 |
| lookup-list-misses | 1 | candidate | 9.844 [9.101, 11.184] | 0.000 | 29.1 |
| lookup-list-misses | 8 | baseline | 7.336 [6.868, 8.846] | 0.000 | 29.3 |
| lookup-list-misses | 8 | candidate | 9.458 [8.856, 10.823] | 0.000 | 29.4 |
| lookup-map-misses | 1 | baseline | 16.102 [15.354, 18.177] | 0.000 | 28.8 |
| lookup-map-misses | 1 | candidate | 17.155 [16.619, 20.174] | 0.000 | 28.9 |
| lookup-map-misses | 8 | baseline | 16.287 [15.741, 17.451] | 0.000 | 29.3 |
| lookup-map-misses | 8 | candidate | 16.562 [15.699, 19.033] | 0.000 | 29.4 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| lookup-list-misses | 1 | +35.3% | n/a | +0.8% | 0 / 0 |
| lookup-list-misses | 8 | +28.9% | n/a | +0.0% | 0 / 0 |
| lookup-map-misses | 1 | +6.5% | n/a | +0.4% | 0 / 0 |
| lookup-map-misses | 8 | +1.7% | n/a | +0.6% | 0 / 0 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| lookup-list-misses | 1 | 9 | +33.3% [+19.3%, +41.0%] | 0 / 9 |
| lookup-list-misses | 8 | 9 | +29.0% [+7.0%, +47.5%] | 0 / 9 |
| lookup-map-misses | 1 | 9 | +10.0% [-4.0%, +14.3%] | 1 / 9 |
| lookup-map-misses | 8 | 9 | +1.0% [-4.1%, +14.0%] | 3 / 9 |

Diagnostic GC phases summed across measured repetitions (wall time):
