# I3a-native-mir

180 measured processes. [Raw data](measurements.json.gz). Negative paired changes are faster; ranges and all samples remain in the raw artifact.

| Case | Workers | Variant | Kernel median ms | Collections | Pause ms | RSS MiB | Managed objects (census) |
|---|---:|---|---:|---:|---:|---:|---:|
| collected | 1 | baseline | 18.804 | 2 | 2.135 | 47.086 | — |
| collected | 1 | candidate | 18.783 | 2 | 2.112 | 46.676 | — |
| collected | 8 | baseline | 18.809 | 2 | 2.092 | 47.461 | — |
| collected | 8 | candidate | 18.715 | 2 | 2.124 | 47.035 | — |
| eager | 1 | baseline | 12.102 | 1 | 0.712 | 42.031 | — |
| eager | 1 | candidate | 11.774 | 1 | 0.681 | 41.871 | — |
| eager | 8 | baseline | 11.659 | 1 | 0.656 | 42.504 | — |
| eager | 8 | candidate | 12.084 | 1 | 0.760 | 42.309 | — |
| stored | 1 | baseline | 17.637 | 2 | 2.200 | 46.980 | — |
| stored | 1 | candidate | 0.561 | 0 | 0.000 | 27.793 | — |
| stored | 8 | baseline | 17.529 | 2 | 2.235 | 47.418 | — |
| stored | 8 | candidate | 0.632 | 0 | 0.000 | 28.770 | — |
| stored-collected | 1 | baseline | 5.168 | 0 | 0.000 | 39.340 | — |
| stored-collected | 1 | candidate | 4.722 | 0 | 0.000 | 39.043 | — |
| stored-collected | 8 | baseline | 4.837 | 0 | 0.000 | 39.977 | — |
| stored-collected | 8 | candidate | 4.786 | 0 | 0.000 | 39.516 | — |
| stored-escape | 1 | baseline | 4.212 | 0 | 0.000 | 37.012 | — |
| stored-escape | 1 | candidate | 4.172 | 0 | 0.000 | 36.883 | — |
| stored-escape | 8 | baseline | 4.198 | 0 | 0.000 | 37.438 | — |
| stored-escape | 8 | candidate | 4.197 | 0 | 0.000 | 37.309 | — |
| stored-list | 1 | baseline | 6.454 | 0 | 0.000 | 37.125 | — |
| stored-list | 1 | candidate | 1.621 | 0 | 0.000 | 27.871 | — |
| stored-list | 8 | baseline | 6.555 | 0 | 0.000 | 37.418 | — |
| stored-list | 8 | candidate | 1.621 | 0 | 0.000 | 28.812 | — |
| stored-pointers | 1 | baseline | 8.696 | 0 | 0.000 | 41.324 | — |
| stored-pointers | 1 | candidate | 3.765 | 0 | 0.000 | 30.707 | — |
| stored-pointers | 8 | baseline | 10.445 | 0 | 0.000 | 41.605 | — |
| stored-pointers | 8 | candidate | 5.713 | 0 | 0.000 | 31.320 | — |
| stored-reused | 1 | baseline | 9.738 | 1 | 1.090 | 47.027 | — |
| stored-reused | 1 | candidate | 0.352 | 0 | 0.000 | 27.938 | — |
| stored-reused | 8 | baseline | 9.702 | 1 | 1.052 | 47.160 | — |
| stored-reused | 8 | candidate | 0.328 | 0 | 0.000 | 28.695 | — |
| visible | 1 | baseline | 0.744 | 0 | 0.000 | 28.582 | — |
| visible | 1 | candidate | 0.567 | 0 | 0.000 | 27.805 | — |
| visible | 8 | baseline | 0.640 | 0 | 0.000 | 28.957 | — |
| visible | 8 | candidate | 0.745 | 0 | 0.000 | 28.773 | — |
