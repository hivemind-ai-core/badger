# G8d corrected-main paired native evidence

Identical saved MIR in fixed native runtimes. Negative changes are faster; all samples and ranges are retained. Controls are independent cohorts, not corrections or confidence intervals.

## G8d-current-native

| Case | Workers | Paired median | Faster pairs | Min / max |
|---|---:|---:|---:|---:|
| buffers-capacity-31 | 1 | -14.70% | 7/7 | -21.5% / -7.8% |
| buffers-capacity-31 | 2 | -15.70% | 7/7 | -17.2% / -11.8% |
| buffers-capacity-31 | 4 | -14.11% | 7/7 | -34.4% / -5.6% |
| buffers-capacity-31 | 8 | -13.82% | 7/7 | -22.8% / -8.1% |
| buffers-capacity-32 | 1 | -13.56% | 7/7 | -16.2% / -12.6% |
| buffers-capacity-32 | 2 | -15.29% | 7/7 | -17.8% / -8.3% |
| buffers-capacity-32 | 4 | -13.66% | 7/7 | -19.0% / -10.5% |
| buffers-capacity-32 | 8 | -14.15% | 7/7 | -16.6% / -9.4% |
| buffers-capacity-33 | 1 | -10.71% | 6/7 | -13.9% / +4.4% |
| buffers-capacity-33 | 2 | -10.91% | 7/7 | -16.4% / -8.9% |
| buffers-capacity-33 | 4 | -11.90% | 7/7 | -15.1% / -6.0% |
| buffers-capacity-33 | 8 | -9.31% | 7/7 | -14.5% / -5.9% |
| buffers-capacity-grow | 1 | -8.78% | 6/7 | -12.4% / +0.0% |
| buffers-capacity-grow | 2 | -11.35% | 7/7 | -18.0% / -5.4% |
| buffers-capacity-grow | 4 | -10.55% | 7/7 | -11.9% / -6.2% |
| buffers-capacity-grow | 8 | -8.87% | 7/7 | -11.2% / -4.1% |
| buffers-capacity-pointers | 1 | -1.82% | 7/7 | -4.9% / -0.1% |
| buffers-capacity-pointers | 2 | -1.94% | 4/7 | -3.2% / +4.4% |
| buffers-capacity-pointers | 4 | -1.12% | 5/7 | -4.1% / +2.6% |
| buffers-capacity-pointers | 8 | +0.52% | 2/7 | -3.9% / +7.4% |
| buffers-grow | 1 | -5.21% | 7/7 | -6.5% / -2.6% |
| buffers-grow | 2 | -9.33% | 6/7 | -10.7% / +0.0% |
| buffers-grow | 4 | -8.23% | 7/7 | -12.5% / -1.5% |
| buffers-grow | 8 | -5.95% | 7/7 | -9.1% / -2.4% |
| buffers-pointers | 1 | -0.73% | 5/7 | -2.5% / +3.3% |
| buffers-pointers | 2 | +0.16% | 3/7 | -8.1% / +7.5% |
| buffers-pointers | 4 | -0.20% | 4/7 | -6.0% / +5.5% |
| buffers-pointers | 8 | +0.10% | 3/7 | -1.8% / +3.2% |
| bytes-257 | 1 | -15.98% | 7/7 | -23.6% / -9.3% |
| bytes-257 | 2 | -21.01% | 7/7 | -24.8% / -14.7% |
| bytes-257 | 4 | -14.97% | 7/7 | -21.3% / -2.1% |
| bytes-257 | 8 | -14.81% | 7/7 | -24.8% / -4.0% |
| bytes-transient | 1 | -13.88% | 7/7 | -23.7% / -9.5% |
| bytes-transient | 2 | -17.41% | 7/7 | -21.3% / -13.9% |
| bytes-transient | 4 | -16.13% | 7/7 | -24.3% / -10.3% |
| bytes-transient | 8 | -17.31% | 7/7 | -21.6% / -5.7% |
| records-window | 1 | -2.09% | 6/7 | -3.5% / +0.9% |
| records-window | 2 | -0.26% | 5/7 | -5.0% / +5.7% |
| records-window | 4 | -0.73% | 4/7 | -2.5% / +2.9% |
| records-window | 8 | -0.13% | 5/7 | -4.7% / +3.1% |
| stored | 1 | +0.52% | 2/7 | -0.6% / +75.7% |
| stored | 2 | +0.31% | 2/7 | -1.8% / +7.3% |
| stored | 4 | +0.29% | 2/7 | -1.9% / +105.0% |
| stored | 8 | +0.29% | 3/7 | -12.5% / +43.2% |
| words-transient | 1 | -19.01% | 7/7 | -26.0% / -14.3% |
| words-transient | 2 | -16.07% | 7/7 | -18.9% / -13.9% |
| words-transient | 4 | -18.73% | 7/7 | -19.6% / -14.7% |
| words-transient | 8 | -17.35% | 7/7 | -20.4% / -13.4% |

## G8d-current-control

| Case | Workers | Paired median | Faster pairs | Min / max |
|---|---:|---:|---:|---:|
| buffers-capacity-pointers | 1 | -0.82% | 5/7 | -18.0% / +25.0% |
| buffers-capacity-pointers | 8 | -1.98% | 5/7 | -4.7% / +7.0% |
| bytes-transient | 1 | +5.92% | 3/7 | -10.5% / +10.9% |
| bytes-transient | 8 | +2.08% | 2/7 | -13.2% / +16.0% |
| records-window | 1 | +0.95% | 2/7 | -3.3% / +3.5% |
| records-window | 8 | +0.42% | 3/7 | -6.1% / +6.1% |
| stored | 1 | +1.61% | 2/7 | -1.4% / +3.9% |
| stored | 8 | -1.50% | 6/7 | -26.5% / +70.6% |

