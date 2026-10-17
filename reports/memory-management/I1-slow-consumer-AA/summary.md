| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| messages-slow | 8 | baseline | 40.654 [35.704, 51.165] | 3.104 | 56.1 |
| messages-slow | 8 | candidate | 40.647 [34.273, 51.114] | 2.977 | 57.1 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| messages-slow | 8 | -0.0% | -4.1% | +1.8% | 3 / 3 |

Diagnostic GC phases summed across measured repetitions (wall time):

