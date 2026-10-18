# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/records | 1 | 3.990 / 3.905 | -2.1% | -3.0% [-8.2%, +4.6%] | 12/15 | +0.0% |
| compare/records | 8 | 3.855 / 3.893 | +1.0% | +1.6% [-8.2%, +8.1%] | 7/15 | -0.2% |
| compare/snapshots | 1 | 0.949 / 0.973 | +2.4% | +3.5% [-13.5%, +27.5%] | 3/15 | +0.3% |
| compare/snapshots | 8 | 0.979 / 0.956 | -2.3% | -0.7% [-10.0%, +18.3%] | 10/15 | +0.2% |
| realworld/json | 1 | 10.649 / 10.650 | +0.0% | +0.7% [-10.5%, +7.0%] | 7/15 | -0.0% |
| realworld/json | 8 | 11.150 / 11.154 | +0.0% | +0.6% [-4.5%, +4.1%] | 5/15 | +0.3% |
| realworld/state | 1 | 1.043 / 1.042 | -0.1% | +0.0% [-6.4%, +3.6%] | 7/15 | +0.1% |
| realworld/state | 8 | 1.045 / 1.039 | -0.5% | +0.2% [-3.7%, +10.0%] | 6/15 | +0.4% |

## Block means retain expensive iterations

One arithmetic mean of measured kernel iterations per process. This includes costly GC iterations but excludes fixture/I/O time as in the original contract. Inner iterations remain correlated.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/records | 1 | 5.265 / 5.220 | -0.8% | -1.9% [-6.8%, +3.7%] | 9/15 | +0.0% |
| compare/records | 8 | 5.172 / 5.183 | +0.2% | -0.0% [-5.3%, +6.6%] | 8/15 | -0.2% |
| compare/snapshots | 1 | 1.075 / 1.111 | +3.3% | +0.9% [-12.1%, +18.8%] | 5/15 | +0.3% |
| compare/snapshots | 8 | 1.078 / 1.088 | +1.0% | +1.0% [-5.8%, +4.3%] | 6/15 | +0.2% |
| realworld/json | 1 | 10.827 / 10.746 | -0.7% | -0.5% [-11.7%, +6.0%] | 8/15 | -0.0% |
| realworld/json | 8 | 11.317 / 11.338 | +0.2% | +0.1% [-4.2%, +7.2%] | 7/15 | +0.3% |
| realworld/state | 1 | 1.217 / 1.210 | -0.6% | -0.2% [-12.9%, +2.7%] | 10/15 | +0.1% |
| realworld/state | 8 | 1.208 / 1.233 | +2.1% | +3.5% [-8.3%, +12.5%] | 7/15 | +0.4% |
