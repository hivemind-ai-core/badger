# The CLI's process-wide allocator (§3.1 follow-up): jemalloc behind a feature

Follow-up to the §3.1 item, whose status ends by saying that what allocation
still costs is better taken by a process-wide allocator, and to
[`reports/2026-10-20-typeck-allocation.md`](../2026-10-20-typeck-allocation.md),
which measures jemalloc on a scratch build (−13 % typecheck, −16 % wall), names
its gates and records it as "the recommended follow-up, not done".

Measured 2026-10-21 on `perf/alloc-and-dead-calls`, based on `main` at
`fcc45219`. Status: **adopted, off by default**.

## What landed

- `crates/badger_cli/Cargo.toml`: an optional `tikv-jemallocator` dependency
  (declared once in the workspace `Cargo.toml`) and a `jemalloc` feature with
  `default = []`.
- `crates/badger_cli/src/main.rs`: `#[global_allocator] static GLOBAL_ALLOCATOR:
  tikv_jemallocator::Jemalloc`, behind `#[cfg(feature = "jemalloc")]`.

The compiler and the JIT'd program share one address space, so one
`#[global_allocator]` covers both. `cargo build --release -p badger_cli
--features jemalloc` selects it for one build; `default = ["jemalloc"]` selects
it for every build. Nothing else changed: no runtime, GC or MIR code.

## Gate: the corpus output is identical

[`sweep.py`](sweep.py) runs `badger check FILE` from the repository root over
`tests/run/*.bdg`, `tests/reject/*.bdg`, `examples/**/*.bdg` and
`tests/projects/**/*.bdg` — **722 files** (302 + 349 + 33 + 38), the globs of the
2026-10-19 sweep. Per file the two binaries run back to back with the order
alternating, and exit status, stdout and stderr are compared; the `[timings]`
lines are excluded, since they are the run's, not the program's. That covers
the 349 `tests/reject` diagnostics, which are stderr-only.

**722 files × 3 rounds = 2166 paired runs, 0 differences.**

## Measurements

The same sweep, `BADGER_TIMINGS=1`, 3 rounds. A round's unit is the sum over
the corpus; the table is the median over rounds, and the parentheses give the
per-file median.

| 3 rounds, paired | `main` | jemalloc | change |
|---|---|---|---|
| typecheck | 16.60 s (22.76 ms) | 13.91 s (19.10 ms) | **−16.2 %** |
| parse | 11.37 s (15.48 ms) | 8.92 s (12.56 ms) | **−21.6 %** |
| resolve | 2.78 s (3.67 ms) | 2.11 s (3.06 ms) | **−24.3 %** |
| wall | 36.55 s (49.8 ms) | 30.53 s (43.1 ms) | **−16.5 %** |

Three runs of the same sweep on the previous tip (`5f477c2`) give typecheck
−15.8/−15.9/−16.0 % and wall −16.8/−16.9/−17.1 %; this run, on the landed
tip, gives −16.2 % and −16.5 %. The effect is stable, and the per-file
pairing (the method the typeck-allocation report used) is what keeps it so
on this shared host. The gain is not in one phase: every phase that allocates gets faster,
which is what a faster `malloc`/`free` predicts, and the 2026-10-20 report's
scratch build measured the same −13 %/−16 % on the 610-file corpus of its
day, before this feature existed.

## Peak RSS

`ru_maxrss` of the child process, one process per measurement (MiB).

| workload | `main` | jemalloc | change |
|---|---:|---:|---:|
| `badger check`, mean over the 722 files | 31.5 | 45.3 | **+44 %** |
| `badger check`, worst file | 47.2 | 60.0 | +27 % |
| `badger run --release bench/messages.bdg` | 47.5 | 64.1 | +35 % |
| `badger run --release bench/parallel_alloc.bdg` | 58.4 | 78.0 | +34 % |
| `badger run --release bench/collections.bdg` | 87.9 | 108.1 | +23 % |
| `badger run --release bench/records.bdg` | 56.9 | 107.0 | +88 % |
| `badger run --release bench/heap.bdg` | 65.5 | 94.7 | +45 % |
| `badger run --release bench/runtime.bdg` | 43.3 | 59.7 | +38 % |
| `badger run --release --workers 4 bench/parallel.bdg` | 36.3 | 66.7 | +84 % |

This is the gate the typeck-allocation report asked for, and it is the reason
the feature is off by default: a throughput gain of ~16 % that costs 23-88 %
of peak RSS is a trade each user has to make. A per-thread arena per worker is
most of the JIT's cost (`--workers 4` is the extreme), and jemalloc's page
return is lazy (the default decay is seconds), so a process that ends quickly
peaks higher than the collector's own retained pool alone would suggest.

## Verification

- `cargo build --release -p badger_cli` and `... --features jemalloc`: both
  build; the jemalloc binary links `tikv-jemallocator` (`nm` shows its
  symbols, the default binary has none).
- `cargo test -p badger_cli --features jemalloc --test programs -- --test-threads=4`:
  **89 passed, 0 failed** — the corpus in interpreted, compiled, `--workers 4`
  and release modes, plus `tests/reject`, through the jemalloc binary
  (`CARGO_BIN_EXE_badger` is built with the feature).
- `cargo test -p badger_cli --features jemalloc --test stress`: 2 passed;
  `--test gc_retention`: 1 passed.
- A first, fully parallel run of the corpus test failed `native_modules`
  ("the replay ran the abandoned call": a replay that is expected to finish
  inside 20 s took longer under 89 tests on 16 cores). It passes alone under
  jemalloc and alone on `main`, so it is the load-sensitive budget, not the
  allocator. It is the same class of flake the typeck-allocation report
  recorded for `run_programs_multicore`.
- `cargo fmt --all --check` reports pre-existing diffs in
  `badger_mir/src/inline.rs`, `badger_mir/src/opt/superopt.rs`,
  `badger_runtime/src/{codec,intrinsics_codec,string}.rs` and
  `badger_sqlite/src/lib.rs`; none is touched by this change.

## Reproducing

```sh
cargo build --release -p badger_cli                && cp target/release/badger /tmp/badger-base
cargo build --release -p badger_cli --features jemalloc && cp target/release/badger /tmp/badger-jem
python3 reports/alloc-deadcalls/sweep.py /tmp/badger-base /tmp/badger-jem --rounds 3 --out /tmp/alloc.json
```

Peak RSS, one process per measurement (`ru_maxrss` is the child's peak):

```sh
python3 -c 'import resource,subprocess,sys
subprocess.run(sys.argv[1:], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
print(resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss)' \
    /tmp/badger-jem run --release bench/records.bdg
```
