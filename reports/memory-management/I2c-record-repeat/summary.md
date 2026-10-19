| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| records-window | 2 | baseline | 120.709 [117.823, 126.144] | 18.379 | 56.4 |
| records-window | 2 | candidate | 120.814 [118.902, 211.369] | 18.451 | 56.1 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| records-window | 2 | +0.1% | +0.4% | -0.6% | 16 / 16 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| records-window | 2 | 15 | +0.4% [-4.3%, +76.1%] | 7 / 15 |

Diagnostic GC phases summed across measured repetitions (wall time):
