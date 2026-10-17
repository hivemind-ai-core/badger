Process-wide managed allocations, including startup and output encoding. Three repetitions unless noted in the raw report.
Requested bytes include headers. Cell bytes include pool rounding but exclude native buffers, allocator metadata and unused pool capacity.
Early reclamation and first survival are observed fractions of all allocations; unswept tail objects censor lifetime measurements.
Survival does not establish escape or unique ownership. Diagnostic timings/RSS include profiler overhead.

| Case | Workers | Objects | Requested MiB | Cell MiB | Early reclaimed bytes | First-survived bytes | Remaining bytes | Side-vector capacity MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| messages-int-fanout-sustained | 1 | 262,857 | 6.04 | 8.05 | 97.5% | 0.2% | 2.5% | 1.00 |
| messages-int-fanout-sustained | 2 | 262,857 | 6.04 | 8.05 | 97.7% | 0.2% | 2.3% | 1.50 |
| messages-int-fanout-sustained | 4 | 262,857 | 6.04 | 8.05 | 98.2% | 0.1% | 1.8% | 1.00 |
| messages-int-fanout-sustained | 8 | 262,857 | 6.04 | 8.05 | 98.9% | 0.1% | 1.1% | 1.50 |
| messages-int-retained-sustained | 1 | 263,496 | 6.08 | 8.09 | 96.9% | 0.9% | 3.1% | 1.00 |
| messages-int-retained-sustained | 2 | 263,496 | 6.08 | 8.09 | 96.9% | 0.8% | 3.1% | 2.00 |
| messages-int-retained-sustained | 4 | 263,496 | 6.08 | 8.09 | 97.0% | 0.8% | 3.0% | 2.50 |
| messages-int-retained-sustained | 8 | 263,496 | 6.08 | 8.09 | 97.1% | 0.9% | 2.9% | 3.56 |
| messages-int-slow-sustained | 1 | 262,221 | 6.00 | 8.01 | 97.8% | 0.0% | 2.2% | 1.00 |
| messages-int-slow-sustained | 2 | 262,221 | 6.00 | 8.01 | 97.8% | 0.0% | 2.2% | 1.50 |
| messages-int-slow-sustained | 4 | 262,221 | 6.00 | 8.01 | 98.3% | 0.0% | 1.7% | 1.00 |
| messages-int-slow-sustained | 8 | 262,221 | 6.00 | 8.01 | 98.6% | 0.0% | 1.4% | 1.12 |
| messages-int-sustained | 1 | 262,536 | 6.02 | 8.03 | 97.6% | 0.1% | 2.4% | 1.00 |
| messages-int-sustained | 2 | 262,536 | 6.02 | 8.03 | 97.6% | 0.1% | 2.4% | 1.00 |
| messages-int-sustained | 4 | 262,536 | 6.02 | 8.03 | 97.6% | 0.1% | 2.4% | 1.00 |
| messages-int-sustained | 8 | 262,536 | 6.02 | 8.03 | 97.6% | 0.1% | 2.4% | 1.00 |

Calibration: workload time medians; positive change means slower. Shared-host ranges are in summary.md.

| Case | Workers | Phase-off vs ordinary | Phase-only vs phase-off | Census vs phase-only |
|---|---:|---:|---:|---:|
| messages-int-fanout-sustained | 1 | +1.5% | -0.1% | +2.2% |
| messages-int-fanout-sustained | 2 | +0.2% | -0.5% | +0.2% |
| messages-int-fanout-sustained | 4 | +0.4% | +0.4% | +4.8% |
| messages-int-fanout-sustained | 8 | -3.7% | +2.2% | +4.3% |
| messages-int-retained-sustained | 1 | -0.3% | +1.5% | +5.2% |
| messages-int-retained-sustained | 2 | +1.0% | -0.0% | +2.8% |
| messages-int-retained-sustained | 4 | +2.2% | -6.4% | +11.3% |
| messages-int-retained-sustained | 8 | +1.7% | +1.8% | +4.3% |
| messages-int-slow-sustained | 1 | -0.6% | +0.3% | +4.1% |
| messages-int-slow-sustained | 2 | -5.0% | +5.8% | +2.7% |
| messages-int-slow-sustained | 4 | -3.0% | +3.8% | +2.2% |
| messages-int-slow-sustained | 8 | -1.5% | +0.6% | +0.2% |
| messages-int-sustained | 1 | +1.0% | +0.1% | +1.7% |
| messages-int-sustained | 2 | +2.1% | +0.1% | +6.2% |
| messages-int-sustained | 4 | +3.0% | -2.7% | +0.5% |
| messages-int-sustained | 8 | +14.4% | -11.6% | +5.2% |

Largest runtime allocation sites (byte shares pooled over repetitions):

