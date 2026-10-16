# Standard library, tooling, and implementation

Status: draft 0, implemented by this repository's toolchain except where marked
**Proposed:**. See [the specification index](../SPEC.md) for how to read it.

## Blessed application facilities

Ship one versioned standard application stack with compatible contracts. Its
modules are ordinary imports and can be replaced at declared interfaces. The
runtime supplies scheduling, memory, I/O, and inspection; it does not impose
an HTTP framework on a CLI program.

| Module | Responsibility |
|---|---|
| `std.prelude` | Primitive types, `Option`, `Result`, common protocols, cells, assertions (imported implicitly) |
| `std.result`, `std.option` | Expected failure, absence, transformations, explicit conversions |
| `std.records` | Typed removal, merge, projection, shape reflection |
| `std.list`, `std.buffer`, `std.array`, `std.map`, `std.set` | Collections and builders |
| `std.seq`, `std.stream` | Typed lazy iteration and suspending streams |
| `std.task`, `std.channel`, `std.broadcast` | Scoped concurrency, joins, bounded queues, batches, one-shot replies, broadcast |
| `std.system`, `std.worker`, `std.pipeline` | Lifecycle graphs, worker policy, typed interceptors |
| `std.io`, `std.fs`, `std.path`, `std.process`, `std.env` | OS interaction and subprocesses |
| `std.text`, `std.bytes`, `std.num` | Unicode text, byte views and decoding, numeric conversions and checked/wrapping arithmetic |
| `std.time`, `std.random` | Explicit clocks, durations, and random providers |
| `std.http`, `std.route` | Request/reply adapters, interceptors, typed routing descriptions |
| `std.sqlite` | A blessed embedded storage adapter and scoped transactions |
| `std.jobs` | Durable single-host jobs on SQLite: deduplicated ingestion, fenced step transitions with an outbox, receipts, explicit uncertainty |
| `std.fetch` | Outbound HTTP(S) requests and origin-bound clients |
| `std.codec`, `std.schema`, `std.config` | Typed schemas, JSON/data conversion, validated configuration |
| `std.parse`, `std.regex` | Grammar/parser values, source spans, structured failures |
| `std.trace`, `std.test` | Structured runtime events, assertions, dependency fixtures |
| `std.inspect` | Runtime snapshots of tasks, channels, systems, and the heap |
| `std.compiler` | Compiler services from code: diagnostics, project checks of proposed edits, hover, definitions, references, symbols |
| `std.ffi` | The C boundary: raw pointers, C memory, C strings |

The [standard library reference](../docs/api/README.md) lists every public
type, function, and method with its declaration and documentation; it is
generated from the checked interfaces (`lang doc std --out docs/api`) and
the test suite keeps it current.

The prelude provides primitive types, `Option`, `Result`, common protocols,
`ok`, `err`, `some`, `none`, `schema`, `panic`, and assertions. Other modules
are imported explicitly. There is no hidden current database, current system,
or process-global dependency lookup.

## Reference API contracts used by worked examples

These standard-library contracts are implemented; the
[reference](../docs/api/README.md) has their exact signatures. Region/effect
parameters not written here are retained by the actual signature.

- `env.args()` returns arguments excluding the executable as `List<String>`.
- `config.from_env<T>(prefix)` decodes required schema fields from prefixed
  environment keys, returning `Result<T, config.Error>`. Missing fields and
  failed numeric conversions are structured errors. Defaults are explicit
  schema/configuration values, not omitted record fields.
- `fs.read_text(path)` returns `Result<String, io.Error>`; `text.lines`,
  `text.trim`, and `text.is_empty` supply typed string operations (`lines`
returns a lazy `Seq<String>`).
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
- Typed SQL: `sqlite.query_as<T>` and `query_one_as<T>` decode rows as a
  `Codec` record. Literal statements given to `execute`, `query`,
  `query_one`, and the typed forms are prepared (not run) when the program
  is checked, against the project's `[database] schema` SQL file and the
  program's own literal `CREATE`/`ALTER` statements in source order:
  statements that do not compile (unknown tables or columns, syntax),
  several statements where one runs, `execute` with parameters on a
  statement that returns rows, and `query_one` on one that returns none
  are E0180; a parameter list literal of the wrong length is E0181; and a
  row type with a field that has no column, a non-`Option` field for a
  nullable column, or a field type that does not fit the declared column
  type is E0182. Without a schema file or literal DDL only syntax is
  checked; statements built at run time are not checked.
