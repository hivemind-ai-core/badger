# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/records | 1 | 3.751 / 3.808 | +1.5% | +1.2% [-9.3%, +18.1%] | 4/15 | -0.1% |
| compare/records | 8 | 3.785 / 3.779 | -0.2% | +1.1% [-9.7%, +17.9%] | 6/15 | +0.0% |
| realworld/reconcile | 1 | 4.936 / 5.004 | +1.4% | +0.2% [-20.6%, +18.7%] | 7/15 | -0.5% |
| realworld/reconcile | 8 | 4.956 / 4.960 | +0.1% | -0.5% [-11.4%, +16.6%] | 9/15 | +0.7% |

## Block means retain expensive iterations

One arithmetic mean of measured kernel iterations per process. This includes costly GC iterations but excludes fixture/I/O time as in the original contract. Inner iterations remain correlated.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/records | 1 | 5.052 / 5.125 | +1.4% | +1.1% [-3.5%, +8.6%] | 6/15 | -0.1% |
| compare/records | 8 | 5.104 / 5.239 | +2.6% | -0.5% [-4.6%, +8.7%] | 8/15 | +0.0% |
| realworld/reconcile | 1 | 5.800 / 5.849 | +0.8% | +0.9% [-8.6%, +6.1%] | 6/15 | -0.5% |
| realworld/reconcile | 8 | 5.804 / 5.791 | -0.2% | -0.5% [-4.8%, +4.0%] | 9/15 | +0.7% |
