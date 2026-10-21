# MCP server

`badger mcp` is a [Model Context Protocol](https://modelcontextprotocol.io)
server on standard input and output. It gives an agent six tools: a
persistent REPL session (`eval`), checking (`check`), compiler queries
(`query`), contract analysis (`contracts`), checked code generation
(`generate`), and program runs (`run`).

This page is the reference. Setup examples, `std.compiler` in code mode,
and how to work with contracts and proposed edits are in
[agents.md](../agents.md).

## Registering

The server is the command `badger mcp` with no arguments or environment.
For Claude Code:

```sh
claude mcp add badger -- badger mcp
```

For a client configured in JSON:

```json
{ "mcpServers": { "badger": { "command": "badger", "args": ["mcp"] } } }
```

## Protocol

- Transport: newline-delimited JSON-RPC 2.0 on stdin/stdout.
- Methods: `initialize`, `ping`, `tools/list`, `tools/call`.
  Notifications (messages without an `id`) are ignored.
- `initialize` answers with the client's `protocolVersion` (default
  `2025-06-18`), `capabilities: {"tools": {"listChanged": false}}`,
  `serverInfo: {"name": "badger", "version": "0.1.0"}`, and
  `instructions` (a short guide to the tools).
- An unknown method is error `-32601` (`method not found: NAME`). A line
  that is not JSON is error `-32700` (`parse error`) with `id: null`.
- A tool result is text content; when the text is JSON (or a summary
  line and then JSON), the result also carries it as
  `structuredContent`: the object, or `{"result": [...]}` for a list.
  Diagnostics in it have a `schema_version`.
- The server exits (status 0) when standard input closes.

Every tool result has the MCP text form:

```json
{"content": [{"type": "text", "text": "..."}], "isError": false}
```

`isError` is `true` when the tool could not do what was asked or what it
ran failed; the text says why. An unknown tool name gives
``unknown tool `NAME` ``.

In the examples below, `→` is the `params` of a `tools/call` request and
`←` the `result` of its response, as the server printed it.

`check`, `query`, `contracts`, and `run` run `badger` subcommands in
subprocesses, separate from the `eval` session. Paths are relative to the
server's working directory.

## `eval`

