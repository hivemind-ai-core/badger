# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/batches | 2 | 0.660 / 0.677 | +2.6% | -1.9% [-46.3%, +70.7%] | 8/15 | -1.8% |
| compare/batches | 8 | 0.626 / 0.642 | +2.6% | +1.7% [-40.5%, +56.7%] | 5/15 | +0.7% |
| realworld/json | 2 | 18.602 / 18.351 | -1.3% | -1.9% [-5.9%, +5.5%] | 8/15 | -0.1% |
| realworld/json | 8 | 18.295 / 18.322 | +0.1% | -0.4% [-19.8%, +2.6%] | 11/15 | -0.2% |
| realworld/messaging | 2 | 1.046 / 1.054 | +0.7% | +0.1% [-25.0%, +29.7%] | 7/15 | +0.8% |
| realworld/messaging | 8 | 1.378 / 1.347 | -2.2% | +1.0% [-9.4%, +10.3%] | 7/15 | +0.2% |

## Block means retain expensive iterations

One arithmetic mean of measured kernel iterations per process. This includes costly GC iterations but excludes fixture/I/O time as in the original contract. Inner iterations remain correlated.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/batches | 2 | 0.861 / 0.863 | +0.3% | +0.9% [-30.4%, +30.2%] | 7/15 | -1.8% |
| compare/batches | 8 | 0.897 / 0.848 | -5.4% | +0.6% [-20.5%, +23.8%] | 7/15 | +0.7% |
| realworld/json | 2 | 18.771 / 18.553 | -1.2% | +0.4% [-6.8%, +4.5%] | 7/15 | -0.1% |
| realworld/json | 8 | 18.591 / 18.412 | -1.0% | -1.0% [-20.0%, +4.7%] | 10/15 | -0.2% |
| realworld/messaging | 2 | 1.169 / 1.131 | -3.2% | +0.1% [-22.4%, +24.3%] | 7/15 | +0.8% |
| realworld/messaging | 8 | 1.438 / 1.411 | -1.9% | -1.4% [-9.7%, +11.8%] | 9/15 | +0.2% |
