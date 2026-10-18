| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| buffers-capacity-32 | 1 | baseline | 16.153 [15.603, 16.826] | 0.218 | 48.0 |
| buffers-capacity-32 | 8 | baseline | 15.913 [15.815, 16.273] | 0.191 | 49.4 |
| bytes-transient | 1 | baseline | 94.824 [89.256, 99.927] | 0.288 | 47.2 |
| bytes-transient | 8 | baseline | 87.953 [87.953, 92.019] | 0.307 | 48.6 |
| map-new | 1 | baseline | 9.915 [9.695, 11.118] | 0.618 | 47.8 |
| map-new | 8 | baseline | 9.804 [9.653, 10.146] | 0.573 | 48.2 |
| persistent-64 | 1 | baseline | 3.372 [3.358, 3.416] | 0.000 | 31.9 |
| persistent-64 | 8 | baseline | 3.412 [3.352, 3.441] | 0.000 | 31.9 |
| records-window | 1 | baseline | 120.189 [116.438, 124.681] | 19.230 | 54.3 |
| records-window | 8 | baseline | 118.440 [117.683, 119.202] | 18.186 | 53.9 |
| scalar-snapshots | 1 | baseline | 4.641 [4.482, 4.678] | 0.000 | 37.8 |
| scalar-snapshots | 8 | baseline | 4.668 [4.508, 5.169] | 0.000 | 38.4 |

Diagnostic GC phases summed across measured repetitions (wall time):
