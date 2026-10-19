# BACKLOG audit — 2026-10-19

Audit of every item still open in [`BACKLOG.md`](../BACKLOG.md) at `529e96f`
(branch `audit/backlog-2026-10-19`). Read-only apart from this report: no code
was changed. For each item: the code it names (`file:line`), what was measured,
what could not be measured cheaply, and a verdict.

## Method and environment

- Host: AMD Ryzen 7 8745HS, x86-64 Linux, 16 logical CPUs. **The machine was
  noisy during the audit** (`uptime` load average 11–13 from concurrent work in
  other worktrees); every wall-clock figure below says so and quotes minima of
  interleaved fresh runs. Structural numbers from `BADGER_TIMINGS` are more
  stable than wall-clock.
- Built once in the worktree: `cargo build --release` (3 m 41 s). All runs use
  `/tmp/language-backlog-audit-2026-10-19/target/release/badger`.
- Fixtures (outside the worktree, `/tmp/audit-fix`): `single.bdg` (one file),
  `proj0` (project, entry only), `proj1/2/4/8` (project + N `[entries]`, plus
  `proj8` also 2 `[tasks]`), `ent7` (project + 7 `[entries]`, no tasks).
- Commands used (exact):

```sh
cargo build --release                       # cwd = worktree
BADGER_TIMINGS=1 ./target/release/badger check /tmp/audit-fix/proj0
BADGER_TIMINGS=2 ./target/release/badger check /tmp/audit-fix/proj0
BADGER_TIMINGS=1 ./target/release/badger check /tmp/audit-fix/ent7 | grep -c 'parse.std'
for i in $(seq 1 8); do  # interleaved min-of-8, proj0 / ent7 / single.bdg
  s=$(date +%s%N); badger check /tmp/audit-fix/proj0 >/dev/null 2>&1; e=$(date +%s%N); echo $(( (e-s)/1000000 ))
done
BADGER_RUNTIME_STATS=1 ./target/release/badger run --release --bench --workers 4 bench/messages.bdg
python3 eval/run.py --reference --badger /tmp/language-backlog-audit-2026-10-19/target/release/badger --json
```

- `BADGER_TIMINGS=1` prints phases from `badger_driver::timed`
  (`crates/badger_driver/src/lib.rs:1447`) and typecheck marks
  (`crates/badger_typeck/src/program.rs:413`); `=2` additionally prints the
  slowest binding groups and the standard-library subtotal
  (`crates/badger_typeck/src/program.rs:579-619`).

## Verdict table (ranked order)

Rank = recommended order. "Owned by" is the owner the item belongs to if it is
not unowned; unowned items are the ones an independent contributor should take.

