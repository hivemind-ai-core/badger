# JSON decode and standard-library bulk copies — 2026-10-19

Status: implemented and measured on `perf/runtime-stdlib-2026-10-19` (base
`f85f6e8`). Scope: the runtime and standard library, excluding the garbage
collector (worked on in parallel).

## Result

Schema-directed JSON **decoding no longer builds a `serde_json::Value` tree**.
`codec::decode` now streams the text into the target value with a
descriptor-driven parser (`badger_runtime::json`), and the encoder writes
integers without `core::fmt`, quotes strings with a byte scan, checks wire
naming once per record, and reserves its output buffer. On the realworld
`json` workload (parse, enrich, serialize, parse; 400,000 lines) the run time
fell **50.4%** (minimum) and **49.4%** (median) over 21 paired samples.
Decode and encode microbenchmarks fell **58.0%** and **38.4%**.

| Measurement (1 worker) | Before | After | Change |
|---|---:|---:|---:|
| `codec.decode_text<Order>` | 469 ns/op | 197 ns/op | −58.0% |
| `codec.encode_text<Order>` | 263 ns/op | 162 ns/op | −38.4% |
| realworld `json`, 400k lines | 557.3 ms | 276.5 ms | −50.4% |

Minimums over 15 (micro) and 21 (workload) interleaved fresh runs; medians
agree. Runs are interleaved so the ratio is the result; the absolute before
values drift with host load (earlier sessions measured 475–519 ns/op and
562–627 ms for the same base). Host: AMD Ryzen 7 8745HS, x86-64 Linux,
`--release` compilation. `cpu`, `reconcile` and `state` workloads are
unchanged within noise (±2%, no consistent direction), matching the fact that
they do not decode JSON.

## What changed

- **`badger_runtime::json`** (new): a recursive-descent reader that decodes
  the next value according to the descriptor — records and structs by wire
  name, tuples by position, `Option`/`Data`/other enums, lists, maps and
  arrays. It reproduces `codec::dec`'s values and its exact `$`-rooted error
  paths and messages (the `tests/run/codec_paths`, `schema_wire`,
  `routes_typed`, `routes_params`, `app_bindings`, `http_scoped` and
  `spec_ch03_snippets` golden outputs are byte-identical). Numeric
  classification mirrors serde_json: an integral literal that fits
  `u64`/`i64` is an integer, anything with a fraction, exponent or overflow
  is a float (so e.g. `"1.0"` of an `Int` still fails as it did).
  Wire-name precedence (current name, then the first declared alias) is
  preserved without materialising the object.
- **`codec::encode`**: integers are written by a small decimal converter
  instead of `write!(.. "{}" ..)`, removing the `core::fmt` machinery per
  number; `write_json_quote` scans bytes and copies unescaped spans, instead
  of decoding and pushing one `char` at a time; the output buffer reserves
  64 bytes up front.
- **Fast paths in the parser**: when no wire naming is active (the ordinary
  case) members match by name directly instead of a thread-local lookup per
  key per field, and variant names compare directly. A record's member slots
  are one vector of `(value, rank)` pairs rather than three vectors, so each
  object costs one allocation. A string token is scanned to its closing quote
  with a byte-position search and borrowed whole when it has no escapes;
  escaped strings unescape into one buffer sized to the raw token. Unescaping
  skips a redundant UTF-8 revalidation (`String::from_utf8_unchecked`), safe
  because copied spans always begin and end on ASCII bytes. A decoded
  `String`/`Bytes` value is written directly into its managed cell (through a
  small stack buffer when escaped), so the temporary Rust string and its
  allocation disappear.
- **Encoder naming**: wire naming is checked once per record or enum instead
  of once per field, and an enum field's `Type.variant.field` key is not
  formatted at all when no wire naming is active.
- **Single-character split** (`text.split`): splitting by a one-character
  separator now uses `split(char)`, whose search is a byte scan, instead of
  the two-way string search (`split(&str)`); results are identical. This is a
  further **4.5%** on the `json` workload, which splits its input into lines.
- **Standard-library bulk copies** (backlog R2): `list.chunks`,
  `List.to_array`, `bytes.from_list`, `bytes_from_buffer`, `buffer_to_list`,
  `buffer_to_bytes`, `list.concat` and `list.slice` copy whole packed runs
  (`copy_nonoverlapping`) instead of one `list_push`/`Items::push` per
  element. Correctness is covered by the existing collection and codec
  tests; on the measured workloads the wall-clock is dominated by allocation,
  so the per-element savings are not separately visible at this granularity.

`codec::dec` (the `Value`-walking decoder) remains for `Data`-valued inputs
(`from_data`, `from_sql_row`, `params_decode`), so the value-tree path is not
yet removed end to end; see the revised R5 in [BACKLOG.md](../BACKLOG.md).

## Verification

- The branch was merged with `main` (which meanwhile added the parallel
  memory-management/GC and MIR work; none of those commits touch
  `crates/badger_runtime`), and every check below was re-run on the merged
  tree.
- Golden outputs for every codec- and route-related `tests/run` program are
  unchanged (`codec_paths`, `schema_wire`, `routes_typed`, `routes_params`,
  `app_bindings`, `http_scoped`, `spec_ch03_snippets`).
- `cargo test -p badger_cli --release --test programs` passes (76 tests),
  including `run_programs`, `run_programs_compiled`, `run_programs_release`
  and `run_programs_multicore`, which compare every `tests/run/*.out`.
- `cargo test --workspace --release` passes except the pre-existing
  `badger_syntax/tests/format_repo.rs` failure for
  `examples/13_rest_service.bdg`, which reproduces on the base revision with
  an unmodified binary and is unrelated to these changes.
- A throwaway edge-case program checks empty containers, `Option` defaults,
  numeric bounds and literal forms (`U64::MAX`, `I64::MIN`, `1e3`, `1.0` for
  `Int`, `18446744073709551616`), escapes and multi-byte strings, `Char`,
  `Bytes`, tuples, all enum forms, unknown members, and trailing input.

## Limits

- The parser is a second implementation of the decode rules that `codec::dec`
  encodes; they must stay in step. The golden tests pin the shared behaviour.
- Duplicate JSON keys are handled per field by wire-name rank; the tree walk
  collapsed them in a `BTreeMap` first. The reachable behaviours (current
  name over alias, first declared alias over later ones) match; a variant
  object with more than one distinct key still reports `expected a variant`.
- Syntax-error messages are this parser's own text (prefixed `invalid JSON:`);
  no test pinned serde_json's wording.
