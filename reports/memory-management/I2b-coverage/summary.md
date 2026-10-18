| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| entries-immediate | 1 | baseline | 30.310 [29.647, 32.783] | 1.057 | 46.6 |
| entries-immediate | 1 | candidate | 9.532 [9.421, 11.066] | 0.000 | 32.8 |
| entries-immediate | 2 | baseline | 36.429 [35.848, 37.488] | 1.079 | 46.8 |
| entries-immediate | 2 | candidate | 15.870 [15.460, 17.269] | 0.000 | 32.5 |
| entries-immediate | 4 | baseline | 35.914 [35.661, 40.285] | 1.086 | 47.1 |
| entries-immediate | 4 | candidate | 16.372 [15.636, 17.307] | 0.000 | 32.8 |
| entries-immediate | 8 | baseline | 36.420 [36.109, 38.789] | 1.071 | 46.9 |
| entries-immediate | 8 | candidate | 16.251 [15.520, 20.274] | 0.000 | 32.8 |
| entries-stored | 1 | baseline | 31.133 [29.933, 33.794] | 1.107 | 47.3 |
| entries-stored | 1 | candidate | 30.713 [29.892, 31.318] | 1.120 | 48.2 |
| entries-stored | 2 | baseline | 36.899 [35.403, 37.915] | 1.051 | 46.7 |
| entries-stored | 2 | candidate | 36.279 [35.994, 36.985] | 1.156 | 48.2 |
| entries-stored | 4 | baseline | 36.899 [35.876, 38.318] | 1.061 | 46.6 |
| entries-stored | 4 | candidate | 36.506 [35.980, 37.565] | 1.148 | 48.5 |
| entries-stored | 8 | baseline | 36.218 [36.064, 41.905] | 1.081 | 46.7 |
| entries-stored | 8 | candidate | 36.246 [35.896, 41.368] | 1.152 | 48.0 |
| parallel-maps | 1 | baseline | 238.604 [228.757, 252.186] | 72.496 | 43.3 |
| parallel-maps | 1 | candidate | 237.746 [226.024, 250.379] | 75.844 | 44.3 |
| parallel-maps | 2 | baseline | 157.789 [154.003, 159.270] | 50.549 | 51.7 |
| parallel-maps | 2 | candidate | 157.391 [154.256, 172.288] | 50.262 | 53.2 |
| parallel-maps | 4 | baseline | 97.382 [94.243, 101.494] | 30.798 | 64.2 |
| parallel-maps | 4 | candidate | 101.636 [93.061, 109.222] | 32.976 | 65.1 |
| parallel-maps | 8 | baseline | 71.995 [66.584, 77.479] | 25.823 | 74.9 |
| parallel-maps | 8 | candidate | 71.016 [68.225, 73.919] | 26.455 | 72.8 |
| records-window | 1 | baseline | 127.946 [126.334, 130.870] | 22.344 | 51.2 |
| records-window | 1 | candidate | 129.892 [127.254, 146.002] | 23.223 | 51.5 |
| records-window | 2 | baseline | 128.483 [127.128, 132.251] | 22.788 | 50.5 |
| records-window | 2 | candidate | 129.848 [124.194, 132.889] | 23.100 | 51.4 |
| records-window | 4 | baseline | 131.111 [128.391, 132.110] | 23.320 | 51.3 |
| records-window | 4 | candidate | 127.790 [126.032, 135.200] | 23.329 | 52.0 |
| records-window | 8 | baseline | 130.475 [127.591, 132.184] | 23.762 | 51.3 |
| records-window | 8 | candidate | 127.654 [124.711, 131.269] | 23.250 | 52.3 |
| snapshots-major | 1 | baseline | 100.796 [98.549, 108.329] | 28.292 | 48.1 |
| snapshots-major | 1 | candidate | 102.835 [99.084, 105.699] | 28.122 | 49.2 |
| snapshots-major | 2 | baseline | 108.499 [106.251, 110.052] | 28.584 | 48.6 |
| snapshots-major | 2 | candidate | 108.356 [102.286, 109.853] | 28.866 | 49.3 |
| snapshots-major | 4 | baseline | 109.453 [106.244, 113.668] | 29.248 | 48.5 |
| snapshots-major | 4 | candidate | 109.776 [105.169, 114.044] | 29.368 | 49.7 |
| snapshots-major | 8 | baseline | 108.087 [106.730, 109.283] | 28.176 | 48.9 |
| snapshots-major | 8 | candidate | 107.863 [105.264, 114.516] | 28.508 | 49.3 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| entries-immediate | 1 | -68.6% | -100.0% | -29.6% | 1 / 0 |
| entries-immediate | 2 | -56.4% | -100.0% | -30.5% | 1 / 0 |
| entries-immediate | 4 | -54.4% | -100.0% | -30.4% | 1 / 0 |
| entries-immediate | 8 | -55.4% | -100.0% | -30.1% | 1 / 0 |
| entries-stored | 1 | -1.3% | +1.2% | +1.8% | 1 / 1 |
| entries-stored | 2 | -1.7% | +9.9% | +3.2% | 1 / 1 |
| entries-stored | 4 | -1.1% | +8.2% | +4.1% | 1 / 1 |
| entries-stored | 8 | +0.1% | +6.5% | +2.8% | 1 / 1 |
| parallel-maps | 1 | -0.4% | +4.6% | +2.3% | 63 / 63 |
| parallel-maps | 2 | -0.3% | -0.6% | +2.8% | 63 / 63 |
| parallel-maps | 4 | +4.4% | +7.1% | +1.4% | 62 / 62 |
| parallel-maps | 8 | -1.4% | +2.4% | -2.8% | 62 / 62 |
| records-window | 1 | +1.5% | +3.9% | +0.5% | 16 / 16 |
| records-window | 2 | +1.1% | +1.4% | +1.9% | 16 / 16 |
| records-window | 4 | -2.5% | +0.0% | +1.5% | 16 / 16 |
| records-window | 8 | -2.2% | -2.2% | +2.0% | 16 / 16 |
| snapshots-major | 1 | +2.0% | -0.6% | +2.3% | 17 / 17 |
| snapshots-major | 2 | -0.1% | +1.0% | +1.6% | 17 / 17 |
| snapshots-major | 4 | +0.3% | +0.4% | +2.6% | 17 / 17 |
| snapshots-major | 8 | -0.2% | +1.2% | +0.9% | 17 / 17 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| entries-immediate | 1 | 7 | -68.5% [-70.9%, -63.7%] | 7 / 7 |
| entries-immediate | 2 | 7 | -55.8% [-58.1%, -53.3%] | 7 / 7 |
| entries-immediate | 4 | 7 | -56.2% [-60.6%, -51.7%] | 7 / 7 |
| entries-immediate | 8 | 7 | -55.4% [-59.7%, -45.9%] | 7 / 7 |
| entries-stored | 1 | 7 | -0.5% [-8.0%, +1.2%] | 5 / 7 |
| entries-stored | 2 | 7 | -0.8% [-4.7%, +2.5%] | 4 / 7 |
| entries-stored | 4 | 7 | -0.5% [-5.5%, +3.0%] | 4 / 7 |
| entries-stored | 8 | 7 | +0.0% [-13.8%, +14.7%] | 3 / 7 |
| parallel-maps | 1 | 7 | +1.4% [-10.4%, +9.3%] | 3 / 7 |
| parallel-maps | 2 | 7 | +0.9% [-2.6%, +9.5%] | 3 / 7 |
| parallel-maps | 4 | 7 | +2.7% [-5.9%, +12.2%] | 1 / 7 |
| parallel-maps | 8 | 7 | +0.9% [-11.9%, +5.0%] | 2 / 7 |
| records-window | 1 | 7 | +1.8% [-2.1%, +15.6%] | 1 / 7 |
| records-window | 2 | 7 | +0.2% [-3.7%, +4.1%] | 3 / 7 |
| records-window | 4 | 7 | -1.3% [-3.4%, +3.1%] | 5 / 7 |
| records-window | 8 | 7 | -2.4% [-3.4%, +2.9%] | 6 / 7 |
| snapshots-major | 1 | 7 | +0.8% [-2.9%, +4.9%] | 1 / 7 |
| snapshots-major | 2 | 7 | -0.2% [-4.1%, +1.9%] | 5 / 7 |
| snapshots-major | 4 | 7 | +0.3% [-4.8%, +4.8%] | 3 / 7 |
| snapshots-major | 8 | 7 | -0.7% [-3.7%, +6.8%] | 4 / 7 |

Diagnostic GC phases summed across measured repetitions (wall time):
