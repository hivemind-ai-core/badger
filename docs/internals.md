# Internals guide

The short map of the Badger compiler and runtime for contributors. Read it
first; [ARCHITECTURE.md](../ARCHITECTURE.md) has the depth on each part,
and [SPEC.md](../SPEC.md) is the language contract.

## The pipeline

```text
source ─ badger_syntax ─▶ AST ─ badger_hir ─▶ HIR ─ badger_typeck ─▶ typed HIR
       ─ badger_mir ─▶ MIR ─ badger_codegen ─▶ machine code ─ calls ─▶ badger_runtime
```

`badger_driver` runs the phases (`Session::check`, `Session::build`,
`Session::run` in `crates/badger_driver/src/lib.rs`). Each phase reports
into `Session::diagnostics`; the driver stops before code generation when
any phase reported an error.

### 1. Syntax: text to AST (`badger_syntax`)

- `token.rs`: the lexer (logos) turns source text into a flat buffer of
  `(Tok, Span)`; interpolated strings' holes are lexed as ordinary tokens.
  `layout` then turns the newlines that end statements into separator
  tokens.
- `grammar.rs`: chumsky parsers over the token slice, producing an owned
  syntax tree (`cst.rs`). A first pass records no errors (fast); a file
  that fails is parsed again with rich errors and recovery, so every
  broken statement and declaration is reported. Expressions are parsed by
  precedence climbing.
- `lower.rs`: the syntax tree to the arena AST (`ast.rs`, nodes addressed by
  typed indices, each with a `Span`), with the case, duplicate-field, and
  literal checks and the `if let` / `while let` desugaring.
- `parser.rs`: the entry point (`parse`, `parse_recovering`), bracket
  balance errors, and diagnostics from parse errors.
- `format.rs`, `spacing.rs`: the canonical formatter (`badger format`).

### 2. HIR: names and desugaring (`badger_hir`)

- `lower.rs` collects modules, resolves imports (the prelude is imported
  implicitly) and every name (local, definition, constructor, type
  namespace), records lambda captures, and removes sugar: pipes, `use`
  continuations, dotted record updates, `pipeline { }`, and
  `system { }` (missing references and cycles are reported here).
- `badger_stdlib` holds the standard library sources, embedded (see below).

### 3. Type checking (`badger_typeck`)

| Part | Where |
|---|---|
| Types, rows, unification (levels, occurs check, skolem escape) | `ty.rs` |
| Declared types, protocols, impls, derived impls | `env.rs` |
| Inference: bidirectional Hindley-Milner with record and effect rows | `infer.rs` |
| Deferred constraints: protocols, methods, operators, row updates, iteration, `?` conversions; structural capabilities (`Transfer`, `Share`, `Codec`) | `solve.rs` |
| Binding groups, generalization, effect fixpoint over resolved calls, capability requirements of generic bodies, per-instance re-checking | `program.rs` |
| Match exhaustiveness | `exhaust.rs` |
| Affine ownership and borrows | `ownership.rs` |
| Region escape (scoped values leaving their scope) | `regions.rs` |
| Task and channel patterns that always deadlock (warnings) | `concurrency.rs` |

After type checking, the driver runs analyses that need the checked
program, in `crates/badger_driver/src/`:

- `contracts.rs`: an abstract interpreter over function bodies that
  refutes calls and returns breaking `requires` / `ensures` (errors),
  proves clauses (their runtime checks are compiled out), and proves
  arithmetic free of overflow (`badger contracts` reports it).
- `authority.rs`: ambient-authority reachability for `restricted`
  projects.
- `sql.rs`, `sqlcheck.rs`: literal SQL prepared against the schema.

### 4. MIR: monomorphization and optimization (`badger_mir`)

