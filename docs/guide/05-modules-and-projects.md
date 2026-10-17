# 5. Modules and projects

How code is split into modules and packages: imports, visibility,
`badger.toml`, dependencies and `badger.lock`, workspaces, and a map of
the standard library.

## Modules

One file is one module. In a project, a file's path under the source
directory is its module path:

| File | Module |
|---|---|
| `src/main.bdg` | the entry module |
| `src/report.bdg` | `report` |
| `src/orders/store.bdg` | `orders.store` |

A single file run with `badger run file.bdg` is a program of its own;
modules it imports are looked up relative to its directory.

## Imports

```badger
import std.io
import std.text as t
import std.list.{sort, sum as total}

pub fn main() -> Result<Unit, io.Error> {
    io.println(t.to_upper("hi"))?
    io.println(sort([3, 1, 2]))?
    io.println(total([1, 2, 3]))?
    ok(())
}
```

```text
HI
[1, 2, 3]
6
```

| Form | Binds |
|---|---|
| `import std.io` | the module, as `io` (the last segment) |
| `import std.text as t` | the module, as `t` |
| `import orders.store.{label, OrderId}` | those items, unqualified (not the module) |
| `import orders.store.{label as order_label}` | an item under another name |

- The prelude (`Option`, `Result`, `ok`, `err`, `some`, `none`, `List`,
  `Map`, `Display`, `Error`, `panic`, `assert`, ...) is always in scope.
  Everything else is imported explicitly.
- Importing two different things under one name is an error (E0141).
  Rename one (`import b.{greet as greet_b}`) or use the module name
  (`b.greet`). Importing the same item twice is fine, and an import may
  shadow a prelude name.
- An unknown module is E0114; an unknown item is E0113; a private item is
  E0112.
- Modules may import each other in a cycle when the cycle has only types
  and functions. A module on an import cycle cannot define values
  (`let`, `const`): E0134.
- Importing a module runs no code. Module values are computed without
  effects (E0137), and cannot hold mutable state or resources (E0138).

## Visibility

Everything is private to its module unless marked `pub`.

```badger fragment
// src/orders/store.bdg
pub struct OrderId {
    value: Int,
}

pub fn order_id(value: Int) -> OrderId { { value } }

pub fn label(id: OrderId) -> String { $"order-{id.value}" }

pub const max_items: Int = 100

fn checksum(id: OrderId) -> Int { id.value % 97 }
```

```badger fragment
// src/main.bdg
import std.io
import orders.store
import orders.store.{label as order_label}

pub fn main() -> Result<Unit, io.Error> {
    let id = store.order_id(7)
    io.println(order_label(id))?
    // store.checksum(id)          E0112: `checksum` is private
    // id.value                    E0308: field `value` is private
    // let x: store.OrderId = { value: 1 }   E0315: private construction
    ok(())
}
```

```text
$ badger run
order-7
```

| Declaration | Visible outside its module |
|---|---|
| `pub fn`, `pub type`, `pub enum`, `pub protocol` | yes |
| `pub struct` | the type; each field only if it is also `pub` |
| `pub let` / `pub const` | yes; the type must be written (`pub const max_items: Int = 100`, E0118) |
| no `pub` | no (E0112) |

- Enum variants are as visible as their enum.
- A struct with a private field can be constructed or updated only in its
  own module (E0315). Expose constructor functions instead; they are where
  invariants are checked.
- `pub fn` signatures must be fully typed (E0118).

## Projects

A project is a directory with a `badger.toml`:

```text
inventory/
├── badger.toml
├── badger.lock        (written by the tool)
└── src/
    ├── main.bdg
    ├── report.bdg
    ├── tasks.bdg
    └── orders/
        └── store.bdg
```

```toml
[package]
name = "inventory"
version = "0.1.0"
entry = "src/main.bdg"

[dependencies]
textkit = { path = "../textkit", version = "1.2" }

[entries]
report = "src/report.bdg"

[tasks]
seed = "src/tasks.bdg#seed"
greet = "src/tasks.bdg#greet"
```

In the project directory, a missing path means "this project":

| Command | Does |
|---|---|
| `badger check` | checks every entry and task module |
| `badger run` | runs the `entry` module's `main` |
| `badger run report` | runs a named `[entries]` module |
| `badger run -- ARGS` | passes arguments (`env.args()`) |
| `badger task greet -- ada bo` | calls a `[tasks]` function |
| `badger test [path] [filter]` | runs `test_*` functions ([chapter 9](09-testing-and-contracts.md)) |
| `badger format` | rewrites sources in the canonical layout |
| `badger update` | resolves dependencies and rewrites `badger.lock` |
| `badger doc .` | Markdown reference for the project's public interface |

Entry and task functions take no parameters or one `List<String>` of
arguments (E0012):

