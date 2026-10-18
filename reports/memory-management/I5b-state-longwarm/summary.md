# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| realworld/state | 1 | 5.478 / 1.096 | -80.0% | -79.6% [-82.8%, -78.5%] | 7/7 | -23.4% |
| realworld/state | 8 | 5.001 / 1.104 | -77.9% | -77.9% [-84.0%, -72.9%] | 7/7 | -23.4% |

## Block means retain expensive iterations

One arithmetic mean of measured kernel iterations per process. This includes costly GC iterations but excludes fixture/I/O time as in the original contract. Inner iterations remain correlated.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| realworld/state | 1 | 5.525 / 1.305 | -76.4% | -76.0% [-80.7%, -74.0%] | 7/7 | -23.4% |
| realworld/state | 8 | 5.255 / 1.328 | -74.7% | -74.9% [-81.5%, -68.0%] | 7/7 | -23.4% |
