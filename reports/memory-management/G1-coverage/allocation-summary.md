Process-wide managed allocations, including startup and output encoding. Three repetitions unless noted in the raw report.
Requested bytes include headers. Cell bytes include pool rounding but exclude native buffers, allocator metadata and unused pool capacity.
Early reclamation and first survival are observed fractions of all allocations; unswept tail objects censor lifetime measurements.
Survival does not establish escape or unique ownership. Diagnostic timings/RSS include profiler overhead.

| Case | Workers | Objects | Requested MiB | Cell MiB | Early reclaimed bytes | First-survived bytes | Remaining bytes | Side-vector capacity MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| bytes-survivors | 1 | 237,628 | 7.13 | 7.75 | 61.4% | 5.3% | 38.6% | 4.00 |
| bytes-survivors | 2 | 237,628 | 7.13 | 7.75 | 61.4% | 5.3% | 38.6% | 4.00 |
| bytes-survivors | 4 | 237,628 | 7.13 | 7.75 | 61.4% | 5.3% | 38.6% | 4.00 |
| bytes-survivors | 8 | 237,628 | 7.13 | 7.75 | 61.4% | 5.3% | 38.6% | 4.00 |
| bytes-transient | 1 | 196,828 | 5.51 | 6.01 | 65.4% | 0.0% | 34.6% | 2.00 |
| bytes-transient | 2 | 196,828 | 5.51 | 6.01 | 65.4% | 0.0% | 34.6% | 2.00 |
| bytes-transient | 4 | 196,828 | 5.51 | 6.01 | 65.4% | 0.0% | 34.6% | 2.00 |
| bytes-transient | 8 | 196,828 | 5.51 | 6.01 | 65.4% | 0.0% | 34.6% | 2.00 |
| messages | 1 | 1,589,640 | 48.40 | 48.53 | 90.5% | 0.0% | 9.5% | 8.00 |
| messages | 2 | 1,589,640 | 48.40 | 48.53 | 90.6% | 0.0% | 9.4% | 16.00 |
| messages | 4 | 1,589,640 | 48.40 | 48.53 | 90.6% | 0.0% | 9.4% | 24.06 |
| messages | 8 | 1,589,640 | 48.40 | 48.53 | 90.6% | 0.0% | 9.4% | 24.31 |
| messages-fanout | 1 | 2,638,537 | 80.42 | 80.55 | 94.0% | 0.0% | 6.0% | 8.00 |
| messages-fanout | 2 | 2,638,537 | 80.42 | 80.55 | 94.2% | 0.0% | 5.8% | 8.00 |
| messages-fanout | 4 | 2,638,537 | 80.42 | 80.55 | 94.7% | 0.0% | 5.3% | 16.00 |
| messages-fanout | 8 | 2,638,537 | 80.42 | 80.55 | 95.1% | 0.0% | 4.9% | 23.00 |
| messages-retained | 1 | 1,598,792 | 48.71 | 48.84 | 89.6% | 0.1% | 10.4% | 8.00 |
| messages-retained | 2 | 1,598,792 | 48.71 | 48.84 | 89.8% | 0.1% | 10.2% | 16.00 |
| messages-retained | 4 | 1,598,792 | 48.71 | 48.84 | 90.1% | 0.1% | 9.9% | 22.00 |
| messages-retained | 8 | 1,598,792 | 48.71 | 48.84 | 90.5% | 0.1% | 9.5% | 26.00 |
| messages-slow | 1 | 1,589,325 | 48.38 | 48.51 | 90.5% | 0.0% | 9.5% | 8.00 |
| messages-slow | 2 | 1,589,325 | 48.38 | 48.51 | 90.5% | 0.0% | 9.5% | 16.00 |
| messages-slow | 4 | 1,589,325 | 48.38 | 48.51 | 90.9% | 0.0% | 9.1% | 12.00 |
| messages-slow | 8 | 1,589,325 | 48.38 | 48.51 | 91.6% | 0.0% | 8.4% | 12.00 |
| snapshots | 1 | 339,711 | 72.87 | 73.31 | 83.1% | 5.0% | 16.9% | 2.00 |
| snapshots | 2 | 339,711 | 72.87 | 73.31 | 83.1% | 5.0% | 16.9% | 2.00 |
| snapshots | 4 | 339,711 | 72.87 | 73.31 | 83.1% | 5.0% | 16.9% | 2.00 |
| snapshots | 8 | 339,711 | 72.87 | 73.31 | 83.1% | 5.0% | 16.9% | 2.00 |
| words-transient | 1 | 196,828 | 5.51 | 6.01 | 93.7% | 0.1% | 6.3% | 0.50 |
| words-transient | 2 | 196,828 | 5.51 | 6.01 | 93.7% | 0.1% | 6.3% | 0.50 |
| words-transient | 4 | 196,828 | 5.51 | 6.01 | 93.7% | 0.1% | 6.3% | 0.50 |
| words-transient | 8 | 196,828 | 5.51 | 6.01 | 93.7% | 0.1% | 6.3% | 0.50 |

