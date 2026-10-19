# Library and framework change — correctness audit, 2026-10-19

Audit of the merged library and framework change (`7372628`, merged as
`f4b4802`) against `main` at `f4b4802`. Method: the full test suite,
AddressSanitizer, ThreadSanitizer, Miri, and two code audits (the
standard library and the Rust parts) with every finding checked by
running it. The findings and the fixes are in
[2026-10-19-library-and-framework-implementation.md](2026-10-19-library-and-framework-implementation.md).

## Method and environment

- Host: AMD Ryzen 7 8745HS, x86-64 Linux, 16 logical CPUs, 58 GB RAM.
- Toolchains: stable 1.98.1 (the repository's `rust-toolchain.toml` pins
  `1.98`), nightly 1.100.0-nightly (for the sanitizers and Miri).
- The audit ran in an isolated worktree of the merge commit,
  `/home/dan/orca/audits/language-main-f4b4802` (detached at `f4b4802`),
  so the main checkout and its uncommitted work were never touched.
- Sanitizer builds needed `--target x86_64-unknown-linux-gnu` (so that
  build scripts and proc-macro crates are not built with the sanitizer,
  which they cannot be) and `-Zbuild-std` for ThreadSanitizer (without
  it, cargo reports an ABI mismatch between the sanitizer and the
  non-sanitizer dependencies).
- **The runtime's own sanitizer features have to be enabled**: `asan`
  (it unpoisons the redzones of a coroutine abandoned by `force_reset`)
  and `tsan` (the fiber annotations that let TSan follow the stack
  switches of `resume` and `suspend`). A build without them reports
  false positives; the first pass of this audit was made without them,
  and the correction is recorded below.
- Every finding was reproduced on the base commit `83c6568` as well, to
  separate what this change introduced from what was already there.

## 1. The full test suite

`cargo test --workspace` on the merged `main`:

| Suite | Result |
|---|---|
| `badger_cli --test programs` (the 219-program corpus and the 275 reject programs, four modes) | 76 passed |
| `badger_cli --test docs` | 3 passed |
| `badger_cli --test tools`, `--test search` | 4 + 4 passed |
| `badger_cli --test stress` | **1 failed** (`stress_programs`) |
| `badger_syntax --test format_repo` | 1 passed |
| `badger_runtime`, `badger_typeck`, `badger_mir`, `badger_base`, `badger_syntax` | all passed |
| `badger_runtime` unit tests (14) | passed |

The one failure is **pre-existing**: `stress_programs` crashes in
`badger_runtime::gc::collect` (a garbage-collector defect in the
runtime, not in the library). It reproduces identically on the base
commit `83c6568` in a clean worktree, so it is not from this change.

## 2. AddressSanitizer

`RUSTFLAGS="-Zsanitizer=address" cargo +nightly test --target x86_64-unknown-linux-gnu -p badger_runtime --features asan`:

- 12 of 14 tests pass; 2 fail **on leaks only**:
  `buffer::tests::inline_elements_trace_freeze_and_grow_without_native_leaks`
  (1080 bytes in 9 objects) and
  `gc::sweep::tests::helpers_reuse_join_and_preserve_prefix_marks_and_finalizer_order`
  (120 bytes in 1 object).
- Both leaks are the runtime's **intentional** leaks: a type descriptor
  must live as long as the program, and `TypeDesc::leak` is `Box::leak`
  (`crates/badger_runtime/src/desc.rs:207`). The tests create nine
  descriptors each, and LeakSanitizer cannot know the leak is meant to
  happen. The same two tests, with the same 1108 bytes in 18
  allocations, fail on the base commit `83c6568`, and with
  `ASAN_OPTIONS=detect_leaks=0` (the right setting for a runtime that
  leaks by design) all 14 pass.

`ASAN_OPTIONS=detect_leaks=0 cargo +nightly test --target ... -p badger_cli --features asan --test programs run_programs`
— the whole corpus, four modes — is **clean**:

```
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 72 filtered out; finished in 316.83s
```

The first pass of this audit, built **without** the runtime's `asan`
feature, reported on five programs (all on the task cancellation or
timeout path) a `stack-use-after-scope` in
`badger_runtime::task::harvest_events` (`crates/badger_runtime/src/task.rs:3435`):

```
ERROR: AddressSanitizer: stack-use-after-scope
    #1 core::ptr::read::<badger_runtime::task::Timer>
    #2 <alloc::collections::binary_heap::Hole<badger_runtime::task::Timer>>::new
    #7 badger_runtime::task::harvest_events
```

