Process-wide managed allocations, including startup and output encoding. Three repetitions unless noted in the raw report.
Requested bytes include headers. Cell bytes include pool rounding but exclude native buffers, allocator metadata and unused pool capacity.
Early reclamation and first survival are observed fractions of all allocations; unswept tail objects censor lifetime measurements.
Survival does not establish escape or unique ownership. Diagnostic timings/RSS include profiler overhead.

| Case | Workers | Objects | Requested MiB | Cell MiB | Early reclaimed bytes | First-survived bytes | Remaining bytes | Side-vector capacity MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| map-api-composed | 1 | 1,005,894 | 306.35 | 308.72 | 96.7% | 2.9% | 0.8% | 2.00 |
| map-api-composed | 8 | 1,005,894 | 306.35 | 308.72 | 96.7% | 2.9% | 0.8% | 2.00 |
| map-api-entry-control | 1 | 275,006 | 17.15 | 17.17 | 91.4% | 2.0% | 8.6% | 4.00 |
| map-api-entry-control | 8 | 275,006 | 17.15 | 17.17 | 91.4% | 2.0% | 8.6% | 4.00 |
| map-api-filter | 1 | 179,526 | 49.18 | 49.77 | 96.5% | 1.3% | 3.5% | 1.00 |
| map-api-filter | 8 | 179,526 | 49.18 | 49.77 | 96.5% | 1.3% | 3.5% | 1.00 |
| map-api-merge | 1 | 525,894 | 114.81 | 116.12 | 91.9% | 6.0% | 8.1% | 4.00 |
| map-api-merge | 8 | 525,894 | 114.81 | 116.12 | 91.9% | 6.0% | 8.1% | 4.00 |
| map-api-parallel | 1 | 1,034,662 | 311.89 | 314.39 | 91.6% | 6.2% | 4.1% | 4.00 |
| map-api-parallel | 8 | 1,034,662 | 311.89 | 314.39 | 92.7% | 6.4% | 2.4% | 4.00 |
| map-api-projections | 1 | 10,814 | 6.51 | 6.53 | 0.0% | 0.0% | 100.0% | 0.25 |
| map-api-projections | 8 | 10,814 | 6.51 | 6.53 | 0.0% | 0.0% | 100.0% | 0.25 |
| map-api-values | 1 | 267,078 | 92.56 | 93.18 | 85.1% | 1.6% | 14.9% | 1.00 |
| map-api-values | 8 | 267,078 | 92.56 | 93.18 | 85.1% | 1.6% | 14.9% | 1.00 |

Allocation paths: median process fractions of managed allocation requests; each reuse counts a new object, not a distinct cell.
Pool bytes below are native chunk reservations. They exclude system cells and other native storage.

