| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| messages-int-sustained | 4 | baseline | 105.517 [99.672, 127.033] | 8.488 | 45.3 |
| messages-int-sustained | 4 | candidate | 103.149 [99.897, 119.847] | 8.007 | 45.4 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| messages-int-sustained | 4 | -2.2% | -5.7% | +0.3% | 5 / 5 |

Diagnostic GC phases summed across measured repetitions (wall time):
