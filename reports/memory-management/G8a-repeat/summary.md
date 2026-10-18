# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/records | 1 | 3.836 / 3.803 | -0.9% | -0.6% [-9.9%, +16.4%] | 9/15 | +0.2% |
| compare/records | 8 | 3.786 / 3.858 | +1.9% | +0.7% [-4.8%, +17.1%] | 6/15 | -0.1% |
| realworld/reconcile | 1 | 4.927 / 4.966 | +0.8% | -1.7% [-12.1%, +14.5%] | 10/15 | -0.4% |
| realworld/reconcile | 8 | 4.974 / 4.987 | +0.3% | +0.5% [-14.2%, +15.1%] | 6/15 | -0.2% |

## Block means retain expensive iterations

One arithmetic mean of measured kernel iterations per process. This includes costly GC iterations but excludes fixture/I/O time as in the original contract. Inner iterations remain correlated.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/records | 1 | 5.086 / 5.128 | +0.8% | +0.8% [-6.9%, +6.1%] | 7/15 | +0.2% |
| compare/records | 8 | 5.096 / 5.154 | +1.1% | +1.6% [-3.0%, +6.4%] | 5/15 | -0.1% |
| realworld/reconcile | 1 | 5.887 / 5.773 | -1.9% | -1.4% [-4.3%, +2.6%] | 12/15 | -0.4% |
| realworld/reconcile | 8 | 5.878 / 5.855 | -0.4% | -0.4% [-10.1%, +4.6%] | 10/15 | -0.2% |
