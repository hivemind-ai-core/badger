| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 4 | baseline | 173.542 [169.406, 177.378] | 43.403 | 70.5 |
| backpressure | 4 | candidate | 160.037 [154.414, 218.160] | 36.315 | 70.9 |
| backpressure | 8 | baseline | 121.218 [117.129, 126.432] | 35.836 | 78.8 |
| backpressure | 8 | candidate | 109.839 [105.352, 117.119] | 28.823 | 78.6 |
| balanced | 4 | baseline | 181.428 [178.096, 195.941] | 56.444 | 97.4 |
| balanced | 4 | candidate | 161.437 [158.560, 179.016] | 43.638 | 96.1 |
| balanced | 8 | baseline | 127.529 [125.352, 130.263] | 48.331 | 103.3 |
| balanced | 8 | candidate | 112.468 [109.101, 130.614] | 37.198 | 106.7 |
| cpu | 4 | baseline | 271.681 [267.759, 288.238] | 54.158 | 96.3 |
| cpu | 4 | candidate | 254.472 [252.175, 269.552] | 42.959 | 94.0 |
| cpu | 8 | baseline | 299.975 [292.465, 317.439] | 53.627 | 125.4 |
| cpu | 8 | candidate | 295.101 [276.193, 312.937] | 42.168 | 128.3 |
| fanout | 4 | baseline | 671.304 [632.039, 698.926] | 142.384 | 129.9 |
| fanout | 4 | candidate | 636.322 [596.937, 683.020] | 121.237 | 126.4 |
| fanout | 8 | baseline | 739.890 [730.932, 747.262] | 124.896 | 108.2 |
| fanout | 8 | candidate | 709.874 [697.676, 720.490] | 102.779 | 107.4 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| backpressure | 4 | -7.8% | -16.3% | +0.7% | 22 / 22 |
| backpressure | 8 | -9.4% | -19.6% | -0.2% | 22 / 21 |
| balanced | 4 | -11.0% | -22.7% | -1.3% | 22 / 22 |
| balanced | 8 | -11.8% | -23.0% | +3.3% | 22 / 21 |
| cpu | 4 | -6.3% | -20.7% | -2.4% | 22 / 22 |
| cpu | 8 | -1.6% | -21.4% | +2.3% | 22 / 22 |
| fanout | 4 | -5.2% | -14.9% | -2.7% | 42 / 42 |
| fanout | 8 | -4.1% | -17.7% | -0.7% | 41 / 41 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| backpressure | 4 | 9 | -7.0% [-12.0%, +25.4%] | 8 / 9 |
| backpressure | 8 | 9 | -9.5% [-16.7%, -1.7%] | 9 / 9 |
| balanced | 4 | 9 | -10.6% [-18.2%, +0.2%] | 8 / 9 |
| balanced | 8 | 9 | -11.9% [-14.5%, +3.5%] | 8 / 9 |
| cpu | 4 | 9 | -5.7% [-12.5%, -0.4%] | 9 / 9 |
| cpu | 8 | 9 | -2.5% [-10.4%, +7.0%] | 5 / 9 |
| fanout | 4 | 9 | -5.5% [-13.8%, +7.6%] | 6 / 9 |
| fanout | 8 | 9 | -4.2% [-5.8%, -1.8%] | 9 / 9 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| backpressure | 4 | baseline | 147456 | 0.272 | 2.304 |
| backpressure | 4 | candidate | 147456 | 0.248 | 1.984 |
| backpressure | 8 | baseline | 147456 | 0.168 | 1.856 |
| backpressure | 8 | candidate | 147456 | 0.160 | 1.536 |
| balanced | 4 | baseline | 147456 | 5.888 | 17.408 |
| balanced | 4 | candidate | 147456 | 5.376 | 13.312 |
| balanced | 8 | baseline | 147456 | 3.968 | 14.336 |
| balanced | 8 | candidate | 147456 | 3.584 | 11.776 |
| cpu | 4 | baseline | 147456 | 9.216 | 20.480 |
| cpu | 4 | candidate | 147456 | 8.704 | 17.408 |
| cpu | 8 | baseline | 147456 | 10.240 | 22.528 |
| cpu | 8 | candidate | 147456 | 9.728 | 20.480 |
| fanout | 4 | baseline | 147456 | 20.480 | 38.912 |
| fanout | 4 | candidate | 147456 | 20.480 | 34.816 |
| fanout | 8 | baseline | 147456 | 24.576 | 38.912 |
| fanout | 8 | candidate | 147456 | 23.552 | 34.816 |

Diagnostic GC phases summed across measured repetitions (wall time):
