# Architecture

The compiler lexes with **logos**, parses with **chumsky**, resolves and desugars into HIR, infers and
checks types, effects, regions, and ownership, monomorphizes into MIR, and
compiles MIR with **Cranelift** into machine code that runs in-process on the
runtime. The language contract is [SPEC.md](SPEC.md); this document describes
the implementation.

```text
 source text
     │  badger_syntax   lexer (logos) → layout (newline separators) → chumsky grammar → syntax tree → arena AST
     ▼
    AST
     │  badger_hir      modules/imports, name resolution, prelude, desugaring, const checks
     ▼
    HIR  (+ embedded standard library, also HIR)
     │  badger_typeck   binding-group SCCs, HM inference with rows/effects/regions,
     │                protocol constraint solving, coherence, exhaustiveness, ownership
     ▼
 typed HIR (side tables: expression types, resolutions, coercions)
     │  badger_mir      monomorphization worklist, layouts, closures, dyn dictionaries,
     │                explicit CFG with checked operations
     ▼
    MIR
     │  badger_codegen  MIR → CLIF, shadow-stack roots, polls, adapters, cranelift-jit
     ▼
 machine code in-process  ──calls──▶  badger_runtime (extern "C" badger_rt_*)
```

`badger_driver` runs the pipeline (`Session::check`, `Session::build`,
`Session::run`) and stops before code generation when any phase reports an
error. `badger_cli` is the `badger` binary.

## Crates

| Crate | Role | Key deps |
|---|---|---|
| `badger_base` | spans, source map, interner, diagnostics (codes, JSON, ariadne), diagnostic catalog | lasso, ariadne |
| `badger_syntax` | lexer, layout, grammar, lowering to the AST, formatter | logos, chumsky |
| `badger_hir` | resolution, desugaring, lang items, embedded `std/*.bdg` | la-arena, petgraph |
| `badger_typeck` | types, unification, solver, schemes, exhaustiveness, ownership | ena, petgraph |
| `badger_mir` | monomorphization, layouts and type descriptors, MIR | index_vec |
| `badger_codegen` | MIR → CLIF, JIT module, program entry | cranelift-* |
| `badger_runtime` | heap/GC, tasks/scheduler, collections, channels, codecs, native module host, parsing | corosensei, serde_json, regex, backtrace |
| `badger_native` | native module ABI, wire format, helpers for modules in Rust | — |
| `badger_sqlite` | the native module behind `std.sqlite` | rusqlite |
| `badger_postgres` | the native module behind `std.postgres` | tokio-postgres, deadpool-postgres, rustls |
| `badger_valkey` | the native module behind `std.valkey` (Valkey/Redis) | redis, tokio |
| `badger_fetch` | the native module behind `std.fetch` (outbound HTTP(S)) | ureq |
| `badger_http` | the native module behind `std.http` (the HTTP/1.1 server, SSE, WebSockets) | hyper, tokio, tokio-tungstenite |
| `badger_formats` | the native module behind `std.toml`, `std.yaml`, `std.csv`, `std.markdown` | toml, serde_yaml_ng, csv, pulldown-cmark |
| `badger_template` | the native module behind `std.template` | minijinja |
| `badger_crypto` | the native module behind `std.encoding`, `std.crypto`, `std.uuid` | base64, sha2, hmac, getrandom, uuid |
| `badger_datetime` | the native module behind `std.datetime` | jiff |
| `badger_compress` | the native module behind `std.compress` (gzip, zlib, deflate, zstd) | flate2, zstd |
| `badger_fs` | the native module behind `std.fs` beyond the runtime's text reads and writes | — |
| `badger_native_demo` | a native module for the native-ABI tests (`tests/native`) | — |
| `badger_driver` | sessions, manifests/profiles, test runner, live reload, compile-time SQL checks; links the standard native modules | all of the above, rusqlite |
| `badger_cli` | command-line interface | clap |

## Front end

- **Lexer and layout.** logos scans tokens (words, literals, punctuation;
  interpolation holes as ordinary tokens); newlines that end statements
  become separator tokens. Words are one token kind, so a keyword can still
  name a field; symbols (`#name`), `>>`, and `>=` are adjacent tokens the
  parser joins, so `List<List<Int>>` closes two argument lists.
- **Grammar.** chumsky parsers over the token slice build an owned syntax
  tree, lowered to the arena AST. The first pass tracks no errors and
  dispatches on the next token; a file that fails is parsed again with rich
  errors and recovery (each broken statement or declaration is skipped to
  the next separator), so one run reports all syntax errors, and an
  unbalanced bracket is reported at both ends. Uppercase/lowercase roles
  are checked when lowering (types and protocols uppercase, values
  lowercase).
