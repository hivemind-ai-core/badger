| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| messages-slow | 8 | baseline | 40.262 [35.997, 44.856] | 2.967 | 56.1 |
| messages-slow | 8 | candidate | 42.916 [37.628, 45.504] | 3.072 | 58.1 |
| snapshots | 8 | baseline | 29.887 [27.753, 32.705] | 7.704 | 39.3 |
| snapshots | 8 | candidate | 30.087 [28.377, 33.070] | 6.891 | 39.4 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| messages-slow | 8 | +6.6% | +3.5% | +3.6% | 3 / 3 |
| snapshots | 8 | +0.7% | -10.5% | +0.3% | 4 / 4 |

Diagnostic GC phases summed across measured repetitions (wall time):

