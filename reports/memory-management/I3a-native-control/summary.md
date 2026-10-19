# I3a-native-control

80 measured processes. [Raw data](measurements.json.gz). Negative paired changes are faster; ranges and all samples remain in the raw artifact.

| Case | Workers | Variant | Kernel median ms | Collections | Pause ms | RSS MiB | Managed objects (census) |
|---|---:|---|---:|---:|---:|---:|---:|
| bytes-transient | 1 | baseline | 106.336 | 1 | 0.298 | 42.805 | — |
| bytes-transient | 1 | candidate | 101.342 | 1 | 0.291 | 42.938 | — |
| bytes-transient | 8 | baseline | 103.334 | 1 | 0.313 | 43.336 | — |
| bytes-transient | 8 | candidate | 103.408 | 1 | 0.304 | 43.227 | — |
| stored | 1 | baseline | 0.548 | 0 | 0.000 | 27.906 | — |
| stored | 1 | candidate | 0.539 | 0 | 0.000 | 27.750 | — |
| stored | 8 | baseline | 0.640 | 0 | 0.000 | 28.934 | — |
| stored | 8 | candidate | 0.633 | 0 | 0.000 | 28.699 | — |
| stored-pointers | 1 | baseline | 3.830 | 0 | 0.000 | 30.828 | — |
| stored-pointers | 1 | candidate | 3.739 | 0 | 0.000 | 30.848 | — |
| stored-pointers | 8 | baseline | 5.777 | 0 | 0.000 | 31.035 | — |
| stored-pointers | 8 | candidate | 5.697 | 0 | 0.000 | 31.121 | — |
| visible | 1 | baseline | 0.566 | 0 | 0.000 | 28.000 | — |
| visible | 1 | candidate | 0.563 | 0 | 0.000 | 27.945 | — |
| visible | 8 | baseline | 0.738 | 0 | 0.000 | 28.910 | — |
| visible | 8 | candidate | 0.768 | 0 | 0.000 | 28.816 | — |