| Case | Workers | Reuse objects | Bump objects | System objects | Reuse cell bytes | Bump cell bytes | System cell bytes | Refill chunks | Refill MiB | Trim chunks | Trim MiB | Reserved MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| map-api-composed | 1 | 71.8% | 8.4% | 19.8% | 57.6% | 4.8% | 37.7% | 9 | 16.25 | 1 | 2.00 | 14.25 |
| map-api-composed | 8 | 71.8% | 8.4% | 19.8% | 57.6% | 4.8% | 37.7% | 9 | 16.25 | 1 | 2.00 | 14.25 |
| map-api-entry-control | 1 | 6.7% | 92.0% | 1.3% | 3.4% | 48.4% | 48.2% | 6 | 10.25 | 0 | 0.00 | 10.25 |
| map-api-entry-control | 8 | 6.7% | 92.0% | 1.3% | 3.4% | 48.4% | 48.2% | 6 | 10.25 | 0 | 0.00 | 10.25 |
| map-api-filter | 1 | 60.9% | 31.8% | 7.3% | 57.2% | 28.8% | 14.0% | 9 | 16.25 | 0 | 0.00 | 16.25 |
| map-api-filter | 8 | 60.9% | 31.8% | 7.3% | 57.2% | 28.8% | 14.0% | 9 | 16.25 | 0 | 0.00 | 16.25 |
| map-api-merge | 1 | 61.0% | 24.4% | 14.6% | 47.2% | 13.1% | 39.7% | 9 | 16.25 | 0 | 0.00 | 16.25 |
| map-api-merge | 8 | 61.0% | 24.4% | 14.6% | 47.2% | 13.1% | 39.7% | 9 | 16.25 | 0 | 0.00 | 16.25 |
| map-api-parallel | 1 | 67.0% | 13.4% | 19.6% | 56.7% | 5.6% | 37.7% | 10 | 18.25 | 0 | 0.00 | 18.25 |
| map-api-parallel | 8 | 64.6% | 15.8% | 19.6% | 54.4% | 7.9% | 37.7% | 24 | 34.00 | 5 | 8.25 | 25.75 |
| map-api-projections | 1 | 0.0% | 76.3% | 23.7% | 0.0% | 13.9% | 86.1% | 2 | 2.25 | 0 | 0.00 | 2.25 |
| map-api-projections | 8 | 0.0% | 76.3% | 23.7% | 0.0% | 13.9% | 86.1% | 2 | 2.25 | 0 | 0.00 | 2.25 |
| map-api-values | 1 | 57.5% | 14.5% | 28.0% | 42.5% | 9.4% | 48.0% | 6 | 10.25 | 0 | 0.00 | 10.25 |
| map-api-values | 8 | 57.5% | 14.5% | 28.0% | 42.5% | 9.4% | 48.0% | 6 | 10.25 | 0 | 0.00 | 10.25 |

Calibration: workload time medians; positive change means slower. Shared-host ranges are in summary.md.

| Case | Workers | Phase-off vs ordinary | Phase-only vs phase-off | Census vs phase-only |
|---|---:|---:|---:|---:|
| map-api-composed | 1 | +0.1% | +8.1% | +23.5% |
| map-api-composed | 8 | +0.5% | +3.6% | +20.1% |
| map-api-entry-control | 1 | +1.7% | -2.7% | +40.2% |
| map-api-entry-control | 8 | +2.9% | -0.3% | +34.0% |
| map-api-filter | 1 | -0.9% | +1.8% | +21.8% |
| map-api-filter | 8 | +1.3% | +0.6% | +22.7% |
| map-api-merge | 1 | +4.4% | -6.7% | +30.1% |
| map-api-merge | 8 | -2.7% | +5.6% | +11.9% |
| map-api-parallel | 1 | +5.1% | +2.6% | +19.1% |
| map-api-parallel | 8 | +7.4% | -5.8% | +31.0% |
| map-api-projections | 1 | -11.9% | +1.4% | -0.5% |
| map-api-projections | 8 | +0.1% | +4.5% | -1.8% |
| map-api-values | 1 | +8.0% | -1.2% | +25.2% |
| map-api-values | 8 | +7.7% | -0.2% | +17.2% |

Largest runtime allocation sites (byte shares pooled over repetitions):

