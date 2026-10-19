# I3a paired timing evidence

Median adjacent candidate/baseline elapsed ratios; negative is faster. All samples retained. Ranges are not confidence intervals.

## I3a-coverage

| Case | Workers | Paired median | Faster pairs | Min / max |
|---|---:|---:|---:|---:|
| bytes-transient | 1 | +6.4% | 3/7 | -6.7% / +82.2% |
| bytes-transient | 2 | +12.4% | 1/7 | -1.3% / +16.3% |
| bytes-transient | 4 | +11.0% | 2/7 | -2.3% / +20.2% |
| bytes-transient | 8 | +8.4% | 0/7 | +1.4% / +9.5% |
| collected | 1 | +1.4% | 3/7 | -9.8% / +6.8% |
| collected | 2 | +1.8% | 2/7 | -10.7% / +3.4% |
| collected | 4 | -1.7% | 4/7 | -20.6% / +8.3% |
| collected | 8 | +3.0% | 0/7 | +0.9% / +10.1% |
| eager | 1 | +3.3% | 1/7 | -13.2% / +17.6% |
| eager | 2 | -0.8% | 4/7 | -10.1% / +5.3% |
| eager | 4 | +0.1% | 3/7 | -26.9% / +18.8% |
| eager | 8 | +0.3% | 3/7 | -36.5% / +14.6% |
| map-api-composed | 1 | +1.4% | 3/7 | -3.0% / +4.6% |
| map-api-composed | 2 | -0.7% | 4/7 | -10.9% / +6.8% |
| map-api-composed | 4 | +0.5% | 3/7 | -11.3% / +7.2% |
| map-api-composed | 8 | +1.6% | 2/7 | -0.9% / +4.5% |
| records-window | 1 | +0.7% | 2/7 | -17.3% / +8.1% |
| records-window | 2 | +0.5% | 3/7 | -4.3% / +5.8% |
| records-window | 4 | +2.5% | 1/7 | -7.7% / +12.6% |
| records-window | 8 | +0.2% | 3/7 | -35.0% / +10.9% |
| stored | 1 | -96.8% | 7/7 | -97.4% / -94.3% |
| stored | 2 | -96.5% | 7/7 | -97.0% / -96.3% |
| stored | 4 | -96.4% | 7/7 | -96.8% / -93.5% |
| stored | 8 | -96.3% | 7/7 | -96.6% / -95.1% |
| stored-collected | 1 | +3.4% | 2/7 | -9.2% / +18.0% |
| stored-collected | 2 | -1.3% | 5/7 | -36.2% / +3.3% |
| stored-collected | 4 | -4.5% | 4/7 | -7.6% / +9.0% |
| stored-collected | 8 | +5.4% | 2/7 | -23.6% / +24.2% |
| stored-escape | 1 | +7.5% | 0/7 | +0.9% / +25.8% |
| stored-escape | 2 | -4.2% | 4/7 | -11.4% / +3.7% |
| stored-escape | 4 | -1.0% | 4/7 | -11.0% / +4.6% |
| stored-escape | 8 | -6.7% | 5/7 | -9.0% / +3.8% |
| stored-list | 1 | -74.8% | 7/7 | -80.7% / -73.8% |
| stored-list | 2 | -75.6% | 7/7 | -78.8% / -75.1% |
| stored-list | 4 | -74.8% | 7/7 | -76.9% / -72.6% |
| stored-list | 8 | -76.3% | 7/7 | -78.5% / -74.6% |
| stored-pointers | 1 | -56.1% | 7/7 | -59.8% / -55.8% |
| stored-pointers | 2 | -47.2% | 7/7 | -55.6% / -44.8% |
| stored-pointers | 4 | -45.1% | 7/7 | -56.9% / -36.5% |
| stored-pointers | 8 | -45.7% | 7/7 | -52.8% / -43.4% |
| stored-reused | 1 | -96.5% | 7/7 | -97.0% / -96.3% |
| stored-reused | 2 | -96.8% | 7/7 | -98.0% / -96.6% |
| stored-reused | 4 | -96.8% | 7/7 | -97.3% / -96.6% |
| stored-reused | 8 | -96.7% | 7/7 | -97.4% / -94.6% |
| visible | 1 | -25.2% | 7/7 | -26.8% / -22.2% |
| visible | 2 | +18.8% | 1/7 | -23.5% / +28.8% |
| visible | 4 | +19.4% | 0/7 | +16.5% / +21.4% |
| visible | 8 | +18.9% | 0/7 | +4.9% / +38.1% |

