| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| map-api-composed | 1 | baseline | 157.725 [151.845, 159.514] | 15.079 | 46.8 |
| map-api-composed | 1 | candidate | 125.954 [119.083, 134.700] | 12.125 | 45.2 |
| map-api-composed | 8 | baseline | 160.334 [155.301, 164.349] | 15.317 | 46.0 |
| map-api-composed | 8 | candidate | 123.821 [122.929, 128.947] | 11.807 | 46.5 |
| map-api-entry-control | 1 | baseline | 20.537 [20.386, 22.556] | 0.840 | 43.5 |
| map-api-entry-control | 1 | candidate | 19.891 [19.229, 22.686] | 0.869 | 42.2 |
| map-api-entry-control | 8 | baseline | 23.000 [22.428, 30.299] | 0.828 | 42.8 |
| map-api-entry-control | 8 | candidate | 22.496 [22.208, 24.966] | 0.834 | 43.3 |
| map-api-filter | 1 | baseline | 27.145 [26.414, 28.578] | 1.595 | 41.4 |
| map-api-filter | 1 | candidate | 17.706 [17.284, 18.596] | 1.272 | 42.7 |
| map-api-filter | 8 | baseline | 29.128 [27.879, 30.325] | 1.628 | 42.9 |
| map-api-filter | 8 | candidate | 20.051 [18.720, 22.969] | 1.350 | 43.0 |
| map-api-merge | 1 | baseline | 87.266 [85.462, 93.642] | 6.555 | 51.2 |
| map-api-merge | 1 | candidate | 64.649 [63.944, 68.873] | 4.454 | 47.4 |
| map-api-merge | 8 | baseline | 90.336 [88.909, 102.284] | 6.355 | 50.6 |
| map-api-merge | 8 | candidate | 72.112 [69.973, 75.570] | 4.646 | 48.6 |
| map-api-parallel | 1 | baseline | 198.541 [195.336, 203.070] | 29.421 | 61.0 |
| map-api-parallel | 1 | candidate | 158.110 [156.644, 163.505] | 21.735 | 54.4 |
| map-api-parallel | 8 | baseline | 62.411 [58.314, 64.345] | 20.317 | 78.7 |
| map-api-parallel | 8 | candidate | 55.710 [51.003, 61.980] | 16.680 | 75.5 |
| map-api-projections | 1 | baseline | 29.732 [29.652, 33.900] | 0.920 | 42.5 |
| map-api-projections | 1 | candidate | 9.542 [9.288, 9.834] | 0.000 | 30.2 |
| map-api-projections | 8 | baseline | 32.414 [31.451, 33.202] | 0.908 | 41.8 |
| map-api-projections | 8 | candidate | 12.423 [12.241, 12.978] | 0.000 | 31.2 |
| map-api-values | 1 | baseline | 39.941 [38.779, 41.696] | 2.933 | 43.1 |
| map-api-values | 1 | candidate | 29.320 [28.926, 30.739] | 2.064 | 41.8 |
| map-api-values | 8 | baseline | 43.083 [42.270, 47.679] | 3.426 | 42.5 |
| map-api-values | 8 | candidate | 33.340 [32.218, 42.939] | 2.135 | 42.8 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| map-api-composed | 1 | -20.1% | -19.6% | -3.4% | 21 / 19 |
| map-api-composed | 8 | -22.8% | -22.9% | +1.0% | 21 / 19 |
| map-api-entry-control | 1 | -3.1% | +3.5% | -2.9% | 1 / 1 |
| map-api-entry-control | 8 | -2.2% | +0.8% | +1.2% | 1 / 1 |
| map-api-filter | 1 | -34.8% | -20.2% | +3.2% | 3 / 3 |
| map-api-filter | 8 | -31.2% | -17.1% | +0.2% | 3 / 3 |
| map-api-merge | 1 | -25.9% | -32.0% | -7.5% | 8 / 7 |
| map-api-merge | 8 | -20.2% | -26.9% | -4.1% | 8 / 7 |
| map-api-parallel | 1 | -20.4% | -26.1% | -10.7% | 21 / 19 |
| map-api-parallel | 8 | -10.7% | -17.9% | -4.1% | 21 / 19 |
| map-api-projections | 1 | -67.9% | -100.0% | -28.9% | 1 / 0 |
| map-api-projections | 8 | -61.7% | -100.0% | -25.4% | 1 / 0 |
| map-api-values | 1 | -26.6% | -29.6% | -3.1% | 6 / 5 |
| map-api-values | 8 | -22.6% | -37.7% | +0.7% | 6 / 5 |

Diagnostic GC phases summed across measured repetitions (wall time):
