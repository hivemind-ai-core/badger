# Attributes

**Status: proposed, not implemented.** This document scopes the design. It is
the place to record decisions and open questions; it changes no grammar,
checker, or codegen yet. When a subset is implemented it should move into
`spec/` as a numbered section.

## Why

The compiler already reads several kinds of source-level fact — `///` docs,
`derive(...)`, `extern "intrinsic"`, and (implicitly) every standard-library
item’s module path. What is missing is a general, *declarative* way for a
library to hand the compiler structured metadata about an item, so that
compiler behaviour is driven by declarations instead of by hard-coded
function names.

Attributes are that channel. The first consumer is **fusion templates**
(see "Application: fusion templates"), which is what motivates a minimal
implementation now. Later consumers include inlining hints, `compile-time
only` (blocked on compile-time evaluation, which does not exist yet),
`expect taken`, `must use`, and, at a later date, macro-generated and
runtime-visible metadata.

## Syntax (tentative)

`@name`, `@name(arg)`, `@name(key = value)`, and a comma-separated mix:
`@name(a, b, key = c)`. Tentative forms:

```text
@inline(always)
@inline(never)
@fusion(role = stage, template = one_to_one)
@must_use
```

`@` is currently free in the lexer (symbols use `#`, interpolation `$`, the
pipe operator `|>`, effects `!`, borrows `&`), so no token conflict exists.
An attribute begins with `@word` and an optional parenthesised argument
list. Arguments should be restricted at first to identifiers, symbols, and
string/int literals; expression arguments would require compile-time
evaluation.

Where attributes may appear is **open** (see "Open questions"). The first
implementation only needs them on function items.

## Semantics, by phase

**Phase 1 — compiler-read metadata (this scope).**

- The set of recognised attributes is fixed in the compiler, like
  `derive(...)` today (a closed set with a designed error for the rest).
- Attributes carry no runtime representation and do not change type
  checking except where a consumer (the fusion pass, the inliner) reads
  them.
- Policy for an unknown attribute is open: error everywhere (catches typos,
  blocks forward compatibility) versus error in standard-library sources and
  warn in user code (allows future/user attributes). A known attribute in a
  wrong position is always an error.
- An attribute is part of an item’s **interface**: it is recorded in
  interfaces and visible to `badger query api`, so changing one is an
  interface change and the generated reference must account for it.

**Phase 2 — macro-generated metadata (later).** Once macros exist, they
will emit attributes (and possibly items annotated with them); the compiler
already reading a fixed set is the prerequisite. No design work here yet.

**Phase 3 — compile-time-recorded, runtime-visible metadata (later).** The
attribute values are fixed at compile time today; making some readable at
runtime (reflection over a type or item) is a separate feature and is out of
scope.

## Application: fusion templates

Today the fusion passes recognise standard-library functions **by def
path** (`std.seq.map`, `std.list.filter`, `std.map.entries`,
`list.filter_then_map`, …) and each new fusible function needs a new
compiler arm. The intent is the reverse: the compiler implements a small,
closed set of **templates** (kinds), and a library function *declares* which
template it is.

Templates the compiler implements once:

- **source**: `counted_range`, `list_view`, `trie_walk`, `stored_seq`, …
- **stage** (by element flow): `one_to_one`, `predicate` (zero-or-one),
  `option_map` (zero-or-one with transform), `flat_map` (one-to-many),
  `counter_limit`, `counter_drop`, `counter_step`, `buffer_n` (windows /
  chunks), `dedup_adjacent`, `enumerate`, `interpose`, `zip`.
- **terminal**: `fold`, `collect`, `count`, `sum`, `last`, `first`,
  `find`, `any`, `all`, `each`, `for_body`.

A declaration names only the role and the template; everything the compiler
otherwise needs is derivable from the call’s **types**:

| Fact | Source |
|---|---|
| pipeline receiver | the parameter of `Seq<T, Eff>` type |
| callback | the parameter of function type |
| counter / value argument | the `Int` or value parameter |
| element transform `T → U` | the callback’s return type |
| flow kind | the callback’s return type (`U` / `Bool` / `Option<U>` / `Seq<U>`) |
| accumulator | the terminal’s parameter/return type |

So a new adapter that reuses an existing template is **an annotation only**;
only a genuinely new *flow* needs a new template implementation in the
compiler. Example (the template ids are illustrative):

```text
@fusion(role = stage, template = one_to_one)
pub fn map<T, U, Eff>(items: Seq<T, Eff>, f: fn(T) -> U !Eff) -> Seq<U, Eff> { … }

@fusion(role = terminal, template = collect)
pub fn collect<T, Eff>(items: Seq<T, Eff>) -> List<T> { … }
```

