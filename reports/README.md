# Reports

Measurements and reviews behind the project's claims, by date. Each report
states the revision it measured; newer work can change its numbers.

## Current

| Report | What it covers |
|---|---|
| [2026-10-20-egg-straight-line-regions.md](2026-10-20-egg-straight-line-regions.md) | Why egg over straight-line MIR regions (§4.2) is not adopted: with the §4.1 rule set it finds 2 rewrites `superopt` does not; the implementation is in [egg-survey/](egg-survey/) |
| [2026-10-20-mir-escape-analysis.md](2026-10-20-mir-escape-analysis.md) | Why MIR escape analysis with stack allocation (§3.2) is not adopted: the remaining allocations escape; survey script in [escape-analysis-survey/](escape-analysis-survey/) |
| [2026-10-19-language-and-compiler-changes.md](2026-10-19-language-and-compiler-changes.md) | Ergonomics backlog for the compiler team: syntax, type system, diagnostics, runtime semantics, incremental build — merge of the ergonomics investigation and the experience report |
| [2026-10-19-library-and-framework-changes.md](2026-10-19-library-and-framework-changes.md) | Ergonomics backlog for the library team: standard library, extended library, application services framework (amendments, improvements, additions), CLI and docs — same merge, other half |
| [2026-10-19-mir-optimization-architecture.md](2026-10-19-mir-optimization-architecture.md) | Release MIR optimization pipeline, closure specialization, and the verified superoptimizer mode |
| [2026-10-19-sequence-fusion-and-presized-builders.md](2026-10-19-sequence-fusion-and-presized-builders.md) | `Seq.collect`/`sum` fusion, pre-sized list builders, and sequence-operation measurements |
| [2026-10-19-map-builder-node-headroom.md](2026-10-19-map-builder-node-headroom.md) | Bulk map builder node growth: fewer reallocations and copies |
| [2026-10-19-eager-list-map-and-map-entries.md](2026-10-19-eager-list-map-and-map-entries.md) | Eager `List.filter`/`map` fusion, map entries fusion, and cursor entry loading |
| [2026-10-19-sequence-collection-audit.md](2026-10-19-sequence-collection-audit.md) | Sequence/collection surface audit against Clojure/Elixir/Rust, the additions made, and the compiler/runtime optimizations still needed |
| [2026-10-17-go-elixir-report-status.md](2026-10-17-go-elixir-report-status.md) | What became of each recommendation in the Go/Elixir design report |
| [2026-10-17-performance-optimization-results.md](2026-10-17-performance-optimization-results.md) | Compiler and runtime optimization results (targeted probes and the telemetry application) |
| [2026-10-17-gc-profiling.md](2026-10-17-gc-profiling.md) | Application and GC profiling; data in [gc-profile-2026-10-17/](gc-profile-2026-10-17/) |
| [memory-management/](memory-management/) | The memory-management work in progress (see `MEMORY_MAN_TODO.md`) |
| [realworld/](realworld/README.md) | Badger, Go, Node.js and Java application workloads: stamped reports, latest comparisons and performance history |
| [language-comparison/2026-10-16/](language-comparison/2026-10-16/README.md) | The same workloads in other languages (`bench/compare`) |
| [benchmarks/](benchmarks/) | `bench/baseline.py` snapshots (the latest, and the one `archive/plans/LOCK_OPTIMIZATION.md` cites) |
| [2026-10-16-go-elixir-alternative-design-report.md](2026-10-16-go-elixir-alternative-design-report.md) | The Go/Elixir alternative design report and its recommendations |
| [2026-10-16-channel-topology-investigation.md](2026-10-16-channel-topology-investigation.md) | Channel topology, specialization, and the SPSC experiment in [channel-topology-investigation/](channel-topology-investigation/) |
| [2026-10-16-channel-latest-comparison.md](2026-10-16-channel-latest-comparison.md) | Channel-fixes benchmark comparison on latest committed main |
| [2026-10-16-runtime-benchmark-update.md](2026-10-16-runtime-benchmark-update.md) | Runtime benchmark update after the receive optimization |
| [2026-10-15-codebase-audit.md](2026-10-15-codebase-audit.md) | Codebase robustness and memory-safety audit |
| [2026-10-15-runtime-concurrency-review.md](2026-10-15-runtime-concurrency-review.md) | Runtime concurrency and memory performance review |
| [2026-10-15-compiler-codegen-optimization-audit.md](2026-10-15-compiler-codegen-optimization-audit.md) | Compiler codegen optimization audit; probes in [compiler-codegen-audit/](compiler-codegen-audit/) |
| [2026-10-15-memory-optimization-plan.md](2026-10-15-memory-optimization-plan.md) | Memory management optimization plan and findings |
| [2026-10-15-adversarial.md](2026-10-15-adversarial.md) | Defects found by adversarial testing (cited by `tests/adversarial`) |

## Archive

[archive/](archive/README.md) holds earlier reports and benchmark snapshots,
kept for their history. Links inside them may point to where files were
before they were archived.
