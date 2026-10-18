Process-wide managed allocations, including startup and output encoding. Three repetitions unless noted in the raw report.
Requested bytes include headers. Cell bytes include pool rounding but exclude native buffers, allocator metadata and unused pool capacity.
Early reclamation and first survival are observed fractions of all allocations; unswept tail objects censor lifetime measurements.
Survival does not establish escape or unique ownership. Diagnostic timings/RSS include profiler overhead.

| Case | Workers | Objects | Requested MiB | Cell MiB | Early reclaimed bytes | First-survived bytes | Remaining bytes | Side-vector capacity MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| buffers-capacity-32 | 1 | 65,725 | 20.01 | 20.01 | 79.9% | 0.1% | 20.1% | 1.00 |
| buffers-capacity-32 | 2 | 65,725 | 20.01 | 20.01 | 79.9% | 0.1% | 20.1% | 1.00 |
| buffers-capacity-32 | 4 | 65,725 | 20.01 | 20.01 | 79.9% | 0.1% | 20.1% | 1.00 |
| buffers-capacity-32 | 8 | 65,725 | 20.01 | 20.01 | 79.9% | 0.1% | 20.1% | 1.00 |
| bytes-transient | 1 | 65,757 | 20.01 | 20.01 | 80.0% | 0.1% | 20.0% | 1.00 |
| bytes-transient | 2 | 65,757 | 20.01 | 20.01 | 80.0% | 0.1% | 20.0% | 1.00 |
| bytes-transient | 4 | 65,757 | 20.01 | 20.01 | 80.0% | 0.1% | 20.0% | 1.00 |
| bytes-transient | 8 | 65,757 | 20.01 | 20.01 | 80.0% | 0.1% | 20.0% | 1.00 |
| parallel-maps | 1 | 7,974 | 2.72 | 2.74 | 0.0% | 0.0% | 100.0% | 0.12 |
| parallel-maps | 2 | 7,974 | 2.72 | 2.74 | 0.0% | 0.0% | 100.0% | 0.14 |
| parallel-maps | 4 | 7,974 | 2.72 | 2.74 | 0.0% | 0.0% | 100.0% | 0.17 |
| parallel-maps | 8 | 7,974 | 2.72 | 2.74 | 0.0% | 0.0% | 100.0% | 0.14 |
| parallel-snapshots | 1 | 2,455,670 | 597.27 | 602.54 | 86.9% | 12.5% | 1.8% | 4.00 |
| parallel-snapshots | 2 | 2,455,670 | 597.27 | 602.54 | 85.9% | 13.8% | 0.9% | 4.00 |
| parallel-snapshots | 4 | 2,455,670 | 597.27 | 602.54 | 77.1% | 20.7% | 2.4% | 5.00 |
| parallel-snapshots | 8 | 2,455,670 | 597.27 | 602.54 | 69.6% | 29.8% | 5.7% | 8.00 |
| persistent-64 | 1 | 116 | 0.01 | 0.01 | 0.0% | 0.0% | 100.0% | 0.00 |
| persistent-64 | 2 | 116 | 0.01 | 0.01 | 0.0% | 0.0% | 100.0% | 0.00 |
| persistent-64 | 4 | 116 | 0.01 | 0.01 | 0.0% | 0.0% | 100.0% | 0.00 |
| persistent-64 | 8 | 116 | 0.01 | 0.01 | 0.0% | 0.0% | 100.0% | 0.00 |
| records-window | 1 | 6,016,562 | 256.86 | 256.86 | 99.5% | 0.4% | 0.4% | 8.00 |
| records-window | 2 | 6,016,562 | 256.86 | 256.86 | 99.5% | 0.4% | 0.4% | 8.00 |
| records-window | 4 | 6,016,562 | 256.86 | 256.86 | 99.5% | 0.4% | 0.4% | 8.00 |
| records-window | 8 | 6,016,562 | 256.86 | 256.86 | 99.5% | 0.4% | 0.4% | 8.00 |
| scalar-snapshots | 1 | 28,392 | 5.98 | 6.02 | 0.0% | 0.0% | 100.0% | 0.50 |
| scalar-snapshots | 2 | 28,392 | 5.98 | 6.02 | 0.0% | 0.0% | 100.0% | 0.50 |
| scalar-snapshots | 4 | 28,392 | 5.98 | 6.02 | 0.0% | 0.0% | 100.0% | 0.50 |
| scalar-snapshots | 8 | 28,392 | 5.98 | 6.02 | 0.0% | 0.0% | 100.0% | 0.50 |

