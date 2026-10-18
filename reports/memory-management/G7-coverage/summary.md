| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| buffers-grow | 1 | baseline | 28.260 [27.479, 29.940] | 1.240 | 40.8 |
| buffers-grow | 1 | candidate | 28.647 [27.460, 29.807] | 1.327 | 41.6 |
| buffers-grow | 2 | baseline | 28.139 [27.790, 29.915] | 1.276 | 41.0 |
| buffers-grow | 2 | candidate | 28.491 [27.214, 29.668] | 1.314 | 42.1 |
| buffers-grow | 4 | baseline | 28.186 [27.490, 30.221] | 1.311 | 40.9 |
| buffers-grow | 4 | candidate | 28.670 [27.597, 29.645] | 1.311 | 41.9 |
| buffers-grow | 8 | baseline | 28.965 [27.689, 30.110] | 1.261 | 40.9 |
| buffers-grow | 8 | candidate | 29.455 [28.233, 30.641] | 1.293 | 42.0 |
| bytes-transient | 1 | baseline | 98.632 [91.128, 104.368] | 0.421 | 45.6 |
| bytes-transient | 1 | candidate | 92.393 [88.341, 96.247] | 0.400 | 46.4 |
| bytes-transient | 2 | baseline | 96.725 [88.134, 100.965] | 0.409 | 46.1 |
| bytes-transient | 2 | candidate | 86.481 [84.778, 95.759] | 0.404 | 46.4 |
| bytes-transient | 4 | baseline | 93.494 [90.599, 96.930] | 0.387 | 46.0 |
| bytes-transient | 4 | candidate | 89.599 [83.775, 97.074] | 0.414 | 46.7 |
| bytes-transient | 8 | baseline | 93.246 [88.632, 96.712] | 0.412 | 45.4 |
| bytes-transient | 8 | candidate | 90.013 [85.399, 95.909] | 0.406 | 46.8 |
| parallel-maps | 1 | baseline | 217.184 [209.431, 232.175] | 61.083 | 41.4 |
| parallel-maps | 1 | candidate | 216.441 [209.046, 226.160] | 62.573 | 43.9 |
| parallel-maps | 2 | baseline | 152.806 [145.313, 161.624] | 48.464 | 51.4 |
| parallel-maps | 2 | candidate | 150.317 [147.904, 159.006] | 46.248 | 51.8 |
| parallel-maps | 4 | baseline | 97.255 [93.225, 98.998] | 32.214 | 63.7 |
| parallel-maps | 4 | candidate | 93.426 [89.985, 97.934] | 29.902 | 66.8 |
| parallel-maps | 8 | baseline | 70.086 [60.062, 73.035] | 23.872 | 74.2 |
| parallel-maps | 8 | candidate | 66.039 [63.185, 77.924] | 21.822 | 73.4 |
| parallel-snapshots | 1 | baseline | 300.749 [293.837, 320.080] | 105.005 | 80.8 |
| parallel-snapshots | 1 | candidate | 294.987 [291.877, 312.514] | 101.707 | 81.6 |
| parallel-snapshots | 2 | baseline | 234.719 [228.426, 242.333] | 83.595 | 78.1 |
| parallel-snapshots | 2 | candidate | 227.856 [220.667, 232.017] | 79.666 | 77.9 |
| parallel-snapshots | 4 | baseline | 231.951 [227.371, 237.384] | 90.853 | 99.0 |
| parallel-snapshots | 4 | candidate | 224.537 [223.278, 241.475] | 86.961 | 99.7 |
| parallel-snapshots | 8 | baseline | 416.260 [404.111, 430.164] | 112.490 | 136.1 |
| parallel-snapshots | 8 | candidate | 427.391 [399.809, 433.288] | 119.568 | 137.4 |
| persistent-4096-major | 1 | baseline | 109.778 [101.048, 115.312] | 27.339 | 48.1 |
| persistent-4096-major | 1 | candidate | 102.313 [100.397, 113.841] | 24.049 | 49.2 |
| persistent-4096-major | 2 | baseline | 111.078 [100.276, 120.127] | 29.996 | 48.2 |
| persistent-4096-major | 2 | candidate | 104.354 [98.439, 107.040] | 24.038 | 49.2 |
| persistent-4096-major | 4 | baseline | 107.447 [101.065, 113.422] | 27.090 | 48.2 |
| persistent-4096-major | 4 | candidate | 103.085 [99.150, 115.371] | 23.841 | 49.4 |
| persistent-4096-major | 8 | baseline | 110.320 [107.445, 117.673] | 29.358 | 48.4 |
| persistent-4096-major | 8 | candidate | 103.741 [99.446, 110.069] | 24.604 | 49.2 |
| persistent-64-major | 1 | baseline | 57.954 [53.523, 63.922] | 14.055 | 46.3 |
| persistent-64-major | 1 | candidate | 59.379 [55.652, 63.837] | 14.993 | 47.1 |
| persistent-64-major | 2 | baseline | 58.147 [53.592, 64.485] | 15.030 | 46.1 |
| persistent-64-major | 2 | candidate | 59.480 [55.818, 65.423] | 13.370 | 47.1 |
| persistent-64-major | 4 | baseline | 59.232 [56.879, 68.463] | 14.586 | 46.5 |
| persistent-64-major | 4 | candidate | 57.503 [54.268, 62.138] | 12.665 | 47.1 |
| persistent-64-major | 8 | baseline | 58.370 [55.104, 70.950] | 14.146 | 46.4 |
| persistent-64-major | 8 | candidate | 57.045 [55.442, 61.419] | 13.701 | 47.2 |
| records-window | 1 | baseline | 126.316 [124.288, 132.881] | 21.833 | 50.6 |
| records-window | 1 | candidate | 128.839 [126.341, 131.805] | 21.645 | 51.6 |
| records-window | 2 | baseline | 127.027 [124.661, 128.745] | 22.330 | 50.5 |
| records-window | 2 | candidate | 128.494 [126.186, 130.740] | 21.377 | 51.6 |
| records-window | 4 | baseline | 125.798 [125.021, 130.685] | 22.029 | 51.3 |
| records-window | 4 | candidate | 128.687 [125.444, 129.760] | 22.126 | 51.6 |
| records-window | 8 | baseline | 126.881 [121.968, 128.736] | 21.722 | 51.2 |
| records-window | 8 | candidate | 129.574 [124.623, 134.886] | 22.288 | 52.6 |
| snapshots-major | 1 | baseline | 97.705 [93.568, 100.736] | 25.403 | 48.0 |
| snapshots-major | 1 | candidate | 94.387 [92.735, 98.585] | 23.876 | 49.4 |
| snapshots-major | 2 | baseline | 107.359 [100.040, 109.897] | 24.942 | 48.3 |
| snapshots-major | 2 | candidate | 101.282 [98.069, 105.773] | 23.874 | 49.3 |
| snapshots-major | 4 | baseline | 104.161 [101.980, 108.099] | 24.736 | 48.3 |
| snapshots-major | 4 | candidate | 102.797 [101.542, 105.254] | 24.809 | 49.5 |
| snapshots-major | 8 | baseline | 104.463 [100.496, 109.776] | 25.781 | 48.3 |
| snapshots-major | 8 | candidate | 100.979 [99.661, 103.876] | 24.942 | 49.3 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| buffers-grow | 1 | +1.4% | +7.0% | +2.0% | 1 / 1 |
| buffers-grow | 2 | +1.3% | +3.0% | +2.5% | 1 / 1 |
| buffers-grow | 4 | +1.7% | +0.0% | +2.4% | 1 / 1 |
| buffers-grow | 8 | +1.7% | +2.5% | +2.5% | 1 / 1 |
| bytes-transient | 1 | -6.3% | -5.0% | +1.8% | 1 / 1 |
| bytes-transient | 2 | -10.6% | -1.3% | +0.6% | 1 / 1 |
| bytes-transient | 4 | -4.2% | +7.0% | +1.6% | 1 / 1 |
| bytes-transient | 8 | -3.5% | -1.3% | +3.0% | 1 / 1 |
| parallel-maps | 1 | -0.3% | +2.4% | +6.1% | 63 / 63 |
| parallel-maps | 2 | -1.6% | -4.6% | +0.8% | 63 / 63 |
| parallel-maps | 4 | -3.9% | -7.2% | +4.8% | 62 / 62 |
| parallel-maps | 8 | -5.8% | -8.6% | -1.1% | 62 / 62 |
| parallel-snapshots | 1 | -1.9% | -3.1% | +1.1% | 37 / 37 |
| parallel-snapshots | 2 | -2.9% | -4.7% | -0.2% | 37 / 37 |
| parallel-snapshots | 4 | -3.2% | -4.3% | +0.7% | 36 / 36 |
| parallel-snapshots | 8 | +2.7% | +6.3% | +0.9% | 32 / 33 |
| persistent-4096-major | 1 | -6.8% | -12.0% | +2.3% | 23 / 23 |
| persistent-4096-major | 2 | -6.1% | -19.9% | +2.1% | 23 / 23 |
| persistent-4096-major | 4 | -4.1% | -12.0% | +2.5% | 23 / 23 |
| persistent-4096-major | 8 | -6.0% | -16.2% | +1.6% | 23 / 23 |
| persistent-64-major | 1 | +2.5% | +6.7% | +1.9% | 14 / 14 |
| persistent-64-major | 2 | +2.3% | -11.0% | +2.3% | 14 / 14 |
| persistent-64-major | 4 | -2.9% | -13.2% | +1.3% | 14 / 14 |
| persistent-64-major | 8 | -2.3% | -3.1% | +1.9% | 14 / 14 |
| records-window | 1 | +2.0% | -0.9% | +2.0% | 16 / 16 |
| records-window | 2 | +1.2% | -4.3% | +2.2% | 16 / 16 |
| records-window | 4 | +2.3% | +0.4% | +0.6% | 16 / 16 |
| records-window | 8 | +2.1% | +2.6% | +2.7% | 16 / 16 |
| snapshots-major | 1 | -3.4% | -6.0% | +2.9% | 17 / 17 |
| snapshots-major | 2 | -5.7% | -4.3% | +2.0% | 17 / 17 |
| snapshots-major | 4 | -1.3% | +0.3% | +2.5% | 17 / 17 |
| snapshots-major | 8 | -3.3% | -3.3% | +1.9% | 17 / 17 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| buffers-grow | 1 | 7 | +0.2% [-5.9%, +6.3%] | 3 / 7 |
| buffers-grow | 2 | 7 | -2.2% [-5.0%, +6.3%] | 4 / 7 |
| buffers-grow | 4 | 7 | +0.0% [-8.5%, +7.1%] | 3 / 7 |
| buffers-grow | 8 | 7 | +2.0% [-4.5%, +6.0%] | 2 / 7 |
| bytes-transient | 1 | 7 | -6.2% [-10.6%, -0.3%] | 7 / 7 |
| bytes-transient | 2 | 7 | -5.3% [-14.2%, -1.9%] | 7 / 7 |
| bytes-transient | 4 | 7 | -4.2% [-10.3%, +6.0%] | 6 / 7 |
| bytes-transient | 8 | 7 | -2.4% [-11.0%, +8.2%] | 5 / 7 |
| parallel-maps | 1 | 7 | -0.9% [-3.9%, +6.9%] | 4 / 7 |
| parallel-maps | 2 | 7 | -1.6% [-8.5%, +3.1%] | 4 / 7 |
| parallel-maps | 4 | 7 | -4.2% [-7.0%, +5.1%] | 6 / 7 |
| parallel-maps | 8 | 7 | -1.1% [-8.5%, +6.7%] | 4 / 7 |
| parallel-snapshots | 1 | 7 | -0.6% [-8.8%, +5.2%] | 4 / 7 |
| parallel-snapshots | 2 | 7 | -2.8% [-7.0%, -0.2%] | 7 / 7 |
| parallel-snapshots | 4 | 7 | -3.2% [-4.2%, +4.2%] | 6 / 7 |
| parallel-snapshots | 8 | 7 | +0.9% [-4.7%, +4.3%] | 3 / 7 |
| persistent-4096-major | 1 | 7 | -4.1% [-12.4%, +8.1%] | 6 / 7 |
| persistent-4096-major | 2 | 7 | -6.0% [-12.2%, +2.6%] | 6 / 7 |
| persistent-4096-major | 4 | 7 | -4.7% [-7.4%, +9.0%] | 5 / 7 |
| persistent-4096-major | 8 | 7 | -7.4% [-14.1%, -0.2%] | 7 / 7 |
| persistent-64-major | 1 | 7 | +4.0% [-11.5%, +13.4%] | 3 / 7 |
| persistent-64-major | 2 | 7 | +4.1% [-10.7%, +14.8%] | 3 / 7 |
| persistent-64-major | 4 | 7 | -2.6% [-18.7%, +9.2%] | 5 / 7 |
| persistent-64-major | 8 | 7 | -0.9% [-18.2%, +8.7%] | 5 / 7 |
| records-window | 1 | 7 | +0.7% [-3.0%, +3.3%] | 1 / 7 |
| records-window | 2 | 7 | +1.9% [-2.0%, +3.1%] | 1 / 7 |
| records-window | 4 | 7 | +0.9% [-0.7%, +2.7%] | 2 / 7 |
| records-window | 8 | 7 | +2.1% [-0.5%, +7.9%] | 2 / 7 |
| snapshots-major | 1 | 7 | -2.6% [-6.3%, +0.4%] | 6 / 7 |
| snapshots-major | 2 | 7 | -4.3% [-8.9%, +5.7%] | 6 / 7 |
| snapshots-major | 4 | 7 | -1.3% [-5.5%, +3.0%] | 5 / 7 |
| snapshots-major | 8 | 7 | -0.8% [-9.2%, +1.2%] | 5 / 7 |

Diagnostic GC phases summed across measured repetitions (wall time):
