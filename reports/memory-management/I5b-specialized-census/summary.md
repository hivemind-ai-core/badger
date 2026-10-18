# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/snapshots | 1 | 5.191 / 1.230 | -76.3% | -76.3% [-76.3%, -76.3%] | 1/1 | -16.1% |
| compare/snapshots | 8 | 6.722 / 1.207 | -82.0% | -82.0% [-82.0%, -82.0%] | 1/1 | -18.5% |
| realworld/reconcile | 1 | 6.530 / 6.558 | +0.4% | +0.4% [+0.4%, +0.4%] | 0/1 | +3.1% |
| realworld/reconcile | 8 | 7.385 / 7.209 | -2.4% | -2.4% [-2.4%, -2.4%] | 1/1 | -3.5% |
| realworld/state | 1 | 5.266 / 2.079 | -60.5% | -60.5% [-60.5%, -60.5%] | 1/1 | -5.2% |
| realworld/state | 8 | 5.615 / 2.105 | -62.5% | -62.5% [-62.5%, -62.5%] | 1/1 | -4.7% |

## Block means retain expensive iterations

One arithmetic mean of measured kernel iterations per process. This includes costly GC iterations but excludes fixture/I/O time as in the original contract. Inner iterations remain correlated.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/snapshots | 1 | 5.051 / 1.229 | -75.7% | -75.7% [-75.7%, -75.7%] | 1/1 | -16.1% |
| compare/snapshots | 8 | 6.396 / 1.193 | -81.4% | -81.4% [-81.4%, -81.4%] | 1/1 | -18.5% |
| realworld/reconcile | 1 | 7.730 / 7.642 | -1.1% | -1.1% [-1.1%, -1.1%] | 1/1 | +3.1% |
| realworld/reconcile | 8 | 8.098 / 7.877 | -2.7% | -2.7% [-2.7%, -2.7%] | 1/1 | -3.5% |
| realworld/state | 1 | 5.416 / 1.954 | -63.9% | -63.9% [-63.9%, -63.9%] | 1/1 | -5.2% |
| realworld/state | 8 | 5.558 / 1.908 | -65.7% | -65.7% [-65.7%, -65.7%] | 1/1 | -4.7% |
