# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| realworld/json | 4 | 10.904 / 10.888 | -0.1% | -0.3% [-4.6%, +6.6%] | 8/15 | +0.1% |
| realworld/json | 8 | 10.912 / 10.843 | -0.6% | -0.3% [-3.5%, +2.3%] | 8/15 | +0.2% |
| realworld/messaging | 4 | 1.136 / 1.167 | +2.8% | -0.9% [-15.2%, +12.5%] | 10/15 | -0.2% |
| realworld/messaging | 8 | 1.402 / 1.357 | -3.2% | -2.8% [-16.8%, +14.8%] | 9/15 | +1.2% |
| realworld/state | 4 | 1.045 / 1.050 | +0.5% | +0.9% [-27.3%, +8.9%] | 5/15 | -0.2% |
| realworld/state | 8 | 1.067 / 1.062 | -0.5% | -1.3% [-7.9%, +13.5%] | 8/15 | -0.3% |

## Block means retain expensive iterations

One arithmetic mean of measured kernel iterations per process. This includes costly GC iterations but excludes fixture/I/O time as in the original contract. Inner iterations remain correlated.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| realworld/json | 4 | 10.977 / 10.932 | -0.4% | +0.3% [-4.2%, +12.6%] | 7/15 | +0.1% |
| realworld/json | 8 | 11.000 / 10.922 | -0.7% | -1.0% [-4.5%, +2.7%] | 9/15 | +0.2% |
| realworld/messaging | 4 | 1.276 / 1.261 | -1.2% | -2.4% [-15.0%, +16.6%] | 9/15 | -0.2% |
| realworld/messaging | 8 | 1.486 / 1.434 | -3.5% | -3.2% [-8.3%, +15.6%] | 9/15 | +1.2% |
| realworld/state | 4 | 1.240 / 1.233 | -0.5% | -1.2% [-24.4%, +15.5%] | 9/15 | -0.2% |
| realworld/state | 8 | 1.257 / 1.231 | -2.1% | -2.1% [-13.0%, +15.1%] | 9/15 | -0.3% |
