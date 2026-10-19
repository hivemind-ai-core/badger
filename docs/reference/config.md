# Configuration

Three sources configure a Badger program:

1. `badger.toml`, the project manifest.
2. Environment variables: `BADGER_*` for the toolchain and runtime, and your
   own, read by `std.config` and `std.env`.
3. Command-line flags ([cli.md](cli.md)).

## `badger.toml`

A directory with a `badger.toml` is a project. Commands given that
directory (or run inside it with no path) read it.

```toml
[package]
name = "shop"
version = "0.3.0"

[runtime]
workers = 4

[config]
port = 8080
db = { path = "shop.db", pool = 4 }

[profile.prod]
port = 80

[entries]
admin = "src/admin.bdg"

[tasks]
seed = "src/tasks.bdg#seed"
```

Rules for the whole file:

- It is TOML, checked exactly: duplicate keys, duplicate sections, and a
  key reused as a table are errors. Manifest errors are E0010, with the
  line.
- Unknown sections and unknown keys are errors. The sections are
  `[package]`, `[toolchain]`, `[entries]`, `[tasks]`, `[dependencies]`,
  `[workspace]`, `[runtime]`, `[config]` (and `[config.TABLE]`),
  `[profile.NAME]` (and `[profile.NAME.TABLE]`, `[profile.NAME.runtime]`),
  `[database]`, and `[env]`.
- A quoted key names the same key (`"workers" = 2`). A quoted key that
  contains `.` is an error: write a dotted key or a table.
- Relative paths in the manifest are relative to the project directory.
- The manifest applies when the project is run as a directory (`badger run`,
  `badger run .`, `badger run DIR`, `badger run ENTRY_NAME`). A source file
  run by its path (`badger run src/main.bdg`) runs without `[config]`,
  profiles, `[runtime]`, and `[env]`, even inside a project. `badger check
  FILE` still checks a file as part of its project.

### `[package]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `name` | string | required | Package name. Only ASCII letters, digits, `_`, and `-` (it names files: saved programs, releases). Required unless the manifest has `[workspace]`. |
| `entry` | string | `"src/main.bdg"` | The main entry module. Its directory is the root of the package's modules. |
| `version` | string | `"0.0.0"` | Package version. A dependent's `version` requirement is matched against it. |
| `edition` | string | `"0"` | Language edition. This toolchain accepts only `"0"`. |
| `kind` | `"application"` or `"library"` | `"application"` | A library is used by other packages and has no `main`: `badger run` refuses it (E0010), `badger check` and `badger test` work. |
| `restricted` | `true` or `false` | `false` | Only the entry module may use ambient authority (functions that reach the filesystem, processes, the environment, databases, or the network by name, and what is built on them); other modules get capabilities from it. Violations are E0170. |

Modules of a package import each other with the package name as prefix:
`import shop.report` loads `src/report.bdg` when the entry is in `src/`.

### `[toolchain]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `language` | string | | The language version the project is written in. This toolchain accepts only `"draft-0"`; any other value is an error. |

### `[entries]`

Additional entry modules, each run by name.

```toml
[entries]
admin = "src/admin.bdg"
```

`badger run admin` runs the `main` function of `src/admin.bdg`. An entry
function takes no parameters or the arguments as `List<String>` (E0012
otherwise). `badger check` and `badger test` cover every entry. A file or
directory named like the entry in the current directory takes precedence
over the entry name.

### `[tasks]`

User tasks: typed functions the tool runs (`badger task NAME -- ARGS`).

```toml
[tasks]
seed = "src/tasks.bdg#seed"
```

The value is `"FILE#FUNCTION"`. The function takes no parameters or a
`List<String>` of the arguments. `badger check` and `badger test` cover
every task module.

### Dependencies

`[dependencies]` names other packages, each as an inline table. Every
value inside it is a string.

```toml
[dependencies]
textutil = { path = "../textutil", version = "0.2" }
json = { git = "https://example.com/json.git", rev = "v1.2", subdir = "lib" }
```

| Key | Meaning |
|---|---|
| `path` | Package directory, relative to this package. |
| `git` | Repository URL or local path, instead of `path`. |
| `rev` | With `git`: a tag, branch, or commit. Default: the repository's default branch. |
| `subdir` | With `git`: the package's directory inside the repository. |
| `version` | Required version: a prefix of the dependency's `[package] version` (`"0.2"` accepts `0.2.1`). |

