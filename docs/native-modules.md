# Native modules

A native module is a library of functions written outside the language —
in Rust, or anything with a C ABI — that a program declares and calls like
its own functions:

```badger fragment
import std.native

extern "native badger_sqlite" fn open(path: String) -> Result<native.Handle, native.Error> !{Io, Wait}
```

Native modules are how the language reaches existing libraries (SQLite,
HTTP, compression, a vendor SDK) without changing the runtime. Use
`extern "c"` instead for a plain C function taking machine words.

## Declaring native functions

`extern "native NAME" fn f` declares function `f` of module `NAME`;
`extern "native NAME.g" fn f` declares the module's function `g` under
the name `f` (as `std.sqlite` declares `badger_sqlite.open` as `sql_open`
beside its public `open`). Its
parameter and result types are the contract: the host copies each argument
into the module's wire format and each result back, checking it against
the declared type.

- Types that cross: `Bool`, integers, floats, `Char`, `Symbol`, `String`,
  `Bytes`, `Duration`/`Instant`/`Timestamp`, `Data`, `native.Handle`, and
  lists, sets, arrays, maps with `String` or `Symbol` keys, `Option`,
  `Result`, tuples, records, structs, and enums made of them. Anything else
  (closures, references, `Cell`, `Atom`, channels, tasks, raw pointers) is
  error E0410.
- A native function is not generic, takes at most 8 parameters, and names
  only its module (`extern "native NAME"`, E0115): how it runs is the
  module's decision.
- Declare its effects as for any function (`!{Io, Wait}` for one that
  does I/O and may wait). A call needs no `unsafe`: nothing the module
  does can corrupt the program's memory.
- In a restricted project, native functions are ambient authority, like
  `extern "c"`: only the entry module may call them (E0170). Standard
  modules built on native modules hand out capabilities instead.

How values cross:

| Declared | Wire value |
|---|---|
| a record or struct | a map by field name (missing `Option` fields are `none`; unknown fields are ignored) |
| a tuple, list, set, or array | a list |
| an enum | a variant: its name and fields in order |
| `Option<T>` | `null` for `none`, else the value |
| `Result<T, E>` | `ok(value)` or `err(error)`; a plain value is read as `ok` |
| `Data` | its natural value (`null`, booleans, numbers, strings, lists, maps) |
| `native.Handle` | the module's handle number |

## How calls run

Each function of a module has a mode:

- **inline**: runs on the calling worker, without the runtime lock, and
  must return promptly (computation, parsing, a quick lookup);
- **blocking**: runs on the runtime's blocking pool while the calling task
  waits (file and database work); cancellation waits for it to finish, as
  for `extern "c blocking"`;
- **async**: the module completes the call later from any thread (an event
  loop, a server accepting connections). Cancelling the waiting task tells
  the module (its `cancel` hook) and discards the late completion.

## Reproduction bundles

`badger run --record` captures a program's external operations so a replay
reproduces it without reaching the world. A module says, per function,
how bundles treat it:

- **recorded**: an external operation identified by its first *k*
  arguments (the rest are what it sends), filed under a kind (`fetch`
  files a request as `fetch GET https://…`). A recording keeps its result;
  a replay answers it from there, and `--compare` reports a call that
  sends different content. An async call the task stopped waiting for (a
  timeout cancelled it) is recorded as such, and its replay waits until
  the task is cancelled again.
- **pure**: no effect outside the call; recordings and replays run it.
- otherwise **uncaptured**: a recording notes the limitation, and a
  replay stops (status 3) at the first call rather than run it for real.

## Failures

A module reports expected failures as `err` values: a `native.Error` with a
`kind` (such as `:sqlite/constraint`) and a `message`, or any declared error
type with those fields. Everything else is a **defect of the calling
task** — contained by `capture`, a scope, or supervision like any other —
never undefined behavior:

- the module panics;
- its result is malformed or does not fit the declared type;
- it reports `err` where the declared result is not a `Result`;
- the module or the function does not exist;
- a handle of one module is passed to another.

## Handles

