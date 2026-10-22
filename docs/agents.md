# Agent setup (MCP)

`badger mcp` serves the Model Context Protocol on stdin/stdout. Register it
with an MCP client, for example:

```sh
claude mcp add badger -- badger mcp
```

or in a client's JSON configuration:

```json
{ "mcpServers": { "badger": { "command": "badger", "args": ["mcp"] } } }
```

## Partial programs

Write `_` where an expression is not known yet: `badger check` (and the
`check` tool) reports each hole as E0345 with the type expected there,
and checks everything around it. Fill the holes one at a time.

## Tools

| Tool | Arguments | Result |
|---|---|---|
| `eval` | `code`, optional `timeout_secs` (default 30) | Output printed by the code, then the value of an expression. Items (`fn`, `struct`, `enum`, `import`, module-level `let`) join the session and replace earlier ones of the same name. |
| `check` | `path` (file or project directory) or `source`; optional `files` | Diagnostics as JSON; an empty list means it checks. A project is checked as `badger check` checks it: the main entry, each `[entries]` module, and each `[tasks]` module. With `files` (path, relative to the program's directory, to proposed text), the project is checked with those contents in place of the files on disk, and nothing is written; see [Checking proposed edits](#checking-proposed-edits). |
| `query` | `kind`, `path`, `line`, `col` (1-based), `name` | `hover`, `definition`, `references` (every use, in all of a project's entry points and tasks), `completion`, `symbols`, `diagnostics`; `rename` with the new name in `name` (the edits, not applied); `explain` for a diagnostic code; `module` for a standard module's source (its API), or the module list; `graph` with `name` `calls`, `modules`, `types`, or `systems` (JSON nodes and edges); `api` for the public interface (signatures with effects and contracts, and docs) of `name` (a standard module such as `std.list`) or of the program at `path`. |
| `contracts` | `path`, optional `tests` | Each `requires`/`ensures` clause and what the compiler decided: `proven` (its runtime check is compiled out), `refuted` (an error), or `runtime`, with why. With `tests: true` it runs the tests observing the program: each clause's coverage (`tested.evaluated` 0 is a gap) and candidate clauses that held in every observed call. See [Working with the compiler on contracts](#working-with-the-compiler-on-contracts). |
| `generate` | `what` (`feature` or `worker`), `name`, `path`, optional `item`, `dry_run` | `badger generate feature` or `worker`: the planned files, then the test report of the generated code checked in a copy of the project. Nothing is written on a dry run or a failed check, and existing files are never replaced. |
| `run` | `path`, optional `args`, `test`, `release`, `timeout_secs`, `breakpoints`, `commands`, `max_stops`, `profile` | Exit status, stdout, and stderr of `badger run` or `badger test`. With `breakpoints` (`file:line`, optionally `if CONDITION`) it runs under gdb and reports the frames, arguments, and locals at each stop (see [debugging](debugging.md)); with `profile: true` it adds a CPU profile (functions by self and total time). |

A query that cannot answer its input fails with a message (a missing
file, a directory where a source file is needed, a file that does not
parse for `symbols`) instead of returning an empty result.

## Code mode

`eval` runs real code, so one call can combine compiler queries with
ordinary computation. `import stdx.compiler` once per session:

```text
import stdx.compiler

// every error code in a project, with its location
compiler.diagnostics("src").map(fn(ds) {
    ds.map(fn(d) { $"{d.file}:{d.line} {d.code.unwrap_or("-")} {d.message}" })
})

// the signatures of a file's functions
compiler.symbols("src/main.bdg").map(fn(ss) {
    ss.filter(fn(s) { s.kind == "function" }).map(fn(s) { $"{s.name}: {s.detail}" })
})
```

| Function | Returns |
|---|---|
| `diagnostics(path)` | `Result<List<Diagnostic>, Error>` |
| `check_source(source)` | `Result<List<Diagnostic>, Error>` |
| `check_project(path, files)` | `Result<Check, Error>` (`files`: `Map<String, String>`) |
| `hover(path, line, col)` | `Result<Option<String>, Error>` |
| `definition(path, line, col)` | `Result<Option<Location>, Error>` |
| `references(path, line, col)` | `Result<List<Location>, Error>` |
| `symbols(path)` | `Result<List<Symbol>, Error>` |
| `completions(path, line, col)` | `Result<List<Completion>, Error>` |
| `api(target)` | `Result<List<ApiItem>, Error>` (`std.list`, or a program path) |
| `graph(kind, path)` | `Result<Graph, Error>` (`calls`, `modules`, `types`, `systems`) |
| `rename(path, line, col, name)` | `Result<List<Edit>, Error>` (not applied) |
| `explain(code)` | `Result<String, Error>` |

A diagnostic has its location (`file`, `line`, `col`, `end_line`,
`end_col`), `severity`, `code`, `message`, and `notes`, and the same
structured facts as `badger check --json`: the primary `label`, `expected`
and `actual` shapes, the enclosing `scope`, secondary `related` locations,
and `declarations` (related definitions, by name and location).

The same answers are available without MCP from the command line:
`badger query hover src/main.bdg --line 12 --col 8`.

## Working with the compiler on contracts

Contracts are where the compiler, the tests, and you meet. The compiler
proves what it can (and rejects calls and returns that cannot satisfy a
clause); what it cannot prove it checks at run time. Ask `contracts` with
`tests: true` and act on the answer:

- a `runtime` clause with `tested.evaluated` 0 is unverified twice over:
  write a test that reaches it (or a call the compiler can follow);
- a clause whose `tested.held` is below `tested.evaluated` is broken by a
  test: fix the code or the clause;
- a suggestion (`take could state ensures result >= 0`) is what held in
  every observed call: state it if it is meant (the compiler then checks
  it, and may prove it), or add a test that shows it is not.

Then ask again. Statements the compiler proves cost nothing at run time.

## Checking proposed edits

To check a change that spans several files (an interface and the modules
that use it) before writing any of them, give the proposed contents with
the project:

```text
compiler.check_project("app", map.from_list([
    ("src/shapes.bdg", new_shapes),
    ("src/report.bdg", new_report),
]))
```

or MCP `check` with `path` and `files`, or `badger query check app
--overlays edits.json` (`--overlays -` reads the JSON object from standard
input). Files not named are read from disk; a proposed file may be new.
The result agrees with checking the same edits saved. Besides
`diagnostics`, it reports:

- `ok`: no errors (warnings may remain);
- `inputs`: every file checked (`badger.toml` included), with `proposed`
  (whether the text came from `files`), `bytes`, and a `hash`;
- `unused_overlays`: proposed files that no checked module imports (a
  misspelled path, or a module nothing imports yet);
- `roots`: each entry point checked (`main`, an `[entries]` name, or
  `task:NAME`) with its `module` and `function`;
- `tests`, and `tasks`: each `spawn` or `capture` site with the function
  containing it.