| # | Item | Verdict | One-line evidence |
|---|---|---|---|
| 1 | **C1.2b** re-parse/re-check per entry | **real - sizeable** | `lib.rs:710-727` loops `check_one` per selection; 7 entries → 8 full loads and ~22–60 ms per extra entry (measured). Unowned. |
| 2 | **C1.3/C1.4** reusable context, parse cache, checked-lib reuse | **real - sizeable** | No cache field on `Session` (`lib.rs:30-41`); `parse.std` 7–16 ms and `typecheck` ~20 ms repeated per selection. Unowned. |
| 3 | **C2** analysis efficiency | **stale claim (39 ms) + real - small** | `standard library groups` measures **10.5–12.5 ms**, not ~39 ms; effect fixpoint is **0.53 ms** (claim holds). Solver counters/fixtures absent. Unowned. |
| 4 | **C3** cross-build/code reuse for watch/REPL | **real - sizeable** | `mem::forget` leaks every accepted build (`live.rs:144,153`; `inspect.rs:208`); a fresh `Session` per reload (`live.rs:114`). Unowned. |
| 5 | **R3** channel admission sizes / indexed leases | **real - small** | `count` is already O(1) (`channel.rs:87-97`), but admission builds a `Vec` of map entries (`channel.rs:645-658`) and `uncharge` re-sizes (`channel.rs:936-944`). Unowned. |
| 6 | **R5** decode `Data` directly | **real - small** | `from_data` still builds `serde_json::Value` (`codec.rs:362-367`); only the text path streams (`json.rs:982`). Prior attempt +25 %, so risky. Unowned. |
| 7 | **C4** root-store/frame counter | **real - small** | Reductions landed (`codegen/lib.rs:718-722,1606-1614`); **no counter exists** anywhere (repo grep). Owner: MIR/codegen. |
| 8 | **R1** collector scratch / memory pressure | **real - small** | Mark stack not reused (`gc.rs:690`), `occupied` sort remains (`gc.rs:966-969`), channels not external-accounted. Owner: GC/memory. |
| 9 | **R4** typed per-intrinsic contract table | **real - small** | `UNLOCKED` is still ~130 name strings (`codegen/lib.rs:1646`); only an arity table (`runtime_sig_of`, `:194`). Owner: MIR/codegen. |
| 10 | **T3** more code actions | **real - sizeable** | Two quick fixes only (`lsp.rs:609-653`); no protocol-stub action (E0219) and no edit hash preconditions. Unowned (tools). |
| 11 | **T4** holes list in-scope locals | **real - sizeable** | E0345 reports the type only (`program.rs:628-648`); no code reads locals at the hole. Unowned (tools). |
| 12 | **T6** lints | **real - sizeable** | No lint subsystem, no `badger lint` subcommand, no manifest lint config. Unowned (tools). |
| 13 | **T7** endpoint/interceptor generators + wiring | **real - sizeable** | `feature`/`worker` generators exist (`generate.rs:303-339,273-301`); no endpoint/interceptor generator, and wiring is left to the author (`generate.rs:5-6`). Unowned (tools). |
| 14 | **T9** run eval with agents, grow task set | **real - small** | Harness + 3 tasks work (`eval/run.py`; reference 3/3 solved), no agent run ever recorded; ENHANCEMENTS named 7 tasks. Unowned. |
| 15 | **S5** capacity-aware builders/bulk ops | **real - small** | Builders and conversions exist; **no capacity hint** on any builder and `list` has no bulk `from_list`/`extend`. Unowned (stdlib). |
| 16 | Sync **slice 0** counters | **partly done / real - small** | Lock counters + reasons exist (`task.rs:566-605`); spin *time*, fallback reasons, GC ack latency, timer lateness, metrics-off control absent. Owner: runtime. |
| 17 | Sync **slice 1** split + stable waiters | **real - sizeable** | One `ChanData` (`channel.rs:30-85`) with fabricated `&mut` via `data()` (`:220-226`); waiters are channel tuples, not records. Owner: runtime. |
| 18 | Sync **slice 2** GC without runtime lock | **real - sizeable / blocked on 1** | Collection requires the lock (`gc.rs:353-363`); global `NoGc` (`:515`). Owner: GC/memory. Needs a state-machine model first. |
| 19 | Sync **slice 3** scoped channel guard | **real - sizeable / blocked on 1-2** | `CLAIMED`/`release_claims` still used (`channel.rs:235,251`; `task.rs:781`); byte-budget/watcher/batch paths park under the claim. Owner: runtime. |
| 20 | Sync **slice 4** selection/close via descriptors | **real - sizeable / blocked on 3** | Selection mutates other tasks' fields (`channel.rs:756-760`; `task.rs:258-259`). Owner: runtime. |
| 21 | Sync **slice 5** producer staging + credits | **real - sizeable / blocked on 4** | Staging is channel-resident (`channel.rs:51`); no credits; charges recomputed. Owner: runtime. |
| 22 | Sync **slice 6** timer lock / park tokens | **real - sizeable / blocked on 4** | Timers on the runtime lock (`task.rs:337`); global `IDLE`/`WAKING`. Owner: runtime (GC-adjacent for park). |
| 23 | Sync **slice 7** per-scope locks, snapshot outside lock | **real - sizeable / blocked on 2,6** | One `SCOPES_LOCK` (`scope.rs:37-43`); snapshots format under the lock (`task.rs:3643-3677`). Owner: runtime. |
| 24 | Sync **slice 8** specialized rings | **blocked on other work (3-6)** | No ring code; `MEMORY_MAN_TODO.md:138` prototype unchecked. |
| 25 | Sync gates (lock-order asserts, models, p99/timer gates) | **real - sizeable** | No lock-order tracker, no model crate, no percentile machinery. Owner: runtime/GC. |