- `jobs.open(path)` opens a job store in a SQLite file. `submit(kind, event,
  version, state, max_attempts)` ingests an event once per key (`created` or
  `duplicate`) as job `kind:event` with a `Codec` state. `claim(kind,
  owner, lease)` takes a due job (a ready one whose timer has passed, or a
  running one whose lease expired, which counts as a failed attempt) and
  bumps its fence. `advance(claim, step, version, state, next, sends)`
  commits the new state, what happens next (`again`, `after(delay)`,
  `done`, `failed(reason)`), and outbox operations (idempotent by key; a
  key reused with another payload is an error) in one transaction that
  fails with `:jobs/lease_lost` or `:jobs/cancelled` when the claim no
  longer holds. `retry` records a failed attempt with a backoff; `cancel`
  fences out the worker and cancels unsent operations. `dispatch(kind,
  limit, timeout, send)` marks each pending operation `sending` (committed)
  before calling `send`, which returns `confirmed(receipt)`,
  `rejected(reason)`, `not_sent(reason)`, or `uncertain(reason)`; an
  operation still `sending` past its deadline becomes `uncertain` and is
  never resent automatically: `reconcile(kind, check)` asks the destination
  by key, and `resolve(key, outcome)` records an operator's decision.
  `migrate` rewrites unfinished jobs' state to a new version; `job`,
  `jobs`, `history`, `operations`, `count`, and `backup` (`VACUUM INTO`)
  inspect and copy the store. In restricted projects `jobs.open` is ambient
  authority; a `Store` is a capability.
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
Implementation: `[runtime] http_max_body_bytes`, `http_max_header_bytes`,
`http_max_in_flight`, and `http_max_stream_buffer_bytes` (bytes of a
streamed reply waiting for a slow client; beyond it the streaming task
waits until the client has taken them).

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

## Memory budget

`[runtime] heap_max_bytes` (default 0: none) bounds live managed data.
When live data plus new allocation would pass it, a full collection runs
first; if live data still exceeds it, the task asking for memory fails
with the defect `heap budget exceeded` (contained like any defect: by
`capture`, a scope, or supervision), and the process keeps running. The
budget is checked where compiled code allocates records, strings, list
elements, and map entries; memory the runtime allocates internally may
pass it briefly.

## Project tool and machine interface

The project file is `lang.toml`, with package identity, dependencies,
entry modules, toolchain version, and runtime profiles. Resolve dependencies with
a lockfile and content/version verification. **Implementation:** a
dependency is a directory (`path`) or a git repository (`git`, with `rev`
and `subdir`), cloned once into a cache and checked out per commit; the
lockfile pins each git dependency's commit, and `lang update` resolves
revisions again. `[workspace] members` groups packages sharing one
lockfile (`lang check`, `lang test`, and `lang update` at the root cover
them all), and `[package] kind = "library"` marks a package that others
import and nothing runs. Configuration cannot run arbitrary
code during import. User tasks are explicit typed functions invoked by the tool.
Profile merging produces inspectable effective configuration before execution.

A project may declare `restricted = true` in `[package]`. Then only the
entry module may use ambient authority: the standard functions that reach
the filesystem, processes, the environment, databases, or the network by
name (`fs.read_text`, `process.run`, `env.get`, `sqlite.open`,
`http.listen`, `fetch.get`, ...) and the standard functions and providers
built on them (`config.from_env`, `sqlite.pool`, `http.server`, ...),
directly or through any chain of calls or function references (E0170, with
the path). Other modules are given capabilities by the entry, such as
`fs.Dir` (a directory tree; `Dir.reader` narrows it to a read-only
`fs.ReadDir`), `env.Vars` (named environment variables), `process.Runner`
(named programs), `fetch.Client` (outbound HTTP to one origin), an open
`jobs.Store`, or an open `sqlite.Pool`, so their parameter types state what
they may touch.
Capabilities have private fields and are made only by their providers.
Clocks and random sources are not ambient authority: they reach nothing
outside the program, their use is visible as `Io`/`State` effects, and
reproduction bundles record them.

Commands:

