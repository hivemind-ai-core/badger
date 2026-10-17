# REPL

`badger repl` evaluates Badger interactively. Definitions accumulate;
expressions are evaluated and their values printed.

| Form | Evaluates in |
|---|---|
| `badger repl` | A private session program started for the REPL. |
| `badger repl --connect ADDR` | A running program started with `badger run --inspect ADDR`. |

Both use the same machinery: each input is compiled into the program as a
new code version, under the same compatibility rules as `badger run
--watch`.

## Input

Each line is one input. A line that leaves a `(`, `[`, or `{` open (outside
a string) continues on the next line (prompt `... `) until the brackets
close. Prompts (`badger> `, `remote> `) are printed only when standard
input is a terminal, so input can be piped.

An input is one of:

| Input | Starts with | Effect |
|---|---|---|
| Import | `import ` | Added to the session (once). |
| Definition | `fn`, `pub`, `type`, `struct`, `enum`, `protocol`, `impl`, `extern`, `const`, `requires`, `ensures`, `///` | Added to the session. It replaces an earlier definition of the same name (an `impl` is keyed by its header, `impl Display for Point`). |
| Module-level value | `let`, on one line | Added to the session; computed once, keeps its identity. |
| Expression | anything else | Run as a task; its value is printed. A `Unit` value prints nothing. Several lines run as one block; the value is the last expression's. |

A `let` that spans several lines is run as a block, not kept: write it on
one line to keep the binding.

## Special commands

A line starting with `:` is a command:

| Command | Prints |
|---|---|
| `:tasks` | Table of tasks: id, state, what it is waiting on, scope, code version it started on, stack, pipeline stage, name. |
| `:channels` | Table of channels: id, kind, capacity, queued, staged, closed, waiting senders/receivers, element type. |
| `:system` | Running systems and their nodes (provider, generation). |
| `:heap` | JSON: live objects and bytes, collections, total and maximum pause. |
| `:code` | JSON: the live code version (`{"version":N}`). |
| `:all` | JSON: all of the above. |
| `:quit`, `:q` | Ends the session (so does end of input). |

Any other `:name` prints ``{"error":"unknown snapshot `name`"}``. These are
the snapshots of `badger inspect WHAT --connect ADDR`.

## What accumulates, and what is rejected

- Each input recompiles the session program with every accepted import
  and definition. An input that does not check prints its diagnostics and
  changes nothing.
- A redefinition must keep the type and effects of the definition it
  replaces. Otherwise it is rejected:
  `` rejected: `square` changed from `fn(Int) -> Int` to `fn(Int) -> String`; the running version continues ``.
- An existing type definition cannot change:
  ``rejected: type `session.Point` changed; the running version continues``.
  New types can be added.
- Code already running (including closures already created) keeps the
  version it started with; new calls use the new code.
- Module-level values are computed once. In the local REPL they may hold
  mutable state (a `Cell`, for example); in a program file they must be
  pure immutable data (E0138).

## Local session: `badger repl`

The session program imports `std.io` and `std.task` and runs a `main` that
sleeps. Other modules need an `import`.

```sh
$ printf 'fn square(x: Int) -> Int { x * x }\nsquare(7)\nlet greeting = "hi"\ngreeting\nio.println("printed")\n:code\n' | badger repl
49
hi
printed
ok(())
{"version":6}
```

`io.println` returns a `Result`, so as the last expression its value
`ok(())` is printed after the output. As an earlier statement in a
multi-line input, an unused `Result` is error E0337: write
`let _ = io.println(...)`.

Exit status: 0 at `:quit` or end of input; 1 if the session program cannot
start; N if the code calls `process.exit(N)` (that ends the session).

## Remote session: `badger repl --connect ADDR`

Start the program with an inspection endpoint, then connect:

```badger
import std.task

pub fn label(n: Int) -> String { $"tick {n}" }

pub fn main() -> Unit {
    loop {
        task.sleep(1s)
    }
}
```

```sh
$ badger run --inspect 127.0.0.1:47123 server.bdg &
$ printf 'label(3)\nfn label(n: Int) -> String { $"tock {n}" }\nlabel(3)\n:code\n' | badger repl --connect 127.0.0.1:47123
tick 3
tock 3
{"version":4}
```

- Inputs are added to the program's entry module. A definition replaces
  the program's own item of the same name, under the rules above.
- Expressions run as tasks inside the program and see its module-level
  values.
- The program's standard output stays the program's: `io.println` in an
  expression prints where the program prints, not in the REPL.
- Access: every request carries the endpoint's token, a random secret made
  when the program starts. It is written to a file only the user can read
  (`$XDG_RUNTIME_DIR/badger-inspect/`, else `~/.cache/badger-inspect/`), so
  only the same user can connect; the file is removed when the program
  ends.

Exit status: 0 at `:quit` or end of input; 1 when the connection fails
(the REPL stops at the first failed request).

## Limits

| Limit | Value |
|---|---|
| Evaluation time | An expression still running after 60 seconds is cancelled; the reply is `error: evaluation timed out after 60s and was cancelled`, and the session stays usable. |
| Evaluations | One at a time per program. |
| Multi-line `let` | Not kept (see Input). |
| Changed types and signatures | Rejected (see above); `badger run --watch --reconstruct` is the way to restart on incompatible changes. |
| Saved programs | `badger run --inspect` needs the sources: a `.bdgmir` file cannot be inspected (E0013). |
| Optimization | Under `--inspect`, the program is compiled with replaceable functions, without MIR inlining. |

For the same session from an agent, see `eval` in [mcp.md](mcp.md).
