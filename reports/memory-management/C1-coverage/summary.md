| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| codec-flat | 1 | baseline | 11.512 [11.077, 14.784] | 0.000 | 35.5 |
| codec-flat | 1 | candidate | 7.271 [6.927, 8.243] | 0.000 | 36.0 |
| codec-flat | 2 | baseline | 11.998 [11.792, 12.299] | 0.000 | 35.8 |
| codec-flat | 2 | candidate | 7.896 [7.747, 8.889] | 0.000 | 36.4 |
| codec-flat | 4 | baseline | 12.146 [11.843, 16.318] | 0.000 | 36.0 |
| codec-flat | 4 | candidate | 8.040 [7.737, 8.537] | 0.000 | 36.1 |
| codec-flat | 8 | baseline | 12.179 [11.938, 12.376] | 0.000 | 35.7 |
| codec-flat | 8 | candidate | 8.471 [7.649, 10.278] | 0.000 | 36.1 |
| codec-invalid | 1 | baseline | 36.754 [35.418, 41.272] | 0.000 | 54.0 |
| codec-invalid | 1 | candidate | 28.069 [26.702, 32.198] | 0.000 | 54.6 |
| codec-invalid | 2 | baseline | 37.859 [37.572, 42.120] | 0.000 | 53.6 |
| codec-invalid | 2 | candidate | 28.620 [28.321, 30.811] | 0.000 | 54.5 |
| codec-invalid | 4 | baseline | 37.932 [37.591, 41.166] | 0.000 | 54.1 |
| codec-invalid | 4 | candidate | 28.841 [27.605, 37.140] | 0.000 | 54.8 |
| codec-invalid | 8 | baseline | 38.221 [36.781, 38.463] | 0.000 | 54.0 |
| codec-invalid | 8 | candidate | 29.559 [28.302, 29.975] | 0.000 | 54.7 |
| codec-nested | 1 | baseline | 106.908 [105.870, 122.783] | 1.371 | 54.2 |
| codec-nested | 1 | candidate | 55.531 [53.716, 58.206] | 1.283 | 54.8 |
| codec-nested | 2 | baseline | 108.082 [105.494, 127.556] | 1.308 | 54.4 |
| codec-nested | 2 | candidate | 55.748 [53.750, 57.418] | 1.275 | 54.3 |
| codec-nested | 4 | baseline | 106.675 [104.901, 118.732] | 1.310 | 54.2 |
| codec-nested | 4 | candidate | 57.454 [54.489, 64.902] | 1.353 | 54.9 |
| codec-nested | 8 | baseline | 111.965 [106.090, 124.248] | 1.322 | 54.5 |
| codec-nested | 8 | candidate | 56.088 [54.031, 59.701] | 1.303 | 54.8 |
| codec-wire | 1 | baseline | 38.293 [37.351, 38.619] | 0.000 | 35.5 |
| codec-wire | 1 | candidate | 20.024 [19.576, 20.672] | 0.000 | 35.9 |
| codec-wire | 2 | baseline | 39.297 [39.019, 44.694] | 0.000 | 35.5 |
| codec-wire | 2 | candidate | 21.041 [20.560, 22.474] | 0.000 | 35.9 |
| codec-wire | 4 | baseline | 39.495 [38.629, 42.105] | 0.000 | 35.6 |
| codec-wire | 4 | candidate | 20.887 [20.486, 24.334] | 0.000 | 36.6 |
| codec-wire | 8 | baseline | 39.520 [38.997, 49.186] | 0.000 | 35.7 |
| codec-wire | 8 | candidate | 20.839 [20.598, 23.968] | 0.000 | 36.2 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| codec-flat | 1 | -36.8% | n/a | +1.5% | 0 / 0 |
| codec-flat | 2 | -34.2% | n/a | +1.6% | 0 / 0 |
| codec-flat | 4 | -33.8% | n/a | +0.2% | 0 / 0 |
| codec-flat | 8 | -30.4% | n/a | +1.1% | 0 / 0 |
| codec-invalid | 1 | -23.6% | n/a | +1.1% | 0 / 0 |
| codec-invalid | 2 | -24.4% | n/a | +1.7% | 0 / 0 |
| codec-invalid | 4 | -24.0% | n/a | +1.1% | 0 / 0 |
| codec-invalid | 8 | -22.7% | n/a | +1.3% | 0 / 0 |
| codec-nested | 1 | -48.1% | -6.4% | +1.1% | 1 / 1 |
| codec-nested | 2 | -48.4% | -2.5% | -0.2% | 1 / 1 |
| codec-nested | 4 | -46.1% | +3.3% | +1.3% | 1 / 1 |
| codec-nested | 8 | -49.9% | -1.4% | +0.4% | 1 / 1 |
| codec-wire | 1 | -47.7% | n/a | +0.9% | 0 / 0 |
| codec-wire | 2 | -46.5% | n/a | +1.1% | 0 / 0 |
| codec-wire | 4 | -47.1% | n/a | +2.9% | 0 / 0 |
| codec-wire | 8 | -47.3% | n/a | +1.4% | 0 / 0 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| codec-flat | 1 | 7 | -35.5% [-52.0%, -28.4%] | 7 / 7 |
| codec-flat | 2 | 7 | -34.2% [-35.7%, -27.7%] | 7 / 7 |
| codec-flat | 4 | 7 | -33.8% [-48.9%, -28.6%] | 7 / 7 |
| codec-flat | 8 | 7 | -31.6% [-38.0%, -14.5%] | 7 / 7 |
| codec-invalid | 1 | 7 | -23.9% [-32.0%, -12.8%] | 7 / 7 |
| codec-invalid | 2 | 7 | -24.7% [-32.1%, -19.2%] | 7 / 7 |
| codec-invalid | 4 | 7 | -23.3% [-32.9%, -4.8%] | 7 / 7 |
| codec-invalid | 8 | 7 | -22.2% [-25.2%, -19.6%] | 7 / 7 |
| codec-nested | 1 | 7 | -48.3% [-54.4%, -45.5%] | 7 / 7 |
| codec-nested | 2 | 7 | -49.9% [-56.4%, -46.2%] | 7 / 7 |
| codec-nested | 4 | 7 | -48.4% [-51.6%, -38.5%] | 7 / 7 |
| codec-nested | 8 | 7 | -50.1% [-54.1%, -48.0%] | 7 / 7 |
| codec-wire | 1 | 7 | -47.3% [-49.0%, -44.7%] | 7 / 7 |
| codec-wire | 2 | 7 | -47.4% [-52.9%, -42.4%] | 7 / 7 |
| codec-wire | 4 | 7 | -46.6% [-51.3%, -38.5%] | 7 / 7 |
| codec-wire | 8 | 7 | -47.2% [-57.3%, -38.6%] | 7 / 7 |

Diagnostic GC phases summed across measured repetitions (wall time):