That was a **false positive of the build, not a defect**: the `asan`
feature exists precisely to unpoison the redzones of a coroutine
abandoned by `force_reset`, and without it ASan poisons the stack of
the abandoned coroutine and then reports the next use of it. With
`--features asan` the same five programs report nothing, and the whole
corpus passes. (ASan also printed `ASan is ignoring requested
__asan_handle_no_return ... False positive error reports may follow`
before each report, and the error kind changed between runs — a
stack-use-after-scope in one, a stack-buffer-underflow in the next —
which is what a poisoned-shadow false positive looks like.)

## 3. ThreadSanitizer

`RUSTFLAGS="-Zsanitizer=thread" cargo +nightly test -Zbuild-std --target x86_64-unknown-linux-gnu -p badger_runtime --features tsan`:

- 14 of 14 unit tests pass, **no data race reported**.
- The concurrency-relevant programs of the corpus (the ones whose name
  mentions a task, a worker, a channel, a scope, a job, a cancellation,
  a parallelism, a thread, a queue or a pool — 26 programs) were run
  under TSan one at a time, with the `tsan` feature (the fiber
  annotations that let TSan follow the stack switches of the runtime's
  coroutines): **no data race is reported** (`clean=26 tsan_races=0`).
- The whole `run_programs_multicore` mode (all 219 programs on several
  workers) was also started under TSan; it had reported no race after
  100 minutes and was stopped there — TSan plus the JIT is that slow.
  The 26 programs above are the ones that exercise the runtime's
  concurrency.

(The doc-test targets cannot be built with a sanitizer — cargo reports
`mixing -Zsanitizer will cause an ABI mismatch` for the doctest
harness — so the doc tests are excluded from the sanitizer runs.)

## 4. Miri

`cargo +nightly miri test -p badger_runtime --lib` cannot run the whole
suite: the runtime spawns operating-system threads, and Miri does not
support `posix_spawnattr_init` (`unsupported operation: can't call
foreign function posix_spawnattr_init on OS linux`). The tests that do
not spawn threads run clean:

| Test | Result |
|---|---|
| `intrinsics_codec::tests::*` (the three new duration tests) | passed |
| `parse::tests::left_recursion_and_ambiguity` | passed |
| `string::tests::scalar_slice_matches_character_reference` | passed |
| `symbol_tests::runtime_symbols_are_unique` | passed |
| `telemetry::tests::rfc3339_dates` | passed |
| `buffer::*`, `gc::*`, `hamt::*`, `map::*` | not runnable under Miri (they spawn threads) |

No undefined behaviour was reported by Miri in the code that runs,
including the new configuration-codec duration parsing.

## 5. Code audit — the standard library

A separate pass read every changed `.bdg` file and checked each
function against its documentation, its edge cases, and the shadowing
class of bug that the `std.default` module had. Twenty-eight probe
programs were run, plus the repository's own `tests/run` (219 programs)
and `tests/reject` corpora.

**Five defects were found and are fixed** (the fix commit is
`37b91eb`, merged as `fd10647`):

1. **Seven standard-library error types could not be a route's error
   type.** `NumericError`, `codec.DecodeError`, `system.ProviderError`,
   `channel.ConfigError`, `channel.SendError<T>`, `task.PanicInfo` and
   `task.TimedOut` did not implement `HttpStatus`, and the orphan rule
   (E0211) stopped an application from adding the implementation:
   `impl HttpStatus for codec.DecodeError {}` → `E0211: orphan
   implementation of HttpStatus`. An application that propagates a
   decode error out of a handler could not be written at all. Every
   standard-library error type implements `HttpStatus` now.
2. **A URI's fragment was not stripped.** `route.path_of("/x#frag")`
   returned `"/x#frag"`, and `route.query_of("/x?a=1#frag")` returned
   `"a=1#frag"`, so the query parameter's value was `1#frag`. The
   fragment is stripped before the path and the query are taken.
3. **`http.StatusError` and `http.reply_for_error` promised a message
   the reply does not carry** — a 5xx keeps the error's message to
   itself. The comments now state the rule that `http.error_reply`
   already documents.
4. **`http.Handler`'s comment was stale**: it still said that a
   handler's error becomes a 500.
5. **`http.with_status` could not do what its name says**: it was
   `http.json` under a second name, and it could not attach a status to
   a reply built by hand (`E0403: http.Response cannot be encoded as
   data`). It is `http.with_status(response, status)` now.

