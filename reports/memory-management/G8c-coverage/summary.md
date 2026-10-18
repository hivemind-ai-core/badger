| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| buffers-capacity-32 | 1 | baseline | 16.446 [16.126, 16.537] | 0.228 | 47.7 |
| buffers-capacity-32 | 1 | candidate | 16.629 [16.170, 18.201] | 0.235 | 47.1 |
| buffers-capacity-32 | 2 | baseline | 16.282 [16.079, 16.635] | 0.202 | 47.6 |
| buffers-capacity-32 | 2 | candidate | 16.484 [16.181, 19.418] | 0.240 | 47.2 |
| buffers-capacity-32 | 4 | baseline | 16.314 [16.158, 16.584] | 0.206 | 48.7 |
| buffers-capacity-32 | 4 | candidate | 16.381 [16.014, 17.738] | 0.225 | 47.0 |
| buffers-capacity-32 | 8 | baseline | 16.420 [16.085, 18.629] | 0.220 | 47.6 |
| buffers-capacity-32 | 8 | candidate | 16.690 [16.153, 17.555] | 0.240 | 47.1 |
| bytes-transient | 1 | baseline | 99.120 [90.657, 107.108] | 0.298 | 47.7 |
| bytes-transient | 1 | candidate | 112.556 [99.431, 116.675] | 0.325 | 46.5 |
| bytes-transient | 2 | baseline | 97.310 [89.502, 103.705] | 0.318 | 47.8 |
| bytes-transient | 2 | candidate | 107.253 [96.539, 111.672] | 0.309 | 47.0 |
| bytes-transient | 4 | baseline | 89.367 [87.569, 98.913] | 0.270 | 47.5 |
| bytes-transient | 4 | candidate | 106.032 [101.044, 107.827] | 0.314 | 47.4 |
| bytes-transient | 8 | baseline | 91.650 [89.441, 95.509] | 0.324 | 47.8 |
| bytes-transient | 8 | candidate | 107.174 [101.937, 114.006] | 0.320 | 47.4 |
| parallel-maps | 1 | baseline | 31.519 [30.968, 34.264] | 0.000 | 35.6 |
| parallel-maps | 1 | candidate | 30.669 [30.453, 31.623] | 0.000 | 35.2 |
| parallel-maps | 2 | baseline | 16.603 [16.000, 17.907] | 0.000 | 35.8 |
| parallel-maps | 2 | candidate | 16.488 [16.057, 17.006] | 0.000 | 35.6 |
| parallel-maps | 4 | baseline | 9.549 [9.290, 11.881] | 0.000 | 36.3 |
| parallel-maps | 4 | candidate | 9.302 [9.151, 9.467] | 0.000 | 35.9 |
| parallel-maps | 8 | baseline | 5.448 [5.049, 8.487] | 0.000 | 37.0 |
| parallel-maps | 8 | candidate | 5.900 [4.987, 7.178] | 0.000 | 36.5 |
| parallel-scalar-snapshots | 1 | baseline | 42.828 [42.350, 44.912] | 3.691 | 82.8 |
| parallel-scalar-snapshots | 1 | candidate | 43.384 [42.585, 44.423] | 4.093 | 83.0 |
| parallel-scalar-snapshots | 2 | baseline | 26.373 [25.052, 27.324] | 3.959 | 83.0 |
| parallel-scalar-snapshots | 2 | candidate | 26.000 [25.408, 27.412] | 4.009 | 81.5 |
| parallel-scalar-snapshots | 4 | baseline | 17.586 [17.009, 18.614] | 4.500 | 87.4 |
| parallel-scalar-snapshots | 4 | candidate | 17.898 [17.624, 19.636] | 4.517 | 85.7 |
| parallel-scalar-snapshots | 8 | baseline | 14.523 [13.095, 15.542] | 4.838 | 87.5 |
| parallel-scalar-snapshots | 8 | candidate | 15.199 [12.553, 16.921] | 5.007 | 86.9 |
| parallel-snapshots | 1 | baseline | 258.000 [250.120, 262.273] | 68.984 | 82.3 |
| parallel-snapshots | 1 | candidate | 247.582 [244.452, 258.668] | 68.389 | 81.6 |
| parallel-snapshots | 2 | baseline | 201.344 [198.776, 210.866] | 60.310 | 80.7 |
| parallel-snapshots | 2 | candidate | 198.082 [190.833, 201.962] | 61.487 | 79.4 |
| parallel-snapshots | 4 | baseline | 212.010 [201.848, 217.778] | 78.870 | 99.1 |
| parallel-snapshots | 4 | candidate | 206.907 [201.264, 216.441] | 76.663 | 99.7 |
| parallel-snapshots | 8 | baseline | 399.376 [382.935, 416.339] | 109.787 | 136.4 |
| parallel-snapshots | 8 | candidate | 392.299 [376.339, 400.674] | 101.520 | 138.7 |
| persistent-4096-major | 1 | baseline | 17.602 [17.398, 18.881] | 0.000 | 33.5 |
| persistent-4096-major | 1 | candidate | 17.826 [17.264, 19.649] | 0.000 | 33.1 |
| persistent-4096-major | 2 | baseline | 17.628 [17.500, 22.525] | 0.000 | 33.9 |
| persistent-4096-major | 2 | candidate | 17.490 [17.301, 18.774] | 0.000 | 32.9 |
| persistent-4096-major | 4 | baseline | 17.544 [17.512, 18.219] | 0.000 | 33.8 |
| persistent-4096-major | 4 | candidate | 17.484 [17.366, 20.001] | 0.000 | 33.0 |
| persistent-4096-major | 8 | baseline | 18.006 [17.503, 19.512] | 0.000 | 33.8 |
| persistent-4096-major | 8 | candidate | 17.706 [17.407, 18.856] | 0.000 | 33.1 |
| records-window | 1 | baseline | 122.775 [120.110, 125.459] | 18.647 | 52.4 |
| records-window | 1 | candidate | 121.273 [119.022, 125.461] | 19.433 | 52.1 |
| records-window | 2 | baseline | 124.542 [120.588, 132.764] | 19.625 | 52.5 |
| records-window | 2 | candidate | 125.689 [119.887, 174.273] | 20.138 | 52.3 |
| records-window | 4 | baseline | 122.022 [119.486, 125.799] | 18.558 | 52.6 |
| records-window | 4 | candidate | 123.567 [119.181, 131.779] | 19.805 | 52.2 |
| records-window | 8 | baseline | 121.600 [120.346, 144.082] | 18.899 | 52.8 |
| records-window | 8 | candidate | 123.244 [120.235, 126.010] | 19.405 | 53.6 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| buffers-capacity-32 | 1 | +1.1% | +3.0% | -1.2% | 1 / 1 |
| buffers-capacity-32 | 2 | +1.2% | +18.5% | -0.7% | 1 / 1 |
| buffers-capacity-32 | 4 | +0.4% | +9.5% | -3.5% | 1 / 1 |
| buffers-capacity-32 | 8 | +1.6% | +8.8% | -1.0% | 1 / 1 |
| bytes-transient | 1 | +13.6% | +8.9% | -2.4% | 1 / 1 |
| bytes-transient | 2 | +10.2% | -2.7% | -1.7% | 1 / 1 |
| bytes-transient | 4 | +18.6% | +16.2% | -0.2% | 1 / 1 |
| bytes-transient | 8 | +16.9% | -1.4% | -0.9% | 1 / 1 |
| parallel-maps | 1 | -2.7% | n/a | -1.1% | 0 / 0 |
| parallel-maps | 2 | -0.7% | n/a | -0.5% | 0 / 0 |
| parallel-maps | 4 | -2.6% | n/a | -1.1% | 0 / 0 |
| parallel-maps | 8 | +8.3% | n/a | -1.1% | 0 / 0 |
| parallel-scalar-snapshots | 1 | +1.3% | +10.9% | +0.2% | 2 / 2 |
| parallel-scalar-snapshots | 2 | -1.4% | +1.3% | -1.9% | 2 / 2 |
| parallel-scalar-snapshots | 4 | +1.8% | +0.4% | -2.0% | 2 / 2 |
| parallel-scalar-snapshots | 8 | +4.7% | +3.5% | -0.7% | 2 / 2 |
| parallel-snapshots | 1 | -4.0% | -0.9% | -0.8% | 37 / 37 |
| parallel-snapshots | 2 | -1.6% | +2.0% | -1.7% | 37 / 37 |
| parallel-snapshots | 4 | -2.4% | -2.8% | +0.6% | 36 / 36 |
| parallel-snapshots | 8 | -1.8% | -7.5% | +1.7% | 33 / 32 |
| persistent-4096-major | 1 | +1.3% | n/a | -1.2% | 0 / 0 |
| persistent-4096-major | 2 | -0.8% | n/a | -2.9% | 0 / 0 |
| persistent-4096-major | 4 | -0.3% | n/a | -2.3% | 0 / 0 |
| persistent-4096-major | 8 | -1.7% | n/a | -1.9% | 0 / 0 |
| records-window | 1 | -1.2% | +4.2% | -0.6% | 16 / 16 |
| records-window | 2 | +0.9% | +2.6% | -0.4% | 16 / 16 |
| records-window | 4 | +1.3% | +6.7% | -0.7% | 16 / 16 |
| records-window | 8 | +1.4% | +2.7% | +1.5% | 16 / 16 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| buffers-capacity-32 | 1 | 7 | +1.0% [-0.1%, +12.6%] | 1 / 7 |
| buffers-capacity-32 | 2 | 7 | +1.3% [-2.7%, +19.7%] | 1 / 7 |
| buffers-capacity-32 | 4 | 7 | +1.2% [-3.1%, +7.7%] | 2 / 7 |
| buffers-capacity-32 | 8 | 7 | +0.3% [-7.6%, +6.3%] | 3 / 7 |
| bytes-transient | 1 | 7 | +9.2% [+4.7%, +24.6%] | 0 / 7 |
| bytes-transient | 2 | 7 | +8.2% [-0.8%, +20.1%] | 1 / 7 |
| bytes-transient | 4 | 7 | +14.4% [+7.6%, +23.1%] | 0 / 7 |
| bytes-transient | 8 | 7 | +14.4% [+11.2%, +27.1%] | 0 / 7 |
| parallel-maps | 1 | 7 | -1.7% [-10.5%, +0.3%] | 6 / 7 |
| parallel-maps | 2 | 7 | -0.4% [-5.0%, +2.3%] | 4 / 7 |
| parallel-maps | 4 | 7 | -2.8% [-20.9%, -0.4%] | 7 / 7 |
| parallel-maps | 8 | 7 | +8.3% [-15.4%, +33.8%] | 2 / 7 |
| parallel-scalar-snapshots | 1 | 7 | +2.0% [-3.0%, +2.9%] | 2 / 7 |
| parallel-scalar-snapshots | 2 | 7 | -2.2% [-4.8%, +9.4%] | 5 / 7 |
| parallel-scalar-snapshots | 4 | 7 | +1.3% [-3.8%, +13.2%] | 3 / 7 |
| parallel-scalar-snapshots | 8 | 7 | +8.9% [-14.3%, +22.5%] | 3 / 7 |
| parallel-snapshots | 1 | 7 | -3.0% [-5.9%, +0.2%] | 6 / 7 |
| parallel-snapshots | 2 | 7 | -2.4% [-8.3%, +0.6%] | 6 / 7 |
| parallel-snapshots | 4 | 7 | -1.0% [-7.6%, +5.8%] | 4 / 7 |
| parallel-snapshots | 8 | 7 | -2.9% [-6.1%, +4.6%] | 5 / 7 |
| persistent-4096-major | 1 | 7 | +1.7% [-7.2%, +12.2%] | 3 / 7 |
| persistent-4096-major | 2 | 7 | -0.5% [-19.1%, +6.3%] | 5 / 7 |
| persistent-4096-major | 4 | 7 | -0.8% [-4.2%, +14.2%] | 5 / 7 |
| persistent-4096-major | 8 | 7 | -0.6% [-10.1%, +4.7%] | 5 / 7 |
| records-window | 1 | 7 | -0.9% [-4.1%, +2.2%] | 5 / 7 |
| records-window | 2 | 7 | +2.7% [-8.2%, +44.2%] | 3 / 7 |
| records-window | 4 | 7 | +0.5% [-3.4%, +8.7%] | 2 / 7 |
| records-window | 8 | 7 | -0.9% [-12.5%, +2.5%] | 4 / 7 |

Diagnostic GC phases summed across measured repetitions (wall time):
