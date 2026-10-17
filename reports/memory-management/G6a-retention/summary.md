| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| tasks-input | 1 | baseline | 97.728 [93.549, 193.525] | 22.930 | 58.5 |
| tasks-input | 1 | candidate | 103.275 [96.968, 244.623] | 29.030 | 51.4 |
| tasks-input | 2 | baseline | 95.898 [92.560, 337.798] | 21.630 | 58.6 |
| tasks-input | 2 | candidate | 102.703 [96.253, 194.276] | 26.620 | 58.4 |
| tasks-input | 4 | baseline | 101.303 [95.660, 232.370] | 25.621 | 61.0 |
| tasks-input | 4 | candidate | 101.603 [97.265, 411.142] | 27.108 | 60.7 |
| tasks-input | 8 | baseline | 98.815 [96.732, 258.720] | 23.081 | 62.6 |
| tasks-input | 8 | candidate | 110.824 [101.888, 258.932] | 33.318 | 61.2 |
| tasks-result | 1 | baseline | 104.819 [94.210, 118.946] | 26.895 | 58.7 |
| tasks-result | 1 | candidate | 101.711 [96.834, 117.431] | 24.341 | 59.0 |
| tasks-result | 2 | baseline | 105.369 [99.697, 231.006] | 25.928 | 59.2 |
| tasks-result | 2 | candidate | 105.546 [96.582, 339.993] | 25.721 | 59.0 |
| tasks-result | 4 | baseline | 104.078 [97.176, 269.438] | 26.559 | 59.7 |
| tasks-result | 4 | candidate | 111.331 [99.749, 259.428] | 28.526 | 60.4 |
| tasks-result | 8 | baseline | 106.857 [96.904, 215.354] | 27.618 | 61.3 |
| tasks-result | 8 | candidate | 105.599 [98.682, 224.200] | 25.997 | 61.7 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| tasks-input | 1 | +5.7% | +26.6% | -12.1% | 15 / 15 |
| tasks-input | 2 | +7.1% | +23.1% | -0.5% | 15 / 15 |
| tasks-input | 4 | +0.3% | +5.8% | -0.4% | 15 / 15 |
| tasks-input | 8 | +12.2% | +44.3% | -2.2% | 15 / 15 |
| tasks-result | 1 | -3.0% | -9.5% | +0.6% | 15 / 15 |
| tasks-result | 2 | +0.2% | -0.8% | -0.3% | 15 / 15 |
| tasks-result | 4 | +7.0% | +7.4% | +1.1% | 15 / 15 |
| tasks-result | 8 | -1.2% | -5.9% | +0.7% | 15 / 15 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| tasks-input | 1 | 7 | +3.7% [-1.0%, +26.4%] | 1 / 7 |
| tasks-input | 2 | 7 | +7.0% [-42.5%, +21.8%] | 1 / 7 |
| tasks-input | 4 | 7 | +2.4% [-15.8%, +76.9%] | 3 / 7 |
| tasks-input | 8 | 7 | +10.9% [+0.1%, +18.1%] | 0 / 7 |
| tasks-result | 1 | 7 | -1.3% [-6.6%, +4.2%] | 6 / 7 |
| tasks-result | 2 | 7 | +4.1% [-8.3%, +47.2%] | 3 / 7 |
| tasks-result | 4 | 7 | +3.6% [-3.7%, +8.3%] | 2 / 7 |
| tasks-result | 8 | 7 | -1.0% [-4.5%, +7.0%] | 4 / 7 |

Lifetime checkpoints: pool reservation is distinct from live objects and process peak RSS.
Idle yields without allocating intentionally or forcing a collection.

