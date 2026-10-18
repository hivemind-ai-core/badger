| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| parallel-maps | 1 | baseline | 32.772 [31.487, 36.498] | 0.000 | 33.8 |
| parallel-maps | 1 | candidate | 32.309 [31.149, 35.350] | 0.000 | 35.0 |
| parallel-maps | 8 | baseline | 6.769 [4.887, 9.810] | 0.000 | 35.8 |
| parallel-maps | 8 | candidate | 7.105 [5.046, 10.912] | 0.000 | 37.4 |
| persistent-64 | 1 | baseline | 3.633 [3.538, 4.117] | 0.000 | 29.9 |
| persistent-64 | 1 | candidate | 3.615 [3.511, 5.113] | 0.000 | 30.5 |
| persistent-64 | 8 | baseline | 3.605 [3.568, 4.660] | 0.000 | 30.1 |
| persistent-64 | 8 | candidate | 3.578 [3.508, 3.710] | 0.000 | 31.0 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| parallel-maps | 1 | -1.4% | n/a | +3.4% | 0 / 0 |
| parallel-maps | 8 | +5.0% | n/a | +4.5% | 0 / 0 |
| persistent-64 | 1 | -0.5% | n/a | +2.1% | 0 / 0 |
| persistent-64 | 8 | -0.7% | n/a | +3.1% | 0 / 0 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| parallel-maps | 1 | 15 | -0.4% [-8.8%, +6.7%] | 8 / 15 |
| parallel-maps | 8 | 15 | +5.6% [-45.2%, +59.2%] | 6 / 15 |
| persistent-64 | 1 | 15 | -0.3% [-10.7%, +24.2%] | 8 / 15 |
| persistent-64 | 8 | 15 | -1.2% [-24.1%, +3.8%] | 10 / 15 |

Diagnostic GC phases summed across measured repetitions (wall time):
