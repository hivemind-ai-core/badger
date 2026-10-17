Process-wide managed allocations, including startup and output encoding. Three repetitions unless noted in the raw report.
Requested bytes include headers. Cell bytes include pool rounding but exclude native buffers, allocator metadata and unused pool capacity.
Early reclamation and first survival are observed fractions of all allocations; unswept tail objects censor lifetime measurements.
Survival does not establish escape or unique ownership. Diagnostic timings/RSS include profiler overhead.

| Case | Workers | Objects | Requested MiB | Cell MiB | Early reclaimed bytes | First-survived bytes | Remaining bytes | Side-vector capacity MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| backpressure | 1 | 1,040,849 | 78.32 | 80.05 | 95.6% | 2.3% | 4.4% | 4.00 |
| backpressure | 2 | 1,040,726 | 78.34 | 80.08 | 95.8% | 2.3% | 4.2% | 4.00 |
| backpressure | 4 | 1,040,793 | 78.33 | 80.06 | 96.3% | 2.3% | 3.7% | 4.00 |
| backpressure | 8 | 1,040,754 | 78.32 | 80.05 | 97.5% | 2.1% | 2.5% | 4.00 |
| balanced | 1 | 1,041,016 | 78.39 | 80.12 | 92.2% | 5.5% | 7.8% | 4.00 |
| balanced | 2 | 1,040,969 | 78.40 | 80.14 | 92.4% | 5.5% | 7.6% | 6.00 |
| balanced | 4 | 1,040,929 | 78.32 | 80.05 | 92.9% | 5.5% | 7.1% | 6.00 |
| balanced | 8 | 1,041,034 | 78.36 | 80.09 | 94.2% | 5.2% | 5.8% | 6.50 |
| cpu | 1 | 1,092,090 | 81.04 | 82.79 | 89.7% | 4.9% | 10.3% | 8.00 |
| cpu | 2 | 1,091,771 | 81.13 | 82.87 | 89.8% | 4.9% | 10.2% | 8.00 |
| cpu | 4 | 1,092,399 | 80.98 | 82.73 | 90.3% | 4.8% | 9.7% | 8.00 |
| cpu | 8 | 1,091,569 | 81.01 | 82.74 | 91.4% | 4.8% | 8.6% | 7.00 |
| fanout | 1 | 2,061,837 | 161.93 | 164.49 | 91.9% | 4.9% | 4.8% | 8.00 |
| fanout | 2 | 2,061,865 | 161.95 | 164.50 | 92.1% | 4.9% | 4.6% | 8.00 |
| fanout | 4 | 2,061,412 | 161.60 | 164.16 | 92.7% | 4.9% | 3.9% | 10.00 |
| fanout | 8 | 2,061,649 | 161.69 | 164.24 | 93.5% | 4.9% | 3.2% | 8.00 |

Calibration: workload time medians; positive change means slower. Shared-host ranges are in summary.md.

| Case | Workers | Phase-off vs ordinary | Phase-only vs phase-off | Census vs phase-only |
|---|---:|---:|---:|---:|
| backpressure | 1 | +3.9% | -5.7% | +29.2% |
| backpressure | 2 | +5.0% | -1.5% | +19.6% |
| backpressure | 4 | -1.1% | -2.2% | +45.6% |
| backpressure | 8 | -4.3% | +8.2% | +48.8% |
| balanced | 1 | -3.8% | -3.7% | +33.9% |
| balanced | 2 | +0.1% | +4.0% | +24.8% |
| balanced | 4 | -15.7% | +6.1% | +31.2% |
| balanced | 8 | +5.7% | -11.2% | +40.1% |
| cpu | 1 | +0.1% | +6.6% | +13.7% |
| cpu | 2 | +2.9% | -3.2% | +23.9% |
| cpu | 4 | -5.0% | +3.5% | +24.4% |
| cpu | 8 | +3.2% | -11.3% | +24.2% |
| fanout | 1 | +4.9% | +6.6% | +13.2% |
| fanout | 2 | -11.0% | -7.9% | +33.0% |
| fanout | 4 | +1.8% | -4.1% | +20.4% |
| fanout | 8 | -4.5% | +2.4% | +12.6% |

Largest runtime allocation sites (byte shares pooled over repetitions):

