| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| bytes-transient | 1 | baseline | 93.728 [90.361, 101.413] | 0.288 | 42.9 |
| bytes-transient | 1 | candidate | 92.059 [90.894, 99.389] | 0.297 | 42.8 |
| bytes-transient | 8 | baseline | 90.787 [84.484, 96.958] | 0.290 | 42.8 |
| bytes-transient | 8 | candidate | 91.770 [85.979, 96.980] | 0.283 | 42.9 |
| map-api-composed | 1 | baseline | 121.765 [116.820, 133.188] | 12.108 | 45.4 |
| map-api-composed | 1 | candidate | 121.443 [117.689, 128.397] | 12.081 | 45.4 |
| map-api-composed | 8 | baseline | 127.664 [124.076, 135.645] | 12.071 | 46.3 |
| map-api-composed | 8 | candidate | 126.911 [120.830, 135.276] | 11.811 | 46.2 |
| map-api-parallel | 1 | baseline | 156.258 [152.401, 163.205] | 21.338 | 56.3 |
| map-api-parallel | 1 | candidate | 157.964 [152.468, 166.080] | 21.955 | 56.2 |
| map-api-parallel | 8 | baseline | 58.085 [52.948, 62.703] | 16.773 | 75.7 |
| map-api-parallel | 8 | candidate | 55.488 [55.145, 60.051] | 15.904 | 76.1 |
| map-api-projections | 1 | baseline | 9.472 [9.188, 9.945] | 0.000 | 30.1 |
| map-api-projections | 1 | candidate | 9.546 [9.358, 10.332] | 0.000 | 30.2 |
| map-api-projections | 8 | baseline | 12.486 [12.290, 12.576] | 0.000 | 31.3 |
| map-api-projections | 8 | candidate | 12.575 [12.050, 18.242] | 0.000 | 31.2 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| bytes-transient | 1 | -1.8% | +3.1% | -0.3% | 1 / 1 |
| bytes-transient | 8 | +1.1% | -2.4% | +0.3% | 1 / 1 |
| map-api-composed | 1 | -0.3% | -0.2% | +0.0% | 19 / 19 |
| map-api-composed | 8 | -0.6% | -2.1% | -0.3% | 19 / 19 |
| map-api-parallel | 1 | +1.1% | +2.9% | -0.2% | 19 / 19 |
| map-api-parallel | 8 | -4.5% | -5.2% | +0.6% | 19 / 19 |
| map-api-projections | 1 | +0.8% | n/a | +0.4% | 0 / 0 |
| map-api-projections | 8 | +0.7% | n/a | -0.1% | 0 / 0 |

Diagnostic GC phases summed across measured repetitions (wall time):
