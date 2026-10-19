# Correct bulk list directory capacity initialization

The broad G8d validation on the merged October 19 main found a debug-build
failure in `option_niche::pointer_option_roots_survive_gc_suspension_and_multiple_workers`:
`misaligned pointer dereference` at `badger_runtime/src/list.rs:582`.

Bulk `list::fill`, introduced by `3bb61f5`, wrote its directory capacity with
`payload().add(1) as *mut u64`. The payload pointer is byte-addressed: capacity
is the second word at byte offset 8, not byte offset 1. The old store was both
misaligned and overlapping the initialized-slot count; release execution was
undefined even where x86 tolerated the store. Correct the offset to 8. Also
remove an explicit auto-dereference in the same new helper so strict runtime
Clippy passes. There is no layout or saved-MIR ABI change.

The new native regression exercises empty, small, exact-page and multi-page
filled lists at 1- and 8-byte element widths. It checks directory capacity,
forked persistent appends, post-collection old contents, and packed array
conversion under GC stress, generation modes 0/1 and verification 0/1 in
isolated processes. It passes in ordinary and diagnostic builds. The previously
failing three-test Option niche suite and four packed-array tests pass, as does
strict runtime all-target Clippy. This is a correctness fix, not a benchmarked
performance improvement. New optimization comparisons must include it in both
baseline and candidate; earlier binaries containing the undefined store are
historical evidence only.
