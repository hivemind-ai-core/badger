# Draft language programs

These are worked examples for [draft 0](../../SPEC.md). They check and run
with `badger`. The original examples have repository integration coverage;
the framework baseline has focused tests run with the commands in its README.

For a tour of individual language features, start with the
[numbered examples](../README.md).

- [normalize.bdg](normalize.bdg): CLI input, sequences, paged collection, errors.
- [batch_jobs.bdg](batch_jobs.bdg): paged messages and two fallible scoped tasks.
- [prefix_views.bdg](prefix_views.bdg): later updates visible through earlier
  interceptor schemas, with no visibility of later-only fields.
- [service/src/main.bdg](service/src/main.bdg): a complete proposed service
  entry module using the standard-library contracts in the specification.
- [framework/README.md](framework/README.md): a runnable modular-service design
  baseline with typed interfaces, isolated fake storage, system/pipeline
  composition, lifecycle failure checks, and seven focused tests.

The service requires `ORDERS_DATABASE_PATH`, `ORDERS_ADDRESS`, and `ORDERS_PORT`
(decoded by `config.from_env`). It accepts `POST /orders` with a JSON body such
as `{"title":"Prepare shipment"}`:

```text
ORDERS_DATABASE_PATH=orders.db ORDERS_ADDRESS=127.0.0.1 ORDERS_PORT=8080 \
    badger run examples/design/service
```

[Semantic acceptance cases](../../spec/08-worked-examples.md) describe what the
implementation must accept, reject, and observe at runtime, and which tests
cover them.
