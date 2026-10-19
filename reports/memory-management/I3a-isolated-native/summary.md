# I3a-isolated-native

180 measured processes. [Raw data](measurements.json.gz). Negative paired changes are faster; ranges and all samples remain in the raw artifact.

| Case | Workers | Variant | Kernel median ms | Collections | Pause ms | RSS MiB | Managed objects (census) |
|---|---:|---|---:|---:|---:|---:|---:|
| bytes-transient | 1 | baseline | 103.023 | 1 | 0.302 | 31.676 | — |
| bytes-transient | 1 | candidate | 100.078 | 1 | 0.286 | 31.664 | — |
| bytes-transient | 8 | baseline | 101.620 | 1 | 0.275 | 31.574 | — |
| bytes-transient | 8 | candidate | 100.324 | 1 | 0.279 | 31.715 | — |
| stored-escape | 1 | baseline | 4.025 | 0 | 0.000 | 24.121 | — |
| stored-escape | 1 | candidate | 4.005 | 0 | 0.000 | 24.191 | — |
| stored-escape | 8 | baseline | 4.079 | 0 | 0.000 | 24.273 | — |
| stored-escape | 8 | candidate | 4.080 | 0 | 0.000 | 24.305 | — |
| visible | 1 | baseline | 0.510 | 0 | 0.000 | 13.465 | — |
| visible | 1 | candidate | 0.517 | 0 | 0.000 | 13.441 | — |
| visible | 8 | baseline | 0.512 | 0 | 0.000 | 13.578 | — |
| visible | 8 | candidate | 0.522 | 0 | 0.000 | 13.602 | — |
