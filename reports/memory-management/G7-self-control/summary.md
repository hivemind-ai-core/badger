| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| parallel-maps | 1 | baseline | 222.378 [211.374, 229.290] | 64.671 | 43.1 |
| parallel-maps | 1 | candidate | 229.803 [224.087, 234.035] | 72.285 | 43.0 |
| parallel-maps | 8 | baseline | 68.746 [60.879, 74.527] | 24.080 | 72.6 |
| parallel-maps | 8 | candidate | 67.624 [65.711, 69.275] | 24.044 | 72.8 |
| records-window | 1 | baseline | 128.986 [125.030, 129.420] | 22.809 | 50.5 |
| records-window | 1 | candidate | 125.317 [122.036, 131.981] | 23.404 | 50.5 |
| records-window | 8 | baseline | 125.510 [124.142, 129.208] | 21.841 | 51.3 |
| records-window | 8 | candidate | 128.367 [124.277, 129.355] | 22.710 | 51.0 |
| snapshots-major | 1 | baseline | 97.347 [95.235, 100.413] | 24.595 | 48.2 |
| snapshots-major | 1 | candidate | 99.804 [95.920, 99.932] | 26.762 | 48.1 |
| snapshots-major | 8 | baseline | 103.162 [99.968, 106.005] | 26.211 | 48.4 |
| snapshots-major | 8 | candidate | 106.309 [102.453, 113.524] | 26.949 | 48.8 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| parallel-maps | 1 | +3.3% | +11.8% | -0.2% | 63 / 63 |
| parallel-maps | 8 | -1.6% | -0.1% | +0.3% | 62 / 62 |
| records-window | 1 | -2.8% | +2.6% | -0.1% | 16 / 16 |
| records-window | 8 | +2.3% | +4.0% | -0.6% | 16 / 16 |
| snapshots-major | 1 | +2.5% | +8.8% | -0.1% | 17 / 17 |
| snapshots-major | 8 | +3.0% | +2.8% | +0.6% | 17 / 17 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| parallel-maps | 1 | 5 | +2.9% [-1.2%, +10.0%] | 1 / 5 |
| parallel-maps | 8 | 5 | -1.6% [-7.8%, +7.9%] | 4 / 5 |
| records-window | 1 | 5 | -0.7% [-5.4%, +2.0%] | 3 / 5 |
| records-window | 8 | 5 | +1.2% [-0.8%, +2.9%] | 1 / 5 |
| snapshots-major | 1 | 5 | +1.8% [-1.5%, +4.9%] | 2 / 5 |
| snapshots-major | 8 | 5 | +3.7% [-1.4%, +7.1%] | 1 / 5 |

Diagnostic GC phases summed across measured repetitions (wall time):
