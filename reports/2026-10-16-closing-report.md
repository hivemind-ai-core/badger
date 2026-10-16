# Closing report: correctness, performance, tooling, and hardening

Dated 16 October 2026. Covers the work that followed the correctness
milestone: a test and benchmark baseline, the runtime lock, the optimization
documents, editor and agent tooling, debugging and profiling, language
additions suggested by `REAL_WORLD_USE.md`, testing infrastructure, and a
final round of independent audits. Every item landed as its own commit on
`main` after the full suite (build, all tests, clippy `-D warnings`,
rustfmt, `lang format --check`); runtime and code-generation changes were
also run over the whole program corpus under `LANG_GC_STRESS=1` (debug and
optimized code), `LANG_GC_VERIFY=1` with chaos scheduling, and multi-worker
chaos, plus randomized stress soaks.

## Performance

Details: [2026-10-16-performance-report.md](2026-10-16-performance-report.md).
Final measurements: `reports/benchmarks/2026-10-16-final.{md,json}` (same
machine and method as the 2026-10-15 baseline).

| Measure | Baseline | Final |
|---|---|---|
| Check an empty program | 126 ms | 21 ms |
| Check a 5,600-line program | 591 ms | 177 ms |
| Function call, scalar / 4 rooted pointers | 3.5 / 7.0 ns | 1.9 / 2.6 ns |
| Row-generic record call | 2.8 ns | 1.2 ns |
| `records` / `runtime` benchmarks | 118 / 53 ms | 84 / 22 ms |
| Paged list iteration | 6 ns/element | 1 ns/element |
| Persistent map insert / lookup | 2,201 / 152 ns | 452 / 68 ns |
| Allocation-heavy loop | 105 ns/record | 39 ns/record |
| GC pause total (that loop) | 96 ms | 20 ms |
| Task spawn + join | 10.9 us | 0.53 us |
| Prompt channel, 8 workers | 362 ns | ~55 ns |
| Prompt round trip, 8 workers | 5,489 ns | ~330 ns |
| Four independent channel pairs, 8 workers | 130 ms | 9.3 ms |
| Four allocating tasks, 8 workers | 320 ms | 39 ms |
| CPU-bound generated code | 656 / 167 ms (1 / 8 workers) | 658 / 168 ms (already native-quality) |

No measured program slows down with more workers any more (before, every
communicating or allocating benchmark did). The global runtime lock remains
for parking, waking, spawning, `select`, timers, and the collector's
stop-the-world; all allocation, list, map/set, and uncontended channel
operations run without it.

Optimized builds also inline small, non-recursive helpers and re-run
scalar replacement over their callers (codegen audit G2, plan C5), so a
record built only to be passed to a helper is not allocated
(`LANG_NO_INLINE=1` turns it off; live dispatch disables it).

## Tooling

- **Language server** (`lang lsp`, [docs/editors.md](../docs/editors.md)):
  diagnostics on every edit (coalesced; unsaved buffers overlaid), hover,
  definition, references, highlights, rename, document and workspace
  symbols, formatting, and completion that keeps working while the text
  being typed does not parse. Setup for Neovim, Helix, Emacs, and VS Code.
- **Queries and graphs**: `lang query` (diagnostics, hover, definition,
  references, completion, symbols; `--source` for text) and `lang query api`
  (a standard module's or program's public interface with signatures,
  effects, contracts, and documentation) as JSON; `lang graph
  calls|modules|types|systems` in DOT, Mermaid, or JSON.
- **Agent access** (`lang mcp`, [docs/agents.md](../docs/agents.md)): four
  tools. `eval` is code mode: a persistent REPL session in which
  `std.compiler` gives programs typed access to the compiler's analysis;
  `check`, `query` (including API export, graphs, explanations, and module
  sources), and `run` (with breakpoints under gdb and CPU profiles).
- **Debugging** (`lang run --debug`, [docs/debugging.md](../docs/debugging.md)):
  DWARF line tables, locals and parameters (stack slots, so readable and
  watchable throughout a function), and unwind tables for generated code,
  registered through the GDB JIT interface. Verified with gdb: breakpoints
  by file and line (conditional too), stepping in, over, and out,
  backtraces from generated frames into the runtime, `info args/locals`,
  and hardware watchpoints. LLDB reads the same interface but was not
  available to test.
