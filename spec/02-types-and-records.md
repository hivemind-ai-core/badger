# Types and records

Status: draft 0, implemented by this repository's toolchain except where marked
**Proposed:**. See [the specification index](../SPEC.md) for how to read it.

## Primitive and composite types

The initial primitives are `Unit`, `Never`, `Bool`, `I8`, `I16`, `I32`, `I64`,
`U8`, `U16`, `U32`, `U64`, `F32`, `F64`, `Char`, and `Symbol`. `Int` aliases
`I64`, `Float` aliases `F64`, `Byte` aliases `U8`, and `Size` is an unsigned
size type (**Implementation:** it aliases `U64`; every supported target is
64-bit). `String` is an immutable UTF-8 managed value.
`Bytes` is immutable byte storage. `Duration`, `Instant`, and `Timestamp` are
distinct nominal standard-library types.

There is no null value, universal implicit `Any`, general implicit subtyping,
or arbitrary intersection/union type syntax. `Option<T>` represents absence;
`Result<T, E>` represents expected failure. Tuple types use `(A, B)`.
Function types use `fn(A, B) -> R`; their ownership and effects are described
in [functions](03-functions-and-errors.md).

Integer arithmetic is checked in every build: overflow, invalid shifts, and
division by zero are task defects with source locations. Integer division
truncates toward zero; remainder has the dividend's sign. Explicit checked
helpers return `Result`, and explicit wrapping helpers provide modular
arithmetic. Floating arithmetic follows IEEE 754. Runtime conversions use
lossless `Target.from(value)` or checked `Target.try_from(value)`; conversions
do not silently truncate.

String indexing by integer is not defined as character indexing. Use byte or
character views and checked slicing with UTF-8 boundary validation. Collection
`get(index)` returns `Option`; `collection[index]` is a checked access that
reports a task defect on an invalid index. No release-only unchecked behavior.

## Structural record inference

```badger
let person = { name: "Ada", age: 36, needs_glasses: false }

fn greet(person) {
    $"Hello, {person.name}"
}

fn birthday(person) {
    { ..person, age: person.age + 1 }
}
```

The literal has the closed shape `{name: String, age: Int,
needs_glasses: Bool}`. A function's accesses constrain its parameter. `greet`
accepts `{name: T, ..Rest}` with `T: Display`; annotating `name: String` or
calling a string-only operation restricts it to `String`. `birthday` requires
a numeric `age` and preserves its entire remaining row. Numeric types resolve
from call constraints or public annotations; a genuinely unresolved overloaded
numeric type requires an annotation rather than silently becoming `Any`.

With an explicit `Int` operation, the birthday scheme is:

```text
forall Rest: Row.
{age: Int, ..Rest} -> {age: Int, ..Rest}
where Rest lacks age
```

Rows contain unique labels and their types. `Rest` is a compile-time row
variable; it is not a runtime field bag. Generic kinds are inferred; explicit
constraints may write `Rest: Row`. The compiler solves presence, absence,
extension, replacement, and merge constraints along with ordinary unification.

An annotation `{x: Int}` is closed. `{x: Int, ..Rest}` is open. Reading a
misspelled field adds a requirement that fails at a call with the wrong shape;
it never silently reads an absent value. Row-generic calls retain the full
concrete representation and do not project or erase unknown fields.

## Labels, updates, and destructuring

Ordinary labels are lowercase identifiers. Qualified symbol labels allow
library-specific extensions without unrelated name collisions:

```badger fragment
let authenticated = { ..request, [#auth/user]: user }
let current_user = authenticated[#auth/user]
let { name, age, .. } = person
```

Identifier `name` denotes the symbol `#name`; qualified labels retain their
full symbol identity. A literal-symbol selection is statically typed.
`record[runtime_symbol]` is rejected: heterogeneous fields do not have a single
inferred result type for arbitrary keys. Convert to `Data` or a map explicitly
when runtime key lookup is required.

One spread may occur first in a literal. Explicit fields replace or add labels,
and may change a field's type. Two explicit occurrences of the same label are
errors, even if their types match. The result is a new value; observed aliases
to the old record are preserved.

```badger fragment
let changed = { ..person, age: 37 }
let extended = { ..person, email: "ada@example.test" }
let relocated = { ..person, address.city: "Cork" }
```

Dotted updates require every intermediate record to exist. The final field
may be added or replaced. Overlapping explicit paths, such as `address` and
`address.city`, are rejected. They lower to nested row-preserving updates.
`records.remove(record, #label)` requires a literal existing label and returns
the row without it. `records.merge(left, right)` is right-biased; an overlapping
field takes the right value and type. Reflection specializes these operations;
it is not runtime hashing of ordinary field accesses.

Branches returning incompatible shapes do not automatically gain nullable
fields. Use an enum, explicit `Option` fields, or explicit projection to a
common shape. Homogeneous collections follow the same rule.

## Aliases, structs, and enums

