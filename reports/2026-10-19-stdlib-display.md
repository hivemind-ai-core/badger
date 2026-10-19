# Display and string interpolation — 2026-10-19

Status: implemented and measured on `main` (`b067f70`). Scope: the standard
library and runtime, excluding the garbage collector.

## Result

`value::display` built one `String` per record field, list element, enum
payload and set/map entry, collected them into a `Vec<String>`, and joined
them — so interpolating an aggregate allocated once per member plus the join.
It now appends into the caller's buffer (`write_display`); only map and set
entries still render into their own buffer, because entries are ordered by
their rendered text.

Interpolating the same values 200,000 times with `derive(Display)`, same host,
before and after:

| Value | Before | After | Change |
|---|---:|---:|---:|
| `{x, y}` record | 229–239 ns | 141–156 ns | −36% |
| 10-element `List<Int>` | 375–390 ns | 228–260 ns | −38% |
| nested struct (4 corners, map, string) | 1482–1530 ns | 539–552 ns | −64% |
| enum with payload | 134–144 ns | 80–86 ns | −40% |
| `Int` scalar | 50–53 ns | 53 ns | unchanged |

The scalar case is the output allocation itself and is unchanged; the
aggregate cases lose the per-member allocation and the join. The nested case
shows the effect when aggregates nest (each level previously materialised its
members).

## What changed

- `value::write_display` appends a value's display form to a `String`:
  records, tuples, enums and lists write members directly, with the same
  separators, quoting (`Kind::String`/`Kind::Char` are Debug-quoted when
  nested, raw at the top), `#symbol` form, `<N bytes>`, `<instant N>`,
  `..`/`..=` ranges, and the error-style `variant: payload` top-level form
  (including the `_` to space substitution).
- `value::display` keeps its signature: it runs a descriptor's `Display` hook
  when present (which is what `derive(Display)` installs) and otherwise reads
  the structural form through the shared buffer.
- `call_display_hook` is factored out for the hook path (top-level and
  nested), preserving the no-collection temporary context used when called
  outside a task.

## Verified

- Every `tests/run/*.out` golden output that interpolates values is
  byte-identical: `cargo test -p badger_cli --release --test programs`
  passes (76 tests), including the four `run_programs*` modes.
- `cargo test -p badger_runtime --release` passes.
- `codec_paths`, `schema_wire`, `routes_typed`, `routes_params`,
  `app_bindings`, `http_scoped`, `spec_ch03_snippets` and
  `collections_everyday` outputs are unchanged.

## Not done

- A single interpolated scalar still costs one `String` (the result), and the
  interpolation lowering calls the runtime per interpolated value; folding a
  whole `$"{a} {b}"` into one buffer would need codegen changes (the MIR
  owner's area).
- `fmt_float_of`, `fmt_duration` and the map/set entry rendering still return
  fresh `String`s.
