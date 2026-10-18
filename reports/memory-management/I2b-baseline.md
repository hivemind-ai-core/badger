# I2b: baseline before Map.entries method elision

Production baseline is `69b3a61` (includes G7a); coverage/fixture-only revision
is `b1335db`. Immutable ordinary and diagnostic binaries are the G7 candidate
artifacts, with hashes in `G7-binaries.json`. All benchmark oracles and the new
map/snapshot/receiver/lifetime fixture pass before compiler changes.

| Dataset | Measured processes |
|---|---:|
| [I2b-app-baseline](I2b-app-baseline/measurements.json.gz) | 60 |
| [I2b-census-before](I2b-census-before/measurements.json.gz) | 16 |
| [I2b-coverage-baseline](I2b-coverage-baseline/measurements.json.gz) | 24 |
| [I2b-smoke-baseline](I2b-smoke-baseline/measurements.json.gz) | 4 |

Total 104. Diagnostic phases/censuses are separately perturbed
variants. Realworld and compare workloads are unchanged. No own builds or tests
overlapped measurements.
