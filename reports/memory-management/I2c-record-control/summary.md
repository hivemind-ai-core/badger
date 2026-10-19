| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| records-window | 2 | baseline | 121.962 [119.708, 135.670] | 19.253 | 56.1 |
| records-window | 2 | candidate | 125.668 [118.629, 131.135] | 19.374 | 56.1 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| records-window | 2 | +3.0% | +0.6% | -0.1% | 16 / 16 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| records-window | 2 | 15 | +2.3% [-11.1%, +7.6%] | 6 / 15 |

Diagnostic GC phases summed across measured repetitions (wall time):
