# Front-end reuse across a project's entries — 2026-10-19

Status: step 1 implemented and measured on `perf/frontend-reuse-2026-10-19`
(base `main` = 529e96f). Scope: compiler front end (backlog C1.2b), which the
parallel GC and MIR/codegen owners do not touch.

## Where the time goes

`badger check` on a project checks the main entry **and every `[entries]` and
`[tasks]` module** (`Session::check_package` → `check_one` per selector), and
each `check_one` → `Session::load` → `load_std` re-parsed every standard
module it needs, then lowered the whole program and typechecked it again.

Summed `BADGER_TIMINGS=1` phases for one `badger check` of a synthetic project
whose entries are trivial programs (created under `/tmp/proj`; the manifest's
`entry` and each `[entries]` name point at `src/e1.bdg`, `src/e2.bdg`, …):

| Project | checks | parse (ms) | resolve | typecheck | front end |
|---|---:|---:|---:|---:|---:|
| 1 entry | 2 | 15.67 | 3.29 | 31.24 | 50.20 |
| 4 entries | 5 | 34.10 | 7.09 | 78.15 | 119.34 |

So each additional entry cost ≈ **5.7 ms of parsing** (the standard library)
plus ≈ **15.6 ms of typechecking** — both identical work, repeated.

## Step 1: parse once per session (done)

`Session` now keeps the parsed standard modules it has loaded
(`std_parsed: HashMap<String, ParsedModule>`); `load_std` clones a cached
module for later entries instead of re-adding and re-parsing its embedded
source. The `FileId`s are kept with the modules, so diagnostics and spans are
unchanged, and the embedded sources cannot change within a session (they are
not affected by editor overlays, which are read for user files only).
`ast::Ast` and `hir::ParsedModule` gained `Clone` to make the reuse possible
without restructuring `lower_program`'s signature.

| Project | parse before | parse after | front end before | after |
|---|---:|---:|---:|---:|
| 1 entry (2 checks) | 15.67 ms | 10.61 ms | 50.20 ms | 46.23 ms |
| 4 entries (5 checks) | 34.10 ms | 12.34 ms | 119.34 ms | 98.53 ms |

−21.8 ms for four entries (**−17.4%** of front-end time), i.e. −5.4 ms per
extra entry; typecheck time is unchanged, as expected.

Verification: `tests/run` goldens byte-identical (`codec_paths`,
`schema_wire`, `routes_typed`, `collections_everyday`), `badger check` clean on
`examples/design/service` and a single file, `cargo test -p badger_cli
--release --test programs` passes (76 tests) and `-p badger_driver` passes.

## Step 2: check once per session (not done, sized)

After step 1 the remaining per-entry cost is the typecheck: ≈15.6 ms per
entry in this fixture (`typecheck.groups` ≈9.5–10.5 ms, `after-groups`
≈3.7–4.2 ms, `schemes` ≈1.2 ms, `env` ≈0.9 ms). That is C1.4's
"checked-library reuse": `badger_typeck::check_program(hir, interner)` builds
a fresh global context and re-infers every standard function body for each
entry, and `Checked<'a>` borrows the program it was made from. Reusing a
checked standard prefix needs the typechecker to accept pre-populated schemes
for standard definitions (and to keep expression-level results consistent),
which is a design change inside `badger_typeck`, not a cache in the driver.
Worth ≈16 ms per extra entry (~64 ms on a four-entry `badger check`).

Same shape applies to `badger build`/`release` when a project has several
entries; `badger run NAME` selects a single entry and does not pay it.

## Method

- Fixture: `/tmp/proj` with `badger.toml` `entry = "src/e1.bdg"` plus
  `[entries] e1..eN` pointing at one trivial `main` module each.
- Reported numbers are phase sums over all checks of one invocation
  (`BADGER_TIMINGS=1`, regex-summed over every printed block), minimum of
  repeated runs; wall-clock agreement was checked separately (`check` 148.6 →
  120.2 ms at four entries under load, minimum of five runs).