- **Formatter** (`badger format`): one canonical layout. Lines inside brackets
  indent one level past the line that opened them; spacing within a line is
  normalized token by token (operators, commas, colons, braces, generic
  arguments, calls) and re-lexed to confirm tokens are unchanged; a trailing
  comma before a same-line closer is dropped; strings and comments are
  verbatim. A repository test checks that formatting every source preserves
  its parse tree and that the sources are canonical.
- **HIR lowering** resolves names (modules, imports, prelude, locals,
  captures) and desugars pipes, `use` continuations, dotted record updates,
  field assignment, `provider` and `stage` declarations (into
  `system.provider({...})` values and factories returning
  `pipeline.stage({...})`; the syntax crate lowers them with references to
  standard items and `#module/name` names the HIR resolves),
  `pipeline.stage({...})`, `pipeline { ... }` (into nested
  stage runners), `system { ... }` (topologically ordered nodes with typed
  `ref(#key)` outputs; missing keys and cycles are reported here), and
  `system.select(plan, [#keys])` (a typed output projection).
- **Standard library.** `crates/badger_hir/std/*.bdg` (60 modules) are ordinary
  language modules. A program loads the core modules (the prelude,
  collections, text, numbers, options and results, codecs, records:
  implementations on prelude types and items the compiler uses without an
  import) plus the modules it imports, closed over the standard modules' own
  imports; `system { }` and `pipeline { }` syntax, and `provider` and `stage`
  declarations, bring in their modules.
  Runtime operations are `extern "intrinsic"` declarations implemented in
  `badger_runtime` or expanded inline by MIR lowering.

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

- **Binary MIR** (`badger compile`, `crates/badger_mir/src/serial.rs`): a lowered
  program is saved with its runtime constants (descriptors, immortal
  strings, symbols, static closures, nullary values, type-argument arrays)
  recorded by how they were made, as a constant pool; loading makes them
  anew and patches the program, so `badger run FILE.bdgmir` only generates
  code. The file also carries the effective configuration, runtime
  settings, and the MIR optimization level it was built with. Saved
  programs run the release MIR pipeline at `badger compile` (inlining,
  closure specialization, simplification, CSE, map/list/enum
  representation passes, and reachability pruning; `--opt aggressive` or
  `--opt superopt` select deeper levels). `badger run FILE.bdgmir` uses the
  saved MIR unchanged, so the same level produces the same MIR and code
  generation as a source run; a higher level requested at run time applies
  its extra passes to the loaded program.

- MIR is a CFG per function instance with typed representations (`I8..I64`,
  `F32/F64`, `Ptr`). Integer arithmetic, conversions, shifts, and indexing are
  checked (defects carry source locations); `checked_*`/`wrapping_*` helpers
  and `try_from` lower to overflow/fit tests.
- Records use a canonical layout (fields ordered by label, aligned), enums a
  tag plus payload, closures a code pointer, word adapter, and captures.
  `dyn Protocol` values carry a method dictionary.
- `xs[i]` on a `List` whose storage representation matches its element is
  lowered inline: a length load and unsigned bounds check (the runtime
  call reports a failure), then the directory slot `i >> log2(cap)` and
  page slot `i & (cap - 1)` (`list::page_cap_for` is a power of two for
  every element size, shared with the runtime).
- Split receives (`badger_mir::split`, optimized builds after inlining): a
  channel receive whose `Option`/`TryRecv` result is only examined (tag
  read, payload loaded, through copies) calls a split form returning the
  tag, with the payload taken from the task right after
  (`badger_rt_take_aux`; a pointer payload is taken as a pointer, so it is
  rooted), and no result object is allocated.
- Self tail calls become loops. Module-level values are lazily initialized
  getters; function values referenced statically are immortal closures.
- **Release MIR pipeline** (`badger_mir::opt`): a fixed, inspectable order
  of named passes — inline, closure specialization, scalar replacement,
  simplification, map/list/enum representation passes, CSE, a second
  inlining/simplification round (`--opt aggressive`), and verified
  superoptimization (`--opt superopt`) — followed by reachability pruning
  and MIR verification. `--release` selects `release`; `--opt LEVEL`
  (`release`, `aggressive`, `superopt`) selects a deeper level, and the
  level is stored in binary MIR. `BADGER_NO_PASS=name[,name...]` disables
  individual passes, `BADGER_DUMP_MIR_PASSES=1` dumps the MIR after each
  one, and `BADGER_TIMINGS=1` reports pass counts.
