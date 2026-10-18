| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| balanced | 4 | baseline | 155.118 [153.513, 173.571] | 39.042 | 99.7 |
| balanced | 4 | candidate | 158.115 [155.608, 163.405] | 39.121 | 102.8 |
| balanced | 8 | baseline | 108.324 [102.910, 124.592] | 33.371 | 115.2 |
| balanced | 8 | candidate | 109.102 [102.568, 124.934] | 34.461 | 111.0 |
| cpu | 4 | baseline | 253.732 [251.647, 262.898] | 39.639 | 96.6 |
| cpu | 4 | candidate | 258.105 [253.453, 279.042] | 39.457 | 101.0 |
| cpu | 8 | baseline | 300.072 [292.985, 323.285] | 39.461 | 124.7 |
| cpu | 8 | candidate | 294.665 [281.486, 306.912] | 39.834 | 125.7 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| balanced | 4 | +1.9% | +0.2% | +3.1% | 22 / 22 |
| balanced | 8 | +0.7% | +3.3% | -3.7% | 21 / 21 |
| cpu | 4 | +1.7% | -0.5% | +4.6% | 22 / 22 |
| cpu | 8 | -1.8% | +0.9% | +0.8% | 22 / 22 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| balanced | 4 | 5 | +1.9% [-8.2%, +6.4%] | 2 / 5 |
| balanced | 8 | 5 | +2.0% [-14.1%, +15.3%] | 2 / 5 |
| cpu | 4 | 5 | +1.7% [-2.6%, +10.9%] | 1 / 5 |
| cpu | 8 | 5 | -1.8% [-12.5%, +4.3%] | 3 / 5 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| balanced | 4 | baseline | 81920 | 5.120 | 11.776 |
| balanced | 4 | candidate | 81920 | 5.120 | 11.776 |
| balanced | 8 | baseline | 81920 | 3.456 | 9.216 |
| balanced | 8 | candidate | 81920 | 3.584 | 10.752 |
| cpu | 4 | baseline | 81920 | 8.704 | 14.848 |
| cpu | 4 | candidate | 81920 | 8.704 | 21.504 |
| cpu | 8 | baseline | 81920 | 10.240 | 19.456 |
| cpu | 8 | candidate | 81920 | 9.728 | 20.480 |

Diagnostic GC phases summed across measured repetitions (wall time):