Evaluate code in the persistent REPL session (the machinery of
`badger repl`; input rules in [repl.md](repl.md#input)). Items (`fn`,
`struct`, `enum`, `import`, module-level `let`) join the session and
replace earlier ones of the same name; an expression is evaluated and its
value displayed. `import std.compiler` gives the compiler services as
functions ([agents.md](../agents.md#code-mode)).

| Parameter | Type | Default | Meaning |
|---|---|---|---|
| `code` | string, required | | Items or an expression. |
| `timeout_secs` | number | 30 | Cancel the evaluation after this long (clamped to 0.1 to 3600). The session stays usable. |

Result text: what the code printed, then the value. `(no value)` when
there is neither. `isError` is `true` when the input does not check, a
redefinition is rejected, the evaluation fails (a defect), or it timed
out.

- Standard output of the session is captured and returned with each
  evaluation. When more than 1 MiB accumulates, the oldest part is dropped
  (the last 512 KiB is kept) and `[earlier output dropped]` marks the cut.
- `process.exit` is a defect here (the session outlives the code).
- An unused `Result` in a multi-line input is error E0337; write
  `let _ = io.println(...)`.

```text
→ {"name":"eval","arguments":{"code":"fn sq(x: Int) -> Int { x * x }"}}
← {"content":[{"text":"(no value)","type":"text"}],"isError":false}
→ {"name":"eval","arguments":{"code":"loop { task.sleep(1s) }","timeout_secs":1}}
← {"content":[{"text":"error: evaluation timed out after 1s and was cancelled","type":"text"}],"isError":true}
```

## `check`

Type-check a file, a project directory, or source text. A project is
checked as `badger check` checks it: the main entry, `[entries]`, and
`[tasks]`.

| Parameter | Type | Meaning |
|---|---|---|
| `path` | string | File or project directory. |
| `source` | string | Source text, checked on its own (used when `path` is absent). |
| `files` | object: path to text | Proposed contents, relative to the program's directory, checked in place of the files on disk. Nothing is written. Needs `path`. |

One of `path` or `source` is required; without either the result is
``give `path` or `source` ``.

Result text:

- without `files`: `no diagnostics: it checks`, or `N diagnostic(s):`
  followed by the JSON array of `badger query diagnostics` (`file`,
  `line`, `col`, `end_line`, `end_col`, `severity`, `code`, `message`,
  `notes`, `label`, `expected`, `actual`, `scope`, `related`,
  `declarations`). `isError` is `true` only when a diagnostic is an error;
  warnings alone leave it `false`.
- with `files`: a summary line, then the JSON of `badger query check
  --overlays`: `ok`, `diagnostics`, `inputs`, `unused_overlays`, `roots`,
  `tests`, `tasks` ([agents.md](../agents.md#checking-proposed-edits)).
  `isError` is `true` when `ok` is false.

Time limit: 120 seconds.

```text
→ {"name":"check","arguments":{"source":"pub fn f() -> Int { \"x\" }"}}
← {"content":[{"text":"1 diagnostic(s):\n[{\"actual\":\"String\",\"code\":\"E0300\",\"col\":19,\"declarations\":[],\"end_col\":26,\"end_line\":1,\"expected\":\"Int\",\"file\":\"/tmp/badger-mcp-f8ffe71b996fa6bb/check.bdg\",\"label\":\"expected `Int`, found `String`\",\"line\":1,\"message\":\"type mismatch: expected `Int`, found `String`\",\"notes\":[\"expected Int, found String\"],\"related\":[],\"scope\":\"fn check.f\",\"severity\":\"error\"}]\n","type":"text"}],"isError":true}
```

## `query`

Ask about code. Positions are 1-based.

| Parameter | Type | Meaning |
|---|---|---|
| `kind` | string, required | One of the kinds below. |
| `path` | string | Source file (or program, for `api` and `graph`). |
| `line`, `col` | integer | Position, for the kinds that need one. |
| `name` | string | Kind-specific: the new name, a diagnostic code, a module, or a graph kind. |

| `kind` | Needs | Result text |
|---|---|---|
| `hover` | `path`, `line`, `col` | `{"text": ...}`: type or signature. |
| `definition` | `path`, `line`, `col` | The definition's location. |
| `references` | `path`, `line`, `col` | Every use, across all of a project's entry points and tasks. |
| `completion` | `path`, `line`, `col` | Completion items. |
| `rename` | `path`, `line`, `col`, `name` | `{"edits": [{file, line, col, end_line, end_col, text}]}`, not applied. |
| `symbols` | `path` | The file's definitions. |
| `symbol` | `path`, `name` | A definition by qualified name (`orders.service.create`, `Order.total`): kind, signature, location, and references. |
| `guide` | optional `name` | The guide built into this toolchain: a chapter by number or a word of its name (`3`, `errors`); without `name`, the orientation page (llms.txt) and the chapter list. |
| `examples` | `name` | The guide's code examples containing `name` (up to 20, with the total count): chapter, heading, whether the block is a complete program, and the code. The documentation tests check every one. |
| `diagnostics` | `path` | The diagnostics array. |
| `explain` | `name` (a code such as `E0301`) | The catalog explanation. |
| `module` | `name` (`list`, `std.list`), or nothing | The standard module's source (its API and documentation); without `name`, the list of modules. |
| `graph` | `path`; `name`: `calls` (default), `modules`, `types`, `systems` | `{"nodes": [...], "edges": [{"from","to"}]}`. |
| `api` | `name` (`std.list`) or `path` | Public interface: functions, types, methods with signatures, effects, contracts, and docs. |

An input a kind cannot answer (a missing file, a directory where a source
file is needed, a file that does not parse for `symbols`) is an error
(`isError: true`), not an empty result; so is a missing `path` where one
is needed, and so is any other `kind` (diagnostics of proposed contents
come from the `check` tool). Time limit: 120 seconds.

```text
→ {"name":"query","arguments":{"kind":"hover","path":"demo/src/main.bdg","line":22,"col":53}}
← {"content":[{"text":"{\"text\":\"half: fn(Int) -> Int\\nrequires n % 2 == 0\\nensures result * 2 == n\"}","type":"text"}],"isError":false}
```

## `contracts`

Each `requires` / `ensures` clause and what the compiler decided:
`proven` (its runtime check is compiled out), `refuted` (an error), or
`runtime` (checked when the program runs), with the reason.

| Parameter | Type | Default | Meaning |
|---|---|---|---|
| `path` | string, required | | File or project directory. |
| `tests` | boolean | `false` | Also run the `test_*` functions, observing the program: per-clause coverage (`tested.evaluated`, `tested.held`) and `suggestions`, candidate clauses that held in every observed call. |

Result text: a summary line (`N clause(s): P proven, R checked at run
time, F refuted`; with `tests`, also tests ran and failed, unproven
clauses no test reaches, and the number of suggestions), then the JSON of
`badger contracts --json [--tests]`. `isError` is `true` when the
program does not check or, with `tests`, a test failed. Time limit: 300
seconds.

```text
→ {"name":"contracts","arguments":{"path":"demo"}}
← {"content":[{"text":"2 clause(s): 0 proven, 2 checked at run time, 0 refuted\n{\"clauses\":[{\"calls\":2,\"file\":\"demo/src/main.bdg\",\"function\":\"half\",\"kind\":\"requires\",\"line\":16,\"reason\":\"not proven at 2 of 2 call(s)\",\"status\":\"runtime\",\"tested\":null,\"text\":\"n % 2 == 0\"},{\"calls\":2,\"file\":\"demo/src/main.bdg\",\"function\":\"half\",\"kind\":\"ensures\",\"line\":17,\"reason\":\"not proven at every return\",\"status\":\"runtime\",\"tested\":null,\"text\":\"result * 2 == n\"}],\"diagnostics\":[],\"ok\":true}\n","type":"text"}],"isError":false}
```

## `generate`

Arguments: `what` (`feature` or `worker`), `name` (a word: letters,
digits, `_`), `path` (the project directory), `item` (optional, `feature`
only), `dry_run` (optional, a boolean). It is `badger generate feature`
([cli.md](cli.md#badger-generate-feature-name-dir---item-name---dry-run))
or `badger generate worker`: the plan (files and line counts), then the
test report of the generated files checked in a copy of the project;
nothing is written on a dry run or a failed check, and an existing file
is an error.

```text
→ { "what": "feature", "name": "orders", "path": "/work/shop", "dry_run": true }
← create src/orders/model.bdg (24 lines)
  ...
  checked in a copy of the project: test result: ok. 3 passed; 0 failed
  dry run: nothing written
```

## `run`

Run a program (`badger run`) or its tests (`badger test`) in a fresh
process with no standard input.

| Parameter | Type | Default | Meaning |
|---|---|---|---|
| `path` | string, required | | File or project directory. |
| `args` | array of strings | | Program arguments (after `--`). |
| `test` | boolean | `false` | Run the `test_*` functions instead (`badger test PATH`). |
| `release` | boolean | `false` | Optimized code (`--release`). |
| `timeout_secs` | number | 60 | Kill the process (and its process group) after this long (clamped to 0.1 to 3600). |
| `breakpoints` | array of strings | | Run under gdb with debug information, stopping at `file:line` or `file:line if CONDITION` (gdb syntax). |
| `commands` | array of strings | `bt 5`, `info args`, `info locals` | gdb commands run at each stop (with `breakpoints`). |
| `max_stops` | integer | 10 | Stops to report (with `breakpoints`); at the last one the run ends. |
| `profile` | boolean | `false` | Also profile CPU time (`BADGER_PROFILE`); the summary of functions by self and total time is in the stderr part. |

Options a call would ignore are refused, not dropped: with `test: true`,
`args`, `breakpoints`, and `release: true` (see the second example below);
`commands` and `max_stops` without `breakpoints`; `profile` with
`breakpoints`.

Result text: `exit status N`, then `--- stdout` and `--- stderr` sections
when they are not empty. `isError` is `true` when the status is not 0. A
run past its time limit gives `timed out after Ns`.

With `breakpoints`, the result is gdb's report (thread chatter removed):
at each stop the frames, arguments, and locals, or the output of
`commands`. `isError` is `true` when the program exited with a non-zero
status or was killed by a signal. It needs `gdb` on the `PATH` (see
[debugging.md](../debugging.md)).

```text
→ {"name":"run","arguments":{"path":"demo"}}
← {"content":[{"text":"exit status 0\n--- stdout\nport=8080 db=demo.db half=4040\n","type":"text"}],"isError":false}
→ {"name":"run","arguments":{"path":"demo","test":true,"args":["x"]}}
← {"content":[{"text":"`args` cannot be used with `test`: tests take no program arguments","type":"text"}],"isError":true}
```

The examples use the project shown in [cli.md](cli.md#example-project).
