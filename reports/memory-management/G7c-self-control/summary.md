| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/batches | 1 | 0.685 / 0.694 | +1.3% | +2.2% [-5.3%, +12.9%] | 4/15 | -0.5% |
| compare/snapshots | 1 | 3.765 / 3.652 | -3.0% | -4.9% [-14.6%, +13.4%] | 11/15 | -0.3% |
| realworld/state | 1 | 4.203 / 4.504 | +7.2% | +7.2% [-39.2%, +29.0%] | 6/15 | -0.1% |

One median per fresh process. All inner outputs checked; negative is faster/smaller. No outliers removed. Ranges are not confidence intervals.