- `lower.rs` (with `lower/`): a worklist of function instances, each
  re-checked at concrete types and lowered to an explicit control-flow
  graph (`mir.rs`) with checked arithmetic and indexing, explicit
  cleanup edges, and source locations. Self tail calls become loops.
  `lower/fusion.rs` turns visible standard-library sequence pipelines over
  ranges and lists into one loop; `sra.rs` (scalar replacement) keeps
  records that never leave a function out of the heap.
- `layout.rs`: concrete layouts and the runtime type descriptors.
- Lazy `Seq` pipelines are lowered as one loop for `collect`, `sum`,
  `count`, `product`, `fold`, `any`, `all`, and `for`; `collect` and eager
  `List.map`/`scan` use a builder pre-sized from the known input length
  (scalar elements). `count`/`sum`/`product` are one accumulator shape with
  a combine step; `any`/`all` stop at the first item that decides.
  `count` over a source that knows its length, through stages that neither
  drop nor re-shape items, is that length: no loop is emitted.
- `@fusion(role = ..., template = ...)` marks a sequence function for
  fusion. The kind is one of a closed set (the sources; the stages `call`,
  `cursor` with a `mode`, `scan`, `enumerate`, `dedup`, `distinct`; the terminals
  `fold`, `accumulate` with a `combine`, `collect`, and the searches), the
  role, arity, and required parameters must match the function it is
  written on (`E0200`), and the values must be ones the compiler knows
  (`E0194`). Any module may annotate — the recognizers still require a
  matching shape and receiver type, so an unsuited marking simply does not
  fuse. A kind names *semantics*, which no signature carries — a search
  that returns the last match has `find`'s shape, and a filter that drops
  what its predicate accepts has `filter`'s — so the annotation is the
  claim, and the shape check is the part the compiler can verify. `call`'s
  choice of `map`/`filter`/`filter_map` from the callback's return type is
  the exception, and is sound only because those three differ by nothing
  else.
- Eager list and map chains are lowered as one traversal before code
  generation: `filter(..).map(..)` / `map(..).filter(..)` when the earlier
  callback is pure and infallible (`BADGER_NO_LIST_FUSION=1` disables), and
  `Map.entries().map/filter/fold` (`BADGER_NO_MAP_FUSION=1` disables).
- Optimized builds (`--release`, not under `--watch` or `--inspect`, not
  with `BADGER_NO_INLINE`) run the pipeline in `opt/`: `opt/globals.rs`
  (module values with constant initializers become constants),
  `inline.rs` (small direct callees and known closures, then scalar
  replacement again),
  `opt/specialize.rs` (clone a helper called with a known closure, then
  call the lambda directly), `opt/simplify.rs` (constant/copy propagation,
  folding, dead definitions, branch folding, switch-chain merging, CFG
  cleanup), map/snapshot
  reuse, `split.rs` (channel receives whose result is only examined),
  `niche.rs` (local options as nullable pointers), jump threading
  (`opt/cfg.rs`, the `thread` pass), `enums.rs` (local enums
  as scalars), `opt/cse.rs` (common subexpressions), and `reach.rs` (drop
  unreferenced functions). `--opt aggressive` adds another
  inlining/simplification round; `--opt superopt` adds the verified
  arithmetic/boolean superoptimizer. `BADGER_NO_PASS` disables named
  passes, `BADGER_DUMP_MIR_PASSES=1` dumps after each pass, and
  `verify.rs` checks MIR invariants at the end.
- `roots.rs`: which pointer locals need a GC root slot.
- `serial.rs`: binary MIR files (`badger compile`).

### 5. Code generation and JIT (`badger_codegen`)

- `lib.rs`: MIR to Cranelift IR, compiled in process with `cranelift-jit`.
  Every function takes `(ctx, env, params...)`. Live managed pointers are
  stored in a shadow-stack frame at each safe point (precise roots).
  Scheduling and cancellation polls go at function entries and loop
  back-edges; each prologue checks the task's stack limit. Closures used
  as values get word adapters so the runtime can call them.
- Under `--watch` or `--inspect`, calls between user functions go
  through dispatch slots so a new version can replace them.
