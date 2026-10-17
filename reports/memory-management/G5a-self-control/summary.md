| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| backpressure | 4 | baseline | 182.661 [178.620, 186.944] | 44.028 | 70.1 |
| backpressure | 4 | candidate | 177.676 [175.036, 182.298] | 43.632 | 71.4 |
| backpressure | 8 | baseline | 135.233 [128.892, 137.960] | 40.139 | 78.6 |
| backpressure | 8 | candidate | 134.894 [129.023, 140.085] | 39.408 | 78.8 |
| balanced | 4 | baseline | 192.601 [186.349, 196.952] | 57.424 | 100.5 |
| balanced | 4 | candidate | 192.267 [185.422, 194.535] | 56.810 | 96.6 |
| balanced | 8 | baseline | 141.327 [135.310, 143.132] | 51.683 | 100.1 |
| balanced | 8 | candidate | 144.419 [141.850, 148.544] | 54.133 | 103.8 |
| cpu | 4 | baseline | 284.653 [281.386, 302.686] | 51.870 | 98.3 |
| cpu | 4 | candidate | 285.711 [275.801, 292.630] | 53.995 | 98.4 |
| cpu | 8 | baseline | 312.264 [295.743, 327.228] | 55.572 | 128.8 |
| cpu | 8 | candidate | 327.481 [304.201, 345.914] | 56.747 | 127.6 |
| fanout | 4 | baseline | 705.124 [655.169, 780.824] | 142.563 | 118.8 |
| fanout | 4 | candidate | 731.597 [694.520, 747.461] | 147.778 | 131.0 |
| fanout | 8 | baseline | 799.045 [778.321, 816.100] | 132.682 | 106.3 |
| fanout | 8 | candidate | 803.548 [779.196, 820.119] | 130.129 | 104.2 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| backpressure | 4 | -2.7% | -0.9% | +1.9% | 22 / 22 |
| backpressure | 8 | -0.3% | -1.8% | +0.3% | 22 / 22 |
| balanced | 4 | -0.2% | -1.1% | -3.9% | 22 / 22 |
| balanced | 8 | +2.2% | +4.7% | +3.8% | 22 / 22 |
| cpu | 4 | +0.4% | +4.1% | +0.1% | 22 / 22 |
| cpu | 8 | +4.9% | +2.1% | -1.0% | 22 / 22 |
| fanout | 4 | +3.8% | +3.7% | +10.3% | 42 / 42 |
| fanout | 8 | +0.6% | -1.9% | -2.0% | 41 / 41 |

End-to-end latency: pooled histogram bucket upper bounds across repetitions.
These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;
p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.

| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |
|---|---:|---|---:|---:|---:|
| backpressure | 4 | baseline | 81920 | 0.288 | 2.304 |
| backpressure | 4 | candidate | 81920 | 0.272 | 2.304 |
| backpressure | 8 | baseline | 81920 | 0.184 | 1.984 |
| backpressure | 8 | candidate | 81920 | 0.192 | 1.984 |
| balanced | 4 | baseline | 81920 | 6.144 | 17.408 |
| balanced | 4 | candidate | 81920 | 6.144 | 17.408 |
| balanced | 8 | baseline | 81920 | 4.352 | 14.848 |
| balanced | 8 | candidate | 81920 | 4.608 | 15.360 |
| cpu | 4 | baseline | 81920 | 9.216 | 20.480 |
| cpu | 4 | candidate | 81920 | 9.216 | 20.480 |
| cpu | 8 | baseline | 81920 | 10.240 | 22.528 |
| cpu | 8 | candidate | 81920 | 10.752 | 22.528 |
| fanout | 4 | baseline | 81920 | 22.528 | 43.008 |
| fanout | 4 | candidate | 81920 | 22.528 | 45.056 |
| fanout | 8 | baseline | 81920 | 26.624 | 40.960 |
| fanout | 8 | candidate | 81920 | 26.624 | 40.960 |

Diagnostic GC phases summed across measured repetitions (wall time):
