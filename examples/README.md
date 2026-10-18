# Language examples

These programs explore the [draft language](../SPEC.md). Each numbered file is
an independent entry module; run one with `badger run examples/01_basics.bdg`
(arguments after `--`). The test suite runs all of them.

| Example | Demonstrates |
| --- | --- |
| [01_basics.bdg](01_basics.bdg) | Inference, functions, closures, generics, expressions, loops, effects |
| [02_records.bdg](02_records.bdg) | Open rows, immutable updates, nested updates, qualified labels, projection, nominal construction |
| [03_domain_types.bdg](03_domain_types.bdg) | Private fields, ordinary constructors, protocols, enums, exhaustive matching |
| [04_errors_and_use.bdg](04_errors_and_use.bdg) | Named errors, direct error conversion, early return, continuation syntax |
| [05_collections.bdg](05_collections.bdg) | Paged builders, lazy sequences, chunk iteration, arrays, buffers, maps, sets |
| [06_schemas.bdg](06_schemas.bdg) | Typed schemas, byte decoding, explicit dynamic data, domain validation |
| [07_tasks.bdg](07_tasks.bdg) | Concurrent file reads, scoped children, result-aware joining, ordinary task values |
| [08_channels.bdg](08_channels.bdg) | Prompt bounded communication, immutable sharing, explicit close, failure coordination |
| [09_batch_channels.bdg](09_batch_channels.bdg) | Paged message batches, producer order, partial-tail publication, leased views |
| [10_interceptors.bdg](10_interceptors.bdg) | Typed stages, live prefix views, context writeback, reverse unwind, recovery |
| [11_systems.bdg](11_systems.bdg) | Providers, typed dependency references, function-record interfaces, ordered cleanup |
| [12_import_notes.bdg](12_import_notes.bdg) | A file-to-SQLite automation with a transaction and managed database lifetime |
| [13_rest_service.bdg](13_rest_service.bdg) | A database-backed REST service with typed routes, validation, a background stats worker, and in-process tests |

Start with the first five files for everyday syntax. The
[larger worked examples](design/README.md) include an HTTP/SQLite service with
configuration, validation, interceptors, and lifecycle management.

[framework/](framework/README.md) is a small application built the way the
[framework design](../FRAMEWORK_DESIGN.md) describes: one feature with typed
interfaces, `provider` and `stage` declarations, and three entry points (an
HTTP API, a command line, and a supervised worker) that each compose the
parts they need. Its tests call the API in-process.

The tasks example expects two text-file paths. The notes importer expects one
text-file path and creates or updates `notes.sqlite` in the current directory.
Its repeated runs append notes; it does not deduplicate them. The other
numbered examples need no command-line input.

The interceptor example deliberately exercises both success and recovery:
later stages update fields an earlier stage already knows, while later-only
fields remain inaccessible to that earlier stage. Recovery skips leave callbacks
of stages already popped in error mode.

[smoke.bdg](smoke.bdg) is the smallest complete program. Compiler/runtime acceptance
obligations and their tests are listed in
[the semantic cases](../spec/08-worked-examples.md).
