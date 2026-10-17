| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| lookup-array-int | 1 | baseline | 2.805 [2.791, 3.833] | 0.000 | 28.9 |
| lookup-array-int | 2 | baseline | 2.951 [2.848, 3.925] | 0.000 | 28.8 |
| lookup-array-int | 4 | baseline | 2.907 [2.827, 2.923] | 0.000 | 28.9 |
| lookup-array-int | 8 | baseline | 2.830 [2.751, 2.832] | 0.000 | 29.1 |
| lookup-list-pointers | 1 | baseline | 25.505 [24.514, 25.693] | 1.129 | 51.0 |
| lookup-list-pointers | 2 | baseline | 30.913 [30.177, 33.159] | 1.100 | 50.9 |
| lookup-list-pointers | 4 | baseline | 30.321 [29.683, 30.711] | 1.122 | 51.2 |
| lookup-list-pointers | 8 | baseline | 31.740 [30.230, 31.741] | 1.137 | 50.6 |
| lookup-map-int | 1 | baseline | 27.304 [27.064, 28.652] | 1.128 | 51.3 |
| lookup-map-int | 2 | baseline | 27.817 [27.133, 29.254] | 1.101 | 51.2 |
| lookup-map-int | 4 | baseline | 27.166 [27.131, 27.197] | 1.150 | 51.2 |
| lookup-map-int | 8 | baseline | 27.628 [27.609, 28.572] | 1.128 | 50.9 |
| lookup-map-pointers | 1 | baseline | 33.608 [33.602, 34.295] | 1.107 | 51.4 |
| lookup-map-pointers | 2 | baseline | 39.079 [38.715, 40.656] | 1.143 | 51.1 |
| lookup-map-pointers | 4 | baseline | 39.439 [39.370, 40.714] | 1.121 | 50.4 |
| lookup-map-pointers | 8 | baseline | 40.890 [39.134, 42.254] | 1.119 | 52.2 |
| lookup-map-stored | 1 | baseline | 37.067 [35.666, 40.124] | 2.285 | 58.7 |
| lookup-map-stored | 2 | baseline | 37.066 [36.445, 38.901] | 2.286 | 58.3 |
| lookup-map-stored | 4 | baseline | 37.832 [36.400, 39.379] | 2.239 | 58.9 |
| lookup-map-stored | 8 | baseline | 36.739 [36.327, 37.777] | 2.247 | 58.9 |

Diagnostic GC phases summed across measured repetitions (wall time):
