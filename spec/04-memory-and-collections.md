# Memory, ownership, and collections

Status: draft 0, implemented by this repository's toolchain except where marked
**Proposed:**. See [the specification index](../SPEC.md) for how to read it.

## Values and access capabilities

Ordinary records, strings, enums, lists, maps, and sets have immutable value
semantics. Local bindings and unique storage can change without changing a value
observed through another immutable alias. The compiler may reuse dead/unique
storage when that preserves these semantics.

Ownership properties are separate from ordinary shape types:

- A shareable value can be duplicated and used safely by multiple tasks.
  Primitive values and transitively shareable records inherit this capability.
- An affine value has one owning use. Moving it invalidates the source binding.
  Builders and mutable buffers are affine. They may be borrowed.
- Task-confined mutable cells can have aliases within their owning task but
  cannot cross a task boundary. `Cell<T>` is an explicit such type.
- `Transfer` means the value can cross a task boundary by safe sharing or by
  consuming exclusive ownership. A record with a task-local cell is not
  transferable simply because its field bindings are immutable.
- A scoped value also carries a region bound, independently of these properties.

Aggregate capabilities follow their fields. A record containing an affine
buffer is affine; one containing a confined cell is confined. A synchronized
runtime type such as `Atom<T>` may be shareable through its checked interface,
even though it manages mutable state internally. Ordinary safe code cannot
assert these capabilities without the compiler verifying their representation.

`let` prevents rebinding. It does not freeze a mutable capability owned by that
binding: a unique buffer may be appended to through its owner interface.
`var` permits rebinding while preserving the binding's type. It does not grant
mutation rights to shared immutable data.

## Borrowing and regions

`&T` borrows read access; `&mut T` borrows exclusive access. Borrows cannot
outlive their owner. Exclusive borrows exclude other accesses to the borrowed
place for their inferred nonlexical lifetime. An exclusive borrow of a unique
container permits its mutable interface; arbitrary immutable heap data does
not gain a mutable interface through borrowing. Mutable borrow of an ordinary
binding's storage requires an assignable place.

