# Go/Elixir design report: what was done (2026-10-17)

Status of each recommendation in
[2026-10-16-go-elixir-alternative-design-report.md](2026-10-16-go-elixir-alternative-design-report.md)
after work order item 10A. "Deferred" items are planned for the 1.0 work
after the milestone tag (the services framework, FRAMEWORK_DESIGN.md
A1–A5, and the enhancement list), not dropped.

## Syntax

| Item | Status |
|---|---|
| S1 stabilize everyday syntax | Done. Expression lambdas (`fn(x) => e`), `if let`/`while let`, `?` in first-argument pipes; the formatter is idempotent over the repository (tested). |
| S2 block form of `use` | Not added. The bounded-`use` helper idiom is documented instead (the report makes the syntax conditional on a reading study). |
| S3 library-shaped service APIs | Kept: no actor or route syntax. Diagnostics say why a closure cannot cross tasks (E0402). |
| S4 one API convention | Partly: options records, `Result` constructors, and scoped `use` are uniform across the new libraries. A written convention guide belongs to the 1.0 docs. |

## Language semantics

| Item | Status |
|---|---|
| M1 effects, captures, package trust | Done. The `is_std` exemption audit removed the broad exemptions; the rest are named trusted boundaries (`TRUSTED_SINGLE_USE`, `OWNERSHIP_TRUSTED_MODULES`, `STORING_CONSTRUCTORS`). Effect-generic types (`Seq<T, Eff>`, `Stream<T, Eff>`, pipeline `Eff`) closed the hole where std function fields had untracked effects and captures (a `Seq` iterator could be shared between tasks). |
| M2 cancellation and resources | Done for the named kinds: `tests/run/cancel_matrix.lang` covers subprocesses (killed), SQLite (interrupted), blocked file reads (abandoned), channel waits, and completion/cancel races; native async calls (withdrawn, handles released) are covered by `tests/native`. The contract is in spec chapter 05. |
| M3 typed services and supervision | Deferred to the framework (A1–A5). Existing: managed workers, restart budgets, escalation through the system graph. |
| M4 overload and boundedness | Partly: channel capacity and byte budgets, HTTP admission limits (413/431/503). Reusable admission permits are deferred (L7). |
| M5 concurrency diagnostics | Done for three always-deadlocking patterns: E0450 (join before close), E0451 (join before send), E0452 (sends past capacity before any receiver). Runtime deadlock reports name each blocked task. |
| M6 compatibility and specialization | Done: recursion past the specialization limit is a compile-time warning (E0147), not a runtime defect. Wire compatibility rules for `Codec` are unchanged. |

## Runtime

| Item | Status |
|---|---|
| R1 native resource boundary | Done: versioned native module ABI (`lang_native`), opaque handles released through the module, inline/blocking/async calls, record/replay hooks. SQLite, HTTP, fetch, Postgres, Valkey, formats, crypto, compression, datetime, and fs are native modules. |
| R2 cancellation-aware I/O | Done for HTTP (hyper/tokio; bounded admission); blocking work uses a bounded pool. |
| R3 scheduler | Continued under the lock-optimization slices; not part of this item. |
| R4 GC and memory | Not part of this item (see the memory optimization plan). |
| R5 fast language libraries | Not part of this item (see archive/plans/OPTIMIZATION_PLAN.md). |
| R6 deployable artifact | Done: `lang release` writes a self-contained directory (executable, binary MIR, native libraries, lockfile, digests); `lang start` refuses changed or incompatible parts before running. See docs/releases.md. |

## Core library

| Item | Status |
|---|---|
| C1 common vocabulary | Done: `io.Reader`/`io.Writer` over files, stdin/stdout, and memory; `io.copy`, `read_all`, line readers. |
| C2 collections and streams | Done for streams: `close` propagates to the source; `take`, `take_while`, `batches`, `try_map`, `try_each`, `try_collect` (closing on error), `fold`, `drain`, and `collect_at_most` (bounded; `collect` is documented as unbounded). Bounded parallel map is deferred (L7); data structures are deferred (S4). |
| C3 errors, time, observability | Done before this item: structured errors with codes and causes, `std.datetime`, `std.log`, spans/metrics, `std.otel`. |
| C4 testing and docs | Done before this item: deterministic clocks and schedules, property tests, `lang doc`, docs/api checked in tests. Conformance suites for adapters are deferred. |

## Extended libraries (section 8)

Done in work order item 6: HTTP (server, client, SSE, websockets), SQLite
and PostgreSQL with typed SQL (`lang generate sql`), Valkey, JSON/TOML/YAML/
CSV, templates, compression, logging and OpenTelemetry. Durable jobs remain
in `std.jobs`.

## Delivery slices (section 9)

A–D and H are covered by the work above. E (slow-client measurements),
F (supervised toolkit), G (integration pilot), I (three-stack comparison),
and J are deferred to the 1.0 work.