- `debuginfo.rs`: DWARF for `badger run --debug`, registered through the
  GDB JIT interface.

### 6. Runtime (`badger_runtime`)

Linked into the `badger` process and called from generated code through
`extern "C"` functions named `badger_rt_*`.

| Part | Where |
|---|---|
| Tasks: stackful coroutines (corosensei), the scheduler (per-worker run queues with stealing, 1 to 64 workers), timers, parking, cancellation, cleanup ladders, the blocking pool | `task.rs`, `intrinsics_task.rs` |
| Task scopes (structured ownership of child tasks) | `scope.rs` |
| Heap: precise, nonmoving, generational mark-sweep; per-worker size-class pools; write barrier | `gc.rs`, `gc/` |
| Type descriptors (layout, tracing, display, equality, hashing, schema) | `desc.rs`, `value.rs` |
| Data: strings, paged lists, HAMT maps and sets, buffers | `string.rs`, `list.rs`, `hamt.rs`, `map.rs`, `buffer.rs` |
| Channels (prompt, rendezvous, batch), selection, one-shot replies | `channel.rs`, `reply.rs` |
| JSON codecs and `Data` | `codec.rs`, `intrinsics_codec.rs` |
| Native modules host (load, call, convert values) | `native.rs` |
| Reproduction bundles (`badger run --record`, `badger replay`) | `repro.rs` |
| Live code versions (`--watch`, REPL) and inspection snapshots | `live.rs`, `inspect.rs` |
| Logs, metrics, trace export | `telemetry.rs` |
| Sampling profiler (`BADGER_PROFILE`) | `profile.rs` |
| Contract observation for `badger contracts --tests` | `observe.rs` |

Most runtime code runs under one runtime lock; channel fast paths, waits,
spawns, joins, and yields do not take it. A collection stops the world.
ARCHITECTURE.md's "Runtime" section has the details.

## Crate map

| Crate | Role |
|---|---|
| `badger_base` | Spans, source map, interned symbols, diagnostics (codes, JSON, rendering), the diagnostic catalog (`badger explain`). |
| `badger_syntax` | Lexer, layout, chumsky grammar, lowering to the AST, formatter. |
| `badger_hir` | Name resolution, desugaring, HIR. |
| `badger_stdlib` | The standard library sources, embedded: `std/*.bdg`, `stdx/*.bdg`. |
| `badger_typeck` | Type inference and the static checks (effects, ownership, regions, exhaustiveness, deadlock warnings). |
| `badger_mir` | Monomorphization, layouts and descriptors, MIR, optimization passes, binary MIR. |
| `badger_codegen` | MIR to Cranelift, the JIT module, program entry, debug information. |
| `badger_runtime` | Everything compiled code calls at run time (see above). |
| `badger_driver` | Sessions and the pipeline; manifests, dependencies, releases; test runner; live reload; inspection endpoint and REPL; analyses for the language server and MCP; contracts, authority, and SQL checks; links the standard native modules. |
| `badger_cli` | The `badger` binary: commands (`main.rs`), language server (`lsp.rs`), MCP server (`mcp.rs`), `badger doc` (`doc.rs`), `badger graph` (`graph.rs`). |
| `badger_native` | The native module ABI, the value wire format, and helpers for writing modules in Rust. |
| `badger_native_demo` | A native module that exercises the ABI, for the tests. |
| `badger_sqlite` | Native module behind `stdx.sqlite` (rusqlite). |
| `badger_postgres` | Native module behind `stdx.postgres` (tokio-postgres). |
| `badger_valkey` | Native module behind `stdx.valkey` (redis-rs). |
| `badger_fetch` | Native module behind `stdx.fetch`: outbound HTTP(S) (ureq). |
| `badger_http` | Native module behind `stdx.http`: the HTTP/1.1 server (hyper on tokio). |
| `badger_fs` | Native module for `std.fs` beyond the runtime's own text reads and writes. |
| `badger_formats` | Native module behind `stdx.toml`, `stdx.yaml`, `stdx.csv`. |
| `badger_template` | Native module behind `stdx.template` (minijinja). |
| `badger_crypto` | Native module behind `stdx.encoding`, `stdx.crypto`, `stdx.uuid`. |
| `badger_datetime` | Native module behind `std.datetime` (jiff). |
| `badger_compress` | Native module behind `stdx.compress` (gzip, zlib, deflate, zstd). |

