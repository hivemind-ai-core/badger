| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 222.037 [220.764, 228.090] | 38.658 | 49.9 |
| backpressure | 1 | candidate | 220.069 [218.381, 225.629] | 38.265 | 50.0 |
| backpressure | 2 | baseline | 167.185 [166.102, 178.313] | 37.937 | 54.6 |
| backpressure | 2 | candidate | 164.485 [161.439, 173.474] | 36.881 | 52.9 |
| backpressure | 4 | baseline | 108.531 [104.223, 112.331] | 28.622 | 64.3 |
| backpressure | 4 | candidate | 108.326 [107.302, 118.084] | 28.958 | 65.3 |
| backpressure | 8 | baseline | 77.845 [75.163, 81.120] | 25.135 | 72.5 |
| backpressure | 8 | candidate | 80.540 [75.803, 81.801] | 26.851 | 73.1 |
| balanced | 1 | baseline | 234.296 [228.190, 235.077] | 48.443 | 76.3 |
| balanced | 1 | candidate | 237.204 [229.299, 239.118] | 49.799 | 76.1 |
| balanced | 2 | baseline | 175.042 [170.430, 200.376] | 46.059 | 79.6 |
| balanced | 2 | candidate | 176.275 [166.610, 200.791] | 45.617 | 79.4 |
| balanced | 4 | baseline | 122.936 [117.417, 131.431] | 37.676 | 84.4 |
| balanced | 4 | candidate | 120.166 [113.151, 121.111] | 35.383 | 84.1 |
| balanced | 8 | baseline | 83.462 [82.729, 86.491] | 32.341 | 90.6 |
| balanced | 8 | candidate | 84.060 [82.578, 85.690] | 32.405 | 87.1 |
| cpu | 1 | baseline | 447.807 [444.015, 453.085] | 49.413 | 74.9 |
| cpu | 1 | candidate | 449.277 [447.721, 452.912] | 48.368 | 74.8 |
| cpu | 2 | baseline | 269.799 [263.830, 283.366] | 42.918 | 78.5 |
| cpu | 2 | candidate | 275.337 [272.085, 286.378] | 43.822 | 77.9 |
| cpu | 4 | baseline | 160.420 [156.667, 163.148] | 34.679 | 87.7 |
| cpu | 4 | candidate | 160.157 [158.735, 163.543] | 33.946 | 82.8 |
| cpu | 8 | baseline | 170.577 [167.700, 180.691] | 35.450 | 112.3 |
| cpu | 8 | candidate | 174.134 [167.016, 184.703] | 36.307 | 109.0 |
| fanout | 1 | baseline | 603.013 [599.691, 685.674] | 117.702 | 80.7 |
| fanout | 1 | candidate | 610.250 [588.750, 622.058] | 116.229 | 80.9 |
| fanout | 2 | baseline | 497.347 [481.424, 530.866] | 108.258 | 92.0 |
| fanout | 2 | candidate | 497.882 [461.250, 511.298] | 103.195 | 88.7 |
| fanout | 4 | baseline | 369.563 [366.091, 395.292] | 82.142 | 104.8 |
| fanout | 4 | candidate | 374.698 [351.113, 396.765] | 85.087 | 103.9 |
| fanout | 8 | baseline | 418.590 [406.477, 436.340] | 78.280 | 93.9 |
| fanout | 8 | candidate | 416.108 [402.757, 431.410] | 79.487 | 96.0 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| backpressure | 1 | -0.9% | -1.0% | +0.2% | 12 / 12 |
| backpressure | 2 | -1.6% | -2.8% | -3.2% | 12 / 12 |
| backpressure | 4 | -0.2% | +1.2% | +1.5% | 12 / 12 |
| backpressure | 8 | +3.5% | +6.8% | +0.8% | 12 / 12 |
| balanced | 1 | +1.2% | +2.8% | -0.3% | 12 / 12 |
| balanced | 2 | +0.7% | -1.0% | -0.3% | 12 / 12 |
| balanced | 4 | -2.3% | -6.1% | -0.3% | 12 / 12 |
| balanced | 8 | +0.7% | +0.2% | -3.9% | 12 / 12 |
| cpu | 1 | +0.3% | -2.1% | -0.1% | 12 / 12 |
| cpu | 2 | +2.1% | +2.1% | -0.8% | 12 / 12 |
| cpu | 4 | -0.2% | -2.1% | -5.6% | 12 / 12 |
| cpu | 8 | +2.1% | +2.4% | -3.0% | 12 / 12 |
| fanout | 1 | +1.2% | -1.3% | +0.2% | 22 / 22 |
| fanout | 2 | +0.1% | -4.7% | -3.6% | 22 / 22 |
| fanout | 4 | +1.4% | +3.6% | -0.8% | 22 / 22 |
| fanout | 8 | -0.6% | +1.5% | +2.2% | 22 / 22 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 40960 | 0.960 | 5.888 |
| backpressure | 1 | candidate | 40960 | 0.960 | 5.376 |
| backpressure | 2 | baseline | 40960 | 0.544 | 4.608 |
| backpressure | 2 | candidate | 40960 | 0.512 | 4.608 |
| backpressure | 4 | baseline | 40960 | 0.320 | 3.072 |
| backpressure | 4 | candidate | 40960 | 0.320 | 3.072 |
| backpressure | 8 | baseline | 40960 | 0.208 | 2.432 |
| backpressure | 8 | candidate | 40960 | 0.208 | 2.688 |
| balanced | 1 | baseline | 40960 | 15.360 | 29.696 |
| balanced | 1 | candidate | 40960 | 15.872 | 28.672 |
| balanced | 2 | baseline | 40960 | 11.264 | 23.552 |
| balanced | 2 | candidate | 40960 | 11.264 | 27.648 |
| balanced | 4 | baseline | 40960 | 7.680 | 21.504 |
| balanced | 4 | candidate | 40960 | 7.424 | 20.480 |
| balanced | 8 | baseline | 40960 | 5.120 | 16.384 |
| balanced | 8 | candidate | 40960 | 5.120 | 16.384 |
| cpu | 1 | baseline | 40960 | 30.720 | 43.008 |
| cpu | 1 | candidate | 40960 | 30.720 | 43.008 |
| cpu | 2 | baseline | 40960 | 18.432 | 31.744 |
| cpu | 2 | candidate | 40960 | 18.432 | 30.720 |
| cpu | 4 | baseline | 40960 | 10.240 | 22.528 |
| cpu | 4 | candidate | 40960 | 10.240 | 20.480 |
| cpu | 8 | baseline | 40960 | 11.264 | 22.528 |
| cpu | 8 | candidate | 40960 | 11.264 | 22.528 |
| fanout | 1 | baseline | 40960 | 40.960 | 65.536 |
| fanout | 1 | candidate | 40960 | 40.960 | 61.440 |
| fanout | 2 | baseline | 40960 | 32.768 | 65.536 |
| fanout | 2 | candidate | 40960 | 32.768 | 53.248 |
| fanout | 4 | baseline | 40960 | 23.552 | 43.008 |
| fanout | 4 | candidate | 40960 | 23.552 | 38.912 |
| fanout | 8 | baseline | 40960 | 27.648 | 40.960 |
| fanout | 8 | candidate | 40960 | 26.624 | 43.008 |

Diagnostic GC phases summed across measured repetitions (wall time):

