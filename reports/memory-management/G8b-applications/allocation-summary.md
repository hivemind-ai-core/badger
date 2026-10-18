# G8b application allocation paths

Diagnostic census, three process repetitions per cell, three warmups/five measured inner samples plus startup/output. Eight fixed logical consumers for messaging. These perturbed timings are not an ordinary-build speed comparison.

Allocation paths: median process fractions of managed allocation requests; each reuse counts a new object, not a distinct cell.
Pool bytes below are native chunk reservations. They exclude system cells and other native storage.

| Case | Workers | Reuse objects | Bump objects | System objects | Reuse cell bytes | Bump cell bytes | System cell bytes | Refill chunks | Refill MiB | Trim chunks | Trim MiB | Reserved MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| compare/records | 1 | 54.2% | 45.6% | 0.2% | 46.4% | 39.1% | 14.5% | 18 | 34.25 | 0 | 0.00 | 34.25 |
| compare/records | 8 | 54.2% | 45.6% | 0.2% | 46.4% | 39.1% | 14.5% | 18 | 34.25 | 0 | 0.00 | 34.25 |
| compare/snapshots | 1 | 0.0% | 100.0% | 0.0% | 0.0% | 99.6% | 0.4% | 7 | 12.25 | 0 | 0.00 | 12.25 |
| compare/snapshots | 8 | 0.0% | 100.0% | 0.0% | 0.0% | 99.6% | 0.4% | 7 | 12.25 | 0 | 0.00 | 12.25 |
| realworld/messaging | 1 | 0.0% | 100.0% | 0.0% | 0.0% | 100.0% | 0.0% | 2 | 2.25 | 0 | 0.00 | 2.25 |
| realworld/messaging | 8 | 0.0% | 100.0% | 0.0% | 0.0% | 100.0% | 0.0% | 8 | 2.00 | 0 | 0.00 | 2.00 |
| realworld/reconcile | 1 | 67.9% | 31.6% | 0.5% | 56.8% | 26.3% | 16.9% | 10 | 18.25 | 0 | 0.00 | 18.25 |
| realworld/reconcile | 8 | 67.9% | 31.6% | 0.5% | 56.8% | 26.3% | 16.9% | 10 | 18.25 | 0 | 0.00 | 18.25 |
| realworld/state | 1 | 0.0% | 63.2% | 36.8% | 0.0% | 23.8% | 76.2% | 3 | 4.25 | 0 | 0.00 | 4.25 |
| realworld/state | 8 | 0.0% | 63.2% | 36.8% | 0.0% | 23.8% | 76.2% | 3 | 4.25 | 0 | 0.00 | 4.25 |

| Case | Workers | Allocated objects | Managed cell MiB | System cell byte share | Top runtime source by requested bytes | Source share |
|---|---:|---:|---:|---:|---|---:|
| compare/records | 1 | 1,603,420 | 85.70 | 14.5% | Record abi.rs:14 | 83.0% |
| compare/records | 8 | 1,603,420 | 85.70 | 14.5% | Record abi.rs:14 | 83.0% |
| compare/snapshots | 1 | 57,628 | 12.14 | 0.4% | MapNode hamt.rs:252 | 94.4% |
| compare/snapshots | 8 | 57,628 | 12.14 | 0.4% | MapNode hamt.rs:252 | 94.4% |
| realworld/messaging | 1 | 8,376 | 0.51 | 0.0% | Buffer buffer.rs:212 | 98.4% |
| realworld/messaging | 8 | 8,376 | 0.51 | 0.0% | Buffer buffer.rs:212 | 98.4% |
| realworld/reconcile | 1 | 902,239 | 67.32 | 16.9% | Order abi.rs:14 | 78.7% |
| realworld/reconcile | 8 | 902,239 | 67.32 | 16.9% | Order abi.rs:14 | 78.7% |
| realworld/state | 1 | 47,455 | 14.77 | 76.2% | MapNode hamt.rs:252 | 98.3% |
| realworld/state | 8 | 47,455 | 14.77 | 76.2% | MapNode hamt.rs:252 | 98.3% |
