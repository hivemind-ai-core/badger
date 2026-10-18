# Longer-warm four-language check after snapshot reuse

Production `49f281c`. [Raw report](2026-10-18T17-29-08.687248Z-49f281c5ef83.json); SHA-256 `fb8b3f0fc75eeae86c371a901a898e88749d8dbd78e3374a30ac290f4a781e89`.

Twelve cells complete, zero failures: five fresh performance processes per cell, 15 warmups and 15 measured kernels each. All checksums pass independent oracles; 48 small-case validation processes also pass. No shared HTML dashboard was changed.

| Workload | Badger ms | Go ms | Node ms | Java ms |
|---|---:|---:|---:|---:|
| state | 1.113 | 2.217 | 2.316 | 1.168 |
| reconcile | 5.406 | 3.120 | 4.609 | 1.462 |
| json | 18.386 | 14.608 | 6.927 | 11.132 |

State is competitive with Java and faster than Go/Node in this cohort. Badger state process-median CV is 11.6%, so the small Java difference is not a reliable rank. Reconciliation and JSON still have substantial gaps; there is no general competitiveness claim. Badger/Go/Node reconcile CVs are 11.1/12.6/10.9%. Raw warmup traces and within-process tails remain.

The earlier G7d cohort uses the same warmup/sample counts but different process times/order/host state. Do not use cross-cohort timing differences as an optimization delta. The fresh adjacent binary pairs in I5b-state-longwarm establish the extended-warm state delta; the main application matrix separately retains the shorter three-warmup/five-sample protocol. Warmup affects Badger too; this does not attribute that effect to its JIT, allocator, or collector individually.

Kernel timing excludes frontend/JIT and fixture/I/O work. RSS includes full language processes and differs in runtime/compiler/GC threads. No equal total thread budget, steady-state JVM/V8 convergence, request p99, cold-start win or universal memory advantage is established.

This official runner lacks per-child absolute UTC intervals; its 60 performance processes are separate from the GC-harness interval audit. It ran sequentially after I5b-app-control and before I5b-control-followup, with no own builds/tests overlapping its timing.
