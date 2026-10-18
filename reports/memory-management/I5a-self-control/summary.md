| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| parallel-maps | 1 | baseline | 193.781 [183.554, 217.004] | 50.694 | 44.4 |
| parallel-maps | 1 | candidate | 192.965 [186.826, 206.720] | 48.608 | 42.7 |
| parallel-maps | 8 | baseline | 67.178 [58.896, 72.474] | 24.122 | 77.5 |
| parallel-maps | 8 | candidate | 63.635 [61.576, 68.113] | 21.937 | 77.2 |
| persistent-4096-major | 1 | baseline | 94.532 [91.264, 100.249] | 20.694 | 49.5 |
| persistent-4096-major | 1 | candidate | 92.448 [90.163, 101.190] | 20.225 | 49.6 |
| persistent-4096-major | 8 | baseline | 96.623 [92.878, 103.493] | 21.350 | 49.4 |
| persistent-4096-major | 8 | candidate | 95.777 [89.593, 104.415] | 21.878 | 49.5 |
| scalar-snapshots | 1 | baseline | 12.379 [11.722, 14.911] | 2.737 | 51.4 |
| scalar-snapshots | 1 | candidate | 12.167 [12.017, 13.987] | 2.605 | 51.4 |
| scalar-snapshots | 8 | baseline | 12.707 [11.726, 13.945] | 2.800 | 51.8 |
| scalar-snapshots | 8 | candidate | 12.544 [12.262, 12.685] | 2.731 | 51.8 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| parallel-maps | 1 | -0.4% | -4.1% | -3.6% | 63 / 63 |
| parallel-maps | 8 | -5.3% | -9.1% | -0.4% | 62 / 62 |
| persistent-4096-major | 1 | -2.2% | -2.3% | +0.1% | 23 / 23 |
| persistent-4096-major | 8 | -0.9% | +2.5% | +0.1% | 23 / 23 |
| scalar-snapshots | 1 | -1.7% | -4.8% | +0.1% | 2 / 2 |
| scalar-snapshots | 8 | -1.3% | -2.5% | +0.0% | 2 / 2 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| parallel-maps | 1 | 5 | -0.9% [-8.3%, +1.8%] | 3 / 5 |
| parallel-maps | 8 | 5 | -6.0% [-12.2%, +8.2%] | 3 / 5 |
| persistent-4096-major | 1 | 5 | +0.7% [-8.0%, +8.5%] | 2 / 5 |
| persistent-4096-major | 8 | 5 | -3.2% [-7.3%, +12.4%] | 4 / 5 |
| scalar-snapshots | 1 | 5 | -1.7% [-19.3%, +17.1%] | 3 / 5 |
| scalar-snapshots | 8 | 5 | -3.5% [-9.4%, +6.2%] | 4 / 5 |

Diagnostic GC phases summed across measured repetitions (wall time):
