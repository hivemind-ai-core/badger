# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/records | 4 | 3.991 / 4.052 | +1.5% | +0.3% [-20.6%, +22.7%] | 7/15 | +0.0% |

## Block means retain expensive iterations

One arithmetic mean of measured kernel iterations per process. This includes costly GC iterations but excludes fixture/I/O time as in the original contract. Inner iterations remain correlated.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/records | 4 | 5.410 / 5.351 | -1.1% | -0.7% [-9.9%, +5.6%] | 11/15 | +0.0% |
