# Architecture

The compiler parses with **pest**, resolves and desugars into HIR, infers and
checks types, effects, regions, and ownership, monomorphizes into MIR, and
compiles MIR with **Cranelift** into machine code that runs in-process on the
runtime. The language contract is [SPEC.md](SPEC.md); this document describes
the implementation.

```text
 source text
     │  lang_syntax   layout pass (newline separators) → pest grammar → arena AST (Pratt operators)
     ▼
    AST
     │  lang_hir      modules/imports, name resolution, prelude, desugaring, const checks
     ▼
    HIR  (+ embedded standard library, also HIR)
     │  lang_typeck   binding-group SCCs, HM inference with rows/effects/regions,
     │                protocol constraint solving, coherence, exhaustiveness, ownership
     ▼
 typed HIR (side tables: expression types, resolutions, coercions)
     │  lang_mir      monomorphization worklist, layouts, closures, dyn dictionaries,
     │                explicit CFG with checked operations
     ▼
    MIR
     │  lang_codegen  MIR → CLIF, shadow-stack roots, polls, adapters, cranelift-jit
     ▼
 machine code in-process  ──calls──▶  lang_runtime (extern "C" lang_rt_*)
```

`lang_driver` runs the pipeline (`Session::check`, `Session::build`,
`Session::run`) and stops before code generation when any phase reports an
error. `lang_cli` is the `lang` binary.

## Crates

| Crate | Role | Key deps |
|---|---|---|
| `lang_base` | spans, source map, interner, diagnostics (codes, JSON, ariadne), diagnostic catalog | lasso, ariadne |
| `lang_syntax` | layout pass, `grammar.pest`, parser, AST, formatter | pest |
| `lang_hir` | resolution, desugaring, lang items, embedded `std/*.lang` | la-arena, petgraph |
| `lang_typeck` | types, unification, solver, schemes, exhaustiveness, ownership | ena, petgraph |
| `lang_mir` | monomorphization, layouts and type descriptors, MIR | index_vec |
| `lang_codegen` | MIR → CLIF, JIT module, program entry | cranelift-* |
| `lang_runtime` | heap/GC, tasks/scheduler, collections, channels, codecs, SQLite, HTTP, parsing | corosensei, rusqlite, serde_json, regex, backtrace |
| `lang_driver` | sessions, manifests/profiles, test runner, live reload | all of the above |
| `lang_cli` | command-line interface | clap |

## Front end

- **Layout.** Newlines that end statements become separator tokens; comments
  are blanked with offsets preserved, so spans stay exact.
- **Grammar.** Keywords are matched as whole words; uppercase/lowercase roles
  are lexical (types and protocols uppercase, values lowercase).
- **Formatter** (`lang format`): one canonical layout. Lines inside brackets
  indent one level past the line that opened them; spacing within a line is
  normalized token by token (operators, commas, colons, braces, generic
  arguments, calls) and re-lexed to confirm tokens are unchanged; a trailing
  comma before a same-line closer is dropped; strings and comments are
  verbatim. A repository test checks that formatting every source preserves
  its parse tree and that the sources are canonical.
- **HIR lowering** resolves names (modules, imports, prelude, locals,
  captures) and desugars pipes, `use` continuations, dotted record updates,
  field assignment, `pipeline.stage({...})`, `pipeline { ... }` (into nested
  stage runners), `system { ... }` (topologically ordered nodes with typed
  `ref(:key)` outputs; missing keys and cycles are reported here), and
  `system.select(plan, [:keys])` (a typed output projection).
- **Standard library.** `crates/lang_hir/std/*.lang` (40 modules) are ordinary
  language modules. A program loads the core modules (the prelude,
  collections, text, numbers, options and results, codecs, records:
  implementations on prelude types and items the compiler uses without an
  import) plus the modules it imports, closed over the standard modules' own
  imports; `system { }` and `pipeline { }` syntax brings in its module.
  Runtime operations are `extern "intrinsic"` declarations implemented in
  `lang_runtime` or expanded inline by MIR lowering.

## Types and checks