| Case | Workers | Type | Runtime source | Share of requested bytes |
|---|---:|---|---|---:|
| messages-int-fanout-sustained | 1 | Buffer | buffer.rs:164 | 99.3% |
| messages-int-fanout-sustained | 1 | Page | list.rs:326 | 0.3% |
| messages-int-fanout-sustained | 1 | PageDirectory | list.rs:349 | 0.1% |
| messages-int-fanout-sustained | 1 | List | list.rs:201 | 0.1% |
| messages-int-fanout-sustained | 2 | Buffer | buffer.rs:164 | 99.3% |
| messages-int-fanout-sustained | 2 | Page | list.rs:326 | 0.3% |
| messages-int-fanout-sustained | 2 | PageDirectory | list.rs:349 | 0.1% |
| messages-int-fanout-sustained | 2 | List | list.rs:201 | 0.1% |
| messages-int-fanout-sustained | 4 | Buffer | buffer.rs:164 | 99.3% |
| messages-int-fanout-sustained | 4 | Page | list.rs:326 | 0.3% |
| messages-int-fanout-sustained | 4 | PageDirectory | list.rs:349 | 0.1% |
| messages-int-fanout-sustained | 4 | List | list.rs:201 | 0.1% |
| messages-int-fanout-sustained | 8 | Buffer | buffer.rs:164 | 99.3% |
| messages-int-fanout-sustained | 8 | Page | list.rs:326 | 0.3% |
| messages-int-fanout-sustained | 8 | PageDirectory | list.rs:349 | 0.1% |
| messages-int-fanout-sustained | 8 | List | list.rs:201 | 0.1% |
| messages-int-retained-sustained | 1 | Buffer | buffer.rs:164 | 98.7% |
| messages-int-retained-sustained | 1 | Page | list.rs:326 | 0.6% |
| messages-int-retained-sustained | 1 | PageDirectory | list.rs:349 | 0.3% |
| messages-int-retained-sustained | 1 | List | list.rs:201 | 0.2% |
| messages-int-retained-sustained | 2 | Buffer | buffer.rs:164 | 98.7% |
| messages-int-retained-sustained | 2 | Page | list.rs:326 | 0.6% |
| messages-int-retained-sustained | 2 | PageDirectory | list.rs:349 | 0.3% |
| messages-int-retained-sustained | 2 | List | list.rs:201 | 0.2% |
| messages-int-retained-sustained | 4 | Buffer | buffer.rs:164 | 98.7% |
| messages-int-retained-sustained | 4 | Page | list.rs:326 | 0.6% |
| messages-int-retained-sustained | 4 | PageDirectory | list.rs:349 | 0.3% |
| messages-int-retained-sustained | 4 | List | list.rs:201 | 0.2% |
| messages-int-retained-sustained | 8 | Buffer | buffer.rs:164 | 98.7% |
| messages-int-retained-sustained | 8 | Page | list.rs:326 | 0.6% |
| messages-int-retained-sustained | 8 | PageDirectory | list.rs:349 | 0.3% |
| messages-int-retained-sustained | 8 | List | list.rs:201 | 0.2% |
| messages-int-slow-sustained | 1 | Buffer | buffer.rs:164 | 99.9% |
| messages-int-slow-sustained | 1 | MapNode | hamt.rs:159 | 0.0% |
| messages-int-slow-sustained | 1 | String | string.rs:13 | 0.0% |
| messages-int-slow-sustained | 1 | Data | lib.rs:143 | 0.0% |
| messages-int-slow-sustained | 2 | Buffer | buffer.rs:164 | 99.9% |
| messages-int-slow-sustained | 2 | MapNode | hamt.rs:159 | 0.0% |
| messages-int-slow-sustained | 2 | String | string.rs:13 | 0.0% |
| messages-int-slow-sustained | 2 | Data | lib.rs:143 | 0.0% |
| messages-int-slow-sustained | 4 | Buffer | buffer.rs:164 | 99.9% |
| messages-int-slow-sustained | 4 | MapNode | hamt.rs:159 | 0.0% |
| messages-int-slow-sustained | 4 | String | string.rs:13 | 0.0% |
| messages-int-slow-sustained | 4 | Data | lib.rs:143 | 0.0% |
| messages-int-slow-sustained | 8 | Buffer | buffer.rs:164 | 99.9% |
| messages-int-slow-sustained | 8 | MapNode | hamt.rs:159 | 0.0% |
| messages-int-slow-sustained | 8 | String | string.rs:13 | 0.0% |
| messages-int-slow-sustained | 8 | Data | lib.rs:143 | 0.0% |
| messages-int-sustained | 1 | Buffer | buffer.rs:164 | 99.6% |
| messages-int-sustained | 1 | Page | list.rs:326 | 0.2% |
| messages-int-sustained | 1 | PageDirectory | list.rs:349 | 0.1% |
| messages-int-sustained | 1 | List | list.rs:201 | 0.0% |
| messages-int-sustained | 2 | Buffer | buffer.rs:164 | 99.6% |
| messages-int-sustained | 2 | Page | list.rs:326 | 0.2% |
| messages-int-sustained | 2 | PageDirectory | list.rs:349 | 0.1% |
| messages-int-sustained | 2 | List | list.rs:201 | 0.0% |
| messages-int-sustained | 4 | Buffer | buffer.rs:164 | 99.6% |
| messages-int-sustained | 4 | Page | list.rs:326 | 0.2% |
| messages-int-sustained | 4 | PageDirectory | list.rs:349 | 0.1% |
| messages-int-sustained | 4 | List | list.rs:201 | 0.0% |
| messages-int-sustained | 8 | Buffer | buffer.rs:164 | 99.6% |
| messages-int-sustained | 8 | Page | list.rs:326 | 0.2% |
| messages-int-sustained | 8 | PageDirectory | list.rs:349 | 0.1% |
| messages-int-sustained | 8 | List | list.rs:201 | 0.0% |
