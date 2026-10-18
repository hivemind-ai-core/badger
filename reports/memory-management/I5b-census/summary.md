# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/snapshots | 1 | 4.680 / 1.240 | -73.5% | -73.5% [-73.5%, -73.5%] | 1/1 | -20.2% |
| compare/snapshots | 8 | 4.640 / 1.237 | -73.3% | -73.3% [-73.3%, -73.3%] | 1/1 | -20.1% |
| realworld/reconcile | 1 | 7.585 / 7.545 | -0.5% | -0.5% [-0.5%, -0.5%] | 1/1 | -1.4% |
| realworld/reconcile | 8 | 6.763 / 6.717 | -0.7% | -0.7% [-0.7%, -0.7%] | 1/1 | +0.0% |
| realworld/state | 1 | 4.713 / 2.057 | -56.4% | -56.4% [-56.4%, -56.4%] | 1/1 | -5.0% |
| realworld/state | 8 | 4.944 / 2.105 | -57.4% | -57.4% [-57.4%, -57.4%] | 1/1 | -5.0% |

## Block means retain expensive iterations

One arithmetic mean of measured kernel iterations per process. This includes costly GC iterations but excludes fixture/I/O time as in the original contract. Inner iterations remain correlated.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/snapshots | 1 | 4.190 / 1.241 | -70.4% | -70.4% [-70.4%, -70.4%] | 1/1 | -20.2% |
| compare/snapshots | 8 | 4.712 / 1.223 | -74.0% | -74.0% [-74.0%, -74.0%] | 1/1 | -20.1% |
| realworld/reconcile | 1 | 8.231 / 8.583 | +4.3% | +4.3% [+4.3%, +4.3%] | 0/1 | -1.4% |
| realworld/reconcile | 8 | 7.815 / 7.989 | +2.2% | +2.2% [+2.2%, +2.2%] | 0/1 | +0.0% |
| realworld/state | 1 | 4.671 / 1.785 | -61.8% | -61.8% [-61.8%, -61.8%] | 1/1 | -5.0% |
| realworld/state | 8 | 4.867 / 1.818 | -62.6% | -62.6% [-62.6%, -62.6%] | 1/1 | -5.0% |
