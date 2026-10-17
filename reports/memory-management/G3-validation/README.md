# G3 validation boundaries

`candidate-*` logs cover the rejected ordinary cached-helper candidate at
`564b492`; `final-*` logs cover the restoration of scoped ordinary helpers and
the diagnostic-only prototype. Historical prototype/control builds are separate.

| Check | Candidate | Final scoped-default tree |
|---|---|---|
| Ordinary runtime tests | 8 passed | 8 passed |
| Diagnostic runtime tests | 10 passed | 10 passed |
| CLI programs / GC retention / transient maps | 68 passed | 68 passed |
| Benchmark harness unit tests | 11 passed | 11 passed |
| Runtime Clippy, all targets, warnings denied | diagnostic feature passed | default and diagnostic feature passed |
| Workspace formatting | passed | passed |
| Ordinary binary/profiling check | 0 versus 3 cached helpers; no diagnostic files | 0 versus 0 helpers; no diagnostic files |

`normal-policy-restoration.json` verifies that the ordinary scoped sweep body
matches main `36091e7` verbatim. The final prototype module is compiled only for
`gc-profile` or unit tests. Its shutdown hook is absent from ordinary builds.
`final-scoped-check.json` checks actual behavior, not just source constants.

The isolated helper regression uses sparse worker indices and no task workers.
It verifies old-prefix preservation, sticky survivor marks, clearing full-sweep
marks, live byte totals, exact-once ordered finalizers on the collector thread,
thread reuse, joined/idempotent shutdown and non-reopening scoped fallback.
Allocator reuse/zeroing/quarantine/retirement tests remain intact.

Final diagnostic checks separately run both scoped/reuse policies at 2/8 workers
on balanced/fanout profiles. Eight measured smokes plus warmups pass their oracles
and sweep-count conservation. Actual native creation is checked against dispatched
jobs: cached processes create only 1 or 7 helpers, scoped processes create one
per job. These are validation smokes, not additional performance claims.

The final diagnostic prototype also checks every retained string in the parallel
snapshot workload at eight workers, in both generational and full-collection modes.
Records and GC work counters are in `final-retained-strings.json.gz`. Heap
verification/stress fixture checks in the CLI suites remain separate: verification
intentionally uses the serial sweep path, so it alone does not exercise helpers.

Commands (target paths and log redirection omitted):

```
cargo test --offline -p badger_runtime --lib
cargo test --offline -p badger_runtime --lib --features gc-profile
cargo clippy --offline -p badger_runtime --all-targets -- -D warnings
cargo clippy --offline -p badger_runtime --all-targets --features gc-profile -- -D warnings
cargo test --offline --release -p badger_cli --test programs --test gc_retention --test transient_maps
python3 -m unittest discover -s bench/gc -p 'test_*.py'
cargo fmt --all -- --check
python3 bench/gc/check_sweep_reuse.py --baseline BASE --candidate FINAL --candidate-policy scoped --out CHECK.json
```

Normal and diagnostic compilers are copied to immutable paths before switching
features/worktrees. No performance comparison overlaps our builds or correctness
suites. Executable/build-source hashes are retained in `../G3-binaries.json`.
