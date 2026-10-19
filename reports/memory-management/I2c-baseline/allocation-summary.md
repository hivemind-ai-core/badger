Process-wide managed allocations, including startup and output encoding. Three repetitions unless noted in the raw report.
Requested bytes include headers. Cell bytes include pool rounding but exclude native buffers, allocator metadata and unused pool capacity.
Early reclamation and first survival are observed fractions of all allocations; unswept tail objects censor lifetime measurements.
Survival does not establish escape or unique ownership. Diagnostic timings/RSS include profiler overhead.

| Case | Workers | Objects | Requested MiB | Cell MiB | Early reclaimed bytes | First-survived bytes | Remaining bytes | Side-vector capacity MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| map-api-composed | 1 | 1,976,390 | 346.51 | 348.87 | 93.9% | 3.1% | 3.6% | 4.00 |
| map-api-composed | 8 | 1,976,390 | 346.51 | 348.87 | 93.9% | 3.1% | 3.6% | 4.00 |
| map-api-entry-control | 1 | 275,006 | 17.15 | 17.17 | 91.4% | 2.0% | 8.6% | 4.00 |
| map-api-entry-control | 8 | 275,006 | 17.15 | 17.17 | 91.4% | 2.0% | 8.6% | 4.00 |
| map-api-filter | 1 | 443,974 | 59.88 | 60.47 | 79.1% | 1.3% | 20.9% | 2.00 |
| map-api-filter | 8 | 443,974 | 59.88 | 60.47 | 79.1% | 1.3% | 20.9% | 2.00 |
| map-api-merge | 1 | 1,055,046 | 136.22 | 137.54 | 88.3% | 6.0% | 11.7% | 4.00 |
| map-api-merge | 8 | 1,055,046 | 136.22 | 137.54 | 88.3% | 6.0% | 11.7% | 4.00 |
| map-api-parallel | 1 | 2,005,158 | 352.05 | 354.54 | 87.3% | 8.4% | 7.1% | 8.00 |
| map-api-parallel | 8 | 2,005,158 | 352.05 | 354.54 | 88.4% | 8.6% | 5.6% | 8.00 |
| map-api-projections | 1 | 539,710 | 27.92 | 27.94 | 56.1% | 1.3% | 43.9% | 8.00 |
| map-api-projections | 8 | 539,710 | 27.92 | 27.94 | 56.1% | 1.3% | 43.9% | 8.00 |
| map-api-values | 1 | 531,526 | 103.27 | 103.88 | 91.4% | 1.9% | 8.6% | 2.00 |
| map-api-values | 8 | 531,526 | 103.27 | 103.88 | 91.4% | 1.9% | 8.6% | 2.00 |

Allocation paths: median process fractions of managed allocation requests; each reuse counts a new object, not a distinct cell.
Pool bytes below are native chunk reservations. They exclude system cells and other native storage.

