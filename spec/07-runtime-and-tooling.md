# Standard library, tooling, and implementation

Status: draft 0 proposal. See [the specification index](../SPEC.md).

## Blessed application facilities

Ship one versioned standard application stack with compatible contracts. Its
modules are ordinary imports and can be replaced at declared interfaces. The
runtime supplies scheduling, memory, I/O, and inspection; it does not impose
an HTTP framework on a CLI program.

| Module | Initial responsibility |
|---|---|
| `std.result`, `std.option` | Expected failure, absence, transformations, explicit conversions |
| `std.records` | Typed removal, merge, projection, shape reflection |
| `std.list`, `std.buffer`, `std.array`, `std.map`, `std.set` | Collections and builders |
| `std.seq`, `std.stream` | Typed lazy iteration and suspending streams |
| `std.task`, `std.channel` | Scoped concurrency, joins, bounded queues, batches |
| `std.system`, `std.worker`, `std.pipeline` | Lifecycle graphs, worker policy, typed interceptors |
| `std.io`, `std.fs`, `std.path`, `std.process`, `std.env` | OS interaction and subprocesses |
| `std.text`, `std.bytes` | Unicode text, byte views, and decoding |
| `std.time`, `std.random` | Explicit clocks, durations, and random providers |
| `std.http`, `std.route` | Request/reply adapters, interceptors, typed routing descriptions |
| `std.sqlite` | A blessed embedded storage adapter and scoped transactions |
| `std.codec`, `std.schema`, `std.config` | Typed schemas, JSON/data conversion, validated configuration |
| `std.parse`, `std.regex` | Grammar/parser values, source spans, structured failures |
| `std.trace`, `std.test` | Structured runtime events, assertions, dependency fixtures |

The prelude provides primitive types, `Option`, `Result`, common protocols,
`ok`, `err`, `some`, `none`, `schema`, `panic`, and assertions. Other modules
are imported explicitly. There is no hidden current database, current system,
or process-global dependency lookup.

## Reference API contracts used by worked examples

These are proposed standard-library contracts, not existing implementations.
Region/effect parameters not written here are retained by the actual signature.

- `env.args()` returns arguments excluding the executable as `List<String>`.
- `config.from_env<T>(prefix)` decodes required schema fields from prefixed
  environment keys, returning `Result<T, config.Error>`. Missing fields and
  failed numeric conversions are structured errors. Defaults are explicit
  schema/configuration values, not omitted record fields.
- `fs.read_text(path)` returns `Result<String, io.Error>`; `text.lines`,
  `text.trim`, and `text.is_empty` supply typed string views/operations.
- `io.println(value)` returns `Result<Unit, io.Error>` and requires `Display`.
- `codec.decode<T>(bytes)` decodes JSON bytes using `Codec`; `codec.from_data<T>`
  validates a `Data` value. Both return `codec.DecodeError` on failure.
- `sqlite.pool` is a provider with `{path: String}` configuration. Its output
  is a shareable, scoped `sqlite.Pool<S>`. `execute` accepts a pool/transaction,
  SQL string, and `List<Data>` parameters. `query_one` returns a row as `Data`.
  Missing/multiple rows and driver failures return `sqlite.Error`.
- `sqlite.transaction(pool, callback)` gives a task-confined `sqlite.Tx<TxScope>`
  to its callback, commits after `ok`, and attempts rollback after error or
  cancellation. Its result includes begin/commit/rollback failures without
  discarding the callback's error. The callback error accepts direct conversions
  from `sqlite.Error`. Concurrent operations on one transaction are not implied.
- `http.server` takes `{address: String, port: U16, handler: Handler}` and
  returns a managed server. It owns sibling request scopes and bounded admission.
  A handler is checked under a fresh request region and may capture a longer-lived
  system pool. `http.handler(callback)` checks that request-scoped callback
  contract and packages a typed handler. `http.json(status, value)` returns `Result<Response, http.Error>`;
  finite response bodies are owned immutable bytes. Streaming replies carry a
  request-region bound and are fully consumed/closed before that request scope
  ends. The adapter validates these bounds rather than erasing them.
- `time.now()` returns the configured clock's `Instant`. Subtracting instants
  gives `Duration`. Tests provide a controlled clock explicitly through the
  enclosing runtime fixture; arbitrary wall-clock reads are not pure.
- `trace.event(symbol, record)` records a structured diagnostic event using
  a derived schema and returns `Unit`. Sink overload follows configured bounded
  diagnostic policy and is reported separately; business I/O is not routed through
  a supposedly infallible trace function.
- `system.provider({name, init})` creates a typed provider. `init` receives its
  initialization scope and resolved config and returns a `Result`. Its scope
  registers cleanup; no missing `halt` field is synthesized in ordinary records.