Calibration: workload time medians; positive change means slower. Shared-host ranges are in summary.md.

| Case | Workers | Phase-off vs ordinary | Phase-only vs phase-off | Census vs phase-only |
|---|---:|---:|---:|---:|
| bytes-survivors | 1 | -2.6% | +2.5% | +6.4% |
| bytes-survivors | 2 | -5.3% | -2.9% | +11.4% |
| bytes-survivors | 4 | -8.7% | +1.2% | +3.2% |
| bytes-survivors | 8 | -9.4% | -1.1% | +7.2% |
| bytes-transient | 1 | -7.4% | -3.7% | +7.7% |
| bytes-transient | 2 | -6.6% | +2.2% | +3.3% |
| bytes-transient | 4 | +2.4% | -7.3% | +1.3% |
| bytes-transient | 8 | -6.3% | +0.5% | +7.9% |
| messages | 1 | +1.1% | +3.9% | +95.9% |
| messages | 2 | +3.1% | -7.4% | +120.8% |
| messages | 4 | -2.8% | +12.2% | +118.9% |
| messages | 8 | +16.8% | -7.9% | +125.5% |
| messages-fanout | 1 | +0.7% | +6.1% | +90.9% |
| messages-fanout | 2 | +7.5% | -3.0% | +80.0% |
| messages-fanout | 4 | -0.5% | +2.6% | +116.9% |
| messages-fanout | 8 | +2.4% | +5.9% | +94.9% |
| messages-retained | 1 | +5.6% | -5.4% | +102.4% |
| messages-retained | 2 | +6.4% | +1.0% | +95.4% |
| messages-retained | 4 | +4.6% | -4.2% | +120.7% |
| messages-retained | 8 | -0.3% | +8.1% | +111.6% |
| messages-slow | 1 | -0.9% | +4.9% | +88.2% |
| messages-slow | 2 | +0.4% | -3.2% | +122.0% |
| messages-slow | 4 | +5.4% | -8.6% | +98.4% |
| messages-slow | 8 | +6.5% | -8.3% | +94.9% |
| snapshots | 1 | -0.9% | -2.6% | +31.9% |
| snapshots | 2 | -2.2% | -1.7% | +26.3% |
| snapshots | 4 | -2.9% | +1.6% | +32.0% |
| snapshots | 8 | -1.6% | -0.6% | +19.7% |
| words-transient | 1 | +3.1% | +2.0% | +5.7% |
| words-transient | 2 | -0.3% | -3.2% | +3.7% |
| words-transient | 4 | -0.7% | +0.3% | +0.7% |
| words-transient | 8 | +4.8% | -1.9% | +6.2% |

Largest runtime allocation sites (byte shares pooled over repetitions):