**What attributes do not remove.** Two things stay in the compiler:

1. **Legality.** Reordering a stage past another requires the earlier
   callback to be total and effect-free (no calls, allocation, mutation,
   indexing, division, checked arithmetic, or shift — the existing
   `callback_pure_infallible` analysis). A template may *require* that
   property, but proving it is an analysis, not metadata.
2. **The template implementations.** The compiler still emits the loop for
   each flow kind.

**Relationship to a plain registry.** An attribute-driven table and a
hard-coded table hold identical data. The difference is where it lives:
attributes put it in the source (next to the function, visible to docs and
tooling, later extensible to user libraries and macro generation); a
registry puts it in the compiler. Recommended order: implement the templates
behind a registry first (no surface change), then re-point the registry at
attributes once the surface exists. The data is the same, so the second step
is mechanical.

## Relationship to protocols and `derive`

The compiler already changes behaviour by dispatching on **protocol impls**
it finds for a type:

- `From`: `?` converts the error (`Res::Try(Option<Target>)`; the checker
  pushes `Constraint::ErrConv`), and `Target.from(value)` calls it;
- `Display`: string interpolation;
- `Error`: the `main` error report and `cause` chains;
- `Eq`/`Hash`: map keys and set membership;
- `Codec`: schemas; `Share`/`Transfer`/`Scoped`: capabilities.

`derive(P)` is **not** a metadata channel. It is a *synthesis directive*:
the compiler generates an impl of `P` for a type from the type’s shape
(validated field-wise by `underivable`), and the generated impl then
participates in resolution exactly like a hand-written one. So there are
three mechanisms with distinct jobs:

| Mechanism | Carries | How the compiler uses it | Example |
|---|---|---|---|
| protocol + impl | methods and types | resolution, dispatch, `dyn`, coherence | `From` for `?`; `Display` |
| `derive(P)` | a synthesis request | generate an impl from a type’s shape | `derive(Eq)`, `derive(From)`, `derive(Error)` |
| attribute (proposed) | non-semantic facts (a name and literal arguments) | read by a specific pass; no methods or types | `@fusion(stage, map)`, `@inline(always)` |

**Decision: keep all three; unify the surface, not the semantics.**
`derive(...)` may be re-expressed as one recognised attribute
(e.g. `@derive(Eq)`), parsed by the same grammar and recorded in the same
registry, with two kinds of consumer — **generators** that produce impls
(the derive set) and **metadata** consumers that a pass reads (fusion,
inlining). Keep `derive`’s generator semantics and its field-wise
validation; do **not** try to express `From`/`Display`/`Hash` as
attributes: they are typed, resolvable, usable with `dyn`, and
implementable by hand, none of which attributes provide.

A useful side effect of unifying: `derive` is currently matched by
**protocol name string** (`env.rs:845-943`), so a user protocol named
`Display` would take the `Display` branch. Resolving the attribute to the
standard-library protocol’s def id fixes that.

**Decision rule.** If the fact must be type-checked or resolved (a method,
a type, `dyn`), it belongs in a protocol. If it is synthesised from a
type’s shape, it belongs in `derive`. If it is a hint or a role that a
specific pass reads, it belongs in an attribute.

## Minimal set to implement, if we do it now

1. **Grammar/lexer**: `Tok::At`; parse `@word` with an optional
   `(` args `)`; attach to `fn` items.
2. **Metadata**: store the parsed attributes on the item’s definition;
   expose them through `badger query api`.
3. **Checker**: a fixed recognised set with a designed diagnostic for
   unknown/misplaced attributes.
4. **Consumers**: only the fusion pass initially (`@fusion(...)`), plus
   `@inline(always|never)` if it proves trivial.
5. **Docs**: decide whether `badger doc std` prints attributes; keep
   `api_reference_current` green either way.

## Open questions

- **Placement**: functions only, or also types, `let`/`var` bindings,
  expressions, modules? (Nothing here depends on the answer for functions.)
- **Unknown-attribute policy** (error everywhere vs warn in user code).
- **Interface stability**: are attributes part of public compatibility?
- **Naming**: one `@fusion(role, template)` versus `@source(…)` /
  `@stage(…)` / `@terminal(…)`.
- **Arguments**: identifiers/symbols/literals only, or paths and
  expressions (needs compile-time evaluation)?
- **Tooling**: formatter preservation, LSP hover/completion, editor
  grammar.
- **Docs**: whether and how attributes appear in the generated reference.

## Non-goals (for now)

- Compile-time function evaluation (blocks `compile-time only`).
- Macros and macro-generated metadata.
- Runtime reflection over attributes.
