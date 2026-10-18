# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| realworld/messaging | 4 | 1.170 / 1.264 | +8.0% | +4.7% [-26.9%, +51.1%] | 5/15 | -1.0% |
| realworld/messaging | 8 | 1.391 / 1.326 | -4.7% | -3.8% [-16.2%, +9.8%] | 10/15 | +2.9% |

## Block means retain expensive iterations

One arithmetic mean of measured kernel iterations per process. This includes costly GC iterations but excludes fixture/I/O time as in the original contract. Inner iterations remain correlated.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| realworld/messaging | 4 | 1.334 / 1.316 | -1.4% | +4.6% [-16.1%, +32.4%] | 6/15 | -1.0% |
| realworld/messaging | 8 | 1.473 / 1.420 | -3.6% | -2.7% [-22.2%, +9.4%] | 11/15 | +2.9% |
