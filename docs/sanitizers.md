# Sanitizer builds

The runtime runs every task on its own stack (corosensei coroutines), so
the sanitizers need to be told about stack switches. Two cargo features
do that; build with a nightly toolchain and the standard library rebuilt
under the same sanitizer.

## ThreadSanitizer

```sh
RUSTFLAGS="-Zsanitizer=thread" CARGO_TARGET_DIR=target/tsan \
  cargo +nightly build -Zbuild-std --target x86_64-unknown-linux-gnu \
  -p lang_cli --release --features tsan
TSAN_OPTIONS="halt_on_error=0 report_thread_leaks=0" \
  target/tsan/x86_64-unknown-linux-gnu/release/lang run --workers 4 PROGRAM
```

The `tsan` feature makes each coroutine a TSan fiber
(`__tsan_create_fiber`) and announces every switch
(`__tsan_switch_to_fiber`) immediately before corosensei's own `resume`
and `suspend`, so the per-fiber shadow call stacks stay balanced; a
coroutine that returns is left on the worker after `resume`, as its fiber
is then destroyed. Worker threads still running at exit are reported as
"thread leaks" (they are detached on purpose): `report_thread_leaks=0`.
Generated code is not instrumented: TSan sees the runtime's
synchronization, not races inside compiled programs (those are ruled out
by the type system's `Share`/`Transfer` rules).

## AddressSanitizer

```sh
RUSTFLAGS="-Zsanitizer=address" CARGO_TARGET_DIR=target/asan \
  cargo +nightly build -Zbuild-std --target x86_64-unknown-linux-gnu \
  -p lang_cli --release --features asan
ASAN_OPTIONS=detect_leaks=0 target/asan/x86_64-unknown-linux-gnu/release/lang run PROGRAM
```

A task cancelled or failed in the middle of its frames is abandoned
(`force_reset`), leaving those frames' redzones poisoned; the `asan`
feature unpoisons a stack before it is reused. ASan still warns once that
it ignores `__asan_handle_no_return` for an unrecognized stack; the
warning is expected. Leak detection is off: the collector frees managed
memory in bulk at exit, not object by object.

## Scripts

The audit runs every `tests/run` program under ASan (comparing output
with the `.out` file) and every concurrent program and `tests/stress`
program under TSan with four workers; a report lists each program with a
sanitizer finding.