Each dependency needs exactly one of `path` and `git`; `rev` and `subdir`
are only for `git`. Dependencies resolve transitively.

Code imports a dependency's entry module as `import textutil` and its
other modules as `import textutil.words`.

**`badger.lock`** sits beside the manifest (at the workspace root in a
workspace). One line per dependency: name, version, source (a relative
path, or `git+URL?rev=REV@COMMIT`, with `#SUBDIR`), and a content hash of
its manifest and sources:

```text
# Generated by `badger update`: dependency versions, sources, and content hashes.
mathlib 1.0.0 ../mathlib fnv1a64:7cc6f80259ac5732
textutil 0.2.1 ../textutil fnv1a64:4591e147c08b93df
```

- A build writes the lockfile when it does not exist. A project with no
  dependencies has no lockfile (`badger update` removes a stale one).
- A build refuses a dependency that changed, is missing from the
  lockfile, or is no longer used, until `badger update` rewrites it.
- Git dependencies build from their locked commit, so builds repeat and
  need no network once cached. `badger update` resolves `rev` again.
- Repositories are cloned into `$BADGER_HOME`, else
  `$XDG_CACHE_HOME/badger`, else `~/.cache/badger`; each commit used is
  checked out into its own directory there.

### `[workspace]`

```toml
[workspace]
members = ["app", "libs/*"]
```

| Key | Type | Meaning |
|---|---|---|
| `members` | list of strings | Member package directories. A final `/*` (or `*` alone) names every directory below that path that has a `badger.toml`. |

The members share one `badger.lock` at the workspace root.
`badger check`, `badger test`, and `badger update` at the root cover
every member. The root manifest may also have a `[package]`; then it is a
member too. A dependency name that resolves to two different packages in
one workspace is an error.

### `[runtime]`

Runtime settings. Every value is validated: an invalid value is error
E0010 in the manifest, E0013 in the environment or on the command line.

| Key | Values | Default | Meaning |
|---|---|---|---|
| `workers` | 1 to 64 | 1 | Worker threads that run tasks. |
| `cleanup_grace_ms` | integer, 0 or more | 5000 | How long a scope waits for cancelled tasks to finish their cleanups. Past it, the waiting task fails with a defect (`cleanup exceeded the grace period`) and the unfinished tasks keep their resources. |
| `task_stack_bytes` | 262144 (256 KiB) to 1073741824 (1 GiB) | 8388608 (8 MiB) | Address space reserved for each task's stack. Running past it is a task defect (`stack overflow`). |
| `blocking_threads` | 1 to 4096 | 16 | Threads of the blocking pool, which runs `extern "c blocking"` calls and blocking native-module functions while the calling task waits. |
| `heap_max_bytes` | integer, 0 or more | 0 (no limit) | Budget for live managed data. When an allocation would pass it, a full collection runs; if live data still exceeds it, the allocating task fails with the defect `heap budget exceeded`. |
| `http_max_body_bytes` | integer, 0 or more | 16777216 (16 MiB) | Default limit on an HTTP request body (larger: 413). |
| `http_max_header_bytes` | 1024 or more | 65536 (64 KiB) | Default limit on HTTP request headers (larger: 431). |
| `http_max_in_flight` | 1 or more | 1024 | Default limit on requests handled at once; more are answered 503 without running the handler. |
| `http_max_stream_buffer_bytes` | 1024 or more | 1048576 (1 MiB) | Bytes of a streamed reply that may wait for a slow client; beyond it the streaming task waits. |
| `trace_capacity` | 1 or more | 1024 | Events the trace sink holds. |
| `trace_overflow` | `"drop_oldest"`, `"drop_newest"` | `"drop_oldest"` | Which event a full trace sink drops. Losses never block; they are reported by `trace.stats()` and on stderr at exit. |
| `log_level` | `"trace"`, `"debug"`, `"info"`, `"warn"`, `"error"`, `"off"` | `"info"` | Lowest `std.log` level written. |
| `log_format` | `"text"`, `"json"`, `"none"` | `"text"` | How `std.log` writes records to stderr. `"none"` writes nothing there (an attached exporter, such as `std.otel`, still receives them). |

The HTTP limits are defaults for servers that do not pass their own
`http.Limits`.

Each setting is taken from the first of these that sets it:

1. `badger run --workers N` (and `compile`, `release`, `start`) for `workers`;
2. the environment variable `BADGER_` + the key in upper case
   (`BADGER_WORKERS`, `BADGER_HEAP_MAX_BYTES`), validated the same way;
