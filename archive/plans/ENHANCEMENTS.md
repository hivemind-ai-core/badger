# Language, library, framework, and tooling enhancements

> **Archived 2026-10-18.** Implemented: F1 (versioned diagnostics, LSP
> `data`), F2, F3, F4, L1, L2 (also named `=>` functions), L3, L4
> (`let ... else`), L5, L6 (`+=` and friends, or-patterns), L7
> (`task.map_bounded`; deadlines and permits via `app.Call` and
> `std.admission`), L8 (provider/stage declarations), S1, S2, S3 (stream
> `filter_map`/`enumerate`/`scan`/`find`, `seq.try_map`), S4 (`std.sorted`,
> `std.heap`, `std.deque`, `std.lru`, `std.bits`), the framework (§4; see
> FRAMEWORK_DESIGN.md §25), T1 (`badger query symbol`), T2 (MCP structured
> content), T3 (insert-import quick fix), T4 (import error cascades), T5
> (`badger test --list/--exact`, failure kinds), T6 (`[layers]` import
> rules), T7 (`badger generate feature`/`worker`), T8 (the guide and its
> checked examples over MCP), T9 (`eval/` tasks and scorer). What remains
> is in [BACKLOG.md](../../BACKLOG.md). References to files below use the
> paths of the time (pest grammar, `lang_*` crates).

Assessment date: 2026-10-16. Status: proposals for review, with accepted directions explicitly identified below. This document does not change the language specification or reorder active work.

The aim is to make the language easier to read, write, compose, and verify, for people and LLMs. Prefer local syntax desugaring, additive library APIs, and extensions to existing compiler services. Preserve the uppercase type convention, explicit errors and dependencies, immutable records, scoped resources, and sequential code within concurrent tasks.

This assessment reads Claude's consolidated work order, backlog, priorities, and end-of-list plan; [SPEC.md](../../SPEC.md) and its chapters; the actual parser, lowering, standard modules, and tooling; and the [Go/Elixir design report](../../reports/2026-10-16-go-elixir-alternative-design-report.md). The backlog already covers performance, FFI/library extraction, supervision, configuration, observability, and a collaborative compiler. The proposals below refine those directions rather than treating them as new discoveries.

Priority: **P0** fixes or prerequisites; **P1** recommended next enhancements; **P2** useful follow-ups. Size: **small** means a local change using existing machinery; **medium** crosses several existing subsystems; **larger** means a sequence of library/tooling slices. These are scope estimates, not delivery promises. Each item needs appropriate validation.

LLM benefits below are hypotheses. They should be measured through real editing tasks, not assumed from shorter syntax or more tools.

## 1. Fixes and gaps

### F1. Preserve full diagnostics through every interface

**P0 · Small to medium · Confirmed gap.**

The core [diagnostic representation](../../crates/badger_base/src/diagnostics.rs) carries labels, expected/actual shapes, enclosing scope, and related declarations. The analysis/query path reduces that information: expected/actual become prose notes, and the CLI query serialization omits secondary locations. MCP `check` uses that query path. Its result is then wrapped as text.

This was reproduced with a failed piped call: `badger check --json` identifies the enclosing function and the called function's declaration; `badger query diagnostics` drops those fields. An LLM receives less useful evidence through its dedicated tool than through the ordinary compiler command.

Use one versioned diagnostic schema across CLI, LSP, MCP, and `std.compiler`. Preserve:

- Primary and related spans, fully qualified symbols, and enclosing definition.
- Expected/actual types, effects, or field sets as separate fields.
- Available ownership, region, dependency, or conversion explanations.
- Stable diagnostic codes, with human prose alongside structured information.

Start by retaining existing information. Richer constraint explanations are a later addition, not required for the first fix.

**Acceptance:** the same failure exposes equivalent diagnostic facts through all four interfaces. A tool consumer can locate the relevant declaration without parsing an English note.