| Case | Workers | Variant | Stage | Pool MiB median [min, max] | Managed live MiB median | Native external MiB median | Collection count median |
|---|---:|---|---|---:|---:|---:|---:|
| tasks-input | 1 | baseline | after churn | 16.25 [16.25, 16.25] | 0.03 | 16.00 | 15 |
| tasks-input | 1 | baseline | idle | 16.25 [16.25, 16.25] | 0.03 | 16.00 | 15 |
| tasks-input | 1 | baseline | tasks done | 0.25 [0.25, 0.25] | 0.01 | 16.00 | 1 |
| tasks-input | 1 | candidate | after churn | 16.25 [16.25, 16.25] | 0.02 | 0.00 | 15 |
| tasks-input | 1 | candidate | idle | 16.25 [16.25, 16.25] | 0.02 | 0.00 | 15 |
| tasks-input | 1 | candidate | tasks done | 0.25 [0.25, 0.25] | 0.00 | 0.50 | 1 |
| tasks-input | 2 | baseline | after churn | 16.50 [16.25, 16.50] | 0.03 | 16.00 | 15 |
| tasks-input | 2 | baseline | idle | 16.50 [16.25, 16.50] | 0.03 | 16.00 | 15 |
| tasks-input | 2 | baseline | tasks done | 0.50 [0.25, 0.50] | 0.01 | 16.00 | 1 |
| tasks-input | 2 | candidate | after churn | 16.25 [16.25, 16.50] | 0.02 | 0.00 | 15 |
| tasks-input | 2 | candidate | idle | 16.25 [16.25, 16.50] | 0.02 | 0.00 | 15 |
| tasks-input | 2 | candidate | tasks done | 0.50 [0.25, 0.50] | 0.00 | 0.25 | 1 |
| tasks-input | 4 | baseline | after churn | 16.75 [16.25, 17.00] | 0.03 | 16.00 | 15 |
| tasks-input | 4 | baseline | idle | 16.75 [16.25, 17.00] | 0.03 | 16.00 | 15 |
| tasks-input | 4 | baseline | tasks done | 0.75 [0.25, 1.00] | 0.01 | 16.00 | 1 |
| tasks-input | 4 | candidate | after churn | 16.50 [16.50, 16.50] | 0.02 | 0.00 | 15 |
| tasks-input | 4 | candidate | idle | 16.50 [16.50, 16.50] | 0.02 | 0.00 | 15 |
| tasks-input | 4 | candidate | tasks done | 0.50 [0.25, 1.00] | 0.00 | 0.25 | 1 |
| tasks-input | 8 | baseline | after churn | 17.00 [17.00, 17.25] | 0.03 | 16.00 | 15 |
| tasks-input | 8 | baseline | idle | 17.00 [17.00, 17.25] | 0.03 | 16.00 | 15 |
| tasks-input | 8 | baseline | tasks done | 1.00 [1.00, 1.25] | 0.01 | 16.00 | 1 |
| tasks-input | 8 | candidate | after churn | 16.50 [16.25, 16.50] | 0.02 | 0.00 | 15 |
| tasks-input | 8 | candidate | idle | 16.50 [16.25, 16.50] | 0.02 | 0.00 | 15 |
| tasks-input | 8 | candidate | tasks done | 0.75 [0.50, 1.50] | 0.00 | 0.25 | 1 |
| tasks-result | 1 | baseline | after churn | 16.25 [16.25, 16.25] | 0.03 | 16.00 | 15 |
| tasks-result | 1 | baseline | idle | 16.25 [16.25, 16.25] | 0.03 | 16.00 | 15 |
| tasks-result | 1 | baseline | tasks done | 0.25 [0.25, 0.25] | 0.01 | 16.00 | 1 |
| tasks-result | 1 | candidate | after churn | 16.25 [16.25, 16.25] | 0.03 | 16.00 | 15 |
| tasks-result | 1 | candidate | idle | 16.25 [16.25, 16.25] | 0.03 | 16.00 | 15 |
| tasks-result | 1 | candidate | tasks done | 0.25 [0.25, 0.25] | 0.01 | 16.00 | 1 |
| tasks-result | 2 | baseline | after churn | 16.50 [16.25, 16.50] | 0.03 | 16.00 | 15 |
| tasks-result | 2 | baseline | idle | 16.50 [16.25, 16.50] | 0.03 | 16.00 | 15 |
| tasks-result | 2 | baseline | tasks done | 0.50 [0.25, 0.50] | 0.01 | 16.00 | 1 |
| tasks-result | 2 | candidate | after churn | 16.50 [16.25, 16.50] | 0.03 | 16.00 | 15 |
| tasks-result | 2 | candidate | idle | 16.50 [16.25, 16.50] | 0.03 | 16.00 | 15 |
| tasks-result | 2 | candidate | tasks done | 0.50 [0.25, 0.50] | 0.01 | 16.00 | 1 |
| tasks-result | 4 | baseline | after churn | 16.50 [16.25, 16.75] | 0.03 | 16.00 | 15 |
| tasks-result | 4 | baseline | idle | 16.50 [16.25, 16.75] | 0.03 | 16.00 | 15 |
| tasks-result | 4 | baseline | tasks done | 0.25 [0.25, 0.75] | 0.01 | 16.00 | 1 |
| tasks-result | 4 | candidate | after churn | 16.50 [16.25, 16.75] | 0.03 | 16.00 | 15 |
| tasks-result | 4 | candidate | idle | 16.50 [16.25, 16.75] | 0.03 | 16.00 | 15 |
| tasks-result | 4 | candidate | tasks done | 0.50 [0.25, 0.75] | 0.01 | 16.00 | 1 |
| tasks-result | 8 | baseline | after churn | 16.50 [16.25, 17.00] | 0.03 | 16.00 | 15 |
| tasks-result | 8 | baseline | idle | 16.50 [16.25, 17.00] | 0.03 | 16.00 | 15 |
| tasks-result | 8 | baseline | tasks done | 0.50 [0.25, 1.00] | 0.01 | 16.00 | 1 |
| tasks-result | 8 | candidate | after churn | 16.75 [16.25, 17.25] | 0.03 | 16.00 | 15 |
| tasks-result | 8 | candidate | idle | 16.75 [16.25, 17.25] | 0.03 | 16.00 | 15 |
| tasks-result | 8 | candidate | tasks done | 0.75 [0.25, 1.25] | 0.01 | 16.00 | 1 |

Diagnostic GC phases summed across measured repetitions (wall time):