| Case | Workers | Type | Runtime source | Share of requested bytes |
|---|---:|---|---|---:|
| bytes-survivors | 1 | Result | abi.rs:14 | 56.1% |
| bytes-survivors | 1 | Buffer | buffer.rs:164 | 23.7% |
| bytes-survivors | 1 | PageDirectory | list.rs:349 | 7.0% |
| bytes-survivors | 1 | List | list.rs:201 | 5.3% |
| bytes-survivors | 2 | Result | abi.rs:14 | 56.1% |
| bytes-survivors | 2 | Buffer | buffer.rs:164 | 23.7% |
| bytes-survivors | 2 | PageDirectory | list.rs:349 | 7.0% |
| bytes-survivors | 2 | List | list.rs:201 | 5.3% |
| bytes-survivors | 4 | Result | abi.rs:14 | 56.1% |
| bytes-survivors | 4 | Buffer | buffer.rs:164 | 23.7% |
| bytes-survivors | 4 | PageDirectory | list.rs:349 | 7.0% |
| bytes-survivors | 4 | List | list.rs:201 | 5.3% |
| bytes-survivors | 8 | Result | abi.rs:14 | 56.1% |
| bytes-survivors | 8 | Buffer | buffer.rs:164 | 23.7% |
| bytes-survivors | 8 | PageDirectory | list.rs:349 | 7.0% |
| bytes-survivors | 8 | List | list.rs:201 | 5.3% |
| bytes-transient | 1 | Result | abi.rs:14 | 72.6% |
| bytes-transient | 1 | Buffer | buffer.rs:164 | 27.2% |
| bytes-transient | 1 | MapNode | hamt.rs:159 | 0.0% |
| bytes-transient | 1 | PageDirectory | list.rs:349 | 0.0% |
| bytes-transient | 2 | Result | abi.rs:14 | 72.6% |
| bytes-transient | 2 | Buffer | buffer.rs:164 | 27.2% |
| bytes-transient | 2 | MapNode | hamt.rs:159 | 0.0% |
| bytes-transient | 2 | PageDirectory | list.rs:349 | 0.0% |
| bytes-transient | 4 | Result | abi.rs:14 | 72.6% |
| bytes-transient | 4 | Buffer | buffer.rs:164 | 27.2% |
| bytes-transient | 4 | MapNode | hamt.rs:159 | 0.0% |
| bytes-transient | 4 | PageDirectory | list.rs:349 | 0.0% |
| bytes-transient | 8 | Result | abi.rs:14 | 72.6% |
| bytes-transient | 8 | Buffer | buffer.rs:164 | 27.2% |
| bytes-transient | 8 | MapNode | hamt.rs:159 | 0.0% |
| bytes-transient | 8 | PageDirectory | list.rs:349 | 0.0% |
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
| snapshots | 1 | MapNode | hamt.rs:159 | 84.1% |
| snapshots | 1 | String | string.rs:13 | 10.4% |
| snapshots | 1 | Option | lib.rs:143 | 2.7% |
| snapshots | 1 | Map | map.rs:102 | 2.7% |
| snapshots | 2 | MapNode | hamt.rs:159 | 84.1% |
| snapshots | 2 | String | string.rs:13 | 10.4% |
| snapshots | 2 | Option | lib.rs:143 | 2.7% |
| snapshots | 2 | Map | map.rs:102 | 2.7% |
| snapshots | 4 | MapNode | hamt.rs:159 | 84.1% |
| snapshots | 4 | String | string.rs:13 | 10.4% |
| snapshots | 4 | Option | lib.rs:143 | 2.7% |
| snapshots | 4 | Map | map.rs:102 | 2.7% |
| snapshots | 8 | MapNode | hamt.rs:159 | 84.1% |
| snapshots | 8 | String | string.rs:13 | 10.4% |
| snapshots | 8 | Option | lib.rs:143 | 2.7% |
| snapshots | 8 | Map | map.rs:102 | 2.7% |
| words-transient | 1 | Result | abi.rs:14 | 72.5% |
| words-transient | 1 | Buffer | buffer.rs:164 | 27.2% |
| words-transient | 1 | Page | list.rs:326 | 0.1% |
| words-transient | 1 | MapNode | hamt.rs:159 | 0.0% |
| words-transient | 2 | Result | abi.rs:14 | 72.5% |
| words-transient | 2 | Buffer | buffer.rs:164 | 27.2% |
| words-transient | 2 | Page | list.rs:326 | 0.1% |
| words-transient | 2 | MapNode | hamt.rs:159 | 0.0% |
| words-transient | 4 | Result | abi.rs:14 | 72.5% |
| words-transient | 4 | Buffer | buffer.rs:164 | 27.2% |
| words-transient | 4 | Page | list.rs:326 | 0.1% |
| words-transient | 4 | MapNode | hamt.rs:159 | 0.0% |
| words-transient | 8 | Result | abi.rs:14 | 72.5% |
| words-transient | 8 | Buffer | buffer.rs:164 | 27.2% |
| words-transient | 8 | Page | list.rs:326 | 0.1% |
| words-transient | 8 | MapNode | hamt.rs:159 | 0.0% |
