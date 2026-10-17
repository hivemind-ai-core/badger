| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| messages-slow | 8 | baseline | 41.109 [36.970, 49.186] | 2.991 | 55.8 |
| messages-slow | 8 | candidate | 41.429 [36.465, 47.850] | 3.145 | 56.1 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| messages-slow | 8 | +0.8% | +5.2% | +0.6% | 3 / 3 |

Diagnostic GC phases summed across measured repetitions (wall time):

