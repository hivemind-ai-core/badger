# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/records | 1 | 3.827 / 3.891 | +1.7% | +2.1% [-8.1%, +3.3%] | 2/7 | +0.0% |
| compare/records | 8 | 3.855 / 3.783 | -1.9% | -1.2% [-8.4%, +7.9%] | 4/7 | +0.1% |
| realworld/json | 1 | 10.533 / 10.317 | -2.1% | -1.2% [-8.2%, +7.6%] | 4/7 | -1.2% |
| realworld/json | 8 | 10.862 / 10.916 | +0.5% | +1.0% [-5.1%, +4.7%] | 3/7 | -0.6% |
| realworld/reconcile | 1 | 4.939 / 5.001 | +1.3% | -1.7% [-13.4%, +15.3%] | 4/7 | +0.7% |
| realworld/reconcile | 8 | 5.027 / 4.930 | -1.9% | -1.7% [-10.5%, +13.2%] | 5/7 | +0.0% |
| realworld/state | 1 | 1.043 / 1.043 | -0.0% | +1.0% [-3.5%, +2.8%] | 3/7 | -0.2% |
| realworld/state | 8 | 1.047 / 1.059 | +1.2% | +1.2% [-4.1%, +2.7%] | 3/7 | +0.2% |

## Block means retain expensive iterations

One arithmetic mean of measured kernel iterations per process. This includes costly GC iterations but excludes fixture/I/O time as in the original contract. Inner iterations remain correlated.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/records | 1 | 5.112 / 5.058 | -1.1% | -0.4% [-8.5%, +4.0%] | 4/7 | +0.0% |
| compare/records | 8 | 5.063 / 5.107 | +0.9% | +0.0% [-7.9%, +1.8%] | 3/7 | +0.1% |
| realworld/json | 1 | 10.594 / 10.499 | -0.9% | +0.1% [-9.2%, +7.1%] | 3/7 | -1.2% |
| realworld/json | 8 | 11.146 / 11.015 | -1.2% | -1.2% [-6.1%, +4.1%] | 4/7 | -0.6% |
| realworld/reconcile | 1 | 5.954 / 5.810 | -2.4% | -2.8% [-5.2%, +2.1%] | 5/7 | +0.7% |
| realworld/reconcile | 8 | 5.956 / 5.844 | -1.9% | -0.6% [-5.1%, +7.4%] | 4/7 | +0.0% |
| realworld/state | 1 | 1.232 / 1.227 | -0.3% | +0.9% [-4.5%, +4.0%] | 3/7 | -0.2% |
| realworld/state | 8 | 1.231 / 1.235 | +0.3% | +0.3% [-5.6%, +7.7%] | 3/7 | +0.2% |
