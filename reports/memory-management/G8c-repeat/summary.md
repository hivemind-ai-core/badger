# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/records | 1 | 3.808 / 3.868 | +1.6% | +1.0% [-8.2%, +12.5%] | 3/15 | -0.4% |
| compare/records | 8 | 3.863 / 3.812 | -1.3% | -0.5% [-10.3%, +11.6%] | 9/15 | -0.5% |
| compare/snapshots | 1 | 0.970 / 0.961 | -1.0% | -1.3% [-11.7%, +15.7%] | 9/15 | -1.3% |
| compare/snapshots | 8 | 0.986 / 0.945 | -4.2% | -2.9% [-18.2%, +27.7%] | 10/15 | -1.1% |
| realworld/json | 1 | 10.503 / 10.721 | +2.1% | +0.7% [-7.0%, +6.3%] | 4/15 | -0.5% |
| realworld/json | 8 | 10.849 / 11.119 | +2.5% | +2.5% [-6.3%, +11.5%] | 2/15 | -0.4% |
| realworld/state | 1 | 1.051 / 1.051 | -0.0% | -1.0% [-3.1%, +10.1%] | 8/15 | -1.8% |
| realworld/state | 8 | 1.045 / 1.051 | +0.6% | -0.1% [-5.2%, +12.7%] | 8/15 | -1.4% |

## Block means retain expensive iterations

One arithmetic mean of measured kernel iterations per process. This includes costly GC iterations but excludes fixture/I/O time as in the original contract. Inner iterations remain correlated.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/records | 1 | 5.124 / 5.254 | +2.5% | +2.1% [-4.5%, +5.9%] | 2/15 | -0.4% |
| compare/records | 8 | 5.153 / 5.172 | +0.4% | -0.4% [-3.8%, +6.2%] | 8/15 | -0.5% |
| compare/snapshots | 1 | 1.092 / 1.079 | -1.3% | -1.4% [-11.7%, +24.2%] | 10/15 | -1.3% |
| compare/snapshots | 8 | 1.095 / 1.086 | -0.7% | -1.4% [-8.9%, +7.8%] | 9/15 | -1.1% |
| realworld/json | 1 | 10.702 / 10.787 | +0.8% | +0.7% [-6.4%, +8.0%] | 6/15 | -0.5% |
| realworld/json | 8 | 10.980 / 11.298 | +2.9% | +2.2% [-6.3%, +11.1%] | 1/15 | -0.4% |
| realworld/state | 1 | 1.231 / 1.228 | -0.2% | +0.1% [-4.2%, +19.1%] | 7/15 | -1.8% |
| realworld/state | 8 | 1.234 / 1.224 | -0.8% | -1.3% [-11.5%, +25.4%] | 10/15 | -1.4% |
