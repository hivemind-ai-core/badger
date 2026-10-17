| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| bytes-transient | 1 | baseline | 114.777 [108.861, 122.521] | 2.005 | 38.3 |
| bytes-transient | 1 | candidate | 114.999 [102.761, 131.042] | 2.010 | 38.4 |
| bytes-transient | 8 | baseline | 110.793 [97.463, 129.166] | 1.983 | 38.0 |
| bytes-transient | 8 | candidate | 109.103 [97.629, 116.548] | 2.075 | 38.2 |
| messages-int-sustained | 1 | baseline | 103.695 [96.188, 107.765] | 9.749 | 38.8 |
| messages-int-sustained | 1 | candidate | 102.747 [96.007, 112.789] | 9.662 | 38.8 |
| messages-int-sustained | 8 | baseline | 111.971 [103.952, 131.665] | 8.531 | 46.9 |
| messages-int-sustained | 8 | candidate | 104.014 [97.818, 119.301] | 8.153 | 45.3 |
| words-transient | 1 | baseline | 129.525 [123.138, 142.268] | 7.633 | 38.6 |
| words-transient | 1 | candidate | 129.439 [121.590, 136.665] | 7.857 | 38.8 |
| words-transient | 8 | baseline | 126.397 [117.008, 134.846] | 7.169 | 38.4 |
| words-transient | 8 | candidate | 120.900 [112.844, 129.211] | 7.237 | 38.2 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| bytes-transient | 1 | +0.2% | +0.3% | +0.2% | 1 / 1 |
| bytes-transient | 8 | -1.5% | +4.6% | +0.5% | 1 / 1 |
| messages-int-sustained | 1 | -0.9% | -0.9% | -0.0% | 5 / 5 |
| messages-int-sustained | 8 | -7.1% | -4.4% | -3.3% | 5 / 5 |
| words-transient | 1 | -0.1% | +2.9% | +0.6% | 8 / 8 |
| words-transient | 8 | -4.3% | +0.9% | -0.4% | 8 / 8 |

Diagnostic GC phases summed across measured repetitions (wall time):
