# `badger` command line

Every subcommand of `badger` (version `0.1.0`), its arguments, flags, and
exit status. `badger --help` and `badger COMMAND --help` print the same
options. Configuration files and environment variables are in
[config.md](config.md).

Conventions used on this page:

- **PATH** is an entry file (`app.bdg`) or a project directory (one with a
  `badger.toml`). Where PATH is optional it defaults to `.`, the current
  directory's project.
- Arguments for the program go after `--`: `badger run app.bdg -- --verbose x`.
- Human-readable diagnostics go to standard error. `--json` output goes to
  standard output.
- Output shown with `$` was printed by the example project below, run from
  its directory, unless the command names another path.

## Example project

`badger.toml`:

```toml
[package]
name = "demo"
entry = "src/main.bdg"
version = "0.1.0"

[runtime]
workers = 2

[config]
port = 8080
db = { path = "demo.db" }

[profile.prod]
port = 80

[profile.prod.runtime]
workers = 4

[entries]
admin = "src/admin.bdg"

[tasks]
greet = "src/tasks.bdg#greet"
```

`src/main.bdg`:

```badger
import std.io
import std.config
import std.test

type Settings = {
    port: Int,
    db: { path: String },
}

pub enum AppError {
    config(source: config.Error),
    io(source: io.Error),
} derive(Error, From)

/// Half of an even number.
requires n % 2 == 0
ensures result * 2 == n
pub fn half(n: Int) -> Int { n / 2 }

pub fn main() -> Result<Unit, AppError> {
    let s = config.load<Settings>("DEMO_")?
    io.println($"port={s.port} db={s.db.path} half={half(s.port)}")?
    ok(())
}

fn test_half() -> Result<Unit, String> {
    test.assert_eq(half(10), 5)
    ok(())
}
```

`src/admin.bdg`:

```badger
import std.io

pub fn main() -> Result<Unit, io.Error> {
    io.println("admin entry")
}
```

`src/tasks.bdg`:

```badger
import std.io

pub fn greet(names: List<String>) -> Result<Unit, io.Error> {
    for name in names {
        io.println($"hello {name}")?
    }
    ok(())
}
```

## Exit status

| Status | Meaning |
|---|---|
| 0 | Success. For `run`, `start`, `task`: the program's `main` returned normally (or `ok(...)`). |
| 1 | Failure: the program does not check, a test failed, a `--check` found differences, `main` returned `err(...)`, a replay's outcome differs from the recording, or the command could not do its job (missing file, bad flag value). |
| 2 | Command-line usage error (unknown flag, missing argument, `--reconstruct` without `--watch`), reported by the argument parser. |
| 3 | `badger replay`: the program asked for an operation the bundle does not hold (or sent different content, or needed a redacted value), or ended without performing operations the bundle recorded. |
| 70 | Internal error: a bug in Badger, not in the program. The message ends with "please report it"; `RUST_BACKTRACE=1` adds a backtrace. |
| 71 | Fatal: the runtime cannot allocate a task stack (address-space limit `ulimit -v`, or `vm.max_map_count`). |
| 101 | The program failed with a defect (a panic, a failed contract, an arithmetic or bounds failure) in its main task, or the main task was cancelled. |
| N | A program that calls `process.exit(N)`, or whose `main` returns the `Int` N, exits with N (0 to 255). `badger replay --compare` also exits with the program's status. |

## Run and check

### `badger check [PATH] [--json]`

Parse and type-check without running. A project is checked as a whole:
its main entry, every `[entries]` module, and every `[tasks]` module; at a
workspace root, every member package. A single file need not have a
`main`.

