# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/records | 4 | 3.949 / 3.978 | +0.7% | +0.4% [-16.2%, +14.9%] | 6/15 | +0.0% |

## Block means retain expensive iterations

One arithmetic mean of measured kernel iterations per process. This includes costly GC iterations but excludes fixture/I/O time as in the original contract. Inner iterations remain correlated.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/records | 4 | 5.342 / 5.352 | +0.2% | -1.2% [-5.6%, +5.9%] | 9/15 | +0.0% |
