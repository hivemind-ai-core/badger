# Adversarial testing report — 2026-10-15

Seven confirmed defects remain in the language implementation. This report is
the fixer handoff: it describes broken contracts and required outcomes, without
test sources, reproducer inputs, fixture names, discovery procedures, or proposed
patches. The independent regression suite is retained outside main for rerunning
after fixes. No implementation changes are part of this report.

## Revision and verification

- Initial isolated baseline: `b03600a`. Its existing
  `cargo test --workspace --offline` suite passed, including the single-worker
  and four-worker program tests and service integration tests.
- Main advanced during testing. The isolated branch was fast-forwarded to
  `f6424729798a8683ad9cfa79120da7afeeec5789`, rebuilt, and the retained adversarial
  suite rerun against that revision. Findings below refer to this newer revision.
- Runtime findings were verified with one worker, four workers, and optimized
  JIT compilation. Static findings were verified through `lang check`.
- The full existing workspace suite was run on the initial baseline only.
  This is a targeted adversarial review, not an exhaustive correctness claim.

## ADV-001 — Evaluation order does not preserve already evaluated values

**Priority: P1. Area: compiler/code generation.**

Calls and tuple construction can observe a later value for an earlier operand
when a subsequent operand reassigns the same local. This affects ordinary calls,
closure calls, method receivers, and tuple elements. Programs check successfully
and exit successfully with incorrect results.

**Required behavior:** evaluate operands and arguments left to right, preserving
each value as it was when evaluated. A later assignment must not retroactively
change an earlier argument, receiver, or tuple element. Preserve the same
behavior across optimization and worker settings.

Contract: [syntax — operator precedence and evaluation](../spec/01-syntax.md#operator-precedence-and-evaluation).

## ADV-002 — Valid unsigned ranges silently omit their elements

**Priority: P1. Area: range compilation/runtime.**

A valid increasing `U64` range that crosses the signed integer boundary can
iterate zero times. No diagnostic or task defect is reported; the loop body
is silently skipped.

**Required behavior:** range ordering and termination must respect the endpoint
type and its complete domain. An increasing unsigned range must visit its
specified elements in order, with the documented exclusive or inclusive bound,
without truncation, sign reinterpretation, or overflow at termination.

Contract: [syntax — bindings and control flow](../spec/01-syntax.md#bindings-and-control-flow).

## ADV-003 — Equal map/set keys are not interchangeable in hash collections

**Priority: P1. Area: collection equality and hashing.**

Separately constructed maps or sets can compare equal, yet a lookup using one
fails to find an entry inserted with the other as its key. These key types are
accepted by the checker. This breaks the `Eq`/`Hash` contract and ordinary
persistent collection value semantics.

**Required behavior:** equal keys must hash coherently and be interchangeable
for lookup, replacement, removal, and membership. Map/set value equality must
remain independent of allocation identity and construction order, including
when such values occur inside other keys.

Contracts: [types — protocols and schemas](../spec/02-types-and-records.md#protocols-and-schemas),
[memory — buffers, arrays, maps, and sequences](../spec/04-memory-and-collections.md#buffers-arrays-maps-and-sequences).

## ADV-004 — Aggregate equality changes IEEE NaN semantics

**Priority: P2. Area: runtime structural equality.**

Records and enum values containing NaN can compare equal to themselves even
though their floating payload does not. Equality depends on whether the
aggregate shares an allocation, so the wrapper changes the result of partial
equality rather than comparing the payload consistently.

**Required behavior:** structural partial equality must recurse into fields and
payloads with their own equality semantics. NaN must remain unequal to itself
inside aggregates; allocation sharing must not change that result. Floats must
remain excluded from total `Eq` and `Hash` guarantees.

Contract: [types — protocols and schemas](../spec/02-types-and-records.md#protocols-and-schemas).

## ADV-005 — Unterminated block comments are accepted

**Priority: P2. Area: lexical/layout analysis and parsing.**

`lang check` accepts a source file ending inside an unterminated block comment,
returning success with no diagnostic. Malformed source is silently treated as
valid source.

**Required behavior:** every opened block comment, including nested comments,
must close. An unterminated comment must produce a normal source diagnostic
and fail checking before compilation or execution.

Contract: [syntax — layout and literals](../spec/01-syntax.md#layout-and-literals).

## ADV-006 — Aggregate nesting bypasses affine ownership checking

**Priority: P1. Area: ownership analysis.**

A buffer inside sufficiently nested ordinary records loses the protection
against repeated owning consumption of its containing value. The checker
accepts ownership violations that it rejects for a shallower equivalent
aggregate. Safe wrappers can therefore duplicate an affine resource's
ownership.

**Required behavior:** aggregate ownership must account for contained affine
values at every valid nesting depth. Consuming a containing value must make
subsequent owning use invalid. Analysis limits must not silently grant
duplicability when the contained ownership obligation is unresolved.

Contract: [memory and ownership](../spec/04-memory-and-collections.md).

## ADV-007 — Accepted array operations lose value contents

**Priority: P1. Area: array runtime capabilities and codecs.**

An `Array<Int>` accepted for codec encoding is serialized as JSON `null`,
discarding its elements. Separately constructed arrays with equal elements also
compare unequal despite being accepted for equality. The checker and runtime
disagree about the value operations supported by arrays.

**Required behavior:** supported immutable array value operations must use the
element contents and order. Encoding must preserve the array's elements and
length; equality must compare values consistently. A capability the runtime
cannot implement must be rejected during checking rather than silently
substituting null or allocation identity.

Contracts: [types — protocols and schemas](../spec/02-types-and-records.md#protocols-and-schemas),
[memory — buffers, arrays, maps, and sequences](../spec/04-memory-and-collections.md#buffers-arrays-maps-and-sequences).

## Fix verification

Address the contracts above without relying on the retained fixtures. Rerun the
existing workspace suite, then return the implementation revision for the
independent regression rerun. Fixes should satisfy the general contracts across
equivalent source forms and supported execution settings.
