| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| buffers-capacity-32 | 1 | baseline | 17.372 [16.934, 19.217] | 0.245 | 45.8 |
| buffers-capacity-32 | 1 | candidate | 16.642 [16.352, 17.325] | 0.248 | 46.9 |
| buffers-capacity-32 | 2 | baseline | 17.331 [16.939, 17.645] | 0.229 | 46.1 |
| buffers-capacity-32 | 2 | candidate | 16.875 [16.324, 17.284] | 0.217 | 46.7 |
| buffers-capacity-32 | 4 | baseline | 17.913 [16.793, 20.082] | 0.270 | 46.2 |
| buffers-capacity-32 | 4 | candidate | 16.745 [16.285, 20.735] | 0.231 | 47.3 |
| buffers-capacity-32 | 8 | baseline | 17.696 [17.272, 18.868] | 0.266 | 46.5 |
| buffers-capacity-32 | 8 | candidate | 17.087 [16.603, 17.829] | 0.265 | 46.9 |
| parallel-maps | 1 | baseline | 32.720 [31.277, 32.912] | 0.000 | 33.8 |
| parallel-maps | 1 | candidate | 32.754 [31.967, 39.017] | 0.000 | 35.1 |
| parallel-maps | 2 | baseline | 16.994 [16.768, 18.850] | 0.000 | 34.2 |
| parallel-maps | 2 | candidate | 18.281 [17.436, 18.824] | 0.000 | 35.3 |
| parallel-maps | 4 | baseline | 9.642 [9.455, 10.822] | 0.000 | 35.2 |
| parallel-maps | 4 | candidate | 9.844 [9.176, 10.589] | 0.000 | 36.6 |
| parallel-maps | 8 | baseline | 5.502 [5.266, 8.070] | 0.000 | 35.7 |
| parallel-maps | 8 | candidate | 6.745 [5.726, 8.253] | 0.000 | 36.8 |
| parallel-scalar-snapshots | 1 | baseline | 151.496 [128.907, 156.602] | 29.846 | 100.3 |
| parallel-scalar-snapshots | 1 | candidate | 48.089 [44.640, 49.741] | 5.655 | 82.9 |
| parallel-scalar-snapshots | 2 | baseline | 84.456 [82.001, 96.762] | 21.203 | 94.1 |
| parallel-scalar-snapshots | 2 | candidate | 26.758 [26.328, 29.619] | 4.037 | 83.8 |
| parallel-scalar-snapshots | 4 | baseline | 58.522 [56.399, 64.334] | 18.626 | 101.0 |
| parallel-scalar-snapshots | 4 | candidate | 18.095 [17.658, 19.563] | 4.755 | 83.7 |
| parallel-scalar-snapshots | 8 | baseline | 44.698 [42.344, 49.480] | 19.802 | 116.5 |
| parallel-scalar-snapshots | 8 | candidate | 16.495 [15.497, 18.897] | 5.891 | 87.0 |
| parallel-snapshots | 1 | baseline | 277.079 [264.006, 336.044] | 76.572 | 81.1 |
| parallel-snapshots | 1 | candidate | 280.593 [260.170, 306.917] | 76.865 | 81.6 |
| parallel-snapshots | 2 | baseline | 212.828 [205.381, 231.633] | 67.445 | 76.6 |
| parallel-snapshots | 2 | candidate | 217.491 [212.342, 249.052] | 70.548 | 78.5 |
| parallel-snapshots | 4 | baseline | 216.836 [203.725, 238.758] | 79.259 | 101.1 |
| parallel-snapshots | 4 | candidate | 218.897 [212.279, 236.805] | 79.357 | 101.5 |
| parallel-snapshots | 8 | baseline | 410.372 [387.364, 457.955] | 104.598 | 135.1 |
| parallel-snapshots | 8 | candidate | 410.050 [398.287, 433.871] | 108.050 | 137.3 |
| persistent-64 | 1 | baseline | 3.598 [3.568, 3.732] | 0.000 | 30.1 |
| persistent-64 | 1 | candidate | 3.716 [3.630, 3.774] | 0.000 | 30.9 |
| persistent-64 | 2 | baseline | 3.563 [3.432, 3.616] | 0.000 | 30.1 |
| persistent-64 | 2 | candidate | 3.654 [3.585, 4.326] | 0.000 | 30.5 |
| persistent-64 | 4 | baseline | 3.578 [3.525, 3.979] | 0.000 | 29.6 |
| persistent-64 | 4 | candidate | 3.787 [3.596, 6.336] | 0.000 | 30.7 |
| persistent-64 | 8 | baseline | 3.573 [3.412, 4.739] | 0.000 | 29.8 |
| persistent-64 | 8 | candidate | 3.664 [3.612, 3.776] | 0.000 | 30.8 |
| records-window | 1 | baseline | 130.149 [124.709, 136.103] | 21.887 | 51.4 |
| records-window | 1 | candidate | 128.062 [125.462, 138.856] | 20.283 | 51.9 |
| records-window | 2 | baseline | 132.207 [125.226, 134.742] | 22.059 | 51.1 |
| records-window | 2 | candidate | 125.656 [123.758, 129.624] | 19.991 | 52.5 |
| records-window | 4 | baseline | 133.071 [125.192, 138.256] | 22.825 | 51.4 |
| records-window | 4 | candidate | 128.475 [126.967, 134.540] | 20.950 | 52.5 |
| records-window | 8 | baseline | 129.157 [123.331, 138.279] | 20.852 | 52.5 |
| records-window | 8 | candidate | 125.492 [121.722, 145.131] | 20.490 | 53.0 |
| scalar-snapshots | 1 | baseline | 13.006 [10.876, 16.066] | 1.859 | 50.9 |
| scalar-snapshots | 1 | candidate | 4.911 [4.674, 5.595] | 0.000 | 36.7 |
| scalar-snapshots | 2 | baseline | 12.054 [11.465, 14.367] | 1.532 | 51.2 |
| scalar-snapshots | 2 | candidate | 4.778 [4.666, 4.997] | 0.000 | 36.7 |
| scalar-snapshots | 4 | baseline | 13.691 [10.990, 14.803] | 1.884 | 51.1 |
| scalar-snapshots | 4 | candidate | 4.714 [4.663, 6.171] | 0.000 | 36.9 |
| scalar-snapshots | 8 | baseline | 12.630 [11.607, 16.125] | 1.823 | 50.9 |
| scalar-snapshots | 8 | candidate | 4.822 [4.675, 5.002] | 0.000 | 36.8 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| buffers-capacity-32 | 1 | -4.2% | +1.3% | +2.4% | 1 / 1 |
| buffers-capacity-32 | 2 | -2.6% | -5.2% | +1.3% | 1 / 1 |
| buffers-capacity-32 | 4 | -6.5% | -14.5% | +2.5% | 1 / 1 |
| buffers-capacity-32 | 8 | -3.4% | -0.5% | +0.9% | 1 / 1 |
| parallel-maps | 1 | +0.1% | n/a | +3.8% | 0 / 0 |
| parallel-maps | 2 | +7.6% | n/a | +3.0% | 0 / 0 |
| parallel-maps | 4 | +2.1% | n/a | +4.1% | 0 / 0 |
| parallel-maps | 8 | +22.6% | n/a | +3.0% | 0 / 0 |
| parallel-scalar-snapshots | 1 | -68.3% | -81.1% | -17.3% | 16 / 2 |
| parallel-scalar-snapshots | 2 | -68.3% | -81.0% | -10.9% | 16 / 2 |
| parallel-scalar-snapshots | 4 | -69.1% | -74.5% | -17.1% | 16 / 2 |
| parallel-scalar-snapshots | 8 | -63.1% | -70.2% | -25.3% | 16 / 2 |
| parallel-snapshots | 1 | +1.3% | +0.4% | +0.6% | 37 / 37 |
| parallel-snapshots | 2 | +2.2% | +4.6% | +2.6% | 37 / 37 |
| parallel-snapshots | 4 | +1.0% | +0.1% | +0.3% | 36 / 36 |
| parallel-snapshots | 8 | -0.1% | +3.3% | +1.6% | 33 / 32 |
| persistent-64 | 1 | +3.3% | n/a | +2.7% | 0 / 0 |
| persistent-64 | 2 | +2.6% | n/a | +1.4% | 0 / 0 |
| persistent-64 | 4 | +5.8% | n/a | +4.0% | 0 / 0 |
| persistent-64 | 8 | +2.5% | n/a | +3.1% | 0 / 0 |
| records-window | 1 | -1.6% | -7.3% | +1.0% | 16 / 16 |
| records-window | 2 | -5.0% | -9.4% | +2.8% | 16 / 16 |
| records-window | 4 | -3.5% | -8.2% | +2.0% | 16 / 16 |
| records-window | 8 | -2.8% | -1.7% | +1.0% | 16 / 16 |
| scalar-snapshots | 1 | -62.2% | -100.0% | -27.9% | 2 / 0 |
| scalar-snapshots | 2 | -60.4% | -100.0% | -28.2% | 2 / 0 |
| scalar-snapshots | 4 | -65.6% | -100.0% | -27.8% | 2 / 0 |
| scalar-snapshots | 8 | -61.8% | -100.0% | -27.6% | 2 / 0 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| buffers-capacity-32 | 1 | 7 | -3.3% [-10.9%, -0.7%] | 7 / 7 |
| buffers-capacity-32 | 2 | 7 | -2.6% [-6.0%, -0.3%] | 7 / 7 |
| buffers-capacity-32 | 4 | 7 | -4.3% [-9.9%, +14.7%] | 6 / 7 |
| buffers-capacity-32 | 8 | 7 | -3.7% [-10.2%, +1.0%] | 6 / 7 |
| parallel-maps | 1 | 7 | +0.1% [-2.9%, +19.2%] | 3 / 7 |
| parallel-maps | 2 | 7 | +7.6% [-6.6%, +11.1%] | 2 / 7 |
| parallel-maps | 4 | 7 | +0.6% [-8.4%, +11.6%] | 3 / 7 |
| parallel-maps | 8 | 7 | +11.4% [-21.1%, +56.7%] | 1 / 7 |
| parallel-scalar-snapshots | 1 | 7 | -67.3% [-69.5%, -63.9%] | 7 / 7 |
| parallel-scalar-snapshots | 2 | 7 | -67.7% [-72.7%, -64.1%] | 7 / 7 |
| parallel-scalar-snapshots | 4 | 7 | -69.0% [-71.4%, -67.7%] | 7 / 7 |
| parallel-scalar-snapshots | 8 | 7 | -63.4% [-66.4%, -56.3%] | 7 / 7 |
| parallel-snapshots | 1 | 7 | -1.5% [-8.7%, +5.5%] | 5 / 7 |
| parallel-snapshots | 2 | 7 | +3.3% [-3.9%, +9.0%] | 1 / 7 |
| parallel-snapshots | 4 | 7 | -0.8% [-2.0%, +7.4%] | 4 / 7 |
| parallel-snapshots | 8 | 7 | -2.9% [-5.3%, +5.9%] | 4 / 7 |
| persistent-64 | 1 | 7 | +1.7% [-0.4%, +5.0%] | 1 / 7 |
| persistent-64 | 2 | 7 | +2.7% [-0.7%, +21.4%] | 1 / 7 |
| persistent-64 | 4 | 7 | +3.3% [-2.4%, +59.2%] | 1 / 7 |
| persistent-64 | 8 | 7 | +1.7% [-22.7%, +7.9%] | 1 / 7 |
| records-window | 1 | 7 | -3.1% [-5.7%, +6.8%] | 4 / 7 |
| records-window | 2 | 7 | -3.3% [-6.7%, -0.6%] | 7 / 7 |
| records-window | 4 | 7 | -1.9% [-4.4%, +2.1%] | 5 / 7 |
| records-window | 8 | 7 | -2.8% [-5.2%, +9.6%] | 5 / 7 |
| scalar-snapshots | 1 | 7 | -62.5% [-69.2%, -55.7%] | 7 / 7 |
| scalar-snapshots | 2 | 7 | -61.3% [-65.4%, -57.2%] | 7 / 7 |
| scalar-snapshots | 4 | 7 | -63.9% [-68.2%, -55.3%] | 7 / 7 |
| scalar-snapshots | 8 | 7 | -61.8% [-70.1%, -58.3%] | 7 / 7 |

Diagnostic GC phases summed across measured repetitions (wall time):
