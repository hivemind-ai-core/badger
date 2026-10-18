| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| buffers-capacity-32 | 1 | baseline | 17.159 [16.635, 20.532] | 0.213 | 46.2 |
| buffers-capacity-32 | 1 | candidate | 16.689 [16.223, 17.652] | 0.309 | 47.1 |
| buffers-capacity-32 | 2 | baseline | 17.233 [16.604, 18.062] | 0.253 | 46.8 |
| buffers-capacity-32 | 2 | candidate | 16.421 [16.253, 18.112] | 0.297 | 46.8 |
| buffers-capacity-32 | 4 | baseline | 16.909 [16.742, 17.983] | 0.273 | 46.6 |
| buffers-capacity-32 | 4 | candidate | 16.536 [16.091, 17.912] | 0.235 | 46.8 |
| buffers-capacity-32 | 8 | baseline | 17.099 [16.683, 19.106] | 0.277 | 46.2 |
| buffers-capacity-32 | 8 | candidate | 16.654 [16.170, 18.015] | 0.292 | 47.2 |
| parallel-maps | 1 | baseline | 31.320 [30.832, 35.681] | 0.000 | 34.1 |
| parallel-maps | 1 | candidate | 32.240 [30.755, 33.163] | 0.000 | 35.0 |
| parallel-maps | 2 | baseline | 17.066 [16.737, 19.107] | 0.000 | 34.4 |
| parallel-maps | 2 | candidate | 16.802 [16.096, 18.372] | 0.000 | 35.1 |
| parallel-maps | 4 | baseline | 9.412 [8.994, 11.095] | 0.000 | 36.6 |
| parallel-maps | 4 | candidate | 9.285 [8.904, 9.454] | 0.000 | 36.1 |
| parallel-maps | 8 | baseline | 6.080 [5.203, 6.654] | 0.000 | 36.0 |
| parallel-maps | 8 | candidate | 5.873 [4.868, 7.482] | 0.000 | 36.9 |
| parallel-scalar-snapshots | 1 | baseline | 149.774 [122.287, 155.815] | 29.555 | 100.9 |
| parallel-scalar-snapshots | 1 | candidate | 46.035 [45.355, 48.248] | 4.868 | 82.8 |
| parallel-scalar-snapshots | 2 | baseline | 92.224 [84.273, 95.689] | 22.227 | 93.6 |
| parallel-scalar-snapshots | 2 | candidate | 29.136 [26.235, 29.683] | 4.823 | 82.6 |
| parallel-scalar-snapshots | 4 | baseline | 59.591 [56.446, 67.581] | 19.465 | 103.2 |
| parallel-scalar-snapshots | 4 | candidate | 18.898 [17.902, 21.130] | 4.652 | 85.2 |
| parallel-scalar-snapshots | 8 | baseline | 49.638 [43.790, 53.102] | 22.534 | 120.1 |
| parallel-scalar-snapshots | 8 | candidate | 16.139 [13.698, 18.323] | 5.758 | 86.9 |
| parallel-snapshots | 1 | baseline | 290.221 [251.499, 316.479] | 83.345 | 81.3 |
| parallel-snapshots | 1 | candidate | 289.491 [272.112, 312.034] | 84.033 | 82.6 |
| parallel-snapshots | 2 | baseline | 218.279 [198.150, 234.192] | 69.377 | 78.4 |
| parallel-snapshots | 2 | candidate | 220.888 [200.131, 231.473] | 68.186 | 80.1 |
| parallel-snapshots | 4 | baseline | 238.396 [211.394, 261.086] | 91.520 | 98.5 |
| parallel-snapshots | 4 | candidate | 232.479 [207.590, 242.413] | 89.879 | 98.0 |
| parallel-snapshots | 8 | baseline | 426.168 [396.603, 439.139] | 118.601 | 139.1 |
| parallel-snapshots | 8 | candidate | 433.827 [389.874, 452.638] | 115.223 | 139.4 |
| persistent-64 | 1 | baseline | 3.538 [3.495, 3.579] | 0.000 | 29.8 |
| persistent-64 | 1 | candidate | 3.590 [3.520, 3.634] | 0.000 | 30.9 |
| persistent-64 | 2 | baseline | 3.544 [3.513, 3.575] | 0.000 | 30.1 |
| persistent-64 | 2 | candidate | 3.567 [3.512, 3.582] | 0.000 | 31.1 |
| persistent-64 | 4 | baseline | 3.572 [3.552, 3.630] | 0.000 | 30.1 |
| persistent-64 | 4 | candidate | 3.573 [3.501, 4.502] | 0.000 | 31.0 |
| persistent-64 | 8 | baseline | 3.664 [3.538, 4.186] | 0.000 | 30.5 |
| persistent-64 | 8 | candidate | 3.559 [3.496, 4.412] | 0.000 | 30.9 |
| records-window | 1 | baseline | 129.143 [124.107, 134.284] | 20.884 | 50.9 |
| records-window | 1 | candidate | 126.464 [121.913, 130.753] | 20.824 | 52.1 |
| records-window | 2 | baseline | 129.875 [122.265, 141.067] | 21.155 | 51.3 |
| records-window | 2 | candidate | 127.509 [122.177, 133.935] | 21.807 | 51.9 |
| records-window | 4 | baseline | 126.955 [124.478, 132.081] | 21.954 | 50.9 |
| records-window | 4 | candidate | 127.806 [125.318, 138.522] | 21.950 | 52.1 |
| records-window | 8 | baseline | 126.940 [125.623, 135.376] | 21.059 | 51.1 |
| records-window | 8 | candidate | 127.790 [120.931, 140.217] | 21.183 | 52.8 |
| scalar-snapshots | 1 | baseline | 12.460 [11.167, 13.396] | 1.674 | 50.9 |
| scalar-snapshots | 1 | candidate | 4.930 [4.680, 6.276] | 0.000 | 36.6 |
| scalar-snapshots | 2 | baseline | 12.135 [11.563, 13.562] | 1.690 | 51.0 |
| scalar-snapshots | 2 | candidate | 4.792 [4.678, 5.898] | 0.000 | 37.3 |
| scalar-snapshots | 4 | baseline | 12.147 [11.156, 14.404] | 1.620 | 51.0 |
| scalar-snapshots | 4 | candidate | 4.803 [4.677, 4.966] | 0.000 | 36.7 |
| scalar-snapshots | 8 | baseline | 12.477 [11.475, 16.066] | 1.893 | 51.2 |
| scalar-snapshots | 8 | candidate | 4.729 [4.643, 4.823] | 0.000 | 36.8 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| buffers-capacity-32 | 1 | -2.7% | +45.2% | +1.9% | 1 / 1 |
| buffers-capacity-32 | 2 | -4.7% | +17.2% | +0.1% | 1 / 1 |
| buffers-capacity-32 | 4 | -2.2% | -13.9% | +0.4% | 1 / 1 |
| buffers-capacity-32 | 8 | -2.6% | +5.3% | +2.2% | 1 / 1 |
| parallel-maps | 1 | +2.9% | n/a | +2.7% | 0 / 0 |
| parallel-maps | 2 | -1.6% | n/a | +2.0% | 0 / 0 |
| parallel-maps | 4 | -1.4% | n/a | -1.2% | 0 / 0 |
| parallel-maps | 8 | -3.4% | n/a | +2.6% | 0 / 0 |
| parallel-scalar-snapshots | 1 | -69.3% | -83.5% | -18.0% | 16 / 2 |
| parallel-scalar-snapshots | 2 | -68.4% | -78.3% | -11.8% | 16 / 2 |
| parallel-scalar-snapshots | 4 | -68.3% | -76.1% | -17.5% | 16 / 2 |
| parallel-scalar-snapshots | 8 | -67.5% | -74.4% | -27.7% | 16 / 2 |
| parallel-snapshots | 1 | -0.3% | +0.8% | +1.6% | 37 / 37 |
| parallel-snapshots | 2 | +1.2% | -1.7% | +2.2% | 37 / 37 |
| parallel-snapshots | 4 | -2.5% | -1.8% | -0.5% | 36 / 36 |
| parallel-snapshots | 8 | +1.8% | -2.8% | +0.3% | 33 / 32 |
| persistent-64 | 1 | +1.5% | n/a | +3.7% | 0 / 0 |
| persistent-64 | 2 | +0.7% | n/a | +3.3% | 0 / 0 |
| persistent-64 | 4 | +0.0% | n/a | +3.1% | 0 / 0 |
| persistent-64 | 8 | -2.9% | n/a | +1.4% | 0 / 0 |
| records-window | 1 | -2.1% | -0.3% | +2.4% | 16 / 16 |
| records-window | 2 | -1.8% | +3.1% | +1.3% | 16 / 16 |
| records-window | 4 | +0.7% | -0.0% | +2.2% | 16 / 16 |
| records-window | 8 | +0.7% | +0.6% | +3.3% | 16 / 16 |
| scalar-snapshots | 1 | -60.4% | -100.0% | -28.1% | 2 / 0 |
| scalar-snapshots | 2 | -60.5% | -100.0% | -26.8% | 2 / 0 |
| scalar-snapshots | 4 | -60.5% | -100.0% | -28.2% | 2 / 0 |
| scalar-snapshots | 8 | -62.1% | -100.0% | -28.1% | 2 / 0 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| buffers-capacity-32 | 1 | 7 | -2.3% [-19.6%, +5.7%] | 5 / 7 |
| buffers-capacity-32 | 2 | 7 | -3.9% [-8.2%, +0.3%] | 6 / 7 |
| buffers-capacity-32 | 4 | 7 | -1.9% [-4.8%, -0.4%] | 7 / 7 |
| buffers-capacity-32 | 8 | 7 | -3.4% [-12.8%, +3.0%] | 6 / 7 |
| parallel-maps | 1 | 7 | +2.2% [-7.1%, +4.5%] | 2 / 7 |
| parallel-maps | 2 | 7 | -3.2% [-7.5%, +8.5%] | 5 / 7 |
| parallel-maps | 4 | 7 | -1.0% [-16.5%, +1.1%] | 4 / 7 |
| parallel-maps | 8 | 7 | -6.4% [-19.5%, +36.0%] | 5 / 7 |
| parallel-scalar-snapshots | 1 | 7 | -68.6% [-70.6%, -60.5%] | 7 / 7 |
| parallel-scalar-snapshots | 2 | 7 | -68.3% [-70.0%, -65.4%] | 7 / 7 |
| parallel-scalar-snapshots | 4 | 7 | -68.3% [-73.5%, -62.6%] | 7 / 7 |
| parallel-scalar-snapshots | 8 | 7 | -67.1% [-70.3%, -63.1%] | 7 / 7 |
| parallel-snapshots | 1 | 7 | +1.7% [-8.3%, +11.5%] | 3 / 7 |
| parallel-snapshots | 2 | 7 | +2.4% [-5.7%, +5.0%] | 1 / 7 |
| parallel-snapshots | 4 | 7 | -3.3% [-9.4%, +3.7%] | 6 / 7 |
| parallel-snapshots | 8 | 7 | +1.9% [-2.8%, +3.7%] | 3 / 7 |
| persistent-64 | 1 | 7 | +1.2% [-0.6%, +4.0%] | 2 / 7 |
| persistent-64 | 2 | 7 | -0.0% [-0.7%, +2.0%] | 4 / 7 |
| persistent-64 | 4 | 7 | -0.3% [-2.0%, +25.5%] | 4 / 7 |
| persistent-64 | 8 | 7 | -3.5% [-16.5%, +24.2%] | 4 / 7 |
| records-window | 1 | 7 | -2.0% [-4.4%, +1.1%] | 6 / 7 |
| records-window | 2 | 7 | -0.3% [-9.6%, +3.1%] | 5 / 7 |
| records-window | 4 | 7 | +1.6% [-2.8%, +6.3%] | 3 / 7 |
| records-window | 8 | 7 | -2.0% [-5.5%, +9.3%] | 4 / 7 |
| scalar-snapshots | 1 | 7 | -61.1% [-64.6%, -47.9%] | 7 / 7 |
| scalar-snapshots | 2 | 7 | -60.3% [-62.0%, -49.1%] | 7 / 7 |
| scalar-snapshots | 4 | 7 | -61.4% [-65.6%, -57.9%] | 7 / 7 |
| scalar-snapshots | 8 | 7 | -62.6% [-70.1%, -58.2%] | 7 / 7 |

Diagnostic GC phases summed across measured repetitions (wall time):
