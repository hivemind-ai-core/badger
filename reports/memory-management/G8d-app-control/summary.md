# Application kernel medians

Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/batches | 1 | 0.597 / 0.590 | -1.2% | -1.5% [-5.9%, +2.4%] | 11/15 | -0.3% |
| compare/batches | 2 | 0.590 / 0.591 | +0.2% | +0.0% [-4.2%, +2.7%] | 7/15 | +0.4% |
| realworld/cpu | 1 | 48.958 / 48.962 | +0.0% | +0.0% [-0.8%, +0.4%] | 6/15 | -0.7% |
| realworld/cpu | 2 | 49.041 / 49.119 | +0.2% | +0.2% [-0.6%, +0.6%] | 6/15 | -0.2% |
| realworld/json | 1 | 4.826 / 4.863 | +0.8% | +0.9% [-2.8%, +6.8%] | 6/15 | +0.1% |
| realworld/json | 2 | 5.207 / 5.246 | +0.7% | +0.7% [-4.4%, +2.6%] | 6/15 | -0.2% |
| realworld/messaging | 1 | 0.929 / 0.921 | -0.8% | -0.8% [-4.6%, +4.6%] | 9/15 | +0.5% |
| realworld/messaging | 2 | 0.636 / 0.618 | -2.8% | +0.2% [-33.1%, +13.6%] | 7/15 | +0.3% |

## Block means retain expensive iterations

One arithmetic mean of measured kernel iterations per process. This includes costly GC iterations but excludes fixture/I/O time as in the original contract. Inner iterations remain correlated.

| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |
|---|---:|---:|---:|---:|---:|---:|
| compare/batches | 1 | 0.659 / 0.646 | -2.0% | -1.5% [-8.3%, +0.0%] | 14/15 | -0.3% |
| compare/batches | 2 | 0.664 / 0.672 | +1.2% | +1.3% [-21.9%, +26.5%] | 7/15 | +0.4% |
| realworld/cpu | 1 | 49.235 / 49.241 | +0.0% | +0.1% [-1.3%, +0.5%] | 4/15 | -0.7% |
| realworld/cpu | 2 | 49.267 / 49.362 | +0.2% | +0.1% [-0.4%, +0.6%] | 6/15 | -0.2% |
| realworld/json | 1 | 5.291 / 5.312 | +0.4% | +0.4% [-3.7%, +6.4%] | 5/15 | +0.1% |
| realworld/json | 2 | 5.672 / 5.691 | +0.3% | +0.2% [-5.0%, +5.3%] | 7/15 | -0.2% |
| realworld/messaging | 1 | 1.057 / 1.042 | -1.4% | -1.6% [-9.7%, +6.8%] | 11/15 | +0.5% |
| realworld/messaging | 2 | 0.766 / 0.745 | -2.7% | -3.7% [-22.8%, +8.9%] | 10/15 | +0.3% |