Allocation paths: median process fractions of managed allocation requests; each reuse counts a new object, not a distinct cell.
Pool bytes below are native chunk reservations. They exclude system cells and other native storage.

| Case | Workers | Reuse objects | Bump objects | System objects | Reuse cell bytes | Bump cell bytes | System cell bytes | Refill chunks | Refill MiB | Trim chunks | Trim MiB | Reserved MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| buffers-capacity-32 | 1 | 19.9% | 80.1% | 0.0% | 20.0% | 80.0% | 0.0% | 9 | 16.25 | 0 | 0.00 | 16.25 |
| buffers-capacity-32 | 2 | 19.9% | 80.1% | 0.0% | 20.0% | 80.0% | 0.0% | 9 | 16.25 | 0 | 0.00 | 16.25 |
| buffers-capacity-32 | 4 | 19.9% | 80.1% | 0.0% | 20.0% | 80.0% | 0.0% | 9 | 16.25 | 0 | 0.00 | 16.25 |
| buffers-capacity-32 | 8 | 19.9% | 80.1% | 0.0% | 20.0% | 80.0% | 0.0% | 9 | 16.25 | 0 | 0.00 | 16.25 |
| bytes-transient | 1 | 19.9% | 80.1% | 0.0% | 20.0% | 80.0% | 0.0% | 9 | 16.25 | 0 | 0.00 | 16.25 |
| bytes-transient | 2 | 19.9% | 80.1% | 0.0% | 20.0% | 80.0% | 0.0% | 9 | 16.25 | 0 | 0.00 | 16.25 |
| bytes-transient | 4 | 19.9% | 80.1% | 0.0% | 20.0% | 80.0% | 0.0% | 9 | 16.25 | 0 | 0.00 | 16.25 |
| bytes-transient | 8 | 19.9% | 80.1% | 0.0% | 20.0% | 80.0% | 0.0% | 9 | 16.25 | 0 | 0.00 | 16.25 |
| parallel-maps | 1 | 0.0% | 73.8% | 26.2% | 0.0% | 54.8% | 45.2% | 2 | 2.25 | 0 | 0.00 | 2.25 |
| parallel-maps | 2 | 0.0% | 73.8% | 26.2% | 0.0% | 54.8% | 45.2% | 3 | 2.50 | 0 | 0.00 | 2.50 |
| parallel-maps | 4 | 0.0% | 73.8% | 26.2% | 0.0% | 54.8% | 45.2% | 5 | 3.00 | 0 | 0.00 | 3.00 |
| parallel-maps | 8 | 0.0% | 73.8% | 26.2% | 0.0% | 54.8% | 45.2% | 9 | 4.00 | 0 | 0.00 | 4.00 |
| parallel-snapshots | 1 | 67.0% | 11.9% | 21.1% | 38.0% | 6.0% | 56.1% | 20 | 38.25 | 5 | 10.00 | 28.25 |
| parallel-snapshots | 2 | 70.4% | 8.5% | 21.1% | 39.6% | 4.4% | 56.1% | 16 | 28.50 | 5 | 8.25 | 20.25 |
| parallel-snapshots | 4 | 68.0% | 10.8% | 21.1% | 38.4% | 5.5% | 56.1% | 23 | 39.00 | 13 | 20.75 | 18.25 |
| parallel-snapshots | 8 | 64.0% | 14.8% | 21.1% | 36.4% | 7.6% | 56.1% | 37 | 60.00 | 3 | 6.00 | 54.00 |
| persistent-64 | 1 | 0.0% | 100.0% | 0.0% | 0.0% | 100.0% | 0.0% | 1 | 0.25 | 0 | 0.00 | 0.25 |
| persistent-64 | 2 | 0.0% | 100.0% | 0.0% | 0.0% | 100.0% | 0.0% | 1 | 0.25 | 0 | 0.00 | 0.25 |
| persistent-64 | 4 | 0.0% | 100.0% | 0.0% | 0.0% | 100.0% | 0.0% | 1 | 0.25 | 0 | 0.00 | 0.25 |
| persistent-64 | 8 | 0.0% | 100.0% | 0.0% | 0.0% | 100.0% | 0.0% | 1 | 0.25 | 0 | 0.00 | 0.25 |
| records-window | 1 | 87.3% | 12.5% | 0.1% | 78.2% | 11.2% | 10.7% | 16 | 30.25 | 7 | 14.00 | 16.25 |
| records-window | 2 | 87.3% | 12.5% | 0.1% | 78.2% | 11.2% | 10.7% | 16 | 30.25 | 7 | 14.00 | 16.25 |
| records-window | 4 | 87.3% | 12.5% | 0.1% | 78.2% | 11.2% | 10.7% | 16 | 30.25 | 7 | 14.00 | 16.25 |
| records-window | 8 | 87.3% | 12.5% | 0.1% | 78.2% | 11.2% | 10.7% | 16 | 30.25 | 7 | 14.00 | 16.25 |
| scalar-snapshots | 1 | 0.0% | 100.0% | 0.0% | 0.0% | 99.5% | 0.5% | 4 | 6.25 | 0 | 0.00 | 6.25 |
| scalar-snapshots | 2 | 0.0% | 100.0% | 0.0% | 0.0% | 99.5% | 0.5% | 4 | 6.25 | 0 | 0.00 | 6.25 |
| scalar-snapshots | 4 | 0.0% | 100.0% | 0.0% | 0.0% | 99.5% | 0.5% | 4 | 6.25 | 0 | 0.00 | 6.25 |
| scalar-snapshots | 8 | 0.0% | 100.0% | 0.0% | 0.0% | 99.5% | 0.5% | 4 | 6.25 | 0 | 0.00 | 6.25 |