- **Profiling**: `LANG_PROFILE=FILE` samples each thread on its own CPU
  clock and writes flamegraph-compatible stacks plus a summary (covering
  compilation too); `LANG_PERF_MAP=1` names generated code for `perf`;
  `LANG_DUMP_ASM` prints generated machine code.

## Language

- **Function contracts** (spec chapter 03): `requires` and `ensures`
  clauses on the lines before a function or method; postconditions are
  checked on every return path with `result` bound; failed preconditions
  blame the call site. Clauses go through the type, effect, and ownership
  checks, and tools show them with signatures.
- Library structs and enums that derive `Codec` are data (they can be
  encoded and decoded like user types).
- **Directory capabilities** (`REAL_WORLD_USE.md` section 4): `fs.root`
  returns a `Dir` confined to one tree (absolute paths, `..`, and symlinks
  out of it are refused); `Dir.sub` narrows it and `Dir.reader` gives a
  read-only `ReadDir`, so a function's parameter types state what it may
  touch. Private fields keep them from being forged.

## Testing infrastructure

- A mutation fuzzer over the corpus (`tests/fuzz.rs`): no compiler panic or
  crash in a 10-minute soak; the formatter is idempotent and never panics on
  mutants; reformatting never changes a program's diagnostics.
- Protocol-level tests of the language server and the MCP server, including
  hostile input; tests for queries, graphs, the API export, contracts, the
  profiler, and debugging under gdb.
- The formatter property test found a non-idempotent case (a stray leading
  separator before a closer), now fixed.
- A guard that kills test servers when a test fails (orphaned processes were
  found from earlier failed runs).
- ThreadSanitizer was attempted and is not usable here: the standard library
  cannot be instrumented offline, and TSan does not follow the runtime's
  coroutine stack switches. The runtime's concurrency relies on the stress,
  chaos, and verification suites instead.

## Audits

Two independent read-only reviews at the end, each confirming findings with
reproducing programs:

- **Runtime and code generation** (GC pools, list directories, map tries,
  root liveness, paged iteration, shared variants, scheduling, contracts):
  five of the eight changes held up under every attack. Fixed: a
  `requires` after an `ensures` could read an unset `result` (crash); a
  postcondition could consume an affine `result` that was then returned;
  `return`/`?` inside clauses and contracts on protocol method signatures
  were accepted; precondition failures could blame a stale call site;
  decoding a scalar rooted it as an object (a crash in ordinary programs,
  predating this work); `ListBuilder.element_address` lacked a bounds check;
  narrow list elements used 8x the memory; and two low-severity
  concurrency issues in statistics and wake-up bookkeeping.
- **Tooling** (LSP, MCP, queries, debugging, profiler): fixed an
  unauthenticated code-execution endpoint (the REPL/MCP session's local
  socket), symlink-following temporary files, agents being told code checks
  when it was unreadable or outside its project's module graph, several
  inputs that crashed or stopped the language server, renames that broke
  programs, MCP error reporting, unbounded debugger runs, a live-reload
  identity collision between methods of different impls, and profiler
  undercounting of parallel work.

## Known limitations

- The global runtime lock still serializes parking, waking, spawning,
  `select`, timers, and collection; removing it needs a lock-free scheduler.
- Inlining covers small direct helpers only; closure devirtualization and
  range-based check elimination (codegen audit G4/G5) were not done.
- Parsing (pest) is now the slowest compiler phase (about 15 us per line).
- A cancelled REPL/MCP evaluation stops at its next safe point, but blocking
  work it already started (a file read, a subprocess) runs to completion;
  each evaluation keeps its compiled code (about 110 KB).
- Debuggers show managed values (strings, records, lists) as addresses.
- Completion after a chained receiver (`a.b.`) or a call result offers
  nothing; a file that does not exist on disk yet is not analysed.
- Contracts are checked at run time only; nothing is proved statically.

Of the `REAL_WORLD_USE.md` directions, contracts, analysis graphs, the API
export, agent tooling, and a first capability (directories) are done. Not
taken up: a restricted mode that withholds the ambient filesystem, network,
and process APIs (without it, capabilities are a discipline rather than a
guarantee), network capabilities, reproduction bundles, durable job state,
and typed SQL; these are product work for a pilot application.