### F2. Check proposed edits in their project context

**P1 · Medium · Confirmed interface gap.**

MCP `check(source=...)` writes a standalone temporary file. It does not offer a way to check a replacement module against its actual project, imports, configuration, and sibling edits. Agents therefore have to write files before checking a multi-file change.

The underlying [analysis entry point](../../crates/badger_driver/src/analysis.rs) already accepts source overlays, and LSP uses them. Expose that capability to MCP and `std.compiler`: a project path plus a set of proposed file contents. Return the identities of the inputs checked and the entry/task roots covered.

The first implementation can perform a full check. Persistent incremental compilation is a separate optimization. Checking edits should remain read-only; applying them is a distinct operation.

**Acceptance:** check changes to an interface and two consumers together, including a failing import and an incompatible signature, without modifying disk files. Results agree with checking the same saved project.

### F3. Reconcile specification and API documentation with implementation

**P1 · Small per correction · Confirmed documentation gaps.**

[SPEC.md](../../SPEC.md) still refers to an implemented placeholder grammar, while the compiler uses the substantial [pest grammar](../../crates/badger_syntax/src/grammar.pest). Chapter 07 calls some reference APIs unimplemented proposals while also documenting their implementation. These contradictions make it harder for an agent to decide what code it can actually use.

Keep a clear distinction between normative semantics, implementation limits, and proposed additions. Generate API reference material from checked interfaces and retain hand-written explanations and examples around it. Compile documentation examples in CI. Put version/toolchain information on generated material.

Do not describe existing record shorthand, dotted record updates, interpolation, contracts, task scopes, one-shot replies, API queries, or graph queries as missing features.

**Acceptance:** a new reader can tell which features work now. Every runnable documentation example is checked against its stated toolchain.

### F4. Complete the existing tooling surface consistently

**P1 · Small to medium · Confirmed API gaps.**

MCP has API and graph queries, but [std.compiler](../../crates/badger_hir/std/compiler.bdg) does not wrap them. LSP supports rename, while the agent query interface has no equivalent semantic edit operation. Some agent documentation also understates the debugging and profiling capabilities already present in MCP `run`.

Expose shared compiler services consistently rather than maintaining separate implementations. Keep the small MCP tool set and extend its operations. An operation should report unsupported inputs explicitly, rather than returning an empty result that resembles successful analysis.

**Acceptance:** an agent can discover the supported operations and obtain the same API, graph, diagnostic, and edit facts through the documented interfaces.

### F5. Treat the existing correctness and platform backlog as dependencies

**P0 where correctness is involved · Existing queued work.**

Claude's list includes FFI adoption, resource cleanup, cancellation/supervision, collection representation, budgets, and remaining conformance work. Those statuses can change while development continues; this assessment does not re-audit or independently reproduce every backlog item.

Framework and library enhancements should declare which of these foundations they need. In particular, extracting HTTP or SQLite must preserve lifetime checking, cancellation, diagnostics, restricted-project authority, and inspection. A convenient wrapper does not close an underlying soundness or resource-lifetime gap.

Keep bug-first priority. The enhancements here should not delay a reproduced correctness fix.

## 2. Improvements: minor language syntax, semantics, and runtime changes

### L1. Make `?` compose naturally with first-argument pipes

**P1 · Small · Best immediate language ergonomics candidate.**

Currently:

```badger
let n = (41 |> increment())?   // checks
let n = 41 |> increment()?     // fails
```

The second form currently treats `increment()?` as the pipe's callable, producing a wrong-argument-count diagnostic and then an attempted call on the extracted value. This follows the current lowering/precedence interaction; changing it is a deliberate language rule, not merely correcting a parser typo.

Propose a narrow rule:

```badger
input |> decode<Config>()? |> validate()?
```

At each stage, insert the left value into the call first, then apply `?` to that call's result. Keep the current direct error-conversion rules and nearest-function/callback boundary.