- **Inference** is Hindley–Milner with Rémy levels over binding-group SCCs.
  Records and effect sets are rows; `{..Rest}` rows give structural
  polymorphism. Declared generics are skolems; scope brands (`for<S>`) are
  region skolems with an escape check.
- **Protocols** are solved as deferred constraints (implementations,
  where-clauses, supertraits, derived `Eq/Hash/Display/Codec/From/Error`,
  structural `Transfer`/`Codec` capabilities). Associated types are
  projections (`T.Item`) normalized through implementation bindings.
  Coherence rejects orphan and overlapping implementations. Each impl
  method must have its protocol method's signature with `Self` and the
  protocol arguments substituted (E0220), and an impl of `P: Q` requires an
  impl of `Q` for the same type.
- **Other checks:** exhaustiveness of `match`, affine use-after-move
  (builders, buffers, `once` closures), must-use results and fallible task
  handles, effect bounds, infinite-size types, `const` initializers (their
  integer arithmetic is evaluated at check time at each expression's type:
  overflow, division by zero, and out-of-range shifts are E0139), and
  numeric literal ranges; an operator alone never picks a numeric type (a
  literal or annotation must, E0332); rows hold unique labels, so `{x: T,
  ..Rest}` implies `Rest lacks x` (E0303); `Rest: Row` is checked as a kind.
  Ignored results include unread `Result` bindings, `Result`s overwritten
  before being read, results inside a discarded tuple or list, and unused
  collections of fallible tasks; a `Result` held anywhere inside an option,
  collection, tuple, record, constructor, or a captured task's `Exit`
  counts (E0337). `ownership.rs` checks borrows (conflicts, escapes,
  assignable places, affine fields/elements); `regions.rs` tracks region
  taint so scoped values cannot escape through sequences, tasks, handles,
  `dyn`, or `Data`. Helpers are summarized by which parameter they store
  into another parameter's container (through other helpers too), so a
  scoped value stored through a helper escapes as well (E0401). A trailing region parameter (`S: Scope`, as in
  `sqlite.Pool<S>` and `system.InitScope<S>`) may be elided in annotations.
