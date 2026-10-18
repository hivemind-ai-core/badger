| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| buffers-pointers | 1 | baseline | 18.030 [17.061, 19.399] | 1.384 | 47.8 |
| buffers-pointers | 1 | candidate | 17.887 [17.441, 20.711] | 1.319 | 48.1 |
| buffers-pointers | 8 | baseline | 24.596 [23.809, 27.909] | 1.444 | 48.3 |
| buffers-pointers | 8 | candidate | 24.486 [24.036, 26.106] | 1.321 | 48.0 |
| persistent-64-major | 1 | baseline | 14.364 [14.080, 15.499] | 0.000 | 30.0 |
| persistent-64-major | 1 | candidate | 14.339 [14.102, 19.453] | 0.000 | 30.1 |
| persistent-64-major | 8 | baseline | 14.377 [14.077, 16.902] | 0.000 | 30.0 |
| persistent-64-major | 8 | candidate | 14.544 [14.176, 17.873] | 0.000 | 30.1 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| buffers-pointers | 1 | -0.8% | -4.7% | +0.7% | 1 / 1 |
| buffers-pointers | 8 | -0.4% | -8.5% | -0.6% | 1 / 1 |
| persistent-64-major | 1 | -0.2% | n/a | +0.3% | 0 / 0 |
| persistent-64-major | 8 | +1.2% | n/a | +0.5% | 0 / 0 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| buffers-pointers | 1 | 15 | -0.5% [-4.1%, +13.3%] | 9 / 15 |
| buffers-pointers | 8 | 15 | -0.4% [-13.9%, +8.5%] | 8 / 15 |
| persistent-64-major | 1 | 15 | -0.3% [-7.5%, +36.2%] | 8 / 15 |
| persistent-64-major | 8 | 15 | +0.8% [-14.8%, +21.5%] | 5 / 15 |

Diagnostic GC phases summed across measured repetitions (wall time):
