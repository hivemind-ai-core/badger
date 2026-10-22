# Badger documentation

Start with the tour. Use the guide to learn one topic properly. Use the
references to look things up.

## Learn

- [Learn Badger in 30 minutes](tour.md): one commented walk through the
  whole language.
- [Language guide](guide/README.md): ten chapters, from values to services.

## Look things up

| Reference | What it covers |
|---|---|
| [Standard library](api/README.md) | Every standard module (`std.*` and `stdx.*`), generated from the checked interfaces by `badger doc std`. |
| [CLI](reference/cli.md) | Every `badger` command, flag, and exit code. |
| [REPL](reference/repl.md) | `badger repl`, local and remote (`--connect`). |
| [MCP server](reference/mcp.md) | `badger mcp` tools and parameters. |
| [Configuration](reference/config.md) | `badger.toml`, `BADGER_*` environment variables, `stdx.config`. |
| Diagnostics | `badger explain diagnostic CODE` prints the explanation for any `E....` code. |

## Topics

- [Migration cookbook](migration-cookbook.md): the same program in Go, Python, Rust and TypeScript, and what changes when you move it to Badger.
- [Common errors](common-errors.md): the diagnostics you meet first, grouped by what you were trying to do.
- [The type system](type-system.md): rows, effect rows, `From`, and how `?` resolves an error.
- [Idiomatic Badger](idiomatic.md): the conventions — one error type, validation, configuration, routes, pipeline stages, background workers, and tests.
- [Collections: choosing and costs](collections.md)
- [Typed SQL](typed-sql.md)
- [Releases](releases.md): packaging a program to run elsewhere.
- [Native modules](native-modules.md): calling Rust or C ABI libraries.
- [Debugging and profiling](debugging.md)
- [Editor setup](editors.md): the language server and the tree-sitter grammar.
- [Agent setup (MCP)](agents.md): using Badger from an AI coding agent.

## Contributing

- [Internals guide](internals.md): the short map of the compiler and runtime.
- [Architecture](../ARCHITECTURE.md): the detailed design and implementation status.
- [Sanitizer builds](sanitizers.md)
- [Specification](../SPEC.md): the language definition, chapter by chapter in [spec/](../spec/).

## Conventions in these docs

- Every ```` ```badger ```` block is a complete program, and the test suite
  type-checks all of them (`crates/badger_cli/tests/docs.rs`).
- Blocks marked `badger fragment` are excerpts and are not checked.
- Output shown after a program is what `badger run` printed.