The existing AST already represents pipes, calls, and `Try`; [pipe lowering](../../crates/badger_hir/src/lower.rs) is the likely implementation point. No new runtime mechanism is needed. Specify the behavior of parenthesized function-producing expressions separately so the rule is predictable.

**Acceptance:** equivalent explicit calls and piped calls have the same results, errors, evaluation order, and cleanup; each left operand runs once. Cover multiple fallible stages and `use` callbacks.

### L2. Add expression-bodied lambdas

**P1 · Small · Parser/formatter change using the existing lambda AST.**

```badger
events
    |> seq.filter(fn(event) => event.enabled)
    |> seq.map(fn(event) => event.name)
    |> list.collect()
```

This is sugar for the existing `fn(event) { ... }` form. Keep `fn`, explicit parameters, normal capture inference, effects, ownership, and `once`; introduce no placeholder scope or implicit argument positions.

Choose one formatter convention: expression bodies for simple expressions, blocks for statement sequences. Define where the body ends inside calls, match arms, and pipelines. Named expression-bodied functions can follow if examples justify them; they are less urgent than callbacks.

**Acceptance:** short and block forms check and execute equivalently, including fallible callbacks and affine captures. Formatting round-trips nested forms.

### L3. Add `if let` and `while let`

**P1 · Small to medium · Desugaring into existing matching and loops.**

The channel example currently needs a `loop` containing a `match`, with a `.none => break` arm. Prefer:

```badger
while let .some(job) = receiver.recv() {
    process(job)?
}

if let .some(user) = users.get(id) {
    greet(user)
}
```

Evaluate the matched expression once per condition. Bind names only in the successful branch. Reuse existing pattern checking and ownership analysis; do not silently add truthiness or optional unwrapping.

For `Result`, examples should encourage explicit error handling when a nonmatching error matters. A short pattern condition must not become the recommended way to ignore failures.

**Acceptance:** receive loops retain suspension, cancellation, close, break, and continue behavior. A value moved by a successful match cannot be reused illegally.

### L4. Consider `let ... else` after pattern conditions

**P2 · Medium · Requires checking divergence and binding scope.**

```badger
let .some(user) = users.get(id) else {
    return err(AppError.not_found(id))
}
```

