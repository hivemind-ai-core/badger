| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| cpu | 1 | baseline | 869.167 [864.283, 881.398] | 88.962 | 79.1 |
| cpu | 1 | candidate | 872.918 [864.289, 880.061] | 77.741 | 80.9 |
| cpu | 2 | baseline | 504.833 [492.133, 554.275] | 76.972 | 86.2 |
| cpu | 2 | candidate | 504.365 [493.331, 540.978] | 70.615 | 89.5 |
| fanout | 1 | baseline | 1173.475 [1156.862, 1268.125] | 217.371 | 83.1 |
| fanout | 1 | candidate | 1172.073 [1147.523, 1231.331] | 191.334 | 88.0 |
| fanout | 2 | baseline | 1042.344 [934.262, 1063.805] | 209.510 | 103.1 |
| fanout | 2 | candidate | 967.376 [910.681, 1048.837] | 183.719 | 109.0 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| cpu | 1 | +0.4% | -12.6% | +2.2% | 22 / 22 |
| cpu | 2 | -0.1% | -8.3% | +3.9% | 22 / 22 |
| fanout | 1 | -0.1% | -12.0% | +5.9% | 42 / 42 |
| fanout | 2 | -7.2% | -12.3% | +5.8% | 42 / 42 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| cpu | 1 | baseline | 245760 | 29.696 | 43.008 |
| cpu | 1 | candidate | 245760 | 29.696 | 43.008 |
| cpu | 2 | baseline | 245760 | 17.408 | 29.696 |
| cpu | 2 | candidate | 245760 | 17.408 | 29.696 |
| fanout | 1 | baseline | 245760 | 38.912 | 57.344 |
| fanout | 1 | candidate | 245760 | 38.912 | 59.392 |
| fanout | 2 | baseline | 245760 | 34.816 | 55.296 |
| fanout | 2 | candidate | 245760 | 32.768 | 53.248 |

Diagnostic GC phases summed across measured repetitions (wall time):
