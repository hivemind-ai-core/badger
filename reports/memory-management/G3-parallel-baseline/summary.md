| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| parallel-maps | 1 | baseline | 270.264 [234.785, 308.103] | 102.220 | 39.9 |
| parallel-maps | 2 | baseline | 190.826 [150.784, 191.528] | 71.600 | 48.5 |
| parallel-maps | 4 | baseline | 126.042 [102.896, 127.144] | 47.694 | 59.7 |
| parallel-maps | 8 | baseline | 87.357 [80.734, 95.039] | 33.863 | 69.1 |
| parallel-snapshots | 1 | baseline | 375.040 [361.126, 411.976] | 127.282 | 76.5 |
| parallel-snapshots | 2 | baseline | 293.959 [260.125, 298.909] | 121.048 | 76.1 |
| parallel-snapshots | 4 | baseline | 281.465 [263.089, 300.200] | 123.318 | 96.4 |
| parallel-snapshots | 8 | baseline | 463.015 [441.426, 470.218] | 145.011 | 134.5 |

Diagnostic GC phases summed across measured repetitions (wall time):
