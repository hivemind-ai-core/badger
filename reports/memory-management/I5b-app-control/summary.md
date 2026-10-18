# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/batches | 1 | 0.634 / 0.630 | -0.7% | -2.8% [-31.9%, +23.4%] | 8/15 | +0.1% |
| compare/batches | 8 | 0.629 / 0.653 | +3.8% | +0.0% [-7.3%, +18.0%] | 7/15 | +0.2% |
| realworld/reconcile | 1 | 5.335 / 4.950 | -7.2% | -4.2% [-26.5%, +24.5%] | 11/15 | -0.5% |
| realworld/reconcile | 8 | 4.935 / 4.818 | -2.4% | +0.2% [-24.3%, +33.4%] | 7/15 | +0.5% |

## Block means retain expensive iterations

One arithmetic mean of measured kernel iterations per process. This includes costly GC iterations but excludes fixture/I/O time as in the original contract. Inner iterations remain correlated.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/batches | 1 | 0.854 / 0.833 | -2.5% | -1.2% [-13.8%, +5.4%] | 10/15 | +0.1% |
| compare/batches | 8 | 0.861 / 0.843 | -2.2% | -2.0% [-24.7%, +6.8%] | 11/15 | +0.2% |
| realworld/reconcile | 1 | 6.193 / 6.101 | -1.5% | -1.2% [-10.9%, +10.3%] | 12/15 | -0.5% |
| realworld/reconcile | 8 | 6.108 / 6.237 | +2.1% | +0.7% [-5.7%, +8.2%] | 4/15 | +0.5% |
