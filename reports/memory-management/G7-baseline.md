# G7 baseline before collector edits

Production baseline is `fdf36dc`; harness revision `cbb799d` changes no
production code. Immutable ordinary/diagnostic binary hashes and successful
coverage checks are in `G7-validation/`. All independent application and coverage
oracles passed. No own builds/tests overlapped measurements.

| Dataset | Measured fresh processes |
|---|---:|
| [G7-app-baseline](G7-app-baseline/measurements.json.gz) | 108 |
| [G7-app-smoke-baseline](G7-app-smoke-baseline/measurements.json.gz) | 18 |
| [G7-census-before](G7-census-before/measurements.json.gz) | 16 |
| [G7-coverage-baseline](G7-coverage-baseline/measurements.json.gz) | 96 |
| [G7-smoke-baseline](G7-smoke-baseline/measurements.json.gz) | 4 |

Total: 242 measured processes. Application processes include three warmups
and five measured inner iterations (smoke uses one/two); each contributes one
warm median. Diagnostic phase/allocation runs are separate perturbed variants,
not ordinary timing evidence. No production changes preceded these baselines.
