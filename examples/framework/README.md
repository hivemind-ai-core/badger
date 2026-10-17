# Tasks: an application built from providers and stages

A small task tracker with three entry points over one feature. It shows how
an application is put together: the feature defines typed interfaces,
`provider` declarations build the parts, `stage` declarations add checks
and cross-cutting behavior, and each entry point composes the parts it
needs in a `system { ... }`.

| File | Role |
|---|---|
| `src/tasks/model.bdg` | The feature's types: `Task`, its errors, the `Store` it needs, and the `Tasks` interface it offers |
| `src/tasks/memory.bdg` | `memory.store`: an in-memory `Store` (another adapter would provide the same interface) |
| `src/tasks/service.bdg` | `service.tasks`: the operations, with title validation as a stage |
| `src/web.bdg` | `web.api`: the HTTP binding, a route table behind request-id and access-log stages |
| `src/api.bdg` | The HTTP service (the default entry), and tests calling the API in-process |
| `src/cli.bdg` | The command line: typed options over the same feature, without a server |
| `src/worker.bdg` | A supervised background activity owned by a node |

Run them from this directory:

```sh
badger run                                   # serve on 127.0.0.1:8080 ([config] in badger.toml)
TASKS_PORT=9000 badger run                   # configuration from the environment
badger run cli -- --add "Write the docs" --add "Ship it" --done 1
badger run worker
badger test                                  # the API tests, without opening a port
badger inspect plans .                       # each plan's nodes and startup order
```

With the service running:

```sh
curl -X POST localhost:8080/tasks -d '{"title": "Write the docs"}'
curl localhost:8080/tasks
curl -X POST localhost:8080/tasks/1/done
curl -X POST localhost:8080/tasks -d '{"title": ""}'    # 400: a task needs a title
```

What to notice:

- The feature never names HTTP, the command line, or a particular store.
  Each binding depends on the `Tasks` interface; `ref(#tasks)` in a plan
  connects them, and startup follows those references.
- Validation is a stage of the feature's own pipeline, so every entry point
  gets it. The HTTP stages (request id, access log) wrap only the HTTP
  binding.
- The CLI and the worker select a smaller plan (no server); the tests run
  the service's plan without its server and call the handler directly.
- Leaving `system.run` halts every node in reverse order, also on an error.

See [chapter 8 of the guide](../../docs/guide/08-systems-and-pipelines.md)
for systems, providers, and stages, and
[`examples/design/framework`](../design/framework) for the descriptor-based
baseline this grows from.