- Inlining (optimized builds without live dispatch; `badger_mir::inline`):
  small, non-recursive direct callees whose argument and result
  representations match are copied into their callers (two rounds, with a
  growth cap), then scalar replacement runs again, so a record built only
  to pass to a helper is not allocated. Calls through a closure whose
  lambda is known (allocated in the caller, reached through plain copies)
  are inlined too, with the closure as the lambda's environment, and a
  callee given such a closure may be up to 96 statements (from 40), so
  `map`, `filter`, and `fold` with a lambda become loops around its body;
  the closure's allocation goes when nothing else uses it.
  `BADGER_NO_INLINE=1` disables it.
- Closure specialization (`badger_mir::opt::specialize`): a helper called
  directly with known closures — allocated in the caller and reached
  through single-definition copies — is cloned once per callee and set of
  closure arguments (all of them in one clone), with calls through those
  closures rewritten to `Callee::Lambda`: a direct call to the lambda that
  still passes the closure environment. This removes indirect dispatch even when the helper
  is too large to inline. A non-capturing lambda is called with a null
  environment, and when the clone no longer reads the closure parameter it
  is dropped from the clone's signature; the caller's closure object then
  has no non-store use and scalar replacement eliminates it. Unknown,
  escaping, or reassigned callbacks keep the original body.
- Eager list fusion (`crates/badger_mir/src/lower.rs`): `xs.filter(p).map(f)`
  lowers to one `list.filter_then_map(xs,p,f)` traversal and
  `xs.map(f).filter(p)` to `list.map_then_filter`. The fused loop interleaves
  the stages, so it is used only when the *earlier* callback is side-effect
  free and cannot fail (no calls, allocation, mutation, indexing, division,
  checked arithmetic, or shift; field reads of concrete records and total
  comparisons/bitwise operations qualify). Then its calls are unobservable
  and the later stage's calls and failures still occur on the same elements
  in the same order. No intermediate list is built. `BADGER_NO_LIST_FUSION=1`
  keeps the two-pass form.
- Sequence terminal fusion (`crates/badger_mir/src/lower/fusion.rs`):
  `Seq` pipelines (`range`, `list.of`, stored sequences, `map`, `filter`,
  `take`) consumed by `collect`, `sum`, `fold`, `count`, or `for` lower to
  one loop. The lazy adapter closures, `Cell`s, and iterator objects are
  not created. `collect` builds one list; when the source count is known
  and no filter can drop items, the builder sizes its pages and directory
  up front.
- Pre-sized list builders (`badger_mir::lower` + `badger_runtime::list`):
  `list_builder_capacity` allocates the final directory and one page per
  `page_cap` elements up front; appending then fills pages in place instead
  of growing/copying the directory and allocating pages during the loop.
  Reserved pages are traced while the object is a builder (`BUILDING` in
  the capacity word), so a collection cannot free not-yet-filled pages;
  `finish` clears the flag and trims the directory to the visible pages.
  Only scalar element types reserve pages: managed elements would make
  every collection scan empty reserved pages, so those builders grow
  incrementally. Eager `List.map` and `List.scan` use the sized builder.
- Map entry fusion (`crates/badger_mir/src/lower.rs`): `m.entries().map(f)`,
  `.filter(p)`, and `.fold(init,f)` lower to `map.entries_map`,
  `.entries_filter`, and `.entries_fold`, one trie walk with no intermediate
  `List<(K,V)>`. Entry order is the same trie order used by `map_entries`.
  `BADGER_NO_MAP_FUSION=1` keeps the two-pass form. Map iteration itself
  stores the current node entry's address in the cursor; generated code
  loads key and value directly from the trie node instead of copying both
  into the cursor.
- Simplification (`badger_mir::opt::simplify`): constant folding of integer
  arithmetic, comparisons, conversions, fits/overflow tests, and arithmetic
  identities; constant and copy propagation into uses dominated by their
  single definition; dead-assignment elimination (never a checked operation
  that can defect); constant-branch and switch folding; unreachable-block
  clearing; empty-goto threading and single-predecessor block merging.
- CSE (`badger_mir::opt::cse`): within a block, identical pure computations
  with unchanged operands read the earlier result. Checked operations keep
  their source location in the key; stores, calls, polls, and reassigned
  operands invalidate affected entries.
- Superoptimization (`badger_mir::opt::superopt`, `--opt superopt`):
  straight-line integer and boolean regions are rewritten by exactly
  equivalent forms — power-of-two multiplication/unsigned division and
  remainder become shifts and masks, shift pairs become masks, wrapping
  add/sub cancellation, comparison inversion, comparison-to-boolean
  normalization, double negation, and canonical operand order for
  commutative operations. Boolean rewrites are checked exhaustively over
  all assignments of their free boolean variables (at most 16 bits) before
  they are applied.
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
  `badger_mir::roots`; functions without such pointers link no frame), inserts scheduling/cancellation polls at function entries and loop
  back-edges, checks the stack pointer against the task's stack limit in each
  prologue (overflow is a task defect), and emits word adapters for
  first-class calls.
