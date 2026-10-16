# Worked programs and semantic acceptance cases

Status: draft 0; the programs and acceptance cases below are executed by the
test suite (see [test status](#test-status)). See [the specification index](../SPEC.md).

## Complete program sketches

The [example directory](../examples/README.md) contains a numbered language tour
and larger proposed entry modules using the documented standard-library APIs.
All of them check and run with the implementation in this repository.

### CLI normalization

[normalize.lang](../examples/design/normalize.lang) reads an argument and file,
constructs a lazy sequence of nonempty trimmed lines, collects into a paged
list, and prints each line. Expected I/O errors are returned as `AppError`;
absence of an argument is an explicit domain error. No runtime lookup of record
field types or custom application lifecycle class is required.

### Batch communication

[batch_jobs.lang](../examples/design/batch_jobs.lang) owns a channel and two
child tasks. Producer messages are accepted into paged staging and published
on limits/scheduling boundaries. Explicit close flushes accepted messages and
ends the consumer after drain. The consumer processes leased pages before
release. `try_join` observes both fallible results and settles the other task
before returning an error; joining one first would be an unsafe general pattern
if the other can fail while it is blocked on the queue.

If consumer I/O fails inside a batch, its affine lease is released during scope
cleanup. Cancellation releases unaccepted reservations but not ownership of an
already accepted message. The channel's enclosing scope performs the declared
close/disposal policy. In-memory acceptance is not durable job acknowledgment.

### Prefix-visible interceptor state

[prefix_views.lang](../examples/design/prefix_views.lang) adds `verified: Bool`
in an early stage, updates it later, and checks the updated value in the early
leave. The later `token: String` is unavailable to that early stage. A terminal
failure after the later update would expose the same updated `verified` to its
error callback. Its diagnostic reads `true` after that update, or `false` if
the later enter failed before committing. It does not assume the update happened.

### Service with a scoped transaction

[service/src/main.lang](../examples/design/service/src/main.lang) loads typed
configuration, starts SQLite, constructs a typed interceptor handler, and then
starts HTTP. Input decode/validation adds `input: OrderInput`; an earlier timer
stage cannot access that later-only field. The terminal function starts a
transaction, inserts with bound parameters, decodes the returned row, and
builds the reply before the transaction helper commits.
An earlier error interceptor maps malformed input to 400 and internal errors
to a generic 500 reply; it cannot read the later-only decoded input field.

A failed commit returns an error even if reply construction succeeded. On
shutdown, HTTP stops admission and settles requests before SQLite closes. If
HTTP startup fails, the already started database and provider scopes clean up.
The typed system graph supplies dependency injection; the schema-directed codec
handles external dynamic data; ordinary request work uses direct calls inside
its own task.

## Static acceptance cases

| ID | Case | Required result |
|---|---|---|
| N01 | `type person = ...` | Reject lowercase type name |
| N02 | `let Count = 1` | Reject uppercase value name |
| N03 | `enum Reply { ok(value: Int) }` | Accept lowercase variant under uppercase type |
| R01 | Field-reading function called with extra fields | Accept and retain concrete shape |
| R02 | A required field is missing/misspelled | Reject with required/actual shape and source |
| R03 | Annotation `{x: Int}` receives `{x, y}` | Reject implicit field loss; suggest open row or projection |
| R04 | Equal closed shapes in different field order | Same structural type and internal layout |
| R05 | Exact record to transparent public struct | Accept if types, ownership, and representation permit |
| R06 | Record construction of a private-invariant struct | Reject outside defining authority |
| R07 | Dotted update preserves nested unknown fields | Accept with row-preserving result |
| R08 | Branch adds a field on only one path | Reject incompatible shapes unless explicitly modeled |
| R09 | Runtime symbol indexes a heterogeneous record | Reject; require explicit dynamic boundary |
| O01 | Affine buffer used after successful send | Reject use after move |
| O02 | Shared immutable message used after send | Accept; payload may be shared |
| O03 | A record containing `Cell<T>` sent to another task | Reject task-confined capture/transfer |
| O04 | Closure containing a transaction returned from its scope | Reject region escape |
| O05 | Resource hidden in `Data` to escape its region | Reject encoding/escape |
| P01 | Later stage reads earlier introduced field | Accept |
| P02 | Earlier leave reads later-only field | Reject even when final shape contains it |
| P03 | Later stage changes the type of an earlier field | Reject at composition |
| P04 | Leave updates a visible field at the same type | Accept; preserve unseen fields |
| P05 | Early stop skips its own unestablished fields/frame | Accept; unwind only established prefixes |
| S01 | Missing or wrongly typed system reference | Reject before initialization |
| S02 | Literal dependency cycle | Reject with cycle path |
| S03 | A scoped running system returned from `system.run` | Reject region escape |
| E01 | `?` has one declared direct error conversion | Accept |
| E02 | Two From-derived variants accept the same source error | Reject ambiguous derivation |
| E03 | IO call inside a `!{}` function | Reject effect bound violation |

## Runtime acceptance scenarios

1. **Live view, success:** stage A adds `verified = false`; stage B commits
   `verified = true` and `token`; A.leave sees `true` and cannot inspect `token`.
2. **Live view, failure:** terminal failure after B invokes B.error and A.error
   with latest committed values. Failure during B.enter does not invoke B.error
   or claim B's fields exist. Uncommitted value updates are not published.
3. **Live view, writeback:** B.leave changes an A-visible counter; A.leave reads
   it. Later-only fields survive writeback even though A cannot access them.
4. **Recovery:** B.error recovers with a reply; A.leave runs once. An error in
   B.leave is handled by A.error, never by calling B.leave/error repeatedly.
5. **Startup:** node C fails after A and B start; B then A halt, all attempts
   run, and cleanup failures attach to C's original initialization error.
6. **Request isolation:** request A fails while request B waits for its database;
   B continues, and A releases its transaction and task scope.
7. **Transaction:** callback succeeds but commit fails; the overall helper
   returns the commit error. Cancellation/rollback uncertainty is reported,
   not labeled a successful transaction.
8. **Task results:** one child returns `err` while a peer is blocked in a
   capacity-limited channel; `try_join` cancels and settles that peer promptly
   at safe points. Ordinary `spawn` still returns arbitrary typed values.
9. **Fairness:** an instrumented long loop cannot indefinitely prevent a ready
   timer or channel task from being scheduled. Foreign blocking work is isolated.
10. **Publication:** append then await a reply cannot strand the outbound message
    in staging. Capacity waits flush accepted data first. Receiver registration
    races never lose the notification that would make it runnable.
11. **Ownership:** producer exit after accepted append preserves queued data;
    failed/cancelled pre-commit reservations return/release their value once.
12. **Closure:** no end-of-stream is reported while accepted staged messages can
    still be published. Receiver discard releases each owned payload once.
13. **Page lifetime:** a retained read-only batch/list view prevents page reuse;
    sender task death and GC do not invalidate it. Releasing all views permits reuse.
14. **Storage:** append growth preserves existing builder element addresses;
    freezing a unique builder preserves page storage. Contiguous growth is
    allowed to relocate a buffer and must respect outstanding borrows.
15. **Multicore:** producer migration preserves per-producer order and detaches
    old thread-local staging. Different producers do not gain an invented total order.
16. **Reload:** compatible new calls use new code; suspended old tasks retain old
    code/shape metadata. Incompatible live layouts require explicit reconstruction.

## Test status

| Cases | Executable evidence |
|---|---|
| N01, N02, R02, R03, R06, R08, R09, O01, O03–O05, P02, P03, S01–S03, E02, E03 | `tests/reject/*.lang` (each must fail with its code) |
| N03, R01, R04, R05, R07, E01, P01, P04, P05 | `tests/run/acceptance_static.lang` |
| O02 | `tests/run/o02_shared_after_send.lang` |
| Scenarios 1–4 | `tests/run/live_views.lang`, `tests/run/prefix_views.lang` |
| Scenario 5 | `tests/run/scenario05_startup_failure.lang` |
| Scenario 6 | `tests/http/isolation.lang`, driven by `crates/lang_cli/tests/service.rs` |
| Scenario 7 | `tests/run/scenario07_commit_failure.lang`, `tests/run/sqlite_rollback_failure.lang` (a failed rollback is returned with the callback's error as its cause) |
| Scenario 8 | `tests/run/scenario08_try_join_cancel.lang` |
| Scenario 9 | `tests/run/scenario09_fairness.lang`; foreign blocking work: `tests/run/ffi_contracts.lang` |
| Scenarios 10–12 | `tests/run/scenario10_publication.lang`, `tests/run/batch_jobs.lang` |
| Scenarios 13–14 | `tests/run/scenario13_storage.lang` |
| Scenario 15 | `multicore_migration` (`tests/projects/multicore`, four workers); all `tests/run` programs also run with `--workers 4` |
| Scenario 16 | `live_reload` (compatible replacement, old frames keep their code, incompatible type and effect changes rejected) and `live_reconstruct` (explicit reconstruction on one and four workers) in `crates/lang_cli/tests/programs.rs` (`tests/live`) |

The worked programs run in `examples_run` and `orders_service`. Live-prefix
writeback is exercised across different concrete layouts, including early
error paths, in the live-view tests.

## Proof/measurement gates before implementation claims

The [benchmarks](../bench/README.md) measure field access and row
specialization, paged versus contiguous iteration, map operations, prompt
versus batch messaging, allocation and collection pauses, task spawn cost,
cancellation latency under backpressure, stack/root overhead, and shutdown
under failure. One run (2026-10-16, `lang run --release`,
AMD Ryzen 7 8745HS) measured:

| Measurement | Result |
|---|---|
| Closed record / struct field read in a loop | ≈1.0 ns per iteration |
| Row-generic reader called with a 6-field shape | ≈1.1 ns per call |
| Paged list append / iterate | 5 / 1 ns per element |
| Buffer push / array iterate | 4 / 2 ns per element |
| Persistent map insert (200k entries) / lookup | ≈410 ns / ≈93 ns |
| Prompt / batch channel throughput | 22 / 31 ns per message |
| Prompt round trip | ≈0.21 µs |
| Record allocation with a retained window | ≈39 ns per record; longest pause ≈2.9 ms |
| Task spawn and join | ≈0.56 µs per task |
| Cancelling 100 producers blocked on a full channel | ≈0.13 ms to settle (≈0.6 ms right after the allocation and spawn measurements) |
| Four CPU-bound tasks (`bench/parallel.lang`), 1 / 2 / 4 workers | 684 / 344 / 174 ms |
| Call with a scalar frame / call holding 4 rooted pointers that makes one more call (`bench/runtime.lang`) | ≈2 / ≈1.8 ns |
| Recursion 20,000 frames deep on a fresh task stack | ≈4 ns per frame |
| Shutdown under failure: body defect with 200 parked tasks and 8 resources | settled, all 8 halts run, in ≈0.2 ms |

These are single-run indications, not tuned results: stack sizes, page sizes,
collector thresholds, and scheduling budgets remain measured engineering
choices. `python3 bench/compare/run.py` compares workloads with other
languages, and `bench/telemetry.py` measures scaling across workers.

The original draft validation also checked that all local links resolve,
fenced code blocks balance, and every example parses against the reference EBNF
with an independent recognizer. The test suite now checks every `lang`
code block in the specification and guides (`crates/lang_cli/tests/docs.rs`;
excerpts are fenced `lang fragment`).