Nothing in the open backlog is `done`; the only outright stale *claim* is C2's
"~39 ms". R2 is already recorded done in the backlog.

## Compiler and runtime details

### C1.2b — parse once per session across `[entries]`/`[tasks]`

- Code: `Session::check_package` loops `check_one` once per selection
  (`crates/badger_driver/src/lib.rs:710-727`); `check_one` always starts at
  `timed("parse", || self.load(path))` (`:731`); `load` re-adds and re-parses
  the standard library every time (`:504`, `load_std` at `:512-591`).
  `project_entry_points` is `:146-154`. Same rerun happens for `run`/`build`
  (`:1117`), `explain_def` (`:1806`), `inspect_plans` (`:1821`) and
  `analysis::analyze` (`analysis.rs:222`).
- Measured: a project with 7 `[entries]` produces **8 `parse.std` phases**
  (`ent7`, count = 1 + selections). Interleaved min-of-8 at load ≈ 12.6:
  `proj0` 69 ms, `ent7` 498 ms → **~61 ms per extra entry** under that load; an
  earlier quieter pass (load ≈ 4) gave `proj0` 41 ms, `proj8` 244 ms →
  **~22 ms per selection**. `BADGER_TIMINGS=1` per selection: `parse.std`
  7–16 ms, `resolve` ~2 ms, `typecheck` ~20 ms. This is consistent with the
  main agent's 21 ms/entry and 119.5 ms 4-entry figure.
- Cannot measure: the exact split of parse vs check on a quiet machine without
  taking the worktree offline from the other workers.

### C1.3/C1.4 — reusable compiler context, content-keyed parse cache, checked-library reuse

- Code: `Session` has no cache/context field
  (`crates/badger_driver/src/lib.rs:30-41`). `badger_syntax::parse` is
  stateless (`parser.rs:42`), `SourceMap::add` always allocates a new `FileId`
  (`badger_base/src/source.rs:40-52`), symbols are per-session, so cached ASTs
  are not portable without span/`FileId`/symbol remapping (as the plan says).
  No cross-build instance or checked-library reuse: `check_typed`
  (`lib.rs:2190`) runs per entry; `Global` is per check (`program.rs:35-70`).
- Measured: the prize is bounded by C1.2b's numbers — removing the repeated
  `parse.std` + `typecheck` for 7 extra entries would take `ent7` toward
  `proj0` (~70–90 % of its wall time at 12.6 load). Not attempted; read-only.
- Cannot measure: cache hit rate (nothing to cache yet).

### C2 — analysis work proportional to changed facts (and the 39 ms claim)

- Code: `effect_fixpoint` (`crates/badger_typeck/src/program.rs:1394`), called
  at `:757`; it rescans every call edge and clones label vectors
  (`:1453-1467`), and `labels_of` clones the scheme type per call
  (`:1397-1398`). Solver restart loop without counters
  (`crates/badger_typeck/src/solve.rs:313-323`); impl search clones candidates
  (`solve.rs:1312-1316`). Binding groups are the SCCs at `program.rs:570-621`.
- Measured: `BADGER_TIMINGS=2` — **`standard library groups: 10.5, 10.75,
  10.9, 11.3, 12.2, 12.5 ms`** across runs (704 groups; essentially all of the
  ~11 ms is std), matching the main agent's ~10 ms. **The "~39 ms" figure is
  stale.** `typecheck.effects` (`effect_fixpoint`) is **0.53 ms**, so "under
  1 ms" holds.
- Cannot measure cheaply: solver retries — no counters exist (plan says
  instrument first); scaling fixtures (long call chains, wide records, many
  impls) do not exist in `bench/`.

