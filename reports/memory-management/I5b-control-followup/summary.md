# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/batches | 2 | 0.650 / 0.651 | +0.0% | +0.6% [-46.3%, +24.6%] | 7/15 | +2.2% |
| compare/batches | 8 | 0.653 / 0.700 | +7.2% | +3.6% [-50.2%, +82.7%] | 6/15 | +12.9% |
| realworld/json | 2 | 18.377 / 18.548 | +0.9% | +1.2% [-2.8%, +25.3%] | 4/15 | +2.5% |
| realworld/json | 8 | 18.221 / 18.399 | +1.0% | +0.1% [-5.3%, +5.5%] | 7/15 | +1.3% |
| realworld/messaging | 2 | 0.948 / 1.001 | +5.6% | +7.0% [-25.0%, +62.3%] | 3/15 | -4.2% |
| realworld/messaging | 8 | 1.390 / 1.360 | -2.2% | -0.9% [-7.6%, +8.8%] | 10/15 | +2.9% |

## Block means retain expensive iterations

One arithmetic mean of measured kernel iterations per process. This includes costly GC iterations but excludes fixture/I/O time as in the original contract. Inner iterations remain correlated.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/batches | 2 | 0.865 / 0.859 | -0.7% | -1.9% [-33.1%, +17.3%] | 10/15 | +2.2% |
| compare/batches | 8 | 0.883 / 0.924 | +4.6% | -0.4% [-28.7%, +24.9%] | 8/15 | +12.9% |
| realworld/json | 2 | 18.558 / 18.676 | +0.6% | +0.4% [-4.4%, +26.6%] | 6/15 | +2.5% |
| realworld/json | 8 | 18.461 / 18.661 | +1.1% | +0.3% [-6.1%, +4.5%] | 7/15 | +1.3% |
| realworld/messaging | 2 | 1.038 / 1.105 | +6.4% | +3.2% [-21.4%, +39.4%] | 5/15 | -4.2% |
| realworld/messaging | 8 | 1.454 / 1.389 | -4.5% | -2.9% [-8.8%, +11.7%] | 12/15 | +2.9% |