This puts the exceptional path first and leaves the successful binding available afterward. Require the `else` branch to leave the current control-flow path through `return`, `break`, `continue`, or a `Never` expression. It must not fall through with an uninitialized binding. [Rust's refutability rules](https://doc.rust-lang.org/book/ch19-02-refutability.html) provide a useful reference.

This is a modest extension, but less purely syntactic than expression lambdas. It should not lower through a callback that changes the meaning of `return` or `?`.

### L5. Prefer an existing bounded `use` idiom before adding syntax

**P1 documentation; P2 new syntax · Small documentation change.**

The Go/Elixir report proposes a local block form of `use`. Much of that benefit is already available:

```badger
let count = {
    use tx <- sqlite.transaction(pool)
    let count = insert_notes(tx, notes)?
    ok(count)
}?

print_summary(count)?
```

The block's result is the helper's whole result, including a commit failure. The resource scope ends before the summary runs. `return` and `?` inside the continuation still belong to its callback.

Document and format this pattern first. Only add a new block-`use` form if real programs remain materially clearer with it; retain exactly the same callback and helper-result semantics.

### L6. Keep additional syntax changes selective

**P2 · Small to medium.**

Compound assignment such as `count += 1` is reasonable sugar. Start with assignable local names; extending it to indexed places requires single evaluation of the receiver/index and correct borrowing. It must retain checked arithmetic.

Pattern alternatives can reduce duplicated error arms, but need consistent bindings, ownership, and exhaustiveness handling. They are not a parser-only addition.

Prioritize these below L1–L3. Broad extension-method search, automatic error unions, new actor keywords, and general macros would introduce substantially more design and implementation work. The current explicit call, type, and dependency rules are useful for both humans and LLMs.

### L7. Add runtime ergonomics through existing structured concurrency

**P2 · Medium per feature · Additive, but not parser work.**

Useful additions include absolute monotonic deadlines, bounded admission permits, and bounded parallel mapping. `task.timeout`, task scopes, cancellation, channels, and fallible joins already exist; these proposals should compose them rather than introduce another task model.

Nested deadlines should use the earlier deadline. Admission permits must release once on every modeled exit. Parallel mapping needs an explicit concurrency limit, ordering policy, and cancellation/error behavior. These overlap the existing backlog and framework needs, so implement them once at the lowest useful layer.

### L8. Streamline provider, system, and interceptor declarations

**P1 · Small to medium per slice · Implemented 2026-10-17 (spec chapter 06).** `provider` and `stage` declarations and direct provider application in `system { ... }` desugar to `system.provider({...})`, factories returning `pipeline.stage({...})`, and `component(...)`; the examples (`examples/10_interceptors.bdg`, `examples/11_systems.bdg`, `examples/design/service`, `examples/design/framework`) use them. The candidate syntax below is the original proposal.

User direction, 2026-10-16: the existing system/pipeline example exposes substantial plumbing. Since systems and interceptors already have compiler support, explore dedicated syntax to make them easier to declare and compose. Preserve their existing semantics while reducing descriptor records, nested callbacks, wrapper calls, repeated metadata, and repeated type annotations.

Candidate provider syntax, not currently implemented:

```badger
provider greeter {
    init(scope, config) -> Result<_, AppError> {
        ok({
            greet: fn(name: String) {
                $"{config.prefix}, {name}"
            },
        })
    }
}
```

This could lower to the existing `let greeter = system.provider({ name, init: fn(...) { ... } })` form. The normal declaration identity supplies a default diagnostic name, with an explicit override available. A provider still returns any ordinary typed interface; the syntax does not require a component base class or object model.

Candidate interceptor-factory syntax, not currently implemented:

```badger
stage logging() {
    enter(ctx) -> Result<_, AppError> {
        io.println($"Request for {ctx.name}")?
        pipeline.next(ctx)
    }

    leave(ctx, reply) -> Result<_, AppError> {
        io.println("Reply ready")?
        pipeline.leave(ctx, reply)
    }
}
```

This could lower to the existing function returning `pipeline.stage({ name, enter: fn(...), leave: fn(...) })`. Support parameterized factories, optional callbacks, and normal closure capture. Consider one shared stage error/reply annotation to avoid repeating callback annotations; its spelling and inference rules need design. Do not guess an error-conversion target when several unrelated source errors are possible.

Also evaluate shorter system entries that replace explicit `component(provider, config)` markers with direct provider application inside `system { ... }`. Such syntax must still build a deferred plan: it must not acquire resources while constructing the description. Keep dependency references explicit and checked.

Design these forms together with the extended core framework's representative applications, while recording the language rules here and framework APIs in [FRAMEWORK_DESIGN.md](../../FRAMEWORK_DESIGN.md). Prefer a few consistent declaration forms to framework-specific syntax for routes, databases, authentication, or workers. Ordinary descriptor APIs should remain usable for programmatic composition and tooling.

**Acceptance:** rewrite the combined greeter/API system-and-pipeline example with substantially less plumbing. The new forms must retain typed provider configuration/output checking, dependency ordering, partial-start cleanup, callback-local `return`/`?`, effects/capture/region checks, prefix-visible live context, separate replies, and cancellation/defect behavior. Generated names and source spans must remain useful in diagnostics, inspection, LSP, and MCP. Compare old and new forms under success, error, cancellation, and shutdown; test parser/formatter round-trips. These proposals do not require custom derives or a general macro system.

## 3. Standard library improvements: collections, data structures, and algorithms

### S1. Complete `Option` and `Result` combinators

**P1 · Small per group · Mostly ordinary language code.**

The current modules provide basic mapping, eager fallback, predicates, and `Option.ok_or`. Add a coherent set:

| Operation | Purpose |
|---|---|
| `unwrap_or_else` | Compute a fallback only when needed |
| `ok_or_else` | Construct an error only when an option is absent |
| `and_then`, `or_else` | Compose fallible or optional operations without nested matching |
| `flatten` | Remove one matching layer of nesting |
| `transpose` | Convert `Option<Result<T, E>>` to `Result<Option<T>, E>`, and its dual |

Lazy fallback matters for correctness as well as allocation: an eager default may perform I/O or fail even when it is unused. [Rust's Option API](https://doc.rust-lang.org/std/option/enum.Option.html) documents this distinction clearly.

Choose consuming versus borrowing signatures deliberately. A borrowed helper must not duplicate an affine payload; a fallback called at most once should accept the appropriate callback mode. Preserve callback effects and regions.

**Acceptance:** fallback callbacks do not run on success; captured affine values are handled soundly; errors keep their concrete types. A standalone `ok_or_else` prototype already ran with the current compiler.

### S2. Make fallible collection processing a standard idiom

**P1 · Small for eager lists; medium for scoped/lazy streams.**

Add `try_map`, `try_fold`, `try_each`, and collection of `Result` values. Keep a simple default: evaluate in source order and stop at the first error.

```badger
let orders = list.try_map(rows, decode_order)?
let total = list.try_fold(orders, 0, add_total)?
```

An eager `try_map` can already be written using a builder, `for`, and `?`; a prototype ran successfully on both success and error inputs. This does not need a new comprehension or error-propagation mechanism.

Handle a separate validation use case explicitly: an operation that collects every validation error is different from fail-fast mapping. Give it a distinct name and result type.

For sequences/streams, define what happens to upstream resources on the first error, early break, and cancellation. Do not silently discard malformed items as a form of error handling.

### S3. Fill everyday sequence and map gaps

**P1 · Small per operation · Use efficient shared implementations.**

Recommended sequence operations: `filter_map`, `find`, `find_map`, `any`, `all`, `enumerate`, `skip`, `take_while`, `scan`, `min_by`, and `max_by`. Add corresponding stream operations where suspension and resource ownership have clear contracts.

Recommended map/set operations: map iteration, keys/values, update-with-callback, merge-with-collision-function, and set union/intersection/difference. Define missing-key behavior explicitly. A map update callback should receive an `Option<V>` rather than an invented null/default value.

Provide stable sorting with key/comparator functions, grouping, partitioning, and deduplication. State ordering and equality requirements. Review the current `group_by` implementation's repeated persistent appends and use builders where measurements justify them.

Keep the API regular: collection/input first for piping; similar callback positions and names; explicit eager versus lazy behavior; predictable module functions and declared methods. Avoid near-identical operations with unrelated conventions.

### S4. Add data structures that service programs actually need

**P2 · Medium per collection · Prioritize from working applications.**

| Structure | Useful workloads | Contract to publish |
|---|---|---|
| Queue/deque | Local work queues, breadth-first traversal, rolling windows | End operations, amortized costs, ownership |
| Binary heap/priority queue | Timers, ranked work, top-k | Comparator, equal-priority ordering, mutable versus persistent form |
| Ordered map/set | Stable output, range queries, deterministic tests | Key ordering and iteration stability |
| Dense bit set | Flags, membership over compact integer domains | Domain/range limits and storage growth |
| Bounded LRU cache | Application caches | Capacity, eviction policy, explicit synchronization wrapper |

Separate a task-confined mutable collection from a shareable synchronized service. A deque should not implicitly become a concurrent queue. Persistent versions are worthwhile where snapshotting matters, rather than mandatory for every structure.

Implement in the language where practical. Retain native kernels when representation access or measured performance warrants them, consistent with the existing runtime/library direction. Check that third-party packages can implement a comparable structure without adding a runtime intrinsic for every operation.

### S5. Improve collection construction and cost visibility

**P1 · Small to medium.**

Add convenient builder helpers, capacity-aware bulk operations, and clear conversion functions. A scoped `list.build` or `map.build` helper could finish the builder automatically while ordinary borrow checking prevents it from escaping improperly.

Document time complexity, copying, retained backing storage, and ordering for each public operation. Explain when to use `List`, `Array`, `Buffer`, persistent maps, builders, `Seq`, and `Stream`. Avoid examples that repeatedly append to immutable snapshots when a builder is the intended path.

Expand `std.text` and `std.bytes` with common service needs such as checked splitting, joining, searching, and multiline dedenting. Reuse maintained implementations for demanding Unicode/protocol behavior. Format parsers and database clients belong in the appropriate extended packages.

**Acceptance:** representative transformations have clear memory bounds, affine cases remain sound, and benchmarks compare whole operations rather than isolated favorable primitives.

## 4. Framework: a cohesive application-service stack

**Accepted direction, 2026-10-16:** the application framework will be an **extended core library** that makes applications and services easier to create, building on systems/components and interceptors. It is intended to be the blessed application/service programming model.

**Accepted first-release scope:** HTTP APIs, workers/durable jobs, and CLI entry points, with HTML and administration added later. The framework draft also selects browser forms/session facilities for the later web layer.

[FRAMEWORK_DESIGN.md](../../FRAMEWORK_DESIGN.md) is the current authority for the framework proposal. It develops the eight requested inspirations into selected architecture/API defaults, operation and module contracts, middleware, configuration, lifecycle/supervision, transport/data/durability rules, tooling, and runnable delivery slices. Detailed framework decisions belong there so this assessment stays focused on overall ergonomics.

The shared ergonomic requirements are:

- Small typed feature interfaces, explicit injected dependencies, fake implementations, and independent tests.
- Inspectable application plans with checked configuration, dependency edges, operations, middleware order, health, and worker policy.
- Preserved error, cancellation, ownership, region, and cleanup contracts across framework wrappers and adapters.
- Compiler-backed scaffolding and queries that let people and LLMs work on one feature with bounded context.
- A consistent ordinary-code path that works before the provider/stage syntax proposed in L8 is implemented.

The [current-language baseline](examples/design/framework/README.md) runs a modular order service using a fake store, providers, and interceptors. Its seven tests cover focused domain/composition/lifecycle cases. It does not implement the proposed extended framework or its production integrations.

## 5. Tooling: compiler, tests, analysis, LSP, linting, formatting, and generators

### T1. Make the compiler easy to query by meaning

**P1 · Small to medium for initial queries; richer analysis later.**

Existing tooling already supplies checks, API queries, hover, definitions, references, completions, graphs, execution, a REPL, debugging, and profiling. Extend those capabilities rather than proposing another generic compiler chat tool.

Add symbol-addressed queries alongside line/column queries. An agent should be able to ask for `orders.service.create` even after surrounding lines change. Return a snapshot-scoped symbol identity, qualified name, definition location, API, docs, and relevant callers/dependencies.

High-value questions include:

- What is the expected type at this expression or argument?
- Which fields, variants, methods, or direct error conversions are legal here?
- Why does this value fail `Transfer`, or escape its resource scope?
- Which stage establishes or requires this pipeline field?
- Which provider/reference edge causes this system error?
- Which public consumers may need changes after this interface edit?

Start with facts the compiler already has. Type-directed API search can initially match indexed signatures; general proof search or program synthesis is unnecessary.

### T2. Provide compact, versioned context and structured MCP results

**P1 · Small for result structure; medium for context bundles.**

Use MCP `structuredContent` and declared `outputSchema`, with a concise text rendering for readers and compatible clients. These facilities are already part of the [MCP version advertised by the server](https://modelcontextprotocol.io/specification/2025-06-18/server/tools#structured-content).

Add a bounded context query: given a symbol or diagnostic, return its definition, relevant public dependency APIs, nearby usages/tests, expected types, effects/contracts, and related locations. Offer field filters, limits, and pagination instead of dumping all source code.

Identify the compiler/toolchain, schema version, source revision/digests, and standard-library version used. Distinguish current and last-successful analysis when serving incomplete code. Do not present stale completion information as freshly checked evidence.

Expose batch queries over one analysis snapshot. The current `std.compiler` wrappers invoke subprocess queries individually; batching can avoid repeated full analysis. A simple batch with a full check is useful before implementing incremental caches.

**Acceptance:** an agent can obtain the context for a signature change in one bounded request, and every answer identifies the source snapshot it describes.

### T3. Add checked code actions and refactoring

**P1 · Medium, starting with a few reliable actions.**

Useful initial actions: insert an unambiguous import, qualify a symbol, fill match arms, materialize an inferred public signature, generate protocol method stubs, and expose existing semantic rename through agent tooling. [rust-analyzer's assists](https://rust-analyzer.github.io/book/assists.html) demonstrate the value of compiler-backed edit operations.

Return a proposed edit with source-hash preconditions, affected symbols, and the check result. Keep applying the edit separate from calculating it. For ambiguous fixes, show alternatives instead of guessing.

Unimplemented generated arms/stubs should remain clearly incomplete and fail the completion gate. Do not make a project appear finished by generating broad wildcard matches, fabricated default return values, or silent error discards.

Extend actions later to extract a function/module and update imports, using the same semantic services as the LSP. API-diff tooling can identify changed parameter, result, error, effect, ownership, or configuration contracts and their consumers.

### T4. Make partial programs and repair loops useful

**P2 · Medium · Keep incomplete-code tooling distinct from accepted executable programs.**

Support asking for expected types and available names while an expression is unfinished. Typed holes or editor-only completion markers could report the required type, effects, accessible values, and legal constructors.

Do not make an unresolved hole into a deployable default. Checking should report incompleteness distinctly, and compile/run should reject it. Supporting useful partial analysis can begin in overlays/LSP without committing to new permanent source syntax.

Diagnostics should group primary failures and dependent cascades, retaining all evidence while making the first repair clear. A failed import should not force an LLM to independently interpret dozens of downstream unknown names.

### T5. Improve the test runner's verification output

**P1 · Small to medium per feature.**

Add or expose consistent test listing, exact test selection, module/package selection, structured results, per-test timeouts, and reproduction commands. Return assertion values, source locations, failure kind, schedule/random seeds, and any retained replay artifacts as fields.

Connect the existing property tests, controlled clocks, scheduling exploration, record/replay, and profiling to the same machine interface. Distinguish an ordinary test failure, a task defect, cancellation, timeout, and test infrastructure failure.

Add affected-test suggestions using module/call graphs, with an explanation of coverage limits. Call graphs cannot establish complete behavioral impact for indirect callbacks, external data, and dynamically chosen providers. Keep a full-suite mode and use it for final validation where appropriate.

Framework generators should produce contract and lifecycle tests, not only handler success tests. Checked documentation examples should also serve as retrievable examples for agents.

### T6. Add focused linting and keep formatting canonical

**P1 · Small per lint; medium for architecture rules.**

Useful lints include accidentally broad effects in service interfaces, unbounded collection of a stream, obvious repeated immutable appends in a construction loop, overly broad wildcard matches at evolving API boundaries, and suspicious detached resource/worker patterns where the compiler can establish them.

Offer opt-in architecture rules over the existing import graph: domain modules must not import transport/database adapters; consumers use a module's public API; composition-root modules own provider wiring. Make exceptions explicit. These are project architecture policies, distinct from type soundness and the existing restricted-project authority rules.

Only hard-error on claims the analysis can justify. Performance and concurrency heuristics should explain uncertainty and avoid pretending to prove arbitrary deadlock freedom.

The formatter should implement each accepted syntax addition, preserve comments, and be idempotent. Supply one canonical style in docs, generated code, and fixes. Avoid formatting churn outside the edited region when applying an assist.

### T7. Generate working framework modules and endpoints

**P1 · Medium for a useful initial generator.**

Proposed commands, not existing CLI features:

```text
lang new service orders-api
badger generate module orders
badger generate endpoint orders.create
badger generate interceptor request_metrics
badger generate worker import_jobs
```

Generate ordinary editable language code, using compiler-checked templates. An endpoint generator should connect its input/output schemas, named errors, handler, route, application composition, and tests. An interceptor generator should show its required/established fields and unwind behavior. A worker generator should include ownership, restart, shutdown, and queue policy decisions.

Prefer explicit required choices to silently inventing business rules. For example, generate authorization hooks and tests without deciding who is authorized. Show a preview and check proposed files through overlays before writing. Existing-file integration should use semantic edits or small explicit patches; do not rely on brittle string insertion.

Keep generated code and templates aligned with the framework version. Support checking whether a generated feature follows the current convention without requiring regeneration that overwrites user code.

### T8. Make language discovery and examples authoritative

**P1 · Small to medium · Move a useful subset forward from final documentation polish.**

Offer a concise versioned language guide through the tool interface: name casing, imports, records, errors, pipelines, `use` boundaries, effects, ownership, tasks, and the normal framework module pattern. Link each rule to checked positive examples and focused counterexamples.

Generate compact API documentation and a searchable example index from checked code. Include recipes for adding an endpoint, fake dependency, transaction, worker, and interceptor. This is more immediately useful to an LLM than exposing the entire runtime implementation as API documentation.

The backlog's final tutorial/reference/`llms.txt` work remains useful. Bring forward enough of it to support ongoing implementation, and derive it from the same current sources. Avoid parallel hand-maintained interface descriptions that can drift.

### T9. Measure whether the changes actually help LLMs

**P1 · Small initial evaluation suite; expand with use.**

Retain a fixed set of tasks: add a typed endpoint, change a public record, handle a new error variant, implement fallible collection processing, diagnose a transfer/scope error, replace a provider in a test, and add a supervised worker.

Compare the existing tooling with each enhancement using the same model/configuration and repeated runs. Measure first-pass parse/type success, final behavioral-test success, repair iterations, tool calls, context/output volume, elapsed time, and unnecessary edits. Keep withheld acceptance tests for the intended behavior. Shorter code alone is not proof of better results.

Tests and profiles are evidence, not universal invariants. They must not authorize optimizer assumptions or correctness claims that the compiler has not independently justified.

Recommended first sequence across the five topics:

1. Preserve full structured diagnostics, reconcile stale docs, and expose project-overlay checking.
2. Improve fallible pipes; add expression lambdas and pattern conditions.
3. Add lazy `Option`/`Result` helpers and eager fallible collection operations.
4. Ship a checked modular-service starter using existing systems/pipelines, explicit interfaces, and fake dependencies.
5. Add compact symbol/context queries, reliable code actions, and framework generators.
6. Grow the framework facilities and data-structure library through working applications and measured demand.

Validation of this assessment: inspected source and ran small probes using the existing release executable. The fallible-pipe limitation, missing short-lambda syntax, and missing `while let` syntax were reproduced; the parenthesized pipe checked; ordinary-language lazy fallback and fallible mapping, bounded `use`, and multi-file dependency injection ran successfully. The executable was not rebuilt against the concurrently changing checkout, and no full compiler/runtime audit or LLM evaluation was performed. No proposed language, library, framework, or tooling enhancement was implemented by this document.
