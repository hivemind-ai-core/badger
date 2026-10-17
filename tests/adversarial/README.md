# Retained adversarial regressions

These are the independent tests behind
[the adversarial report](../../reports/2026-10-15-adversarial.md), now published
after verifying the fixes. They include 12 regression cases and 17 controls.
Each input has either an `.out` with exact expected stdout or a `.code` with
the required rejection diagnostic.

`cargo test --workspace` includes the Rust integration runner in
`crates/badger_cli/tests/adversarial.rs`. To run only this suite:

```sh
cargo test -p badger_cli --test adversarial
```

The 23 runtime programs run with one worker, four workers, and optimized JIT
compilation; six static cases must fail checking normally with their diagnostic
code. Together these are 75 checks. Each subprocess has a 20-second timeout.

| Report | Regression fixtures |
|---|---|
| ADV-001 | `call_snapshot_int`, `closure_snapshot`, `method_snapshot`, `tuple_snapshot_fields` |
| ADV-002 | `u64_cross_signed_range` |
| ADV-003 | `map_keys_map`, `map_keys_set` |
| ADV-004 | `float_aggregate_nan` |
| ADV-005 | `unterminated_comment` |
| ADV-006 | `affine_nesting_10` |
| ADV-007 | `array_value_equal`, `codec_array` |

The other fixtures are controls. The retained suite originally reproduced all
seven defects on `f642472`. Verification of `b8f36f6` passed 74/75 frozen checks;
the remaining assertion incorrectly required E0002 for the unterminated comment
instead of the correct syntax diagnostic E0001. That expectation is corrected
here. Sources have only been canonically formatted for the repository, except
for the byte-preserved input described below; runtime expectations and the
other diagnostic expectations are unchanged.

## Formatter limitation discovered during publication

`raw_interpolation.source` retains its original source bytes because formatting
an inline raw interpolated string inserts spaces between the triple quotes,
producing invalid syntax. The `.source` suffix keeps this format-sensitive
runtime input out of the repository's canonical-formatting scan; the CLI runs
it directly and the same output assertion applies in all three configurations.
The formatter defect remains unresolved; it is separate from ADV-001 through
ADV-007 and is not fixed by this test-only change.
