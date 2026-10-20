# Translation validation: IR parity as a debug gate (§4.3)

Implemented 2026-10-20 against `3d8f5c6`. Status: the gate is in, the
literal "bit-for-bit optimized vs unoptimized" comparison it was specified
as is **not** the gate that shipped, and this report records why with the
measurement. The gate that shipped is the sound one.

## What was asked, and the measurement that rejects it

§4.3 / the Gates table ask that "the post-MIR-optimisation program must be
bit-equivalent to the pre-MIR-optimisation program", checked by
`BADGER_MIR_VALIDATE=1`. That is impossible for a *correct* optimiser: the
release passes restructure the program by design, so the optimised program
is never bit-equal to the unoptimised one, and a raw comparison fails on
every program the optimiser touches — correct or not.

Measured on `tests/run/acceptance_static.bdg` with the release CLI:

- `run --dump-mir` without `--release`: 3069 lines; with `--release`: 3059
  lines. The two dumps diverge at local index 155 and never re-converge
  (function and local tables are renumbered by `reach`/inlining, then the
  bodies are rewritten).
- Per-pass change counts (`BADGER_DUMP_MIR_PASSES=1`) on the same program:
  `inline 83, specialize 2, sra 70, simplify 2324, niche 2, simplify2 12,
  reach 44`. There is no structural invariant of the *program* that all of
  these preserve.

So a bit-for-bit comparison of the optimised and unoptimised programs
cannot distinguish a correct optimisation from a broken one: it rejects
both. It is not a gate.

## What shipped

Two checks, both behind `BADGER_MIR_VALIDATE=1` (the name the backlog
gives), the second being the gate for the meaning question:

1. **Structural determinism** — `badger_mir::validate::compare`, called from
   `badger_driver::optimize_mir`. The optimizer is run twice from the same
   lowered program and the two results are compared **bit for bit over the
   whole program**: every function's name, kind, parameters, return
   representation, locals, blocks, statements in order, and terminator;
   every operand, constant, callee, and representation; the entry, globals,
   static closures, display hooks, tests, and their error types. The first
   difference is reported with its function, block, statement index, and
   both values, and fails the compilation. A pass that depends on hidden
   state, on iteration order, or on anything but its input is caught here.
   The program handed to code generation is always the first run's, so the
   check never changes the compiler's output. This is exact where it looks;
   the only things it does not compare are a local's debug name and the
   lowering-side metadata (`warnings`, `entry_error`, `debug_files`,
   `origins`, `live`, `workers`, `natives`) that the pass pipeline does not
   produce.

2. **Behavioural parity** — the CLI, in the same mode, runs the program
   twice: `run --release` (optimised) and the same run with the MIR
   pipeline turned off (`BADGER_NO_INLINE=1`; see
   `Session::lower_to_mir`, where the whole release pipeline is skipped). It
   compares stdout and exit status and fails the run with the first
   differing line when they disagree. This is the sound translation
   validation for the passes: the optimiser must preserve *observable
   behaviour*, and the two programs cannot be compared structurally because
   the passes restructure them.

The two together are the gate. The structural half is exact but only
catches pipeline non-determinism; the behavioural half is the one that
catches a meaning-changing rewrite.

## Coverage and exclusions (explicit)

Covers: every function, block, statement, operand, constant, callee, and
representation (structural, two runs of the same configuration); every
program's stdout and exit status (behavioural, optimised vs unoptimized).

Explicitly excluded from the structural comparison, with the reason:
a local's **debug name** (renaming is semantically irrelevant), and the
lowering-side **metadata** the pass pipeline does not produce. Explicitly
not attempted: a structural comparison of the *optimised* and
*unoptimized* programs — the measurement above shows it rejects correct
optimisations, so it is not a gate. The alternative — proving the two
programs equivalent with an SMT model of MIR — is the Alive2 line the
backlog cites and is out of scope for a debug gate.

## Cost

- **Off:** zero. The driver clones the program only when the mode is set,
  and the CLI wrapper returns before any child is spawned.
- **On:** one extra pass-pipeline run per compilation (structural), and the
  program is compiled and run twice (behavioural). On
  `tests/run/acceptance_static.bdg`, `run --release` is ~1.3 s to 1.4 s of
  compilation; the gate adds a second pipeline (a small fraction of that)
  and a second child compile+run. The behavioural half makes a program with
  external effects happen twice, so the mode is for the test corpus, not
  for such a program; the CLI wrapper skips `--watch`, `--inspect`,
  `--record`, `--bench`, `--debug`, saved binary MIR, and runs above one
  worker (the program's output can depend on the scheduler, so two runs of
  the same code can already differ; the comparison is a question about the
  compiled program and is answered at one worker).

## The deliberately broken rewrite

`crates/badger_mir/src/opt/superopt.rs` gains a debug-only hook
(`BADGER_MIR_BREAK=superopt`, gated on `cfg!(debug_assertions)` so a release
binary cannot take it): the multiplication-by-a-power-of-two rewrite shifts
by one bit too many, so `n * 4` becomes `n << 3`. Without the check the
miscompile is silent (`y=24`); with the check the run fails and reports the
first differing line. `crates/badger_cli/tests/mir_validation.rs` proves
both halves of the acceptance: the correct pipeline passes with the check
on and produces identical output with it off, and the broken rewrite is
caught. The hook and the test land in the commit whose subject is
`test(mir): deliberately broken rewrite the parity gate must catch` (the
gate itself is the preceding commit, `996c018`); the task summary records
that commit's SHA.

## Verification

- `cargo build --workspace`: clean.
- `cargo test --workspace`: pass.
- `cargo test -p badger_cli --test programs` with `BADGER_MIR_VALIDATE=1`
  and without: pass, and the stdout of every `tests/run` program is
  identical with the check on and off.
