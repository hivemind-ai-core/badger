# Draft language programs

These are worked examples for [draft 0](../../SPEC.md). They check and run
with `lang`, and the test suite executes each of them.

For a tour of individual language features, start with the
[numbered examples](../README.md).

- [normalize.lang](normalize.lang): CLI input, sequences, paged collection, errors.
- [batch_jobs.lang](batch_jobs.lang): paged messages and two fallible scoped tasks.
- [prefix_views.lang](prefix_views.lang): later updates visible through earlier
  interceptor schemas, with no visibility of later-only fields.
- [service/src/main.lang](service/src/main.lang): a complete proposed service
  entry module using the standard-library contracts in the specification.

The service requires `ORDERS_DATABASE_PATH`, `ORDERS_ADDRESS`, and `ORDERS_PORT`
(decoded by `config.from_env`). It accepts `POST /orders` with a JSON body such
as `{"title":"Prepare shipment"}`:

```text
ORDERS_DATABASE_PATH=orders.db ORDERS_ADDRESS=127.0.0.1 ORDERS_PORT=8080 \
    lang run examples/design/service
```

[Semantic acceptance cases](../../spec/08-worked-examples.md) describe what the
implementation must accept, reject, and observe at runtime, and which tests
cover them.