### C3 — cross-build instance/code reuse and code reclamation

- Code: every accepted build is leaked: `std::mem::forget(built)` at
  `crates/badger_driver/src/live.rs:144` and `:153`
  ("old and new code stay mapped for the life of the process"), and for the
  REPL at `crates/badger_driver/src/inspect.rs:208`; dispatch slots are
  `Box::leak`ed (`badger_runtime/src/live.rs:37-38`). `reload` builds a fresh
  `Session` (`live.rs:114-115`). No cross-build instance cache: monomorphization
  dedups only within one build (`badger_mir/src/lower.rs:169-171`).
- Cannot measure cheaply: reload pause and RSS/code growth over hundreds of
  edits needs a watch/REPL harness that does not exist; the plan calls for it.
- Note: the GC/memory owner's `MEMORY_MAN_TODO.md` I3/I4 work interacts with
  this (code and instance lifetime), but reclamation itself is the
  driver/JIT-side work named here.

### C4 — root-store/frame counter

- Code: the reductions are present — frames are omitted and slots only for
  safepoint-live pointers (`crates/badger_codegen/src/lib.rs:718-722`), slots
  are reused per safe point (`:1606-1614`), and `safepoint_roots` clears
  always-null pointers (`badger_mir/src/roots.rs:100-124`). `rooted_locals`
  exists "for diagnostics and metrics" (`roots.rs:195`).
- No counter: repo-wide `grep ROOT_STATS|root_stores|BADGER_ROOTS|frames_linked`
  returns nothing; codegen has no stat hook around root emission. The nearest
  probe is the offline example `crates/badger_codegen/examples/mir_audit.rs:124`
  (prints `roots=`/`always_null_roots=` per function from a saved MIR file).
- Cannot measure: no counter to read; adding one is the item. Owner:
  MIR/codegen.

### R1 — collector scratch and memory pressure (GC/memory owner)

- Done: no per-object child vector (`gc.rs:790-806`), in-place sweep with no
  survivor vector (`gc.rs:1044-1064`), scalar-map tracing returns early
  (`map.rs:56-61`, `hamt.rs:49-53`), buffer storage accounted
  (`buffer.rs:93,147`).
- Open: fresh mark stack per collection (`gc.rs:690`), remembered buffer
  `take`+`collect` (`gc.rs:700-701`), `occupied.sort_unstable()` in
  `trim_empty_chunks` (`gc.rs:966-992`), small per-collection `Vec`s
  (`gc.rs:832,853`), **channels are not external-accounted** (no
  `add_pressure`/`external_alloc` in `channel.rs`), `stats()` still lumps
  managed + external pressure (`gc.rs:1311-1319`), and RSS is absent from the
  runtime. The product intrinsic splits managed/external/pool
  (`intrinsics_codec.rs:781-795`) but the inspect path does not
  (`inspect.rs:89-95`).
- Contract note: `gc::budget_check` per element in `list_builder_append` is GC
  accounting and must not be changed.

### R3 — channel admission sizes, stored charges, indexed leases

- Done: `count` is O(1) with `accepted`/`leased` counters, validated by a
  `debug_assert` recount (`channel.rs:87-97`); buffer element cloning is gone.
- Open: admission still materializes a `Vec` of map entries
  (`channel.rs:645-658` via `map::entries`, `map.rs:216-220`; a
  `hamt::for_each` visitor exists); batch/staged/leased carry no stored sizes
  (`Batch` is `{items, released}` at `channel.rs:19-22`), so `uncharge`
  recomputes every size (`channel.rs:936-944`); leases are a `Vec` with a
  linear `retain` (`channel.rs:56,1231`).
- Measured: not separately measurable with the current fixtures — the lock
  report shows batch slow paths at 1564 acquisitions / 1.17 ms held for
  `messages` at 4 workers, so this is bookkeeping micro-cost, not wall-clock.
  The `debug_assert` already proves the counters are exact.
