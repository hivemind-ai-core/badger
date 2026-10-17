# Syntax and control flow

Status: draft 0, implemented by this repository's toolchain except where marked
**Proposed:**. See [the specification index](../SPEC.md) for how to read it.

## Names, modules, and visibility

Type identifiers match `[A-Z][A-Za-z0-9_]*`. Value identifiers match
`[a-z][A-Za-z0-9_]*` or `_` followed by a lowercase letter and identifier
characters. `_` alone is a discard pattern, not a readable value. Identifiers
are ASCII initially; strings, characters, and comments support Unicode.

Every uppercase identifier names a type-level entity: a concrete type, generic
type parameter, protocol, or effect type. Constructor functions and enum
variants are lowercase. Constants are lowercase too. A namespace qualifier is
lowercase; `http.Request` names a type, and `http.listen` names a function.
`Result.ok` accesses a constructor through the type namespace; `Result` itself
remains a type identifier, not an ordinary value.

```lang fragment
import std.http
import std.result.{Result, ok, err}
import app.orders as orders

pub type Order = { id: U64, total: Int }

pub fn order_total(order: Order) -> Int {
    order.total
}
```

A file is one module; its path inside `src` determines its module path.
`src/orders/store.lang` is `orders.store`. Import cycles involving value
initialization are rejected. Pure type/function cycles may form one checked
module group. Declarations are private unless marked `pub`. Module initialization
can define pure immutable values; acquisition and other effects belong in
functions or system initialization, never hidden import-time work.

Implementation note: a module value whose type stores mutable state (`Cell`,
`Atom`, buffers, builders) or an acquired resource (channel endpoints, pools,
transactions, tasks) is rejected with E0138. Function values that merely take
such types are fine. Bindings typed at the local REPL are session state and
are exempt.

Keywords include `import`, `as`, `pub`, `type`, `struct`, `enum`, `protocol`,
`impl`, `for`, `where`, `derive`, `fn`, `once`, `let`, `var`, `const`, `if`,
`else`, `match`, `while`, `loop`, `in`, `break`, `continue`, `return`, `use`,
`system`, `pipeline`, `ref`, `map`, `set`, `unsafe`, `extern`, `dyn`, `repr`,
`requires`, and `ensures`.
`true` and `false` are boolean literals. `mut` is reserved for exclusive borrows.
`system`, `pipeline`, `ref`, `map`, `set`, `derive`, `repr`, `requires`, and
`ensures` are contextual:
their special syntax is recognized in the corresponding form, while module
paths such as `std.system` and calls such as `pipeline.compile` remain valid.
Keywords after a dot can name a declared member; ordinary declarations still
cannot shadow a reserved control-flow keyword.

## Layout and literals

Newlines separate statements. Semicolons may separate same-line statements and
explicitly discard an expression's value. Newlines inside parentheses and
brackets, after an unfinished operator/comma, or before a continuing `|>` do not
terminate an expression. Braces contain a sequence whose final expression is
its value, unless that expression has a semicolon. An empty block has `Unit`.
Comments use `//`; block comments use `/* ... */` and may nest. `///` documents
the following declaration. The formatter chooses one canonical layout.
Within an aggregate literal/type or plan description, surrounding newlines are
layout and commas separate entries. A nested lambda/control-flow block restores
statement boundaries even when it occurs inside a call's parentheses.

```lang fragment
let count = 42
let ratio = 0.5
let mask = 0xFFU32
let message = "hello"
let formatted = $"received {count} jobs"
let initial = 'A'
let key = :database
let namespaced_key = :app/database
let pair = (count, message)
let nothing = ()
let jobs = [first_job, second_job]
let request = { path: "/orders", method: :post }
let headers = map { "accept" => "application/json" }
let methods = set { :get, :post }
```

Integer literals support decimal, `0x`, and `0b`, with `_` separators.
Type suffixes use uppercase primitive names, such as `12U8` or `0.5F32`.
An unsuffixed number takes an expected numeric type when it fits; otherwise
integers default to `Int` and decimal/exponent literals to `Float`. No implicit
runtime numeric widening is introduced. Duration literals have fixed unit
suffixes `ns`, `us`, `ms`, `s`, `min`, and `h`; these are literal units, not type
identifiers. Negative durations require explicit signed arithmetic helpers.

Strings are immutable UTF-8. Characters are Unicode scalar values. Escapes
include `\n`, `\r`, `\t`, `\0`, `\\`, `\"`, `\'`, and `\u{...}`. Interpolation uses
`$"...{expression}..."`; `{{` and `}}` escape interpolation braces. Interpolated
values need a `Display` implementation. Triple-quoted strings preserve raw
multiline text without escape processing; a `$` prefix enables interpolation.
Symbols are interned qualified names, distinct from strings and enum variants.

`{}` is an empty record. A brace expression beginning with a field label, a
spread, or shorthand fields (`{ name }`, `{ a, b }`, meaning `{ name: name }`)
is a record; other expression braces form a block. Records allow one
spread, first. Lists, records, maps, sets, parameter lists, and match arms allow
trailing commas. A tuple of one item requires `(item,)`.

## Functions and calls

