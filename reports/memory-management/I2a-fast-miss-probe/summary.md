| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| lookup-list-misses | 1 | baseline | 7.841 [7.420, 8.255] | 0.000 | 28.8 |
| lookup-list-misses | 1 | candidate | 7.795 [7.233, 9.097] | 0.000 | 29.3 |
| lookup-list-misses | 8 | baseline | 7.863 [7.159, 8.798] | 0.000 | 29.1 |
| lookup-list-misses | 8 | candidate | 7.337 [7.000, 8.573] | 0.000 | 29.4 |
| lookup-map-misses | 1 | baseline | 17.397 [16.260, 19.421] | 0.000 | 29.0 |
| lookup-map-misses | 1 | candidate | 15.993 [14.918, 17.269] | 0.000 | 29.1 |
| lookup-map-misses | 8 | baseline | 17.311 [16.486, 22.605] | 0.000 | 29.4 |
| lookup-map-misses | 8 | candidate | 15.197 [14.540, 18.014] | 0.000 | 29.5 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| lookup-list-misses | 1 | -0.6% | n/a | +1.6% | 0 / 0 |
| lookup-list-misses | 8 | -6.7% | n/a | +0.9% | 0 / 0 |
| lookup-map-misses | 1 | -8.1% | n/a | +0.7% | 0 / 0 |
| lookup-map-misses | 8 | -12.2% | n/a | +0.1% | 0 / 0 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| lookup-list-misses | 1 | 9 | +1.8% [-6.1%, +12.2%] | 2 / 9 |
| lookup-list-misses | 8 | 9 | -3.9% [-19.7%, +3.3%] | 8 / 9 |
| lookup-map-misses | 1 | 9 | -7.7% [-15.7%, +1.9%] | 8 / 9 |
| lookup-map-misses | 8 | 9 | -10.5% [-27.3%, -3.7%] | 9 / 9 |

Diagnostic GC phases summed across measured repetitions (wall time):