## I3a-native-mir

| Case | Workers | Paired median | Faster pairs | Min / max |
|---|---:|---:|---:|---:|
| collected | 1 | -0.2% | 3/5 | -6.7% / +5.5% |
| collected | 8 | -0.6% | 3/5 | -9.2% / +1.2% |
| eager | 1 | -1.4% | 5/5 | -11.6% / -0.0% |
| eager | 8 | +2.4% | 0/5 | +0.2% / +12.1% |
| stored | 1 | -96.8% | 5/5 | -96.9% / -96.1% |
| stored | 8 | -96.4% | 5/5 | -96.5% / -96.3% |
| stored-collected | 1 | -7.7% | 3/5 | -14.2% / +36.6% |
| stored-collected | 8 | +0.4% | 2/5 | -6.2% / +8.6% |
| stored-escape | 1 | -2.3% | 4/5 | -5.1% / +14.7% |
| stored-escape | 8 | -0.0% | 3/5 | -28.2% / +5.5% |
| stored-list | 1 | -75.0% | 5/5 | -80.4% / -74.1% |
| stored-list | 8 | -75.6% | 5/5 | -79.4% / -70.8% |
| stored-pointers | 1 | -56.8% | 5/5 | -58.1% / -55.9% |
| stored-pointers | 8 | -46.2% | 5/5 | -47.1% / -42.0% |
| stored-reused | 1 | -96.3% | 5/5 | -96.5% / -95.0% |
| stored-reused | 8 | -96.5% | 5/5 | -96.7% / -95.3% |
| visible | 1 | -23.8% | 5/5 | -25.9% / -21.5% |
| visible | 8 | +16.4% | 1/5 | -4.5% / +17.2% |

## I3a-native-control

| Case | Workers | Paired median | Faster pairs | Min / max |
|---|---:|---:|---:|---:|
| bytes-transient | 1 | -1.1% | 3/5 | -6.4% / +1.8% |
| bytes-transient | 8 | -0.9% | 3/5 | -9.6% / +15.0% |
| stored | 1 | -2.8% | 3/5 | -9.7% / +2.6% |
| stored | 8 | -1.7% | 4/5 | -4.3% / +122.2% |
| stored-pointers | 1 | -2.5% | 4/5 | -2.9% / +5.6% |
| stored-pointers | 8 | +1.1% | 2/5 | -4.6% / +19.2% |
| visible | 1 | -0.6% | 4/5 | -4.0% / +1.1% |
| visible | 8 | +3.5% | 1/5 | -0.4% / +99.5% |

## I3a-isolated-source

| Case | Workers | Paired median | Faster pairs | Min / max |
|---|---:|---:|---:|---:|
| bytes-transient | 1 | +13.9% | 0/15 | +5.0% / +18.3% |
| bytes-transient | 8 | +10.6% | 2/15 | -0.7% / +45.6% |
| stored-escape | 1 | -1.2% | 10/15 | -14.5% / +3.4% |
| stored-escape | 8 | +0.8% | 7/15 | -15.8% / +30.8% |
| visible | 1 | +0.5% | 6/15 | -9.4% / +86.3% |
| visible | 8 | -0.3% | 8/15 | -37.7% / +71.7% |

## I3a-isolated-native

| Case | Workers | Paired median | Faster pairs | Min / max |
|---|---:|---:|---:|---:|
| bytes-transient | 1 | -1.2% | 8/15 | -15.0% / +12.6% |
| bytes-transient | 8 | -2.1% | 9/15 | -15.4% / +17.5% |
| stored-escape | 1 | +0.8% | 7/15 | -17.2% / +45.1% |
| stored-escape | 8 | -2.7% | 9/15 | -26.7% / +16.8% |
| visible | 1 | -0.6% | 9/15 | -56.4% / +8.0% |
| visible | 8 | +2.2% | 5/15 | -49.9% / +87.5% |

## I3a-isolated-cross-native

| Case | Workers | Paired median | Faster pairs | Min / max |
|---|---:|---:|---:|---:|
| bytes-transient | 1 | +3.7% | 2/15 | -7.8% / +18.4% |
| bytes-transient | 8 | +5.5% | 4/15 | -3.0% / +19.3% |
| stored-escape | 1 | +0.2% | 7/15 | -12.0% / +18.4% |
| stored-escape | 8 | +4.5% | 4/15 | -5.4% / +47.6% |
| visible | 1 | +0.0% | 7/15 | -53.7% / +172.7% |
| visible | 8 | -0.0% | 8/15 | -9.9% / +97.5% |
