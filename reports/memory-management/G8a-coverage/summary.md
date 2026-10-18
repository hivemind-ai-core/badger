| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| buffers-capacity-32 | 1 | baseline | 16.236 [15.931, 17.115] | 0.227 | 47.9 |
| buffers-capacity-32 | 1 | candidate | 16.181 [15.989, 16.873] | 0.241 | 48.1 |
| buffers-capacity-32 | 2 | baseline | 16.037 [15.810, 16.452] | 0.211 | 47.7 |
| buffers-capacity-32 | 2 | candidate | 16.092 [15.807, 16.247] | 0.212 | 47.8 |
| buffers-capacity-32 | 4 | baseline | 16.020 [15.861, 16.571] | 0.218 | 48.0 |
| buffers-capacity-32 | 4 | candidate | 16.275 [15.998, 19.027] | 0.231 | 48.3 |
| buffers-capacity-32 | 8 | baseline | 16.593 [15.993, 17.827] | 0.225 | 48.0 |
| buffers-capacity-32 | 8 | candidate | 16.528 [15.931, 16.806] | 0.211 | 48.2 |
| bytes-transient | 1 | baseline | 97.689 [89.562, 102.945] | 0.298 | 48.1 |
| bytes-transient | 1 | candidate | 110.115 [101.402, 119.156] | 0.303 | 48.1 |
| bytes-transient | 2 | baseline | 92.753 [88.571, 95.131] | 0.283 | 47.9 |
| bytes-transient | 2 | candidate | 101.736 [93.877, 104.227] | 0.295 | 47.5 |
| bytes-transient | 4 | baseline | 88.789 [87.054, 93.608] | 0.284 | 48.0 |
| bytes-transient | 4 | candidate | 104.991 [103.227, 109.209] | 0.292 | 48.0 |
| bytes-transient | 8 | baseline | 89.173 [85.275, 92.987] | 0.294 | 47.9 |
| bytes-transient | 8 | candidate | 103.097 [96.024, 105.335] | 0.303 | 47.8 |
| map-new | 1 | baseline | 10.016 [9.571, 10.513] | 0.579 | 48.1 |
| map-new | 1 | candidate | 10.139 [9.584, 15.159] | 0.665 | 48.3 |
| map-new | 2 | baseline | 10.114 [9.836, 13.514] | 0.638 | 48.2 |
| map-new | 2 | candidate | 9.733 [9.494, 10.176] | 0.572 | 48.1 |
| map-new | 4 | baseline | 9.723 [9.648, 10.609] | 0.586 | 48.1 |
| map-new | 4 | candidate | 10.128 [9.695, 11.922] | 0.597 | 48.4 |
| map-new | 8 | baseline | 9.972 [9.840, 10.764] | 0.622 | 48.2 |
| map-new | 8 | candidate | 9.964 [9.691, 11.571] | 0.595 | 47.9 |
| persistent-64 | 1 | baseline | 3.519 [3.474, 3.876] | 0.000 | 32.0 |
| persistent-64 | 1 | candidate | 3.489 [3.362, 3.534] | 0.000 | 31.7 |
| persistent-64 | 2 | baseline | 3.543 [3.415, 4.563] | 0.000 | 32.0 |
| persistent-64 | 2 | candidate | 3.518 [3.385, 4.044] | 0.000 | 32.0 |
| persistent-64 | 4 | baseline | 3.488 [3.430, 3.562] | 0.000 | 31.8 |
| persistent-64 | 4 | candidate | 3.478 [3.444, 3.556] | 0.000 | 31.9 |
| persistent-64 | 8 | baseline | 3.495 [3.427, 3.552] | 0.000 | 32.1 |
| persistent-64 | 8 | candidate | 3.616 [3.447, 4.096] | 0.000 | 31.9 |
| records-window | 1 | baseline | 121.068 [117.538, 172.691] | 18.953 | 53.0 |
| records-window | 1 | candidate | 115.023 [112.725, 170.524] | 18.215 | 53.0 |
| records-window | 2 | baseline | 121.279 [119.252, 125.202] | 18.376 | 53.4 |
| records-window | 2 | candidate | 116.635 [114.854, 121.288] | 18.648 | 53.1 |
| records-window | 4 | baseline | 122.271 [118.655, 128.508] | 18.485 | 53.1 |
| records-window | 4 | candidate | 114.025 [113.176, 120.023] | 18.851 | 53.3 |
| records-window | 8 | baseline | 120.131 [117.331, 124.544] | 18.275 | 53.1 |
| records-window | 8 | candidate | 114.517 [112.753, 117.351] | 18.324 | 53.1 |
| scalar-snapshots | 1 | baseline | 4.605 [4.507, 5.179] | 0.000 | 37.7 |
| scalar-snapshots | 1 | candidate | 4.628 [4.548, 5.169] | 0.000 | 38.0 |
| scalar-snapshots | 2 | baseline | 4.572 [4.511, 4.724] | 0.000 | 37.8 |
| scalar-snapshots | 2 | candidate | 4.669 [4.491, 5.027] | 0.000 | 38.4 |
| scalar-snapshots | 4 | baseline | 4.651 [4.570, 4.816] | 0.000 | 38.1 |
| scalar-snapshots | 4 | candidate | 4.585 [4.457, 4.646] | 0.000 | 37.9 |
| scalar-snapshots | 8 | baseline | 4.611 [4.533, 5.688] | 0.000 | 38.1 |
| scalar-snapshots | 8 | candidate | 4.646 [4.494, 4.780] | 0.000 | 37.9 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| buffers-capacity-32 | 1 | -0.3% | +6.1% | +0.4% | 1 / 1 |
| buffers-capacity-32 | 2 | +0.3% | +0.5% | +0.3% | 1 / 1 |
| buffers-capacity-32 | 4 | +1.6% | +5.9% | +0.6% | 1 / 1 |
| buffers-capacity-32 | 8 | -0.4% | -6.0% | +0.4% | 1 / 1 |
| bytes-transient | 1 | +12.7% | +1.6% | +0.1% | 1 / 1 |
| bytes-transient | 2 | +9.7% | +4.5% | -0.8% | 1 / 1 |
| bytes-transient | 4 | +18.2% | +2.8% | -0.1% | 1 / 1 |
| bytes-transient | 8 | +15.6% | +3.2% | -0.2% | 1 / 1 |
| map-new | 1 | +1.2% | +14.8% | +0.5% | 1 / 1 |
| map-new | 2 | -3.8% | -10.3% | -0.3% | 1 / 1 |
| map-new | 4 | +4.2% | +2.0% | +0.6% | 1 / 1 |
| map-new | 8 | -0.1% | -4.3% | -0.7% | 1 / 1 |
| persistent-64 | 1 | -0.8% | n/a | -0.8% | 0 / 0 |
| persistent-64 | 2 | -0.7% | n/a | -0.2% | 0 / 0 |
| persistent-64 | 4 | -0.3% | n/a | +0.5% | 0 / 0 |
| persistent-64 | 8 | +3.5% | n/a | -0.8% | 0 / 0 |
| records-window | 1 | -5.0% | -3.9% | +0.0% | 16 / 16 |
| records-window | 2 | -3.8% | +1.5% | -0.4% | 16 / 16 |
| records-window | 4 | -6.7% | +2.0% | +0.3% | 16 / 16 |
| records-window | 8 | -4.7% | +0.3% | +0.1% | 16 / 16 |
| scalar-snapshots | 1 | +0.5% | n/a | +0.8% | 0 / 0 |
| scalar-snapshots | 2 | +2.1% | n/a | +1.4% | 0 / 0 |
| scalar-snapshots | 4 | -1.4% | n/a | -0.5% | 0 / 0 |
| scalar-snapshots | 8 | +0.7% | n/a | -0.4% | 0 / 0 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| buffers-capacity-32 | 1 | 7 | +0.9% [-6.5%, +3.2%] | 2 / 7 |
| buffers-capacity-32 | 2 | 7 | +0.0% [-1.7%, +1.6%] | 3 / 7 |
| buffers-capacity-32 | 4 | 7 | +2.3% [-3.5%, +19.0%] | 1 / 7 |
| buffers-capacity-32 | 8 | 7 | -0.1% [-9.9%, +4.4%] | 4 / 7 |
| bytes-transient | 1 | 7 | +12.7% [-1.5%, +32.1%] | 1 / 7 |
| bytes-transient | 2 | 7 | +9.1% [+1.4%, +17.7%] | 0 / 7 |
| bytes-transient | 4 | 7 | +17.1% [+12.2%, +23.0%] | 0 / 7 |
| bytes-transient | 8 | 7 | +17.4% [+3.3%, +21.8%] | 0 / 7 |
| map-new | 1 | 7 | +1.2% [-7.1%, +55.8%] | 2 / 7 |
| map-new | 2 | 7 | -2.6% [-29.7%, +0.6%] | 6 / 7 |
| map-new | 4 | 7 | +3.2% [-8.6%, +22.6%] | 1 / 7 |
| map-new | 8 | 7 | +0.7% [-10.0%, +14.3%] | 3 / 7 |
| persistent-64 | 1 | 7 | -1.3% [-9.3%, +1.7%] | 6 / 7 |
| persistent-64 | 2 | 7 | +0.1% [-25.8%, +18.4%] | 3 / 7 |
| persistent-64 | 4 | 7 | -0.2% [-1.6%, +1.1%] | 4 / 7 |
| persistent-64 | 8 | 7 | +2.2% [-2.8%, +18.0%] | 3 / 7 |
| records-window | 1 | 7 | -4.1% [-31.5%, +44.2%] | 6 / 7 |
| records-window | 2 | 7 | -4.3% [-6.7%, -0.7%] | 7 / 7 |
| records-window | 4 | 7 | -7.2% [-11.3%, -0.8%] | 7 / 7 |
| records-window | 8 | 7 | -5.4% [-6.7%, +0.0%] | 6 / 7 |
| scalar-snapshots | 1 | 7 | +0.5% [-7.6%, +14.0%] | 3 / 7 |
| scalar-snapshots | 2 | 7 | +1.1% [-2.6%, +10.0%] | 3 / 7 |
| scalar-snapshots | 4 | 7 | -1.4% [-4.1%, -0.2%] | 7 / 7 |
| scalar-snapshots | 8 | 7 | +1.2% [-19.7%, +4.3%] | 3 / 7 |

Diagnostic GC phases summed across measured repetitions (wall time):
