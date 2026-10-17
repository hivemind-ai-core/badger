# Functions, effects, errors, and protocols

Status: draft 0, implemented by this repository's toolchain except where marked
**Proposed:**. See [the specification index](../SPEC.md) for how to read it.

## Function values and capture

Functions are eager first-class values. They may be stored in records, passed,
returned, and captured by closures. There is no implicit receiver or ambient
application dependency injection. A record of functions is a useful service
interface; a component factory closes those functions over its dependencies.

An ordinary `fn(A) -> R` is callable repeatedly subject to its parameter modes.
A `once fn(A) -> R` may be called at most once and can capture affine values
that it consumes. Closure mode and captures infer. A closure capturing a
task-local cell cannot be transferred to another task. Capturing a scoped
resource preserves its region bound, even if the closure is otherwise shareable.
No closure erases the ownership or lifetime of what it captures.

Parameters are by value unless borrowed with `&T` or `&mut T`. Shareable values
can be used again; affine arguments move. Borrowed parameters do not acquire
ownership. Standard methods auto-borrow their receiver according to their
declared signature; ordinary function arguments use explicit borrows when
required. A `once` callback parameter allows ordinary one-shot closures without
writing a `move` modifier at every call site.

Scope helpers check callbacks against restricted higher-rank region signatures.
They do not introduce unrestricted impredicative inference into ordinary code.
See [scope ownership](04-memory-and-collections.md).

## Inferred effects without async coloring

Every function can execute inside a runtime task. There are no separate `async`
functions, futures that must be manually polled, or duplicated sync/async APIs.
I/O or channel calls may suspend the current task while preserving sequential
source-level control flow.

The compiler records effect summaries. Draft 0 effect types are:

| Effect | Meaning |
|---|---|
| `Io` | External filesystem, network, process, device, or environment interaction |
| `Wait` | Explicit parking, timers, or coordination that can suspend the task |
| `State` | Access to mutable state observable outside a function's private computation |
| `Foreign` | An unsafe foreign operation outside the language's ordinary guarantees |

Clock and random-source access are explicit dependencies and appear as `Io` or
`State` according to the provider. Mutation of a unique local builder alone
does not introduce externally observable `State`. Scope cleanup contributes
the effects of the cleanup operation.
Implementation: `time.now`, `time.timestamp`, and `random.system` carry
`Io` / `State`, tests control them (`std.test` clocks, `random.seeded`),
and reproduction bundles record them; they are not ambient authority in
restricted projects (chapter 07), since they reach nothing outside the
program.

```lang
import std.io

fn double(value: Int) -> Int !{} {
    value * 2
}

fn load(path: String) -> Result<String, io.Error> !{Io, Wait} {
    io.read_text(path)
}
```

An omitted effect annotation is inferred. `!{}` permits no modeled external
effects; it is not a totality or panic-freedom proof. Effect rows can be generic:
`fn(A) -> B !Eff`, with `Eff: Effect`. Callback effects propagate to their
caller. An explicit annotation is an upper bound; calling a less effectful
function where a larger bound is allowed is safe.

Implementation note: an omitted effect is inferred only where inference can
see it. In a parameter type it is effect-polymorphic, so a callback's effects
are charged to the caller, including callbacks carried in records, lists,
structs, and enums. In a struct field, enum payload, alias, or a signature's
result, nothing determines it, so such a function type may perform any
modeled effect (`Io`, `Wait`, `State`, `Foreign`); annotate it (`!{}`,
`!{Io}`) to call it under a tighter bound.

Scheduler budget checks, tracing bookkeeping, and GC safe points are runtime
mechanisms, not a requirement to annotate every pure function with `Wait`.
Source-level nondeterminism still comes from concurrency and effectful calls.
Effects are static summaries for checking/tooling, not algebraic effect handlers
or arbitrary control-flow interception. No handler syntax is added in draft 0.

## Expected errors

The prelude contains:

```lang
enum Option<T> {
    none,
    some(value: T),
}

enum Result<T, E> {
    ok(value: T),
    err(error: E),
}
```

`some`, `none`, `ok`, and `err` are ordinary prelude helpers. Other enum
constructors use their type namespace. Results are eager values, not exceptions.
Ignoring a `Result` is a diagnostic error by default; `let _ = expression`
explicitly acknowledges discarding it. Ordinary values may be discarded.

`?` requires a surrounding function or callback returning `Result`. For
`Result<T, E1>` inside `Result<R, E2>`, it accepts `E1 = E2` or one unique
direct `From<E1>` implementation for `E2`. It does not search conversion chains,
erase to `Data`, invent a union, or catch task failure. `Option` must be matched
or converted explicitly with a missing-value error.

```lang
import std.codec
import std.sqlite as db

pub enum AppError {
    decode(source: codec.DecodeError),
    database(source: db.Error),
    validation(message: String),
} derive(Error, From)
```

`derive(From)` generates direct conversions for variants with exactly one
payload whose type is unique among eligible variants. Duplicate source types
require explicit mapping; there is no arbitrary preferred variant. `Error`
provides display, optional cause, and structured diagnostic details. Domain
validation remains normal code returning a named error.

