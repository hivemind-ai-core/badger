| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| cpu | 2 | baseline | 542.609 [524.625, 587.395] | 80.161 | 88.5 |
| cpu | 2 | candidate | 549.075 [516.831, 1937.293] | 77.050 | 89.9 |
| cpu | 8 | baseline | 350.587 [307.580, 1090.548] | 69.557 | 128.0 |
| cpu | 8 | candidate | 365.034 [305.169, 461.891] | 67.738 | 126.3 |
| fanout | 2 | baseline | 1063.123 [925.288, 3828.889] | 201.306 | 104.5 |
| fanout | 2 | candidate | 1049.989 [919.390, 2987.418] | 198.842 | 105.0 |
| fanout | 8 | baseline | 881.062 [790.311, 1602.838] | 159.025 | 103.9 |
| fanout | 8 | candidate | 850.111 [787.168, 3326.423] | 148.669 | 104.5 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| cpu | 2 | +1.2% | -3.9% | +1.6% | 22 / 22 |
| cpu | 8 | +4.1% | -2.6% | -1.3% | 22 / 22 |
| fanout | 2 | -1.2% | -1.2% | +0.5% | 42 / 42 |
| fanout | 8 | -3.5% | -6.5% | +0.6% | 41 / 41 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| cpu | 2 | baseline | 245760 | 18.432 | 31.744 |
| cpu | 2 | candidate | 245760 | 18.432 | 77.824 |
| cpu | 8 | baseline | 245760 | 11.776 | 47.104 |
| cpu | 8 | candidate | 245760 | 11.776 | 27.648 |
| fanout | 2 | baseline | 245760 | 36.864 | 155.648 |
| fanout | 2 | candidate | 245760 | 36.864 | 147.456 |
| fanout | 8 | baseline | 245760 | 28.672 | 102.400 |
| fanout | 8 | candidate | 245760 | 28.672 | 139.264 |

Diagnostic GC phases summed across measured repetitions (wall time):