| Case | Workers | Type | Runtime source | Share of requested bytes |
|---|---:|---|---|---:|
| backpressure | 1 | MapNode | hamt.rs:159 | 62.0% |
| backpressure | 1 | Result | abi.rs:14 | 13.1% |
| backpressure | 1 | Record | abi.rs:14 | 11.3% |
| backpressure | 1 | Map | map.rs:102 | 3.2% |
| backpressure | 2 | MapNode | hamt.rs:159 | 62.0% |
| backpressure | 2 | Result | abi.rs:14 | 13.1% |
| backpressure | 2 | Record | abi.rs:14 | 11.3% |
| backpressure | 2 | Map | map.rs:102 | 3.2% |
| backpressure | 4 | MapNode | hamt.rs:159 | 62.0% |
| backpressure | 4 | Result | abi.rs:14 | 13.1% |
| backpressure | 4 | Record | abi.rs:14 | 11.3% |
| backpressure | 4 | Map | map.rs:102 | 3.2% |
| backpressure | 8 | MapNode | hamt.rs:159 | 62.0% |
| backpressure | 8 | Result | abi.rs:14 | 13.1% |
| backpressure | 8 | Record | abi.rs:14 | 11.3% |
| backpressure | 8 | Map | map.rs:102 | 3.2% |
| balanced | 1 | MapNode | hamt.rs:159 | 62.0% |
| balanced | 1 | Result | abi.rs:14 | 13.1% |
| balanced | 1 | Record | abi.rs:14 | 11.3% |
| balanced | 1 | Map | map.rs:102 | 3.2% |
| balanced | 2 | MapNode | hamt.rs:159 | 62.0% |
| balanced | 2 | Result | abi.rs:14 | 13.1% |
| balanced | 2 | Record | abi.rs:14 | 11.3% |
| balanced | 2 | Map | map.rs:102 | 3.2% |
| balanced | 4 | MapNode | hamt.rs:159 | 62.0% |
| balanced | 4 | Result | abi.rs:14 | 13.1% |
| balanced | 4 | Record | abi.rs:14 | 11.3% |
| balanced | 4 | Map | map.rs:102 | 3.2% |
| balanced | 8 | MapNode | hamt.rs:159 | 62.0% |
| balanced | 8 | Result | abi.rs:14 | 13.1% |
| balanced | 8 | Record | abi.rs:14 | 11.3% |
| balanced | 8 | Map | map.rs:102 | 3.2% |
| cpu | 1 | MapNode | hamt.rs:159 | 61.4% |
| cpu | 1 | Result | abi.rs:14 | 12.7% |
| cpu | 1 | Record | abi.rs:14 | 10.9% |
| cpu | 1 | Option | lib.rs:143 | 4.5% |
| cpu | 2 | MapNode | hamt.rs:159 | 61.5% |
| cpu | 2 | Result | abi.rs:14 | 12.7% |
| cpu | 2 | Record | abi.rs:14 | 10.9% |
| cpu | 2 | Option | lib.rs:143 | 4.4% |
| cpu | 4 | MapNode | hamt.rs:159 | 61.4% |
| cpu | 4 | Result | abi.rs:14 | 12.7% |
| cpu | 4 | Record | abi.rs:14 | 10.9% |
| cpu | 4 | Option | lib.rs:143 | 4.5% |
| cpu | 8 | MapNode | hamt.rs:159 | 61.5% |
| cpu | 8 | Result | abi.rs:14 | 12.7% |
| cpu | 8 | Record | abi.rs:14 | 10.9% |
| cpu | 8 | Option | lib.rs:143 | 4.5% |
| fanout | 1 | MapNode | hamt.rs:159 | 65.4% |
| fanout | 1 | Record | abi.rs:14 | 8.7% |
| fanout | 1 | Result | abi.rs:14 | 7.5% |
| fanout | 1 | Map | map.rs:102 | 3.7% |
| fanout | 2 | MapNode | hamt.rs:159 | 65.4% |
| fanout | 2 | Record | abi.rs:14 | 8.7% |
| fanout | 2 | Result | abi.rs:14 | 7.5% |
| fanout | 2 | Map | map.rs:102 | 3.7% |
| fanout | 4 | MapNode | hamt.rs:159 | 65.3% |
| fanout | 4 | Record | abi.rs:14 | 8.8% |
| fanout | 4 | Result | abi.rs:14 | 7.5% |
| fanout | 4 | Map | map.rs:102 | 3.7% |
| fanout | 8 | MapNode | hamt.rs:159 | 65.3% |
| fanout | 8 | Record | abi.rs:14 | 8.8% |
| fanout | 8 | Result | abi.rs:14 | 7.5% |
| fanout | 8 | Map | map.rs:102 | 3.7% |
