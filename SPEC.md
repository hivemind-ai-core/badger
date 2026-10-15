# Language specification, draft 0

This is the proposed complete language design for review and implementation.
The language is temporarily called `lang`; source files use `.lang`. It is a
native application language for services and automation.

The user's accepted decisions are authoritative. Additional decisions below
are the selected defaults of this draft, not claims of subsequent user approval.
They resolve the earlier alternatives into one coherent design. The compiler
and runtime in this repository implement this draft and run the example
applications; [ARCHITECTURE.md](ARCHITECTURE.md) describes the implementation
and its scope notes.

## Accepted constraints

- All named types start uppercase, including primitives. Initial uppercase is
  the type role; ordinary values, fields, functions, and variants start lowercase.
- Inferred statically typed records are the everyday data model, with efficient
  native representations rather than universal dynamic maps.
- Lifecycle-managed application descriptions take Integrant as their reference;
  interceptors are central to operation composition.
- Sequential execution within each task, concurrent tasks throughout the
  application, and optional multicore execution underneath.
- Native compile-and-run through Cranelift; a substantial runtime is acceptable.
- Learn from Clojure ergonomics without requiring Lisp syntax or semantics.

## Selected draft defaults

| Area | Decision |
|---|---|
| Surface | Braces, newline-separated statements, `fn`, `let`, `var`, `match`, first-argument pipelines |
| Ordinary data | Structural immutable records; named aliases; opt-in nominal structs and enums |
| Inference | Rank-1 polymorphism plus record/effect rows; explicit public value types |
| Errors | `Result<T, E>`, `?`, coherent direct `From` conversions; no implicit general unions |
| Effects | Inferred summaries; ordinary functions can suspend; no `async` function category |
| Memory | Managed immutable values; affine builders; task-confined mutable cells; inferred scoped borrows |
| Initial collector | Precise nonmoving tracing heap; explicit scopes own external resources |
| Collections | Paged frozen `List<T>` and unique `ListBuilder<T>`; contiguous `Buffer<T>` and `Array<T>` |
| Communication | Typed bounded `Channel<T>` and explicitly buffered `BatchChannel<T>` |
| Application graphs | Typed `system { ... }` descriptions, compiler-assisted reference checking, blessed lifecycle runtime |
| Processing | Typed `pipeline { ... }` descriptions; forward context changes and prefix-limited live views during unwind |
| Recovery | Ordinary task scopes; opt-in managed worker restart policy |
| First execution strategy | Stackful native tasks and compiler safe points; one worker first, multicore later |
| Reflection | Derived typed schemas and explicit `Data` boundaries; no general macros in draft 0 |

## Reading order

1. [Syntax and control flow](spec/01-syntax.md)
2. [Types and records](spec/02-types-and-records.md)
3. [Functions, effects, errors, and protocols](spec/03-functions-and-errors.md)
4. [Memory, ownership, and collections](spec/04-memory-and-collections.md)
5. [Tasks, channels, and supervision](spec/05-tasks-and-channels.md)
6. [Systems, resources, and interceptors](spec/06-systems-and-pipelines.md)
7. [Standard library, tooling, and implementation](spec/07-runtime-and-tooling.md)
8. [Worked programs and semantic acceptance cases](spec/08-worked-examples.md)

[Reference EBNF](spec/grammar.ebnf) accompanies the prose syntax rules. It is
not a replacement for the currently implemented placeholder grammar.

[DESIGN.md](DESIGN.md) retains the earlier reasoning and alternatives. When it
conflicts with this draft, this draft states the current proposal. The original
[research notes](researxh/README.md) are reference material, not language rules.

The crucial implementation experiments are row inference, same-layout record
construction, scope escape checking, typed reverse pipeline execution, precise
roots on suspended native stacks, and cancellation-safe page publication.
Allocator sizes, scheduling budgets, and tuning constants require measurement;
they are not additional user-facing language semantics.
