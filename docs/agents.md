# Agent setup (MCP)

`lang mcp` serves the Model Context Protocol on stdin/stdout. Register it
with an MCP client, for example:

```sh
claude mcp add lang -- lang mcp
```

or in a client's JSON configuration:

```json
{ "mcpServers": { "lang": { "command": "lang", "args": ["mcp"] } } }
```

## Tools

| Tool | Arguments | Result |
|---|---|---|
| `eval` | `code`, optional `timeout_secs` (default 30) | Output printed by the code, then the value of an expression. Items (`fn`, `struct`, `enum`, `import`, module-level `let`) join the session and replace earlier ones of the same name. |
| `check` | `path` (file or project directory) or `source`; optional `files` | Diagnostics as JSON; an empty list means it checks. A project is checked as `lang check` checks it: the main entry, each `[entries]` module, and each `[tasks]` module. With `files` (path, relative to the program's directory, to proposed text), the project is checked with those contents in place of the files on disk, and nothing is written; see [Checking proposed edits](#checking-proposed-edits). |
| `query` | `kind`, `path`, `line`, `col` (1-based), `name` | `hover`, `definition`, `references`, `completion`, `symbols`, `diagnostics`; `explain` for a diagnostic code; `module` for a standard module's source (its API), or the module list; `graph` with `name` `calls`, `modules`, `types`, or `systems` (JSON nodes and edges); `api` for the public interface (signatures with effects and contracts, and docs) of `name` (a standard module such as `std.list`) or of the program at `path`. |
| `run` | `path`, optional `args`, `test`, `release`, `timeout_secs` | Exit status, stdout, and stderr of `lang run` or `lang test`. |

## Code mode

`eval` runs real code, so one call can combine compiler queries with
ordinary computation. `import std.compiler` once per session:

```text
import std.compiler

// every error code in a project, with its location
compiler.diagnostics("src").map(fn(ds) {
    ds.map(fn(d) { $"{d.file}:{d.line} {d.code.unwrap_or("-")} {d.message}" })
})

// the signatures of a file's functions
compiler.symbols("src/main.lang").map(fn(ss) {
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
| `explain(code)` | `Result<String, Error>` |

A diagnostic has its location (`file`, `line`, `col`, `end_line`,
`end_col`), `severity`, `code`, `message`, and `notes`, and the same
structured facts as `lang check --json`: the primary `label`, `expected`
and `actual` shapes, the enclosing `scope`, secondary `related` locations,
and `declarations` (related definitions, by name and location).

The same answers are available without MCP from the command line:
`lang query hover src/main.lang --line 12 --col 8`.

## Checking proposed edits

To check a change that spans several files (an interface and the modules
that use it) before writing any of them, give the proposed contents with
the project:

```text
compiler.check_project("app", map.from_list([
    ("src/shapes.lang", new_shapes),
    ("src/report.lang", new_report),
]))
```

or MCP `check` with `path` and `files`, or `lang query check app
--overlays edits.json` (`--overlays -` reads the JSON object from standard
input). Files not named are read from disk; a proposed file may be new.
The result agrees with checking the same edits saved. Besides
`diagnostics`, it reports:

- `ok`: no errors (warnings may remain);
- `inputs`: every file checked (`lang.toml` included), with `proposed`
  (whether the text came from `files`), `bytes`, and a `hash`;
- `unused_overlays`: proposed files that no checked module imports (a
  misspelled path, or a module nothing imports yet);
- `roots`: each entry point checked (`main`, an `[entries]` name, or
  `task:NAME`) with its `module` and `function`;
- `tests`, and `tasks`: each `spawn` or `capture` site with the function
  containing it.