- Verdict: real - small; low risk, local to `channel.rs`, but no user-visible
  speedup is currently demonstrable.

### R4 — typed per-intrinsic contract table

- Code: `UNLOCKED` is a flat `&[&str]` of ~130 names with prose comments
  (`crates/badger_codegen/src/lib.rs:1646-1778`); `rt_call` short-circuits on
  `self.prog.workers > 1 && !UNLOCKED.contains(&name)` (`:1477`); enter/leave
  imports are gated on `workers > 1` (`:592-595`). The only per-intrinsic table
  is arity, `runtime_sig_of` (`:194-224`), which carries no lock domain. The
  one existing typed contract is the FFI adapter contract
  (`badger_hir/src/hir.rs:177-181`, carried as `Foreign { contract: u8 }`,
  consumed at `codegen/lib.rs:607-610,1299-1328`) — not an intrinsic contract.
- Measured: no perf claim; this is maintainability/correctness (a name typo or
  a new unlocked intrinsic is caught only by review). Owner: MIR/codegen.

### R5 — decode `Data` without the intermediate JSON value

- Done: `badger_runtime::json` streams text into the target value
  (`json.rs:982-999`), wired as `codec::decode` (`codec.rs:350-353`); the
  descriptor-fact cache (`desc.rs:154-203`, `value.rs:111-134`) is in use.
- Open: `from_data` still builds a `serde_json::Value` (`codec.rs:362-367` via
  `intrinsics_io::data_value`, `intrinsics_io.rs:97-110`) and `from_sql_row`
  delegates to it (`codec.rs:375-388`); `error_details` round-trips text
  (`intrinsics_std.rs:88-95`). The report
  `reports/2026-10-19-json-decode-streaming.md` records that routing
  `from_data` through the text decoder measured **+25 %** (850 → 1065 ns/op on
  a 64-row round trip), so only a direct `Data` walker remains worthwhile.
- Verdict: real - small, but the obvious approach already failed once; treat as
  a design task, not a mechanical change.

## Runtime synchronization slices 0–8 (LOCK_OPTIMIZATION.md)

Current profile (measured, `BADGER_RUNTIME_STATS=1 --bench --workers 4` on
`bench/messages.bdg`): 3941 lock acquisitions, 19 after spinning, 4 after
parking (0.96 ms parked) against 78 ms of run time (36.6 ms at 1 worker);
largest holds are the slow paths (batch slow 1564×/1.17 ms, batch_recv
783×/0.53 ms). This confirms the archived header: remaining multi-worker cost is
**not** lock waits, so slices 1–8 are correctness-and-scale work, not
throughput wins. Slice 0's counters are partly present
(`task.rs:566-605`); spin *time*, per-channel fallback reasons, GC
request→ack, timer lateness and a metrics-off control are absent
(`task.rs:570,759-764`; `gc.rs:638-660`; `task.rs:337,3472-3481`). Slice 1's
`ChanData` split and stable waiter records are open (`channel.rs:30-85,220-226`;
waiters are tuples at `:63,66`). Slices 3–8 are open as scouted
(`CLAIMED`/`release_claims` at `channel.rs:235,251`,`task.rs:781`; selection
cross-task writes at `channel.rs:756-760`; channel-resident staging at
`:51`; timers on the runtime lock at `task.rs:337`; one `SCOPES_LOCK` at
`scope.rs:37-43`; no ring code). The three gates (lock-order assertions,
state-machine models, p99/timer-lateness gates) are all open. Several
documents still cite the pre-rename crate paths (`crates/lang_runtime/...`) and
the old `LANG_RUNTIME_STATS` name (code uses `BADGER_RUNTIME_STATS`,
`task.rs:563`) — stale text, not stale work.

## Libraries and tools

- **T3**: only two quick fixes exist (`lsp.rs:609-653`: insert-import E0127 and
  missing-arm E0342/`missing_arm` at `:951-1016`); no action for E0219
  protocol-method stubs, no hash preconditions on edits (`code_actions` emits
  no hash), and no action surface in MCP or `std.compiler`.
