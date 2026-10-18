use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

mod doc;
mod generate;
mod graph;
mod lsp;
mod mcp;
use badger_base::diagnostics::json_str;
use badger_driver::{Outcome, Session};

#[derive(Parser)]
#[command(name = "badger", version, about = "Badger: compiler, JIT runner, and tools (draft-0)")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Parse and statically check a program without running it.
    Check {
        /// Entry file or project directory (with `badger.toml`; default `.`).
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Emit diagnostics as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Check a program and save it as binary MIR, which `badger run FILE`
    /// loads without parsing or checking (only code generation remains).
    Compile {
        /// Entry file or project directory (default: the current project).
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Output file (default: `FILE.bdgmir` beside an entry file;
        /// `DIR/<package>.bdgmir` for a project).
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
        /// Merge `[profile.NAME]` from `badger.toml` into the saved configuration.
        #[arg(long)]
        profile: Option<String>,
        /// Worker threads to save as the default (`badger run --workers` still overrides).
        #[arg(long, value_name = "N")]
        workers: Option<usize>,
    },
    /// Write a self-contained release directory: the `badger` executable,
    /// the program as binary MIR, the native libraries it calls, and a
    /// manifest of their digests. `DIR/run ARGS` (or `badger start DIR`)
    /// starts it without the sources or a compiler installation.
    Release {
        /// Entry file or project directory (default: the current project).
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Release directory (default: `release/<package>`); an earlier
        /// release there is replaced.
        #[arg(short, long, value_name = "DIR")]
        output: Option<PathBuf>,
        /// Merge `[profile.NAME]` from `badger.toml` into the saved configuration.
        #[arg(long)]
        profile: Option<String>,
        /// Worker threads to save as the default.
        #[arg(long, value_name = "N")]
        workers: Option<usize>,
    },
    /// Start a release written by `badger release`: its target, program, and
    /// native libraries (digests and ABI version) are checked before any
    /// program code runs.
    Start {
        /// The release directory.
        dir: PathBuf,
        /// Worker threads (default: the release's saved setting).
        #[arg(long, value_name = "N")]
        workers: Option<usize>,
        /// Arguments passed to the program (after `--`).
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Compile a program with the JIT and run it (a source program, or a
    /// binary MIR file written by `badger compile`).
    Run {
        /// Entry file, project directory (with `badger.toml`), or the name of
        /// an `[entries]` module of the project in the current directory
        /// (default: the current project's main entry).
        entry: Option<String>,
        /// Optimize generated code.
        #[arg(long)]
        release: bool,
        /// Print MIR before code generation.
        #[arg(long)]
        dump_mir: bool,
        /// Merge `[profile.NAME]` from `badger.toml` into the configuration.
        #[arg(long)]
        profile: Option<String>,
        /// Recompile on source changes and replace compatible functions in
        /// the running program (new calls use new code).
        #[arg(long)]
        watch: bool,
        /// Serve inspection snapshots (`badger inspect ... --connect ADDR`).
        #[arg(long, value_name = "ADDR")]
        inspect: Option<String>,
        /// Worker threads running tasks (default: `[runtime] workers`, or 1).
        #[arg(long, value_name = "N")]
        workers: Option<usize>,
        /// With `--watch`: when a change is incompatible (a type definition or
        /// function type changed), cancel the program, halt its resources,
        /// and restart it on the new version.
        #[arg(long, requires = "watch")]
        reconstruct: bool,
        /// Time the program's execution after all compilation (reported on stderr).
        #[arg(long)]
        bench: bool,
        /// Register debug information (source lines, locals, unwinding) for
        /// generated code with a debugger: `gdb --args badger run --debug ...`.
        #[arg(long)]
        debug: bool,
        /// Write a reproduction bundle: the program's identity, the outcome
        /// of every external operation (HTTP, subprocesses, files, the
        /// environment, clocks, random seeds), its output, and how it ended
        /// (`badger replay FILE` runs it again offline). Runs on one worker.
        #[arg(long, value_name = "FILE")]
        record: Option<PathBuf>,
        /// Arguments passed to the program (after `--`).
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Run a program again from a reproduction bundle (`badger run --record`):
    /// every external operation is answered from the bundle, nothing is
    /// requested, run, or written, and an operation the bundle does not hold
    /// ends the replay (exit 3). Exits 0 when the recorded outcome (exit
    /// code, output, failure) is reproduced and 1 when it differs.
    Replay {
        /// The bundle written by `badger run --record`.
        bundle: PathBuf,
        /// Run a changed program against the recording: report differing
        /// operations instead of stopping, and exit with the program's code.
        #[arg(long)]
        compare: bool,
        /// Optimize generated code.
        #[arg(long)]
        release: bool,
    },
    /// Run the `test_*` functions of the entry module and the package modules it imports (not those of dependencies): `badger test [path] [filter]`
    /// (path defaults to the current project).
    Test {
        /// An optional program path, then an optional name filter.
        #[arg(num_args = 0..=2)]
        args: Vec<String>,
        /// Print the results as one JSON object (with each failure's kind,
        /// message, location, and duration).
        #[arg(long)]
        json: bool,
        /// Print the selected tests' names instead of running them.
        #[arg(long)]
        list: bool,
        /// The filter is one test's full name, or its function's name.
        #[arg(long)]
        exact: bool,
    },
    /// Run a user task: a typed function declared in the project's
    /// `[tasks]` table (`name = "src/file.bdg#function"`).
    Task {
        /// The task's name in `[tasks]`.
        name: String,
        /// Project directory (default `.`).
        #[arg(long, default_value = ".")]
        project: PathBuf,
        /// Merge `[profile.NAME]` from `badger.toml` into the configuration.
        #[arg(long)]
        profile: Option<String>,
        /// Arguments passed to the task (after `--`).
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Rewrite sources in the canonical layout.
    Format {
        /// Files or directories (recursively, `*.bdg`).
        paths: Vec<PathBuf>,
        /// Report files that would change instead of rewriting them.
        #[arg(long)]
        check: bool,
    },
    /// Interactive session: definitions accumulate; expressions evaluate.
    Repl {
        /// Evaluate in a program started with `badger run --inspect ADDR`.
        #[arg(long, value_name = "ADDR")]
        connect: Option<String>,
    },
    /// Inspect program structure.
    Inspect {
        /// `plans` (system graphs), `config` (effective project
        /// configuration), or `runtime` (effective runtime settings and their
        /// sources) of PATH; or, with `--connect`, `tasks`, `channels`,
        /// `system`, `heap`, `code`, or `all` of a running program.
        what: String,
        /// Entry file or project directory (for `plans`, `config`, `runtime`;
        /// default `.`).
        path: Option<PathBuf>,
        /// Print JSON instead of a table.
        #[arg(long)]
        json: bool,
        /// Merge `[profile.NAME]` into what `config` and `runtime` show.
        #[arg(long)]
        profile: Option<String>,
        /// Address of a program started with `badger run --inspect ADDR`.
        #[arg(long, value_name = "ADDR")]
        connect: Option<String>,
    },
    /// Resolve path and git dependencies and rewrite the project's `badger.lock`.
    Update {
        /// Project directory (with `badger.toml`).
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Language server over stdio (diagnostics, hover, definition,
    /// references, rename, symbols, formatting, completion).
    Lsp,
    /// Model Context Protocol server over stdio: a persistent REPL session
    /// ("code mode", with `std.compiler`), checking, queries, and runs.
    Mcp,
    /// Program structure as a graph: `calls`, `modules`, `types`, or
    /// `systems`, in DOT (default), Mermaid, or JSON.
    Graph {
        /// `calls`, `modules`, `types`, or `systems`.
        kind: String,
        /// Entry file or project directory (default `.`).
        path: Option<PathBuf>,
        /// `dot`, `mermaid`, or `json`.
        #[arg(long, default_value = "dot")]
        format: String,
        /// Include standard-library nodes.
        #[arg(long)]
        std: bool,
    },
    /// Answer one question about a program as JSON (1-based positions):
    /// `diagnostics`, `check` (diagnostics with the inputs and roots
    /// checked; with `--overlays`, of proposed contents), `symbols`, `api`
    /// (public interface; PATH may also be a standard module such as
    /// `std.list`), or at `--line`/`--col` `hover`, `definition`,
    /// `references`, `completion`, `rename` (with `--name`: the edits, not
    /// applied).
    Query {
        /// What to ask (see above).
        kind: String,
        /// Source file (checked with its project, if any).
        path: Option<PathBuf>,
        /// Line of the position asked about (1-based).
        #[arg(long)]
        line: Option<usize>,
        /// Column of the position asked about (1-based).
        #[arg(long)]
        col: Option<usize>,
        /// Ask about this source text (checked on its own) instead of a file
        /// (`-` reads it from standard input).
        #[arg(long)]
        source: Option<String>,
        /// The new name, for `rename` (the edits are returned, not applied).
        #[arg(long)]
        name: Option<String>,
        /// Proposed file contents to check in place of the files on disk: a
        /// JSON object from path (relative to the program's directory) to
        /// text, in this file (`-` reads standard input). Nothing is written.
        #[arg(long)]
        overlays: Option<String>,
    },
    /// Reference documentation in Markdown from checked interfaces: `std`
    /// (every standard module, one page each plus an index), a standard
    /// module (`std.list`), or the program at a path.
    Doc {
        /// `std`, a standard module (`std.list`), or a program path.
        target: String,
        /// Write the pages into this directory instead of printing them.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Compare with the pages in `--out` and fail if any differ.
        #[arg(long)]
        check: bool,
    },
    /// Generate code: `sql` turns SQL files of named queries
    /// (`-- name: find_user :one`) into typed functions, `NAME.bdg` next
    /// to each `NAME.sql`. Types come from Postgres (`--database`, or
    /// `DATABASE_URL`) or from a SQLite schema (`--schema`, or the
    /// project's `[database] schema`). `feature NAME [DIR]` adds a typed
    /// feature module (types, an in-memory store, operations, an HTTP
    /// binding, and tests) to the project, after checking it and running
    /// its tests in a copy of the project; existing files are never
    /// replaced.
    Generate {
        /// What to generate: `sql` or `feature`.
        what: String,
        /// The SQL files; for `feature`, its name and the project directory
        /// (default `.`).
        files: Vec<PathBuf>,
        /// `feature`: the name of one record (default: the feature's name
        /// without a final `s`).
        #[arg(long)]
        item: Option<String>,
        /// `feature`: print the plan and check it, but write nothing.
        #[arg(long)]
        dry_run: bool,
        /// Postgres URL to take types from (default: `DATABASE_URL`).
        #[arg(long)]
        database: Option<String>,
        /// SQLite schema file to take types from (default: the project's
        /// `[database] schema`).
        #[arg(long)]
        schema: Option<PathBuf>,
        /// Compare with the generated files and fail if any differ.
        #[arg(long)]
        check: bool,
    },
    /// Each contract clause (`requires`, `ensures`) and what the compiler
    /// decided about it: proven (its runtime check is compiled out),
    /// refuted (an error), or checked at run time, and why.
    Contracts {
        /// Entry file or project directory (default `.`).
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Print JSON.
        #[arg(long)]
        json: bool,
        /// Also run the tests, observing the program: each clause's test
        /// coverage, and candidate clauses that held in every call.
        #[arg(long)]
        tests: bool,
    },
    /// Explain a diagnostic code, or the type/effect of a definition.
    Explain {
        /// `diagnostic`, `type`, or `effect` (or a diagnostic code itself:
        /// `badger explain E0301`).
        kind: String,
        /// Diagnostic code (e.g. E0301), or a definition name.
        target: Option<String>,
        /// Program to examine (for `type` and `effect`).
        path: Option<PathBuf>,
        /// Print JSON.
        #[arg(long)]
        json: bool,
    },
}

/// Exit status of an internal error (a bug in the compiler or runtime,
/// not in the program): `EX_SOFTWARE`.
const INTERNAL_ERROR: i32 = 70;

/// An internal error ends the whole process at once, from whichever thread
/// it happened on: runtime invariants may no longer hold, and a worker
/// thread that died alone could leave the others waiting forever (a
/// supervisor such as systemd restarts the process; the program's own
/// defects are task failures, not this).
fn exit_on_internal_errors() {
    std::panic::set_hook(Box::new(|info| {
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown".into());
        // a reader that stopped early (`badger doc std | head`): end quietly
        // with the status a SIGPIPE death reports in a shell
        if msg.starts_with("failed printing to stdout") && msg.contains("Broken pipe") {
            std::process::exit(141);
        }
        let at = info.location().map(|l| format!(" at {}:{}", l.file(), l.line())).unwrap_or_default();
        let thread = std::thread::current();
        eprintln!(
            "internal error: {msg}{at} (thread {})\nthis is a bug in Badger, not in the program: please report it",
            thread.name().unwrap_or("unnamed")
        );
        if std::env::var_os("RUST_BACKTRACE").is_some() {
            eprintln!("{}", std::backtrace::Backtrace::force_capture());
        } else {
            eprintln!("(RUST_BACKTRACE=1 shows where)");
        }
        std::process::exit(INTERNAL_ERROR);
    }));
}

fn main() -> ExitCode {
    exit_on_internal_errors();
    // `std.compiler` runs this executable for its queries
    if std::env::var_os("BADGER_EXE").is_none()
        && let Ok(exe) = std::env::current_exe()
    {
        // SAFETY: no other threads exist yet
        unsafe { std::env::set_var("BADGER_EXE", exe) };
    }
    let cli = Cli::parse();
    // the compiler recurses once per nesting level of a program (the parser
    // refuses deeper than its limits): give it room in debug builds too.
    // Only the pages it touches become resident.
    let worker = std::thread::Builder::new().name("main".into()).stack_size(512 << 20).spawn(move || {
        // BADGER_PROFILE covers compilation too, not only the program run
        // (sampling the thread that does the work)
        badger_driver::start_profile();
        command(cli)
    });
    match worker.map(|w| w.join()) {
        Ok(Ok(code)) => code,
        // a panic was already reported by the internal-error hook
        Ok(Err(_)) => ExitCode::from(INTERNAL_ERROR as u8),
        Err(e) => {
            eprintln!("error: cannot start: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Run one CLI command.
fn command(mut cli: Cli) -> ExitCode {
    // a missing path means the current directory's project
    if let Command::Inspect { path, connect: None, .. } = &mut cli.command
        && path.is_none()
    {
        *path = Some(PathBuf::from("."));
    }
    let mut session = Session::new();
    match cli.command {
        Command::Run {
            entry,
            release,
            dump_mir,
            profile,
            watch,
            inspect,
            workers,
            reconstruct,
            bench,
            debug,
            record,
            args,
        } => {
            let path = match entry {
                None => PathBuf::from("."),
                Some(e) if Path::new(&e).exists() => PathBuf::from(e),
                Some(e) if looks_like_path(&e) => {
                    eprintln!("error: no such file or directory: {e}");
                    return ExitCode::FAILURE;
                }
                Some(e) => {
                    // a named `[entries]` module of the current project
                    badger_driver::select_entry(Some(e));
                    PathBuf::from(".")
                }
            };
            let opts = badger_driver::RunOptions {
                optimize: release,
                dump_mir,
                tests: false,
                filter: None,
                exact: false,
                list: false,
                json: false,
                profile,
                watch,
                inspect,
                workers,
                reconstruct,
                bench,
                debug,
                record,
                replay: None,
                observe: false,
            };
            run_and_exit(&mut session, &path, args, &opts)
        }
        Command::Replay { bundle, compare, release } => {
            let b = match badger_driver::load_bundle(&bundle) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::FAILURE;
                }
            };
            let program = &b["program"];
            if let Some(name) = program["selected"].as_str() {
                match name.strip_prefix("task:") {
                    Some(t) => badger_driver::select_task(t),
                    None => badger_driver::select_entry(Some(name.to_string())),
                }
            }
            let path = PathBuf::from(program["entry"].as_str().unwrap_or("."));
            let args: Vec<String> = program["args"]
                .as_array()
                .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            let opts = badger_driver::RunOptions {
                optimize: release,
                profile: program["profile"].as_str().map(str::to_string),
                replay: Some((bundle, compare)),
                ..Default::default()
            };
            run_and_exit(&mut session, &path, args, &opts)
        }
        Command::Compile { path, output, profile, workers } => {
            // saved programs get the MIR optimizations of `run --release`
            // (code generation then optimizes as `badger run` is told)
            let opts = badger_driver::RunOptions { profile, workers, optimize: true, ..Default::default() };
            match session.compile_binary(&path, &opts) {
                Ok(bytes) => {
                    let out = output.unwrap_or_else(|| default_binary_name(&path));
                    if let Err(e) = std::fs::write(&out, &bytes) {
                        eprintln!("error: cannot write {}: {e}", out.display());
                        return ExitCode::FAILURE;
                    }
                    report(&session, false);
                    println!("wrote {} ({} bytes)", out.display(), bytes.len());
                    ExitCode::SUCCESS
                }
                Err(_) => {
                    report(&session, false);
                    ExitCode::FAILURE
                }
            }
        }
        Command::Release { path, output, profile, workers } => {
            let opts = badger_driver::RunOptions { profile, workers, optimize: true, ..Default::default() };
            let out = output.unwrap_or_else(|| {
                let stem = default_binary_name(&path);
                PathBuf::from("release").join(stem.file_stem().unwrap_or_default())
            });
            match badger_driver::release::build(&mut session, &path, &out, &opts) {
                Ok(b) => {
                    report(&session, false);
                    let libs = if b.libraries.is_empty() {
                        String::new()
                    } else {
                        format!(", native {}", b.libraries.join(", "))
                    };
                    println!(
                        "wrote release {} ({}: {} bytes{libs}); start it with {}/run",
                        out.display(),
                        b.name,
                        b.program_bytes,
                        out.display()
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    report(&session, false);
                    if let Some(e) = e {
                        eprintln!("error: {e}");
                    }
                    ExitCode::FAILURE
                }
            }
        }
        Command::Start { dir, workers, args } => {
            let program = match badger_driver::release::verify(&dir) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("error: cannot start {}: {e}", dir.display());
                    return ExitCode::FAILURE;
                }
            };
            let opts = badger_driver::RunOptions { workers, optimize: true, ..Default::default() };
            run_and_exit(&mut session, &program, args, &opts)
        }
        Command::Task { name, project, profile, args } => {
            badger_driver::select_task(&name);
            let opts = badger_driver::RunOptions { profile, ..Default::default() };
            run_and_exit(&mut session, &project, args, &opts)
        }
        Command::Test { args, json, list, exact } => {
            // `[path] [filter]`: a first argument naming an existing file or
            // directory is the program; anything else is the filter
            let mut args = args.into_iter();
            let (path, filter) = match args.next() {
                Some(a) if Path::new(&a).exists() => (PathBuf::from(a), args.next()),
                // a mistyped path is not a filter that matches nothing
                Some(a) if looks_like_path(&a) => {
                    eprintln!("error: no such file or directory: {a}");
                    return ExitCode::FAILURE;
                }
                Some(a) => (PathBuf::from("."), Some(a)),
                None => (PathBuf::from("."), None),
            };
            badger_driver::LISTING.store(list, std::sync::atomic::Ordering::Relaxed);
            let opts =
                badger_driver::RunOptions { tests: true, filter, json, list, exact, ..Default::default() };
            // every package of a workspace, else the one program
            for (i, pkg) in badger_driver::packages_of(&path).into_iter().enumerate() {
                let mut s = if i == 0 { std::mem::take(&mut session) } else { Session::new() };
                badger_driver::select_entry(None);
                if s.run(&pkg, vec![], &opts).is_err() {
                    report(&s, json);
                    return ExitCode::FAILURE;
                }
                report(&s, false);
                // a project's other entry points: tests in modules they reach
                for sel in badger_driver::project_entry_points(&pkg) {
                    badger_driver::select_entry(Some(sel));
                    let mut s = Session::new();
                    if s.run(&pkg, vec![], &opts).is_err() {
                        report(&s, json);
                        return ExitCode::FAILURE;
                    }
                }
            }
            exit_now(badger_driver::finish_tests(json))
        }
        Command::Contracts { path, json, tests: true } => match session.contract_test_report(&path) {
            Ok((clauses, suggestions, (ran, failed))) => {
                if json {
                    let v: Vec<serde_json::Value> = clauses.iter().map(|c| c.to_json()).collect();
                    let s: Vec<serde_json::Value> = suggestions.iter().map(|c| c.to_json()).collect();
                    println!(
                        "{}",
                        serde_json::json!({ "ok": failed == 0, "tests": { "ran": ran, "failed": failed }, "clauses": v, "suggestions": s, "diagnostics": diagnostics_json(&session) })
                    );
                } else {
                    report(&session, false);
                    println!("{ran} test(s) ran, {failed} failed");
                    for c in &clauses {
                        let (e, h) = c.tested.unwrap_or((0, 0));
                        let tested = if e == 0 {
                            "never exercised by a test".to_string()
                        } else if h == e {
                            format!("held all {e} time(s) the tests evaluated it")
                        } else {
                            format!("FAILED {} of {e} time(s)", e - h)
                        };
                        println!(
                            "{}:{}: {} {} `{}`: {} ({}); {tested}",
                            c.file, c.line, c.function, c.kind, c.text, c.status, c.reason
                        );
                    }
                    for s in &suggestions {
                        println!(
                            "{}:{}: {} could state `{}` ({})",
                            s.file, s.line, s.function, s.clause, s.basis
                        );
                    }
                }
                if failed == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE }
            }
            Err(_) => {
                report(&session, json);
                ExitCode::FAILURE
            }
        },
        Command::Contracts { path, json, tests: false } => {
            let (outcome, clauses) = session.contract_report(&path);
            if json {
                let v: Vec<serde_json::Value> = clauses.iter().map(|c| c.to_json()).collect();
                // the diagnostics too: a program that does not check says why
                println!(
                    "{}",
                    serde_json::json!({ "ok": outcome == Outcome::Ok, "clauses": v, "diagnostics": diagnostics_json(&session) })
                );
            } else {
                report(&session, false);
                for c in &clauses {
                    println!(
                        "{}:{}: {} {} `{}`: {} ({})",
                        c.file, c.line, c.function, c.kind, c.text, c.status, c.reason
                    );
                }
                let proven = clauses.iter().filter(|c| c.status == "proven").count();
                println!(
                    "{} clause(s): {proven} proven, {} checked at run time, {} refuted",
                    clauses.len(),
                    clauses.iter().filter(|c| c.status == "runtime").count(),
                    clauses.iter().filter(|c| c.status == "refuted").count()
                );
            }
            if outcome == Outcome::Ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        Command::Check { path, json } => {
            let outcome = session.check(&path);
            report(&session, json);
            match outcome {
                Outcome::Ok => ExitCode::SUCCESS,
                Outcome::Errors => ExitCode::FAILURE,
            }
        }
        // no paths: the current directory's sources
        Command::Lsp => ExitCode::from(lsp::serve() as u8),
        Command::Mcp => mcp::serve(),
        Command::Graph { kind, path, format, std } => {
            match graph::graph(&kind, &path.unwrap_or_else(|| PathBuf::from(".")), &format, std) {
                Ok(g) => {
                    // every format ends with a newline
                    if g.ends_with('\n') {
                        print!("{g}")
                    } else {
                        println!("{g}")
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Query { kind, path, line, col, source, name, overlays } => match source
            .map(|src| {
                let src = if src == "-" {
                    let mut t = String::new();
                    std::io::Read::read_to_string(&mut std::io::stdin(), &mut t)
                        .map_err(|e| format!("standard input: {e}"))?;
                    t
                } else {
                    src
                };
                let dir = badger_driver::private_temp_dir("badger-check").map_err(|e| e.to_string())?;
                let f = dir.join("source.bdg");
                std::fs::write(&f, src).map_err(|e| e.to_string())?;
                Ok(f)
            })
            .unwrap_or_else(|| path.ok_or_else(|| "give a PATH or --source".to_string()))
            .and_then(|p| {
                let o = match &overlays {
                    Some(spec) => lsp::read_overlays(spec, &p)?,
                    None => Default::default(),
                };
                lsp::query(&kind, &p, line, col, name.as_deref(), o)
            }) {
            Ok(v) => {
                println!("{v}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
        Command::Doc { target, out, check } => match doc::run(&target, out.as_deref(), check) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
        Command::Generate { what, files, item, dry_run, .. } if what == "feature" => {
            let Some(name) = files.first().map(|f| f.to_string_lossy().to_string()) else {
                eprintln!("error: `badger generate feature` needs a name (`badger generate feature orders`)");
                return ExitCode::from(2);
            };
            let dir = files.get(1).cloned().unwrap_or_else(|| PathBuf::from("."));
            let plan = match generate::plan(&dir, &name, item.as_deref()) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::FAILURE;
                }
            };
            for (p, text) in &plan.files {
                println!("create {} ({} lines)", p.display(), text.lines().count());
            }
            match generate::verify(&dir, &plan) {
                Ok(report) => {
                    let passed =
                        report.lines().find(|l| l.starts_with("test result:")).unwrap_or("").to_string();
                    println!("checked in a copy of the project: {passed}");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::FAILURE;
                }
            }
            if dry_run {
                println!("dry run: nothing written");
                return ExitCode::SUCCESS;
            }
            if let Err(e) = generate::write(&dir, &plan) {
                eprintln!("error: {e}");
                return ExitCode::FAILURE;
            }
            println!(
                "\nnext: import {name}.memory, {name}.service, and {name}.http where the application is composed:\n\n    \
                 store: memory.store(),\n    {name}: service.{name}({{ store: ref(#store) }}),\n\n\
                 and serve `http.routes(running.{name})`; its tests run once an entry imports it (`badger test`)."
            );
            ExitCode::SUCCESS
        }
        Command::Generate { what, files, database, schema, check, .. } => {
            if what != "sql" {
                eprintln!("error: unknown generator `{what}` (available: sql, feature)");
                return ExitCode::FAILURE;
            }
            if files.is_empty() {
                eprintln!(
                    "error: `badger generate sql` needs one or more SQL files (`badger generate sql queries/*.sql`)"
                );
                return ExitCode::from(2);
            }
            let database = database.or_else(|| std::env::var("DATABASE_URL").ok());
            match badger_driver::sqlgen::run(&files, database.as_deref(), schema.as_deref(), check) {
                Ok(written) => {
                    for w in written {
                        println!("wrote {w}");
                    }
                    ExitCode::SUCCESS
                }
                Err(es) => {
                    for e in es {
                        eprintln!("error: {e}");
                    }
                    ExitCode::FAILURE
                }
            }
        }
        Command::Format { paths, check } => {
            format(if paths.is_empty() { vec![PathBuf::from(".")] } else { paths }, check)
        }
        Command::Update { path } => match badger_driver::deps::update(&path) {
            Ok(deps) => {
                for d in &deps {
                    println!("locked {} {} ({})", d.name, d.version, d.hash);
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error[E0015]: {e}");
                ExitCode::FAILURE
            }
        },
        Command::Repl { connect } => match connect {
            Some(addr) => remote_repl(&addr),
            None => repl(),
        },
        Command::Inspect { what, json, connect: Some(addr), .. } => {
            const REMOTE: [&str; 6] = ["tasks", "channels", "system", "heap", "code", "all"];
            if !REMOTE.contains(&what.as_str()) {
                eprintln!(
                    "error: `{what}` is not a snapshot of a running program (one of: {})",
                    REMOTE.join(", ")
                );
                return ExitCode::from(2);
            }
            match badger_driver::inspect::request(&addr, &what) {
                Ok(text) if text.starts_with("{\"error\":") => {
                    let e = serde_json::from_str::<serde_json::Value>(&text)
                        .ok()
                        .and_then(|v| v["error"].as_str().map(str::to_string))
                        .unwrap_or(text);
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
                Ok(text) => {
                    if json {
                        println!("{text}");
                    } else {
                        print_snapshot(&what, &text);
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Inspect { what, path: Some(path), json, profile, .. } if what == "config" => {
            match Session::effective_config(&path, profile.as_deref()) {
                Ok(entries) => {
                    let src = |s: &badger_driver::manifest::Source| match s {
                        badger_driver::manifest::Source::Base => "config".to_string(),
                        badger_driver::manifest::Source::Profile(p) => format!("profile.{p}"),
                    };
                    if json {
                        let es: Vec<String> = entries
                            .iter()
                            .map(|(k, v, s)| {
                                format!(
                                    "{{\"key\":{},\"value\":{},\"source\":{}}}",
                                    json_str(k),
                                    json_str(v),
                                    json_str(&src(s))
                                )
                            })
                            .collect();
                        println!("[{}]", es.join(","));
                    } else {
                        for (k, v, s) in &entries {
                            println!("{k} = {v}    # from [{}]", src(s));
                        }
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error[E0013]: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Inspect { what, path: Some(path), json, profile, .. } if what == "runtime" => {
            // effective runtime settings and where each comes from
            let rows = match Session::runtime_settings_with_sources(&path, profile.as_deref()) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("error[E0013]: {e}");
                    return ExitCode::FAILURE;
                }
            };
            if json {
                let es: Vec<String> = rows
                    .iter()
                    .map(|(k, v, s)| {
                        format!(
                            "{{\"key\":{},\"value\":{},\"source\":{}}}",
                            json_str(k),
                            json_str(v),
                            json_str(s)
                        )
                    })
                    .collect();
                println!("[{}]", es.join(","));
            } else {
                for (k, v, s) in &rows {
                    if s == "default" || s == "env" {
                        println!("{k} = {v}    # {s}");
                    } else {
                        println!("{k} = {v}    # from [{s}]");
                    }
                }
            }
            ExitCode::SUCCESS
        }
        Command::Inspect { path: None, .. } => unreachable!("defaulted above"),
        Command::Inspect { what, path: Some(path), json, .. } => {
            if what != "plans" {
                eprintln!(
                    "error: `badger inspect {what}` reads a running program: start it with `badger run --inspect ADDR` and pass `--connect ADDR`"
                );
                return ExitCode::FAILURE;
            }
            let Some(plans) = session.inspect_plans(&path) else {
                report(&session, json);
                return ExitCode::FAILURE;
            };
            if json {
                let ps: Vec<String> = plans
                    .iter()
                    .map(|p| {
                        let nodes: Vec<String> = p
                            .nodes
                            .iter()
                            .map(|n| {
                                let refs: Vec<String> = n.refs.iter().map(|r| json_str(r)).collect();
                                format!(
                                    "{{\"key\":{},\"provider\":{},\"refs\":[{}]}}",
                                    json_str(&n.key),
                                    json_str(&n.provider),
                                    refs.join(",")
                                )
                            })
                            .collect();
                        let order: Vec<String> = p.startup_order().iter().map(|k| json_str(k)).collect();
                        format!(
                            "{{\"kind\":{},\"line\":{},\"nodes\":[{}],\"startup\":[{}]}}",
                            json_str(p.kind),
                            p.line,
                            nodes.join(","),
                            order.join(",")
                        )
                    })
                    .collect();
                println!("[{}]", ps.join(","));
            } else {
                for p in &plans {
                    if p.kind == "pipeline" {
                        let stages: Vec<String> = p
                            .nodes
                            .iter()
                            .map(|n| {
                                let first = n.provider.lines().next().unwrap_or("").trim().to_string();
                                if first.len() < n.provider.trim().len() {
                                    format!("{first} ...")
                                } else {
                                    first
                                }
                            })
                            .collect();
                        println!("pipeline at line {}: {}", p.line, stages.join(" -> "));
                        continue;
                    }
                    println!("system plan at line {}", p.line);
                    for n in &p.nodes {
                        let refs = if n.refs.is_empty() {
                            String::new()
                        } else {
                            format!(
                                " <- {}",
                                n.refs.iter().map(|r| format!("#{r}")).collect::<Vec<_>>().join(", ")
                            )
                        };
                        println!("  #{} = {}{refs}", n.key, n.provider);
                    }
                    let order: Vec<String> = p.startup_order().iter().map(|k| format!("#{k}")).collect();
                    println!("  startup order: {}", order.join(" -> "));
                }
            }
            ExitCode::SUCCESS
        }
        Command::Explain { kind, target, path, json } => {
            // `badger explain E0301` names the code directly
            let is_code = |s: &str| {
                s.len() == 5 && s[..1].eq_ignore_ascii_case("e") && s[1..].bytes().all(|b| b.is_ascii_digit())
            };
            let (kind, target) = match target {
                Some(t) => (kind, t),
                None if is_code(&kind) => ("diagnostic".to_string(), kind),
                None => {
                    eprintln!(
                        "error: `badger explain {kind}` needs what to explain: a diagnostic code, or a definition name (`badger explain type NAME FILE`)"
                    );
                    return ExitCode::FAILURE;
                }
            };
            match kind.as_str() {
                "diagnostic" => match badger_base::catalog::explain(&target) {
                    Some((title, text)) => {
                        if json {
                            println!(
                                "{{\"code\":{},\"title\":{},\"explanation\":{}}}",
                                json_str(&target.to_uppercase()),
                                json_str(title),
                                json_str(text)
                            );
                        } else {
                            println!("{} {title}\n\n{text}", target.to_uppercase());
                        }
                        ExitCode::SUCCESS
                    }
                    None => {
                        eprintln!("error: unknown diagnostic code `{target}`");
                        ExitCode::FAILURE
                    }
                },
                "type" | "effect" => {
                    let path = path.unwrap_or_else(|| PathBuf::from("."));
                    match session.explain_def(&path, &target) {
                        Some((ty, eff)) => {
                            let v = if kind == "type" { ty } else { eff };
                            if json {
                                println!("{{\"name\":{},\"{kind}\":{}}}", json_str(&target), json_str(&v));
                            } else {
                                println!("{target}: {v}");
                            }
                            ExitCode::SUCCESS
                        }
                        None => {
                            report(&session, json);
                            eprintln!("error: no definition `{target}` in the entry module");
                            ExitCode::FAILURE
                        }
                    }
                }
                other => {
                    eprintln!("error: unknown explain kind `{other}` (expected diagnostic, type, or effect)");
                    ExitCode::FAILURE
                }
            }
        }
    }
}

fn source_files(p: &Path, out: &mut Vec<PathBuf>) {
    if p.is_dir() {
        let mut entries: Vec<std::fs::DirEntry> =
            std::fs::read_dir(p).into_iter().flatten().flatten().collect();
        entries.sort_by_key(|e| e.path());
        for e in entries {
            // a symlinked directory is not followed (a link back up would
            // never end)
            if e.file_type().is_ok_and(|t| t.is_symlink()) && e.path().is_dir() {
                continue;
            }
            source_files(&e.path(), out);
        }
    } else if p.extension().is_some_and(|x| x == "bdg") {
        out.push(p.to_path_buf());
    }
}

fn format(paths: Vec<PathBuf>, check: bool) -> ExitCode {
    let mut files = vec![];
    for p in &paths {
        // a path given by name must be there, and be a source or a directory
        if !p.exists() {
            eprintln!("error: no such file or directory: {}", p.display());
            return ExitCode::FAILURE;
        }
        if p.is_file() && p.extension().is_none_or(|x| x != "bdg") {
            eprintln!("error: {} is not a Badger source (`.bdg`)", p.display());
            return ExitCode::FAILURE;
        }
        source_files(p, &mut files);
    }
    let mut changed = 0;
    for f in files {
        let src = match std::fs::read_to_string(&f) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: cannot read {}: {e}", f.display());
                return ExitCode::FAILURE;
            }
        };
        let out = badger_syntax::format::format(&src);
        if out != src {
            changed += 1;
            if check {
                println!("would reformat {}", f.display());
            } else if let Err(e) = std::fs::write(&f, out) {
                eprintln!("error: cannot write {}: {e}", f.display());
                return ExitCode::FAILURE;
            }
        }
    }
    if check && changed > 0 { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}

/// Human-readable rendering of an inspection snapshot (JSON from the
/// running program's endpoint).
fn print_snapshot(what: &str, text: &str) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
        println!("{text}");
        return;
    };
    // a field as text: strings bare, `null` as `-`, anything else as JSON
    let field = |o: &serde_json::Value, key: &str| -> String {
        match &o[key] {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Null => "-".to_string(),
            other => other.to_string(),
        }
    };
    let items = |v: &serde_json::Value| v.as_array().cloned().unwrap_or_default();
    match what {
        "tasks" => {
            println!(
                "{:>5}  {:<9} {:<16} {:>6} {:>4} {:>8}  {:<18} name",
                "id", "state", "waiting on", "scope", "code", "stack", "stage"
            );
            for t in items(&v) {
                let stack = t["memory"]["stack_resident"].as_u64().map(|b| format!("{}K", b / 1024));
                println!(
                    "{:>5}  {:<9} {:<16} {:>6} {:>4} {:>8}  {:<18} {}",
                    field(&t, "id"),
                    field(&t, "state"),
                    field(&t, "wait"),
                    field(&t, "scope"),
                    format!("v{}", field(&t, "code_version")),
                    stack.unwrap_or_else(|| "-".into()),
                    field(&t, "stage"),
                    field(&t, "name")
                );
            }
        }
        "channels" => {
            println!(
                "{:>5}  {:<7} {:>8} {:>7} {:>7} {:>7}  waiting(s/r)  element",
                "id", "kind", "capacity", "queued", "staged", "closed"
            );
            for c in items(&v) {
                println!(
                    "{:>5}  {:<7} {:>8} {:>7} {:>7} {:>7}  {:>5}/{:<6}  {}",
                    field(&c, "id"),
                    field(&c, "kind"),
                    field(&c, "capacity"),
                    field(&c, "queued"),
                    field(&c, "staged"),
                    field(&c, "closed"),
                    field(&c, "waiting_senders"),
                    field(&c, "waiting_receivers"),
                    field(&c, "element")
                );
            }
        }
        "system" => {
            for s in items(&v) {
                println!("system {} ({})", field(&s, "id"), field(&s, "state"));
                for n in items(&s["nodes"]) {
                    println!(
                        "  :{} = {} (generation {})",
                        field(&n, "key"),
                        field(&n, "provider"),
                        field(&n, "generation")
                    );
                }
            }
        }
        _ => println!("{text}"),
    }
}

/// REPL connected to a running program: definitions become new compatible
/// code versions; expressions run inside the program; `:tasks`, `:channels`,
/// `:system`, `:heap`, `:code` show inspection snapshots.
fn remote_repl(addr: &str) -> ExitCode {
    if repl_loop(addr, "remote> ") == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

/// Read inputs and evaluate each in the program at `addr`; `:tasks`,
/// `:channels`, `:system`, `:heap`, `:code` show inspection snapshots.
fn repl_loop(addr: &str, prompt_text: &str) -> i32 {
    let interactive = std::io::stdin().is_terminal();
    let prompt = |cont: bool| {
        if interactive {
            print!("{}", if cont { "... " } else { prompt_text });
            let _ = std::io::stdout().flush();
        }
    };
    let mut buf = String::new();
    prompt(false);
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        buf.push_str(&line);
        buf.push('\n');
        if depth(&buf) > 0 {
            prompt(true);
            continue;
        }
        let input = std::mem::take(&mut buf);
        let trimmed = input.trim();
        if trimmed == ":quit" || trimmed == ":q" {
            break;
        }
        if trimmed.is_empty() {
            prompt(false);
            continue;
        }
        let reply = match trimmed.strip_prefix(':') {
            Some(what) => badger_driver::inspect::request(addr, what).map(|t| {
                print_snapshot(what, &t);
                String::new()
            }),
            None => badger_driver::inspect::eval(addr, trimmed),
        };
        match reply {
            Ok(text) if !text.is_empty() => println!("{text}"),
            Ok(_) => {}
            Err(e) => {
                eprintln!("error: {e}");
                return 1;
            }
        }
        prompt(false);
    }
    0
}

/// Local REPL: an explicit session program runs with live dispatch, and
/// each input is compiled into it as a new compatible code version (the
/// same machinery as `badger repl --connect`). Definitions accumulate;
/// module-level `let` values are computed once and keep their identity;
/// expressions evaluate inside the session.
/// A REPL session program, ready to run as an inspection endpoint: the
/// private directory holding it and the local address it will serve.
pub(crate) struct SessionSetup {
    pub dir: PathBuf,
    pub file: PathBuf,
    pub addr: String,
}

/// Write the session program into a fresh private directory and reserve a
/// local port for it (the errors are printed).
pub(crate) fn prepare_session(prefix: &str) -> Option<SessionSetup> {
    let dir = match badger_driver::private_temp_dir(prefix) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: cannot create a session directory: {e}");
            return None;
        }
    };
    let file = dir.join("session.bdg");
    let session = "// session program\nimport std.io\nimport std.task\n\npub fn main() -> Unit {\n    loop {\n        task.sleep(3600s)\n    }\n}\n";
    if std::fs::write(&file, session).is_err() {
        eprintln!("error: cannot write the session file");
        return None;
    }
    let port = match std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()) {
        Ok(a) => a.port(),
        Err(e) => {
            eprintln!("error: cannot reserve a local port: {e}");
            return None;
        }
    };
    Some(SessionSetup { dir, file, addr: format!("127.0.0.1:{port}") })
}

/// Wait until the session program serves `addr` (the process ends when it
/// has not started within 30 seconds).
pub(crate) fn wait_for_session(addr: &str) {
    let start = std::time::Instant::now();
    while std::net::TcpStream::connect(addr).is_err() {
        if start.elapsed() > std::time::Duration::from_secs(30) {
            eprintln!("error: the session did not start");
            exit_now(1);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// Run the session program on this thread (it returns only when the process
/// ends, or when the program does not build).
pub(crate) fn run_session(setup: &SessionSetup) -> ExitCode {
    // session bindings are interactive state, not a program's module values
    // (which must be pure immutable data)
    badger_driver::set_repl_session();
    let mut s = Session::new();
    let opts = badger_driver::RunOptions { inspect: Some(setup.addr.clone()), ..Default::default() };
    run_and_exit(&mut s, &setup.file, vec![], &opts)
}

fn repl() -> ExitCode {
    let Some(setup) = prepare_session("badger-repl") else { return ExitCode::FAILURE };
    let (addr, cleanup) = (setup.addr.clone(), setup.dir.clone());
    std::thread::spawn(move || {
        wait_for_session(&addr);
        let code = repl_loop(&addr, "badger> ");
        let _ = std::fs::remove_dir_all(&cleanup);
        exit_now(code)
    });
    run_session(&setup)
}

fn depth(s: &str) -> i32 {
    let mut d = 0;
    let mut in_str = false;
    let mut prev = ' ';
    for c in s.chars() {
        match c {
            '"' if prev != '\\' => in_str = !in_str,
            '{' | '(' | '[' if !in_str => d += 1,
            '}' | ')' | ']' if !in_str => d -= 1,
            _ => {}
        }
        prev = c;
    }
    d
}

/// Run a program, report its diagnostics, and leave the process with its
/// exit code (or failure, when it did not build).
fn run_and_exit(
    session: &mut Session,
    path: &Path,
    args: Vec<String>,
    opts: &badger_driver::RunOptions,
) -> ExitCode {
    let r = session.run(path, args, opts);
    report(session, false);
    match r {
        Ok(code) => exit_now(code),
        Err(_) => ExitCode::FAILURE,
    }
}

/// Leave the process after a program ran: the runtime's task stacks, JIT
/// image, and heap live for the whole process, so skip their destructors.
pub(crate) fn exit_now(code: i32) -> ExitCode {
    badger_driver::finish();
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    // Abandoned task stacks may still be mid-call; process teardown hooks
    // must not run on their behalf.
    unsafe { libc::_exit(code) }
}

/// The session's diagnostics as JSON (the `check --json` form).
fn diagnostics_json(session: &Session) -> serde_json::Value {
    serde_json::from_str(&session.diagnostics.render_json(&session.sources)).unwrap_or_default()
}

pub(crate) fn report(session: &Session, json: bool) {
    if json {
        println!("{}", session.diagnostics.render_json(&session.sources));
    } else {
        let color = std::io::stderr().is_terminal();
        let _ = session.diagnostics.render_with(&session.sources, &mut std::io::stderr(), color);
    }
}

/// `prog.bdg` -> `prog.bdgmir`; a project directory -> `<package>.bdgmir`
/// in it (the directory's name when there is no manifest name).
fn default_binary_name(path: &Path) -> PathBuf {
    if path.is_dir() {
        return path.join(format!("{}.bdgmir", badger_driver::release::release_name(path)));
    }
    path.with_extension("bdgmir")
}

/// An argument meant as a path (not an entry name or a test filter).
fn looks_like_path(a: &str) -> bool {
    a.contains('/') || a.ends_with(".bdg") || a.ends_with(".bdgmir")
}