```badger fragment
// src/tasks.bdg
import std.io

pub fn seed() -> Result<Unit, io.Error> {
    io.println("seeding")
}

pub fn greet(names: List<String>) -> Result<Unit, io.Error> {
    for name in names {
        io.println($"hello {name}")?
    }
    ok(())
}
```

```text
$ badger task greet -- ada bo
hello ada
hello bo
```

## `badger.toml` essentials

| Section | Keys | Purpose |
|---|---|---|
| `[package]` | `name` (required), `version` (default `0.0.0`), `entry` (default `src/main.bdg`), `kind` (`"application"` or `"library"`), `restricted` (`true`/`false`) | identity and entry point |
| `[dependencies]` | `NAME = { path = "..." }` or `{ git = "...", rev = "...", subdir = "..." }`, optional `version` | other packages |
| `[entries]` | `NAME = "src/file.bdg"` | more runnable modules |
| `[tasks]` | `NAME = "src/file.bdg#function"` | typed functions run by `badger task` |
| `[workspace]` | `members = ["app", "libs/*"]` | packages sharing one lockfile |
| `[config]`, `[profile.NAME]` | any keys | configuration data, overlaid by `--profile NAME` |
| `[runtime]`, `[profile.NAME.runtime]` | `workers`, `heap_max_bytes`, `log_level`, ... | runtime settings |
| `[env]` | `dotenv = ".env"` or a list | environment files loaded before the program starts |
| `[database]` | `schema = "schema.sql"` | the SQL schema typed queries are checked against |
| `[toolchain]` | `language = "draft-0"` | the language version this project targets |

- Unknown sections and keys are errors (E0010), so a typo cannot be
  ignored silently.
- `kind = "library"` marks a package that others import; it has no
  `main`, and `badger run` refuses it.
- `restricted = true` lets only the entry module use ambient authority
  (the filesystem, processes, environment, network, databases, by name);
  other modules receive capabilities as parameters (E0170). See
  [chapter 7](07-effects-and-ownership.md).
- Every runtime setting, profile overlay, and environment override is in
  the [configuration reference](../reference/config.md);
  `badger inspect runtime` and `badger inspect config` print the
  effective values.

## Dependencies and `badger.lock`

A dependency is a directory or a git repository holding another package.

```toml
[dependencies]
textkit = { path = "../textkit", version = "1.2" }
json = { git = "https://example.com/json.git", rev = "v1.2", subdir = "lib" }
```

- Import a dependency's entry module by its name (`import textkit`), and
  its other modules by path (`import textkit.words`). A `-` in the name
  becomes `_`.
- Inside a package, its own modules may also be written with the package
  name first (`import inventory.orders.store`).
- `version` is a prefix of the dependency's `[package] version`: `"1.2"`
  accepts `1.2.0` and `1.2.7`, and `"2"` rejects `1.2.0` (E0015).
- A git `rev` is a tag, branch, or commit. The repository is cloned once
  into the cache (`$BADGER_HOME`, else `$XDG_CACHE_HOME/badger`, else
  `~/.cache/badger`).

The first build writes `badger.lock`: each dependency's version, source,
and a content hash (git dependencies also record the commit).

```text
# Generated by `badger update`: dependency versions, sources, and content hashes.
textkit 1.2.0 ../textkit fnv1a64:3fadcb6998ecd8f7
```

Later builds verify it. When a dependency changes, the build stops until
you accept the change:

```text
$ badger run
Error: [E0015] dependency `textkit` changed since badger.lock was written (locked `textkit 1.2.0 ../textkit fnv1a64:3fadcb6998ecd8f7`, found `textkit 1.2.0 ../textkit fnv1a64:1230a8cd5600bc1e`); run `badger update`
$ badger update
locked textkit 1.2.0 (fnv1a64:1230a8cd5600bc1e)
```

Commit `badger.lock`. Git dependencies build from the locked commit, so
builds repeat and need no network once cached.

## Workspaces

A workspace groups packages that share one `badger.lock`.

```text
ws/
├── badger.toml          [workspace] members = ["app", "libs/*"]
├── badger.lock
├── app/
│   ├── badger.toml      [dependencies] util = { path = "../libs/util" }
│   └── src/main.bdg
└── libs/
    └── util/
        ├── badger.toml  kind = "library"
        └── src/lib.bdg
```

- `members` lists package directories; a final `*` means every package
  directory below that path.
- At the root, `badger check .`, `badger test .`, and `badger update .`
  cover every member. `badger run app` runs one member; `badger run .`
  refuses, because a workspace is not a package (E0010).
- The root `badger.toml` may also describe a package of its own.

## The standard library at a glance

Each module is imported as `std.NAME`. The [standard library
reference](../api/README.md) lists every type and function, generated
from the checked interfaces; `badger doc std.list` prints one module, and
`badger query api std.list` gives the same facts as JSON.

### Core