```text
lang check [path] [--json]
lang run [entry] [--profile name] [--record bundle] [-- args...]
lang replay bundle [--compare]
lang test [filter] [--json]
lang task name [-- args...]
lang compile [path] [-o out]
lang format [path] [--check]
lang repl [--connect endpoint]
lang inspect [plans|config|runtime] [path] [--json]
lang inspect [system|tasks|channels|heap|code|all] --connect addr [--json]
lang explain [type|effect|diagnostic] target [--json]
lang query kind [path] [--line l --col c] [--overlays file]
lang graph [calls|modules|types|systems] [path]
lang doc [std|std.module|path] [--out dir] [--check]
lang update [path]
lang lsp
lang mcp
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

`lang run --record FILE` writes a reproduction bundle: the program's
identity (toolchain version and executable digest, every source file's
digest, arguments, a digest of the effective configuration), the outcome of
every external operation (HTTP requests, subprocesses, file reads, writes,
listings, and directory resolution, environment variables, both clocks, and
OS-seeded random sources) keyed by kind and identity, a digest of what each
write or request sends, the program's output, and how it ended (exit code,
failure, and the failing source line). The run uses one worker. Values of
environment variables whose names look like credentials (`TOKEN`, `SECRET`,
`PASSWORD`, `API_KEY`, `AUTH`, ...) and credential-like URL query values
are not stored. `lang replay FILE` runs the same program with every
operation answered from the bundle: nothing is requested, run, or written,
and an operation the bundle does not hold (or that sends different content,
or needs a redacted value, a database, or an HTTP server) ends the replay
with status 3 instead of reaching the world. Operations match by identity in
recorded order per identity, so tasks may interleave differently. The
replay exits 0 when the recorded outcome (exit code, output, failure) is
reproduced and 1 when it differs. A program that differs from the recorded
one is refused (E0016); `--compare` runs it against the recording, reports
differing payloads and operations it no longer performs, compares the
outcome, and exits with the program's code. Bundles reproduce the managed
computation from recorded inputs, not the external world's current state
or arbitrary multicore interleavings.

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
code. Deployment uses source, or a checked program saved as binary MIR
(`lang compile`; only code generation remains when it runs), plus a
versioned runtime. **Proposed:** a native machine-code cache keyed by
target/ISA, compiler, runtime ABI, dependencies, and source/interface
hashes, and a standalone AOT backend as a separate output mode (not an
inherent property of the JIT).

A REPL can evaluate new typed definitions in an explicit session. Code versions
remain alive while tasks/closures use them. Compatible function replacement
changes new dispatches; existing stacks keep their code version. Changing a
record shape, effect/ownership contract, or resource layout requires rechecking
and controlled reconstruction/migration. Failed checks leave the old running
system intact. Reload never reinterprets live bytes as a new type.
**Implementation:** `lang run --watch` replaces compatible functions;
`--reconstruct` cancels the running `main` (its scopes settle and halt
their resources in reverse order) and starts the new version.
**Proposed:** resource reload that suspends and resumes, or restarts, only
the affected dependency group.

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

Native modules are the boundary for libraries that exchange data rather
than machine words. `extern "native NAME" fn` declares a function of
module `NAME` (`"native NAME.FUNCTION"` names the module's function when
the declaration's name differs), linked into the executable or loaded
from `libNAME.so`; `std.sqlite`, `std.fetch`, and `std.http` are built on
the `lang_sqlite`, `lang_fetch`, and `lang_http` modules.
Arguments and results cross by copy in a typed wire format, so module code
never sees managed memory: values of the declared types (primitives,
text, bytes, `Data`, `native.Handle`, and collections, options, results,
records, structs, and enums of them; E0410 otherwise), checked again when
they return. The module declares each function's mode: inline (on the
worker, without the runtime lock), blocking (on the blocking pool, with
the task waiting and cancellation waiting for it), or async (completed
from any thread; cancelling the waiting task tells the module and discards
the completion). A module's panic, a malformed result, or a result that
does not fit the declared type is a defect of the calling task. Handles
name module-owned state, released when unreachable. Native functions are
ambient authority in restricted projects. [docs/native-modules.md](../docs/native-modules.md)
describes the ABI and the Rust crate (`lang_native`) for writing modules.

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

An implementation cannot be called conforming if it omits declared
cancellation, fairness, cleanup, or publication semantics, on one worker or
many. Work lands in vertical slices with executable acceptance cases from
[the worked examples](08-worked-examples.md).

## Implementation order

The implementation landed in this order:

1. Literals, declarations, functions, immutable records, basic enums, diagnostics.
2. Row inference/update/projection, matching, `Result`/`?`, coherent conversions.
3. Managed roots/heap, collection builders, closures, scope brands and cleanup.
4. Single-worker tasks, cancellation/budgets, prompt channels, safe blocking I/O.
5. Paged batch publication, byte/capacity accounting, multicore transfer checks.
6. Typed systems and live-prefix pipelines, SQLite/HTTP reference application.
7. Structured inspection, controlled tests, REPL and versioned live replacement.

Library breadth grew alongside these slices. General CFG parsing and advanced
routing follow the typed core rather than blocking the first useful service.
Benchmark gates cover native record access, paged versus contiguous collection
workloads, stack/root overhead, queue latency/throughput, collection pauses, and
shutdown under failure. Algorithm/tuning choices can change without inventing
new surface syntax for each optimization.
