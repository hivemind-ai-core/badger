# Fresh four-language comparison after private-map reuse

Clean production candidate `fc759e3`; raw source/executable/tool hashes and
commands are in [the stamped report](2026-10-18T12-36-54.536840Z-fc759e3f7af6.json).
All 48 cells complete: 240 performance processes, 192 small-case validation
processes, 1,920 full-size iteration results checked (including warmups).
Three warmups/five measured iterations per fresh process; median of five
process warm medians. `*` marks process-median CV >10%. Quiet host/power policy
and JVM steady-state convergence are not claimed.

| Workload | Badger ms | Go ms | Node ms | Java ms |
|---|---:|---:|---:|---:|
| cpu | 50.622 | 49.778 | 50.514 | 47.092 |
| reconcile | 4.561 | 3.056 | 8.548 | 2.678* |
| json | 18.101 | 14.823 | 7.223 | 38.856 |
| state | 4.553 | 2.081 | 3.317* | 2.394 |
| messaging | 0.931 | 0.496* | 11.676 | 1.836 |
| pipeline | 52.530 | 50.673 | 72.864 | 50.780 |

At eight pipeline consumers: Badger 7.903 ms, Go 9.795 ms, Node 10.734 ms,
Java 6.993 ms. Consumer count is not a cap on total runtime/compiler/GC threads;
coordinator/queue topologies differ as described by the unchanged suite contract.

Whole-process RSS includes Badger frontend/JIT and JVM/V8 runtime work. For
reconcile: Badger 49.7 MiB, Go 10.9 MiB, Node 172.1 MiB, Java 100.8 MiB. These
are peak process values, not live retained language heaps.

Java JSON's short warm sequence is visibly transient in every process (e.g.
92.0, 19.1, 31.0, 40.2, 38.9, 40.3, 32.4, 16.2 ms). Across-process median CV
alone does not identify within-process warmup convergence. Do not interpret its
short-cohort 38.856 ms as a steady-state Java ranking. A separate longer-warm
study uses 15 warmups/15 measured samples and three processes per language:

| Language | Longer-warm JSON ms | Process-median CV |
|---|---:|---:|
| badger | 17.516 | 11.8% |
| go | 14.625 | 0.9% |
| node | 6.594 | 0.6% |
| java | 10.460 | 3.9% |

[Long-warm raw report](../I5a-realworld-longwarm/2026-10-18T12-42-15.894170Z-fc759e3f7af6.json).
That probe still does not prove universal convergence. Badger's process CV there
is 11.8%. Keep both cohorts and all observations; never substitute the favorable
warmup window for one language. The JSON port/runtime sources are unchanged.

Reproduction used the immutable candidate and existing pinned Gson jar, with
`GOCACHE=/home/dan/orca/projects/language/target/memory-competitive-go-cache`:

```sh
python3 bench/realworld/run.py --badger /home/dan/orca/projects/language/target/memory-binaries-2026-10-18/private-map-candidate --gson /home/dan/orca/projects/language/bench/realworld/.tools/gson-2.14.0.jar --reports reports/memory-management/I5a-realworld --no-html --order-seed 2026101826
python3 bench/realworld/run.py --badger /home/dan/orca/projects/language/target/memory-binaries-2026-10-18/private-map-candidate --gson /home/dan/orca/projects/language/bench/realworld/.tools/gson-2.14.0.jar --reports reports/memory-management/I5a-realworld-longwarm --no-html --workloads json --workers 1 --processes 3 --warmups 15 --samples 15 --build-samples 1 --order-seed 2026101827
```

This update leaves the shared dashboard and its existing cohorts untouched.
