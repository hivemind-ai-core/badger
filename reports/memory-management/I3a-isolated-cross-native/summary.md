# I3a-isolated-cross-native

180 measured processes. [Raw data](measurements.json.gz). Negative paired changes are faster; ranges and all samples remain in the raw artifact.

| Case | Workers | Variant | Kernel median ms | Collections | Pause ms | RSS MiB | Managed objects (census) |
|---|---:|---|---:|---:|---:|---:|---:|
| bytes-transient | 1 | baseline | 91.332 | 1 | 0.300 | 31.590 | — |
| bytes-transient | 1 | candidate | 94.670 | 1 | 0.306 | 31.629 | — |
| bytes-transient | 8 | baseline | 90.129 | 1 | 0.286 | 31.711 | — |
| bytes-transient | 8 | candidate | 92.122 | 1 | 0.289 | 31.641 | — |
| stored-escape | 1 | baseline | 4.079 | 0 | 0.000 | 24.027 | — |
| stored-escape | 1 | candidate | 4.164 | 0 | 0.000 | 23.941 | — |
| stored-escape | 8 | baseline | 4.090 | 0 | 0.000 | 24.172 | — |
| stored-escape | 8 | candidate | 4.378 | 0 | 0.000 | 24.133 | — |
| visible | 1 | baseline | 0.509 | 0 | 0.000 | 13.383 | — |
| visible | 1 | candidate | 0.513 | 0 | 0.000 | 13.516 | — |
| visible | 8 | baseline | 0.511 | 0 | 0.000 | 13.559 | — |
| visible | 8 | candidate | 0.508 | 0 | 0.000 | 13.578 | — |
