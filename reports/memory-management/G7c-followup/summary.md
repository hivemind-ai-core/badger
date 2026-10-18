| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/batches | 1 | 0.693 / 0.737 | +6.4% | +1.2% [-21.2%, +14.5%] | 4/15 | +0.0% |
| compare/snapshots | 1 | 3.842 / 3.578 | -6.9% | -5.1% [-22.4%, +12.7%] | 10/15 | -0.5% |
| realworld/state | 1 | 4.334 / 4.464 | +3.0% | +0.1% [-32.0%, +39.2%] | 7/15 | +0.1% |

One median per fresh process. All inner outputs checked; negative is faster/smaller. No outliers removed. Ranges are not confidence intervals.
