# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/records | 1 | 8.512 / 8.516 | +0.0% | +0.8% [-6.3%, +1.5%] | 1/3 | -0.5% |
| compare/records | 8 | 8.655 / 8.452 | -2.3% | -2.3% [-4.5%, +1.0%] | 2/3 | -0.8% |
| compare/snapshots | 1 | 1.201 / 1.210 | +0.7% | +1.6% [-1.1%, +7.9%] | 1/3 | -2.0% |
| compare/snapshots | 8 | 1.239 / 1.174 | -5.2% | -3.5% [-5.2%, -2.6%] | 3/3 | -2.9% |
| realworld/reconcile | 1 | 6.655 / 6.331 | -4.9% | -4.9% [-22.3%, +3.0%] | 2/3 | -1.0% |
| realworld/reconcile | 8 | 6.424 / 6.323 | -1.6% | -1.6% [-3.4%, -1.3%] | 3/3 | -2.8% |
| realworld/state | 1 | 1.998 / 2.020 | +1.1% | +1.1% [-2.6%, +2.4%] | 1/3 | -1.2% |
| realworld/state | 8 | 2.028 / 1.989 | -1.9% | -2.3% [-2.8%, -1.9%] | 3/3 | -2.2% |

## Block means retain expensive iterations

One arithmetic mean of measured kernel iterations per process. This includes costly GC iterations but excludes fixture/I/O time as in the original contract. Inner iterations remain correlated.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/records | 1 | 8.117 / 8.013 | -1.3% | -1.3% [-2.9%, +0.8%] | 2/3 | -0.5% |
| compare/records | 8 | 8.036 / 7.953 | -1.0% | -1.0% [-2.1%, +1.3%] | 2/3 | -0.8% |
| compare/snapshots | 1 | 1.212 / 1.197 | -1.2% | +0.9% [-3.5%, +13.1%] | 1/3 | -2.0% |
| compare/snapshots | 8 | 1.238 / 1.180 | -4.6% | -4.6% [-5.1%, -4.2%] | 3/3 | -2.9% |
| realworld/reconcile | 1 | 7.815 / 7.392 | -5.4% | -3.4% [-7.8%, -0.3%] | 3/3 | -1.0% |
| realworld/reconcile | 8 | 7.502 / 7.340 | -2.2% | -2.3% [-2.4%, -0.3%] | 3/3 | -2.8% |
| realworld/state | 1 | 1.828 / 1.788 | -2.2% | -2.2% [-6.5%, +2.2%] | 2/3 | -1.2% |
| realworld/state | 8 | 1.871 / 1.788 | -4.4% | -2.3% [-4.4%, -1.8%] | 3/3 | -2.2% |
