# Attributes

**Status: Phase 1 implemented.** The syntax (lexer, grammar, CST, AST, HIR
storage) and the unknown-attribute policy are in place. Attributes may be
written on declarations, methods, local functions, struct fields, enum
variants, parameters, and statements (see
[Placement, namespaces, and visibility](#placement-namespaces-and-visibility));
a library's attributes are namespaced by its module (`@lib.name`).
The old `derive(...)` form has since been removed (Phase 3): `@derive(...)` is
the only spelling. The design and open questions below still apply.

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
list. Arguments are identifiers, symbols, string/int literals, and
**resolvable item/type paths** (needed by `@derive(Eq)`); general
expression arguments would require compile-time evaluation and are out of
scope.

A name may be qualified, `@lib.name(...)`: see the next section.

## Placement, namespaces, and visibility

**Where an attribute may be written** (each on the lines before what it
tags, or on the same line):

| Position | Example |
|---|---|
| a top-level declaration (`fn`, `struct`, `enum`, `type`, `protocol`, `impl`, `let`, `import`) | `@derive(Eq) pub struct Point { ... }` |
| an `impl` method | `@allow(E0197) pub fn norm(self: &Self) -> Int { ... }` |
| a local (nested) function | `@allow(E0196) fn helper() -> Int { ... }` |
| a struct field | `@flatten pub id: BookId` (where it applies), `@app.column("px") pub x: Int` |
| an enum variant | `@status(404) @message("no book {id}") not_found(id: Int)` |
| a parameter (functions, methods, lambdas) | `fn f(@allow(E0197) x: Int)` |
| a statement (a block written as a statement included) | `@allow(E0197) let total = 0` |

Expressions (calls included) take no attributes: a caller-side hint such as
"inline this call" is not part of the language.

**Namespaces.** A name without a namespace (`@allow`, `@derive`) is the
compiler's: the set is closed, so an unknown one is an error (E0191) — a
typo, or a library's attribute written without its namespace — and a known
one written where it does not apply is an error (E0199). A library's
attribute is namespaced by its module, `@lib.name` (`@app.operation`, after
`import std.app`): the first segment must be a module the file imports (an
error, E0191, otherwise), which also counts as a use of the import. One the
module does not declare is a warning in user code (an error in
standard-library sources), so a library newer or older than its user does
not break the build; modules cannot declare attributes yet, so until
declared attributes (below) every attribute of an imported module is
accepted, and nothing checks its arguments.

**`@allow(CODE, ...)`** works in every position: it silences that warning
within what it tags (an item, a method, a field, a variant, a parameter's
pattern, a statement), in `check`, `run`, and `test` alike.

**Who can see an attribute.** Attributes live on the syntax tree at compile
time and are not carried through values: an attribute can be read where the
tagged node itself is visible, never through a variable that happens to hold
the tagged thing.

| Reader | Sees | Not |
|---|---|---|
| The compiler | every position above (whether a pass acts on one depends on the pass) | — |
| Macros (when they exist) | top-level functions and types by name; statements, blocks, and local functions inside the body passed to the macro | `impl` methods (the macro cannot see which impl a value has); a function stored in a variable (it could change at run time) |
| Run time (user-defined attributes, later) | top-level declarations, enum variants, and struct fields, compiled into a static table on the type's descriptor (as `@message` already is) | methods, statements, parameters, local functions |

Run-time-readable, user-declared attributes are planned after 1.0 (next
section); the namespacing rule above is what keeps them from colliding with
the compiler's own names.

## User-defined attributes (design, agreed 2026-10-20; not implemented)

A library declares an attribute; other modules annotate with it; only the
declaring module reads it, at run time, as the list of every use in the
program — typically from an `init` function (a service's start-up) that
builds a route table, a schema, or a registry from it.

**Declaration.** A new top-level definition, fields as in a struct, after
`on` the positions it may be written at:

```text
pub attribute route on fn(http.Request) -> Result<http.Response, E> {
    method: Method,            // an enum: a bare variant at the use site
    path: String,
    summary: Option<String>,   // an `Option` field may be left out
}

pub attribute column on field {
    name: String,
    primary: Option<Bool>,
}
```

- **Positions**: `type`, `fn`, `field`, `variant`, `value` (a module-level
  `let`) — the positions visible at run time (see the table above).
- **Field types**: values the compiler can build statically: `Bool`, the
  integer and float types, `String`, `Symbol`, `Duration`, payload-free enum
  variants, and `Option`/`List` of these. No functions, maps, or `Data`.
- **`pub`** lets other modules annotate with it; reading stays private to
  the declaring module. The declaring module does not annotate with its own
  attribute (it does not import itself, so `@lib.name` cannot name it).
- **One use per item** unless declared `repeatable` (a function serving two
  routes).
- A name of the compiler's (`allow`, `derive`, ...) cannot be declared.

**Use.** Namespaced by the declaring module, with `key = value` arguments
(one positional argument when the attribute has exactly one required
field):

```text
@web.route(method = get, path = "/books/:id")
pub fn show(request: http.Request) -> Result<http.Response, AppError> { ... }

@db.table("books")
pub type Book = { @db.column(name = "id", primary = true) id: Int, title: String }
```

An argument is checked against the field's type; a missing non-`Option`
field, an unknown key, or a position the attribute does not list is an
error. Once declarations exist, an attribute its module does not declare is
a warning in user code (an error in standard-library sources).

**Reading.** In the declaring module, the attribute's name is a value: the
uses in the whole program (dependencies included), in a deterministic order
(module path, then source order):

```text
for use in route.uses {
    use.value    // { method: Method, path: String, summary: Option<String> }
    use.target   // { module: String, name: String, kind: Symbol,
                 //   owner: Option<String>, file: String, line: Int }
    use.call     // fn(http.Request) -> Result<http.Response, E>: the function
}
```

**An entry carries the annotated item**, not only its name — otherwise
annotating a particular function or type has no point:

- `on fn(SIGNATURE)`: each use carries the function as a value of that type
  (`use.call`); annotating a function of another signature is an error.
- `on type`: each use carries the type's description (its schema, as
  `schema.of<T>()` gives) and, when the declaration names a protocol
  (`on type: Codec`), what that protocol offers for it.
- `on field` / `on variant`: the owning type (as for `type`) and the
  field's or variant's name and type.

**Implementation notes.** The table is static: it is built when the program
is lowered, stored with the type descriptors (where `@message` already
lives) and in saved MIR, and must keep what it references alive — the
reachability pass may not prune a function or type that only an attribute
table reaches.

## Semantics, by phase

**Phase 1 — compiler-read metadata (this scope).**

- The set of recognised attributes is fixed in the compiler, like
  `derive(...)` today (a closed set with a designed error for the rest).
- Attributes carry no runtime representation and do not change type
  checking except where a consumer (the fusion pass, the inliner) reads
  them.
- An unknown attribute without a namespace is an error everywhere; a
  library's (`@lib.name`) attribute its module does not declare is a
  warning in user code and an error in standard-library sources; a
  namespace that is not an imported module is an error. A known attribute
  in a wrong position is always an error.
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
| attribute (proposed) | a name and arguments (literals or resolvable item/type paths) | read by a specific consumer; no methods or types of its own | `@fusion(stage, map)`, `@derive(From)`, `@inline(always)` |

**Framing: attributes are the *trigger* channel; consumers are the
*implementation*.** A `derive` is nothing more than "an AST node the
compiler reacts to"; an attribute is "metadata the compiler reacts to". So
the trigger is unified and the behaviour is unchanged: `derive(From)`
becomes `@derive(From)`, an ordinary attribute whose *consumer* happens to
be the impl-synthesis generator. Fusion is the same shape: `@fusion(...)`
is an attribute whose consumer is the fusion pass. "Attribute" and "derive"
are then a *property of the consumer*, not of the syntax.

Two consequences make this more than cosmetic:

- **Placement must include item position** (`fn`, `struct`, `enum`, …),
  because `derive` attaches to type declarations. Placement is therefore
  load-bearing for this unification, not merely a future question.
- **Attribute arguments must include resolvable item/type paths**, because
  `@derive(...)` names protocols (and today checks that they are protocols,
  `E0117`). Literal-only arguments are too narrow for it.

Keep each consumer's semantics: `derive`’s field-wise validation
(`underivable`) and its exemption from orphan/overlap rules are unchanged;
do **not** try to express `From`/`Display`/`Hash` *themselves* as
attributes: they are typed, resolvable, usable with `dyn`, and
implementable by hand, none of which attributes provide.

### Consumers (attribute name → consumer class)

The compiler’s recognised set becomes a table of name → consumer, which is
itself the "reduce per-function knowledge" goal in another guise — a new
recognised attribute is a row, and the consumer does the work.

| Attribute | Consumer | Class |
|---|---|---|
| `@derive(P, …)` | impl synthesis from the type’s shape | generator |
| `@flatten` | codecs and schemas: a one-field struct is its field (done) | metadata |
| `@status(code)` (variant) | `derive(HttpError)`: the variant's HTTP status (done) | metadata |
| `@message("text {field}")` (variant) | `derive(Error)` / `derive(Display)`: the variant's display (done) | metadata |
| `@fusion(role, template)` | sequence/eager fusion templates | metadata |
| `@inline(always\|never)` | inliner | metadata |
| `@must_use` | unused-value diagnostic | metadata |
| `@expect_taken` | branch-probability hint | metadata |
| `@compile_time_only` | compile-time evaluation | blocked (no CT eval) |

Migration (Phase 3, done): every `derive(...)` clause was converted to
`@derive(...)` and the old clause was deleted from the grammar, so the CST no
longer carries a `derives` clause; the AST field (`StructDecl::derives` /
`EnumDecl::derives`) is populated solely from the attribute.

A useful side effect of unifying: `derive` is currently matched by
**protocol name string** (`env.rs:845-943`), so a user protocol named
`Display` would take the `Display` branch. Resolving the attribute to the
standard-library protocol’s def id fixes that.

**Decision rule.** If the fact must be type-checked or resolved (a method,
a type, `dyn`), it belongs in a protocol. If it is synthesised from a
type’s shape, it belongs in `derive`. If it is a hint or a role that a
specific pass reads, it belongs in an attribute.

## Implementation status

**Phase 1 (done, `feature/attributes`).**

1. **Grammar/lexer** — `Tok::At` (`token.rs`); parses
   `@name` / `@name(a, b)` / `@name(key = value)` with word, symbol,
   integer, and string arguments (`grammar.rs`), on **item position**;
   carried through `cst::Item` and `ast::Item`.
2. **Metadata** — stored per definition in `Hir::attributes`
   (`FxHashMap<DefId, Vec<ast::Attribute>>`); accessible to passes.
   (Not yet surfaced by `badger query api`.)
3. **Policy** — `KNOWN_ATTRIBUTES` in `badger_hir/src/lower.rs` (empty in
   Phase 1) with `E0191`: an unrecognised attribute is an **error**
   (`tests/reject/unknown_attribute.bdg`).
4. **Consumers** — none yet; the first built-in attributes are Phase 2
   (`@derive`, the scoped warning-silencing attribute, `@inline`, …).
5. **Docs** — `badger doc std` does not print attributes yet; the
   generated reference and `doc_examples_check` are unchanged and green.

**Phase 2 (in progress, `feature/attributes-phase2`).**

- `@derive(A, B)` — **done**. The names feed the same derive path as
  `derive(A, B)` (resolution and validation shared), so `@derive(Eq,
  Display)` behaves exactly like the old form. `tests/run/attributes_derive.bdg`
  and `tests/reject/derive_attribute_nonprotocol.bdg` cover it.
- `@allow(<code>, ...)` — **done (every position)**. During lowering each
  `@allow` records a suppression region (what it tags) in `Hir::allows`;
  `suppress_allowed` drops any *warning* whose code matches and whose
  primary span lies inside a region (errors are never suppressed), when a
  check finishes and when a program is compiled to run or test.
  `crates/badger_cli/tests/programs.rs::attribute_allow_silences_warnings`
  and `attributes_in_every_position` cover it.
- **Phase 3** — **done**: every source migrated to `@derive(...)` and the old
  clause removed from the grammar (see the migration plan).
- Methods', fields', variants' (`VariantDef::attributes`, `FieldDef::attributes`,
  `Hir::attributes` for methods) and statements' and parameters'
  (`Hir::scoped_attributes`) attributes are stored for passes to read.
- **Next**: `@inline(always|never)` (needs a MIR function flag from the HIR
  attributes).

## Migration plan (replacing the `derive` syntax)

Measured 2026-10-19 on this tree:

- **153** Badger files use `derive(...)` (std, `examples/`, `tests/`) — about
  189 clauses; **74** references in `docs/` and `spec/`.
- Compiler surfaces: lexer (new `@`), CST (`cst.rs` carries
  `derives` on `Struct` and `Enum`), grammar (`grammar.rs:1095`), CST→AST,
  the AST `derives` fields, the formatter (`format.rs`), HIR
  (`lower_derives`), and typeck (`derive_impls`).
- Editor grammar: `editors/tree-sitter-badger` (hand-maintained
  `grammar.js` plus generated `parser.c`/`grammar.json`; regeneration needs
  the tree-sitter CLI).
- Diagnostics mention `derive` by name: `E0117`, `E0202`, `E0203`,
  `E0204`, and their catalog text.

**Do not do this as one change.** Stage it:

0. **Decide** the open questions below first — unknown-attribute policy,
   the supported placement set (must include item position), the argument
   forms (paths included), whether attributes are interface-significant,
   and the docs/tooling obligations.
1. **Add the surface, keep `derive` working.** Lexer/grammar/CST/AST/HIR for
   `@name(args)` on items; route `@derive(...)` to the existing generator;
   parse `derive(...)` as sugar into the same attribute list. Nothing is
   migrated, so this is reversible and reviewable on its own.
2. **Migrate.** Convert std/`examples`/`tests`/`docs`/`spec` to
   `@derive(...)`; make the formatter print the chosen form; regenerate the
   tree-sitter grammar; update the diagnostic text/catalog.
3. **Delete.** Remove the `derive(...)` grammar form and the `derives`
   CST/AST fields.

**This is not required for fusion.** The fusion templates behind a registry
need no surface change; do that first if the goal is the data-processing
work, and treat the attribute unification as its own project.

## Open questions

- ~~**Placement**~~: decided (see
  [Placement, namespaces, and visibility](#placement-namespaces-and-visibility));
  expressions take none.
- ~~**Unknown-attribute policy**~~: decided — an unknown compiler
  attribute is an error; a library's undeclared one warns in user code
  (errors in std); an unimported namespace is an error.
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
