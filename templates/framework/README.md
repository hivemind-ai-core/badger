# @name@

An application with the shape of a framework: a store, a service over it,
and the routes that bind the service to HTTP.

- `src/store.bdg` — what a record is, what can go wrong, and the
  interface the service needs from storage (with an in-memory adapter).
- `src/service.bdg` — the service: the operations every caller shares.
- `src/routes.bdg` — the HTTP binding of those operations.
- `src/main.bdg` — the entry: it composes the three.

```sh
badger check    # check the program
badger run      # answer a few requests in-process
badger test     # run its tests
```

Next: give the store a real adapter (SQLite, Postgres) and add the
entries the application needs (a command line, a worker).