- **T4**: holes lower to `prelude.hole` (`prelude.bdg:259`) and E0345 reports
  only the resolved type (`program.rs:628-648`); nothing reads locals at the
  hole. The nearest machinery is unfiltered LSP completion
  (`lsp.rs:792-798`).
- **T6**: no lint subsystem at all (repo grep for `fn lint|Lint|W[0-9]{4}` →
  none), no `badger lint`, no manifest lint config; `stream.collect`'s
  unboundedness is documented prose only (`stream.bdg:263-266`), with
  `collect_at_most` as the bounded alternative.
- **T7**: `badger generate feature|worker` exist and check-and-test in a copy
  before writing (`generate.rs:303-339,273-301,359-403`); no endpoint-,
  interceptor-, module- or operation-only generator, and no composition-root
  wiring — the module doc says wiring is left to the author
  (`generate.rs:5-6`), and `Plan` is a file list only (`:266-268`).
- **T9**: `eval/run.py` works (`--reference` scored 3/3 in 1.8 s) with 3 tasks
  (`add_endpoint`, `add_field`, `fix_errors`); no agent run has ever been
  recorded, and ENHANCEMENTS named 7 tasks.
- **S5**: `list.build`/`map.build`/`set.build` exist
  (`list.bdg:118-128`, `map.bdg:26-34`, `set.bdg:19-27`) and conversions are
  complete, but no builder takes a capacity hint (only
  `buffer.with_capacity`, `buffer.bdg:62`), runtime growth is a fixed ladder
  (`list.rs:304,462`), and `list` has no bulk `from_list`/`extend` (map/set/
  array/bytes/stream do).

## Recommended order

1. **C1.2b** — largest measured win (multi-entry `badger check` and
   `run`/`build`), localized to `badger_driver`, no GC/MIR interaction. Do it
   before C1.3/C1.4, since it shrinks the same repeated work without needing
   span/`FileId`/symbol remapping.
2. **C1.3/C1.4** — depends on the same `Session`/`load` surface; the
   content-keyed parse cache is the enabling step and must handle remapping and
   user-implementation-sensitive std checking.
3. **C2 instrumentation** — add solver retry counters and scaling fixtures
   (cheap, unblocks measurement); C2's "~39 ms" premise should be corrected in
   `BACKLOG.md` to ~10–12 ms now.
4. **C3** — the watch/REPL correctness/scale item; large, and the natural
   follow-on to C1.3/C1.4.
5. **R3, R5, C4, R1, R4** — small, independently landable; R1 is the GC/memory
   owner's, C4 and R4 the MIR/codegen owner's.
6. **T3–T7, T9, S5** — tooling/library features, unowned; each is an
   independent, well-scoped contribution.
7. **Sync slices** — slice 0 counters first (measurement), then 1 → 2 → 3 → …
   in dependency order, only against a fresh profile; runtime/GC owners.

## Ownership split

- **Unowned contributor** (safe to pick up without the GC/MIR owners): C1.2b,
  C1.3/C1.4, C2, C3, R3, R5, T3, T4, T6, T7, T9, S5.
- **GC/memory owner**: R1, sync slice 2 (GC coordination), slice 6's park
  coordination, sync gates, and the GC request→ack counter in slice 0.
- **MIR/codegen owner**: C4 (emission counter lives in `badger_codegen`) and R4
  (`UNLOCKED`/`rt_call`).
- **Runtime-synchronization owner**: sync slices 1, 3, 4, 5, 7, 8 and slice 0's
  channel/timer counters.

## Reproduction limits

- Wall-clock figures are inflated by concurrent load in sibling worktrees
  (load average 11–13); live interleaved runs are required for any before/after
  claim. Structural `BADGER_TIMINGS` numbers and phase *counts* are the reliable
  part of this audit.
- No GC, MIR or codegen change was made, so C4/R1/R4 and C1.3/C1.4 were verified
  by code inspection only; their costs are inferred, not measured.
- The `eval/` harness was exercised, but no LLM/agent run (T9's actual subject)
  was performed here.
