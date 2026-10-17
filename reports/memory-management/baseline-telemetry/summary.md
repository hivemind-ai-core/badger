| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 224.122 [220.694, 245.012] | 38.787 | 49.6 |
| backpressure | 2 | baseline | 175.763 [163.280, 177.039] | 37.491 | 55.0 |
| backpressure | 4 | baseline | 120.059 [116.355, 126.254] | 31.446 | 66.4 |
| backpressure | 8 | baseline | 86.764 [85.517, 93.271] | 27.704 | 68.7 |
| balanced | 1 | baseline | 244.708 [243.114, 269.596] | 52.813 | 76.1 |
| balanced | 2 | baseline | 182.766 [178.803, 186.109] | 47.144 | 79.6 |
| balanced | 4 | baseline | 129.298 [125.590, 141.953] | 39.339 | 86.6 |
| balanced | 8 | baseline | 94.129 [93.384, 102.391] | 36.881 | 86.6 |
| cpu | 1 | baseline | 471.002 [453.637, 507.795] | 49.607 | 74.9 |
| cpu | 2 | baseline | 290.332 [286.859, 291.173] | 44.314 | 78.7 |
| cpu | 4 | baseline | 183.289 [172.029, 199.958] | 35.307 | 86.2 |
| cpu | 8 | baseline | 196.957 [182.137, 219.628] | 41.179 | 110.9 |
| fanout | 1 | baseline | 675.253 [606.026, 678.181] | 119.406 | 81.1 |
| fanout | 2 | baseline | 503.546 [481.739, 626.428] | 103.597 | 88.8 |
| fanout | 4 | baseline | 430.845 [395.699, 464.099] | 89.400 | 112.8 |
| fanout | 8 | baseline | 460.766 [443.681, 479.546] | 85.166 | 96.3 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 24576 | 0.992 | 5.120 |
| backpressure | 2 | baseline | 24576 | 0.544 | 4.864 |
| backpressure | 4 | baseline | 24576 | 0.368 | 3.328 |
| backpressure | 8 | baseline | 24576 | 0.240 | 2.560 |
| balanced | 1 | baseline | 24576 | 17.408 | 31.744 |
| balanced | 2 | baseline | 24576 | 11.776 | 26.624 |
| balanced | 4 | baseline | 24576 | 8.704 | 24.576 |
| balanced | 8 | baseline | 24576 | 5.888 | 17.408 |
| cpu | 1 | baseline | 24576 | 31.744 | 47.104 |
| cpu | 2 | baseline | 24576 | 19.456 | 36.864 |
| cpu | 4 | baseline | 24576 | 12.288 | 23.552 |
| cpu | 8 | baseline | 24576 | 13.312 | 25.600 |
| fanout | 1 | baseline | 24576 | 43.008 | 69.632 |
| fanout | 2 | baseline | 24576 | 34.816 | 63.488 |
| fanout | 4 | baseline | 24576 | 27.648 | 55.296 |
| fanout | 8 | baseline | 24576 | 29.696 | 47.104 |

Diagnostic GC phases summed across measured repetitions (wall time):

