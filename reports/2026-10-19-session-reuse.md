# One session across a rebuild loop (watch, `badger test`, REPL)

Landed 2026-10-19 on `perf/session-reuse-2026-10-19` (`bda588a`), merged to
`main` as `bda588a`. Closes the watch/REPL and `badger test` parts of C1.3/C1.4
and the C1.2b gap: each of those loops started a fresh `Session` per rebuild,
throwing away both caches a session keeps — the parsed standard modules
(C1.2b) and the checked standard library (C1.4).

## The mechanism

`Session::reset_for_build` drops the state a previous program left behind and
keeps those caches:

```rust
pub fn reset_for_build(&mut self) {
    self.diagnostics.clear();   // new `Diagnostics::clear`
    self.natives.clear();
}
```

Source files are deliberately *not* dropped: the cached standard-library spans
belong to their `FileId`s, so the cache stays valid only while those sources
stay in the map. Diagnostics must be dropped because `check_one` short-circuits
on `has_errors` and callers render the whole list.

Three loops now use one session:

| loop | site | lifetime of the session |
|---|---|---|
| `--watch` reload | `driver/src/live.rs` | thread-local; `Session` is single-threaded (its cache holds `Rc`s) and reloads run on the scheduler thread |
| `badger test` selections | `badger_cli/src/main.rs` | one per package, across `[entries]`/`[tasks]` |
| REPL evaluations | `driver/src/inspect.rs` | thread-local, taken for the duration of one evaluation and put back; the redefinition probes use the same session instead of two new ones |

## Measurements

Before/after binaries from the same tree, min of 3-5 runs:

| loop | before | after |
|---|---|---|
| `badger test`, 5-selection project: parse + typecheck summed over all selections | 208.1 ms | 70.4 ms |
| `--watch` reload (one edit): parse + typecheck | 8.6 + 30.2 ms | 0.9 + 5.0 ms |
| REPL, 5 typed items: front-end phases | 279.6 ms | 91.2 ms |

`BADGER_TIMINGS` shows the mechanism directly: a first rebuild reports
`typecheck.std-cache: miss` and parse.std ≈ 8-14 ms; the next reports
`hit (hits 1, misses 1)`, parse.std ≈ 0.9 ms, and `typecheck.groups` 9.8 →
0.4 ms.

## Verification

- `cargo test -p badger_cli --release --test programs` → 76 passed;
  `badger_driver` → 9 + 1 passed; `badger_base` → ok; `cargo fmt --check` clean.
- Goldens byte-identical: `codec_paths`, `collections_everyday`, `map_helpers`,
  `select_and_budget`, `budget_all_payloads`, `data_symbol_keys`,
  `sqlite_typed_rows`, `routes_typed`.
- REPL transcript identical to the pre-change binary over `let`, expression,
  definition, redefinition and *incompatible* redefinition (the rejection path
  is exactly the code that changed), apart from the random scratch-directory
  name in a diagnostic path.
- `--watch` exercised for real: run the program, edit a source file twice, and
  confirm `[live] version N: ... replaced` still fires and the second reload is
  the cheap one.

## Caveats

- The first rebuild in each loop is a cache miss (a watch session's first
  reload, a REPL's first line), so the standard library is paid once per loop.
- The watch session and the REPL session grow their `SourceMap`: every rebuild
  re-reads the changed files and allocates new `FileId`s
  (`SourceMap::add` always appends). A long session therefore retains one copy
  of each re-read file per rebuild. `badger test` is unaffected (bounded number
  of selections). A path-keyed source reuse would fix this and is not done.
- The REPL's session is not shared with the LSP (`analysis.rs` builds one per
  query) or with `badger inspect`; both are separate entry points.
