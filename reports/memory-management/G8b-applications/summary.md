# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

## Single-variant baselines

| Suite/case | Workers | Variant | Kernel ms median [min, max] | Block mean ms | Peak RSS MiB | Processes |
|---|---:|---|---:|---:|---:|---:|
| compare/records | 1 | baseline | 8.694 [8.334, 8.778] | 8.376 | 83.6 | 3 |
| compare/records | 8 | baseline | 8.822 [8.545, 9.044] | 8.132 | 83.8 | 3 |
| compare/snapshots | 1 | baseline | 1.185 [1.182, 1.186] | 1.191 | 40.3 | 3 |
| compare/snapshots | 8 | baseline | 1.187 [1.184, 1.213] | 1.201 | 40.5 | 3 |
| realworld/messaging | 1 | baseline | 1.006 [0.998, 1.018] | 1.028 | 32.2 | 3 |
| realworld/messaging | 8 | baseline | 1.362 [1.323, 1.375] | 1.417 | 37.1 | 3 |
| realworld/reconcile | 1 | baseline | 6.349 [6.341, 6.577] | 7.495 | 58.9 | 3 |
| realworld/reconcile | 8 | baseline | 6.399 [6.344, 8.066] | 7.524 | 58.7 | 3 |
| realworld/state | 1 | baseline | 2.059 [2.030, 2.080] | 1.853 | 38.6 | 3 |
| realworld/state | 8 | baseline | 2.106 [1.976, 2.183] | 1.854 | 39.9 | 3 |

