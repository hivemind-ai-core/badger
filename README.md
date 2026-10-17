# Badger

Badger is a statically typed language for services and automation. It is
built around inferred native records, lifecycle-managed systems, typed
interceptor pipelines, lightweight tasks, and paged collections and message
batches. Programs are checked, then compiled with Cranelift and run
in-process on the Badger runtime.

## Start here

- [Learn Badger in 30 minutes](docs/tour.md): one commented walk through
  the language.
- [The guide](docs/guide/README.md): one chapter per topic.
- [All documentation](docs/README.md): references for the
  [CLI](docs/reference/cli.md), [REPL](docs/reference/repl.md),
  [MCP server](docs/reference/mcp.md), and
  [`badger.toml`](docs/reference/config.md); the
  [standard library](docs/api/README.md); topic notes; and the
  [internals guide](docs/internals.md).

## Build and run

```text
cargo build --release
target/release/badger run examples/01_basics.bdg
target/release/badger run examples/design/service     # a project with badger.toml
target/release/badger check examples/design/service
target/release/badger repl
```

Every command and flag is in the [CLI reference](docs/reference/cli.md).

## Design

- [Draft-0 language specification](SPEC.md): syntax and semantics.
- [Language examples](examples/README.md): everyday syntax, records, errors,
  collections, tasks and channels, typed systems, interceptors, and worked
  applications.
- [Semantic acceptance cases](spec/08-worked-examples.md) and their test status.
- [Compiler architecture and implementation status](ARCHITECTURE.md).
- [Benchmarks](bench/README.md) for the specification's measurement gates.
- [Earlier design discussion](DESIGN.md) and [library research](research/README.md).

## Status

The draft-0 language and runtime are implemented end to end. The twelve
numbered examples, the design examples (including the HTTP/SQLite service),
every static acceptance case, and the runtime scenarios run as automated
tests (`cargo test --workspace`); the runtime tests run on one worker and
on four.

A program runs from source (`badger run`), from saved binary MIR
(`badger compile`), or from a self-contained release directory
(`badger release`, started with `badger start`). Dependencies are local
paths or git repositories, pinned by `badger.lock`; there is no registry.
There is no ahead-of-time object output; the specification (§07) treats
that as a separate future output mode.

Design choices not yet confirmed by the user are proposed defaults; the
specification marks the accepted decisions.
