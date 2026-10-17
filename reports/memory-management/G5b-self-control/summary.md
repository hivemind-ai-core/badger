| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| buffers-capacity-33 | 1 | baseline | 23.973 [22.761, 26.292] | 1.151 | 38.2 |
| buffers-capacity-33 | 1 | candidate | 24.010 [22.631, 25.141] | 1.113 | 38.1 |
| buffers-capacity-33 | 8 | baseline | 24.425 [23.609, 26.187] | 1.158 | 38.4 |
| buffers-capacity-33 | 8 | candidate | 24.750 [23.027, 25.481] | 1.180 | 38.6 |
| buffers-empty | 1 | baseline | 13.044 [12.926, 13.319] | 1.060 | 47.0 |
| buffers-empty | 1 | candidate | 12.978 [12.764, 13.394] | 1.069 | 47.1 |
| buffers-empty | 8 | baseline | 13.045 [12.958, 13.937] | 1.044 | 47.1 |
| buffers-empty | 8 | candidate | 13.190 [12.789, 13.419] | 1.062 | 47.2 |
| persistent-4096 | 1 | baseline | 29.350 [27.260, 35.216] | 6.203 | 45.5 |
| persistent-4096 | 1 | candidate | 27.542 [27.241, 31.735] | 6.075 | 45.7 |
| persistent-4096 | 8 | baseline | 29.768 [28.645, 32.018] | 6.711 | 47.4 |
| persistent-4096 | 8 | candidate | 29.731 [28.152, 33.990] | 6.684 | 47.2 |
| words-transient | 1 | baseline | 117.045 [111.632, 122.783] | 1.377 | 39.8 |
| words-transient | 1 | candidate | 117.755 [114.280, 123.279] | 1.348 | 39.3 |
| words-transient | 8 | baseline | 118.208 [112.657, 121.863] | 1.359 | 39.8 |
| words-transient | 8 | candidate | 119.526 [112.732, 121.936] | 1.366 | 39.6 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| buffers-capacity-33 | 1 | +0.2% | -3.3% | -0.3% | 1 / 1 |
| buffers-capacity-33 | 8 | +1.3% | +1.8% | +0.4% | 1 / 1 |
| buffers-empty | 1 | -0.5% | +0.8% | +0.3% | 1 / 1 |
| buffers-empty | 8 | +1.1% | +1.7% | +0.4% | 1 / 1 |
| persistent-4096 | 1 | -6.2% | -2.1% | +0.3% | 5 / 5 |
| persistent-4096 | 8 | -0.1% | -0.4% | -0.4% | 5 / 5 |
| words-transient | 1 | +0.6% | -2.1% | -1.3% | 8 / 8 |
| words-transient | 8 | +1.1% | +0.5% | -0.4% | 8 / 8 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| buffers-capacity-33 | 1 | 5 | -1.2% [-5.6%, +5.4%] | 3 / 5 |
| buffers-capacity-33 | 8 | 5 | -2.3% [-5.7%, +6.8%] | 3 / 5 |
| buffers-empty | 1 | 5 | -0.5% [-1.5%, +0.6%] | 4 / 5 |
| buffers-empty | 8 | 5 | -0.9% [-4.4%, +3.6%] | 4 / 5 |
| persistent-4096 | 1 | 5 | -0.5% [-18.0%, +8.1%] | 4 / 5 |
| persistent-4096 | 8 | 5 | -2.0% [-7.1%, +18.7%] | 3 / 5 |
| words-transient | 1 | 5 | +0.1% [-3.1%, +8.6%] | 2 / 5 |
| words-transient | 8 | 5 | -1.0% [-4.6%, +6.1%] | 3 / 5 |

Diagnostic GC phases summed across measured repetitions (wall time):
