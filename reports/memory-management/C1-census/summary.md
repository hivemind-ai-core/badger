# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/records | 1 | 8.249 / 8.363 | +1.4% | +2.5% [+1.0%, +67.5%] | 0/3 | +2.2% |
| compare/records | 8 | 8.403 / 8.400 | -0.0% | -0.1% [-3.6%, +3.1%] | 2/3 | +1.5% |
| realworld/json | 1 | 19.406 / 11.902 | -38.7% | -40.9% [-46.2%, -29.6%] | 3/3 | +3.0% |
| realworld/json | 8 | 18.483 / 13.200 | -28.6% | -27.7% [-35.8%, -26.5%] | 3/3 | +3.6% |
| realworld/reconcile | 1 | 6.181 / 6.534 | +5.7% | +5.7% [+2.2%, +7.6%] | 0/3 | -0.3% |
| realworld/reconcile | 8 | 6.227 / 6.176 | -0.8% | -0.8% [-4.4%, -0.2%] | 3/3 | +2.7% |

## Block means retain expensive iterations

One arithmetic mean of measured kernel iterations per process. This includes costly GC iterations but excludes fixture/I/O time as in the original contract. Inner iterations remain correlated.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/records | 1 | 7.885 / 7.858 | -0.3% | +0.3% [-1.1%, +65.3%] | 1/3 | +2.2% |
| compare/records | 8 | 8.083 / 8.418 | +4.1% | +3.5% [-3.7%, +7.1%] | 1/3 | +1.5% |
| realworld/json | 1 | 19.999 / 12.357 | -38.2% | -38.2% [-44.1%, -32.9%] | 3/3 | +3.0% |
| realworld/json | 8 | 19.528 / 13.717 | -29.8% | -29.8% [-34.1%, -24.7%] | 3/3 | +3.6% |
| realworld/reconcile | 1 | 7.272 / 7.354 | +1.1% | +1.1% [+0.5%, +6.2%] | 0/3 | -0.3% |
| realworld/reconcile | 8 | 7.289 / 7.211 | -1.1% | -1.6% [-4.6%, +4.9%] | 2/3 | +2.7% |
