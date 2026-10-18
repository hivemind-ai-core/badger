| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| parallel-maps | 1 | baseline | 33.716 [31.890, 35.477] | 0.000 | 35.0 |
| parallel-maps | 1 | candidate | 32.916 [31.035, 35.185] | 0.000 | 35.1 |
| parallel-maps | 8 | baseline | 6.263 [5.156, 9.807] | 0.000 | 36.6 |
| parallel-maps | 8 | candidate | 6.959 [4.776, 8.820] | 0.000 | 36.8 |
| persistent-64 | 1 | baseline | 3.609 [3.545, 3.915] | 0.000 | 30.8 |
| persistent-64 | 1 | candidate | 3.589 [3.518, 4.114] | 0.000 | 30.9 |
| persistent-64 | 8 | baseline | 3.546 [3.503, 4.311] | 0.000 | 30.7 |
| persistent-64 | 8 | candidate | 3.570 [3.521, 5.269] | 0.000 | 30.8 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| parallel-maps | 1 | -2.4% | n/a | +0.1% | 0 / 0 |
| parallel-maps | 8 | +11.1% | n/a | +0.5% | 0 / 0 |
| persistent-64 | 1 | -0.5% | n/a | +0.6% | 0 / 0 |
| persistent-64 | 8 | +0.7% | n/a | +0.1% | 0 / 0 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| parallel-maps | 1 | 15 | -2.7% [-6.8%, +6.1%] | 10 / 15 |
| parallel-maps | 8 | 15 | -12.4% [-35.5%, +64.5%] | 8 / 15 |
| persistent-64 | 1 | 15 | -0.3% [-5.7%, +14.0%] | 9 / 15 |
| persistent-64 | 8 | 15 | +0.6% [-18.0%, +49.7%] | 4 / 15 |

Diagnostic GC phases summed across measured repetitions (wall time):