| Case | Workers | Reuse objects | Bump objects | System objects | Reuse cell bytes | Bump cell bytes | System cell bytes | Refill chunks | Refill MiB | Trim chunks | Trim MiB | Reserved MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| map-api-composed | 1 | 83.2% | 6.5% | 10.3% | 59.8% | 3.8% | 36.4% | 8 | 14.25 | 0 | 0.00 | 14.25 |
| map-api-composed | 8 | 83.2% | 6.5% | 10.3% | 59.8% | 3.8% | 36.4% | 8 | 14.25 | 0 | 0.00 | 14.25 |
| map-api-entry-control | 1 | 6.7% | 92.0% | 1.3% | 3.4% | 48.4% | 48.2% | 6 | 10.25 | 0 | 0.00 | 10.25 |
| map-api-entry-control | 8 | 6.7% | 92.0% | 1.3% | 3.4% | 48.4% | 48.2% | 6 | 10.25 | 0 | 0.00 | 10.25 |
| map-api-filter | 1 | 69.4% | 27.5% | 3.2% | 60.9% | 23.3% | 15.9% | 8 | 14.25 | 0 | 0.00 | 14.25 |
| map-api-filter | 8 | 69.4% | 27.5% | 3.2% | 60.9% | 23.3% | 15.9% | 8 | 14.25 | 0 | 0.00 | 14.25 |
| map-api-merge | 1 | 73.7% | 18.8% | 7.5% | 50.6% | 12.0% | 37.4% | 10 | 18.25 | 0 | 0.00 | 18.25 |
| map-api-merge | 8 | 73.7% | 18.8% | 7.5% | 50.6% | 12.0% | 37.4% | 10 | 18.25 | 0 | 0.00 | 18.25 |
| map-api-parallel | 1 | 75.6% | 14.1% | 10.3% | 57.7% | 5.9% | 36.4% | 12 | 22.25 | 0 | 0.00 | 22.25 |
| map-api-parallel | 8 | 74.0% | 15.7% | 10.3% | 56.0% | 7.6% | 36.4% | 25 | 36.00 | 0 | 0.00 | 36.00 |
| map-api-projections | 1 | 43.2% | 55.9% | 0.9% | 25.9% | 34.9% | 39.1% | 6 | 10.25 | 0 | 0.00 | 10.25 |
| map-api-projections | 8 | 43.2% | 55.9% | 0.9% | 25.9% | 34.9% | 39.1% | 6 | 10.25 | 0 | 0.00 | 10.25 |
| map-api-values | 1 | 70.2% | 15.5% | 14.3% | 45.3% | 9.1% | 45.6% | 6 | 10.25 | 0 | 0.00 | 10.25 |
| map-api-values | 8 | 70.2% | 15.5% | 14.3% | 45.3% | 9.1% | 45.6% | 6 | 10.25 | 0 | 0.00 | 10.25 |

Calibration: workload time medians; positive change means slower. Shared-host ranges are in summary.md.

| Case | Workers | Phase-off vs ordinary | Phase-only vs phase-off | Census vs phase-only |
|---|---:|---:|---:|---:|
| map-api-composed | 1 | +13.2% | -5.8% | +29.5% |
| map-api-composed | 8 | +6.8% | +0.2% | +26.1% |
| map-api-entry-control | 1 | +4.7% | +0.1% | +33.2% |
| map-api-entry-control | 8 | +1.9% | +4.4% | +25.8% |
| map-api-filter | 1 | +8.9% | -0.6% | +31.3% |
| map-api-filter | 8 | +5.2% | -2.9% | +34.5% |
| map-api-merge | 1 | +2.3% | +3.2% | +35.0% |
| map-api-merge | 8 | -0.6% | +3.9% | +30.6% |
| map-api-parallel | 1 | +2.8% | -2.3% | +31.8% |
| map-api-parallel | 8 | +7.2% | -4.0% | +54.0% |
| map-api-projections | 1 | +6.8% | -3.7% | +40.7% |
| map-api-projections | 8 | +3.0% | +0.5% | +35.6% |
| map-api-values | 1 | +8.8% | -4.1% | +30.5% |
| map-api-values | 8 | +2.6% | +0.9% | +26.9% |

Largest runtime allocation sites (byte shares pooled over repetitions):

