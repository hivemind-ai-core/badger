| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| balanced | 4 | baseline | 207.469 [196.176, 800.329] | 55.199 | 103.1 |
| balanced | 4 | candidate | 190.717 [183.893, 842.348] | 47.895 | 100.0 |
| balanced | 8 | baseline | 148.919 [131.417, 656.697] | 48.130 | 108.6 |
| balanced | 8 | candidate | 138.775 [126.075, 713.507] | 46.069 | 110.6 |
| fanout | 4 | baseline | 901.188 [741.681, 3610.634] | 162.814 | 120.6 |
| fanout | 4 | candidate | 848.052 [730.183, 3464.972] | 164.778 | 120.8 |
| fanout | 8 | baseline | 999.697 [874.553, 3293.529] | 154.680 | 111.0 |
| fanout | 8 | candidate | 947.880 [848.570, 3601.265] | 139.904 | 115.4 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| balanced | 4 | -8.1% | -13.2% | -3.0% | 22 / 21 |
| balanced | 8 | -6.8% | -4.3% | +1.8% | 21 / 21 |
| fanout | 4 | -5.9% | +1.2% | +0.2% | 42 / 40 |
| fanout | 8 | -5.2% | -9.6% | +3.9% | 41 / 40 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| balanced | 4 | 15 | -7.9% [-16.3%, +12.2%] | 12 / 15 |
| balanced | 8 | 15 | -5.5% [-18.0%, +8.7%] | 14 / 15 |
| fanout | 4 | 15 | -1.9% [-15.1%, +15.3%] | 11 / 15 |
| fanout | 8 | 15 | -3.2% [-15.0%, +123.1%] | 10 / 15 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| balanced | 4 | baseline | 245760 | 7.168 | 38.912 |
| balanced | 4 | candidate | 245760 | 6.912 | 36.864 |
| balanced | 8 | baseline | 245760 | 5.120 | 34.816 |
| balanced | 8 | candidate | 245760 | 4.864 | 36.864 |
| fanout | 4 | baseline | 245760 | 30.720 | 147.456 |
| fanout | 4 | candidate | 245760 | 29.696 | 139.264 |
| fanout | 8 | baseline | 245760 | 34.816 | 139.264 |
| fanout | 8 | candidate | 245760 | 32.768 | 155.648 |

Diagnostic GC phases summed across measured repetitions (wall time):