Error diagnostics include a stable code, message, source span, causal chain,
and typed or explicitly encoded context. Codecs may render that context as
`Data`. A programmer-defined error does not require arbitrary dynamic fields.

## Contracts

A function or method may state contract clauses on the lines before it:

```text
requires amount > 0, "withdraw a positive amount"
ensures result.is_err() || result.unwrap_or(account).balance == account.balance - amount
pub fn withdraw(account: Account, amount: Int) -> Result<Account, BankError> { ... }
```

`requires COND` is checked when the function is entered, before its body;
`ensures COND` is checked whenever it returns (falling off the end, `return`,
and `?` propagation alike), with `result` bound to the value being returned.
Conditions are `Bool` expressions over the parameters (and `result`), checked
by the ordinary type, effect, and ownership rules: a postcondition runs after
the body, so it cannot read a value the body moved, and it can read `result`
but not consume it. Parameters passed by value are immutable, so a
postcondition reads them as they were on entry; a `&mut` parameter is read as
the function leaves it. A clause cannot `return` or propagate with `?`
(E0140), and contracts belong on functions with bodies (a protocol method's
implementations carry their own). The optional string explains the clause.

A failed clause is a task defect, like a failed assertion. A failed
precondition is reported at the call site (the caller broke the contract) and
names the clause; a failed postcondition is reported at the clause (the
function broke it). Contracts are checked in every build, like overflow
checks. Tools show a function's clauses with its signature.

The compiler decides what it can before the program runs. It follows each
function with known integer ranges, booleans, texts, and list lengths,
learning from constants, immutable bindings, enclosing `if` conditions and
early-return guards, the function's own preconditions, and the
postconditions of the functions it calls. A call whose arguments cannot
satisfy a precondition is an error (E0145), as is a returned value that
cannot satisfy a postcondition (E0146). A clause it proves is compiled
out: a postcondition proven at every return (including `?` exits), and a
precondition proven at every call of a function that is only called
directly (not used as a value). Everything else is checked at run time as
above. `lang contracts PATH` lists each clause with its status
(`proven`, `refuted`, or `runtime`) and why; `--json` gives it to tools.

Tests and contracts inform each other. `lang contracts --tests PATH` runs
the tests with the program observed: each clause reports how often the
tests evaluated it and how often it held (a clause no test reaches is a
gap), and integer arguments and results whose every observed value fit a
bound become candidate clauses (`take could state ensures result >= 0`).
Candidates are suggestions from the tests' data, for a person or an agent
to accept, sharpen, or refute with another test; the compiler then checks
what is stated as above.

## Task defects and cancellation

Checked overflow, bounds violations, failed assertions, and explicit `panic`
terminate the current task through its failure boundary. They are not expected
`Result` errors and are not silently caught by ordinary interceptors. A task
capture boundary can report a typed `task.Exit<T>` to a parent or supervisor.
Cancellation is its own exit reason and is observed at supported safe points.

All language exit paths run registered scope cleanup. Cleanup can suspend under
a bounded shutdown policy with cancellation shielding for the cleanup itself.
An initial failure remains primary; cleanup failures are attached rather than
replacing it. A process abort, memory corruption, or unsupported foreign fault
does not promise recoverable language-task isolation.

There is no source-level `throw`/`catch`/`finally` in draft 0. Expected failures
use `Result`; external-resource cleanup uses scoped helpers; task defects are
handled at explicit task boundaries. A transaction helper can fail after its
callback returns, so callers must preserve the helper's result.

## Protocols and coherent extension

```lang
pub protocol Display {
    fn display(self: &Self) -> String
}

pub struct UserId { value: U64 }

impl Display for UserId {
    fn display(self: &Self) -> String {
        $"user-{self.value}"
    }
}
```

Protocols define statically checked behavior. Bounds use `where`:
`fn render<T>(value: &T) -> String where T: Display`. The initial protocol
system supports type parameters and named associated types, but no higher-kinded
type parameters, specialization, blanket overlapping instances, or inheritance
of storage. One resolved implementation applies to a protocol/type combination.

An implementation must belong to the package defining the protocol or the
outer nominal type. Otherwise use a local nominal wrapper. Structural aliases
are not distinct implementation identities. Primitive/structural operations are
provided through generic implementations and compiler derivation.

`derive(Eq, Hash, Display, Codec, From, Error)` requests supported derivations;
each has a checked schema and can fail with a normal diagnostic. Derivation
does not expose a general macro evaluator. Equality and hashing preserve their
laws; floats do not automatically satisfy total equality/hash constraints.

`dyn Protocol` is an explicit dictionary-dispatched value for heterogeneous
interfaces. Object-safe protocols must have a representable calling convention;
generic methods are not dynamically callable without a separate concrete
adapter. Ordinary generic calls specialize or use typed dictionaries.

The memory capabilities `Share`, `Transfer`, and affine ownership are compiler
properties, not protocols that arbitrary safe code may implement dishonestly.
Unsafe runtime/FFI extensions require an audited declaration and remain explicit.

The continuation syntax is informed by [Gleam's `use`](https://tour.gleam.run/advanced-features/use/).
The error, effect, and protocol rules above are this draft's own design, and
the toolchain implements them.
