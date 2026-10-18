# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| realworld/messaging | 4 | 1.113 / 1.151 | +3.5% | +2.2% [-15.5%, +41.5%] | 7/15 | +0.2% |
| realworld/messaging | 8 | 1.332 / 1.374 | +3.1% | +2.4% [-14.9%, +10.1%] | 5/15 | +1.6% |

## Block means retain expensive iterations

One arithmetic mean of measured kernel iterations per process. This includes costly GC iterations but excludes fixture/I/O time as in the original contract. Inner iterations remain correlated.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| realworld/messaging | 4 | 1.272 / 1.286 | +1.2% | -1.7% [-19.4%, +29.6%] | 8/15 | +0.2% |
| realworld/messaging | 8 | 1.470 / 1.478 | +0.5% | +3.3% [-11.4%, +16.0%] | 6/15 | +1.6% |
