# Reusing the checked standard library across a session's entries (C1.4)

Landed 2026-10-19 on `perf/std-check-reuse-2026-10-19` (`feecb04`), merged to
`main` as `ea63b58`. Implements steps 1-7 of
[2026-10-19-std-check-reuse-plan.md](2026-10-19-std-check-reuse-plan.md), with
one premise of that plan corrected below.

## What it does

`Session` keeps a `StdCache` keyed by the std module set. On a hit,
`check_program_with` seeds the standard library's declared schemes, skips
`check_group` for std-only binding groups, replays cached std diagnostics,
seeds `store_summaries` and the effect fixpoint's std body-derived call edges —
so each additional entry in a project re-checks only its own code.

`store_summaries` also stopped cloning the whole summary map on every read
(`regions.rs` returned a fresh `FxHashMap` per `stores()` call); it now hands
out an `Rc`.

## The corrected premise

The plan claimed that a matching std module set yields matching `DefId`s, so the
cache could be keyed by `DefId` alone. **That is false in this repo**: `lower`
allocates standard definitions in an order that depends on the interner's
state, so two programs with an identical std module set produce different
`DefId`s (adding one user `impl` permutes std `num` impl-method ids). Keying by
`DefId` produced a flood of phantom `E0220`s on the second entry. The cache now
records each standard definition's `(Span, Symbol)` source identity and remaps
them onto the current program's `DefId`s on a hit.

## Traps, as implemented

- `skolem_owner` / `skolem_bounds` are never cached (program-global skolem ids).
- Effect rows are never cached as answers: cached std **body-derived call
  edges** are seeded into the merged fixpoint and protocol-method → impl edges
  are re-added from the current program, so a user `impl Display` that performs
  `Io` still widens its callers. Test `tests/projects/effectcache` checks the
  entry that widens an unannotated std protocol method *second*, i.e. on a cache
  hit, and still reports `E0501`.
- Capture asserts (debug) that no cached std scheme or constraint contains
  `Var`, `Proj` or `Skolem`.
- Diagnostic parity relies on `dedupe` (first occurrence wins).

## Measurements

`tests/projects/stdreuse` (package + 4 entries, all reaching the same std
modules; min of 5 runs, one run shown):

| phase (one entry) | before | after |
|---|---|---|
| `typecheck` | 16.4 ms | 2.9-3.0 ms |
| `typecheck.groups` | 9.75 ms | 0.44 ms |
| `typecheck.env` | 0.85 ms | 0.59 ms |
| `typecheck.after-groups` | 3.91 ms | 1.53 ms |
| `typecheck.schemes` | 1.24 ms | 0.015 ms |

Reported by the run: `typecheck.std-cache: hit (hits 4, misses 1)`. Over the
whole project, `BADGER_TIMINGS=1` typecheck drops from 76.8 ms to 28.2 ms and
the binding-group phase from 45.5 ms to 9.8 ms. A different std module set
(`tests/projects/stdmix`) reports `miss (hits 0, misses 2)` and checks normally.

## Verification

- `cargo test -p badger_cli --release --test programs` → 76 passed;
  `-p badger_typeck` → ok; `--test std_cache` → 3 passed; `cargo fmt --check` clean.
- Golden outputs byte-identical: `codec_paths`, `collections_everyday`,
  `map_helpers`, `text_bytes_builders`, `routes_typed`, `schema_wire`,
  `select_and_budget`, `budget_all_payloads`, `data_symbol_keys`,
  `sqlite_typed_rows`, `range_as_seq`.
- **Differential sweep**: `badger check <file>` output, stderr and exit status
  identical between this branch's binary and the pre-change binary for every
  `tests/run/*.bdg`, `tests/reject/*.bdg`, `examples/**/*.bdg` and
  `tests/projects/**/*.bdg` — 559 files, 0 differences.

## What remains

- Step 8 of the plan (`TypeEnv::build` std prefix, ~0.9 ms/check) — the hardest
  step, because `derive_impls` appends after explicit impls and
  `impls_by_head`/`structural_impls` hold `impls` indices.
- A persistent `Session` for watch/REPL (`driver/src/live.rs:114-117`, three
  `Session::new()` per REPL line in `inspect.rs`) — independently worth the
  ≈2.5 ms parse + ≈1.4 ms resolve per reload, and the only way this cache
  survives a reload.
- `badger test` builds a fresh `Session` per `[entries]`/`[tasks]` selection
  (`badger_cli/src/main.rs:637`), so it gets neither the parse cache nor this
  one; reuse needs a diagnostics reset (they accumulate and short-circuit
  `check_one`).
