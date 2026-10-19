# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| realworld/json | 4 | 11.053 / 10.887 | -1.5% | -1.3% [-8.4%, +2.9%] | 12/15 | -0.4% |
| realworld/json | 8 | 10.966 / 10.866 | -0.9% | -0.2% [-8.5%, +7.6%] | 9/15 | +0.7% |
| realworld/messaging | 4 | 1.164 / 1.230 | +5.7% | +6.7% [-27.3%, +36.2%] | 4/15 | -0.1% |
| realworld/messaging | 8 | 1.343 / 1.359 | +1.1% | +0.6% [-12.8%, +17.4%] | 6/15 | +3.1% |
| realworld/state | 4 | 1.046 / 1.053 | +0.6% | +1.1% [-7.7%, +51.0%] | 5/15 | -0.2% |
| realworld/state | 8 | 1.060 / 1.058 | -0.2% | +1.6% [-4.2%, +7.5%] | 5/15 | +0.0% |

## Block means retain expensive iterations

One arithmetic mean of measured kernel iterations per process. This includes costly GC iterations but excludes fixture/I/O time as in the original contract. Inner iterations remain correlated.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| realworld/json | 4 | 11.078 / 10.910 | -1.5% | -1.3% [-6.0%, +2.6%] | 10/15 | -0.4% |
| realworld/json | 8 | 11.059 / 10.913 | -1.3% | -1.4% [-9.7%, +6.6%] | 8/15 | +0.7% |
| realworld/messaging | 4 | 1.279 / 1.342 | +4.9% | +6.7% [-25.5%, +32.3%] | 6/15 | -0.1% |
| realworld/messaging | 8 | 1.498 / 1.495 | -0.2% | +1.4% [-6.6%, +17.3%] | 7/15 | +3.1% |
| realworld/state | 4 | 1.226 / 1.223 | -0.2% | +0.4% [-21.2%, +31.2%] | 5/15 | -0.2% |
| realworld/state | 8 | 1.227 / 1.257 | +2.4% | +2.5% [-13.3%, +23.3%] | 5/15 | +0.0% |
