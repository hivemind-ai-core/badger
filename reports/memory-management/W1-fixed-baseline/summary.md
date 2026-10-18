# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

## Single-variant baselines

| Suite/case | Workers | Variant | Kernel ms median [min, max] | Block mean ms | Peak RSS MiB | Processes |
|---|---:|---|---:|---:|---:|---:|
| realworld/messaging | 1 | baseline | 1.323 [1.303, 1.500] | 1.345 | 39.8 | 7 |
| realworld/messaging | 2 | baseline | 0.861 [0.852, 1.302] | 1.078 | 44.5 | 7 |
| realworld/messaging | 4 | baseline | 1.253 [1.180, 1.386] | 1.383 | 52.0 | 7 |
| realworld/messaging | 8 | baseline | 1.420 [1.348, 1.579] | 1.467 | 52.9 | 7 |
| realworld/pipeline | 1 | baseline | 50.104 [49.814, 51.548] | 50.710 | 39.8 | 7 |
| realworld/pipeline | 2 | baseline | 25.338 [25.283, 25.832] | 25.559 | 44.4 | 7 |
| realworld/pipeline | 4 | baseline | 13.045 [12.807, 13.125] | 13.151 | 51.3 | 7 |
| realworld/pipeline | 8 | baseline | 7.454 [7.229, 7.624] | 7.529 | 48.1 | 7 |