3. `[profile.NAME.runtime]`, with `--profile NAME`;
4. `[runtime]`;
5. the default.

`badger inspect runtime [PATH] [--profile NAME]` prints each effective
value and its source. A program saved by `badger compile` or `badger
release` carries the settings it was saved with; the environment and
`--workers` still apply when it starts.

### `[config]` and `[profile.NAME]`

Application configuration, read by `config.load` (see
[std.config](#reading-configuration-stdconfig)).

```toml
[config]
port = 8080
hosts = ["a.example", "b.example"]
token = "${SHOP_TOKEN:-dev-token}"
db = { path = "shop.db", pool = 4 }

[profile.prod]
port = 80

[profile.prod.db]
path = "/var/lib/shop/shop.db"
```

- Keys form dotted paths: `db = { path = ... }`, `db.path = ...`, and
  `path = ...` under `[config.db]` all set `db.path`.
- `[profile.NAME]` (and `[profile.NAME.TABLE]`) overlay `[config]` key by
  key when the program runs with `--profile NAME`. An unknown profile name
  is error E0013. A single file has no profiles.
- Values are kept as written (a string's contents without quotes) and
  converted to the field's type when the program reads them. A list value
  is read as JSON (`["a", "b"]`).
- A string value may read the environment: `"${NAME}"` (the variable must
  be set), `"${NAME:-default}"` (the default when unset or empty). `$${`
  is a literal `${`. Expansion happens when `config.load` reads the value.
- `[profile.NAME.runtime]` is not configuration: it overrides `[runtime]`.
- `[profile.NAME.config]` is an error (E0010): a profile overlays
  `[config]` directly, so its keys go in `[profile.NAME]`.

`badger inspect config [PATH] [--profile NAME]` prints the merged values
as written (before `${...}` expansion and environment overrides), each
with its source:

```sh
$ badger inspect config --profile prod
port = 80    # from [profile.prod]
hosts = ["a.example", "b.example"]    # from [config]
token = ${SHOP_TOKEN:-dev-token}    # from [config]
db.path = /var/lib/shop/shop.db    # from [profile.prod]
db.pool = 4    # from [config]
```

### `[database]`

| Key | Type | Meaning |
|---|---|---|
| `schema` | string | SQL file (relative to the project) that literal SQL statements given to `std.sqlite` are prepared against at check time (E0180 to E0182), and the default `--schema` of `badger generate sql`. See [typed-sql.md](../typed-sql.md). |

### `[layers.NAME]`

Import rules between the package's modules. `forbid` lists module paths
that the module `NAME` and every module below it (`NAME.*`) may not
import, directly or as a module below one of them. A forbidden import is
E0017 at the import. Standard modules are named as imported
(`std.http`).

```toml
# the feature's core knows nothing of HTTP or of a particular store
[layers.orders.service]
forbid = ["web", "orders.sqlite_store", "std.http", "std.sqlite"]
```

### `[env]`

| Key | Type | Meaning |
|---|---|---|
| `dotenv` | string, or list of strings | Environment files loaded before the program starts. |

```toml
[env]
dotenv = [".env", ".env.local"]
```

- Files are relative to the project directory; a listed file that does
  not exist is skipped.
- Lines are `NAME=value`. Blank lines and lines starting with `#` are
  ignored; an `export ` prefix is allowed. `"..."` values take `\n` and
  `\"` escapes; `'...'` values are literal; an unquoted value ends at ` #`.
- A variable already set in the environment keeps its value; earlier
  files win over later ones.
- A malformed line is error E0013 with the file and line.

## Reading configuration: `std.config`

| Function | Reads |
|---|---|
| `config.load<T>(prefix)` | The effective `[config]` (with the selected profile), overlaid by environment variables `PREFIX` + field path. |
| `config.from_env<T>(prefix)` | Environment variables `PREFIX` + field path only. |

Both return `Result<T, config.Error>` and have the `Io` effect. `T` is a
record type; in a `restricted` project both are ambient authority.

Each field of `T` maps to a configuration key and an environment variable:

| Field | `[config]` key | Environment variable (prefix `SHOP_`) |
|---|---|---|
| `port` | `port` | `SHOP_PORT` |
| `db.path` (nested record) | `db.path` | `SHOP_DB_PATH` |

- The environment variable wins over the configured value.
- A missing field is an error naming both places to set it, except an
  `Option` field, which becomes `none`.
- Conversion by field type: `String` as is; `Bool` from `true`, `false`,
  `1`, `0`; integers and floats parsed; a duration (`Duration`,
  `Instant`, `Timestamp`) from the duration spelling the language uses
  (`10s`, `500ms`, `2us`, `1ns`, `5min`, `1h`) or from a plain number of
  milliseconds; other types (lists, records) from JSON text, or as a
  string when it is not JSON.
- `config.Error` has `key` (the configuration key or environment variable
  that failed) and `message`.
- Environment reads by `std.config` are recorded in reproduction bundles
  (`badger run --record`) like `env.get`: a replay sees the recorded values,
  not the current environment. Values of secret-like names (containing
  `SECRET`, `TOKEN`, `PASSWORD`, `API_KEY`, ...) are not stored, and a
  replay that reads one stops with status 3.

```badger
import std.io
import std.config

type Settings = {
    port: Int,
    hosts: List<String>,
    token: String,
    debug: Option<Bool>,
    db: { path: String, pool: Int },
}

pub enum AppError {
    config(source: config.Error),
    io(source: io.Error),
} derive(Error, From)

pub fn main() -> Result<Unit, AppError> {
    let s = config.load<Settings>("SHOP_")?
    io.println($"port={s.port} hosts={s.hosts} token={s.token} debug={s.debug}")?
    io.println($"db={s.db.path} pool={s.db.pool}")?
    ok(())
}
```

With the `[config]` above and a `.env` file holding `SHOP_DB_POOL=8`
(`[env] dotenv = ".env"`):

```sh
$ badger run
port=8080 hosts=["a.example", "b.example"] token=dev-token debug=none
db=shop.db pool=8
$ badger run --profile prod
port=80 hosts=["a.example", "b.example"] token=dev-token debug=none
db=/var/lib/shop/shop.db pool=8
$ SHOP_PORT=9000 SHOP_DEBUG=true SHOP_TOKEN=s3cret badger run
port=9000 hosts=["a.example", "b.example"] token=s3cret debug=some(true)
db=shop.db pool=8
$ SHOP_PORT=x badger run
error[config]: config: SHOP_PORT: expected an integer, found `x`
  caused by: SHOP_PORT: expected an integer, found `x`
  details: {"config":{"source":{"key":"SHOP_PORT","message":"expected an integer, found `x`"}}}
  at ./src/main.bdg:18:13
```

## Environment variables

### Settings

| Variable | Meaning |
|---|---|
| `BADGER_WORKERS`, `BADGER_CLEANUP_GRACE_MS`, `BADGER_TASK_STACK_BYTES`, `BADGER_BLOCKING_THREADS`, `BADGER_HEAP_MAX_BYTES`, `BADGER_HTTP_MAX_BODY_BYTES`, `BADGER_HTTP_MAX_HEADER_BYTES`, `BADGER_HTTP_MAX_IN_FLIGHT`, `BADGER_HTTP_MAX_STREAM_BUFFER_BYTES`, `BADGER_TRACE_CAPACITY`, `BADGER_TRACE_OVERFLOW`, `BADGER_LOG_LEVEL`, `BADGER_LOG_FORMAT` | Override the `[runtime]` setting of the same name (see [`[runtime]`](#runtime)). An invalid value is error E0013. |
| `BADGER_HOME` | Cache directory for git dependencies (default `$XDG_CACHE_HOME/badger`, else `~/.cache/badger`). |
| `BADGER_NATIVE_PATH` | Directories (`:`-separated) searched first for native module libraries (`libNAME.so`), before `lib/badger` and `../lib/badger` next to the executable and the executable's own directory. See [native-modules.md](../native-modules.md). |
| `BADGER_EXE` | The `badger` executable that `std.compiler` runs for its queries. `badger` sets it to itself when it is not set. |
| `BADGER_PROFILE=FILE` | Sample CPU time and write collapsed stacks to FILE at exit, with a summary on stderr. Covers compilation too. See [debugging.md](../debugging.md#profiling). |
| `BADGER_PROFILE_SCOPE=runtime` | With `BADGER_PROFILE`: sample only the program's run, not compilation. |
| `BADGER_PERF_MAP=1` | Write `/tmp/perf-PID.map` so `perf` names generated functions. |
| `BADGER_TRACE` | Set (any value): also write trace events to stderr (`trace NAME PAYLOAD`). |
| `BADGER_HTTP_ANNOUNCE` | Set: print `listening on http://ADDR` on stderr when an HTTP server starts. |
| `BADGER_SHUTDOWN_AFTER_MS=N` | A running system's `wait_for_shutdown()` returns after N milliseconds instead of waiting for SIGINT or SIGTERM. |
| `BADGER_SCHED_SEED=N` | Seed the controlled scheduler that `std.test` uses in tests. |
| `DATABASE_URL` | Default `--database` of `badger generate sql`. |
| `RUST_BACKTRACE=1` | Add a backtrace to an internal-error report (exit 70). |

### Debugging and diagnostics

For working on Badger itself. Output goes to stderr unless noted.

| Variable | Effect |
|---|---|
| `BADGER_TIMINGS=1` | Print each compiler phase's duration (`[timings] typecheck: 12.345 ms`); `parse.deps` (dependency resolution and verification) and `parse.std` (standard modules) are parts of `parse`, and `typecheck.env`, `.groups` (inference), `.after-groups` (deferred ownership checks), `.schemes`, and `.effects` are parts of `typecheck`. `BADGER_TIMINGS=2` also lists the slowest binding groups. |
| `BADGER_RUNTIME_STATS=1` | Print per-worker lock and scheduling counters when the program ends. |
| `BADGER_NO_INLINE` | Set: turn off MIR inlining and the passes after it in optimized builds. |
| `BADGER_NO_CONTRACT_ELISION` | Set: skip static contract analysis; every contract check stays in the code. |
| `BADGER_DEBUG_INFO` | Set: register debug information, as `badger run --debug`. |
| `BADGER_DEBUG_IMAGE=DIR` | Also write each debug-information image to `DIR/badger-jit-PID.elf`. |
| `BADGER_DUMP_CLIF` | Set: print the Cranelift IR of every generated function. |
| `BADGER_DUMP_ASM=PATTERNS` | Print machine code of generated functions whose names contain one of the comma-separated patterns; `1` or empty for all. |
| `BADGER_DEBUG_INSTANCE` | Set: print each instance as it is lowered, and report diagnostics from instance re-checks (normally suppressed). |
| `BADGER_DEBUG_LIVE` | Set: print the entries compared on each live reload. |
| `BADGER_DEBUG_SCHED` | Set: print scheduler state when it goes idle. |
| `BADGER_DEBUG_PANIC_WORKER` | Set: worker thread 1 panics at start (tests the internal-error path). |
| `BADGER_GC_STRESS=1` | Collect before every allocation (exposes missing roots). |
| `BADGER_GC_VERIFY=1` | Verify the heap at every collection; freed objects are poisoned and quarantined. |
| `BADGER_GC_GENERATIONAL=0` | Every collection is a full one. |
| `BADGER_CHAOS=SEED` | Perturb scheduling to expose races: random task budgets, random choice of ready tasks, yields and short sleeps around runtime entry and exit, collections at random allocations. Reproducible from the seed on one worker. |

Only in builds with the `badger_runtime` feature `gc-profile`:

| Variable | Effect |
|---|---|
| `BADGER_GC_PROFILE=FILE` | Write collector phase measurements to FILE (JSON). |
| `BADGER_ALLOC_PROFILE=FILE` | Write an allocation census by type and source location to FILE (JSON). |
| `BADGER_GC_MINOR_BYTES=N` | Bytes allocated between collections (default: half the old generation, clamped to 16 to 256 MiB). |
| `BADGER_GC_MAX_MINORS=N` | Consecutive minor collections before a full one. |
| `BADGER_GC_TRIM=0` | After full collections, keep empty memory chunks instead of releasing them. |
| `BADGER_GC_POOL_MAX=N` | Largest object size served from the size-class pools: 256, 512 (default), or 1024. |

Read by the test suite only (`cargo test`):

| Variable | Effect |
|---|---|
| `BADGER_TEST_POSTGRES_URL` | A Postgres database the `postgres` tests may change (otherwise they start a throwaway cluster with `initdb`, or skip). |
| `BADGER_TEST_VALKEY_URL` | A Valkey/Redis server whose database 0 the `valkey` tests may flush (otherwise they start `valkey-server` or `redis-server`, or skip). |
| `BADGER_FUZZ_SECS=N`, `BADGER_FUZZ_SEED=N` | Fuzz test duration and seed. |
| `BADGER_STRESS_SECS=N`, `BADGER_STRESS_SEED=N` | Stress soak duration and starting seed. |
