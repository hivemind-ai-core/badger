Process-wide managed allocations, including startup and output encoding. Three repetitions unless noted in the raw report.
Requested bytes include headers. Cell bytes include pool rounding but exclude native buffers, allocator metadata and unused pool capacity.
Early reclamation and first survival are observed fractions of all allocations; unswept tail objects censor lifetime measurements.
Survival does not establish escape or unique ownership. Diagnostic timings/RSS include profiler overhead.

| Case | Workers | Objects | Requested MiB | Cell MiB | Early reclaimed bytes | First-survived bytes | Remaining bytes | Side-vector capacity MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| messages | 1 | 1,589,640 | 48.40 | 48.53 | 90.5% | 0.0% | 9.5% | 8.00 |
| messages | 2 | 1,589,640 | 48.40 | 48.53 | 90.5% | 0.0% | 9.5% | 16.00 |
| messages | 4 | 1,589,640 | 48.40 | 48.53 | 90.5% | 0.0% | 9.5% | 16.06 |
| messages | 8 | 1,589,640 | 48.40 | 48.53 | 90.5% | 0.0% | 9.5% | 18.31 |
| messages-fanout | 1 | 2,638,537 | 80.42 | 80.55 | 94.0% | 0.0% | 6.0% | 8.00 |
| messages-fanout | 2 | 2,638,537 | 80.42 | 80.55 | 94.2% | 0.0% | 5.8% | 8.00 |
| messages-fanout | 4 | 2,638,537 | 80.42 | 80.55 | 94.6% | 0.0% | 5.4% | 16.00 |
| messages-fanout | 8 | 2,638,537 | 80.42 | 80.55 | 95.3% | 0.0% | 4.7% | 21.00 |
| messages-int | 1 | 16,776 | 0.40 | 0.53 | 0.0% | 0.0% | 100.0% | 0.50 |
| messages-int | 2 | 16,776 | 0.40 | 0.53 | 0.0% | 0.0% | 100.0% | 0.50 |
| messages-int | 4 | 16,776 | 0.40 | 0.53 | 0.0% | 0.0% | 100.0% | 0.50 |
| messages-int | 8 | 16,776 | 0.40 | 0.53 | 0.0% | 0.0% | 100.0% | 0.50 |
| messages-int-fanout | 1 | 17,097 | 0.42 | 0.55 | 0.0% | 0.0% | 100.0% | 0.50 |
| messages-int-fanout | 2 | 17,097 | 0.42 | 0.55 | 0.0% | 0.0% | 100.0% | 0.38 |
| messages-int-fanout | 4 | 17,097 | 0.42 | 0.55 | 0.0% | 0.0% | 100.0% | 0.44 |
| messages-int-fanout | 8 | 17,097 | 0.42 | 0.55 | 0.0% | 0.0% | 100.0% | 0.36 |
| messages-int-retained | 1 | 17,736 | 0.46 | 0.59 | 0.0% | 0.0% | 100.0% | 0.50 |
| messages-int-retained | 2 | 17,736 | 0.46 | 0.59 | 0.0% | 0.0% | 100.0% | 0.51 |
| messages-int-retained | 4 | 17,736 | 0.46 | 0.59 | 0.0% | 0.0% | 100.0% | 0.51 |
| messages-int-retained | 8 | 17,736 | 0.46 | 0.59 | 0.0% | 0.0% | 100.0% | 0.51 |
| messages-int-slow | 1 | 16,461 | 0.38 | 0.51 | 0.0% | 0.0% | 100.0% | 0.50 |
| messages-int-slow | 2 | 16,461 | 0.38 | 0.51 | 0.0% | 0.0% | 100.0% | 0.28 |
| messages-int-slow | 4 | 16,461 | 0.38 | 0.51 | 0.0% | 0.0% | 100.0% | 0.38 |
| messages-int-slow | 8 | 16,461 | 0.38 | 0.51 | 0.0% | 0.0% | 100.0% | 0.38 |
| messages-retained | 1 | 1,598,792 | 48.71 | 48.84 | 89.6% | 0.1% | 10.4% | 8.00 |
| messages-retained | 2 | 1,598,792 | 48.71 | 48.84 | 89.9% | 0.1% | 10.1% | 16.00 |
| messages-retained | 4 | 1,598,792 | 48.71 | 48.84 | 90.3% | 0.1% | 9.7% | 17.50 |
| messages-retained | 8 | 1,598,792 | 48.71 | 48.84 | 90.4% | 0.1% | 9.6% | 21.50 |
| messages-slow | 1 | 1,589,325 | 48.38 | 48.51 | 90.5% | 0.0% | 9.5% | 8.00 |
| messages-slow | 2 | 1,589,325 | 48.38 | 48.51 | 90.5% | 0.0% | 9.5% | 12.00 |
| messages-slow | 4 | 1,589,325 | 48.38 | 48.51 | 91.0% | 0.0% | 9.0% | 12.00 |
| messages-slow | 8 | 1,589,325 | 48.38 | 48.51 | 91.7% | 0.0% | 8.3% | 14.00 |

