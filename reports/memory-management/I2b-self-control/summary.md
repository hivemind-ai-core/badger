| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| entries-immediate | 1 | baseline | 29.747 [29.436, 32.124] | 1.031 | 47.3 |
| entries-immediate | 1 | candidate | 30.627 [29.859, 33.125] | 1.012 | 46.7 |
| entries-immediate | 8 | baseline | 35.864 [35.446, 36.075] | 1.021 | 47.1 |
| entries-immediate | 8 | candidate | 35.593 [35.363, 37.273] | 1.011 | 47.1 |
| entries-stored | 1 | baseline | 30.014 [29.645, 31.063] | 1.009 | 46.7 |
| entries-stored | 1 | candidate | 31.271 [29.878, 32.609] | 1.044 | 46.5 |
| entries-stored | 8 | baseline | 35.935 [35.716, 41.620] | 1.082 | 46.5 |
| entries-stored | 8 | candidate | 35.972 [35.697, 39.179] | 1.053 | 47.2 |

Ordinary candidate versus baseline (negative time/RSS change is better):

| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |
|---|---:|---:|---:|---:|---:|
| entries-immediate | 1 | +3.0% | -1.9% | -1.2% | 1 / 1 |
| entries-immediate | 8 | -0.8% | -1.0% | +0.1% | 1 / 1 |
| entries-stored | 1 | +4.2% | +3.5% | -0.4% | 1 / 1 |
| entries-stored | 8 | +0.1% | -2.6% | +1.5% | 1 / 1 |

Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.
Pairs reduce time separation; these counts are not a significance test.

| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |
|---|---:|---:|---:|---:|
| entries-immediate | 1 | 5 | +1.4% [-5.5%, +11.6%] | 2 / 5 |
| entries-immediate | 8 | 5 | -0.8% [-2.0%, +4.7%] | 3 / 5 |
| entries-stored | 1 | 5 | +5.3% [-3.7%, +8.2%] | 1 / 5 |
| entries-stored | 8 | 5 | -0.1% [-11.1%, +9.0%] | 4 / 5 |

Diagnostic GC phases summed across measured repetitions (wall time):