| Flag | Meaning |
|---|---|
| `--json` | Print diagnostics as a JSON array on standard output (`[]` when it checks). Each has `code`, `severity`, `message`, `labels` (file, module, start and end line/column, message), `notes`, `expected`, `actual`, `scope`, `related`, and `schema_version` (1; it changes when a field changes meaning or is removed). Lines and columns are 1-based; columns count characters. (`badger query diagnostics` gives the same diagnostics flattened to one location each, with the same `schema_version`; the language server sends `label`, `expected`, `actual`, `scope`, and `notes` in each diagnostic's `data`.) |

Exit status: 0 when there are no errors (warnings allowed), 1 otherwise.

```sh
badger check                 # the current project
badger check src/util.bdg    # one file (checked with its project, if any)
```

### `badger run [ENTRY] [FLAGS] [-- ARGS...]`

Compile with the JIT and run. ENTRY is:

- an existing file or directory: a source file, a project directory, or a
  binary MIR file written by `badger compile`;
- otherwise, a name in the current project's `[entries]` table
  (`badger run admin`);
- omitted: the current project's main entry.

A project's `[config]`, profiles, `[runtime]`, and `[env]` apply when the
project is run as a directory (`.`, a directory path, or an entry name). A
source file run by its path runs without them, even inside a project.

An argument that looks like a path (contains `/`, or ends in `.bdg` or
`.bdgmir`) but does not exist is an error, not an entry name.

| Flag | Value | Meaning |
|---|---|---|
| `--release` | | Optimize: the release MIR pipeline and optimized code generation. |
| `--opt LEVEL` | `release`, `aggressive`, `superopt` | With `--release`: select the MIR level. `release` is the default; `aggressive` repeats inlining and simplification; `superopt` also runs the verified arithmetic/boolean superoptimizer. The level is recorded when compiling binary MIR. |
| `--workers N` | 1 to 64 | Worker threads that run tasks. Default: `[runtime] workers`, else 1. Out of range is error E0013. |
| `--profile NAME` | | Merge `[profile.NAME]` into the configuration that `config.load` reads, and `[profile.NAME.runtime]` into the runtime settings. Needs a project directory. |
| `--watch` | | Keep running; on a source change, recheck and replace compatible functions (new calls use new code). An incompatible change (a type definition, a function's type or effects) is rejected and the old version keeps running. |
| `--reconstruct` | | With `--watch` only: on an incompatible change, cancel the program, halt its resources, and start the new version. |
| `--inspect ADDR` | `HOST:PORT` | Serve inspection snapshots and a remote REPL at ADDR (see `badger inspect --connect`, [repl.md](repl.md)). |
| `--record FILE` | | Write a reproduction bundle to FILE (see `badger replay`). Runs on one worker (`--workers` above 1 is refused, E0013). Not with `--watch` or `--inspect`. |
| `--bench` | | After the program ends, report on stderr how long it ran, timed after all compilation. |
| `--debug` | | Register debug information for generated code with a debugger (`gdb --args badger run --debug app.bdg`). See [debugging.md](../debugging.md). |
| `--dump-mir` | | Print the program's MIR on stderr before code generation, after MIR optimization (`BADGER_DUMP_MIR_PASSES=1` also prints it after each pass). |

A binary MIR file runs as saved: `--watch`, `--inspect`, `--profile`, and
`--record` are refused (E0013). `--workers`, `--release`, and
`BADGER_<SETTING>` variables still apply.

Exit status: the program's (see the table above); 1 when it does not
check.

```sh
$ badger run
port=8080 db=demo.db half=4040
$ badger run --profile prod
port=80 db=demo.db half=40
$ badger run admin
admin entry
$ badger run --release --workers 4 . -- some args
port=8080 db=demo.db half=4040
```

### `badger test [PATH] [FILTER] [--json] [--list] [--exact]`

Run `test_*` functions. A test function takes no parameters and returns
`Unit` or a `Result`; a failed assertion, a defect, or an `err` result
fails it. A `test_*` function with parameters is not run (warning E0142). Tests run from the
package's modules the program loads: the entry module and the modules of
the same package it imports (a dependency's tests run when that package is
tested). For a project, the modules reached from each `[entries]` and
`[tasks]` module are included too (each test runs once); at a workspace
root, every member package is tested.

The first argument is PATH when it names an existing file or directory;
otherwise it is FILTER. FILTER keeps the tests whose name contains it.

| Flag | Meaning |
|---|---|
| `--json` | Print one JSON object at the end: `{"passed":N,"failed":N,"tests":[...]}`; each test has `name`, `ok`, `kind` (`passed`, `assertion`, `defect`, `cancelled`, or `error` for a returned `err`), `message`, `location` (`file:line:column` of a failed assertion or defect), `ms`, and `detail` (the line the text report prints). |
| `--list` | Print the selected tests' names (with `--json`, `{"tests":[{"name"}]}`) without running them. |
| `--exact` | FILTER names one test: its full name (`orders.service.test_create`) or its function's name. |

Exit status: 0 when every test that ran passed (also when the filter
matched none), 1 when one failed or the program does not check.

```sh
$ badger test half
test test_half ... ok

test result: ok. 1 passed; 0 failed
```

### `badger task NAME [--project DIR] [--profile NAME] [-- ARGS...]`

Run a user task: a function declared in the project's `[tasks]` table
(`greet = "src/tasks.bdg#greet"`). The function takes no parameters or a
`List<String>` of ARGS.

| Flag | Default | Meaning |
|---|---|---|
| `--project DIR` | `.` | Project directory. |
| `--profile NAME` | | As for `badger run`. |

```sh
$ badger task greet -- ann bob
hello ann
hello bob
```

### `badger replay BUNDLE [--compare] [--release]`

Run a program again from a bundle written by `badger run --record`. The
program, its arguments, and its profile are taken from the bundle.

- Every recorded external operation (HTTP requests, subprocesses, file
  reads, writes and listings, environment variables read with `std.env`
  or `std.config`, clocks, OS-seeded random sources) is answered from the bundle; nothing
  is requested, run, or written.
- Operations a bundle cannot hold (databases, the HTTP server, native
  module functions not marked as recorded) stop the replay with status 3.
- A program whose sources differ from the recorded ones is refused
  (E0016).

| Flag | Meaning |
|---|---|
| `--compare` | Run a changed program against the recording: report differing operations and outcome instead of stopping, and exit with the program's status. |
| `--release` | Optimize generated code. |

Exit status: 0 when the recorded outcome (exit code, output, failure) is
reproduced, 1 when it differs, 3 when the run left the recording (see the
table above).

```sh
$ badger run --record bug.json
port=8080 db=demo.db half=4040
recorded 0 operations to bug.json
$ badger replay bug.json
port=8080 db=demo.db half=4040
replay: 0 recorded operation(s) answered from the bundle
replay: reproduced the recorded outcome (exit 0)
```

### `badger watch [PATH] [-- COMMAND...]`

Re-run a command when a source file of the project changes. The files
are polled (their size and modification time), so the command needs no
file-watching dependency. The default command is `badger check PATH`;
with `-- COMMAND...` the given command runs instead, in the project's
directory.

| Argument | Meaning |
|---|---|
| `PATH` | Project directory to watch (default `.`). |
| `COMMAND...` | The command to run on a change (after `--`). |

Every `.bdg` file under PATH is watched. A change is reported, and the
command runs again. Its exit status is reported, but does not end the
watch; Ctrl-C does.

```sh
$ badger watch
watching /home/you/shop (3 source file(s))
running: badger check /home/you/shop
exit status: 0
changed: /home/you/shop/src/main.bdg
running: badger check /home/you/shop
exit status: 0
```

## Compile and deploy

### `badger compile [PATH] [-o FILE] [--profile NAME] [--workers N] [--opt LEVEL]`

Check a program and save it as binary MIR. `badger run FILE.bdgmir` then
only generates code. The saved program has the MIR optimizations of the
selected level (`release` by default), and carries the effective
configuration, runtime settings, and optimization level.

| Flag | Default | Meaning |
|---|---|---|
| `-o`, `--output FILE` | `NAME.bdgmir` | For a file `app.bdg`: `app.bdgmir` beside it. For a project directory: `DIR/PACKAGE.bdgmir`. |
| `--profile NAME` | | Merge `[profile.NAME]` into the saved configuration. |
| `--workers N` | | Worker count saved as the default (`badger run --workers` still overrides). |
| `--opt LEVEL` | `release` | MIR level: `release`, `aggressive`, or `superopt`. A run at a higher level applies its extra passes to the loaded program. |

```sh
$ badger compile
wrote ./demo.bdgmir (14674 bytes)
```

### `badger release [PATH] [-o DIR] [--profile NAME] [--workers N] [--opt LEVEL]`

Write a self-contained release directory: the `badger` executable, the
program as binary MIR, the native libraries it calls, the lockfile, and a
manifest of digests (`release.json`). An earlier release at DIR is
replaced only when the new one is complete. Details: [releases.md](../releases.md).

| Flag | Default | Meaning |
|---|---|---|
| `-o`, `--output DIR` | `release/PACKAGE` (relative to the current directory) | Release directory. |
| `--profile NAME` | | Merge `[profile.NAME]` into the saved configuration. |
| `--workers N` | | Worker count saved as the default. |
| `--opt LEVEL` | `release` | MIR level saved with the program: `release`, `aggressive`, or `superopt`. |

```sh
$ badger release
wrote release release/demo (demo: 14674 bytes); start it with release/demo/run
```

### `badger start DIR [--workers N] [-- ARGS...]`

Start a release. Its target, `badger` version, program, lockfile, and
native libraries (digests and native ABI version) are checked before any
program code runs. `DIR/run ARGS` does the same.

| Flag | Default | Meaning |
|---|---|---|
| `--workers N` | the release's saved setting | Worker threads. |

```sh
badger start release/demo -- --port 8080
```

## Format

### `badger format [PATHS...] [--check]`

Rewrite `.bdg` files in the canonical layout. A directory is searched
recursively for `*.bdg` (symlinked directories are not followed). Default:
`.`. A named file must exist and end in `.bdg`.

| Flag | Meaning |
|---|---|
| `--check` | Do not write. Print `would reformat FILE` for each file that would change; exit 1 if any would. |

```sh
$ badger format --check fmt.bdg
would reformat fmt.bdg
```

## REPL

### `badger repl [--connect ADDR]`

Interactive session: definitions accumulate, expressions evaluate. With
`--connect ADDR`, evaluate inside a program started with
`badger run --inspect ADDR`. Full reference: [repl.md](repl.md).

## Tools

### `badger inspect WHAT [PATH] [--json] [--profile NAME] [--connect ADDR]`

Without `--connect`, WHAT reads the program or project at PATH:

| WHAT | Prints |
|---|---|
| `plans` | Each `system { }` plan (its nodes, providers, references, startup order) and each `pipeline { }` (its stages). |
| `config` | The effective `[config]` values, merged with `--profile NAME`, each with its source (`[config]` or `[profile.NAME]`). |
| `runtime` | Every runtime setting's effective value and its source: `default`, `[runtime]`, `[profile.NAME.runtime]`, or `env`. `--profile` applies here too. |

With `--connect ADDR` (a program started with `badger run --inspect
ADDR`), WHAT is a snapshot of the running program: `tasks`, `channels`,
`system`, `heap`, `code` (the live code version), or `all`. `tasks`,
`channels`, and `system` print as tables; the others print JSON.

| Flag | Meaning |
|---|---|
| `--json` | Print JSON instead of text. |
| `--profile NAME` | Profile for `config` and `runtime`. |
| `--connect ADDR` | Address of a running program. |

```sh
$ badger inspect runtime tests/projects/tooling --profile strict
workers = 1    # default
cleanup_grace_ms = 2000    # from [runtime]
http_max_body_bytes = 16777216    # default
http_max_header_bytes = 65536    # default
http_max_in_flight = 1024    # default
http_max_stream_buffer_bytes = 1048576    # default
trace_capacity = 2    # from [profile.strict.runtime]
trace_overflow = drop_newest    # from [profile.strict.runtime]
log_level = info    # default
log_format = text    # default
task_stack_bytes = 8388608    # default
blocking_threads = 16    # default
heap_max_bytes = 0    # default
```

### `badger query KIND [PATH] [FLAGS]`

Answer one question about a program as JSON on standard output. Positions
are 1-based; columns count characters (Unicode scalar values), as in
`badger check --json`. PATH is a source file, checked with its project if it is in
one.

| KIND | Needs | Answer |
|---|---|---|
| `diagnostics` | PATH | Array of diagnostics (`file`, `line`, `col`, `end_line`, `end_col`, `severity`, `code`, `message`, `notes`, `label`, `expected`, `actual`, `scope`, `related`, `declarations`). |
| `check` | PATH (file or project) | `{ok, diagnostics, inputs, unused_overlays, roots, tests, tasks}`; with `--overlays`, of the proposed contents. See [agents.md](../agents.md#checking-proposed-edits). |
| `symbols` | PATH | The file's definitions (`name`, `kind`, `detail`, `line`, `col`, `container`). |
| `symbol` | PATH, `--name` | A definition by name: `orders.service.create` (module path and name), `Order.total` (a type's member), or just `create`. Each match has `name`, `kind`, `container`, `detail`, `hover`, `file`, `line`, `col`, and `references`. PATH may be a project. |
| `api` | PATH, or a standard module (`std.list`) | The public interface: declarations, signatures with effects and contracts, docs. |
| `hover` | PATH, `--line`, `--col` | `{"text": ...}`: type or signature at the position. |
| `definition` | PATH, `--line`, `--col` | Location of the definition. |
| `references` | PATH, `--line`, `--col` | Every use, across all of a project's entry points and tasks. |
| `completion` | PATH, `--line`, `--col` | Completion items at the position. |
| `rename` | PATH, `--line`, `--col`, `--name` | `{"edits": [...]}`: the edits a rename makes. Nothing is written. |

| Flag | Meaning |
|---|---|
| `--line N`, `--col N` | Position (1-based). |
| `--source TEXT` | Ask about this source text, checked on its own, instead of PATH. `-` reads it from standard input. |
| `--name NAME` | The new name for `rename`. |
| `--overlays FILE` | Proposed file contents to check instead of the files on disk: a JSON object from path (relative to the program's directory) to text. `-` reads standard input. Nothing is written. |

Exit status: 0 when the question was answered, also when the answer lists
errors; 1 when it could not be (no PATH or `--source`, unknown KIND, a
missing file).

```sh
$ badger query hover src/main.bdg --line 22 --col 53
{"text":"half: fn(Int) -> Int\nrequires n % 2 == 0\nensures result * 2 == n"}
```

### `badger search QUERY [PATH] [--json]`

Find a definition: the query is looked for in the names and in the
documentation of the standard library and of the project's modules. PATH
is the project to search as well (default `.`).

| Flag | Meaning |
|---|---|
| `--json` | Print the matches as a JSON array. |

Each match names the module, the item, and its documentation. The
standard library is searched in its sources, the project in its checked
interface (the same one `badger query api` prints).

```sh
$ badger search percent-decoded
std.route.query_params  pub fn query_params(uri: String) -> Params {
    The query parameters of a request URI (`?a=1&b=2`): the names and values are percent-decoded.
```

### `badger graph KIND [PATH] [--format FORMAT] [--std]`

Program structure as a graph. PATH defaults to `.`.

| KIND | Nodes and edges |
|---|---|
| `calls` | Functions; an edge from caller to callee. |
| `modules` | Modules; an edge for each import. |
| `types` | Types; an edge to each type used by a struct field or enum variant. |
| `systems` | System plan nodes (`systemN:key`); an edge to each node referenced. |

| Flag | Default | Meaning |
|---|---|---|
| `--format FORMAT` | `dot` | `dot`, `mermaid`, or `json` (`{"nodes": [...], "edges": [{"from","to"}]}`). |
| `--std` | off | Include standard-library nodes (`calls`, `modules`, `types`). |

```sh
$ badger graph calls
digraph calls {
  rankdir=LR;
  node [shape=box, fontname="monospace"];
  "main.half";
  "main.main";
  "main.test_half";
  "main.main" -> "main.half";
  "main.test_half" -> "main.half";
}
```

### `badger doc TARGET [--out DIR] [--check]`

Reference documentation in Markdown from checked interfaces. TARGET is
`std` (every standard module, one page each plus an index), one standard
module (`std.list`), or a program path.

| Flag | Meaning |
|---|---|
| `--out DIR` | Write the pages into DIR instead of printing them. |
| `--check` | With `--out`: compare with the pages in DIR and fail if any differ or a page has no module. |

```sh
badger doc std --out docs/api --check
```

### `badger explain KIND [TARGET] [PATH] [--json]`

| Form | Prints |
|---|---|
| `badger explain diagnostic CODE`, or `badger explain CODE` | The catalog entry for a diagnostic code (`E0301`). |
| `badger explain type NAME [PATH]` | The type of definition NAME in PATH's entry module (PATH defaults to `.`). |
| `badger explain effect NAME [PATH]` | Its effects. |

`--json` prints `{"code","title","explanation"}` or `{"name","type"}` /
`{"name","effect"}`. Exit status 1 for an unknown code or name.

```sh
$ badger explain type half
half: fn(Int) -> Int
$ badger explain effect main
main: !{Io}
```

### `badger contracts [PATH] [--json] [--tests]`

List each `requires` / `ensures` clause and what the compiler decided:
`proven` (its runtime check is compiled out), `refuted` (an error), or
`runtime` (checked when the program runs), with the reason.

| Flag | Meaning |
|---|---|
| `--json` | `{"ok", "clauses": [{"file","line","function","kind","text","status","reason","calls","tested"}], "diagnostics"}`. |
| `--tests` | Also run the tests while observing the program: each clause's test coverage (`tested.evaluated`, `tested.held`), and `suggestions`, candidate clauses that held in every observed call. |

Exit status: 1 when the program does not check, or with `--tests` when a
test failed.

```sh
$ badger contracts
./src/main.bdg:16: half requires `n % 2 == 0`: runtime (not proven at 2 of 2 call(s))
./src/main.bdg:17: half ensures `result * 2 == n`: runtime (not proven at every return)
2 clause(s): 0 proven, 2 checked at run time, 0 refuted
```

### `badger generate sql FILES... [--database URL] [--schema FILE] [--check]`

Turn SQL files of named queries (`-- name: find_user :one`) into typed
functions: `NAME.bdg` next to each `NAME.sql`. Guide: [typed-sql.md](../typed-sql.md).

| Flag | Default | Meaning |
|---|---|---|
| `--database URL` | `DATABASE_URL` | A Postgres URL (`postgres://`, `postgresql://`, or containing `host=`). The database describes each statement. |
| `--schema FILE` | the project's `[database] schema` | SQLite schema file; statements are prepared against it. Used when no database URL is given. |
| `--check` | | Do not write; fail if a generated file is missing or out of date. |

```sh
badger generate sql src/queries.sql --schema schema.sql
```

### `badger new NAME`

Creates the directory `NAME` with `badger.toml` (package `NAME`, entry
`src/main.bdg`) and `src/main.bdg`: a program with a `main` and one test.
`NAME` is a lowercase identifier; an existing directory is an error.

### `badger init [PATH] [--lib]`

Turns a directory into a project: `badger.toml` (the package's name is
the directory's name) and `src/main.bdg` (or `src/lib.bdg` with
`--lib`). The directory is created when it does not exist. An existing
file is never overwritten: the command fails and says so.

| Flag | Meaning |
|---|---|
| `--lib` | A library package: `src/lib.bdg`, `kind = "library"`, no `main`. |

Exit status: 0 when the project was written, 1 when a file was there
already.

```sh
$ badger init ~/shop
created /home/you/shop/badger.toml and /home/you/shop/src/main.bdg
```

### `badger generate feature NAME [DIR] [--item NAME] [--dry-run]`

Adds a typed feature module to the project in `DIR` (default `.`), under
the entry's directory: `NAME/model.bdg` (a record type, its errors, the
`Store` it needs, and its `Operations`), `NAME/memory.bdg` (an in-memory
store provider), `NAME/service.bdg` (the feature's provider with `create`,
`find`, and `list` operations, and tests), and `NAME/http.bdg` (routes
binding the operations, and a test).

| Flag | Default | Effect |
|---|---|---|
| `--item NAME` | `NAME` without a final `s` | The record's name (`--item stock_item` gives `StockItem`). |
| `--dry-run` | | Print the plan and check it, but write nothing. |

Before anything is written, the generated files are checked and their
tests run in a copy of the project. The command refuses a name that
exists as a module or directory, and never replaces a file. Nothing else
changes: it prints the plan nodes to add where the application is
composed. Generated code is ordinary code to edit; its storage and rules
are placeholders for the feature's own.

```sh
badger generate feature orders
```

### `badger generate worker NAME [DIR] [--dry-run]`

Adds `NAME.bdg` under the entry's directory: a durable job kind
(`std.jobs`) with `Job` (its state), `step` (one attempt's work, a
placeholder), `processing`, a `processor` provider running a supervised
worker, `submit`, and a test that a submitted job is processed. It is
checked, and its test run, in a copy of the project first, like
`generate feature`; an existing module is never replaced.

### `badger update [PATH]`

Resolve dependencies again (git `rev`s included) and rewrite the
`badger.lock` of the project at PATH (default `.`), or of its workspace.
Prints `locked NAME VERSION (HASH)` per dependency. See
[config.md](config.md#dependencies).

```sh
badger update
```

### `badger doctor [--json]`

The environment, for a bug report: the version, the current directory,
the project (its name, its manifest, its number of sources), the
effective runtime settings (the worker count and the HTTP limits, as
`badger inspect runtime` computes them), and the `BADGER_*` environment
variables that are set. Only what can be determined is reported: a fact
that is unknown is left out.

| Flag | Meaning |
|---|---|
| `--json` | Print the report as a JSON object. |

```sh
$ badger doctor
badger: 0.1.0
cwd: /home/you/shop

project:
  root: /home/you/shop
  name: shop
  config: /home/you/shop/badger.toml
  source files: 3

runtime:
  workers: 2
  http_max_body_bytes: 16777216
  http_max_header_bytes: 65536
  http_max_in_flight: 1024
  http_max_stream_buffer_bytes: 1048576

environment:
  BADGER_EXE: /usr/local/bin/badger
```

### `badger completions SHELL`

Prints a shell completion script for SHELL: the names of the commands
and the main flags of each of them, so they are not typed by hand. The
script says how to install it.

| SHELL | Install with |
|---|---|
| `bash` | `source <(badger completions bash)` |
| `zsh` | `badger completions zsh > "${fpath[1]}/_badger"` |
| `fish` | `badger completions fish > ~/.config/fish/completions/badger.fish` |
| `powershell` | `badger completions powershell \| Out-String \| Invoke-Expression` |

Exit status: 2 for an unknown shell (the argument parser's usage
error).

```sh
$ badger completions bash | head -3
# badger completion for bash.
#
# Install:  source <(badger completions bash)
```

## Servers

### `badger lsp`

Language server on standard input and output: diagnostics, hover,
definition, references, rename, symbols, formatting, completion, and
quick fixes (a missing standard import, a missing `match` arm). Editor
setup: [editors.md](../editors.md).

### `badger mcp`

Model Context Protocol server on standard input and output. Reference:
[mcp.md](mcp.md); setup and workflows: [agents.md](../agents.md).

## Other

| Command | Meaning |
|---|---|
| `badger help [COMMAND]` | Print help for `badger` or a command. |
| `badger --version`, `-V` | Print `badger 0.1.0`. |
