# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| realworld/messaging | 2 | 0.951 / 1.033 | +8.6% | +13.3% [-23.3%, +42.0%] | 2/15 | -3.2% |

## Block means retain expensive iterations

One arithmetic mean of measured kernel iterations per process. This includes costly GC iterations but excludes fixture/I/O time as in the original contract. Inner iterations remain correlated.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| realworld/messaging | 2 | 1.030 / 1.139 | +10.6% | +11.1% [-11.9%, +32.6%] | 2/15 | -3.2% |
