# I5a: private scalar-map baseline before compiler/runtime edits

Production baseline is `67d3f73` (G7a and I2b), using immutable entries-candidate
and entries-diagnostic artifacts from `I2b-binaries.json`. Coverage/fixture-only
revision `c518383` adds full scalar-snapshot validation and alias/cursor/input
regressions. All oracles and the baseline fixture pass. No own builds or tests
overlapped measurement.

| Dataset | Measured processes |
|---|---:|
| [I5a-app-baseline](I5a-app-baseline/measurements.json.gz) | 60 |
| [I5a-census-before](I5a-census-before/measurements.json.gz) | 16 |
| [I5a-coverage-baseline](I5a-coverage-baseline/measurements.json.gz) | 48 |
| [I5a-smoke-baseline](I5a-smoke-baseline/measurements.json.gz) | 8 |

Total: 132 processes before production edits. Diagnostic census
variants are perturbed and separate from ordinary timing.