- **Ownership details:** a `let` re-initializes its names (a fresh affine
  value per loop iteration); handles (channel endpoints, cells, atoms,
  pools) are not affine even when their payload is; a `Seq` of affine items
  is one-shot; borrows of locals cannot escape inside records, tuples,
  lists, or constructor results; an owned affine value passed where a
  function takes `&T` must be borrowed explicitly (E0412; method receivers
  are borrowed automatically). A shared borrow never grants mutation or
  exclusive access (E0408); copies of a borrow, call results holding a
  reference, and values carrying one keep the source borrowed, and a held
  borrow conflicts with other uses of its source in the same call. A `Seq`
  built from an affine source is consuming. A closure copies captured
  borrows, moves a capture it consumes or that is not used after it, and
  otherwise borrows the capture (holding that loan while the closure is
  used). A consuming closure kept in a
  record or list, or returned through a call, makes its holder single-use.
  A borrow cannot be stored in a cell, atom, builder, buffer, or channel
  (E0407). Generic bodies are re-checked with their type variables treated
  as affine: a parameter the body may use twice cannot be instantiated with
  an affine type (E0404 at the call). Module-level values are pure immutable
  data: no mutable state (`Cell`, `Atom`, buffers, builders) or acquired
  resources (channel endpoints, pools, transactions, tasks), judged by what
  a value stores rather than by function signatures (E0138; a local REPL
  session's bindings are exempt). Values coerced to `dyn` must be
  `Share` (a `dyn` value is copied freely and may cross tasks), and
  task-confined cells are found through struct and enum fields. A
  declared function may return closures over a cell (at its result's
  function types); the body's inferred confinement is recorded in its
  scheme, so callers cannot send them to another task (E0402). Function
  types in struct fields, payloads, and aliases stay transferable and so
  cannot hold such closures (E0501). Generic
  bodies record the capabilities they need of their type parameters
  (`Transfer` for values sent to a task, `Share` for `dyn` coercions); every
  instantiation is held to them after checking, and requirements reaching a
  caller's own type parameters pass on to its callers. A sequence or
  closure read from an affine value by a method borrows that value while it
  is used (releasing it first is E0405). The compiler capability `Unscoped` (no region-parameterized value
  inside) is required of provider errors. A scoped value stored into an
  outer cell, atom, collection, or `var` escapes its callback (E0401).
- **Systems:** each node's output must be concrete after inference (a
  generic provider's free type argument is E0606); region arguments are
  fixed when the system runs. A node output left to the enclosing
  function's generics makes them node parameters: each instantiation of
  them must be concrete in turn, transitively through callers.
- **Effects:** after inference, a fixpoint over resolved calls charges
  every callee's modeled effects to its callers (functions and methods
  declared without an annotation are effect-polymorphic until their bodies
  are checked); a call through a protocol bound or `dyn` carries the union
  of the method's implementations; an explicit bound that is exceeded is
  E0501. Callback effects propagate through `for<S>` callback types
  and through functions carried in collections, structs, and enums; a
  function type written without `!` in a user declaration (struct field,
  payload, alias) or in a signature's result may perform any modeled
  effect (`Io`, `Wait`, `State`, `Foreign`), so calling it is charged
  accordingly (the standard library's internal plumbing is exempt);
  reading or writing a `Cell` received as a parameter or borrow is `State`;
  closure capture marks describe closure values, not effects of calling them.
- **Let-polymorphism** generalizes nonexpansive local bindings over value
  types only; an effect-polymorphic closure stays one closure (identity is
  observable, e.g. for scoped iterator closers).
- **Diagnostics** carry codes, spans with module paths, expected/actual
  shapes, the enclosing definition (`fn app.Point.norm`), and related
  declarations (the callee of an arity error, the receiver type of a
  missing method, the enum of a non-exhaustive match), rendered for people
  and as JSON (`--json`).
- **Monomorphic re-checking.** Each instance is re-checked at concrete types
  (`check_instance`), so MIR sees fully resolved types and method targets.

## MIR and code generation

- **Binary MIR** (`lang compile`, `crates/lang_mir/src/serial.rs`): a lowered
  program is saved with its runtime constants (descriptors, immortal
  strings, symbols, static closures, nullary values, type-argument arrays)
  recorded by how they were made, as a constant pool; loading makes them
  anew and patches the program, so `lang run FILE.langmir` only generates
  code. The file also carries the effective configuration and runtime
  settings.

- MIR is a CFG per function instance with typed representations (`I8..I64`,
  `F32/F64`, `Ptr`). Integer arithmetic, conversions, shifts, and indexing are
  checked (defects carry source locations); `checked_*`/`wrapping_*` helpers
  and `try_from` lower to overflow/fit tests.
- Records use a canonical layout (fields ordered by label, aligned), enums a
  tag plus payload, closures a code pointer, word adapter, and captures.
  `dyn Protocol` values carry a method dictionary.
- Self tail calls become loops. Module-level values are lazily initialized
  getters; function values referenced statically are immortal closures.
- Scalar replacement: an object used only through field loads and stores in
  its function (records, tuples, ranges — possibly through copies of the
  reference) is not heap-allocated; its fields become locals.
- Specialization: every instance is monomorphized, with a typed-descriptor
  fallback for polymorphic recursion: when a function requests itself at a
  deeper type, the growing arguments become `Dynamic` (pointer values whose
  header descriptor drives display, equality, hashing, ordering, and
  encoding), giving one fallback instance. Functions with other constraints
  keep specialization up to 24 nested type levels, past which instances are
  stubs whose call is a task defect.
- `?` records where an error started propagating; a `main` that returns an
  `Error` is reported with its code, causal chain, details, and that
  location.
- Code generation keeps every managed pointer that is live across a safe
  point (call, allocation, poll) in a shadow-stack frame (precise roots;
  `lang_mir::roots`; functions without such pointers link no frame), inserts scheduling/cancellation polls at function entries and loop
  back-edges, checks the stack pointer against the task's stack limit in each
  prologue (overflow is a task defect), and emits word adapters for
  first-class calls.
- `for` loops over sequences close a scoped iterator when control leaves
  early (`break`, `return`, `?`); closers are also task cleanups, so a
  defect or cancellation closes them too.
- `extern "c"` calls run inline; `extern "c blocking"` and `"c pinned"`
  contracts route word-sized calls through `lang_rt_ffi_call` to the bounded
  blocking pool or the dedicated pinned thread while the task parks.
- In `--watch` mode, calls between user functions dispatch through per-instance
  slots so a compatible new version replaces them (see below).

## Runtime

- **Heap:** precise, nonmoving mark-sweep. Objects carry a type descriptor
  (layout, tracing, display/equality/hashing, codec schema). Roots are task
  shadow stacks, pointer-valued module-level values (lowering records which
  slots hold managed pointers), and pinned/temporary runtime roots. Floats
  display their own width's shortest round-trip digits, with `.0` on any
  finite whole number.
  Off-heap memory owned by objects (map tries) counts toward collection.
- **Tasks:** stackful coroutines (corosensei) with cooperative polls,
  timers, parking/waking, cancellation, cleanup ladders, task scopes,
  `capture`/`try_join`/`try_all`/`timeout`, and supervised workers. Blocking
  I/O runs on a bounded thread pool and completes through an event queue.
  Task stacks are 8 MiB of address space by default (`task_stack_bytes`).
  Tests can control the clock (sleeps wait on virtual time, advanced
  explicitly or automatically when nothing else can run) and the scheduler
  (seeded choice among ready tasks, recorded decisions, replay). With
  several workers, `lang test` restarts the worker pool per test; under a
  controlled scheduler only the first worker runs tasks, so schedules are
  reproducible at any worker count.
- **Workers:** one by default (1..64); `[runtime] workers = N`, `--workers N`,
  or `LANG_WORKERS` runs
  N OS threads sharing the ready queue, and tasks migrate between them at
  suspension points. Runtime code runs under one runtime lock (an atomic
  fast path) while compiled code runs in parallel: in multi-worker builds the
  code generator brackets each runtime call with `lang_rt_enter`/`lang_rt_leave`,
  and the runtime releases the lock around calls back into compiled code.
  A collection stops the world by forcing running tasks' budgets to zero and
  waiting until no worker executes compiled code. Module-level values
  initialize once (other tasks wait), `Atom` uses atomic compare-and-swap,
  and batch staging is keyed by task, so a migrating producer keeps its
  order. Thread-local runtime state is read only through out-of-line
  accessors because a coroutine may resume on a different thread.
- **Numbers and ranges:** float↔integer conversions are `try_from` and
  succeed only when exact (finite, integral, in range, or round-tripping);
  lossless integer→float conversions are `From`. An integer range is a
  `Seq<Int>` where one is expected. Encoding a `U64` above `I64::MAX` as
  `Data` is a defect (no silent float). Bounds defects of `Bytes` and
  `Buffer` carry the caller's location.
- **Data:** paged lists (stable pages, page-sharing persistent append,
  page-retaining element references), buffers (growable and fixed-capacity)
  and arrays, persistent HAMT maps and sets with affine builders,
  (list headers, page directories, pages, and CHAMP trie nodes are all
  managed objects: versions share them, a contested directory slot or tail
  page slot is claimed atomically by one version and copied by the others,
  and none needs a finalizer; small objects come from per-worker 16-byte
  size-class pools refilled by the sweep),
  strings/bytes/symbols, JSON codecs and `Data` with versioned wire names and
  JSON Schema output, regular expressions, and an Earley parser library
  (grammar strings or productions, packed forests, bounded tree enumeration
  with cycle reports, error recovery).
- **Channels:** prompt (including capacity-zero rendezvous) and batch channels
  with paged staging, publication on limits/close, byte budgets, leased
  batches, and `select`.
- **Selection:** `channel.select` over one message type, and
  `channel.choose` over cases of different types (`on_recv`, `on_send`,
  `on_send_keep`) whose outcomes map into the caller's enum; exactly one
  case commits. A send on a closed channel resolves a `select` as
  `closed(index)` (the value stays with the caller) and drops out of a
  `choose`; an oversized send resolves as `too_large(index)`. A selection
  parked on a rendezvous channel posts its cases, so a partner (plain or
  selecting) commits one of them on its behalf. Batch receivers take part through `Op.recv_batch` and
  `on_recv_batch`, one published message at a time; batch endpoints also
  offer `try_append`/`try_send`, `recv`, and `try_recv`. Plain `select` and `on_send` drop an unsent payload, so
  they require `T: Share`; `select_keep` and `on_send_keep` return an
  affine payload to the caller.
- **Services:** SQLite (scoped transactions; a failed rollback is returned
  with the callback's error as its cause), an HTTP/1.1 server with request
  tasks, explicit limits (413/431, bounded admission with 503), streamed
  chunked replies, request-scoped handlers (`http.scoped`: a fresh request
  region, a region-branded body reader, streams consumed inside the
  request), and graceful shutdown; typed routes (path parameters, bodies,
  and replies with schemas; overlapping patterns rejected) with OpenAPI
  output;
  environment and profile configuration; a bounded trace sink whose overload
  is reported separately.

## Live reload

`lang run --watch` keeps the program running while it polls the sources. On
a change it rechecks and recompiles on the scheduler thread between task
steps. The new version is accepted only if its user type definitions are
unchanged (new types may be added) and every function present in both
versions has the same type, including its effects.
Accepted versions store their code addresses in the dispatch slots: new calls
use new code; frames already executing and closures already created keep the
code they started with (old code is never unmapped); module-level value
getters keep their state. A version that fails to check is reported and the
running version continues. An incompatible version (a changed type
definition, function type, or effect) is rejected unless `--reconstruct` is given:
then the runtime performs controlled reconstruction. It publishes the new
version, cancels the running main task (its scopes settle their tasks and
cleanups halt system resources in reverse order), and starts the new
version's `main`, which builds every value afresh. Live bytes are never
reinterpreted as a new type.

## Projects and dependencies

`lang.toml` (a TOML subset) names the package, its version and entry module,
the toolchain language (`draft-0`), `[runtime]` settings with
`[profile.NAME.runtime]` overrides, `[config]` with `[profile.NAME]` overlays,
named `[entries]`, user `[tasks]` (typed functions run by `lang task`), and
`[dependencies]` as path inline tables with an optional version prefix.
Unknown sections and keys are errors. `lang check` and `lang test` cover the
main entry and every `[entries]` and `[tasks]` module. An entry or task
function takes no parameters or the arguments as `List<String>`. Dependencies resolve transitively; a module imports
a dependency's entry as `import dep` and its other modules as
`import dep.module` (inside a package, siblings are imported with the
package's own name as prefix). `lang.lock` records each dependency's
version, path, and a content hash of its manifest and sources; builds verify
it and `lang update` rewrites it.

## Inspection and remote REPL

`lang run --inspect ADDR` serves a line protocol on a local TCP address.
Snapshot requests (`tasks` with state, wait reason, owner scope, the code
version the task started on, and the pipeline stages it is inside;
`channels` with capacity, queued/staged/leased messages, closure, and waiters;
`system` with running systems and their nodes' providers and generations; `heap` with collections and
pause times; `code` with the live version) are computed on the scheduler
thread between task steps. `eval` requests come from `lang repl --connect`:
the input is appended to the entry module (a definition replaces the
program's item of the same name), the program is rebuilt as a new code
version under the live-reload compatibility rules, and an expression runs as
a task of that version whose displayed result is the reply. The program
itself is compiled with slot dispatch while the endpoint is active. The local
`lang repl` runs the same way against an in-process session program, so
module-level values are computed once and keep their identity. Redefining
a definition with another type or effect is rejected; new types may be
added.

## Editor support

`lang lsp` is a language server on stdio (`crates/lang_cli/src/lsp.rs`).
Each edit re-checks the program the document belongs to (its project, or
the file alone when no `lang.toml` encloses it or the project's module graph
does not reach it) with every open buffer overlaid (`Session::overlays`);
a burst of edits is checked once. The check is the ordinary one, so
diagnostics are exactly `lang check`'s; with `set_record_types` the checker
also returns a `TypeIndex` (printed types of user expressions and locals,
resolved call/method/constructor targets, and the fields and methods of each
local's type). `lang_driver::analysis` turns that and the HIR into owned
per-file data: references (span, definition target, a key identifying the
referent, hover text from `describe_def`, the same rendering as
`lang explain`), document symbols, and locals with their scopes. Hover,
definition, references, highlights, rename (occurrences spelled as the name;
library definitions are not renamed), document and workspace symbols,
formatting (the canonical formatter), and completion are answered from it.
Completion uses the last analysis that indexed the file, so it keeps working
while the text being typed does not parse: names in scope, module members
after `module.`, fields and methods after `value.`. Setup for editors is in
[docs/editors.md](docs/editors.md).

`lang query KIND PATH [--line L --col C]` answers one of the same
questions (diagnostics, hover, definition, references, completion, symbols)
as JSON with 1-based positions. `std.compiler` wraps it for programs: typed
records (`Diagnostic`, `Location`, `Symbol`, `Completion`) decoded from a
`lang query` subprocess (`LANG_EXE` names the running executable), plus
`check_source` and `explain`.

`lang graph calls|modules|types|systems [PATH] [--format dot|mermaid|json]
[--std]` draws program structure from the same analysis: calls from each
resolved call target to its enclosing function, imports between modules,
types used by struct fields and enum variants, and system plan nodes with
the nodes they reference (a node without references is drawn alone).

## Agent tooling (MCP)

`lang mcp` is a Model Context Protocol server on stdio
(`crates/lang_cli/src/mcp.rs`) with four tools. `eval` is code mode: a
persistent REPL session (the `lang repl` machinery: an in-process session
program served over the inspection endpoint) where definitions accumulate
and expressions evaluate; with `import std.compiler` the agent queries and
post-processes analysis in the language itself instead of through many
narrow tools. Evaluations have a time limit (`evalt` requests: the
endpoint cancels the evaluation task when it expires, so the session stays
usable). `check` (a path or source text), `query` (the `lang query` kinds,
diagnostic explanations, and standard module sources as API reference) and
`run` (a program or its tests, with a time limit) run in subprocesses,
isolated from the session. The session's standard output is redirected to
a pipe and returned with each evaluation, since stdout carries the
protocol. Setup in [docs/agents.md](docs/agents.md).

## Contracts

`requires` / `ensures` lines before a function parse into `FnDecl::contracts`.
HIR lowering turns each `requires` into a check at the start of the body
(`contract_caller()` reads, once on entry, the call site that direct and
method calls of such functions record with `lang_rt_caller_loc`;
`precondition_failed` reports there, naming the clause) and keeps `ensures`
clauses on `FnDef` with a `result` local. Type checking checks postconditions
as `Bool` after the body (with `result` of the return type), the ownership
pass walks them after the body, and MIR lowering routes every return (the
body's value, `return`, and `?`) through one epilogue that checks them.

## Debug information

`Stmt::Loc` statements mark source positions in MIR (each statement and
block result of user code; `Program::debug_files` names the files). With
`lang run --debug` (`lang_codegen::set_debug_info`) code generation turns
them into Cranelift source locations, keeps every named local in a stack
slot (stored on each assignment), and after finalization builds DWARF
(`crates/lang_codegen/src/debuginfo.rs`): a line program, a subprogram per
function with parameters and locals at frame-pointer offsets taken from
Cranelift's frame layout, base types by representation, and `.debug_frame`
from Cranelift's unwind information. The DWARF goes into an in-memory ELF
image whose `.text` is `NOBITS` at the code's addresses, registered through
the GDB JIT interface (`__jit_debug_register_code`), so gdb or lldb set
breakpoints by source line, step, unwind, and read and watch locals. See
[docs/debugging.md](docs/debugging.md).

## Testing

- `tests/run/*.lang` must run and print `NAME.out` (exit code `NAME.exit`),
  both on one worker and with `--workers 4`.
- `tests/reject/*.lang` must fail with the codes in `// expect-error:` lines.
- `crates/lang_cli/tests/programs.rs` also runs the examples, profile
  configuration, path dependencies and lockfile checks, project tooling
  (entries, tasks, runtime settings, strict manifests; `tests/projects`),
  live reload (`tests/live`), inspection and the local and remote REPL
  (`tests/inspect`), `lang test` fixtures (`tests/testing`: controlled
  clock, scheduling, property checks), structured diagnostics
  (`tests/diagnostics`), `lang explain` (`tests/explain`), and checks
  `bench/`; `tests/service.rs` drives the HTTP/SQLite service, request
  isolation, HTTP limits, and streaming (`tests/http`) with a real client.
- `tests/stress/*.lang` are self-checking concurrency programs (message
  conservation across prompt, batch, and rendezvous channels, selections
  committing exactly once, cancellation and timeout storms with cleanup
  invariants). `crates/lang_cli/tests/stress.rs` runs each on 1, 4, and 8
  workers under chaos scheduling, heap verification, and optimized code;
  `LANG_STRESS_SECS=N cargo test -p lang_cli --test stress` soaks
  randomized configurations (also GC stress and binary MIR) and reports
  the exact configuration of any failure.
- `tests/adversarial` holds the retained independent regressions and
  controls behind `reports/2026-10-15-adversarial.md`.
- Unit tests cover the formatter (spacing and indentation), layout, and
  unification; `crates/lang_syntax/tests/format_repo.rs` formats every
  repository source.
- `crates/lang_cli/tests/fuzz.rs` mutates corpus programs (deleted ranges,
  inserted tokens, duplicated and swapped lines, truncation) and requires
  `lang check` (and `lang compile` when they check) to answer with
  diagnostics, never a panic or signal; `LANG_FUZZ_SECS=N` soaks, and a
  failing input is saved. It also checks that the formatter never panics
  and is idempotent on mutants, and that reformatting every corpus program
  leaves its diagnostics unchanged.
- `tests/lsp.rs` and `tests/mcp.rs` drive the language server and the MCP
  server over their protocols; `programs.rs` also covers `lang query`,
  `lang graph`, contracts, the sampling profiler, and (when gdb is
  installed) debugging generated code.

Debug switches: `LANG_GC_STRESS=1` (collect before every allocation, to
expose missing roots), `LANG_GC_VERIFY=1` (check every root and traced
pointer at each collection; freed objects are poisoned and quarantined so
a use-after-free aborts with the object's type instead of corrupting
memory), `LANG_CHAOS=<seed>` (random task budgets, random choice among
ready tasks, yields and short sleeps around runtime entry and exit, and
collections at random allocations; reproducible on one worker),
`LANG_DEBUG_INSTANCE` (instances as lowered),
`LANG_DEBUG_SCHED` (scheduler state on idle), `LANG_DEBUG_LIVE` (reload
entries), `LANG_TRACE` (export trace events to stderr), `LANG_SCHED_SEED`
(seeded scheduling in tests), `LANG_DUMP_CLIF`, `LANG_DUMP_ASM=NAME,...`
(machine code of generated functions whose names contain a pattern; `1` for
all), and `lang run --dump-mir`.

Profiling: `LANG_PROFILE=FILE` samples CPU time at 1 kHz (`SIGPROF`; the
handler walks frame pointers with fault-tolerant reads) and writes
collapsed stacks to `FILE` (flamegraph/speedscope input) plus a self/total
summary on stderr. Generated code keeps frame pointers and is symbolized by
source function name; build with `RUSTFLAGS="-C force-frame-pointers=yes"`
to also see callers of runtime functions. `LANG_PERF_MAP=1` writes
`/tmp/perf-PID.map` so external profilers attribute generated code.

## Scope notes

Programs deploy from source; dependencies are path dependencies with a
verified lockfile; there is no ahead-of-time object output (a future mode in
§07). See [README.md](README.md#status).

Known limitation: capture marks (`%region`, `%confined`) share the row
representation with effects, and calling a function charges the effect
rows of the functions it receives. So a scope-local cell first passed to
a storing helper (which ties the cell's closure rows to the helper's
effect row) and then given a closure over the scope is reported as an
escape (E0401) although nothing leaves the scope. Passing the cell to the
helper after storing scope-capturing closures, or not through a helper,
avoids it. Separating capture marks from effect rows removes it.
