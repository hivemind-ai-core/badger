# Merged-main G2 validation

Ordinary candidate source: merge `8596e6d` of main `0958059`, 512-byte pools.
The subsequent source edits add a rationale comment and the boundary stress
regression; they do not change production behavior. Baseline/candidate hashes
used in every timing matrix are in `../G2-main-binaries.json`.

| Check | Result | Log |
|---|---|---|
| Ordinary runtime unit tests | 7 passed | runtime-default.log |
| Diagnostic runtime unit tests | 9 passed | runtime-diagnostic.log |
| CLI programs, GC retention, transient maps | 67 passed | cli-validation.log |
| New boundary handoff regression | 1 passed, 16 stress combinations | boundary-stress.log |
| Benchmark harness unit tests | 9 passed | runner-tests.log |
| Runtime Clippy, all targets, diagnostic feature, warnings denied | passed | clippy.log |
| Workspace formatting check | passed | fmt.log |
| Binary pool behavior probe | passed | pool-limit-check.json |

Runtime pool unit tests run in isolated children with verification on/off.
The diagnostic test exercises all 256/512/1024-byte limits. It covers every
payload size through 1,040 bytes, zeroed reuse, pool/native boundaries,
crossing chunks, retaining quarantined cells, and purging links before trim.
No timed runs overlap our compiler builds or correctness suites.

Commands (target directories omitted):

```
cargo test --offline -p badger_runtime --lib
cargo test --offline -p badger_runtime --lib --features gc-profile
cargo clippy --offline -p badger_runtime --all-targets --features gc-profile -- -D warnings
cargo test --offline --release -p badger_cli --test programs --test gc_retention --test transient_maps
cargo test --offline --release -p badger_cli --test transient_maps pool_boundary_strings_survive_task_handoff_and_later_collections
python3 -m unittest discover -s bench/gc -p 'test_*.py'
cargo fmt --all -- --check
```

The first CLI invocation preceded the added boundary stress test; the second
runs that test separately. The existing program corpus already checks its
fixture in ordinary/source/release/saved-MIR/multicore execution. The new test
explicitly adds stress/verification/chaos in both collector modes at all worker
counts and both language optimization modes. Complete string contents retained
from other tasks are checked after further allocations, rather than lengths only.

Build logs pin both ordinary compiler source paths. A reused Cargo artifact was
detected and rejected before timing; the candidate runtime artifacts were
cleaned and rebuilt. The behavior probe and distinct hashes verify the actual
immutable comparison pair. Diagnostic timings/censuses in `../G2-pre-main/`
are historical; no pre-main binary is used by the fresh ordinary matrices.
