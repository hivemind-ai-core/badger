# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| realworld/messaging | 2 | 1.315 / 1.280 | -2.7% | -3.2% [-9.2%, +9.4%] | 12/15 | -6.1% |

## Block means retain expensive iterations

One arithmetic mean of measured kernel iterations per process. This includes costly GC iterations but excludes fixture/I/O time as in the original contract. Inner iterations remain correlated.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| realworld/messaging | 2 | 1.381 / 1.342 | -2.8% | -2.8% [-14.3%, +9.6%] | 12/15 | -6.1% |