| Case | Workers | Type | Runtime source | Share of requested bytes |
|---|---:|---|---|---:|
| map-api-composed | 1 | MapNode | hamt.rs:252 | 84.1% |
| map-api-composed | 1 | Tuple | lib.rs:250 | 8.5% |
| map-api-composed | 1 | MapNode | hamt.rs:186 | 4.2% |
| map-api-composed | 1 | Page | list.rs:326 | 3.1% |
| map-api-composed | 8 | MapNode | hamt.rs:252 | 84.1% |
| map-api-composed | 8 | Tuple | lib.rs:250 | 8.5% |
| map-api-composed | 8 | MapNode | hamt.rs:186 | 4.2% |
| map-api-composed | 8 | Page | list.rs:326 | 3.1% |
| map-api-entry-control | 1 | Page | list.rs:326 | 47.2% |
| map-api-entry-control | 1 | Tuple | lib.rs:250 | 46.6% |
| map-api-entry-control | 1 | MapNode | hamt.rs:252 | 3.8% |
| map-api-entry-control | 1 | PageDirectory | list.rs:430 | 0.8% |
| map-api-entry-control | 8 | Page | list.rs:326 | 47.2% |
| map-api-entry-control | 8 | Tuple | lib.rs:250 | 46.6% |
| map-api-entry-control | 8 | MapNode | hamt.rs:252 | 3.8% |
| map-api-entry-control | 8 | PageDirectory | list.rs:430 | 0.8% |
| map-api-filter | 1 | MapNode | hamt.rs:252 | 79.7% |
| map-api-filter | 1 | Tuple | lib.rs:250 | 13.4% |
| map-api-filter | 1 | Page | list.rs:326 | 4.5% |
| map-api-filter | 1 | MapNode | hamt.rs:186 | 2.0% |
| map-api-filter | 8 | MapNode | hamt.rs:252 | 79.7% |
| map-api-filter | 8 | Tuple | lib.rs:250 | 13.4% |
| map-api-filter | 8 | Page | list.rs:326 | 4.5% |
| map-api-filter | 8 | MapNode | hamt.rs:186 | 2.0% |
| map-api-merge | 1 | MapNode | hamt.rs:252 | 66.9% |
| map-api-merge | 1 | Tuple | lib.rs:250 | 11.7% |
| map-api-merge | 1 | String | string.rs:13 | 9.0% |
| map-api-merge | 1 | MapNode | hamt.rs:186 | 8.2% |
| map-api-merge | 8 | MapNode | hamt.rs:252 | 66.9% |
| map-api-merge | 8 | Tuple | lib.rs:250 | 11.7% |
| map-api-merge | 8 | String | string.rs:13 | 9.0% |
| map-api-merge | 8 | MapNode | hamt.rs:186 | 8.2% |
| map-api-parallel | 1 | MapNode | hamt.rs:252 | 84.1% |
| map-api-parallel | 1 | Tuple | lib.rs:250 | 8.3% |
| map-api-parallel | 1 | MapNode | hamt.rs:186 | 4.2% |
| map-api-parallel | 1 | Page | list.rs:326 | 3.1% |
| map-api-parallel | 8 | MapNode | hamt.rs:252 | 84.1% |
| map-api-parallel | 8 | Tuple | lib.rs:250 | 8.3% |
| map-api-parallel | 8 | MapNode | hamt.rs:186 | 4.2% |
| map-api-parallel | 8 | Page | list.rs:326 | 3.1% |
| map-api-projections | 1 | Tuple | lib.rs:250 | 57.3% |
| map-api-projections | 1 | Page | list.rs:326 | 38.6% |
| map-api-projections | 1 | MapNode | hamt.rs:252 | 2.4% |
| map-api-projections | 1 | PageDirectory | list.rs:430 | 0.7% |
| map-api-projections | 8 | Tuple | lib.rs:250 | 57.3% |
| map-api-projections | 8 | Page | list.rs:326 | 38.6% |
| map-api-projections | 8 | MapNode | hamt.rs:252 | 2.4% |
| map-api-projections | 8 | PageDirectory | list.rs:430 | 0.7% |
| map-api-values | 1 | MapNode | hamt.rs:252 | 87.0% |
| map-api-values | 1 | Tuple | lib.rs:250 | 7.7% |
| map-api-values | 1 | Page | list.rs:326 | 2.6% |
| map-api-values | 1 | MapNode | hamt.rs:186 | 2.4% |
| map-api-values | 8 | MapNode | hamt.rs:252 | 87.0% |
| map-api-values | 8 | Tuple | lib.rs:250 | 7.7% |
| map-api-values | 8 | Page | list.rs:326 | 2.6% |
| map-api-values | 8 | MapNode | hamt.rs:186 | 2.4% |
