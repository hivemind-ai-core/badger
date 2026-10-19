# I3a-native-census

108 measured processes. [Raw data](measurements.json.gz). Negative paired changes are faster; ranges and all samples remain in the raw artifact.

| Case | Workers | Variant | Kernel median ms | Collections | Pause ms | RSS MiB | Managed objects (census) |
|---|---:|---|---:|---:|---:|---:|---:|
| collected | 1 | baseline | 45.145 | 2 | 6.164 | 55.184 | 1,223,948 |
| collected | 1 | candidate | 45.748 | 2 | 6.351 | 54.766 | 1,223,948 |
| collected | 8 | baseline | 44.858 | 2 | 6.140 | 55.574 | 1,223,948 |
| collected | 8 | candidate | 45.242 | 2 | 6.180 | 57.043 | 1,223,948 |
| eager | 1 | baseline | 23.710 | 1 | 2.147 | 48.789 | 527,100 |
| eager | 1 | candidate | 24.201 | 1 | 2.065 | 49.559 | 527,100 |
| eager | 8 | baseline | 23.345 | 1 | 2.091 | 49.004 | 527,100 |
| eager | 8 | candidate | 23.536 | 1 | 2.052 | 48.754 | 527,100 |
| stored | 1 | baseline | 43.723 | 2 | 6.577 | 55.422 | 1,048,716 |
| stored | 1 | candidate | 0.562 | 0 | 0.000 | 28.051 | 60 |
| stored | 8 | baseline | 43.213 | 2 | 6.779 | 55.941 | 1,048,716 |
| stored | 8 | candidate | 0.637 | 0 | 0.000 | 28.789 | 60 |
| stored-collected | 1 | baseline | 11.254 | 0 | 0.000 | 45.656 | 306,080 |
| stored-collected | 1 | candidate | 11.857 | 0 | 0.000 | 45.582 | 306,080 |
| stored-collected | 8 | baseline | 11.416 | 0 | 0.000 | 46.215 | 306,080 |
| stored-collected | 8 | candidate | 11.386 | 0 | 0.000 | 46.305 | 306,080 |
| stored-escape | 1 | baseline | 10.189 | 0 | 0.000 | 41.410 | 262,244 |
| stored-escape | 1 | candidate | 10.503 | 0 | 0.000 | 41.184 | 262,244 |
| stored-escape | 8 | baseline | 10.307 | 0 | 0.000 | 43.723 | 262,244 |
| stored-escape | 8 | candidate | 10.309 | 0 | 0.000 | 41.582 | 262,244 |
| stored-list | 1 | baseline | 12.430 | 0 | 0.000 | 41.621 | 262,321 |
| stored-list | 1 | candidate | 1.627 | 0 | 0.000 | 28.164 | 137 |
| stored-list | 8 | baseline | 12.792 | 0 | 0.000 | 42.258 | 262,321 |
| stored-list | 8 | candidate | 1.607 | 0 | 0.000 | 29.066 | 137 |
| stored-pointers | 1 | baseline | 17.215 | 0 | 0.000 | 48.023 | 295,089 |
| stored-pointers | 1 | candidate | 4.360 | 0 | 0.000 | 31.812 | 32,905 |
| stored-pointers | 8 | baseline | 16.888 | 0 | 0.000 | 48.754 | 295,089 |
| stored-pointers | 8 | candidate | 6.383 | 0 | 0.000 | 32.551 | 32,905 |
| stored-reused | 1 | baseline | 23.867 | 1 | 3.343 | 55.273 | 524,404 |
| stored-reused | 1 | candidate | 0.349 | 0 | 0.000 | 28.168 | 60 |
| stored-reused | 8 | baseline | 22.742 | 1 | 3.301 | 55.957 | 524,404 |
| stored-reused | 8 | candidate | 0.327 | 0 | 0.000 | 29.047 | 60 |
| visible | 1 | baseline | 0.751 | 0 | 0.000 | 29.051 | 60 |
| visible | 1 | candidate | 0.560 | 0 | 0.000 | 28.031 | 60 |
| visible | 8 | baseline | 0.673 | 0 | 0.000 | 29.164 | 60 |
| visible | 8 | candidate | 0.755 | 0 | 0.000 | 29.031 | 60 |
