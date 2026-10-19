# I3a-isolated-source

180 measured processes. [Raw data](measurements.json.gz). Negative paired changes are faster; ranges and all samples remain in the raw artifact.

| Case | Workers | Variant | Kernel median ms | Collections | Pause ms | RSS MiB | Managed objects (census) |
|---|---:|---|---:|---:|---:|---:|---:|
| bytes-transient | 1 | baseline | 93.287 | 1 | 0.280 | 41.125 | — |
| bytes-transient | 1 | candidate | 106.683 | 1 | 0.312 | 41.223 | — |
| bytes-transient | 8 | baseline | 90.558 | 1 | 0.302 | 41.332 | — |
| bytes-transient | 8 | candidate | 100.731 | 1 | 0.280 | 41.305 | — |
| stored-escape | 1 | baseline | 4.028 | 0 | 0.000 | 35.277 | — |
| stored-escape | 1 | candidate | 4.001 | 0 | 0.000 | 35.164 | — |
| stored-escape | 8 | baseline | 4.050 | 0 | 0.000 | 35.180 | — |
| stored-escape | 8 | candidate | 4.061 | 0 | 0.000 | 35.191 | — |
| visible | 1 | baseline | 0.511 | 0 | 0.000 | 24.914 | — |
| visible | 1 | candidate | 0.519 | 0 | 0.000 | 24.875 | — |
| visible | 8 | baseline | 0.512 | 0 | 0.000 | 24.949 | — |
| visible | 8 | candidate | 0.511 | 0 | 0.000 | 24.922 | — |