| Case | Workers | Type | Runtime source | Share of requested bytes |
|---|---:|---|---|---:|
| map-api-composed | 1 | MapNode | hamt.rs:252 | 95.1% |
| map-api-composed | 1 | MapNode | hamt.rs:186 | 4.7% |
| map-api-composed | 1 | MapCursor | hamt.rs:601 | 0.1% |
| map-api-composed | 1 | String | string.rs:13 | 0.0% |
| map-api-composed | 8 | MapNode | hamt.rs:252 | 95.1% |
| map-api-composed | 8 | MapNode | hamt.rs:186 | 4.7% |
| map-api-composed | 8 | MapCursor | hamt.rs:601 | 0.1% |
| map-api-composed | 8 | String | string.rs:13 | 0.0% |
| map-api-entry-control | 1 | Page | list.rs:326 | 47.2% |
| map-api-entry-control | 1 | Tuple | lib.rs:250 | 46.6% |
| map-api-entry-control | 1 | MapNode | hamt.rs:252 | 3.8% |
| map-api-entry-control | 1 | PageDirectory | list.rs:430 | 0.8% |
| map-api-entry-control | 8 | Page | list.rs:326 | 47.2% |
| map-api-entry-control | 8 | Tuple | lib.rs:250 | 46.6% |
| map-api-entry-control | 8 | MapNode | hamt.rs:252 | 3.8% |
| map-api-entry-control | 8 | PageDirectory | list.rs:430 | 0.8% |
| map-api-filter | 1 | MapNode | hamt.rs:252 | 97.0% |
| map-api-filter | 1 | MapNode | hamt.rs:186 | 2.5% |
| map-api-filter | 1 | MapCursor | hamt.rs:601 | 0.3% |
| map-api-filter | 1 | String | string.rs:13 | 0.2% |
| map-api-filter | 8 | MapNode | hamt.rs:252 | 97.0% |
| map-api-filter | 8 | MapNode | hamt.rs:186 | 2.5% |
| map-api-filter | 8 | MapCursor | hamt.rs:601 | 0.3% |
| map-api-filter | 8 | String | string.rs:13 | 0.2% |
| map-api-merge | 1 | MapNode | hamt.rs:252 | 79.4% |
| map-api-merge | 1 | String | string.rs:13 | 10.7% |
| map-api-merge | 1 | MapNode | hamt.rs:186 | 9.7% |
| map-api-merge | 1 | MapCursor | hamt.rs:601 | 0.2% |
| map-api-merge | 8 | MapNode | hamt.rs:252 | 79.4% |
| map-api-merge | 8 | String | string.rs:13 | 10.7% |
| map-api-merge | 8 | MapNode | hamt.rs:186 | 9.7% |
| map-api-merge | 8 | MapCursor | hamt.rs:601 | 0.2% |
| map-api-parallel | 1 | MapNode | hamt.rs:252 | 94.9% |
| map-api-parallel | 1 | MapNode | hamt.rs:186 | 4.7% |
| map-api-parallel | 1 | String | string.rs:13 | 0.3% |
| map-api-parallel | 1 | MapCursor | hamt.rs:601 | 0.1% |
| map-api-parallel | 8 | MapNode | hamt.rs:252 | 94.9% |
| map-api-parallel | 8 | MapNode | hamt.rs:186 | 4.7% |
| map-api-parallel | 8 | String | string.rs:13 | 0.3% |
| map-api-parallel | 8 | MapCursor | hamt.rs:601 | 0.1% |
| map-api-projections | 1 | Page | list.rs:326 | 82.8% |
| map-api-projections | 1 | MapNode | hamt.rs:252 | 10.1% |
| map-api-projections | 1 | MapCursor | hamt.rs:601 | 2.0% |
| map-api-projections | 1 | String | string.rs:13 | 1.7% |
| map-api-projections | 8 | Page | list.rs:326 | 82.8% |
| map-api-projections | 8 | MapNode | hamt.rs:252 | 10.1% |
| map-api-projections | 8 | MapCursor | hamt.rs:601 | 2.0% |
| map-api-projections | 8 | String | string.rs:13 | 1.7% |
| map-api-values | 1 | MapNode | hamt.rs:252 | 97.0% |
| map-api-values | 1 | MapNode | hamt.rs:186 | 2.7% |
| map-api-values | 1 | MapCursor | hamt.rs:601 | 0.1% |
| map-api-values | 1 | String | string.rs:13 | 0.1% |
| map-api-values | 8 | MapNode | hamt.rs:252 | 97.0% |
| map-api-values | 8 | MapNode | hamt.rs:186 | 2.7% |
| map-api-values | 8 | MapCursor | hamt.rs:601 | 0.1% |
| map-api-values | 8 | String | string.rs:13 | 0.1% |