Calibration: workload time medians; positive change means slower. Shared-host ranges are in summary.md.

| Case | Workers | Phase-off vs ordinary | Phase-only vs phase-off | Census vs phase-only |
|---|---:|---:|---:|---:|
| buffers-capacity-32 | 1 | +2.3% | +5.2% | +8.1% |
| buffers-capacity-32 | 2 | +2.7% | +2.6% | +5.2% |
| buffers-capacity-32 | 4 | -1.0% | +2.6% | +8.2% |
| buffers-capacity-32 | 8 | +8.6% | -4.4% | +7.3% |
| bytes-transient | 1 | -4.8% | +13.1% | -8.0% |
| bytes-transient | 2 | +1.6% | +7.0% | +1.3% |
| bytes-transient | 4 | +1.1% | +2.8% | +11.1% |
| bytes-transient | 8 | +10.8% | -8.5% | +7.9% |
| parallel-maps | 1 | +2.4% | -4.2% | +0.2% |
| parallel-maps | 2 | -4.5% | -0.4% | +2.0% |
| parallel-maps | 4 | +0.5% | +1.3% | -2.2% |
| parallel-maps | 8 | -24.8% | +25.8% | -6.9% |
| parallel-snapshots | 1 | +4.6% | -1.1% | +30.8% |
| parallel-snapshots | 2 | +7.8% | -2.7% | +43.2% |
| parallel-snapshots | 4 | +2.6% | +3.3% | +28.4% |
| parallel-snapshots | 8 | +2.2% | +1.1% | +19.1% |
| persistent-64 | 1 | -1.5% | -1.1% | +3.7% |
| persistent-64 | 2 | -1.5% | -3.1% | +2.4% |
| persistent-64 | 4 | +0.2% | -1.5% | +1.3% |
| persistent-64 | 8 | -0.0% | -2.3% | +1.4% |
| records-window | 1 | +9.6% | +0.3% | +90.9% |
| records-window | 2 | +7.5% | -0.4% | +91.7% |
| records-window | 4 | +7.8% | -0.5% | +90.8% |
| records-window | 8 | +7.3% | -0.8% | +87.6% |
| scalar-snapshots | 1 | +2.6% | -0.8% | +12.6% |
| scalar-snapshots | 2 | +0.2% | -1.0% | +12.3% |
| scalar-snapshots | 4 | +2.3% | -0.1% | +9.3% |
| scalar-snapshots | 8 | +0.2% | -0.9% | +12.3% |

Largest runtime allocation sites (byte shares pooled over repetitions):

