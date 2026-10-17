# Language guide

Ten chapters, each one topic. Read them in order the first time; after
that, jump to what you need. New to Badger? Read the [tour](../tour.md)
first.

| # | Chapter | Covers |
|---|---|---|
| 1 | [Values and functions](01-values-and-functions.md) | Literals, bindings, operators, functions, lambdas, pipes, control flow. |
| 2 | [Types and records](02-types-and-records.md) | Records, structs, enums, patterns, protocols, methods, generics. |
| 3 | [Errors](03-errors.md) | `Option`, `Result`, `?`, the `Error` protocol, defects. |
| 4 | [Collections](04-collections.md) | `List`, `Map`, `Set`, `Array`, `Bytes`, `Seq`, `Stream`. |
| 5 | [Modules and projects](05-modules-and-projects.md) | Imports, visibility, `badger.toml`, dependencies, the standard library. |
| 6 | [Tasks and channels](06-tasks-and-channels.md) | Task scopes, cancellation, timeouts, channels, broadcast. |
| 7 | [Effects and ownership](07-effects-and-ownership.md) | Effect rows, `Transfer` and `Share`, affine values, scoped regions. |
| 8 | [Systems and pipelines](08-systems-and-pipelines.md) | Typed systems with lifecycles, interceptor pipelines. |
| 9 | [Testing and contracts](09-testing-and-contracts.md) | `badger test`, `std.test`, `requires`/`ensures`, record and replay. |
| 10 | [Services](10-services.md) | HTTP, databases, configuration, logging, releases. |

The full API of every module is in the [standard library
reference](../api/README.md).
