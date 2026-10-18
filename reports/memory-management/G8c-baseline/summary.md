# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

## Single-variant baselines

| Suite/case | Workers | Variant | Kernel ms median [min, max] | Block mean ms | Peak RSS MiB | Processes |
|---|---:|---|---:|---:|---:|---:|
| compare/batches | 1 | baseline | 0.557 [0.552, 0.573] | 0.639 | 38.6 | 3 |
| compare/batches | 8 | baseline | 0.577 [0.565, 0.581] | 0.620 | 39.3 | 3 |
| compare/records | 1 | baseline | 3.983 [3.833, 5.931] | 5.213 | 117.8 | 3 |
| compare/records | 8 | baseline | 3.832 [3.746, 3.865] | 5.107 | 119.9 | 3 |
| compare/snapshots | 1 | baseline | 0.932 [0.928, 1.041] | 1.038 | 44.7 | 3 |
| compare/snapshots | 8 | baseline | 1.018 [0.943, 1.082] | 1.082 | 45.2 | 3 |
| realworld/json | 1 | baseline | 10.510 [10.200, 10.534] | 10.448 | 59.0 | 3 |
| realworld/json | 8 | baseline | 10.496 [10.394, 10.733] | 10.689 | 60.9 | 3 |
| realworld/reconcile | 1 | baseline | 4.985 [4.896, 5.055] | 5.790 | 79.7 | 3 |
| realworld/reconcile | 8 | baseline | 5.010 [4.943, 5.534] | 5.842 | 79.4 | 3 |
| realworld/state | 1 | baseline | 1.037 [1.033, 1.054] | 1.226 | 39.4 | 3 |
| realworld/state | 8 | baseline | 1.055 [1.020, 1.083] | 1.244 | 39.5 | 3 |