| Case | Workers | Type | Runtime source | Share of requested bytes |
|---|---:|---|---|---:|
| buffers-capacity-32 | 1 | Buffer | buffer.rs:212 | 100.0% |
| buffers-capacity-32 | 1 | MapNode | hamt.rs:252 | 0.0% |
| buffers-capacity-32 | 1 | Cell | abi.rs:14 | 0.0% |
| buffers-capacity-32 | 1 | String | string.rs:13 | 0.0% |
| buffers-capacity-32 | 2 | Buffer | buffer.rs:212 | 100.0% |
| buffers-capacity-32 | 2 | MapNode | hamt.rs:252 | 0.0% |
| buffers-capacity-32 | 2 | Cell | abi.rs:14 | 0.0% |
| buffers-capacity-32 | 2 | String | string.rs:13 | 0.0% |
| buffers-capacity-32 | 4 | Buffer | buffer.rs:212 | 100.0% |
| buffers-capacity-32 | 4 | MapNode | hamt.rs:252 | 0.0% |
| buffers-capacity-32 | 4 | Cell | abi.rs:14 | 0.0% |
| buffers-capacity-32 | 4 | String | string.rs:13 | 0.0% |
| buffers-capacity-32 | 8 | Buffer | buffer.rs:212 | 100.0% |
| buffers-capacity-32 | 8 | MapNode | hamt.rs:252 | 0.0% |
| buffers-capacity-32 | 8 | Cell | abi.rs:14 | 0.0% |
| buffers-capacity-32 | 8 | String | string.rs:13 | 0.0% |
| bytes-transient | 1 | Buffer | buffer.rs:212 | 100.0% |
| bytes-transient | 1 | MapNode | hamt.rs:252 | 0.0% |
| bytes-transient | 1 | PageDirectory | list.rs:349 | 0.0% |
| bytes-transient | 1 | List | list.rs:201 | 0.0% |
| bytes-transient | 2 | Buffer | buffer.rs:212 | 100.0% |
| bytes-transient | 2 | MapNode | hamt.rs:252 | 0.0% |
| bytes-transient | 2 | PageDirectory | list.rs:349 | 0.0% |
| bytes-transient | 2 | List | list.rs:201 | 0.0% |
| bytes-transient | 4 | Buffer | buffer.rs:212 | 100.0% |
| bytes-transient | 4 | MapNode | hamt.rs:252 | 0.0% |
| bytes-transient | 4 | PageDirectory | list.rs:349 | 0.0% |
| bytes-transient | 4 | List | list.rs:201 | 0.0% |
| bytes-transient | 8 | Buffer | buffer.rs:212 | 100.0% |
| bytes-transient | 8 | MapNode | hamt.rs:252 | 0.0% |
| bytes-transient | 8 | PageDirectory | list.rs:349 | 0.0% |
| bytes-transient | 8 | List | list.rs:201 | 0.0% |
| parallel-maps | 1 | MapNode | hamt.rs:252 | 97.0% |
| parallel-maps | 1 | MapNode | hamt.rs:186 | 2.8% |
| parallel-maps | 1 | MapCursor | hamt.rs:601 | 0.1% |
| parallel-maps | 1 | String | string.rs:13 | 0.0% |
| parallel-maps | 2 | MapNode | hamt.rs:252 | 97.0% |
| parallel-maps | 2 | MapNode | hamt.rs:186 | 2.8% |
| parallel-maps | 2 | MapCursor | hamt.rs:601 | 0.1% |
| parallel-maps | 2 | String | string.rs:13 | 0.0% |
| parallel-maps | 4 | MapNode | hamt.rs:252 | 97.0% |
| parallel-maps | 4 | MapNode | hamt.rs:186 | 2.8% |
| parallel-maps | 4 | MapCursor | hamt.rs:601 | 0.1% |
| parallel-maps | 4 | String | string.rs:13 | 0.0% |
| parallel-maps | 8 | MapNode | hamt.rs:252 | 97.0% |
| parallel-maps | 8 | MapNode | hamt.rs:186 | 2.8% |
| parallel-maps | 8 | MapCursor | hamt.rs:601 | 0.1% |
| parallel-maps | 8 | String | string.rs:13 | 0.0% |
| parallel-snapshots | 1 | MapNode | hamt.rs:252 | 82.1% |
| parallel-snapshots | 1 | String | string.rs:13 | 15.2% |
| parallel-snapshots | 1 | Map | map.rs:105 | 2.7% |
| parallel-snapshots | 1 | MapNode | hamt.rs:186 | 0.0% |
| parallel-snapshots | 2 | MapNode | hamt.rs:252 | 82.1% |
| parallel-snapshots | 2 | String | string.rs:13 | 15.2% |
| parallel-snapshots | 2 | Map | map.rs:105 | 2.7% |
| parallel-snapshots | 2 | MapNode | hamt.rs:186 | 0.0% |
| parallel-snapshots | 4 | MapNode | hamt.rs:252 | 82.1% |
| parallel-snapshots | 4 | String | string.rs:13 | 15.2% |
| parallel-snapshots | 4 | Map | map.rs:105 | 2.7% |
| parallel-snapshots | 4 | MapNode | hamt.rs:186 | 0.0% |
| parallel-snapshots | 8 | MapNode | hamt.rs:252 | 82.1% |
| parallel-snapshots | 8 | String | string.rs:13 | 15.2% |
| parallel-snapshots | 8 | Map | map.rs:105 | 2.7% |
| parallel-snapshots | 8 | MapNode | hamt.rs:186 | 0.0% |
| persistent-64 | 1 | MapNode | hamt.rs:252 | 65.0% |
| persistent-64 | 1 | MapNode | hamt.rs:186 | 14.0% |
| persistent-64 | 1 | String | string.rs:13 | 10.7% |
| persistent-64 | 1 | Data | lib.rs:165 | 5.0% |
| persistent-64 | 2 | MapNode | hamt.rs:252 | 65.0% |
| persistent-64 | 2 | MapNode | hamt.rs:186 | 14.0% |
| persistent-64 | 2 | String | string.rs:13 | 10.7% |
| persistent-64 | 2 | Data | lib.rs:165 | 5.0% |
| persistent-64 | 4 | MapNode | hamt.rs:252 | 65.0% |
| persistent-64 | 4 | MapNode | hamt.rs:186 | 14.0% |
| persistent-64 | 4 | String | string.rs:13 | 10.7% |
| persistent-64 | 4 | Data | lib.rs:165 | 5.0% |
| persistent-64 | 8 | MapNode | hamt.rs:252 | 65.0% |
| persistent-64 | 8 | MapNode | hamt.rs:186 | 14.0% |
| persistent-64 | 8 | String | string.rs:13 | 10.7% |
| persistent-64 | 8 | Data | lib.rs:165 | 5.0% |
| records-window | 1 | List | list.rs:492 | 53.4% |
| records-window | 1 | Record | abi.rs:14 | 35.6% |
| records-window | 1 | Page | list.rs:326 | 10.7% |
| records-window | 1 | PageDirectory | list.rs:430 | 0.1% |
| records-window | 2 | List | list.rs:492 | 53.4% |
| records-window | 2 | Record | abi.rs:14 | 35.6% |
| records-window | 2 | Page | list.rs:326 | 10.7% |
| records-window | 2 | PageDirectory | list.rs:430 | 0.1% |
| records-window | 4 | List | list.rs:492 | 53.4% |
| records-window | 4 | Record | abi.rs:14 | 35.6% |
| records-window | 4 | Page | list.rs:326 | 10.7% |
| records-window | 4 | PageDirectory | list.rs:430 | 0.1% |
| records-window | 8 | List | list.rs:492 | 53.4% |
| records-window | 8 | Record | abi.rs:14 | 35.6% |
| records-window | 8 | Page | list.rs:326 | 10.7% |
| records-window | 8 | PageDirectory | list.rs:430 | 0.1% |
| scalar-snapshots | 1 | MapNode | hamt.rs:252 | 94.5% |
| scalar-snapshots | 1 | MapCursor | hamt.rs:601 | 4.3% |
| scalar-snapshots | 1 | Map | map.rs:173 | 0.7% |
| scalar-snapshots | 1 | Page | list.rs:326 | 0.5% |
| scalar-snapshots | 2 | MapNode | hamt.rs:252 | 94.5% |
| scalar-snapshots | 2 | MapCursor | hamt.rs:601 | 4.3% |
| scalar-snapshots | 2 | Map | map.rs:173 | 0.7% |
| scalar-snapshots | 2 | Page | list.rs:326 | 0.5% |
| scalar-snapshots | 4 | MapNode | hamt.rs:252 | 94.5% |
| scalar-snapshots | 4 | MapCursor | hamt.rs:601 | 4.3% |
| scalar-snapshots | 4 | Map | map.rs:173 | 0.7% |
| scalar-snapshots | 4 | Page | list.rs:326 | 0.5% |
| scalar-snapshots | 8 | MapNode | hamt.rs:252 | 94.5% |
| scalar-snapshots | 8 | MapCursor | hamt.rs:601 | 4.3% |
| scalar-snapshots | 8 | Map | map.rs:173 | 0.7% |
| scalar-snapshots | 8 | Page | list.rs:326 | 0.5% |