- `pipeline.stage({name, enter, leave, error})` is the checked descriptor factory;
  optional callbacks are normalized by this factory's intrinsic schema. These
  omissions are not general record-field defaulting. `handle` and
  `handle_context` mark terminal handlers in a pipeline description.

## HTTP, routing, and parsing

Keep the Ring-style boundary: request/reply values, a typed handler, and a thin
transport adapter. Standard request metadata is immutable; scoped body readers
and request task groups retain their lifetime/capability types. Request size,
concurrency, and stream buffering limits are explicit runtime configuration.

Routes are inspectable typed descriptions. A route associates method/path,
parameter/body schemas, a handler/pipeline, and a stable name. Validate conflicts
and required fields before serving; derive URL construction from the same
description. Runtime path/schema failures remain structured errors. The router
is a library compiling route data, not a second general-purpose language or an
untyped annotation system. OpenAPI generation uses the same schemas when their
wire contracts support it.

Parser values accept grammar strings or programmatic productions. Preserve
support for left recursion, ambiguity, source spans, detailed failures, and
optional recovery. Return packed parse forests where ambiguity requires them;
enumerating trees is bounded/lazy, and cycles or enumeration budgets are reported
explicitly. This standard parsing library is independent of the language's
own pest grammar. Compile-time grammar optimization may cache a constant grammar
later, but general macros/evaluation are not prerequisites for the API.

## Project tool and machine interface

The proposed project file is `lang.toml`, with package identity, dependencies,
entry modules, toolchain version, and runtime profiles. Resolve dependencies with
a lockfile and content/version verification. Configuration cannot run arbitrary
code during import. User tasks are explicit typed functions invoked by the tool.
Profile merging produces inspectable effective configuration before execution.

A project may declare `restricted = true` in `[package]`. Then only the
entry module may use ambient authority: the standard functions that reach
the filesystem, processes, the environment, databases, or the network by
name (`fs.read_text`, `process.run`, `env.get`, `sqlite.open`,
`http.listen`, ...), directly or through any chain of calls or function
references (E0170, with the path). Other modules are given capabilities by
the entry, such as `fs.Dir` (a directory tree; `Dir.reader` narrows it to a
read-only `fs.ReadDir`), so their parameter types state what they may touch.
Capabilities have private fields and are made only by their providers.

Target commands:

```text
lang check [path] [--json]
lang run [entry] [--profile name] [-- args...]
lang test [filter] [--json]
lang format [path] [--check]
lang repl [--connect endpoint]
lang inspect [system|tasks|channels|plans] [--json]
lang explain [type|effect|diagnostic] target [--json]
```

All of these commands are implemented; a missing path means the current
directory's project, and `lang run NAME` selects a manifest `[entries]`
module. User tasks are `[tasks]` entries (`name = "src/file.lang#function"`)
run by `lang task NAME [-- args]`. Runtime settings live in `[runtime]`, with
`[profile.NAME.runtime]` overrides and `LANG_<SETTING>` environment
overrides; `lang inspect runtime` prints the effective values and their
sources. Unknown manifest sections and keys are errors; `lang inspect runtime` lists
every setting, including defaults, and invalid environment overrides are
errors. `lang check`/`lang test` cover every entry and task module of a
project; entry and task functions take no parameters or `List<String>`. `inspect` reads
`plans` and the effective `config` from a project, and `system`, `tasks`,
`channels`, `heap`, and the code version from a program started with
`lang run --inspect ADDR` (`--connect ADDR`), which also serves
`lang repl --connect`. `lang run --watch` performs compatible live
replacement from source changes; `--reconstruct` adds controlled
reconstruction for incompatible changes.

Structured diagnostics contain stable codes, severity, module/source spans,
expected/actual shapes, ownership/scope paths, and related definitions. Render
the same information for people and tools. Introspection reports running task
state, wait reason, owner scope, bounded queue occupancy, provider generation,
pipeline stage, and code version. High-volume value tracing is explicit and
schema-driven; code need not be stored as dynamic maps to inspect it.

Implementation: JSON diagnostics (`--json`) carry `code`, `severity`,
`message`, labels with file, module, and line/column spans, `expected` and
`actual` shapes, the enclosing definition as `scope` (`fn app.Point.norm`),
`related` declarations, and notes; the human rendering shows the same fields.
Task snapshots include the code version a task started on and the pipeline
stages it is inside; system snapshots list each node's provider and
generation (acquisitions of that key and provider in the process). The trace
sink is bounded (`trace_capacity`, `trace_overflow = "drop_oldest" |
"drop_newest"`); losses never block the caller and are reported separately
(`trace.stats()` and a summary on stderr at exit).

