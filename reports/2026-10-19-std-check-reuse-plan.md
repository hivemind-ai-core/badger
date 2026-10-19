# Reusing the checked standard library across entries and checks (C1.3/C1.4)

Plan of record for the largest remaining front-end item. Reconnaissance and
file:line evidence are from a read-only pass on `main` at `703dd04`;
measurements are from
[2026-10-19-frontend-session-reuse.md](2026-10-19-frontend-session-reuse.md)
and [2026-10-19-backlog-audit.md](2026-10-19-backlog-audit.md).

**Implemented 2026-10-19** (`perf/std-check-reuse-2026-10-19`, merged as
`ea63b58`): see [2026-10-19-std-check-reuse.md](2026-10-19-std-check-reuse.md)
for the landed design, measurements and the 559-file differential check.
**One premise below is wrong**: a matching std module set does **not** yield
matching `DefId`s (lowering allocates standard definitions in an order that
depends on the interner), so the cache keys definitions by `(Span, Symbol)` and
remaps them. Steps 1-7 are done; step 8 and the watch/REPL follow-ups are not.

## Why

Per extra `[entries]`/`[tasks]` selection the front end costs ≈15.6 ms
typecheck + ≈1.4 ms resolve + ≈2.5 ms parse, and 10.5–12.5 ms of the
typecheck is the standard library's ~700 binding groups
(`BADGER_TIMINGS=2`). Parsing was fixed by caching parsed std modules in the
`Session` (`43adbae`); checking is not reused at all.

## What is reused, and where

Design: **reuse the std *prefix* inside one `check_program`, keyed by the std
module set** (`StdKey`). Not a standalone std program, not scheme import by
name:

- Every entry's HIR has the std modules at `DefId`s `[0, k)` when the std
  module set matches, because `load` puts std first (`driver/src/lib.rs:509-513`)
  and `lower::collect` allocates deterministically (`hir/src/lower.rs:231-299`).
  When the set differs the key differs and the cache is not used.
- **Std never calls user code** (every std import is `std.*`), so no binding
  group mixes std and user definitions: skipping cached std-only groups is
  sound, not heuristic.
- Rejected: importing schemes keyed by `(path, name)` — `Scheme` embeds
  `DefId`s and `GenericId`s, and methods are not addressable by name at all
  (`lower.rs:274-287` vs `:799-812`); checking std into a standalone program —
  coherence/derives/effect rows are per-program (`env.rs:618-724`, `:418`;
  `program.rs:1436-1448`), and `Checked` borrows its own `Hir`.

## Steps

| # | step | sites | ~lines |
|---|---|---|---|
| 1 | `StdKey` = the sorted std module index list computed by `load_std` | `driver/src/lib.rs:52-56`, `:517-608`, `:509-513` | 25 |
| 2 | `StdCache` + `check_program_with(hir, interner, Option<&StdCache>)`, `check_program` as the wrapper so all callers keep compiling | `typeck/src/program.rs:70-105`, `:420`, `typeck/src/lib.rs:20-22` | 90 |
| 3 | Declared-scheme loop: seed `g.schemes` for std defs from the cache, replay cached std diagnostics | `program.rs:446-471` | 35 |
| 4 | Binding groups: skip `check_group` for cached std-only SCCs, seed `pending_schemes`; keep the graph/edge build | `program.rs:582-604` | 50 |
| 5 | `Session::std_check: Option<StdCache>` + key; pass in on a hit, capture on a miss | `driver/src/lib.rs:29-46`, `:755`, `:2204-2218` | 40 |
| 6 | `effect_fixpoint`: seed from the cache **and merge** (see traps) | `program.rs:1343-1497`, `:757` | 35 |
| 7 | `store_summaries`: seed the fixpoint from the cache, skip cached defs, and stop cloning the whole map per read — return `Rc<FxHashMap<DefId, Vec<(usize, usize)>>>` (`regions.rs:607-608` clones on every call, and `stores()` calls it per store site) | `regions.rs:604-693`, `:313`, `program.rs:67-69`, `:444` | 30 |

Out of scope here: splitting `TypeEnv::build` for the std prefix (~0.9 ms; the
hardest step — `derive_impls` appends after explicit impls, so std impls are
not a prefix, and `impls_by_head`/`structural_impls` hold indices) and a
persistent `Session` for watch/REPL (`driver/src/live.rs:114-117`, three
`Session::new()` per REPL line in `inspect.rs:130,161,168`).

## Traps (all observed in the source; each needs its guard)

- **Never cache `skolem_owner`/`skolem_bounds`** — keyed by a program-global
  skolem id (`ty.rs:254-258`, writers `program.rs:1815-1817`, reader
  `solve.rs:1647`). Keep them per program.
- **Effect rows are not closed over std.** `effect_fixpoint` adds
  protocol-method → impl edges for *user* impls (`program.rs:1436-1448`) and
  `std/prelude.bdg:155-157` declares `Display::display` unannotated, so a
  user impl that performs `Io` widens a std scheme today. The cached std
  subgraph must be seeded into the merged call graph, not used as the answer.
- **A cached scheme must be `Var`-free and `Proj`-free.** `TyVid` indexes the
  per-`InferCtxt` table (`ty.rs:206-207`) — a stale `Var` is undefined
  behaviour — and `Proj` binds to the thread-local `ASSOC` table
  (`ty.rs:92-100`). Assert both on capture.
- **Diagnostics parity depends on `dedupe`** (`driver/src/lib.rs:736`,
  `base/src/diagnostics.rs:103-110`): replaying cached std diagnostics once
  per session is equivalent only because equal diagnostics are collapsed, and
  `check_one` alone does not dedupe. Keep the replay on the existing path.
- **Spans/`FileId`s** stay valid only if the session keeps the parsed std
  module's `FileId` (it does since `43adbae`). Do not let a cache outlive its
  `Session`.

## Acceptance

- `badger check` diagnostics byte-identical before/after on
  `examples/design/service` and a multi-entry fixture, both with diagnostic
  text and with `--json`.
- A fixture whose entries import *different* std modules must miss the cache
  and still be correct (`std.http` in one entry only).
- `tests/run` goldens and `cargo test -p badger_cli --release --test programs`
  unchanged; `cargo test -p badger_typeck` passes.
- `BADGER_TIMINGS=1` per-entry numbers before/after on the 4-entry fixture,
  plus `BADGER_TIMINGS=2` for the group phase.
- A counter for cache hits/misses (print under `BADGER_TIMINGS`), so the win
  is visible and a future regression is detectable.
