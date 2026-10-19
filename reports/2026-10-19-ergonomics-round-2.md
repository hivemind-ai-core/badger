# Ergonomics, round two — implementation notes

**Date:** 2026-10-19
**Branch:** `feature/ergonomics-round-2-2026-10-19` (worktree `/tmp/language-ergonomics-2-2026-10-19`)
**Implements:** the unblocked items of
[2026-10-19-library-and-framework-changes.md](2026-10-19-library-and-framework-changes.md)
and [2026-10-19-language-and-compiler-changes.md](2026-10-19-language-and-compiler-changes.md)
that need nothing from the compiler team.

This round picks up where
[2026-10-19-library-and-framework-implementation.md](2026-10-19-library-and-framework-implementation.md)
stopped: the items that were ready but not started, and the ones that were
waiting on a decision that has since been made.

## What landed

| Item | What | Where |
|---|---|---|
| **T-06** | `badger bench [PATH] [--out FILE] [--compare FILE] [--workers N]`: the measurement programs under `bench/` are run in release mode, their measurements collected, written as JSON, and compared with a baseline | `crates/badger_cli/src/bench.rs` |
| **T-07** | `badger rename PATH --line N --col N --name NEW`: the definition and every use of it are renamed in the files of the project | `crates/badger_cli/src/main.rs` |
| **T-08** | The language server advertises `textDocument/formatting`, and the editors page says how to turn on format-on-save (VS Code, Neovim, Helix) | `crates/badger_cli/src/lsp.rs`, `docs/editors.md` |
| **T-09** | `badger new --template=cli\|service\|framework\|lib\|worker`: the templates live in `templates/` and are embedded in the binary | `crates/badger_cli/src/templates.rs`, `templates/` |
| **T-10** | `badger repl --load FILE`: a file's definitions are loaded into the session before the prompt | `crates/badger_cli/src/main.rs` |
| **T-12** | `badger test --watch`: the tests run again on every change of the project's sources | `crates/badger_cli/src/main.rs` |
| **T-13** | `badger generate service\|cli\|library`: typed modules, checked and tested in a copy of the project before anything is written | `crates/badger_cli/src/generate.rs` |
| **T-14** | `badger doc --open`: the reference is written as one HTML page (the Markdown rendered) and opened | `crates/badger_cli/src/doc.rs` |
| **D-03** | A migration cookbook: Go, Python, Rust and TypeScript, one representative program each, the Badger equivalent, and the differences to know about | `docs/migration-cookbook.md` |

## What is still open, and why

| Item | Why |
|---|---|
| **T-01** `badger install` | Needs a decision the report does not make: where a dependency comes from (a registry, a git URL, a path). The lockfile machinery (`badger update`) is there; the source of a package is not. |
| **T-15** REPL history and completion | Needs a line-editing dependency (`rustyline` or `reedline`); the CLI has no such dependency today, and adding one is a supply-chain decision rather than a feature. |
| **T-12 --changed` | Needs the compiler team's incremental check (**L-17**) to be fast; `--watch` landed. |
| **S-14** a lazy `Seq` from a streaming query | The collection side is there (`std.seq` has `map`, `filter`, `take`, `skip` and `scoped`); the query side needs a cursor in the native SQLite module, which is a change to the native module's handle and cancellation rules. Not a library-only change. |
| **F-I3** a pipeline `around` stage | The type checker must infer a function-typed parameter for a stage whose type parameters are inferred at the use site. |
| **S-05**, **S-09**, **S-11**, **S-12** | Compiler work: literal inference, the flatten marker (**L-09**), channels across the system graph (**L-16**), and the method-resolution rule. |
| **F-N3**, **F-N4**, **F-I2**'s derive half | Blocked on the derive machinery with per-variant metadata (**L-20**). The `HttpStatus` protocol and `pipeline.recover_with` are the fallbacks that landed. |
| **S-01**, **S-07**, **S-15**'s macros | `bail!`, `validate!` and `default!` are macros; the function forms are what the standard library can offer on its own. |
| **F-A5**'s compile-time check | The rule is documented; rejecting `task.scope()` inside a provider is compiler work (**L-15**). |

## Notes on the design

**`badger bench` collects what the programs print.** The measurement
programs print their own numbers (`label: value unit`), so the command
runs them, collects those lines, and writes them as JSON
(`{program: {label: {value, unit}}}`). A baseline is that file; a
comparison reports the change of every measurement against it. The
command runs the programs through the tool itself (`badger run
--release`), so what it measures is what a user measures.

**`badger rename` applies the edits the query computes.** The rename
query already worked out the edits; the command applies them, from the
end of each file backwards, so the ranges stay valid.

**`badger doc --open` writes one page.** The reference is Markdown; the
page renders it (with `pulldown-cmark`, already in the lockfile) and
rewrites the links between the pages to point at the sections of the same
page. The page goes into `--out` when a directory is given, and into a
private directory otherwise.

**`badger new --template` puts the framework where a new project starts
from.** The framework example was the de-facto template; it is
`--template=framework` now, and the other templates are the shapes a new
project usually has: a program with options (`cli`), a service with
routes (`service`), a library (`lib`), a worker (`worker`). The template
files live in `templates/` and are embedded in the binary, so the tool
does not depend on the repository's layout at run time; `@name@` in a
file is replaced with the package name.

A library package has no `main`, so `badger run` on one exits 1 with
"`x` is a library: it has no `main` to run" — the same as `badger init
--lib`. `badger check` and `badger test` are what it is for.

## The one thing the round does not do

`badger new --template=service` and `--template=framework` answer
requests **in process** rather than binding a socket, so the program
ends; a real server never exits, which would make the template
impossible to check. Their `README.md` files show how to bind
`http.server` for real serving.

## Tests

- `crates/badger_cli/tests/ergonomics.rs`: `rename`, `repl --load`,
  `bench` (with its baseline and comparison), and `doc --open`.
- `crates/badger_cli/tests/templates.rs`: the templates and the new
  generators.
- `crates/badger_cli/tests/lsp_format.rs`: the formatting capability and
  the edits it answers with.
- The whole CLI suite passes (`cargo test -p badger_cli`), except the
  pre-existing `stress_programs` garbage-collector crash, which is not
  from this work.
