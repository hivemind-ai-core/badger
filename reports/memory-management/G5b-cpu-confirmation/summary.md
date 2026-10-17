| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| cpu | 4 | baseline | 270.983 [256.847, 295.260] | 46.089 | 92.5 |
| cpu | 4 | candidate | 257.289 [239.050, 277.376] | 39.867 | 96.1 |
| cpu | 8 | baseline | 328.250 [279.797, 360.159] | 48.404 | 127.1 |
| cpu | 8 | candidate | 312.665 [287.414, 371.730] | 43.286 | 129.0 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| cpu | 4 | -5.1% | -13.5% | +3.9% | 22 / 22 |
| cpu | 8 | -4.7% | -10.6% | +1.6% | 22 / 22 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| cpu | 4 | 21 | -5.9% [-12.5%, +0.7%] | 20 / 21 |
| cpu | 8 | 21 | -3.4% [-16.0%, +11.7%] | 11 / 21 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| cpu | 4 | baseline | 344064 | 9.216 | 18.432 |
| cpu | 4 | candidate | 344064 | 8.704 | 14.848 |
| cpu | 8 | baseline | 344064 | 10.752 | 20.480 |
| cpu | 8 | candidate | 344064 | 10.752 | 21.504 |

Diagnostic GC phases summed across measured repetitions (wall time):
