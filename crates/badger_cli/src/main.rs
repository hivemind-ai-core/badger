use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

mod bench;
mod completions;
mod doc;
mod doctor;
mod generate;
mod graph;
mod init;
mod lsp;
mod mcp;
mod search;
mod templates;
mod watch;
use badger_base::diagnostics::json_str;
use badger_driver::{Outcome, Session};

/// The MIR optimization levels `--opt` takes.
const OPT_LEVELS: [&str; 3] = ["release", "aggressive", "superopt"];

/// The graphs `badger graph` draws.
const GRAPH_KINDS: [&str; 4] = ["calls", "modules", "types", "systems"];

/// The generators `badger generate` runs.
const GENERATORS: [&str; 6] = ["sql", "feature", "worker", "service", "cli", "library"];

/// What `badger inspect` reads from a program or a project.
const INSPECT_LOCAL: [&str; 3] = ["plans", "config", "runtime"];

/// What `badger inspect --connect` reads from a running program.
const INSPECT_REMOTE: [&str; 6] = ["tasks", "channels", "system", "heap", "code", "all"];

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
        #[arg(short = 'o', long = "out", alias = "output", value_name = "FILE")]
        output: Option<PathBuf>,
        /// Merge `[profile.NAME]` from `badger.toml` into the saved configuration.
        #[arg(long)]
        profile: Option<String>,
        /// Worker threads to save as the default (`badger run --workers` still overrides).
        #[arg(long, value_name = "N")]
        workers: Option<usize>,
        /// MIR optimization level: `release` (default), `aggressive`, or
        /// `superopt`.
        #[arg(long, value_name = "LEVEL", value_parser = OPT_LEVELS)]
        opt: Option<String>,
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
        #[arg(short = 'o', long = "out", alias = "output", value_name = "DIR")]
        output: Option<PathBuf>,
        /// Merge `[profile.NAME]` from `badger.toml` into the saved configuration.
        #[arg(long)]
        profile: Option<String>,
        /// Worker threads to save as the default.
        #[arg(long, value_name = "N")]
        workers: Option<usize>,
        /// MIR optimization level: `release` (default), `aggressive`, or
        /// `superopt`.
        #[arg(long, value_name = "LEVEL", value_parser = OPT_LEVELS)]
        opt: Option<String>,
    },
    /// Write a self-contained release directory: its target, program, and
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
        /// Print MIR before code generation (after MIR optimization).
        #[arg(long)]
        dump_mir: bool,
        /// MIR optimization level with `--release`: `release` (default),
        /// `aggressive` (repeat inlining and simplification), or `superopt`
        /// (also search for cheaper arithmetic and boolean forms).
        #[arg(long, value_name = "LEVEL", requires = "release", value_parser = OPT_LEVELS)]
        opt: Option<String>,
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
    /// Re-run a command when a source file of the project changes: the
    /// default is `badger check PATH`, and with `-- COMMAND...` the
    /// given command runs instead (in the project's directory). The
    /// files are polled, so the command needs no file-watching
    /// dependency. The command's exit status is reported, but does not
    /// end the watch (Ctrl-C does).
    Watch {
        /// Project directory to watch (default `.`).
        #[arg(default_value = ".")]
        path: PathBuf,
        /// The command to run on a change (after `--`); the default is
        /// `badger check PATH`.
        #[arg(last = true)]
        command: Vec<String>,
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
    /// Run the `test_*` functions of the entry module and the package modules it imports (not those of dependencies): `badger test [PATH]`
    /// (path defaults to the current project).
    Test {
        /// The program or project to test (default `.`).
        #[arg(default_value = ".")]
        path: PathBuf,
        /// The tests to run: the ones whose name contains it.
        #[arg(long, value_name = "NAME")]
        filter: Option<String>,
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
        /// Re-run the tests when a source file of the project changes.
        #[arg(long)]
        watch: bool,
    },
    /// Run a user task: a typed function declared in the project's
    /// `[tasks]` table (`name = "src/file.bdg#function"`).
    Task {
        /// The task's name in `[tasks]`.
        name: String,
        /// Project directory (default `.`).
        #[arg(default_value = ".")]
        path: PathBuf,
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
        #[arg(default_value = ".")]
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
        /// Load a file's definitions into the session before the prompt.
        #[arg(long, value_name = "FILE")]
        load: Option<PathBuf>,
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
        #[arg(default_value = ".")]
        path: PathBuf,
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
        #[arg(value_parser = GRAPH_KINDS)]
        kind: String,
        /// Entry file or project directory (default `.`).
        #[arg(default_value = ".")]
        path: PathBuf,
        /// `dot`, `mermaid`, or `json`.
        #[arg(long, default_value = "dot", value_parser = ["dot", "mermaid", "json"])]
        format: String,
        /// Print JSON (the same as `--format json`).
        #[arg(long)]
        json: bool,
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
        #[arg(value_parser = lsp::QueryKind)]
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
        #[arg(short = 'o', long = "out", value_name = "DIR")]
        out: Option<PathBuf>,
        /// Compare with the pages in `--out` and fail if any differ.
        #[arg(long, requires = "out")]
        check: bool,
        /// Write the pages as one HTML file and open it.
        #[arg(long)]
        open: bool,
    },
    /// Rename a definition and every use of it: the edits are computed
    /// and applied to the files.
    Rename {
        /// The file the definition (or a use of it) is in.
        path: PathBuf,
        /// The line of the definition (1-based).
        #[arg(long)]
        line: usize,
        /// The column of the definition (1-based).
        #[arg(long)]
        col: usize,
        /// The new name.
        #[arg(long)]
        name: String,
        /// Print the edits as JSON and write nothing.
        #[arg(long)]
        json: bool,
    },
    /// Run the benchmark programs and report their measurements.
    Bench {
        /// The directory holding the benchmark programs (default `bench`).
        #[arg(default_value = "bench")]
        path: PathBuf,
        /// Write the measurements to this file as JSON.
        #[arg(short = 'o', long = "out", value_name = "FILE")]
        out: Option<PathBuf>,
        /// Compare the measurements with a baseline file written by `--out`.
        #[arg(long, value_name = "FILE")]
        baseline: Option<PathBuf>,
        /// The number of workers the programs run with.
        #[arg(long, value_name = "N")]
        workers: Option<usize>,
    },
    /// Turn a directory into a project: `badger.toml` and
    /// `src/main.bdg` (a program with a test). The directory is created
    /// when it does not exist; an existing file is never overwritten.
    Init {
        /// The directory of the project (default `.`).
        #[arg(default_value = ".")]
        path: PathBuf,
        /// The shape of the project: `cli`, `service`, `framework`,
        /// `library`, or `worker` (default: a program with a test).
        #[arg(long, value_name = "NAME", value_parser = templates::names())]
        template: Option<String>,
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
        /// What to generate: `sql`, `feature`, `worker`, `service`, `cli`,
        /// or `library`.
        #[arg(value_parser = GENERATORS)]
        what: String,
        /// The SQL files; for the other generators, its name and the project
        /// directory (default `.`).
        files: Vec<PathBuf>,
        /// `feature`: the name of one record (default: the feature's name
        /// without a final `s`).
        #[arg(long)]
        item: Option<String>,
        /// The other generators: print the plan and check it, but write
        /// nothing.
        #[arg(long)]
        dry_run: bool,
        /// Postgres URL to take types from (default: `DATABASE_URL`).
        #[arg(long)]
        database: Option<String>,
        /// SQLite schema file to take types from (default: the project's
        /// `[database] schema`).
        #[arg(long)]
        schema: Option<PathBuf>,
        /// `sql`: compare with the generated files and fail if any differ.
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
    /// Report the environment for bug reports: the version, the current
    /// directory, the project (its name, its manifest, its sources), the
    /// effective runtime settings, and the `BADGER_*` environment
    /// variables that are set.
    Doctor {
        /// Print the report as a JSON object.
        #[arg(long)]
        json: bool,
    },
    /// Print a shell completion script for the given shell, so the
    /// command names and their flags are not typed by hand.
    Completions {
        /// The shell to write the script for.
        #[arg(value_parser = ["bash", "zsh", "fish", "powershell"])]
        shell: String,
    },
    /// Find a definition in the standard library and in the project's
    /// modules: the name (or a word of it) is looked for in the names
    /// and in the documentation.
    Search {
        /// The text to look for.
        query: String,
        /// The project to search as well (default `.`).
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Print the matches as JSON.
        #[arg(long)]
        json: bool,
    },
}

/// Exit status of an internal error (a bug in the compiler or runtime,
/// not in the program): `EX_SOFTWARE`.
const INTERNAL_ERROR: i32 = 70;

/// A flag that belongs to another command: a usage error (status 2),
/// never a flag that is quietly ignored.
fn wrong_flag(what: &str, flag: &str) -> ExitCode {
    eprintln!("error: `--{flag}` is not for `badger generate {what}`");
    ExitCode::from(2)
}

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

/// `BADGER_MIR_VALIDATE=1` for `badger run`: run the program twice — the
/// optimized build and the build with the MIR optimizer off — and fail when
/// their stdout or exit status differs.
///
/// The optimizer restructures the program by design (inlining, dead-code
/// removal, representation changes), so the two programs cannot be compared
/// structurally; running both and comparing what they print is the sound
/// translation-validation question. `BADGER_NO_INLINE=1` turns the whole
/// release MIR pipeline off, so the reference is the program the
/// unoptimized pipeline produced. The driver's structural check
/// (`badger_mir::validate`) covers the pipeline's determinism in the same
/// mode.
///
/// Returns `Some` when the check ran (the process then exits with the
/// returned code); `None` when it does not apply. Each child runs the
/// program once, so a program with external effects happens twice: the mode
/// is for the test corpus, not for such a program.
fn mir_validate_run() -> Option<ExitCode> {
    if std::env::var_os("BADGER_MIR_VALIDATE").is_none() {
        return None;
    }
    // The two children keep the mode on so the driver's structural check
    // runs inside them; the marker stops the wrapper from re-entering.
    if std::env::var_os("BADGER_MIR_VALIDATE_CHILD").is_some() {
        return None;
    }
    let argv: Vec<std::ffi::OsString> = std::env::args_os().collect();
    if argv.get(1).is_none_or(|a| a != "run") {
        return None;
    }
    let flags: Vec<String> = argv[2..].iter().map(|a| a.to_string_lossy().into_owned()).collect();
    if !flags.iter().any(|f| f == "--release") {
        return None;
    }
    // an interactive, recording, or benchmarking run has effects the check
    // must not double, and saved binary MIR has no unoptimized form
    if flags.iter().any(|f| matches!(f.as_str(), "--watch" | "--inspect" | "--record" | "--bench" | "--debug")) {
        return None;
    }
    if flags.iter().any(|f| f.ends_with(".bdgmir")) {
        return None;
    }
    // Above one worker the program's output can depend on the scheduler, so
    // two runs of the *same* code can already differ; the comparison is a
    // question about the compiled program and is answered at one worker.
    if workers_of(&flags).is_some_and(|n| n != 1) {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    let run = |unoptimized: bool| {
        let mut c = std::process::Command::new(&exe);
        c.args(&argv[1..]);
        c.env("BADGER_MIR_VALIDATE_CHILD", "1");
        if unoptimized {
            c.env("BADGER_NO_INLINE", "1");
        }
        c.output().ok()
    };
    let optimized = run(false)?;
    let reference = run(true)?;
    if optimized.stdout == reference.stdout && optimized.status.code() == reference.status.code() {
        let _ = std::io::stdout().write_all(&optimized.stdout);
        let _ = std::io::stderr().write_all(&optimized.stderr);
        return Some(ExitCode::from(optimized.status.code().unwrap_or(1).clamp(0, 255) as u8));
    }
    eprintln!("error: MIR translation validation failed: the optimized and unoptimized programs differ");
    eprintln!("[mir-validate] {}", first_differing_line(&optimized.stdout, &reference.stdout));
    Some(ExitCode::FAILURE)
}

/// The `--workers N` (or `--workers=N`) value in a command's arguments.
fn workers_of(flags: &[String]) -> Option<usize> {
    let mut it = flags.iter();
    while let Some(f) = it.next() {
        if let Some(v) = f.strip_prefix("--workers=") {
            return v.parse().ok();
        }
        if f == "--workers" {
            return it.next().and_then(|v| v.parse().ok());
        }
    }
    None
}

/// The first line two outputs disagree on, with its 1-based position.
fn first_differing_line(a: &[u8], b: &[u8]) -> String {
    let al: Vec<&[u8]> = a.split(|c| *c == b'\n').collect();
    let bl: Vec<&[u8]> = b.split(|c| *c == b'\n').collect();
    for (i, (x, y)) in al.iter().zip(&bl).enumerate() {
        if x != y {
            return format!(
                "line {}: optimized `{}`, unoptimized `{}`",
                i + 1,
                String::from_utf8_lossy(x),
                String::from_utf8_lossy(y)
            );
        }
    }
    format!("optimized {} lines, unoptimized {} lines", al.len(), bl.len())
}

fn main() -> ExitCode {
    exit_on_internal_errors();
    if let Some(code) = mir_validate_run() {
        return code;
    }
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

/// Parse `--opt` into a MIR level, reporting an unknown name.
fn opt_level(opt: Option<String>) -> Option<Option<badger_mir::opt::Level>> {
    match opt.as_deref().map(badger_mir::opt::Level::parse) {
        None => Some(None),
        Some(Some(level)) => Some(Some(level)),
        Some(None) => {
            eprintln!("error: --opt takes release, aggressive, or superopt");
            None
        }
    }
}

/// Run one CLI command.
fn command(cli: Cli) -> ExitCode {
    let mut session = Session::new();
    match cli.command {
        Command::Run {
            entry,
            release,
            dump_mir,
            opt,
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
            let Some(opt) = opt_level(opt) else { return ExitCode::from(2) };
            let opts = badger_driver::RunOptions {
                optimize: release,
                opt,
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
        Command::Compile { path, output, profile, workers, opt } => {
            // saved programs get the MIR optimizations of `run --release`
            // (code generation then optimizes as `badger run` is told); the
            // level is recorded with the program, and a later run applies a
            // higher requested level
            let Some(opt) = opt_level(opt) else { return ExitCode::from(2) };
            let opts =
                badger_driver::RunOptions { profile, workers, optimize: true, opt, ..Default::default() };
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
        Command::Release { path, output, profile, workers, opt } => {
            let Some(opt) = opt_level(opt) else { return ExitCode::from(2) };
            let opts =
                badger_driver::RunOptions { profile, workers, optimize: true, opt, ..Default::default() };
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
        Command::Task { name, path, profile, args } => {
            badger_driver::select_task(&name);
            let opts = badger_driver::RunOptions { profile, ..Default::default() };
            run_and_exit(&mut session, &path, args, &opts)
        }
        Command::Test { path, filter, json, list, exact, watch } => {
            if !path.exists() {
                // a test name is not a path: it goes after `--filter`
                eprintln!(
                    "error: no such file or directory: {} (a test's name goes after --filter)",
                    path.display()
                );
                return ExitCode::FAILURE;
            }
            if watch {
                // the tests run again on every change, as `badger watch`
                // runs its command
                let mut command = vec![exe_path()];
                command.push("test".into());
                if json {
                    command.push("--json".into());
                }
                if list {
                    command.push("--list".into());
                }
                if exact {
                    command.push("--exact".into());
                }
                if let Some(f) = &filter {
                    command.push("--filter".into());
                    command.push(f.clone());
                }
                command.push(path.display().to_string());
                return watch::watch(&path, command);
            }
            badger_driver::LISTING.store(list, std::sync::atomic::Ordering::Relaxed);
            let opts =
                badger_driver::RunOptions { tests: true, filter, json, list, exact, ..Default::default() };
            // every package of a workspace, else the one program
            for (i, pkg) in badger_driver::packages_of(&path).into_iter().enumerate() {
                let mut s = if i == 0 { std::mem::take(&mut session) } else { Session::new() };
                badger_driver::select_entry(None);
                if s.run(&pkg, vec![], &opts).is_err() {
                    test_failure_report(&s, json);
                    return ExitCode::FAILURE;
                }
                report(&s, false);
                // a project's other entry points: tests in modules they reach.
                // One session for all of them: the parsed and checked standard
                // library is reused, so only the entry itself is re-checked.
                for sel in badger_driver::project_entry_points(&pkg) {
                    badger_driver::select_entry(Some(sel));
                    s.reset_for_build();
                    if s.run(&pkg, vec![], &opts).is_err() {
                        test_failure_report(&s, json);
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
        Command::Graph { kind, path, format, json, std } => {
            // `--json` is the short way to ask for the JSON format
            let format = if json { "json" } else { format.as_str() };
            match graph::graph(&kind, &path, format, std) {
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
        Command::Doc { target, out, open, .. } if open => match doc::open(&target, out.as_deref()) {
            Ok(written) => {
                println!("wrote {written}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
        Command::Doc { target, out, check, .. } => match doc::run(&target, out.as_deref(), check) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
        Command::Rename { path, line, col, name, json } => {
            let edits = match lsp::query(
                lsp::RENAME,
                &path,
                Some(line),
                Some(col),
                Some(&name),
                std::collections::HashMap::new(),
            ) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::FAILURE;
                }
            };
            // `--json` reports the edits and writes nothing
            if json {
                println!("{edits}");
                return ExitCode::SUCCESS;
            }
            match apply_edits(&edits) {
                Ok(files) => {
                    if files.is_empty() {
                        eprintln!("error: nothing to rename");
                        return ExitCode::FAILURE;
                    }
                    for f in files {
                        println!("renamed in {f}");
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Bench { path, out, baseline, workers } => {
            match bench::run(&path, workers, out.as_deref(), baseline.as_deref()) {
                Ok(text) => {
                    print!("{text}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Init { path, template } => match init::init(&path, template.as_deref()) {
            Ok(files) => {
                println!("{}", init::report(&path, &files));
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
        Command::Generate {
            what,
            files,
            item,
            dry_run,
            check,
            database,
            schema,
        } if what != "sql" =>
        {
            // a flag of another generator is a usage error, not one that
            // is quietly ignored
            if check {
                return wrong_flag(&what, "check");
            }
            if database.is_some() {
                return wrong_flag(&what, "database");
            }
            if schema.is_some() {
                return wrong_flag(&what, "schema");
            }
            if item.is_some() && what != "feature" {
                return wrong_flag(&what, "item");
            }
            let Some(name) = files.first().map(|f| f.to_string_lossy().to_string()) else {
                eprintln!("error: `badger generate {what}` needs a name (`badger generate {what} orders`)");
                return ExitCode::from(2);
            };
            let dir = files.get(1).cloned().unwrap_or_else(|| PathBuf::from("."));
            let planned = match what.as_str() {
                "worker" => generate::plan_worker(&dir, &name),
                "service" => generate::plan_service(&dir, &name),
                "cli" => generate::plan_cli(&dir, &name),
                "library" => generate::plan_library(&dir, &name),
                _ => generate::plan(&dir, &name, item.as_deref()),
            };
            let plan = match planned {
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
            if what == "worker" {
                println!(
                    "\nnext: import {name} where the application is composed:\n\n    \
                     store: jobs.store({{ path: settings.jobs }}),\n    {name}: {name}.processor({{ store: ref(#store) }}),\n\n\
                     and call `{name}.submit(store, event, job)`; replace `Job` and `step` with the work."
                );
                return ExitCode::SUCCESS;
            }
            if what == "service" || what == "cli" || what == "library" {
                println!(
                    "\nnext: import {name} where it is used, and read the generated module's\n\
                     leading comment: it says what the module offers."
                );
                return ExitCode::SUCCESS;
            }
            println!(
                "\nnext: import {name}.memory, {name}.service, and {name}.http where the application is composed:\n\n    \
                 store: memory.store(),\n    {name}: service.{name}({{ store: ref(#store) }}),\n\n\
                 and serve `http.routes(running.{name})`; its tests run once an entry imports it (`badger test`)."
            );
            ExitCode::SUCCESS
        }
        Command::Generate {
            what,
            files,
            item,
            dry_run,
            check,
            database,
            schema,
        } => {
            if dry_run {
                return wrong_flag(&what, "dry-run");
            }
            if item.is_some() {
                return wrong_flag(&what, "item");
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
        Command::Format { paths, check } => format(paths, check),
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
        Command::Repl { connect, load } => match connect {
            Some(addr) => remote_repl(&addr, load),
            None => repl(load),
        },
        Command::Inspect { what, json, connect: Some(addr), .. } => {
            if !INSPECT_REMOTE.contains(&what.as_str()) {
                eprintln!(
                    "error: `{what}` is not a snapshot of a running program (one of: {})",
                    INSPECT_REMOTE.join(", ")
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
        Command::Inspect { what, path, json, profile, .. } if what == "config" => {
            match Session::effective_config(&path, profile.as_deref()) {
                Ok(entries) => {
                    let src = |s: &badger_driver::manifest::Source| match s {
                        badger_driver::manifest::Source::Base => "config".to_string(),
                        badger_driver::manifest::Source::Profile(p) => format!("profile.{p}"),
                    };
                    // the settings' types are not known here: a value
                    // whose key names a credential, or a URL's password,
                    // is shown redacted (as a `config.Secret` would be)
                    let entries: Vec<_> = entries
                        .into_iter()
                        .map(|(k, v, s)| {
                            let v = redact_setting(&k, &v);
                            (k, v, s)
                        })
                        .collect();
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
        Command::Inspect { what, path, json, profile, .. } if what == "runtime" => {
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
        Command::Inspect { what, path, json, .. } => {
            if what != "plans" {
                eprintln!(
                    "error: `{what}` is not something `badger inspect` reads (one of: {}; with `--connect ADDR`: {})",
                    INSPECT_LOCAL.join(", "),
                    INSPECT_REMOTE.join(", ")
                );
                return ExitCode::from(2);
            }
            let Some(plans) = session.inspect_plans(&path) else {
                report(&session, json);
                return ExitCode::FAILURE;
            };
            if plans.is_empty() && !json {
                println!("(no `system` or `pipeline` plans in {})", path.display());
                return ExitCode::SUCCESS;
            }
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
                    return ExitCode::from(2);
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
                            println!(
                                "\nThe common diagnostics are grouped by what you were probably\ntrying to do in docs/common-errors.md."
                            );
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
                    ExitCode::from(2)
                }
            }
        }
        Command::Doctor { json } => {
            let report = doctor::report();
            if json {
                println!("{}", report.json());
            } else {
                print!("{}", report.text());
            }
            ExitCode::SUCCESS
        }
        Command::Completions { shell } => match completions::script(&shell) {
            Some(script) => {
                print!("{script}");
                ExitCode::SUCCESS
            }
            None => {
                eprintln!("error: no completion script for `{shell}`");
                ExitCode::FAILURE
            }
        },
        Command::Watch { path, command } => watch::watch(&path, command),
        Command::Search { query, path, json } => {
            let found = search::search(&query, &path);
            if json {
                println!("{}", search::json(&found));
            } else if found.is_empty() {
                eprintln!("no match for `{query}`");
            } else {
                print!("{}", search::text(&found));
            }
            ExitCode::SUCCESS
        }
    }
}

/// The `.bdg` files under `p` (recursively).
pub(crate) fn source_files(p: &Path, out: &mut Vec<PathBuf>) {
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
    // a file that cannot be read or does not parse is reported and left
    // alone; the others are still formatted
    let mut failed = 0;
    let mut sources = badger_base::SourceMap::default();
    let mut interner = badger_base::Interner::default();
    for f in files {
        let src = match std::fs::read_to_string(&f) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: cannot read {}: {e}", f.display());
                failed += 1;
                continue;
            }
        };
        let id = sources.add(f.display().to_string(), src.clone());
        let mut diags = vec![];
        if let Err(e) = badger_syntax::parse(id, &src, &mut interner, &mut diags) {
            let at = e.span();
            let (line, col) = sources.get(id).line_col(at.start as usize);
            // named on the command line: an error; found in a directory (a
            // test of a syntax error, say): skipped, with a warning
            if paths.contains(&f) {
                eprintln!("error: {}:{line}:{col}: does not parse ({e}); not formatted", f.display());
                failed += 1;
            } else {
                eprintln!("warning: {}:{line}:{col}: does not parse ({e}); skipped", f.display());
            }
            continue;
        }
        let out = badger_syntax::format::format(&src);
        if out != src {
            changed += 1;
            if check {
                println!("would reformat {}", f.display());
            } else if let Err(e) = std::fs::write(&f, out) {
                eprintln!("error: cannot write {}: {e}", f.display());
                failed += 1;
            }
        }
    }
    if failed > 0 || (check && changed > 0) { ExitCode::FAILURE } else { ExitCode::SUCCESS }
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
fn remote_repl(addr: &str, load: Option<PathBuf>) -> ExitCode {
    if load_file(addr, load).is_err() {
        return ExitCode::FAILURE;
    }
    if repl_loop(addr, "remote> ") == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

/// Load a file's definitions into the session: the file's text is
/// evaluated as one block, so every definition in it is in scope
/// afterwards.
fn load_file(addr: &str, load: Option<PathBuf>) -> Result<(), String> {
    let Some(path) = load else { return Ok(()) };
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    match badger_driver::inspect::eval(addr, &text) {
        Ok(reply) => {
            if !reply.trim().is_empty() {
                println!("{reply}");
            }
            Ok(())
        }
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
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

fn repl(load: Option<PathBuf>) -> ExitCode {
    let Some(setup) = prepare_session("badger-repl") else { return ExitCode::FAILURE };
    let (addr, cleanup) = (setup.addr.clone(), setup.dir.clone());
    std::thread::spawn(move || {
        wait_for_session(&addr);
        if let Err(e) = load_file(&addr, load) {
            eprintln!("error: {e}");
            let _ = std::fs::remove_dir_all(&cleanup);
            exit_now(1);
        }
        let code = repl_loop(&addr, "badger> ");
        let _ = std::fs::remove_dir_all(&cleanup);
        exit_now(code)
    });
    run_session(&setup)
}

/// This executable's path: the commands that run the tool again use it.
fn exe_path() -> String {
    std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|_| "badger".into())
}

/// The byte offset of a 1-based line and column (columns count
/// characters, as in `badger check --json`).
fn offset_of(text: &str, line: usize, col: usize) -> usize {
    let mut offset = 0;
    for (i, l) in text.split_inclusive('\n').enumerate() {
        if i + 1 == line {
            for (chars, (b, _)) in l.char_indices().enumerate() {
                if chars + 1 == col {
                    return offset + b;
                }
            }
            return offset + l.len();
        }
        offset += l.len();
    }
    offset
}

/// Apply the edits a rename computed: every edit names a file, a range
/// (1-based lines and columns), and the text that replaces it. The
/// edits of a file are applied from its end, so the ranges stay valid.
fn apply_edits(edits: &serde_json::Value) -> Result<Vec<String>, String> {
    let list = edits["edits"].as_array().cloned().unwrap_or_default();
    let mut by_file: std::collections::HashMap<String, Vec<&serde_json::Value>> = Default::default();
    for e in &list {
        let file = e["file"].as_str().unwrap_or_default().to_string();
        by_file.entry(file).or_default().push(e);
    }
    let mut changed = vec![];
    for (file, mut es) in by_file {
        let mut text = std::fs::read_to_string(&file).map_err(|e| format!("{file}: {e}"))?;
        es.sort_by_key(|e| {
            (-(e["line"].as_i64().unwrap_or(0)), -(e["col"].as_i64().unwrap_or(0)))
        });
        for e in es {
            let line = e["line"].as_i64().unwrap_or(1).max(1) as usize;
            let col = e["col"].as_i64().unwrap_or(1).max(1) as usize;
            let end_line = e["end_line"].as_i64().unwrap_or(line as i64).max(1) as usize;
            let end_col = e["end_col"].as_i64().unwrap_or(col as i64).max(1) as usize;
            let a = offset_of(&text, line, col);
            let b = offset_of(&text, end_line, end_col);
            if a > b || b > text.len() {
                return Err(format!("{file}: an edit is outside the file"));
            }
            let new = e["text"].as_str().unwrap_or_default();
            text.replace_range(a..b, new);
        }
        std::fs::write(&file, &text).map_err(|e| format!("{file}: {e}"))?;
        changed.push(file);
    }
    changed.sort();
    Ok(changed)
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

/// `badger test` when the program does not build: with `--json`, the
/// documented result object (no tests ran) carrying the diagnostics.
fn test_failure_report(session: &Session, json: bool) {
    if json && session.diagnostics.has_errors() {
        println!(
            "{{\"passed\":0,\"failed\":0,\"tests\":[],\"diagnostics\":{}}}",
            session.diagnostics.render_json(&session.sources)
        );
    } else {
        report(session, json);
    }
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

/// An argument meant as a path (not an entry name).
fn looks_like_path(a: &str) -> bool {
    a.contains('/') || a.ends_with(".bdg") || a.ends_with(".bdgmir")
}

/// A configured value as `badger inspect config` shows it: redacted when
/// its key names a credential (`password`, `secret`, `token`, `api_key`,
/// `dsn`, ...), with a URL's password masked otherwise. A reference to
/// the environment (`${NAME}`) is shown as written.
fn redact_setting(key: &str, value: &str) -> String {
    if value.starts_with("${") && value.ends_with('}') {
        return value.to_string();
    }
    let last = key.rsplit('.').next().unwrap_or(key).to_ascii_lowercase();
    let sensitive =
        ["password", "passwd", "secret", "token", "credential", "private_key", "api_key", "apikey", "dsn"];
    if sensitive.iter().any(|s| last.contains(s)) {
        return "[redacted]".to_string();
    }
    // scheme://user:password@host
    if let Some(at) = value.find('@')
        && let Some(start) = value.find("://")
        && start < at
        && let Some(colon) = value[start + 3..at].find(':')
    {
        let colon = start + 3 + colon;
        return format!("{}:[redacted]{}", &value[..colon], &value[at..]);
    }
    value.to_string()
}
