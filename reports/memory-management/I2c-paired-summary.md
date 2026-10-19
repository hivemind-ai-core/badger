# I2c paired coverage and attribution

Median of adjacent candidate/baseline process ratios. Parentheses count faster pairs; ranges and every sample remain in the raw reports. These are not confidence bounds.

## I2c-coverage

| Case | Workers | Paired median | Faster pairs | Min / max |
|---|---:|---:|---:|---:|
| bytes-transient | 1 | +0.7% | 3/7 | -7.1% / +10.0% |
| bytes-transient | 2 | +1.3% | 3/7 | -4.1% / +6.7% |
| bytes-transient | 4 | -1.2% | 5/7 | -5.0% / +8.1% |
| bytes-transient | 8 | +0.1% | 3/7 | -15.5% / +6.4% |
| map-api-composed | 1 | -21.8% | 7/7 | -27.3% / -16.6% |
| map-api-composed | 2 | -21.7% | 7/7 | -24.1% / -18.3% |
| map-api-composed | 4 | -22.2% | 7/7 | -30.8% / -12.7% |
| map-api-composed | 8 | -21.0% | 7/7 | -25.2% / -13.9% |
| map-api-entry-control | 1 | -3.5% | 5/7 | -8.5% / +10.4% |
| map-api-entry-control | 2 | -4.2% | 5/7 | -8.4% / +13.0% |
| map-api-entry-control | 4 | -3.8% | 5/7 | -13.2% / +5.0% |
| map-api-entry-control | 8 | -1.7% | 5/7 | -3.7% / +14.1% |
| map-api-filter | 1 | -33.1% | 7/7 | -45.2% / -22.8% |
| map-api-filter | 2 | -32.4% | 7/7 | -37.8% / -21.7% |
| map-api-filter | 4 | -33.3% | 7/7 | -34.8% / -28.1% |
| map-api-filter | 8 | -33.9% | 7/7 | -43.7% / -20.9% |
| map-api-merge | 1 | -23.2% | 7/7 | -24.8% / -13.0% |
| map-api-merge | 2 | -22.2% | 7/7 | -28.6% / -18.8% |
| map-api-merge | 4 | -22.3% | 7/7 | -28.2% / -18.4% |
| map-api-merge | 8 | -21.2% | 7/7 | -25.5% / -14.4% |
| map-api-parallel | 1 | -21.8% | 7/7 | -25.3% / -12.3% |
| map-api-parallel | 2 | -21.3% | 7/7 | -22.9% / -18.3% |
| map-api-parallel | 4 | -20.2% | 7/7 | -26.1% / -7.2% |
| map-api-parallel | 8 | -13.5% | 7/7 | -21.0% / -4.9% |
| map-api-projections | 1 | -67.9% | 7/7 | -68.6% / -66.6% |
| map-api-projections | 2 | -63.9% | 7/7 | -65.1% / -61.9% |
| map-api-projections | 4 | -62.5% | 7/7 | -65.5% / -60.0% |
| map-api-projections | 8 | -63.1% | 7/7 | -63.5% / -62.6% |
| map-api-values | 1 | -26.3% | 7/7 | -30.1% / -23.5% |
| map-api-values | 2 | -25.1% | 7/7 | -31.0% / -14.8% |
| map-api-values | 4 | -26.1% | 7/7 | -36.0% / -15.9% |
| map-api-values | 8 | -23.5% | 7/7 | -34.2% / -19.1% |
| records-window | 1 | -1.4% | 5/7 | -2.9% / +3.6% |
| records-window | 2 | +3.4% | 1/7 | -8.4% / +7.5% |
| records-window | 4 | -1.6% | 5/7 | -4.3% / +2.6% |
| records-window | 8 | +0.6% | 3/7 | -4.4% / +4.7% |
| scalar-snapshots | 1 | +0.1% | 3/7 | -3.3% / +6.4% |
| scalar-snapshots | 2 | +0.1% | 2/7 | -4.7% / +5.9% |
| scalar-snapshots | 4 | +1.2% | 2/7 | -3.0% / +6.0% |
| scalar-snapshots | 8 | +1.2% | 3/7 | -3.0% / +13.2% |

## I2c-native-mir

| Case | Workers | Paired median | Faster pairs | Min / max |
|---|---:|---:|---:|---:|
| map-api-composed | 1 | -19.9% | 7/7 | -24.9% / -11.6% |
| map-api-composed | 8 | -22.9% | 7/7 | -24.9% / -18.1% |
| map-api-entry-control | 1 | -6.0% | 5/7 | -11.8% / +9.3% |
| map-api-entry-control | 8 | -1.0% | 5/7 | -26.4% / +10.7% |
| map-api-filter | 1 | -35.9% | 7/7 | -38.8% / -29.6% |
| map-api-filter | 8 | -31.2% | 7/7 | -37.3% / -22.8% |
| map-api-merge | 1 | -25.4% | 7/7 | -26.9% / -19.4% |
| map-api-merge | 8 | -21.3% | 7/7 | -31.6% / -15.0% |
| map-api-parallel | 1 | -20.0% | 7/7 | -21.7% / -16.3% |
| map-api-parallel | 8 | -8.9% | 7/7 | -20.7% / -2.2% |
| map-api-projections | 1 | -68.4% | 7/7 | -71.9% / -66.8% |
| map-api-projections | 8 | -61.7% | 7/7 | -62.5% / -59.4% |
| map-api-values | 1 | -25.6% | 7/7 | -30.6% / -21.6% |
| map-api-values | 8 | -24.3% | 7/7 | -27.8% / -0.3% |

## I2c-native-control

| Case | Workers | Paired median | Faster pairs | Min / max |
|---|---:|---:|---:|---:|
| bytes-transient | 1 | -0.8% | 4/7 | -6.7% / +1.9% |
| bytes-transient | 8 | +5.1% | 3/7 | -7.8% / +14.8% |
| map-api-composed | 1 | +0.0% | 3/7 | -7.0% / +7.1% |
| map-api-composed | 8 | -0.3% | 4/7 | -5.5% / +5.1% |
| map-api-parallel | 1 | -0.2% | 4/7 | -5.4% / +7.8% |
| map-api-parallel | 8 | -1.5% | 5/7 | -11.6% / +7.1% |
| map-api-projections | 1 | +0.6% | 3/7 | -4.0% / +10.0% |
| map-api-projections | 8 | +1.7% | 3/7 | -2.9% / +46.1% |

## I2c-record-repeat

| Case | Workers | Paired median | Faster pairs | Min / max |
|---|---:|---:|---:|---:|
| records-window | 2 | +0.4% | 7/15 | -4.3% / +76.1% |

## I2c-record-control

| Case | Workers | Paired median | Faster pairs | Min / max |
|---|---:|---:|---:|---:|
| records-window | 2 | +2.3% | 6/15 | -11.1% / +7.6% |
