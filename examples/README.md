# Language examples

These programs explore the [draft language](../SPEC.md). Each numbered file is
an independent entry module; run one with `lang run examples/01_basics.lang`
(arguments after `--`). The test suite runs all of them.

| Example | Demonstrates |
| --- | --- |
| [01_basics.lang](01_basics.lang) | Inference, functions, closures, generics, expressions, loops, effects |
| [02_records.lang](02_records.lang) | Open rows, immutable updates, nested updates, qualified labels, projection, nominal construction |
| [03_domain_types.lang](03_domain_types.lang) | Private fields, ordinary constructors, protocols, enums, exhaustive matching |
| [04_errors_and_use.lang](04_errors_and_use.lang) | Named errors, direct error conversion, early return, continuation syntax |
| [05_collections.lang](05_collections.lang) | Paged builders, lazy sequences, chunk iteration, arrays, buffers, maps, sets |
| [06_schemas.lang](06_schemas.lang) | Typed schemas, byte decoding, explicit dynamic data, domain validation |
| [07_tasks.lang](07_tasks.lang) | Concurrent file reads, scoped children, result-aware joining, ordinary task values |
| [08_channels.lang](08_channels.lang) | Prompt bounded communication, immutable sharing, explicit close, failure coordination |
| [09_batch_channels.lang](09_batch_channels.lang) | Paged message batches, producer order, partial-tail publication, leased views |
| [10_interceptors.lang](10_interceptors.lang) | Typed stages, live prefix views, context writeback, reverse unwind, recovery |
| [11_systems.lang](11_systems.lang) | Providers, typed dependency references, function-record interfaces, ordered cleanup |
| [12_import_notes.lang](12_import_notes.lang) | A file-to-SQLite automation with a transaction and managed database lifetime |

Start with the first five files for everyday syntax. The
[larger worked examples](design/README.md) include an HTTP/SQLite service with
configuration, validation, interceptors, and lifecycle management.

The tasks example expects two text-file paths. The notes importer expects one
text-file path and creates or updates `notes.sqlite` in the current directory.
Its repeated runs append notes; it does not deduplicate them. The other
numbered examples need no command-line input.

The interceptor example deliberately exercises both success and recovery:
later stages update fields an earlier stage already knows, while later-only
fields remain inaccessible to that earlier stage. Recovery skips leave callbacks
of stages already popped in error mode.

[smoke.lang](smoke.lang) is the smallest complete program. Compiler/runtime acceptance
obligations and their tests are listed in
[the semantic cases](../spec/08-worked-examples.md).
