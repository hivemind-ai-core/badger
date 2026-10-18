| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| scalar-snapshots | 1 | baseline | 14.239 [12.421, 18.256] | 3.544 | 51.5 |
| scalar-snapshots | 1 | candidate | 14.177 [12.890, 17.960] | 3.310 | 51.5 |
| scalar-snapshots | 8 | baseline | 14.103 [12.422, 18.085] | 3.174 | 51.6 |
| scalar-snapshots | 8 | candidate | 14.411 [12.209, 17.072] | 3.491 | 51.6 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| scalar-snapshots | 1 | -0.4% | -6.6% | +0.2% | 2 / 2 |
| scalar-snapshots | 8 | +2.2% | +10.0% | -0.0% | 2 / 2 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| scalar-snapshots | 1 | 15 | +2.8% [-24.7%, +32.2%] | 7 / 15 |
| scalar-snapshots | 8 | 15 | -0.3% [-21.1%, +21.1%] | 8 / 15 |

Diagnostic GC phases summed across measured repetitions (wall time):
