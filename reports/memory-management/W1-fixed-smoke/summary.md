# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

## Single-variant baselines

| Suite/case | Workers | Variant | Kernel ms median [min, max] | Block mean ms | Peak RSS MiB | Processes |
|---|---:|---|---:|---:|---:|---:|
| realworld/messaging | 1 | baseline | 0.011 [0.011, 0.011] | 0.011 | 29.9 | 1 |
| realworld/messaging | 8 | baseline | 0.265 [0.265, 0.265] | 0.265 | 32.3 | 1 |
| realworld/pipeline | 1 | baseline | 0.121 [0.121, 0.121] | 0.121 | 29.4 | 1 |
| realworld/pipeline | 8 | baseline | 0.113 [0.113, 0.113] | 0.113 | 32.1 | 1 |