Calibration: workload time medians; positive change means slower. Shared-host ranges are in summary.md.

| Case | Workers | Phase-off vs ordinary | Phase-only vs phase-off | Census vs phase-only |
|---|---:|---:|---:|---:|
| messages | 1 | -0.1% | +1.4% | +104.7% |
| messages | 2 | +3.7% | -1.0% | +115.5% |
| messages | 4 | -2.3% | +5.8% | +113.1% |
| messages | 8 | -11.7% | +15.4% | +105.3% |
| messages-fanout | 1 | +2.5% | -0.0% | +109.9% |
| messages-fanout | 2 | +1.8% | +1.6% | +79.7% |
| messages-fanout | 4 | -3.2% | +7.8% | +109.7% |
| messages-fanout | 8 | +10.0% | -3.4% | +108.0% |
| messages-int | 1 | -4.3% | +1.7% | +9.4% |
| messages-int | 2 | +61.1% | +1.7% | +7.7% |
| messages-int | 4 | -0.5% | +19.6% | -11.5% |
| messages-int | 8 | -8.2% | +0.4% | +13.2% |
| messages-int-fanout | 1 | +0.1% | +0.1% | +5.2% |
| messages-int-fanout | 2 | +6.5% | -0.4% | +7.6% |
| messages-int-fanout | 4 | -7.6% | -1.8% | +14.4% |
| messages-int-fanout | 8 | +11.1% | -2.7% | +1.3% |
| messages-int-retained | 1 | -2.0% | +1.9% | +6.0% |
| messages-int-retained | 2 | -12.1% | +11.1% | -1.2% |
| messages-int-retained | 4 | -3.6% | +10.8% | -2.6% |
| messages-int-retained | 8 | -0.3% | +12.5% | -2.1% |
| messages-int-slow | 1 | -0.7% | +1.6% | +4.7% |
| messages-int-slow | 2 | +9.1% | -2.7% | +30.0% |
| messages-int-slow | 4 | +0.1% | -0.9% | +22.2% |
| messages-int-slow | 8 | +6.9% | -9.6% | +9.9% |
| messages-retained | 1 | +1.4% | +4.2% | +98.0% |
| messages-retained | 2 | +7.9% | -3.6% | +94.7% |
| messages-retained | 4 | -2.6% | +15.7% | +99.7% |
| messages-retained | 8 | +6.1% | +6.9% | +113.1% |
| messages-slow | 1 | +0.3% | -4.4% | +104.0% |
| messages-slow | 2 | +12.2% | -0.4% | +102.0% |
| messages-slow | 4 | +6.8% | +1.1% | +82.5% |
| messages-slow | 8 | -3.6% | +0.3% | +93.5% |

Largest runtime allocation sites (byte shares pooled over repetitions):

