| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 436.853 [412.600, 454.155] | 62.402 | 55.1 |
| backpressure | 1 | candidate | 400.607 [385.396, 556.471] | 51.087 | 58.8 |
| backpressure | 2 | baseline | 286.744 [279.194, 288.724] | 50.517 | 62.9 |
| backpressure | 2 | candidate | 264.192 [247.131, 265.440] | 39.784 | 68.2 |
| backpressure | 4 | baseline | 165.325 [161.435, 171.533] | 37.837 | 72.9 |
| backpressure | 4 | candidate | 145.517 [144.690, 149.182] | 26.731 | 69.2 |
| backpressure | 8 | baseline | 128.438 [110.330, 130.107] | 34.787 | 76.0 |
| backpressure | 8 | candidate | 113.122 [97.937, 114.956] | 25.446 | 71.3 |
| balanced | 1 | baseline | 386.512 [386.205, 407.115] | 62.588 | 83.8 |
| balanced | 1 | candidate | 396.727 [393.032, 421.835] | 66.118 | 87.6 |
| balanced | 2 | baseline | 315.487 [303.180, 414.054] | 59.763 | 102.5 |
| balanced | 2 | candidate | 292.224 [246.564, 379.639] | 53.408 | 94.6 |
| balanced | 4 | baseline | 173.962 [160.373, 180.397] | 48.813 | 93.9 |
| balanced | 4 | candidate | 165.254 [152.578, 177.427] | 40.692 | 105.1 |
| balanced | 8 | baseline | 112.130 [111.963, 120.916] | 36.511 | 110.7 |
| balanced | 8 | candidate | 105.022 [98.908, 123.543] | 30.861 | 114.8 |
| cpu | 1 | baseline | 838.592 [799.030, 877.616] | 62.568 | 77.7 |
| cpu | 1 | candidate | 792.810 [787.655, 848.975] | 61.174 | 82.0 |
| cpu | 2 | baseline | 528.159 [522.703, 579.965] | 58.305 | 90.4 |
| cpu | 2 | candidate | 515.322 [446.249, 556.380] | 50.980 | 88.1 |
| cpu | 4 | baseline | 268.675 [254.472, 275.273] | 43.516 | 93.7 |
| cpu | 4 | candidate | 262.564 [251.757, 393.485] | 39.681 | 97.9 |
| cpu | 8 | baseline | 308.042 [307.390, 321.370] | 45.591 | 128.5 |
| cpu | 8 | candidate | 307.463 [296.158, 319.628] | 49.779 | 132.5 |
| fanout | 1 | baseline | 1039.580 [1018.683, 1358.528] | 160.539 | 85.8 |
| fanout | 1 | candidate | 1049.305 [1003.251, 1143.444] | 162.352 | 86.3 |
| fanout | 2 | baseline | 849.690 [802.165, 1182.471] | 151.631 | 106.7 |
| fanout | 2 | candidate | 807.728 [769.536, 1302.833] | 149.753 | 104.0 |
| fanout | 4 | baseline | 671.889 [634.206, 713.586] | 121.531 | 128.8 |
| fanout | 4 | candidate | 645.869 [625.628, 668.323] | 130.373 | 134.7 |
| fanout | 8 | baseline | 715.912 [714.493, 752.328] | 104.819 | 105.3 |
| fanout | 8 | candidate | 717.004 [696.580, 770.069] | 106.891 | 113.9 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| backpressure | 1 | -8.3% | -18.1% | +6.8% | 22 / 22 |
| backpressure | 2 | -7.9% | -21.2% | +8.3% | 22 / 22 |
| backpressure | 4 | -12.0% | -29.4% | -5.0% | 22 / 22 |
| backpressure | 8 | -11.9% | -26.9% | -6.1% | 21 / 21 |
| balanced | 1 | +2.6% | +5.6% | +4.6% | 22 / 22 |
| balanced | 2 | -7.4% | -10.6% | -7.7% | 22 / 22 |
| balanced | 4 | -5.0% | -16.6% | +11.9% | 22 / 22 |
| balanced | 8 | -6.3% | -15.5% | +3.7% | 21 / 21 |
| cpu | 1 | -5.5% | -2.2% | +5.6% | 22 / 22 |
| cpu | 2 | -2.4% | -12.6% | -2.5% | 22 / 22 |
| cpu | 4 | -2.3% | -8.8% | +4.5% | 22 / 22 |
| cpu | 8 | -0.2% | +9.2% | +3.1% | 22 / 22 |
| fanout | 1 | +0.9% | +1.1% | +0.5% | 42 / 42 |
| fanout | 2 | -4.9% | -1.2% | -2.5% | 42 / 42 |
| fanout | 4 | -3.9% | +7.3% | +4.6% | 42 / 42 |
| fanout | 8 | +0.2% | +2.0% | +8.2% | 41 / 41 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| backpressure | 1 | 3 | -2.9% [-11.8%, +22.5%] | 2 / 3 |
| backpressure | 2 | 3 | -7.9% [-14.4%, -4.9%] | 3 / 3 |
| backpressure | 4 | 3 | -12.0% [-13.0%, -10.4%] | 3 / 3 |
| backpressure | 8 | 3 | -11.6% [-11.9%, -11.2%] | 3 / 3 |
| balanced | 1 | 3 | +2.6% [+1.8%, +3.6%] | 0 / 3 |
| balanced | 2 | 3 | -8.3% [-18.7%, -7.4%] | 3 / 3 |
| balanced | 4 | 3 | +2.0% [-15.4%, +3.0%] | 1 / 3 |
| balanced | 8 | 3 | -6.3% [-11.7%, +2.2%] | 2 / 3 |
| cpu | 1 | 3 | -1.4% [-9.7%, +1.2%] | 2 / 3 |
| cpu | 2 | 3 | -4.1% [-15.5%, -1.4%] | 3 / 3 |
| cpu | 4 | 3 | +3.2% [-8.5%, +46.5%] | 1 / 3 |
| cpu | 8 | 3 | -0.5% [-3.9%, +0.0%] | 2 / 3 |
| fanout | 1 | 3 | -1.5% [-15.8%, +0.9%] | 2 / 3 |
| fanout | 2 | 3 | -4.1% [-4.9%, +10.2%] | 2 / 3 |
| fanout | 4 | 3 | -6.9% [-9.5%, +5.4%] | 2 / 3 |
| fanout | 8 | 3 | +0.2% [-2.5%, +2.4%] | 1 / 3 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| backpressure | 1 | baseline | 49152 | 0.992 | 4.352 |
| backpressure | 1 | candidate | 49152 | 0.992 | 4.864 |
| backpressure | 2 | baseline | 49152 | 0.496 | 3.200 |
| backpressure | 2 | candidate | 49152 | 0.464 | 2.432 |
| backpressure | 4 | baseline | 49152 | 0.256 | 1.984 |
| backpressure | 4 | candidate | 49152 | 0.240 | 1.536 |
| backpressure | 8 | baseline | 49152 | 0.176 | 1.792 |
| backpressure | 8 | candidate | 49152 | 0.168 | 1.408 |
| balanced | 1 | baseline | 49152 | 13.312 | 23.552 |
| balanced | 1 | candidate | 49152 | 13.312 | 24.576 |
| balanced | 2 | baseline | 49152 | 10.752 | 26.624 |
| balanced | 2 | candidate | 49152 | 9.728 | 21.504 |
| balanced | 4 | baseline | 49152 | 5.632 | 17.408 |
| balanced | 4 | candidate | 49152 | 5.120 | 13.312 |
| balanced | 8 | baseline | 49152 | 3.584 | 11.776 |
| balanced | 8 | candidate | 49152 | 3.456 | 17.408 |
| cpu | 1 | baseline | 49152 | 28.672 | 38.912 |
| cpu | 1 | candidate | 49152 | 27.648 | 36.864 |
| cpu | 2 | baseline | 49152 | 18.432 | 29.696 |
| cpu | 2 | candidate | 49152 | 16.384 | 36.864 |
| cpu | 4 | baseline | 49152 | 8.704 | 17.408 |
| cpu | 4 | candidate | 49152 | 9.216 | 25.600 |
| cpu | 8 | baseline | 49152 | 10.752 | 19.456 |
| cpu | 8 | candidate | 49152 | 10.240 | 26.624 |
| fanout | 1 | baseline | 49152 | 36.864 | 73.728 |
| fanout | 1 | candidate | 49152 | 36.864 | 51.200 |
| fanout | 2 | baseline | 49152 | 30.720 | 73.728 |
| fanout | 2 | candidate | 49152 | 28.672 | 61.440 |
| fanout | 4 | baseline | 49152 | 21.504 | 55.296 |
| fanout | 4 | candidate | 49152 | 20.480 | 40.960 |
| fanout | 8 | baseline | 49152 | 23.552 | 36.864 |
| fanout | 8 | candidate | 49152 | 24.576 | 40.960 |

Diagnostic GC phases summed across measured repetitions (wall time):
