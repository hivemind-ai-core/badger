# Tasks: an application built from providers and stages

A small task tracker with three entry points over one feature. It shows how
an application is put together: the feature offers typed operations
(`std.app`), `provider` declarations build the parts, `stage` declarations
add checks and cross-cutting behavior, the HTTP and command-line bindings
(`std.endpoint`, `std.command`) expose the same operations, and each entry
point composes the parts it needs in a `system { ... }`.

| File | Role |
|---|---|
| `src/tasks/model.bdg` | The feature's types: `Task`, its errors, the `Store` it needs, and the `Tasks` operations it offers |
| `src/tasks/memory.bdg` | `memory.store`: an in-memory `Store` (another adapter would provide the same interface) |
| `src/tasks/service.bdg` | `service.tasks`: the operations (one a pipeline with title validation as a stage) |
| `src/web.bdg` | `web.api`: each operation bound to a method and path, behind an access-log stage |
| `src/api.bdg` | The HTTP service (the default entry), and tests calling the API and the commands in-process |
| `src/cli.bdg` | The command line: a command per operation, with typed options and exit statuses, without a server |
| `src/worker.bdg` | A supervised background activity owned by a node |

Run them from this directory:

```sh
badger run                                   # serve on 127.0.0.1:8080 ([config] in badger.toml)
TASKS_PORT=9000 badger run                   # configuration from the environment
badger run cli -- add --title "Write the docs"
badger run cli -- help                       # the commands, from the operations' documentation
badger run worker
badger test                                  # the API tests, without opening a port
badger inspect plans .                       # each plan's nodes and startup order
```

With the service running:

```sh
curl -X POST localhost:8080/tasks -d '{"title": "Write the docs"}'
curl localhost:8080/tasks
curl -X POST localhost:8080/tasks/1/done
curl -X POST localhost:8080/tasks -d '{"title": ""}'    # 400 {"error":"tasks/invalid_title",...}
```

What to notice:

- The feature never names HTTP, the command line, or a particular store.
  Each binding depends on the `Tasks` interface; `ref(#tasks)` in a plan
  connects them, and startup follows those references.
- One operation implementation serves both transports. A binding states
  how input is decoded (path parameters and body, or options), how the
  output is rendered, and how each expected error is answered (a status
  and code, or an exit status); no error is turned into a reply implicitly.
- Validation is a stage of the create operation's pipeline, so every entry
  point gets it. The access-log stage wraps only the HTTP binding.
- Route names, URLs, OpenAPI summaries, and command help come from the
  operations' identities and documentation.
- The CLI and the worker select a smaller plan (no server); the tests run
  the service's plan without its server and call the handler directly.
- Leaving `system.run` halts every node in reverse order, also on an error.

See [chapter 8 of the guide](../../docs/guide/08-systems-and-pipelines.md)
for systems, providers, and stages, and
[`examples/design/framework`](../design/framework) for the descriptor-based
baseline this grows from.
