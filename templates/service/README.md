# @name@

An HTTP service written in Badger.

- `src/service.bdg` — the service: a typed set of operations
  (`stdx.app`) over an in-memory store.
- `src/routes.bdg` — the routes that bind the operations to HTTP
  (`stdx.http`, `stdx.route`). The errors carry their status, so the
  service's own errors answer with the status the service means.
- `src/main.bdg` — the entry: it composes the service and answers a few
  requests in-process.

```sh
badger check    # check the program
badger run      # answer a few requests in-process
badger test     # run its tests
```

To serve it for real, bind the routes to a server (`http.server`) and
wait for the shutdown:

```badger
let plan = system {
    notes: service.notes(),
    api: http.server({ address: "127.0.0.1", port: 8080, handler: ref(#api) }),
}
use running <- app.run(plan, app.health([]))
running.wait_for_shutdown()?
```

Next: replace the in-memory store with a real one (SQLite, Postgres), and
the operations with the ones the service really offers.
