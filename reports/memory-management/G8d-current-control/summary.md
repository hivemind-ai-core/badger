| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| buffers-capacity-pointers | 1 | baseline | 15.422 [15.301, 18.654] | 0.714 | 44.7 |
| buffers-capacity-pointers | 1 | candidate | 15.295 [15.048, 19.339] | 0.710 | 44.8 |
| buffers-capacity-pointers | 8 | baseline | 21.554 [21.214, 22.130] | 0.696 | 45.2 |
| buffers-capacity-pointers | 8 | candidate | 21.101 [20.742, 23.065] | 0.695 | 45.2 |
| bytes-transient | 1 | baseline | 85.910 [83.960, 91.847] | 0.286 | 41.0 |
| bytes-transient | 1 | candidate | 91.199 [82.174, 94.785] | 0.258 | 40.9 |
| bytes-transient | 8 | baseline | 86.778 [82.596, 96.239] | 0.252 | 43.4 |
| bytes-transient | 8 | candidate | 90.757 [83.077, 95.858] | 0.248 | 41.5 |
| records-window | 1 | baseline | 122.578 [119.094, 124.995] | 18.064 | 43.9 |
| records-window | 1 | candidate | 122.415 [120.918, 124.257] | 18.320 | 43.9 |
| records-window | 8 | baseline | 119.933 [117.740, 129.299] | 18.262 | 44.5 |
| records-window | 8 | candidate | 121.380 [118.238, 127.192] | 18.997 | 44.4 |
| stored | 1 | baseline | 0.634 [0.620, 0.651] | 0.000 | 26.3 |
| stored | 1 | candidate | 0.643 [0.632, 0.650] | 0.000 | 26.2 |
| stored | 8 | baseline | 0.639 [0.633, 1.533] | 0.000 | 26.7 |
| stored | 8 | candidate | 0.635 [0.624, 1.126] | 0.000 | 26.7 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| buffers-capacity-pointers | 1 | -0.8% | -0.5% | +0.1% | 1 / 1 |
| buffers-capacity-pointers | 8 | -2.1% | -0.1% | +0.1% | 1 / 1 |
| bytes-transient | 1 | +6.2% | -9.8% | -0.1% | 1 / 1 |
| bytes-transient | 8 | +4.6% | -1.4% | -4.4% | 1 / 1 |
| records-window | 1 | -0.1% | +1.4% | +0.2% | 16 / 16 |
| records-window | 8 | +1.2% | +4.0% | -0.1% | 16 / 16 |
| stored | 1 | +1.4% | n/a | -0.4% | 0 / 0 |
| stored | 8 | -0.6% | n/a | -0.1% | 0 / 0 |

Diagnostic GC phases summed across measured repetitions (wall time):