| Module | Contents |
|---|---|
| `std.prelude` | Imported implicitly: primitives, `Option`, `Result`, `Data`, `Cell`, `Atom`, `Box`, `Display`, `Error`, `From`, `panic`, `assert` |
| `std.option`, `std.result` | Combinators for `Option` and `Result` ([chapter 3](03-errors.md#combinators)) |
| `std.num` | Numeric conversions and checked/wrapping arithmetic |
| `std.text` | Unicode text: search, split, trim, pad, parse |
| `std.records` | Typed record operations: `project`, `merge`, `remove` |
| `std.time` | Monotonic clock, wall-clock timestamps, durations, `sleep` |
| `std.datetime` | Calendar dates, times, time zones, spans, `strftime`/`strptime`, RFC 3339 |
| `std.random` | Explicit random sources, seeded or OS-seeded |
| `std.uuid` | Random (v4) and time-ordered (v7) UUIDs |

### Collections

| Module | Contents |
|---|---|
| `std.list` | Paged immutable lists and builders |
| `std.map`, `std.set` | Persistent maps and sets, builders |
| `std.array` | Immutable fixed-length contiguous arrays |
| `std.buffer` | Mutable contiguous buffers, fixed-capacity buffers |
| `std.bytes` | Immutable byte storage |
| `std.seq` | Lazy sequences |
| `std.stream` | Suspending streams, for example over channels |

### Concurrency and structure

| Module | Contents |
|---|---|
| `std.task` | Task scopes, spawn, join, capture, cancellation, timeouts |
| `std.channel` | Bounded channels and batch channels |
| `std.broadcast` | Publish to every subscriber |
| `std.worker` | Restart policies for work that may fail |
| `std.system` | Lifecycle-managed systems of components |
| `std.pipeline` | Typed interceptor pipelines |

### Operating system

| Module | Contents |
|---|---|
| `std.io` | Standard input and output, readers and writers |
| `std.fs` | Files and directories, `fs.Dir` capabilities |
| `std.path` | Paths as strings |
| `std.env` | Arguments and environment variables |
| `std.process` | Subprocesses and process exit |
| `std.cli` | Typed command-line arguments from a record type, with generated help |

### Data formats

| Module | Contents |
|---|---|
| `std.codec` | JSON encoding and validating decoding through schemas |
| `std.schema` | Typed schemas and versioned wire contracts |
| `std.config` | Validated configuration from the environment and `[config]` |
| `std.toml`, `std.yaml`, `std.csv` | Those formats, as `Data` or typed values |
| `std.template` | Jinja-syntax templates |
| `std.markdown` | Markdown to HTML |
| `std.encoding` | Base64, hex, URL percent-encoding |
| `std.crypto` | Hashes, HMAC, constant-time comparison, secure random bytes |
| `std.compress` | gzip, zlib, deflate, zstd, with bounded decompression |
| `std.parse` | Grammar parsers with source spans and structured failures |
| `std.regex` | Regular expressions (linear-time matching) |

### Services and storage

| Module | Contents |
|---|---|
| `std.http` | HTTP server: request/reply handlers, streaming, server-sent events, WebSocket upgrades |
| `std.route` | Typed routing tables, URL construction, OpenAPI |
| `std.websocket` | WebSocket clients and upgraded server sockets |
| `std.fetch` | Outbound HTTP(S) requests and origin-bound clients |
| `std.sqlite` | Embedded SQLite with scoped transactions and typed rows |
| `std.postgres` | PostgreSQL pools and transactions |
| `std.valkey` | Valkey/Redis client with pub/sub |
| `std.jobs` | Durable single-host jobs on SQLite, with an outbox and leases |

### Observability and tooling

| Module | Contents |
|---|---|
| `std.log` | Structured logs, text or JSON lines |
| `std.trace` | Structured runtime events and spans |
| `std.metrics` | Counters, gauges, histograms |
| `std.otel` | OpenTelemetry export over OTLP/HTTP |
| `std.inspect` | Runtime snapshots of tasks, channels, systems, the heap |
| `std.test` | Assertions, controlled clock, seeded scheduling, property checks |
| `std.compiler` | Compiler services from code: diagnostics, hover, definitions |

### Foreign code

| Module | Contents |
|---|---|
| `std.ffi` | The C boundary: raw pointers, C memory, C strings |
| `std.native` | Native modules written in Rust or any C-ABI language ([guide](../native-modules.md)) |

## Documenting your code

`///` before a declaration is its documentation. `badger doc PATH`
renders the public interface of a program or project as Markdown, the
language server shows it on hover, and `badger query api PATH` returns it
as JSON.

```badger
/// The total price in cents, after `discount` percent off.
pub fn total(cents: Int, discount: Int) -> Int {
    cents * (100 - discount) / 100
}

pub fn main() -> Unit {
    assert(total(1000, 10) == 900)
}
```

---

Next: [Tasks and channels](06-tasks-and-channels.md)