- `for` loops over sequences close a scoped iterator when control leaves
  early (`break`, `return`, `?`); closers are also task cleanups, so a
  defect or cancellation closes them too.
- `extern "c"` calls run inline; `extern "c blocking"` and `"c pinned"`
  contracts route word-sized calls through `badger_rt_ffi_call` to the bounded
  blocking pool or the dedicated pinned thread while the task parks.
- In `--watch` mode, calls between user functions dispatch through per-instance
  slots so a compatible new version replaces them (see below).

## Runtime

- **Heap:** precise, nonmoving, generational mark-sweep. Objects carry a
  type descriptor (layout, tracing, display/equality/hashing, codec schema).
  Roots are task shadow stacks, pointer-valued module-level values
  (lowering records which slots hold managed pointers), and pinned/temporary
  runtime roots. Floats display their own width's shortest round-trip
  digits, with `.0` on any finite whole number.
  Off-heap memory owned by objects (map tries) counts toward collection.
  Small objects (header and payload up to 512 bytes) come from per-worker size-class pools
  (2 MiB huge-page chunks after a worker's first); larger ones from the
  system allocator. A survivor keeps its mark (sticky mark bits): marked
  objects are the old generation. A minor collection marks young objects
  reachable from the roots and from *remembered* old objects and sweeps
  only objects allocated since the last collection; a full collection
  runs when the old generation has doubled. The write barrier remembers an
  old object written with a pointer: generated code checks after every
  pointer store into an object not freshly allocated in the same block, and
  runtime containers (list pages and directories, map builders, buffers,
  channels, atoms) call `gc::barrier`. `BADGER_GC_VERIFY=1` checks every
  minor collection for a reachable unmarked object (a missed barrier),
  `BADGER_GC_STRESS=1` collects at every allocation, and
  `BADGER_GC_GENERATIONAL=0` turns generations off. Sweeps run per worker
  list in parallel. `heap_max_bytes` budgets live data: past it the
  allocating task fails with a defect after a full collection.
- **Tasks:** stackful coroutines (corosensei) with cooperative polls,
  timers, parking/waking, cancellation, cleanup ladders, task scopes,
  `capture`/`try_join`/`try_all`/`timeout`, and supervised workers. Blocking
  I/O runs on a bounded thread pool and completes through an event queue.
  Task stacks are 8 MiB of address space by default (`task_stack_bytes`).
  Tests can control the clock (sleeps wait on virtual time, advanced
  explicitly or automatically when nothing else can run) and the scheduler
  (seeded choice among ready tasks, recorded decisions, replay). With
  several workers, `badger test` restarts the worker pool per test; under a
  controlled scheduler only the first worker runs tasks, so schedules are
  reproducible at any worker count.
- **Workers:** one by default (1..64); `[runtime] workers = N`, `--workers N`,
  or `BADGER_WORKERS` runs N OS threads; tasks migrate between them at
  suspension points. Each worker has its own run queue (a next slot for a
  task its running task woke, then a deque), and idle workers steal (a
  next-slot task only after 50 µs, for cache locality). Common operations
  need no global lock: channel fast paths hold only the channel; waits
  park without it (a task is marked parking under its own lock and its
  worker parks or queues it after it leaves its stack); yields, task start,
  spawn, join, and normal task exit run without it; idle workers wait
  without it while others are active. The remaining runtime code (most
  intrinsics, cleanups, failures, select) runs under one runtime lock:
  the code generator brackets those calls with `badger_rt_enter`/
  `badger_rt_leave`; entries listed as unlocked in the code generator take
  the lock themselves only on their slow paths. A collection stops the
  world by waiting until no worker executes compiled code (each worker
  counts itself; a worker resuming a task without the lock waits out a
  stop). Module-level values initialize once (other tasks wait), `Atom`
  uses atomic compare-and-swap, and batch staging is keyed by task, so a
  migrating producer keeps its order. Thread-local runtime state is read
  only through out-of-line accessors because a coroutine may resume on a
  different thread.
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
  JSON Schema output, regular expressions (`regex.compile` reports invalid
  patterns; pattern-string functions treat them as defects; compiled
  patterns are cached), and an Earley parser library (grammar strings or
  productions, packed forests, bounded tree enumeration
  with cycle reports, error recovery).
- **Authority:** `fs.root(path)` returns a `Dir`, a capability over one
  directory tree. Its paths are relative, and absolute paths, `..`, and
  symlinks leading out of the tree are refused (the deepest existing part of
  a path is canonicalized and must stay under the root). `Dir.sub` narrows
  it, and `Dir.reader` gives a `ReadDir`, which has no writing operations.
  Private fields keep both from being made any other way. The ambient
  `fs.read_text`/`write_text` remain, withheld from non-entry modules in
  restricted projects (see Projects); a symlink swapped in between
  resolution and use is not defended against. `std.fetch` makes outbound
  HTTP(S) requests (ureq with rustls, on the blocking pool; redirects are
  not followed, every status is a reply); `fetch.client(base)` gives a
  `Client` bound to one origin, whose request paths must start with a
  single `/`, and the ambient `fetch.get`/`post` are withheld like the
  filesystem's.
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
- **Services:** SQLite (the `badger_sqlite` native module, `crates/badger_sqlite`:
  each pool has up to four connections, each on its own thread, and every
  statement is an async native call, so a task waiting for a connection
  only parks; a transaction pins one connection from `BEGIN IMMEDIATE`
  until it ends, a cleanup rolls it back if the task is abandoned, and an
  unreachable one is rolled back; a failed rollback is returned with the
  callback's error as its cause), an HTTP/1.1 server (the `badger_http`
  native module, hyper on its own small tokio runtime: it reads requests
  and enforces the limits, and `std.http` takes admitted ones with an async
  `accept` in an acceptor task, each request in its own task of a service
  scope with a cleanup answering 500 if the task dies first) with request
  tasks, explicit limits (413/431, bounded admission with 503), streamed
  chunked replies, request-scoped handlers (`http.scoped`: a fresh request
  region, a region-branded body reader, streams consumed inside the
  request), and graceful shutdown; typed routes (path parameters, bodies,
  and replies with schemas; overlapping patterns rejected) with OpenAPI
  output;
  environment and profile configuration; a bounded trace sink whose overload
  is reported separately.
- **Typed SQL** (`crates/badger_driver/src/sql.rs`): after type checking, the
  driver finds calls of `sqlite.execute/query/query_one/query_as/
  query_one_as` with a literal SQL argument, loads the `[database] schema`
  file and the program's literal DDL into an in-memory SQLite, and prepares
  each statement (`badger_driver::sqlcheck`, with SQLite's column
  declared-type and NOT NULL metadata). Row types come from the recorded
  type of the call. The checks run in `badger check`, `badger run`, and the
  language server.
- **Durable jobs** (`crates/badger_hir/std/jobs.bdg`, written in the language
  over `std.sqlite`): tables for jobs (state as `Codec` JSON with a version,
  status, step, attempts, run-at time, owner, lease deadline, fence),
  ingested event keys, outbox operations (unique key, status, receipt,
  send deadline), and transition history; WAL journal with synchronous
  FULL. Each operation is one transaction; transitions are conditional
  updates on the claim's fence and lease, so a worker that lost its claim
  commits nothing. Dispatch commits `sending` before calling the adapter;
  an operation found `sending` past its deadline is `uncertain` and goes to
  reconciliation, never back to sending.
- **Reproduction bundles** (`crates/badger_runtime/src/repro.rs`): external
  intrinsics (fetch, process, fs, env, clocks, OS random seeds) go through
  `repro::external(kind, key, digest, redact, real)`, which in record mode
  performs the operation and keeps its outcome as JSON (serde), in replay
  mode answers from the bundle by identity and ends the process (status 3)
  on anything unrecorded, redacted, or sending different content, and is a
  single atomic load otherwise. Standard output is kept with the bundle;
  the driver adds the program identity and checks it before a replay, and
  `repro::finish` writes the bundle or compares the outcome on every exit
  path (return, error, defect, `process.exit`). Native module calls
  (SQLite among them) and the HTTP server are marked uncaptured.

## Logs

`std.log` writes structured records (`crates/badger_runtime/src/telemetry.rs`):
a level check against `[runtime] log_level` is one atomic load; a record
encodes its fields through their schema, merges the task's `log.with`
context (copied to tasks it spawns), and is written to standard error as
text or a JSON line (`log_format`), and kept in a bounded buffer while an
exporter is attached. `trace.span` sets the task's current span (trace and
span ids, inherited by spawned tasks, and stamped on log records inside
it); a task cleanup ends it as failed on a defect or cancellation.
`std.metrics` aggregates counters, gauges, and histograms (OpenTelemetry's
default bounds) by name and attributes. `std.otel` is written in the
language: a background task in a service scope drains the buffers every
interval (and once more when stopped) into OTLP/HTTP JSON bodies the
runtime builds, and posts them with `std.fetch`.