`native.Handle` names state the module owns: a connection, a server, a
compiled template. The module returns one; the program passes it back.
When the program can no longer reach a handle, the host calls the module's
`drop_handle` (off the collector, on its own thread). Modules should also
offer an explicit `close`, since collection is not prompt. Standard modules
wrap handles in typed structs (`sqlite.Pool<S>`), so region brands and
capability rules apply to them as to any value.

## Finding modules

A module is either linked into the `badger` executable (the standard library's
own modules: `badger_sqlite`, `badger_fetch`, and `badger_http`, behind
`std.sqlite`, `std.fetch`, and `std.http`) or loaded from `libNAME.so` (`libNAME.dylib` on macOS) in the
first of these directories that has it:

1. each directory of `BADGER_NATIVE_PATH` (separated like `PATH`);
2. `lib/badger` next to the `badger` executable, then `../lib/badger`;
3. the executable's own directory, and its `deps` (cargo builds).

A library is loaded once, at its first call; a missing module or function
is a defect at the call that needs it.

## Writing a module in Rust

The `badger_native` crate has the ABI and helpers. A module is a `cdylib`
(add `rlib` to link it into an executable too):

```toml
[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
badger_native = { path = "…/crates/badger_native" }
```

```rust
use badger_native::{Args, Error, Handler, ModuleDef, Outcome, Value};

fn add(a: &Args) -> Outcome {
    a.int(0)?.checked_add(a.int(1)?).map(Value::Int).ok_or_else(|| Error::new("demo/overflow", "too big"))
}

pub static DEF: ModuleDef = ModuleDef {
    name: "demo",
    functions: &[("add", Handler::Inline(add))],
    drop_handle: None,
    cancel: None,
    recorded: &[],   // (function, kind, identifying arguments)
    pure: &["add"],
};

badger_native::export!("demo", DEF);
```

`Handler::Blocking(f)` marks a function for the blocking pool, and
`Handler::Async(f)` gives it a `Completion` to finish from any thread.
`Handles<T>` is a thread-safe table for module-owned state. Panics are
caught and reported as defects. `crates/badger_native_demo` exercises every
part of the ABI.

## The ABI

For implementers in other languages. A module exports
`badger_native_v1_NAME`, called once with the host's table:

```c
typedef struct Call Call;                  /* opaque, owned by the host */
typedef struct {
    uint32_t abi_version;                  /* 1 */
    const uint8_t *(*args)(Call *, size_t *len);  /* encoded argument list */
    void (*complete)(Call *, const uint8_t *result, size_t len);  /* once */
} Host;
typedef struct {
    const char *name;
    uint32_t mode;         /* 0 inline, 1 blocking, 2 async */
    uint32_t record;       /* 0 uncaptured; k + 1 recorded by its first k arguments; 0xFFFFFFFF pure */
    const char *record_kind;  /* bundle kind of a recorded call, or NULL for "native MODULE.FUNCTION" */
} FnDesc;
typedef struct {
    uint32_t abi_version;
    const char *name;
    const FnDesc *fns; size_t nfns;
    void (*call)(const Host *, Call *, size_t index);
    void (*drop_handle)(uint64_t);         /* may be NULL */
    void (*cancel)(Call *);                /* may be NULL */
} Module;
const Module *badger_native_v1_NAME(const Host *host);  /* NULL: unsupported ABI */
```

`call` must eventually call `complete` exactly once for the call (before
returning, for inline and blocking functions). A result is a status byte
(`0` value, `1` defect followed by a message) and a value. Values are a
tag byte and a payload, little-endian, with `u32` lengths and counts:

| Tag | Value | Payload |
|---|---|---|
| 0 | unit | |
| 1 | null | |
| 2, 3 | false, true | |
| 4 | integer | `i64` |
| 5 | float | `f64` |
| 6 | character | `u32` scalar value |
| 7 | string | length, UTF-8 |
| 8 | symbol | length, UTF-8 |
| 9 | bytes | length, bytes |
| 10 | list | count, values |
| 11 | map | count, (string, value) pairs |
| 12 | variant | name string, count, values |
| 13 | handle | `u64` |
