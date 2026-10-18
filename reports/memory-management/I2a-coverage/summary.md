| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| lookup-array-int | 1 | baseline | 3.002 [2.853, 3.118] | 0.000 | 28.7 |
| lookup-array-int | 1 | candidate | 2.972 [2.816, 3.080] | 0.000 | 28.9 |
| lookup-array-int | 2 | baseline | 3.037 [3.016, 4.558] | 0.000 | 29.0 |
| lookup-array-int | 2 | candidate | 3.044 [3.014, 3.825] | 0.000 | 28.7 |
| lookup-array-int | 4 | baseline | 3.026 [2.998, 3.537] | 0.000 | 29.1 |
| lookup-array-int | 4 | candidate | 3.028 [2.947, 4.476] | 0.000 | 28.5 |
| lookup-array-int | 8 | baseline | 3.049 [2.960, 4.538] | 0.000 | 29.0 |
| lookup-array-int | 8 | candidate | 2.998 [2.847, 3.694] | 0.000 | 29.0 |
| lookup-list-pointers | 1 | baseline | 26.682 [25.947, 28.527] | 1.178 | 51.1 |
| lookup-list-pointers | 1 | candidate | 19.650 [18.846, 21.550] | 0.000 | 28.9 |
| lookup-list-pointers | 2 | baseline | 31.786 [31.117, 34.040] | 1.188 | 50.7 |
| lookup-list-pointers | 2 | candidate | 27.710 [26.075, 29.999] | 0.000 | 28.7 |
| lookup-list-pointers | 4 | baseline | 32.159 [30.298, 33.989] | 1.169 | 51.1 |
| lookup-list-pointers | 4 | candidate | 28.201 [25.372, 28.530] | 0.000 | 28.9 |
| lookup-list-pointers | 8 | baseline | 31.909 [31.206, 34.595] | 1.233 | 51.3 |
| lookup-list-pointers | 8 | candidate | 27.015 [26.081, 27.765] | 0.000 | 28.7 |
| lookup-map-int | 1 | baseline | 29.401 [28.648, 31.446] | 1.227 | 50.4 |
| lookup-map-int | 1 | candidate | 22.006 [20.235, 22.402] | 0.000 | 29.0 |
| lookup-map-int | 2 | baseline | 29.419 [28.500, 30.764] | 1.184 | 50.4 |
| lookup-map-int | 2 | candidate | 20.513 [19.257, 22.149] | 0.000 | 28.9 |
| lookup-map-int | 4 | baseline | 29.231 [27.027, 31.459] | 1.198 | 50.9 |
| lookup-map-int | 4 | candidate | 21.150 [20.197, 22.413] | 0.000 | 29.2 |
| lookup-map-int | 8 | baseline | 28.719 [28.436, 30.167] | 1.201 | 50.9 |
| lookup-map-int | 8 | candidate | 20.421 [19.562, 22.063] | 0.000 | 28.7 |
| lookup-map-pointers | 1 | baseline | 36.589 [35.385, 41.995] | 1.206 | 51.8 |
| lookup-map-pointers | 1 | candidate | 27.855 [25.526, 29.385] | 0.000 | 30.8 |
| lookup-map-pointers | 2 | baseline | 43.075 [41.857, 43.442] | 1.217 | 50.6 |
| lookup-map-pointers | 2 | candidate | 34.705 [33.737, 36.609] | 0.000 | 31.0 |
| lookup-map-pointers | 4 | baseline | 42.105 [40.423, 45.440] | 1.196 | 51.1 |
| lookup-map-pointers | 4 | candidate | 34.984 [34.444, 38.671] | 0.000 | 30.6 |
| lookup-map-pointers | 8 | baseline | 43.270 [40.978, 47.863] | 1.189 | 51.2 |
| lookup-map-pointers | 8 | candidate | 34.505 [33.232, 36.189] | 0.000 | 30.8 |
| lookup-map-stored | 1 | baseline | 38.915 [37.763, 72.233] | 2.397 | 59.0 |
| lookup-map-stored | 1 | candidate | 38.309 [37.301, 41.891] | 2.478 | 59.1 |
| lookup-map-stored | 2 | baseline | 40.368 [38.413, 43.614] | 2.472 | 59.0 |
| lookup-map-stored | 2 | candidate | 38.720 [38.279, 41.049] | 2.348 | 59.2 |
| lookup-map-stored | 4 | baseline | 37.829 [36.391, 39.972] | 2.439 | 58.6 |
| lookup-map-stored | 4 | candidate | 39.886 [37.468, 41.239] | 2.367 | 58.8 |
| lookup-map-stored | 8 | baseline | 40.473 [37.862, 41.537] | 2.480 | 58.6 |
| lookup-map-stored | 8 | candidate | 39.762 [38.349, 43.044] | 2.370 | 59.0 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| lookup-array-int | 1 | -1.0% | n/a | +0.9% | 0 / 0 |
| lookup-array-int | 2 | +0.2% | n/a | -1.3% | 0 / 0 |
| lookup-array-int | 4 | +0.1% | n/a | -1.9% | 0 / 0 |
| lookup-array-int | 8 | -1.7% | n/a | +0.1% | 0 / 0 |
| lookup-list-pointers | 1 | -26.4% | -100.0% | -43.5% | 1 / 0 |
| lookup-list-pointers | 2 | -12.8% | -100.0% | -43.5% | 1 / 0 |
| lookup-list-pointers | 4 | -12.3% | -100.0% | -43.3% | 1 / 0 |
| lookup-list-pointers | 8 | -15.3% | -100.0% | -44.0% | 1 / 0 |
| lookup-map-int | 1 | -25.2% | -100.0% | -42.4% | 1 / 0 |
| lookup-map-int | 2 | -30.3% | -100.0% | -42.6% | 1 / 0 |
| lookup-map-int | 4 | -27.6% | -100.0% | -42.8% | 1 / 0 |
| lookup-map-int | 8 | -28.9% | -100.0% | -43.7% | 1 / 0 |
| lookup-map-pointers | 1 | -23.9% | -100.0% | -40.5% | 1 / 0 |
| lookup-map-pointers | 2 | -19.4% | -100.0% | -38.6% | 1 / 0 |
| lookup-map-pointers | 4 | -16.9% | -100.0% | -40.2% | 1 / 0 |
| lookup-map-pointers | 8 | -20.3% | -100.0% | -39.8% | 1 / 0 |
| lookup-map-stored | 1 | -1.6% | +3.4% | +0.2% | 1 / 1 |
| lookup-map-stored | 2 | -4.1% | -5.0% | +0.2% | 1 / 1 |
| lookup-map-stored | 4 | +5.4% | -3.0% | +0.2% | 1 / 1 |
| lookup-map-stored | 8 | -1.8% | -4.4% | +0.6% | 1 / 1 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| lookup-array-int | 1 | 7 | -1.2% [-6.3%, +4.2%] | 4 / 7 |
| lookup-array-int | 2 | 7 | +0.2% [-31.6%, +23.4%] | 3 / 7 |
| lookup-array-int | 4 | 7 | +0.5% [-2.4%, +26.5%] | 3 / 7 |
| lookup-array-int | 8 | 7 | +0.1% [-25.1%, +12.4%] | 3 / 7 |
| lookup-list-pointers | 1 | 7 | -27.0% [-29.3%, -19.2%] | 7 / 7 |
| lookup-list-pointers | 2 | 7 | -12.9% [-18.1%, -10.6%] | 7 / 7 |
| lookup-list-pointers | 4 | 7 | -14.7% [-19.5%, -8.0%] | 7 / 7 |
| lookup-list-pointers | 8 | 7 | -14.2% [-21.2%, -13.0%] | 7 / 7 |
| lookup-map-int | 1 | 7 | -25.3% [-33.5%, -22.6%] | 7 / 7 |
| lookup-map-int | 2 | 7 | -29.0% [-37.4%, -26.6%] | 7 / 7 |
| lookup-map-int | 4 | 7 | -27.7% [-31.8%, -19.6%] | 7 / 7 |
| lookup-map-int | 8 | 7 | -29.7% [-31.9%, -25.6%] | 7 / 7 |
| lookup-map-pointers | 1 | 7 | -23.3% [-33.7%, -19.7%] | 7 / 7 |
| lookup-map-pointers | 2 | 7 | -19.3% [-21.7%, -12.5%] | 7 / 7 |
| lookup-map-pointers | 4 | 7 | -15.5% [-18.8%, -14.4%] | 7 / 7 |
| lookup-map-pointers | 8 | 7 | -18.6% [-30.6%, -14.1%] | 7 / 7 |
| lookup-map-stored | 1 | 7 | +0.0% [-42.1%, +7.2%] | 3 / 7 |
| lookup-map-stored | 2 | 7 | -0.3% [-11.2%, +1.8%] | 4 / 7 |
| lookup-map-stored | 4 | 7 | +2.3% [-1.6%, +10.3%] | 1 / 7 |
| lookup-map-stored | 8 | 7 | -0.4% [-5.4%, +3.6%] | 4 / 7 |

Diagnostic GC phases summed across measured repetitions (wall time):