| Case | Workers | Type | Runtime source | Share of requested bytes |
|---|---:|---|---|---:|
| messages | 1 | Result | abi.rs:14 | 99.2% |
| messages | 1 | Buffer | buffer.rs:164 | 0.8% |
| messages | 1 | Page | list.rs:326 | 0.0% |
| messages | 1 | PageDirectory | list.rs:349 | 0.0% |
| messages | 2 | Result | abi.rs:14 | 99.2% |
| messages | 2 | Buffer | buffer.rs:164 | 0.8% |
| messages | 2 | Page | list.rs:326 | 0.0% |
| messages | 2 | PageDirectory | list.rs:349 | 0.0% |
| messages | 4 | Result | abi.rs:14 | 99.2% |
| messages | 4 | Buffer | buffer.rs:164 | 0.8% |
| messages | 4 | Page | list.rs:326 | 0.0% |
| messages | 4 | PageDirectory | list.rs:349 | 0.0% |
| messages | 8 | Result | abi.rs:14 | 99.2% |
| messages | 8 | Buffer | buffer.rs:164 | 0.8% |
| messages | 8 | Page | list.rs:326 | 0.0% |
| messages | 8 | PageDirectory | list.rs:349 | 0.0% |
| messages-fanout | 1 | Result | abi.rs:14 | 99.5% |
| messages-fanout | 1 | Buffer | buffer.rs:164 | 0.5% |
| messages-fanout | 1 | Page | list.rs:326 | 0.0% |
| messages-fanout | 1 | PageDirectory | list.rs:349 | 0.0% |
| messages-fanout | 2 | Result | abi.rs:14 | 99.5% |
| messages-fanout | 2 | Buffer | buffer.rs:164 | 0.5% |
| messages-fanout | 2 | Page | list.rs:326 | 0.0% |
| messages-fanout | 2 | PageDirectory | list.rs:349 | 0.0% |
| messages-fanout | 4 | Result | abi.rs:14 | 99.5% |
| messages-fanout | 4 | Buffer | buffer.rs:164 | 0.5% |
| messages-fanout | 4 | Page | list.rs:326 | 0.0% |
| messages-fanout | 4 | PageDirectory | list.rs:349 | 0.0% |
| messages-fanout | 8 | Result | abi.rs:14 | 99.5% |
| messages-fanout | 8 | Buffer | buffer.rs:164 | 0.5% |
| messages-fanout | 8 | Page | list.rs:326 | 0.0% |
| messages-fanout | 8 | PageDirectory | list.rs:349 | 0.0% |
| messages-int | 1 | Buffer | buffer.rs:164 | 94.3% |
| messages-int | 1 | Page | list.rs:326 | 2.4% |
| messages-int | 1 | PageDirectory | list.rs:349 | 1.0% |
| messages-int | 1 | List | list.rs:201 | 0.7% |
| messages-int | 2 | Buffer | buffer.rs:164 | 94.3% |
| messages-int | 2 | Page | list.rs:326 | 2.4% |
| messages-int | 2 | PageDirectory | list.rs:349 | 1.0% |
| messages-int | 2 | List | list.rs:201 | 0.7% |
| messages-int | 4 | Buffer | buffer.rs:164 | 94.3% |
| messages-int | 4 | Page | list.rs:326 | 2.4% |
| messages-int | 4 | PageDirectory | list.rs:349 | 1.0% |
| messages-int | 4 | List | list.rs:201 | 0.7% |
| messages-int | 8 | Buffer | buffer.rs:164 | 94.3% |
| messages-int | 8 | Page | list.rs:326 | 2.4% |
| messages-int | 8 | PageDirectory | list.rs:349 | 1.0% |
| messages-int | 8 | List | list.rs:201 | 0.7% |
| messages-int-fanout | 1 | Buffer | buffer.rs:164 | 90.3% |
| messages-int-fanout | 1 | Page | list.rs:326 | 4.7% |
| messages-int-fanout | 1 | PageDirectory | list.rs:349 | 1.9% |
| messages-int-fanout | 1 | List | list.rs:201 | 1.4% |
| messages-int-fanout | 2 | Buffer | buffer.rs:164 | 90.3% |
| messages-int-fanout | 2 | Page | list.rs:326 | 4.7% |
| messages-int-fanout | 2 | PageDirectory | list.rs:349 | 1.9% |
| messages-int-fanout | 2 | List | list.rs:201 | 1.4% |
| messages-int-fanout | 4 | Buffer | buffer.rs:164 | 90.3% |
| messages-int-fanout | 4 | Page | list.rs:326 | 4.7% |
| messages-int-fanout | 4 | PageDirectory | list.rs:349 | 1.9% |
| messages-int-fanout | 4 | List | list.rs:201 | 1.4% |
| messages-int-fanout | 8 | Buffer | buffer.rs:164 | 90.3% |
| messages-int-fanout | 8 | Page | list.rs:326 | 4.7% |
| messages-int-fanout | 8 | PageDirectory | list.rs:349 | 1.9% |
| messages-int-fanout | 8 | List | list.rs:201 | 1.4% |
| messages-int-retained | 1 | Buffer | buffer.rs:164 | 83.2% |
| messages-int-retained | 1 | Page | list.rs:326 | 8.5% |
| messages-int-retained | 1 | PageDirectory | list.rs:349 | 3.4% |
| messages-int-retained | 1 | List | list.rs:201 | 2.6% |
| messages-int-retained | 2 | Buffer | buffer.rs:164 | 83.2% |
| messages-int-retained | 2 | Page | list.rs:326 | 8.5% |
| messages-int-retained | 2 | PageDirectory | list.rs:349 | 3.4% |
| messages-int-retained | 2 | List | list.rs:201 | 2.6% |
| messages-int-retained | 4 | Buffer | buffer.rs:164 | 83.2% |
| messages-int-retained | 4 | Page | list.rs:326 | 8.5% |
| messages-int-retained | 4 | PageDirectory | list.rs:349 | 3.4% |
| messages-int-retained | 4 | List | list.rs:201 | 2.6% |
| messages-int-retained | 8 | Buffer | buffer.rs:164 | 83.2% |
| messages-int-retained | 8 | Page | list.rs:326 | 8.5% |
| messages-int-retained | 8 | PageDirectory | list.rs:349 | 3.4% |
| messages-int-retained | 8 | List | list.rs:201 | 2.6% |
| messages-int-slow | 1 | Buffer | buffer.rs:164 | 98.7% |
| messages-int-slow | 1 | MapNode | hamt.rs:159 | 0.5% |
| messages-int-slow | 1 | String | string.rs:13 | 0.3% |
| messages-int-slow | 1 | Data | lib.rs:143 | 0.1% |
| messages-int-slow | 2 | Buffer | buffer.rs:164 | 98.7% |
| messages-int-slow | 2 | MapNode | hamt.rs:159 | 0.5% |
| messages-int-slow | 2 | String | string.rs:13 | 0.3% |
| messages-int-slow | 2 | Data | lib.rs:143 | 0.1% |
| messages-int-slow | 4 | Buffer | buffer.rs:164 | 98.7% |
| messages-int-slow | 4 | MapNode | hamt.rs:159 | 0.5% |
| messages-int-slow | 4 | String | string.rs:13 | 0.3% |
| messages-int-slow | 4 | Data | lib.rs:143 | 0.1% |
| messages-int-slow | 8 | Buffer | buffer.rs:164 | 98.7% |
| messages-int-slow | 8 | MapNode | hamt.rs:159 | 0.5% |
| messages-int-slow | 8 | String | string.rs:13 | 0.3% |
| messages-int-slow | 8 | Data | lib.rs:143 | 0.1% |
| messages-retained | 1 | Result | abi.rs:14 | 99.1% |
| messages-retained | 1 | Buffer | buffer.rs:164 | 0.8% |
| messages-retained | 1 | Page | list.rs:326 | 0.1% |
| messages-retained | 1 | PageDirectory | list.rs:349 | 0.0% |
| messages-retained | 2 | Result | abi.rs:14 | 99.1% |
| messages-retained | 2 | Buffer | buffer.rs:164 | 0.8% |
| messages-retained | 2 | Page | list.rs:326 | 0.1% |
| messages-retained | 2 | PageDirectory | list.rs:349 | 0.0% |
| messages-retained | 4 | Result | abi.rs:14 | 99.1% |
| messages-retained | 4 | Buffer | buffer.rs:164 | 0.8% |
| messages-retained | 4 | Page | list.rs:326 | 0.1% |
| messages-retained | 4 | PageDirectory | list.rs:349 | 0.0% |
| messages-retained | 8 | Result | abi.rs:14 | 99.1% |
| messages-retained | 8 | Buffer | buffer.rs:164 | 0.8% |
| messages-retained | 8 | Page | list.rs:326 | 0.1% |
| messages-retained | 8 | PageDirectory | list.rs:349 | 0.0% |
| messages-slow | 1 | Result | abi.rs:14 | 99.2% |
| messages-slow | 1 | Buffer | buffer.rs:164 | 0.8% |
| messages-slow | 1 | MapNode | hamt.rs:159 | 0.0% |
| messages-slow | 1 | String | string.rs:13 | 0.0% |
| messages-slow | 2 | Result | abi.rs:14 | 99.2% |
| messages-slow | 2 | Buffer | buffer.rs:164 | 0.8% |
| messages-slow | 2 | MapNode | hamt.rs:159 | 0.0% |
| messages-slow | 2 | String | string.rs:13 | 0.0% |
| messages-slow | 4 | Result | abi.rs:14 | 99.2% |
| messages-slow | 4 | Buffer | buffer.rs:164 | 0.8% |
| messages-slow | 4 | MapNode | hamt.rs:159 | 0.0% |
| messages-slow | 4 | String | string.rs:13 | 0.0% |
| messages-slow | 8 | Result | abi.rs:14 | 99.2% |
| messages-slow | 8 | Buffer | buffer.rs:164 | 0.8% |
| messages-slow | 8 | MapNode | hamt.rs:159 | 0.0% |
| messages-slow | 8 | String | string.rs:13 | 0.0% |
