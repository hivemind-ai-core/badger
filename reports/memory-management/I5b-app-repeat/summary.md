# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/batches | 1 | 0.638 / 0.618 | -3.2% | -0.5% [-10.2%, +27.1%] | 8/15 | +2.5% |
| compare/batches | 8 | 0.666 / 0.662 | -0.5% | -4.3% [-46.5%, +85.1%] | 10/15 | +2.8% |
| realworld/reconcile | 1 | 5.239 / 5.258 | +0.4% | -1.8% [-14.8%, +27.7%] | 9/15 | +1.4% |
| realworld/reconcile | 8 | 5.025 / 5.125 | +2.0% | +1.6% [-19.4%, +24.2%] | 5/15 | +0.9% |

## Block means retain expensive iterations

One arithmetic mean of measured kernel iterations per process. This includes costly GC iterations but excludes fixture/I/O time as in the original contract. Inner iterations remain correlated.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/batches | 1 | 0.831 / 0.824 | -0.8% | -0.8% [-17.7%, +8.1%] | 8/15 | +2.5% |
| compare/batches | 8 | 0.872 / 0.862 | -1.2% | -1.7% [-26.8%, +18.1%] | 9/15 | +2.8% |
| realworld/reconcile | 1 | 6.322 / 6.263 | -0.9% | +1.3% [-5.4%, +3.9%] | 6/15 | +1.4% |
| realworld/reconcile | 8 | 6.081 / 6.285 | +3.3% | +2.2% [-2.6%, +11.0%] | 5/15 | +0.9% |