```badger
pub type Person = { name: String, age: Int, needs_glasses: Bool }

pub struct UserId {
    value: U64,
}

pub fn user_id(value: U64) -> UserId {
    { value }
}

pub enum Job {
    import_file(path: String),
    purge(before: Timestamp),
    shutdown,
}
```

An alias introduces no new identity. A `struct` has nominal identity. Its
fields are private unless individually `pub`; its defining module may
construct it using a contextual record literal. Outside that module, literal
construction, struct update (`{ ..value, field: x }`, which constructs a new
value), or record coercion requires all fields to be public and no
restricted construction contract. A struct with any private field has a
restricted construction contract: only its defining module constructs it
(by literal, coercion, or update, E0308); other modules obtain values
through that module's functions, which establish its invariants (as
`fs.root` does for `fs.Dir`). A constructor is an ordinary lowercase
function; there is no required repeated same-name constructor declaration.

`Job.import_file(path)` and `Job.shutdown` construct variants. Variant
parameters are typed and ordered; record-valued variant payloads are supported.
Enums are closed, nominal, and exhaustively matched. Representation is a
discriminant plus a statically determined payload, with niche optimization
permitted. Recursive inline definitions need indirection such as `Box<T>` or
a collection; infinitely sized types are rejected.

`pub struct` exposes the type, not its private fields. `repr c struct` uses
declared C field order and representation; it is a foreign boundary and does
not inherit internal layout equivalence. Dynamic object interfaces are explicit
`dyn Protocol` values with a data handle and method dictionary, not a universal
object superclass.

## Contextual construction and projection

A record literal defaults to its inferred record shape. Expected types can
construct directly into a compatible closed record, transparent nominal struct,
or `Map<Symbol, V>`. Every map field must fit `V`; an expected `Data` value type
authorizes explicit dynamic encoding. String-key maps use `map { ... }`.

An existing exact-shape record may coerce to a transparent nominal struct with
matching field types and compatible ownership. Private invariants are never
bypassed. An existing superset must use `records.project<Target>(value)` to
construct a closed owned subset; this may copy or move fields. A borrowed,
row-generic consumer can read a subset without constructing it.

Internal record layouts order fields by canonical qualified label, align each
field to its representation's alignment, and round the final size accordingly.
Transparent nominal structs use the same internal layout rule. Type identity
ignores source field order. Same types, layout, and ownership can permit a
conversion without reconstruction; merely matching names is insufficient.
ABI-stable layouts, boxed/unboxed choices, and different scope brands remain
distinct constraints. Physical layout compatibility never authorizes a type
or lifetime violation.

## Inference and specialization

Use Hindley–Milner-style rank-1 generalization with record rows and effects.
Shareable nonexpansive values and function definitions can generalize; mutable
cells, affine captures, and effectful allocations follow a value restriction.
Public functions declare argument and result types; explicit generic row/type
parameters describe open APIs. Private functions and local values infer.
Public effect summaries may infer but are materialized in interface artifacts.
Private annotations may use `_` as an inference hole; public signatures must
name their types and generic parameters completely. A hole is not a type name.
When a private fallible function has no expected error type, one source error
can infer exactly; multiple distinct source errors require an error annotation
or explicit mapping rather than guessing a conversion target.
Polymorphic recursion requires an explicit signature. Scope callbacks have
restricted higher-rank checking, described in [memory](04-memory-and-collections.md).
A struct or enum whose last type parameter is a region (`Node<R: Scope>`) is a
region record, branded with the region its values belong to; a `region` block
names a region that ends with the block, and its records cannot outlive it
(see [region blocks and region records](04-memory-and-collections.md#region-blocks-and-region-records)).

Specialize generic record operations for concrete shapes. Keep shape descriptors
for reflection and diagnostics once per shape, not a field-name map on each
value. Where code-size policy uses a generic body, provide typed layout/offset
dictionaries. Shape erasure across heterogeneously stored runtime objects must
be explicit; specialization does not make arbitrary dynamic shapes static.

## Protocols and schemas

Automatic structural equality requires all fields to support equality; field
order does not matter. Hashing requires `Eq` and `Hash`. Floats have IEEE
`PartialEq`/`PartialOrd`, not unconditional `Eq`/`Hash` guarantees. Lists compare
by their ordered values, maps by entries, and sets by membership. No identity
comparison is inferred for ordinary record values.

`schema<T>()` produces a typed `Schema<T>`. Derivation supports literals,
records, nominal structs with permitted construction, enums, and supported
collections. Wire field/variant names are versionable schema choices, not
memory offsets. A schema decoding operation validates external input and
returns `Result<T, DecodeError>` with source/path context. It does not replace
domain validation. Explicit `Located<T>` values attach spans; there is no
universal hidden metadata whose equality rules all values must inherit.

Primary precedent: [Flix's row-polymorphic records](https://doc.flix.dev/records.html).
The native layouts, ownership rules, and syntax here are this draft's choices.
