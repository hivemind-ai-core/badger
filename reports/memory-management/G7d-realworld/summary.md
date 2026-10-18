# Longer-warm allocation-heavy four-language check

Production binary `38855c7`, integrated in main `d392ed0`. [Raw report](2026-10-18T16-39-52.040460Z-d392ed09ef86.json); SHA-256 `4c30d7f1457efb20500e0394d68a99545d91851884dc98043d6dbbda92f80b7d`.

Twelve cells complete with zero failures. Five fresh performance processes per cell, 15 warmups and 15 measured kernels each; all iteration checksums are independently validated. Medians below are medians of process warm medians. Source/tool/binary identities and raw iterations are in the report.

| Workload | Badger ms | Go ms | Node ms | Java ms |
|---|---:|---:|---:|---:|
| state | 4.264 | 2.085 | 2.170 | 1.126 |
| reconcile | 4.964 | 2.960 | 4.906 | 1.425 |
| json | 17.661 | 14.095 | 6.417 | 10.573 |

The application gap remains substantial. Longer warming makes Java state/reconcile materially faster than the earlier three-warmup cohort, and Node reconciliation also changes. Do not infer a prefetch regression or a new optimization delta from those different protocols. G7d deltas come from fixed-protocol binary pairs in the G7d report.

This does not establish universal JVM/V8 convergence. Process-median CV exceeds 10% for Node reconcile (11.0%), Java reconcile (12.3%) and Badger JSON (11.6%); raw within-process tails and warmup traces remain. No quiet-host, equal total thread budget or live-heap RSS claim is made.

State still allocates 131,080 map wrappers and 285,200 trie nodes across eight inner iterations in the separately instrumented fixed-protocol census. Reusing nodes between observable snapshots is the next allocation-reduction candidate; collector-only speedups have not closed the gap.

Reproduce without modifying the shared HTML dashboard:

```sh
GOCACHE=/home/dan/orca/projects/language/target/memory-competitive-go-cache python3 bench/realworld/run.py --badger /home/dan/orca/projects/language/target/memory-binaries-2026-10-18/prefetch32-candidate --gson /home/dan/orca/projects/language/bench/realworld/.tools/gson-2.14.0.jar --reports reports/memory-management/G7d-realworld --no-html --workloads state reconcile json --workers 1 --processes 5 --warmups 15 --samples 15 --build-samples 1 --order-seed 2026101828
```

This official runner lacks per-child absolute UTC intervals, so its 60 performance processes are kept separately from the 1,382-process GC-harness interval audit. It ran after that matrix and before trace-counter builds/tests; no own build/test overlapped its timing.