## The standard library, intrinsics, and native modules

- **Sources.** Every standard module is an ordinary Badger file in
  `crates/badger_stdlib/`: `std/*.bdg` (the foundation) and `stdx/*.bdg`
  (the extended library), so `std/list.bdg` is `std.list` and
  `stdx/http.bdg` is `stdx.http`. They are listed in
  `crates/badger_stdlib/src/lib.rs` and embedded with `include_str!`. A
  program loads the core modules plus the ones it imports. A new module
  needs a file and an entry in that list (`crates/badger_stdlib/tests/parity.rs`
  keeps the two in step).
- **Intrinsics.** A standard function implemented by the runtime is
  declared `extern "intrinsic" fn NAME(...)`. MIR lowering either expands
  it inline or calls the runtime symbol `badger_rt_i_NAME`. Runtime
  intrinsics share one ABI:
  `fn(ctx: *mut TaskCtx, targs: *const &TypeDesc, ret: &TypeDesc, args: u64...) -> u64`
  (see `crates/badger_runtime/src/lib.rs`; `symbols()` lists them for the
  JIT linker).
- **Native modules.** `extern "native NAME" fn` declares a function of
  native module `NAME`. The standard ones (`badger_sqlite`,
  `badger_http`, ...) are linked into the executable
  (`register_native_modules` in `badger_driver`); others load from
  `libNAME.so` (search path: `BADGER_NATIVE_PATH`, then next to the
  executable). Values cross by copy in a typed wire format.
  [native-modules.md](native-modules.md) describes the ABI.
- **Reference docs.** `docs/api` is generated: `badger doc std --out
  docs/api`. A test fails when it is out of date.

## Tests

### Program fixtures (`tests/`)

| Directory | Contents |
|---|---|
| `tests/run` | Programs that must run and print `NAME.out` (exit code `NAME.exit`, default 0), on one worker and on four. |
| `tests/reject` | Programs that must fail to check with the codes in their `// expect-error: CODE` lines. |
| `tests/warnings` | Programs that check with the warnings in their `// expect-warning:` lines. |
| `tests/stress` | Self-checking concurrency programs, run under chaos scheduling and heap verification. |
| `tests/projects` | Project fixtures: dependencies and lockfiles, profiles, tooling (entries, tasks, runtime settings), restricted projects, typed SQL. |
| `tests/live`, `tests/inspect` | Live reload; inspection and the REPL. |
| `tests/testing` | `std.test` fixtures: controlled clock, simulation. |
| `tests/diagnostics`, `tests/explain`, `tests/errors`, `tests/contracts` | Structured diagnostics, `badger explain`, `main` errors, contracts. |
| `tests/runtime`, `tests/cleanup` | Runtime behaviour: deadlock reports, cleanup grace, heap budget. |
| `tests/http`, `tests/net`, `tests/native`, `tests/log`, `tests/jobs`, `tests/sqlgen`, `tests/postgres`, `tests/valkey` | Services and libraries. |
| `tests/adversarial` | Retained independent regressions with their controls. |

### Cargo tests

