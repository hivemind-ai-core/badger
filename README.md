# Native application language for services and automation

A statically typed language centered on inferred native records,
lifecycle-managed systems, typed interceptor pipelines, lightweight tasks,
and efficient paged collections/message batches. Programs are checked, then
compiled with Cranelift and run in-process on a substantial runtime.

- [Draft-0 language specification](SPEC.md): the coherent syntax and semantics.
- [Language examples](examples/README.md): everyday syntax, records, errors,
  collections, tasks/channels, typed systems, interceptors, and worked applications.
- [Semantic acceptance cases](spec/08-worked-examples.md) and their test status.
- [Compiler architecture and implementation status](ARCHITECTURE.md).
- [Benchmarks](bench/README.md) for the specification's measurement gates.
- [Earlier design discussion](DESIGN.md) and [library research](researxh/README.md).

## Using it

```text
cargo build --release
target/release/lang run examples/01_basics.lang
target/release/lang run examples/design/service        # project with lang.toml
target/release/lang run [ENTRY]                        # current project; ENTRY: file, dir, or [entries] name
target/release/lang compile [FILE_OR_PROJECT] [-o OUT] # save binary MIR (OUT.langmir)
target/release/lang run OUT.langmir [-- ARGS]          # run it: code generation only, no parse/check
target/release/lang task NAME [-- ARGS]                # a [tasks] function of the current project
target/release/lang check [FILE_OR_PROJECT] [--json]
target/release/lang test [FILE_OR_PROJECT] [FILTER] [--json]
target/release/lang format PATH... [--check]
target/release/lang repl [--connect ADDR]
target/release/lang inspect plans|config|runtime PATH [--profile NAME] [--json]
target/release/lang inspect tasks|channels|system|heap|code|all --connect ADDR [--json]
target/release/lang update PROJECT                     # rewrite lang.lock
target/release/lang explain diagnostic E0301
target/release/lang explain type|effect NAME FILE      # functions, types, protocols
```

Paths default to the current directory's project. `lang compile` checks a
program and saves its lowered form (binary MIR, with the effective
configuration and runtime settings) so `lang run FILE.langmir` starts in
milliseconds: it only generates code. `LANG_<SETTING>` and `--workers` still
apply at run time; `--watch`, `--inspect`, `--profile`, and tests need the
sources. `lang run` accepts
`--release` (optimized code), `--workers N` (run tasks on N worker threads;
default `[runtime] workers` or 1), `--profile NAME` (merge a
`[profile.NAME]` table of `lang.toml` into the configuration read by
`config.load`, and `[profile.NAME.runtime]` into the runtime settings),
`--watch` (replace compatible functions in the running program when sources
change; a changed type, function type, or effect is incompatible, and with
`--reconstruct` cancels the program, halts its resources, and restarts it on
the new version), `--inspect ADDR` (serve inspection snapshots and a remote
REPL to `lang inspect --connect` and `lang repl --connect`), and program
arguments after `--`.

A project's `lang.toml` has `[package]` (`name`, `entry`, `version`,
`edition`), `[toolchain]`, `[dependencies]` (path dependencies), `[entries]`
(named entry modules), `[tasks]` (`name = "src/file.lang#function"`),
`[config]`/`[profile.NAME]` (application configuration), and `[runtime]`
settings: `workers`, `cleanup_grace_ms`, `task_stack_bytes`,
`blocking_threads`, `http_max_body_bytes`, `http_max_header_bytes`,
`http_max_in_flight`, `trace_capacity`, `trace_overflow`. Each runtime
setting can also be given as `LANG_<SETTING>` in the environment. Settings
are range-checked: `workers` is 1..64, `task_stack_bytes` at most 1 GiB, and
`blocking_threads` at most 4096; out-of-range values are E0013.
`lang inspect runtime` shows each value and where it came from. Unknown
sections and keys are errors. Entry and task functions take no parameters
or a `List<String>` of arguments (E0012).

## Status

The draft-0 language and runtime are implemented end to end: all twelve
numbered examples, the four design examples (including the HTTP/SQLite
service), every static acceptance case, and all sixteen runtime scenarios run
as automated tests (`cargo test --workspace`), the runtime tests both on one
worker and on four.

Deployment runs from source, the first of the specification's options
(§07: "source or a validated typed-code cache"); dependencies are path
dependencies verified by `lang.lock` (§07 asks for lockfile and
content/version verification, not a registry). An ahead-of-time backend is,
per §07, "a separate future output mode", so there is no object output.

New design choices are proposed defaults; accepted user decisions are
identified in the specification.