## Native modules

`crates/badger_native` defines the native module ABI and wire format, and
helpers for writing modules in Rust; `crates/badger_runtime/src/native.rs`
is the host. `extern "native NAME" fn` lowers to intrinsic
`native_callN` (N arguments) whose first argument names the site
(`"NAME\0function"`, an immortal string) and whose type arguments are the
parameter types: the host resolves the site once (a module linked in with
`native::register_builtin`, else `libNAME.so` by `dlopen` from
`BADGER_NATIVE_PATH`, the executable's directory, or `lib/badger` next to it),
encodes the arguments by their descriptors, runs the function by its
mode, and decodes the result by the result descriptor. The calls are
unlocked entries: an inline function runs on the worker without the
runtime lock; a blocking one goes through `run_blocking`; an async one
counts as outstanding external work (so the scheduler waits rather than
reporting a deadlock), completes through the external queue, and
registers a task cleanup that, if the task is abandoned first, withdraws
it and calls the module's `cancel`. Handle objects carry their module and
number; their finalizer hands `drop_handle` to a background thread.
A function a module marks recorded (`FnDesc::record`: identified by its
first k arguments, filed under a kind such as `fetch`) goes through
`repro::external` with its encoded result, so bundles capture and replay
it; one marked pure runs as usual; any other is `repro::uncaptured`.
`crates/badger_native_demo` is a module for the tests (`tests/native`).

## Live reload

`badger run --watch` keeps the program running while it polls the sources. On
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

`badger.toml` (a TOML subset) names the package, its version and entry module,
the toolchain language (`draft-0`), `[runtime]` settings with
`[profile.NAME.runtime]` overrides, `[config]` with `[profile.NAME]` overlays,
named `[entries]`, user `[tasks]` (typed functions run by `badger task`), and
`[dependencies]` as inline tables with an optional version prefix: a
`path`, or a `git` repository with `rev` and `subdir`
(`crates/badger_driver/src/deps.rs`: the repository is cloned bare into the
cache, `$BADGER_HOME` or `~/.cache/badger`, and each commit used is checked
out into its own directory; `badger.lock` records the requested `rev` and
the commit, and a build uses the locked commit while the manifest asks for
the same `rev`). `[workspace] members` (with `dir/*` patterns) groups
packages that share the lockfile at the workspace root; `badger check` and
`badger test` there cover every member. `[package] kind = "library"` marks a
package without `main`. `[env] dotenv` lists files of `NAME=value` lines
the driver loads into the environment before a source run starts
(variables already set win), and `${NAME}` / `${NAME:-default}` in
`[config]` values read the environment when `config.load` decodes them.
`[package] restricted = true` confines ambient authority to the entry module
(`crates/badger_driver/src/authority.rs`: a reachability check over resolved
calls and function references, reporting the path to the ambient function;
the ambient set is the listed functions plus every standard free function
or module value, such as a provider, that reaches an OS-facing intrinsic
through other standard free functions and values; methods act on given
capabilities and are not ambient).
Unknown sections and keys are errors. `badger check` and `badger test` cover the
main entry and every `[entries]` and `[tasks]` module. An entry or task
function takes no parameters or the arguments as `List<String>`. Dependencies resolve transitively; a module imports
a dependency's entry as `import dep` and its other modules as
`import dep.module` (inside a package, siblings are imported with the
package's own name as prefix). `badger.lock` records each dependency's
version, path, and a content hash of its manifest and sources; builds verify
it and `badger update` rewrites it.

## Inspection and remote REPL

`badger run --inspect ADDR` serves a line protocol on a local TCP address.
Every request carries the endpoint's token, a random secret made at start,
kept in memory for in-process clients (the local REPL, `badger mcp`) and
written for the user's other processes to a 0600 file in a 0700 per-user
directory (`$XDG_RUNTIME_DIR/badger-inspect`, else `~/.cache/badger-inspect`);
other local users cannot inspect or evaluate. Each connection is served
on its own thread with a read timeout; evaluations run one at a time.
Snapshot requests (`tasks` with state, wait reason, owner scope, the code
version the task started on, and the pipeline stages it is inside;
`channels` with capacity, queued/staged/leased messages, closure, and waiters;
`system` with running systems and their nodes' providers and generations; `heap` with collections and
pause times; `code` with the live version) are computed on the scheduler
thread between task steps. `eval` requests come from `badger repl --connect`:
the input is appended to the entry module (a definition replaces the
program's item of the same name), the program is rebuilt as a new code
version under the live-reload compatibility rules, and an expression runs as
a task of that version whose displayed result is the reply. The program
itself is compiled with slot dispatch while the endpoint is active. The local
`badger repl` runs the same way against an in-process session program, so
module-level values are computed once and keep their identity. Redefining
a definition with another type or effect is rejected; new types may be
added.

## Editor support

`badger lsp` is a language server on stdio (`crates/badger_cli/src/lsp.rs`).
Each edit re-checks the program the document belongs to (its project, or
the file alone when no `badger.toml` encloses it or the project's module graph
does not reach it) with every open buffer overlaid (`Session::overlays`);
a burst of edits is checked once. The check is the ordinary one, so
diagnostics are exactly `badger check`'s; with `set_record_types` the checker
also returns a `TypeIndex` (printed types of user expressions and locals,
resolved call/method/constructor targets, and the fields and methods of each
local's type). `badger_driver::analysis` turns that and the HIR into owned
per-file data: references (span, definition target, a key identifying the
referent, hover text from `describe_def`, the same rendering as
`badger explain`), document symbols, and locals with their scopes. Hover,
definition, references, highlights, rename (occurrences spelled as the name;
library definitions are not renamed), document and workspace symbols,
formatting (the canonical formatter), and completion are answered from it.
Completion uses the last analysis that indexed the file, so it keeps working
while the text being typed does not parse: names in scope, module members
after `module.`, fields and methods after `value.`. Setup for editors is in
[docs/editors.md](docs/editors.md).

`badger query KIND PATH [--line L --col C]` answers one of the same
questions (diagnostics, hover, definition, references, completion, symbols)
as JSON with 1-based positions. `std.compiler` wraps it for programs: typed
records (`Diagnostic`, `Location`, `Symbol`, `Completion`) decoded from a
`badger query` subprocess (`BADGER_EXE` names the running executable), plus
`check_source` and `explain`. `badger query check PATH --overlays FILE`
(`std.compiler.check_project`, MCP `check` with `files`) checks a project
with proposed file contents overlaid on the files on disk, the way the
language server checks unsaved buffers: every entry point and task, as
`badger check` does, reporting the inputs checked and the roots covered.

References and rename consider every entry point of a project: the
server also analyses each `[entries]` and `[tasks]` program (when first
asked, until the next change) and looks the definition up in each, so a
module reachable only from a task is renamed with the rest. `badger query
rename` returns the edits without applying them.

`badger doc std --out docs/api` writes the standard library reference
(`crates/badger_cli/src/doc.rs`) from the `api` query: declarations and `///`
documentation of each module's public types, methods, and functions, with
the module's leading comment as its introduction. `tests/docs.rs` keeps it
current and checks every `badger` code block in the specification and guides.

`badger graph calls|modules|types|systems [PATH] [--format dot|mermaid|json]
[--std]` draws program structure from the same analysis: calls from each
resolved call target to its enclosing function, imports between modules,
types used by struct fields and enum variants, and system plan nodes with
the nodes they reference (a node without references is drawn alone).

## Agent tooling (MCP)

`badger mcp` is a Model Context Protocol server on stdio
(`crates/badger_cli/src/mcp.rs`) with five tools. `eval` is code mode: a
persistent REPL session (the `badger repl` machinery: an in-process session
program served over the inspection endpoint) where definitions accumulate
and expressions evaluate; with `import std.compiler` the agent queries and
post-processes analysis in the language itself instead of through many
narrow tools. Evaluations have a time limit (`evalt` requests: the
endpoint cancels the evaluation task when it expires, so the session stays
usable). `check` (a path or source text, or a project with proposed `files`), `query` (the `badger query` kinds,
diagnostic explanations, and standard module sources as API reference) and
`contracts` (each contract clause and what the compiler decided, with test
coverage) and `run` (a program or its tests, with a time limit; under gdb
with breakpoints) run in subprocesses,
isolated from the session. The session's standard output is redirected to
a pipe and returned with each evaluation, since stdout carries the
protocol. Setup in [docs/agents.md](docs/agents.md).

## Contracts

`requires` / `ensures` lines before a function parse into `FnDecl::contracts`.
HIR lowering turns each `requires` into a check at the start of the body
(`contract_caller()` reads, once on entry, the call site that direct and
method calls of such functions record with `badger_rt_caller_loc`;
`precondition_failed` reports there, naming the clause) and keeps `ensures`
clauses on `FnDef` with a `result` local. Type checking checks postconditions
as `Bool` after the body (with `result` of the return type), the ownership
pass walks them after the body, and MIR lowering routes every return (the
body's value, `return`, and `?`) through one epilogue that checks them.

## Debug information

`Stmt::Loc` statements mark source positions in MIR (each statement and
block result of user code; `Program::debug_files` names the files). With
`badger run --debug` (`badger_codegen::set_debug_info`) code generation turns
them into Cranelift source locations, keeps every named local in a stack
slot (stored on each assignment), and after finalization builds DWARF
(`crates/badger_codegen/src/debuginfo.rs`): a line program, a subprogram per
function with parameters and locals at frame-pointer offsets taken from
Cranelift's frame layout, base types by representation, and `.debug_frame`
from Cranelift's unwind information. The DWARF goes into an in-memory ELF
image whose `.text` is `NOBITS` at the code's addresses, registered through
the GDB JIT interface (`__jit_debug_register_code`), so gdb or lldb set
breakpoints by source line, step, unwind, and read and watch locals. See
[docs/debugging.md](docs/debugging.md).

## Testing

- `tests/run/*.bdg` must run and print `NAME.out` (exit code `NAME.exit`),
  both on one worker and with `--workers 4`.
- `tests/reject/*.bdg` must fail with the codes in `// expect-error:` lines.
- `crates/badger_cli/tests/programs.rs` also runs the examples, profile
  configuration, path dependencies and lockfile checks, project tooling
  (entries, tasks, runtime settings, strict manifests; `tests/projects`),
  live reload (`tests/live`), inspection and the local and remote REPL
  (`tests/inspect`), `badger test` fixtures (`tests/testing`: controlled
  clock, scheduling, property checks), structured diagnostics
  (`tests/diagnostics`), `badger explain` (`tests/explain`), and checks
  `bench/`; `tests/service.rs` drives the HTTP/SQLite service, request
  isolation, HTTP limits, and streaming (`tests/http`) with a real client.
- `tests/stress/*.bdg` are self-checking concurrency programs (message
  conservation across prompt, batch, and rendezvous channels, selections
  committing exactly once, cancellation and timeout storms with cleanup
  invariants). `crates/badger_cli/tests/stress.rs` runs each on 1, 4, and 8
  workers under chaos scheduling, heap verification, and optimized code;
  `BADGER_STRESS_SECS=N cargo test -p badger_cli --test stress` soaks
  randomized configurations (also GC stress and binary MIR) and reports
  the exact configuration of any failure.
- `tests/adversarial` holds the retained independent regressions and
  controls behind `reports/2026-10-15-adversarial.md`.
- Unit tests cover the formatter (spacing and indentation), layout, and
  unification; `crates/badger_syntax/tests/format_repo.rs` formats every
  repository source.
- `crates/badger_cli/tests/fuzz.rs` mutates corpus programs (deleted ranges,
  inserted tokens, duplicated and swapped lines, truncation) and requires
  `badger check` (and `badger compile` when they check) to answer with
  diagnostics, never a panic or signal; `BADGER_FUZZ_SECS=N` soaks, and a
  failing input is saved. It also checks that the formatter never panics
  and is idempotent on mutants, and that reformatting every corpus program
  leaves its diagnostics unchanged.
- `tests/lsp.rs` and `tests/mcp.rs` drive the language server and the MCP
  server over their protocols; `programs.rs` also covers `badger query`,
  `badger graph`, contracts, the sampling profiler, and (when gdb is
  installed) debugging generated code.

Debug switches: `BADGER_GC_STRESS=1` (collect before every allocation, to
expose missing roots), `BADGER_GC_VERIFY=1` (check every root and traced
pointer at each collection; freed objects are poisoned and quarantined so
a use-after-free aborts with the object's type instead of corrupting
memory), `BADGER_CHAOS=<seed>` (random task budgets, random choice among
ready tasks, yields and short sleeps around runtime entry and exit, and
collections at random allocations; reproducible on one worker),
`BADGER_DEBUG_INSTANCE` (instances as lowered),
`BADGER_DEBUG_SCHED` (scheduler state on idle), `BADGER_DEBUG_LIVE` (reload
entries), `BADGER_TRACE` (export trace events to stderr), `BADGER_SCHED_SEED`
(seeded scheduling in tests), `BADGER_DUMP_CLIF`, `BADGER_DUMP_ASM=NAME,...`
(machine code of generated functions whose names contain a pattern; `1` for
all), and `badger run --dump-mir`.

Profiling: `BADGER_PROFILE=FILE` samples CPU time at 1 kHz (`SIGPROF`; the
handler walks frame pointers with fault-tolerant reads) and writes
collapsed stacks to `FILE` (flamegraph/speedscope input) plus a self/total
summary on stderr. Generated code keeps frame pointers and is symbolized by
source function name; build with `RUSTFLAGS="-C force-frame-pointers=yes"`
to also see callers of runtime functions. `BADGER_PERF_MAP=1` writes
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