| Test | Covers |
|---|---|
| `crates/badger_cli/tests/programs.rs` | The fixtures above, examples, projects, tooling commands, live reload, REPL, profiler, debugging (when gdb is installed). |
| `.../stress.rs` | `tests/stress` on 1, 4, and 8 workers with `BADGER_CHAOS` and `BADGER_GC_VERIFY`; `BADGER_STRESS_SECS=N` soaks. |
| `.../fuzz.rs` | Mutated corpus programs must get diagnostics, never a crash; the formatter is idempotent and keeps diagnostics. |
| `.../docs.rs` | Every ` ```badger ` block in `SPEC.md`, `spec/`, `ARCHITECTURE.md`, and `docs/` checks; `docs/api` is current; every diagnostic code has a catalog entry. |
| `.../lsp.rs`, `.../mcp.rs` | The language server and MCP server over their protocols. |
| `.../service.rs` | The HTTP/SQLite service example with a real HTTP client. |
| `.../postgres.rs`, `.../valkey.rs` | Against live servers; skipped when none is available. |
| `.../adversarial.rs` | `tests/adversarial`. |
| `.../gc_retention.rs`, `option_niche.rs`, `packed_arrays.rs`, `scalar_enums.rs`, `sequence_fusion.rs`, `transient_maps.rs` | Targeted regressions for collector safety and MIR optimizations. |
| `crates/badger_runtime/tests/*.rs` | Runtime unit-level tests (cancelled waiters, task ids, retention). |
| `crates/badger_syntax/tests/format_repo.rs` | Formatting every repository source preserves its parse tree; sources are canonical. |

Unit tests live next to the code (`#[cfg(test)]` modules).

## Building and running the suite

The toolchain is pinned in `rust-toolchain.toml` (Rust 1.98).

```sh
cargo build --release                      # target/release/badger
cargo test --workspace --release           # everything
cargo test --release -p badger_cli         # the CLI's tests (plain `cargo test` runs only these)
cargo test --release -p badger_cli --test docs        # documentation examples
cargo test --release -p badger_cli --test programs run_programs   # tests/run (name filter)
```

- **Fuzzing:** `cargo test --release -p badger_cli --test fuzz` runs a fixed
  number of cases; `BADGER_FUZZ_SECS=N` runs for N seconds and
  `BADGER_FUZZ_SEED=N` picks the seed. A failing input is saved under the
  system temp directory and named in the report.
- **Stress soak:** `BADGER_STRESS_SECS=N cargo test --release -p badger_cli
  --test stress` cycles randomized configurations; `BADGER_STRESS_SEED`
  picks the start.
- **Sanitizers:** ThreadSanitizer and AddressSanitizer builds need nightly
  and the `tsan` / `asan` features: [sanitizers.md](sanitizers.md).
- **Allocator:** `cargo build --release -p badger_cli --features jemalloc`
  uses jemalloc as the process's global allocator — −16 % wall / −16 %
  typecheck over the 722-file check corpus, at +23–88 % peak RSS, so it is
  off by default:
  [reports/alloc-deadcalls/2026-10-21-global-allocator.md](../reports/alloc-deadcalls/2026-10-21-global-allocator.md).
- **Debug switches** (`BADGER_GC_STRESS`, `BADGER_GC_VERIFY`,
  `BADGER_CHAOS`, `BADGER_DUMP_ASM`, `BADGER_TIMINGS`, ...):
  [reference/config.md](reference/config.md#debugging-and-diagnostics).
- **Debugging generated code and profiling:** [debugging.md](debugging.md).

## Where to start

| To change | Start in |
|---|---|
| Syntax | `crates/badger_syntax/src/token.rs`, `grammar.rs`, `lower.rs`; then `format.rs` |
| A diagnostic | the phase that reports it; add catalog text in `crates/badger_base/src/catalog.rs` |
| A standard function | `crates/badger_stdlib/std/MODULE.bdg` or `crates/badger_stdlib/stdx/MODULE.bdg`; an intrinsic also in `crates/badger_runtime/src/intrinsics*.rs` |
| A CLI command | `crates/badger_cli/src/main.rs`, and the `Session` method it calls in `badger_driver` |
| A runtime setting | `RUNTIME_KEYS`, `RUNTIME_DEFAULTS`, `check_runtime` in `crates/badger_driver/src/manifest.rs`; applied in `apply_runtime_setting` (`lib.rs`) |