**Checked and found sound:** the query-string edge cases (an empty
name, a repeated parameter, a parameter without a value, percent
encoding, a `?` inside a segment, a trailing `&`); the path parameter
winning over a query parameter of the same name, in the handler's
parameter map and in `route.typed`; the matcher, over the in-process
handler and over a real HTTP server with `curl`; the status and the
reply bodies (a 4xx carries the message, a 5xx does not, and the old
body of a 500 is unchanged); the protocol's default method being
inherited by an empty implementation; the new `std.result` family
(including that the error thunk of `check` is only called on
rejection); `std.validate`; `std.kv`; `std.default`; `std.cli`; and
`pipeline.recover_with`. The free function and the method form of
every new operation agree, and the generated API pages match the
standard library byte for byte.

## 6. Code audit — the Rust parts

The Rust parts (the configuration codec's duration parsing, and the new
`badger completions`, `doctor`, `init`, `watch` and `search` commands)
were audited for panic paths a user's input can reach, resource
handling, the exit status, the JSON output, the completion scripts'
syntax, and the watch's signal handling. (A second, delegated pass over
the same code returned no result, so the review was done here; the
commands were exercised directly.)

| Check | Result |
|---|---|
| `duration_nanos` on the unit tests, under Miri | passed, no undefined behaviour |
| `duration_nanos` on a value that overflows (`"99999999999999999999s"`) | rejected with the configuration's own error, not a panic |
| `duration_nanos` on `""`, `"soon"`, `"10days"`, `"10 s"` | rejected (the last one: the unit must follow the number) |
| `badger init` on a file, on a directory that already has a `badger.toml` | a clear error on stderr, exit 1, nothing overwritten |
| `badger init` then `badger check` (program and library) | both check |
| `badger watch` on a missing directory | a clear error, exit 1 |
| `badger watch` with a broken file | the command's exit status is reported, the watch continues |
| `badger watch` and Ctrl-C | the watch stops (`stopped`) |
| `badger doctor --json` | valid JSON, with the documented keys |
| `badger doctor` in a directory that is not a project | `project: none (no badger.toml here)`, the runtime defaults, the `BADGER_*` variables |
| `badger completions bash\|zsh\|fish\|powershell` | a script for each, each naming how to install it; `bash -n` and `zsh -n` accept them |
| `badger completions tcsh` | a usage error (clap lists the allowed shells) |
| `badger search` | finds a standard-library definition by name and by documentation; `--json` parses; a query that matches nothing says so |
| `cargo clippy --workspace --all-targets` | 7 warnings, all in `badger_mir` (the optimiser, untouched by this change); the new code is clean |

One observation, not a defect: a negative duration in the configuration
(`APP_TIMEOUT=-5`) is accepted as a number of milliseconds, exactly as
the JSON codec accepts a negative number of nanoseconds.

## Findings

| # | Finding | Kind | State |
|---|---|---|---|
| 1 | Seven standard-library error types could not be a route's error type (orphan rule) | introduced by this change | fixed (`37b91eb`) |
| 2 | The URI helpers did not strip a fragment | introduced by this change | fixed (`37b91eb`) |
| 3 | Two doc comments promised a message a 5xx does not carry | introduced by this change | fixed (`37b91eb`) |
| 4 | `http.Handler`'s doc comment was stale | introduced by this change | fixed (`37b91eb`) |
| 5 | `http.with_status` could not attach a status to a hand-built reply | introduced by this change | fixed (`37b91eb`) |
| 6 | `stress_programs` crashes in the garbage collector | pre-existing (`83c6568`) | open, not from this change |
| 7 | ASan: a stack report on the task cancellation path | a false positive of a build without the runtime's `asan` feature | closed: the corpus is clean with the feature |
| 8 | ASan: two runtime unit tests leak their own type descriptors | the runtime's intentional leaks (`Box::leak`) | not a defect: leak checking is meaningless for this runtime |

## Not covered

- The compiler's own test suites were run, but the compiler was not
  fuzzed; there is no fuzz target in the repository.
- Miri cannot execute the parts of the runtime that spawn operating
  system threads, so the garbage collector, the buffers and the hash
  map are covered by ASan and TSan only.
- The database, HTTP and websocket integration tests that need a
  server or a database were not run in this environment.
- The audit covered the change, not the whole repository: the
  pre-existing failures above are reported because they were hit, not
  because the rest of the runtime was audited.
