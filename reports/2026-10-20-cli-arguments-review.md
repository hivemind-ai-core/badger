# The `badger` command line: a review of its arguments

2026-10-20. The tool grew to 29 commands and 63 flags. This review
looks at how they are named, where they live, and what they do when the
answer is "no"; it changes what was safe to change and records the rest
as proposals.

The rules the surface is measured against:

- **one way to say a thing** — the same idea has the same name and the
  same shape everywhere (a flag is a flag, a path is a path);
- **a flag does what it says** — a flag is never quietly ignored, and a
  value that cannot work is refused where it is written;
- **the parser is the single source of truth** — `--help`, the shell
  completions, and the reference documentation say the same thing, and
  cannot drift apart;
- **a usage error is a usage error** — a bad command, a bad option value,
  and a flag that belongs to another command all end with status 2.

## What was wrong

### One idea, several names

| Idea | Was | Now |
|---|---|---|
| the file or directory a command writes | `-o/--output` (compile, release), `--out` (doc, bench) | `-o/--out` everywhere |
| the project a command works on | a positional path everywhere but `task`, which had `--project DIR` | a positional path everywhere |
| JSON output | `--json` everywhere but `graph`, which had `--format json` | `--json` everywhere (`--format` keeps dot/mermaid) |
| a file to compare with | `--compare FILE` (bench), `--compare` as a switch (replay) | `--baseline FILE` (bench), `--compare` (replay) |
| the value name of `--workers` | `N` everywhere but `bench` (`WORKERS`) | `N` everywhere |

### Flags that did nothing

`badger generate` has six generators and one flat set of flags, so a flag
of one generator was accepted by the others and ignored:

- `badger generate feature orders --check` wrote the files. `--check` is
  `sql`'s flag (compare, do not write); the module generators' flag is
  `--dry-run` (print the plan, write nothing). A byte-for-byte `--check`
  would be wrong for the module generators anyway: their output is
  ordinary code to edit.
- `badger generate sql --dry-run x.sql` wrote the files.
- `badger generate service orders --item order` ignored the record name
  (the service takes it from the module name).

Each is now a usage error (status 2) that names the generator it belongs
to.

### Values the parser did not know

`badger graph bogus`, `badger generate bogus`, and `badger new x --template
bogus` were refused at run time with status 1 and a message of the tool's
own making, while `badger completions tcsh` was refused by the parser
with status 2. The kind arguments are now the parser's: `--help` lists
the possible values, the completions complete them, and an unknown one
is status 2.

`badger query` names an unknown question in the tool's own words
(`unknown query \`bogus\` (diagnostics, check, ...)`), as the other
commands do; the parser still lists the questions in the help and
completes them.

`badger explain` keeps its runtime check: `badger explain E0301` names
the code in the place the kind goes, so the kind cannot be a fixed set.
Its error is now status 2, like the others.

`badger inspect WHAT` has two sets (the program's `plans`, `config`,
`runtime`; a running program's `tasks`, `channels`, `system`, `heap`,
`code`, `all`), so it is checked at run time too. Its error now names
both sets instead of telling the user to start a program it did not ask
for.

### The path of the project, three ways

The current project's directory was the default in three spellings: a
parser default (`check`, `compile`, `search`, ...), a value the command
filled in later (`graph`, `explain`), and a patch-up in the dispatcher
that only `inspect` got (`main.rs`). All of them are parser defaults
now, and the patch-up is gone.

### The completions were a second copy of the interface

`crates/badger_cli/src/completions.rs` held a hand-written table of the
commands and their flags, next to the parser that holds the real ones.
The two had already drifted:

- `--template`, `--open`, `--load`, and `--shell` were not in the table,
  so they were completed nowhere;
- `--compare` was recorded as taking no value, but it takes a file in
  `bench`;
- the command list was in another order than `badger --help`.

The scripts are still written by hand (no completion generator, and no
dependency on one), but what they complete is read from the parser: the
commands, their flags, the values those flags take, and the values of
the first argument where it has a fixed set. `crates/badger_cli/tests/completions.rs`
checks that every command and flag of `--help` is in all four scripts,
that the fixed values are, that the bash script parses, and that every
command is in the reference documentation.

### Exit status

The documentation says status 2 is a usage error and status 1 is a
failure to do the job. The parser produced 2; the tool's own checks
produced 1 for the same class of mistake. They now agree: an unknown
command, an unknown option value, a missing argument, and a flag of
another command are all status 2.

## What was changed

- `crates/badger_cli/src/main.rs` — the arguments, as above.
- `crates/badger_cli/src/completions.rs` — the scripts, now derived from
  the parser.
- `crates/badger_cli/src/lsp.rs` — the questions `badger query` answers
  (`QUERY_KINDS`) live in one place, and the parser uses them.
- `crates/badger_cli/src/bench.rs` — `--baseline` instead of `--compare`.
- `docs/reference/cli.md` — the reference, with the new flags and the
  exit status.
- `crates/badger_cli/tests/completions.rs` — the new tests.

`--output` still works for `compile` and `release` (a hidden alias of
`--out`); `badger task --project` does not (the path is positional now).

## Proposed, not done

These change what a user types, so they need a decision rather than a
patch.

1. **`badger new` and `badger init` are one command.** Both write
   `badger.toml` and an entry module; `new` refuses an existing
   directory, `init` refuses an existing file. One command
   (`badger init [PATH] [--template NAME]`) covers both, and `new` can
   go. The `--lib` flag of `init` and the `lib` template of `new` are the
   same thing twice.
2. **"library" has three spellings**: the `lib` template of `new`, the
   `--lib` flag of `init`, and the `library` generator of `generate`.
   The manifest's `kind` is `library`; the template list should use the
   same word.
3. **`badger query rename` and `badger rename` are one operation**: the
   first returns the edits as JSON, the second applies them. A
   `badger rename --json` that returns the edits without writing them
   would cover the machine-readable case, and `query rename` could go.
4. **`badger check`, `badger query check`, and `badger query
   diagnostics`** overlap: the first prints diagnostics (as text or
   JSON), the second prints them with what was checked, the third prints
   them flattened to one location each. They serve a person, an agent,
   and an editor; the split is defensible, but `check --json` and
   `query check` differ only in what surrounds the diagnostics.
5. **`badger test [PATH] [FILTER]`** is the only command whose
   positional argument means two things (a path if it exists, a filter
   otherwise). `badger run` does the same with a path and an `[entries]`
   name, so the pattern is familiar; a `--filter` flag would end it.
6. **`badger doctor` repeats `badger inspect runtime`.** The doctor
   report is for bug reports, so it may keep the settings; but it could
   point at the command instead of repeating it.

## Notes

- `badger test` and `badger run --watch` re-run through `badger watch`,
  which is why `watch` takes a command after `--`. The three are not
  redundant: `run --watch` replaces code in a running program, `test
  --watch` re-runs the tests, and `watch` runs any command.
- `badger bench` (a suite of programs) and `badger run --bench` (the
  time of one program) are different things that share a word.
