# Make it check

The program (`src/main.bdg` and `src/orders.bdg`) does not check. Read the diagnostics (`badger check`) and
fix the program without changing what `total` and `label` compute:
`total` sums the quantities of the order lines, and `label` describes an
order (`"3 items"`, or `"empty"` for none).
