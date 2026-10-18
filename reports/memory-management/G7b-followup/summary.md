| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/batches | 1 | 0.706 / 0.699 | -1.0% | -1.9% [-27.3%, +17.6%] | 11/15 | +0.5% |
| compare/snapshots | 1 | 3.967 / 3.502 | -11.7% | -11.4% [-29.2%, -0.7%] | 15/15 | -0.2% |
| realworld/state | 1 | 4.197 / 4.309 | +2.7% | +6.6% [-14.4%, +66.5%] | 5/15 | -0.1% |

One median per fresh process. All inner outputs checked; negative is faster/smaller. No outliers removed. Ranges are not confidence intervals.
