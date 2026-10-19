# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/batches | 1 | 0.582 / 0.589 | +1.1% | +1.6% [-2.7%, +6.7%] | 5/15 | +0.8% |
| compare/batches | 2 | 0.583 / 0.586 | +0.5% | +0.6% [-1.8%, +4.8%] | 4/15 | +45.2% |
| realworld/cpu | 1 | 48.837 / 48.995 | +0.3% | +0.1% [-0.4%, +1.2%] | 5/15 | +1.1% |
| realworld/cpu | 2 | 48.978 / 48.957 | -0.0% | -0.0% [-0.6%, +0.2%] | 8/15 | +1.1% |
| realworld/json | 1 | 4.815 / 4.871 | +1.2% | +1.5% [-4.7%, +4.7%] | 3/15 | +0.4% |
| realworld/json | 2 | 5.240 / 5.185 | -1.0% | -1.1% [-3.1%, +2.6%] | 10/15 | +0.6% |
| realworld/messaging | 1 | 0.994 / 0.924 | -7.0% | -7.3% [-10.4%, -2.4%] | 15/15 | +1.0% |
| realworld/messaging | 2 | 0.738 / 0.621 | -15.8% | -14.7% [-36.5%, +3.4%] | 14/15 | +10.1% |

## Block means retain expensive iterations

One arithmetic mean of measured kernel iterations per process. This includes costly GC iterations but excludes fixture/I/O time as in the original contract. Inner iterations remain correlated.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/batches | 1 | 0.640 / 0.659 | +3.0% | +2.7% [-2.7%, +7.0%] | 4/15 | +0.8% |
| compare/batches | 2 | 0.655 / 0.787 | +20.1% | +4.2% [-7.1%, +31.4%] | 5/15 | +45.2% |
| realworld/cpu | 1 | 49.060 / 49.189 | +0.3% | +0.3% [-0.6%, +1.8%] | 3/15 | +1.1% |
| realworld/cpu | 2 | 49.240 / 49.245 | +0.0% | +0.0% [-0.7%, +0.7%] | 6/15 | +1.1% |
| realworld/json | 1 | 5.228 / 5.307 | +1.5% | +1.5% [-2.1%, +8.2%] | 4/15 | +0.4% |
| realworld/json | 2 | 5.680 / 5.670 | -0.2% | -0.4% [-4.1%, +2.9%] | 8/15 | +0.6% |
| realworld/messaging | 1 | 1.129 / 1.045 | -7.4% | -7.6% [-14.3%, -0.8%] | 15/15 | +1.0% |
| realworld/messaging | 2 | 0.817 / 0.736 | -9.9% | -9.2% [-28.2%, +0.5%] | 13/15 | +10.1% |
