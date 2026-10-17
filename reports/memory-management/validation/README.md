# I1 validation

Production candidate: `19d35fc3a7593fa1464bd23a2b447dcd52182ec3`.
No production changes followed that commit during these checks or comparisons.

- `cargo fmt --all -- --check`: passed.
- `python3 -m unittest discover -s bench/gc -p 'test_*.py'`: seven passed.
- Strict default workspace/all-target Clippy: passed (`clippy-default.txt`).
- Strict runtime/all-target Clippy with `gc-profile`: passed
  (`clippy-diagnostics.txt`). Both use `-- -D warnings`.
- Debug runtime unit/integration tests: passed (`runtime-debug.txt`), including
  collision-heavy transient/persistent models and zero managed allocations during
  replacement-only updates.
- Dedicated debug/optimized builder tests: passed (`builder-debug-stress.txt`):
  generational collection on/off, workers 1/4/8, stress, verification, chaos,
  snapshot preservation, pointer fields, task returns and saved MIR.
- Both fixed ordinary executables passed all strengthened benchmark smoke cases,
  workers 1/4. Smoke timing is not used as performance evidence.

Full release workspace coverage was completed in parts. The original command
was `cargo test --release --offline --workspace --target-dir
/tmp/language-gc-profile-target -- --test-threads=1`.
`workspace-release-prefix.txt` records successful base, adversarial, docs, fuzz,
GC-retention and LSP tests, followed by an MCP failure caused by the sandbox
rejecting a local-port reservation. This environment failure is retained rather
than hidden. The remaining CLI targets were run with local-network access:

```
cargo test --release --offline -p lang_cli --target-dir /tmp/language-gc-profile-target --test mcp --test option_niche --test packed_arrays --test postgres --test programs --test scalar_enums --test sequence_fusion --test service --test stress --test transient_maps --test valkey -- --test-threads=1
cargo test --release --offline --workspace --exclude lang_cli --target-dir /tmp/language-gc-profile-target -- --test-threads=1
```

Both commands passed (`workspace-release-remaining.txt` and
`workspace-release-libraries.txt`). Combined with the successful prefix, all
workspace targets are covered: 146 distinct passing tests, one intentionally
ignored native-library doctest. There were 147 passing test executions because
the base-library test ran again in the library command.

`workspace-debug-incomplete.txt` is explicitly a partial run. Its adversarial,
docs and fuzz tests passed; the first unoptimized GC soak was still busy after
five minutes, so the run was stopped and broad coverage moved to release builds.
It is not a complete debug-workspace pass. The dedicated debug tests above passed.

Timings exclude active validation builds/tests: the ordinary performance matrices
started only after the final test commands completed. They remain shared-host
measurements; ranges and control workloads limit interpretation of small changes.
