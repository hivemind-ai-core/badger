# Diagnostic same-binary distance comparison

Five repetitions per case/worker/policy. Four policies are grouped and shuffled; all use the same executable. Values below are workload-only collection phases, filtered by before/after collection IDs. Instrumented work is not ordinary throughput.

| Case | Workers | Lookahead | Sweep ms | Sweep change vs 0 | Work change vs 0 | Paired sweep median |
|---|---:|---:|---:|---:|---:|---:|
| buffers-pointers | 1 | 0 | 1.436 | +0.0% | +0.0% | +0.0% |
| buffers-pointers | 1 | 16 | 1.479 | +3.0% | +1.0% | +2.6% |
| buffers-pointers | 1 | 32 | 1.469 | +2.2% | +0.3% | +2.2% |
| buffers-pointers | 1 | 64 | 1.623 | +13.0% | +1.6% | +9.4% |
| buffers-pointers | 8 | 0 | 1.725 | +0.0% | +0.0% | +0.0% |
| buffers-pointers | 8 | 16 | 1.499 | -13.1% | +2.9% | -8.8% |
| buffers-pointers | 8 | 32 | 1.484 | -14.0% | +1.6% | -15.3% |
| buffers-pointers | 8 | 64 | 1.472 | -14.7% | +2.2% | -14.7% |
| parallel-snapshots | 1 | 0 | 75.547 | +0.0% | +0.0% | +0.0% |
| parallel-snapshots | 1 | 16 | 43.286 | -42.7% | -10.0% | -42.6% |
| parallel-snapshots | 1 | 32 | 39.737 | -47.4% | -11.4% | -47.4% |
| parallel-snapshots | 1 | 64 | 40.855 | -45.9% | -12.2% | -47.5% |
| parallel-snapshots | 8 | 0 | 23.222 | +0.0% | +0.0% | +0.0% |
| parallel-snapshots | 8 | 16 | 18.942 | -18.4% | -3.0% | -22.9% |
| parallel-snapshots | 8 | 32 | 17.815 | -23.3% | -4.0% | -24.5% |
| parallel-snapshots | 8 | 64 | 17.134 | -26.2% | -0.7% | -26.6% |
| records-window | 1 | 0 | 21.633 | +0.0% | +0.0% | +0.0% |
| records-window | 1 | 16 | 21.722 | +0.4% | +2.3% | -0.8% |
| records-window | 1 | 32 | 20.550 | -5.0% | +0.3% | -4.5% |
| records-window | 1 | 64 | 20.051 | -7.3% | -2.1% | -7.2% |
| records-window | 8 | 0 | 22.403 | +0.0% | +0.0% | +0.0% |
| records-window | 8 | 16 | 21.851 | -2.5% | -3.3% | -3.5% |
| records-window | 8 | 32 | 21.058 | -6.0% | -0.9% | -4.4% |
| records-window | 8 | 64 | 20.318 | -9.3% | -1.1% | -9.2% |
| scalar-snapshots | 1 | 0 | 3.430 | +0.0% | +0.0% | +0.0% |
| scalar-snapshots | 1 | 16 | 1.548 | -54.9% | -17.8% | -60.8% |
| scalar-snapshots | 1 | 32 | 1.233 | -64.1% | -21.2% | -62.7% |
| scalar-snapshots | 1 | 64 | 1.544 | -55.0% | -17.4% | -59.9% |
| scalar-snapshots | 8 | 0 | 3.201 | +0.0% | +0.0% | +0.0% |
| scalar-snapshots | 8 | 16 | 1.356 | -57.6% | -9.3% | -57.6% |
| scalar-snapshots | 8 | 32 | 1.059 | -66.9% | -19.4% | -68.1% |
| scalar-snapshots | 8 | 64 | 1.432 | -55.2% | -6.7% | -59.9% |
| snapshots-major | 1 | 0 | 25.157 | +0.0% | +0.0% | +0.0% |
| snapshots-major | 1 | 16 | 13.739 | -45.4% | -11.1% | -46.8% |
| snapshots-major | 1 | 32 | 11.983 | -52.4% | -14.2% | -52.0% |
| snapshots-major | 1 | 64 | 12.474 | -50.4% | -13.1% | -50.9% |
| snapshots-major | 8 | 0 | 26.587 | +0.0% | +0.0% | +0.0% |
| snapshots-major | 8 | 16 | 13.605 | -48.8% | -12.6% | -48.8% |
| snapshots-major | 8 | 32 | 11.914 | -55.2% | -16.1% | -54.7% |
| snapshots-major | 8 | 64 | 13.517 | -49.2% | -11.1% | -48.9% |