Implementation note: an exclusive borrow grants the borrowed value's mutable
interface (a buffer's `push`, a builder's `append`, a cell's `set`); it does not
make the borrowed binding assignable through the reference (`x = x + 1` with
`x: &mut Int` is E0130). A function that computes a new plain value returns it,
and the caller assigns its own `var`.

Ordinary local borrow lifetimes infer. Public APIs that express scope relations
may use region type parameters, written uppercase like other type-level names.
Trusted scope helpers introduce a fresh abstract region for their callback.
Schematically:

```text
with_resource<R>(..., body: for<S: Scope> fn(Handle<S>) -> R) -> R
```

`R` is outside the fresh region binder, so it cannot contain `Handle<S>` or a
closure capturing it. The source type grammar supports `for<S: Scope> fn(...)`
for these checked callback contracts, not unrestricted higher-rank inference.
Resource implementations declare their handle's region parameter; the compiler
may elide it in diagnostic/display contexts where its origin is unambiguous.

Child tasks in a scope may borrow enclosing *shareable* resources if they finish
before that resource closes. A task-local transaction does not become shareable
just because a child has a shorter lifetime. Captures and returned records keep
their region constraints; boxing, `Data`, protocols, and channels cannot erase
them. External-resource ownership never depends on eventual GC finalization.

## Heap strategy

The runtime uses a precise, nonmoving tracing heap. Ordinary heap objects and
collection pages keep their addresses while alive. Scalar replacement removes
the allocation of records that do not escape. Records do not receive a
per-object dynamic map.

The collector traces typed roots and reclaims unreachable allocations.
Suspended task stacks, channels, pending batches, closure environments, and
registered foreign roots are traced. Published managed references need proper
cross-worker memory ordering even though their payload is immutable.

**Implementation:** a collection starts once no worker is running compiled
code (workers stop at allocation and poll safe points). Roots are per-task
shadow-stack frames, module-level pointer slots, and pinned runtime roots;
there is no native stack walking. The collector is generational without
moving objects: mark bits are sticky, so a minor collection traces only
from objects written since the last one (a write barrier records them).
Each worker sweeps its own pages in parallel. Records are heap objects with
a descriptor header unless scalar replacement removes them; list elements
of record type are references to such objects. `[runtime] heap_max_bytes`
bounds live data ([memory budget](07-runtime-and-tooling.md#memory-budget)).

This choice handles cyclic closures and explicit mutable cells without requiring
reference-count operations on every shared value. It costs collector metadata,
fragmentation, and collection pauses. Low pause times are a performance goal,
not a real-time promise. Collector policy may evolve while preserving language
semantics and address-stability contracts.

Unique buffers and page pools may reclaim storage earlier when exclusivity and
outstanding views permit it. A pool retains only a bounded reserve
(**Implementation:** four pages per channel).
Managed page views retain their backing allocation. Raw FFI borrows require
explicit rooting and cannot survive an unregistered lifetime boundary.

## Paged lists

`List<T>` is a frozen paged sequence. `ListBuilder<T>` is its affine append
builder. List literals and `list.collect` construct through a unique builder and
then freeze. `finish` transfers the page directory without moving elements.

```lang fragment
let builder = list.builder<Event>()
for event in source {
    builder.append(event)
}
let events = builder.finish()
```

Fixed-capacity pages fill in order; only the final page may be partial. A page
directory provides constant-time indexing with a page lookup plus an offset.
Growing a builder may relocate directory metadata, never existing page elements.
Type size/alignment and configured byte budgets determine page capacity; it is
not a language-visible magic number. Pages are stable through append growth.

Element iteration hides page boundaries. `chunks()` returns each page as
a frozen contiguous `Array` (**Implementation:** a copy of the page;
zero-copy views are proposed). A managed stable-element reference is available for `T: Share` and
retains its page; appending can continue because initialized slots cannot be
modified. References into affine elements instead use ordinary checked borrows
and do not grant simultaneous mutation or transfer rights. Clearing/recycling a
page waits for its outstanding views.

The frozen list has no mutating `push`. `list.append`/`list.concat` construct
new lists and may copy a partial tail and page-directory metadata while sharing
complete compatible pages. Repeated construction should use builders or lazy
`Seq<T>` views. Uniqueness optimizations can reuse storage, but immutable
snapshots do not promise free append or universal stable addresses in a *new*
snapshot. Old snapshots keep their own elements alive and unchanged.

Middle insertion, removal, sorting, and flattening are transformations, not
part of the stable append builder contract. They may allocate and move values.
An affine-element transformation must consume its input or borrow it without
duplicating ownership. A persistent operation that requires copying elements
requires shareable elements.

## Buffers, arrays, maps, and sequences

`Buffer<T>` owns contiguous mutable storage, grows by relocation, and is affine.
`buffer.freeze` consumes a buffer into an immutable `Array<T>`, and
`bytes.freeze` a `Buffer<Byte>` into `Bytes` (**Implementation:** both copy;
reusing the allocation is proposed). It requires no
surviving mutable borrow. Capacity hints are advisory unless a fixed-capacity
constructor is chosen; a full fixed-capacity append returns a checked error.

`Array<T>` has fixed length selected at construction. It is immutable and
contiguous; draft 0 does not add const-generic extent syntax. Mutable fixed-size
workspaces use fixed-capacity buffers. Building an array initializes every item
or returns an error and destroys the initialized prefix safely.
List syntax `[a, b]` defaults to `List<T>`; an expected `Array<T>` or `Buffer<T>`
can construct directly in that representation. An existing list needs an
explicit contiguous collection/flattening operation rather than a hidden copy.

`Map<K, V>` and `Set<T>` have persistent value semantics and structural sharing.
Keys require coherent `Eq` and `Hash`; iteration order is unspecified, so use
explicit sorting or an ordered collection for stable output. Builders support
bulk construction. Record labels remain static even though maps have runtime
keys. Map lookup returns `Option<V>` or a borrowed view, never implicit null.

`Seq<T>` is a typed lazy iterator/view. `map`, `filter`, `take`, `flat_map`,
`zip`, `fold`, `group_by`, and explicit collection are standard operations.
Pure sequence descriptions can be reused when their inputs permit it; consuming
an affine iterator is one-shot and inferred as such. Effects happen during
iteration, in order, not when constructing a lazy view (**Implementation:**
`Seq<T, Eff>` carries the effects of iterating; the omitted argument is
inferred in signatures. A sequence description may cross tasks when what it
captures may; an iterator taken from one stays on its task). Early termination closes
scoped iterators and their resources. APIs cannot return a lazy view borrowing
a resource that has already closed. Tasks/channels expose `Stream<T>` for
suspending input; an arbitrary stream is not silently a pure sequence.

## Dynamic data and reflection

`Data` is an explicit owned enum for null, booleans, supported numbers, strings,
lists, and string/symbol-keyed objects. Conversions encode or validate and decode;
they do not expose arbitrary language pointers or scoped resources. `Data` is
shareable when fully constructed. There is no unchecked `Data as T` cast.

Typed reflection uses compiler-generated shape/schema descriptors and visitors.
Turning heterogeneous record fields into a homogeneous sequence needs a typed
visitor or explicit `Data`. Debug inspection may encode values at its boundary;
that does not change how production records are stored.

## Channel page ownership

Message pages follow the same page concepts as list builders, with a
different public lifetime contract (**Implementation:** a channel holds its
pages as runtime vectors of element words, not list pages). A page is uniquely writable while open,
sealed at publication, rooted by the runtime while queued/read, and reusable only
after every view is released. Appending an already-created inline value may move
its fields once; an in-place reservation initializes a slot directly when that
cost matters. Failure during construction destroys only its initialized prefix.
Neither a collector nor stable addresses permit a retained mutable alias across
publication. See [channels](05-tasks-and-channels.md).

Precedents: [Pony access capabilities](https://tutorial.ponylang.io/reference-capabilities/reference-capabilities.html),
[segmented append storage](https://docs.rs/segvec/latest/segvec/), and
[indexed split vectors](https://docs.rs/orx-split-vec/latest/orx_split_vec/).
The collector and source-level ownership combination are draft choices.