Testing includes ordinary tests, schema round trips, behavioral/property checks,
and controlled-runtime fixtures. A simulation controls clock, randomness,
external providers, and scheduler decisions. Record/replay promises apply only
to controlled or recorded inputs; arbitrary native I/O is not replayable by
declaration. Implementation: `std.test` provides a controlled clock
(sleeps complete by virtual time, advanced explicitly or automatically once
nothing else can run; each test starts on the real clock), seeded scheduling
(`schedule_seed`, `LANG_SCHED_SEED`), recording and replay of scheduling
decisions, `explore` (run a body under many schedules and report the failing
seed with its decisions), and `property` (seeded generators; failures report
the value and seed). Assertion failures report the caller's location.
Providers are replaced with `system.replace`. The runtime can report invalid cleanup/publication/lifetime behavior
in debug profiles, but runtime assertions do not replace the static contracts.

## Native execution, reload, and deployment

Compile modules into typed HIR, explicit MIR, specialized layouts, and Cranelift
code. Initial deployment uses source or a validated typed-code cache plus a
versioned runtime. Native machine-code cache keys include target/ISA, compiler,
runtime ABI, dependencies, and source/interface hashes. A standalone AOT backend
is a separate future output mode, not an inherent property of the current JIT.

A REPL can evaluate new typed definitions in an explicit session. Code versions
remain alive while tasks/closures use them. Compatible function replacement
changes new dispatches; existing stacks keep their code version. Changing a
record shape, effect/ownership contract, or resource layout requires rechecking
and controlled reconstruction/migration. Failed checks leave the old running
system intact. Resource reload coordinates explicit suspend/resume or restart
of an affected dependency group; it does not reinterpret live bytes as a new type.

Native host FFI is explicit `extern "c" fn` over approved primitive, pointer,
and `repr c` types. Managed/scoped values need a rooting/ownership adapter. Raw
pointer use requires `unsafe` and contributes `Foreign`. Foreign panics or C++
exceptions must not unwind through the language ABI. Blocking, cancellation,
thread affinity, and callback reentry are declared adapter contracts. No direct
JVM or JavaScript host model is assumed.

Implementation note on specialization: functions are monomorphized. When a
function's body requests itself at a deeper type (polymorphic recursion),
the compiler switches to one fallback instance whose growing type
arguments are `Dynamic`: heap values whose own runtime descriptor serves as
the typed dictionary for display, equality, hashing, ordering, and
encoding. This applies to functions whose constraints are among those
(`Display`, `Eq`, `PartialEq`, `Hash`, `PartialOrd`, `Transfer`,
`Share`); for others, instances nested deeper than 24 type levels are
stubs whose call is a task defect.

Implementation: contracts are words after `c` in the ABI string.
`extern "c blocking" fn` runs each call on the bounded blocking pool
(`[runtime] blocking_threads`) while the calling task parks; cancellation
waits for the call to return. `extern "c pinned" fn` runs every call on one
dedicated OS thread (thread affinity). Both move calls as machine words
(integers, `Bool`, `Char`, `ffi.Ptr<T>`; at most 8 parameters). Plain
`extern "c"` calls run inline on the worker and must return promptly. C code
cannot call back into the language: the `callbacks` contract is rejected.

## Compiler and runtime obligations

1. Lex uppercase type roles and preserve source identity through desugaring.
2. Resolve modules, generics, protocol coherence, and binding-group dependencies.
3. Solve ordinary, row, effect, and restricted region constraints. Check affine
   moves/borrows and sum-type exhaustiveness before execution.
4. Elaborate system reference holes and pipeline prefix-view obligations.
5. Choose native layouts and specialize functions with typed dictionary fallbacks.
6. Emit explicit normal/failure/cancellation edges and cleanup ladders in MIR.
   Language cleanup must not depend on undefined unwinding through C/host frames.
7. Insert precise roots, scheduling/GC polls, publication boundaries, and checked
   arithmetic/access operations. Trace suspended stacks and runtime pending data.
8. Register code/shape metadata and retain it for active tasks, values, and tools.

An initial single-worker prototype may precede multicore support, but cannot be
called conforming if it omits declared cancellation, fairness, cleanup, or
publication semantics. Work should land in vertical slices with executable
acceptance cases from [the worked examples](08-worked-examples.md).

## Implementation order

1. Literals, declarations, functions, immutable records, basic enums, diagnostics.
2. Row inference/update/projection, matching, `Result`/`?`, coherent conversions.
3. Managed roots/heap, collection builders, closures, scope brands and cleanup.
4. Single-worker tasks, cancellation/budgets, prompt channels, safe blocking I/O.
5. Paged batch publication, byte/capacity accounting, multicore transfer checks.
6. Typed systems and live-prefix pipelines, SQLite/HTTP reference application.
7. Structured inspection, controlled tests, REPL and versioned live replacement.

Library breadth grows alongside these slices. General CFG parsing and advanced
routing follow the typed core rather than blocking the first useful service.
Benchmark gates cover native record access, paged versus contiguous collection
workloads, stack/root overhead, queue latency/throughput, collection pauses, and
shutdown under failure. Algorithm/tuning choices can change without inventing
new surface syntax for each optimization.
