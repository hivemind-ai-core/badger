# Modular service baseline

This is a runnable example for the [service-first framework design](../../../FRAMEWORK_DESIGN.md). It uses the current language and standard library; it does not implement the proposed `app.*` framework APIs or provider/stage syntax.

Run from the repository root:

```text
target/release/lang check examples/design/framework
target/release/lang run examples/design/framework
target/release/lang test examples/design/framework --json
```

Expected program output:

```text
created 1: first order
store.start -> orders.start -> orders.stop -> store.stop
```

The [public interface](src/orders/api.lang) defines input, output, errors, and explicitly effect-bounded service/store functions. The project enables restricted mode: ambient output belongs to the entry module. The [service](src/orders/service.lang) validates and normalizes input through a pipeline before calling its injected store. Its audit interceptor observes the latest value of a field that it introduced earlier in the chain.

The [fake store](src/orders/fake_store.lang) creates independent state per fixture. Its `Atom.update` callback is pure and may be retried; no external effect runs inside it. The [providers](src/providers.lang) wire the typed interfaces into a system and register cleanup. [main.lang](src/main.lang) contains the demo and seven focused tests:

- Creation through a fake-backed service.
- Validation failure without a store write.
- Isolated fake state.
- Dependency-ordered startup and reverse shutdown.
- Cleanup after partial startup failure.
- Cleanup after an operation error.
- Bounded retry of a supervised worker factory.

The fake is an in-memory demonstration, not durable persistence. The example does not exercise an HTTP listener, authentication, transactions, an outbox, external effects, or process-crash recovery. Those have separate acceptance gates in the framework design.
