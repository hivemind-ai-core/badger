| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| balanced | 4 | baseline | 165.515 [159.810, 174.399] | 40.821 | 102.2 |
| balanced | 4 | candidate | 163.603 [157.742, 166.355] | 39.895 | 99.3 |
| balanced | 8 | baseline | 118.061 [115.482, 125.674] | 34.555 | 109.1 |
| balanced | 8 | candidate | 114.138 [113.006, 119.050] | 34.648 | 111.2 |
| cpu | 4 | baseline | 269.109 [250.974, 274.618] | 39.309 | 99.6 |
| cpu | 4 | candidate | 261.888 [259.052, 288.600] | 38.057 | 100.5 |
| cpu | 8 | baseline | 326.129 [303.539, 369.211] | 40.812 | 129.3 |
| cpu | 8 | candidate | 325.226 [296.002, 363.786] | 41.377 | 126.0 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| balanced | 4 | -1.2% | -2.3% | -2.8% | 22 / 22 |
| balanced | 8 | -3.3% | +0.3% | +1.9% | 21 / 21 |
| cpu | 4 | -2.7% | -3.2% | +0.8% | 22 / 22 |
| cpu | 8 | -0.3% | +1.4% | -2.5% | 22 / 22 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| balanced | 4 | 5 | -3.2% [-4.6%, +0.6%] | 4 / 5 |
| balanced | 8 | 5 | -4.3% [-8.7%, +3.1%] | 4 / 5 |
| cpu | 4 | 5 | -0.3% [-5.7%, +15.0%] | 3 / 5 |
| cpu | 8 | 5 | -3.5% [-9.2%, +19.8%] | 4 / 5 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| balanced | 4 | baseline | 81920 | 5.376 | 12.288 |
| balanced | 4 | candidate | 81920 | 5.376 | 11.776 |
| balanced | 8 | baseline | 81920 | 3.840 | 10.240 |
| balanced | 8 | candidate | 81920 | 3.712 | 8.704 |
| cpu | 4 | baseline | 81920 | 8.704 | 15.872 |
| cpu | 4 | candidate | 81920 | 8.704 | 19.456 |
| cpu | 8 | baseline | 81920 | 11.264 | 20.480 |
| cpu | 8 | candidate | 81920 | 11.264 | 20.480 |

Diagnostic GC phases summed across measured repetitions (wall time):
