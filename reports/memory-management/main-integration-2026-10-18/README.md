# Lookup optimization integration with main, 2026-10-18

Combined tree `1c4963c` merges the measured lookup changes/evidence with main
`1c8d910` without conflicts. Historical I2a timings remain pinned to their
original production revisions; this is integration correctness evidence.

- All 76 CLI/program tests pass with local socket, PTY and debugger access.
- Restricted execution first passed 66 and blocked 10 environment-dependent
  cases; both full outputs are retained, with no failed evidence removed.
- Library tests pass: one codegen/JIT test, six MIR tests, ten runtime tests.
- 88 focused lookup ABI/root/GC stress checks and two MIR probes pass, including
  baseline saved-program compatibility, actual freeing/quarantine, generations
  on/off and workers 1/4/8.
- Ordinary release compiler rebuild succeeds. Production source hashes are
  retained in `sources.json`; no new speed claim is based on this integration.

Commands used the isolated worktree and target directory
`/home/dan/orca/projects/language/target/memory-2026-10-18`:

```sh
cargo test --release --locked --offline -p badger_cli --test programs
cargo test --release --locked --offline -p badger_runtime -p badger_mir -p badger_codegen --lib
cargo build --release --locked --offline -p badger_cli
python3 bench/gc/check_lookup_split.py --ordinary /home/dan/orca/projects/language/target/memory-2026-10-18/release/badger --compatible /tmp/badger-I2a-binaries/baseline-fab8a02 --out reports/memory-management/main-integration-2026-10-18/lookup.json.gz
```