```lang
fn identity<T>(value: T) -> T {
    value
}

let increment = fn(value) { value + 1 }
let double = fn(value) => value * 2
let answer = identity<Int>(42)
```

A lambda's body is a block, or `=> expression`: `fn(x) => e` is
`fn(x) { e }`, with the same parameters, captures, effects, `once`, and
`return`/`?` boundary. The expression extends as far as an expression
can (to the closing `)` or `,` of an enclosing call, or the end of a
match arm), so `fn(n) => fn(k) => k + n` nests. Use the short form for a
single expression and a block for statements.

Calls are explicit and positional. Use an options record when several arguments
need labels or defaults. There is no automatic currying or parameter default
evaluation. Generic arguments normally infer; explicit arguments use `<...>`.
The uppercase type role and syntactic type grammar distinguish generic arguments
from value comparisons. A value identifier is not accepted as a type argument.

`value.field` reads a record field. Calling a function-valued field does not
implicitly pass the containing record: `services.save(order)` calls that field
with `order`. Nominal and built-in types may expose declared methods; method
resolution is type-directed, and receiver borrowing follows its signature.
There is no unrestricted search for extension functions with matching names.
Ordinary module functions compose through calls and pipelines.

```lang fragment
events
    |> seq.filter(fn(event) => event.enabled)
    |> seq.map(fn(event) => event.name)
    |> list.collect()
```

`x |> f(a, b)` means `f(x, a, b)`. Pipe evaluation is left to right, with the
left side evaluated once. There are no implicit placeholders or alternative
last-argument pipes. The pipe operator has lower precedence than comparisons
and arithmetic and higher precedence than assignment. Postfix `?` binds
tighter than `|>`, so `x |> f(a)?` applies `?` to `f(a)`; write
`(x |> f(a))?` to propagate the pipeline's error.

## Bindings and control flow

`let` binds an immutable name; shadowing is allowed. `var` makes a binding
assignable but does not make its value mutable. Assignment returns `Unit` and
preserves the binding's type. Mutable containers have separate ownership rules.
`const` permits literal/constructor aggregates and supported pure constant
operators; arbitrary function evaluation is not part of draft 0.

```lang fragment
var completed = 0
for job in jobs {
    process(job)?
    completed = completed + 1
}

let label = if completed == 0 { "empty" } else { "processed" }
```

Conditions require `Bool`; no truthiness. `if` is an expression; without `else`
its value is `Unit`. Both branches with an `else` must agree on a type. `for`
and `while` evaluate to `Unit`. `loop` can produce a value using `break value`;
all reachable breaks agree on its type. `return` exits the nearest function or
lambda. `continue` targets the nearest loop. Ranges `start..end` and
`start..=end` are lazy integer sequences with exclusive/inclusive upper bounds.
Self tail recursion is lowered to a loop; other tail-call optimization is not
a source-level stack-space guarantee.

`match` is exhaustive. Arms use `=>`; guards use `if`. Guarded arms do not
establish exhaustiveness. Patterns include literals, bindings, `_`, tuples,
records, and enum variants. Record patterns may use `..` to ignore other fields;
closed patterns require an exact shape. A refutable pattern belongs in `match`,
not plain `let` or a function parameter.

```lang fragment
match result {
    .ok(order) => order.id,
    .err(error) => report(error),
}
```

A leading-dot variant pattern takes its enum from the scrutinee. Constructor
expressions use `Enum.variant(...)`, or an imported ordinary helper such as
`ok(...)`; draft 0 does not infer an enum by searching for a bare variant name.

## Errors and continuation binding

`expression?` extracts the success case of `Result` or returns its error from
the nearest function/lambda, using an authorized direct error conversion. It
does not unwrap `Option`, panic, or skip cleanup.

```lang fragment
use tx <- sqlite.transaction(pool)
let order = insert_order(tx, input)?
ok(order)
```

This lowers to:

```lang fragment
sqlite.transaction(pool, fn(tx) {
    let order = insert_order(tx, input)?
    ok(order)
})
```

`use patterns <- callable(arguments)` inserts the remaining block as the final
callback argument. Its right side must be a call or a callable name, not a
postfix `?` expression. Multiple bound patterns are callback parameters.
`return` and `?` after `use` affect that callback. The helper's overall result is
the enclosing block's value, so helper-level commit/cleanup errors are retained.
The callback's call count, ownership, and lifetime follow the helper's type;
`use` itself promises neither cleanup nor a one-shot invocation.

## Operator precedence and evaluation

From strongest to weakest: calls/field/index/`?`; unary `-`, `!`, borrow;
`* / %`; `+ -`; integer shifts; bitwise `&`, then `^`, then `|`; ranges;
comparisons; `&&`; `||`; `|>`; assignment. Comparisons do not chain.
Operands, arguments, and record fields evaluate left to right. `&&` and `||`
short-circuit. All other evaluation is eager. Assignment is a statement, not
permitted inside a larger value expression; its target is a `var` binding, a
field of one (`r.a = 2`), or an index (`b[i] = v` calls the collection's
`set` method, as on a `Buffer`; frozen lists have none).

Numeric arithmetic, comparisons, indexing, and errors are specified in
[types](02-types-and-records.md) and [functions](03-functions-and-errors.md).
