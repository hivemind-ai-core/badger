| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| lookup-array-int | 1 | baseline | 2.853 [2.781, 4.451] | 0.000 | 29.0 |
| lookup-array-int | 1 | candidate | 2.840 [2.785, 3.741] | 0.000 | 29.2 |
| lookup-array-int | 2 | baseline | 2.840 [2.800, 3.117] | 0.000 | 29.1 |
| lookup-array-int | 2 | candidate | 2.917 [2.834, 4.588] | 0.000 | 29.3 |
| lookup-array-int | 4 | baseline | 2.880 [2.836, 3.127] | 0.000 | 28.6 |
| lookup-array-int | 4 | candidate | 2.865 [2.789, 3.144] | 0.000 | 29.3 |
| lookup-array-int | 8 | baseline | 3.235 [2.847, 4.646] | 0.000 | 28.9 |
| lookup-array-int | 8 | candidate | 3.148 [2.817, 4.117] | 0.000 | 29.2 |
| lookup-list-misses | 1 | baseline | 6.879 [6.753, 7.589] | 0.000 | 28.7 |
| lookup-list-misses | 1 | candidate | 7.086 [6.841, 7.773] | 0.000 | 29.0 |
| lookup-list-misses | 2 | baseline | 6.869 [6.797, 7.500] | 0.000 | 29.1 |
| lookup-list-misses | 2 | candidate | 6.593 [6.510, 8.726] | 0.000 | 29.1 |
| lookup-list-misses | 4 | baseline | 6.896 [6.835, 8.370] | 0.000 | 29.2 |
| lookup-list-misses | 4 | candidate | 6.585 [6.540, 8.695] | 0.000 | 29.0 |
| lookup-list-misses | 8 | baseline | 6.930 [6.798, 7.812] | 0.000 | 30.3 |
| lookup-list-misses | 8 | candidate | 6.617 [6.531, 8.342] | 0.000 | 29.6 |
| lookup-list-pointers | 1 | baseline | 25.070 [23.970, 27.295] | 1.127 | 50.6 |
| lookup-list-pointers | 1 | candidate | 16.804 [16.059, 19.311] | 0.000 | 29.2 |
| lookup-list-pointers | 2 | baseline | 30.364 [28.891, 33.394] | 1.139 | 51.3 |
| lookup-list-pointers | 2 | candidate | 24.453 [23.169, 26.462] | 0.000 | 28.7 |
| lookup-list-pointers | 4 | baseline | 29.742 [29.662, 31.231] | 1.126 | 51.4 |
| lookup-list-pointers | 4 | candidate | 23.622 [23.225, 26.966] | 0.000 | 28.9 |
| lookup-list-pointers | 8 | baseline | 30.837 [29.096, 32.125] | 1.113 | 51.0 |
| lookup-list-pointers | 8 | candidate | 24.126 [23.438, 26.106] | 0.000 | 29.0 |
| lookup-map-int | 1 | baseline | 27.787 [26.726, 29.658] | 1.133 | 51.2 |
| lookup-map-int | 1 | candidate | 17.537 [16.835, 18.488] | 0.000 | 28.9 |
| lookup-map-int | 2 | baseline | 27.828 [26.341, 30.039] | 1.100 | 51.3 |
| lookup-map-int | 2 | candidate | 18.492 [16.814, 19.347] | 0.000 | 28.7 |
| lookup-map-int | 4 | baseline | 26.685 [26.251, 31.896] | 1.085 | 51.3 |
| lookup-map-int | 4 | candidate | 17.277 [17.098, 19.667] | 0.000 | 29.1 |
| lookup-map-int | 8 | baseline | 27.095 [26.467, 29.870] | 1.128 | 51.2 |
| lookup-map-int | 8 | candidate | 17.813 [16.832, 19.027] | 0.000 | 29.1 |
| lookup-map-misses | 1 | baseline | 15.640 [15.105, 16.570] | 0.000 | 28.9 |
| lookup-map-misses | 1 | candidate | 14.098 [13.611, 14.951] | 0.000 | 28.9 |
| lookup-map-misses | 2 | baseline | 15.384 [15.183, 16.438] | 0.000 | 28.9 |
| lookup-map-misses | 2 | candidate | 14.065 [13.362, 14.488] | 0.000 | 29.3 |
| lookup-map-misses | 4 | baseline | 15.509 [15.339, 17.541] | 0.000 | 29.2 |
| lookup-map-misses | 4 | candidate | 14.156 [13.606, 14.806] | 0.000 | 29.0 |
| lookup-map-misses | 8 | baseline | 16.046 [15.261, 16.931] | 0.000 | 29.2 |
| lookup-map-misses | 8 | candidate | 14.361 [13.642, 15.665] | 0.000 | 29.0 |
| lookup-map-pointers | 1 | baseline | 35.627 [33.351, 40.203] | 1.125 | 50.6 |
| lookup-map-pointers | 1 | candidate | 24.833 [23.709, 30.235] | 0.000 | 30.9 |
| lookup-map-pointers | 2 | baseline | 39.387 [38.081, 42.392] | 1.120 | 50.8 |
| lookup-map-pointers | 2 | candidate | 32.814 [31.024, 34.759] | 0.000 | 31.0 |
| lookup-map-pointers | 4 | baseline | 40.303 [38.265, 42.746] | 1.120 | 51.2 |
| lookup-map-pointers | 4 | candidate | 31.641 [30.826, 34.376] | 0.000 | 31.0 |
| lookup-map-pointers | 8 | baseline | 39.787 [38.133, 40.784] | 1.117 | 50.7 |
| lookup-map-pointers | 8 | candidate | 31.664 [30.901, 32.935] | 0.000 | 30.8 |
| lookup-map-stored | 1 | baseline | 37.945 [35.615, 38.708] | 2.361 | 59.5 |
| lookup-map-stored | 1 | candidate | 36.914 [34.484, 40.014] | 2.222 | 58.4 |
| lookup-map-stored | 2 | baseline | 38.080 [34.673, 40.222] | 2.276 | 59.3 |
| lookup-map-stored | 2 | candidate | 37.114 [35.703, 40.872] | 2.231 | 59.2 |
| lookup-map-stored | 4 | baseline | 36.625 [34.577, 39.548] | 2.278 | 59.1 |
| lookup-map-stored | 4 | candidate | 37.774 [35.532, 43.628] | 2.332 | 58.7 |
| lookup-map-stored | 8 | baseline | 36.018 [35.386, 39.156] | 2.235 | 59.3 |
| lookup-map-stored | 8 | candidate | 36.709 [35.655, 39.583] | 2.212 | 59.3 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| lookup-array-int | 1 | -0.5% | n/a | +0.7% | 0 / 0 |
| lookup-array-int | 2 | +2.7% | n/a | +0.9% | 0 / 0 |
| lookup-array-int | 4 | -0.5% | n/a | +2.4% | 0 / 0 |
| lookup-array-int | 8 | -2.7% | n/a | +1.1% | 0 / 0 |
| lookup-list-misses | 1 | +3.0% | n/a | +1.0% | 0 / 0 |
| lookup-list-misses | 2 | -4.0% | n/a | +0.1% | 0 / 0 |
| lookup-list-misses | 4 | -4.5% | n/a | -0.6% | 0 / 0 |
| lookup-list-misses | 8 | -4.5% | n/a | -2.4% | 0 / 0 |
| lookup-list-pointers | 1 | -33.0% | -100.0% | -42.2% | 1 / 0 |
| lookup-list-pointers | 2 | -19.5% | -100.0% | -44.0% | 1 / 0 |
| lookup-list-pointers | 4 | -20.6% | -100.0% | -43.8% | 1 / 0 |
| lookup-list-pointers | 8 | -21.8% | -100.0% | -43.0% | 1 / 0 |
| lookup-map-int | 1 | -36.9% | -100.0% | -43.6% | 1 / 0 |
| lookup-map-int | 2 | -33.5% | -100.0% | -44.0% | 1 / 0 |
| lookup-map-int | 4 | -35.3% | -100.0% | -43.3% | 1 / 0 |
| lookup-map-int | 8 | -34.3% | -100.0% | -43.2% | 1 / 0 |
| lookup-map-misses | 1 | -9.9% | n/a | -0.2% | 0 / 0 |
| lookup-map-misses | 2 | -8.6% | n/a | +1.4% | 0 / 0 |
| lookup-map-misses | 4 | -8.7% | n/a | -0.9% | 0 / 0 |
| lookup-map-misses | 8 | -10.5% | n/a | -0.9% | 0 / 0 |
| lookup-map-pointers | 1 | -30.3% | -100.0% | -38.9% | 1 / 0 |
| lookup-map-pointers | 2 | -16.7% | -100.0% | -38.9% | 1 / 0 |
| lookup-map-pointers | 4 | -21.5% | -100.0% | -39.4% | 1 / 0 |
| lookup-map-pointers | 8 | -20.4% | -100.0% | -39.2% | 1 / 0 |
| lookup-map-stored | 1 | -2.7% | -5.9% | -1.9% | 1 / 1 |
| lookup-map-stored | 2 | -2.5% | -2.0% | -0.0% | 1 / 1 |
| lookup-map-stored | 4 | +3.1% | +2.4% | -0.6% | 1 / 1 |
| lookup-map-stored | 8 | +1.9% | -1.0% | +0.1% | 1 / 1 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| lookup-array-int | 1 | 7 | -0.5% [-30.8%, +34.5%] | 5 / 7 |
| lookup-array-int | 2 | 7 | +1.2% [-0.4%, +62.3%] | 2 / 7 |
| lookup-array-int | 4 | 7 | -0.8% [-3.2%, +1.3%] | 5 / 7 |
| lookup-array-int | 8 | 7 | -4.1% [-31.0%, +44.6%] | 5 / 7 |
| lookup-list-misses | 1 | 7 | +2.4% [-7.9%, +8.7%] | 1 / 7 |
| lookup-list-misses | 2 | 7 | -3.5% [-9.4%, +20.3%] | 5 / 7 |
| lookup-list-misses | 4 | 7 | -4.0% [-5.8%, +3.9%] | 6 / 7 |
| lookup-list-misses | 8 | 7 | -4.2% [-9.7%, +20.4%] | 6 / 7 |
| lookup-list-pointers | 1 | 7 | -32.8% [-37.4%, -25.9%] | 7 / 7 |
| lookup-list-pointers | 2 | 7 | -20.8% [-24.0%, -15.4%] | 7 / 7 |
| lookup-list-pointers | 4 | 7 | -21.0% [-24.4%, -11.2%] | 7 / 7 |
| lookup-list-pointers | 8 | 7 | -20.4% [-25.7%, -10.3%] | 7 / 7 |
| lookup-map-int | 1 | 7 | -38.0% [-40.9%, -31.4%] | 7 / 7 |
| lookup-map-int | 2 | 7 | -35.6% [-39.0%, -28.7%] | 7 / 7 |
| lookup-map-int | 4 | 7 | -35.0% [-38.3%, -31.2%] | 7 / 7 |
| lookup-map-int | 8 | 7 | -36.0% [-38.6%, -29.8%] | 7 / 7 |
| lookup-map-misses | 1 | 7 | -9.9% [-16.1%, -5.4%] | 7 / 7 |
| lookup-map-misses | 2 | 7 | -10.2% [-13.5%, -8.0%] | 7 / 7 |
| lookup-map-misses | 4 | 7 | -10.5% [-17.4%, -7.4%] | 7 / 7 |
| lookup-map-misses | 8 | 7 | -10.6% [-15.2%, -3.2%] | 7 / 7 |
| lookup-map-pointers | 1 | 7 | -30.5% [-32.7%, -24.3%] | 7 / 7 |
| lookup-map-pointers | 2 | 7 | -17.4% [-18.7%, -13.2%] | 7 / 7 |
| lookup-map-pointers | 4 | 7 | -19.6% [-23.6%, -18.4%] | 7 / 7 |
| lookup-map-pointers | 8 | 7 | -19.3% [-23.2%, -18.4%] | 7 / 7 |
| lookup-map-stored | 1 | 7 | +0.8% [-8.0%, +6.0%] | 3 / 7 |
| lookup-map-stored | 2 | 7 | +1.2% [-6.5%, +12.6%] | 3 / 7 |
| lookup-map-stored | 4 | 7 | +1.6% [-4.5%, +19.1%] | 2 / 7 |
| lookup-map-stored | 8 | 7 | +1.8% [-1.0%, +4.3%] | 2 / 7 |

Diagnostic GC phases summed across measured repetitions (wall time):
