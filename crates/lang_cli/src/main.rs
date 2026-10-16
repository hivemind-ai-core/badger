use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

mod graph;
mod lsp;
mod mcp;
use lang_base::diagnostics::json_str;
use lang_driver::{Outcome, Session};

#[derive(Parser)]
#[command(version, about = "Compiler, JIT runner, and tools for the draft-0 language")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Parse and statically check a program without running it.
    Check {
        /// Entry file or project directory (with `lang.toml`; default `.`).
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Emit diagnostics as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Check a program and save it as binary MIR, which `lang run FILE`
    /// loads without parsing or checking (only code generation remains).
    Compile {
        /// Entry file or project directory (default: the current project).
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Output file (default: the entry's name with `.langmir`).
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
        /// Merge `[profile.NAME]` from `lang.toml` into the saved configuration.
        #[arg(long)]
        profile: Option<String>,
        /// Worker threads to save as the default (`lang run --workers` still overrides).
        #[arg(long, value_name = "N")]
        workers: Option<usize>,
    },
    /// Compile a program with the JIT and run it (a source program, or a
    /// binary MIR file written by `lang compile`).
    Run {
        /// Entry file, project directory (with `lang.toml`), or the name of
        /// an `[entries]` module of the project in the current directory
        /// (default: the current project's main entry).
        entry: Option<String>,
        /// Optimize generated code.
        #[arg(long)]
        release: bool,
        /// Print MIR before code generation.
        #[arg(long)]
        dump_mir: bool,
        /// Merge `[profile.NAME]` from `lang.toml` into the configuration.
        #[arg(long)]
        profile: Option<String>,
        /// Recompile on source changes and replace compatible functions in
        /// the running program (new calls use new code).
        #[arg(long)]
        watch: bool,
        /// Serve inspection snapshots (`lang inspect ... --connect ADDR`).
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
        /// generated code with a debugger: `gdb --args lang run --debug ...`.
        #[arg(long)]
        debug: bool,
        /// Write a reproduction bundle: the program's identity, the outcome
        /// of every external operation (HTTP, subprocesses, files, the
        /// environment, clocks, random seeds), its output, and how it ended
        /// (`lang replay FILE` runs it again offline). Runs on one worker.
        #[arg(long, value_name = "FILE")]
        record: Option<PathBuf>,
        /// Arguments passed to the program (after `--`).
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Run a program again from a reproduction bundle (`lang run --record`):
    /// every external operation is answered from the bundle, nothing is
    /// requested, run, or written, and an operation the bundle does not hold
    /// ends the replay (exit 3). Exits 0 when the recorded outcome (exit
    /// code, output, failure) is reproduced and 1 when it differs.
    Replay {
        bundle: PathBuf,
        /// Run a changed program against the recording: report differing
        /// operations instead of stopping, and exit with the program's code.
        #[arg(long)]
        compare: bool,
        /// Optimize generated code.
        #[arg(long)]
        release: bool,
    },
    /// Run the entry module's `test_*` functions: `lang test [path] [filter]`
    /// (path defaults to the current project).
    Test {
        /// An optional program path, then an optional name filter.
        #[arg(num_args = 0..=2)]
        args: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    /// Run a user task: a typed function declared in the project's
    /// `[tasks]` table (`name = "src/file.lang#function"`).
    Task {
        name: String,
        /// Project directory (default `.`).
        #[arg(long, default_value = ".")]
        project: PathBuf,
        #[arg(long)]
        profile: Option<String>,
        /// Arguments passed to the task (after `--`).
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Rewrite sources in the canonical layout.
    Format {
        /// Files or directories (recursively, `*.lang`).
        paths: Vec<PathBuf>,
        /// Report files that would change instead of rewriting them.
        #[arg(long)]
        check: bool,
    },
    /// Interactive session: definitions accumulate; expressions evaluate.
    Repl {
        /// Evaluate in a program started with `lang run --inspect ADDR`.
        #[arg(long, value_name = "ADDR")]
        connect: Option<String>,
    },
    /// Inspect program structure.
    Inspect {
        /// `plans` (system graphs) or `config` (effective project
        /// configuration) of PATH; or, with `--connect`, `tasks`, `channels`,
        /// `system`, `heap`, `code`, or `all` of a running program.
        what: String,
        path: Option<PathBuf>,
        #[arg(long)]
        json: bool,
        /// Profile for `config`.
        #[arg(long)]
        profile: Option<String>,
        /// Address of a program started with `lang run --inspect ADDR`.
        #[arg(long, value_name = "ADDR")]
        connect: Option<String>,
    },
    /// Resolve path dependencies and rewrite the project's `lang.lock`.
    Update {
        /// Project directory (with `lang.toml`).
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
        kind: String,
        /// Entry file or project directory (default `.`).
        path: Option<PathBuf>,
        #[arg(long, default_value = "dot")]
        format: String,
        /// Include standard-library nodes.
        #[arg(long)]
        std: bool,
    },
    /// Answer one question about a program as JSON (1-based positions):
    /// `diagnostics`, `symbols`, `api` (public interface; PATH may also be a
    /// standard module such as `std.list`), or at `--line`/`--col` `hover`,
    /// `definition`, `references`, `completion`.
    Query {
        kind: String,
        /// Source file (checked with its project, if any).
        path: Option<PathBuf>,
        #[arg(long)]
        line: Option<usize>,
        #[arg(long)]
        col: Option<usize>,
        /// Ask about this source text (checked on its own) instead of a file.
        #[arg(long)]
        source: Option<String>,
    },
    /// Explain a diagnostic code, or the type/effect of a definition.
    Explain {
        /// `diagnostic`, `type`, or `effect`.
        kind: String,
        /// Diagnostic code (e.g. E0301), or a definition name.
        target: String,
        /// Program to examine (for `type` and `effect`).
        path: Option<PathBuf>,
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
        let at = info.location().map(|l| format!(" at {}:{}", l.file(), l.line())).unwrap_or_default();
        let thread = std::thread::current();
        eprintln!(
            "internal error: {msg}{at} (thread {})\nthis is a bug in lang, not in the program: please report it",
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
    // LANG_PROFILE covers compilation too, not only the program run
    lang_driver::start_profile();
    // `std.compiler` runs this executable for its queries
    if std::env::var_os("LANG_EXE").is_none()
        && let Ok(exe) = std::env::current_exe()
    {
        // SAFETY: no other threads exist yet
        unsafe { std::env::set_var("LANG_EXE", exe) };
    }
    let mut cli = Cli::parse();
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
                Some(e) => {
                    // a named `[entries]` module of the current project
                    lang_driver::select_entry(Some(e));
                    PathBuf::from(".")
                }
            };
            let opts = lang_driver::RunOptions {
                optimize: release,
                dump_mir,
                tests: false,
                filter: None,
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
            };
            match session.run(&path, args, &opts) {
                Ok(code) => {
                    report(&session, false);
                    exit_now(code)
                }
                Err(_) => {
                    report(&session, false);
                    ExitCode::FAILURE
                }
            }
        }
        Command::Replay { bundle, compare, release } => {
            let b = match lang_driver::load_bundle(&bundle) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::FAILURE;
                }
            };
            let program = &b["program"];
            if let Some(name) = program["selected"].as_str() {
                match name.strip_prefix("task:") {
                    Some(t) => lang_driver::select_task(t),
                    None => lang_driver::select_entry(Some(name.to_string())),
                }
            }
            let path = PathBuf::from(program["entry"].as_str().unwrap_or("."));
            let args: Vec<String> = program["args"]
                .as_array()
                .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            let opts = lang_driver::RunOptions {
                optimize: release,
                profile: program["profile"].as_str().map(str::to_string),
                replay: Some((bundle, compare)),
                ..Default::default()
            };
            match session.run(&path, args, &opts) {
                Ok(code) => {
                    report(&session, false);
                    exit_now(code)
                }
                Err(_) => {
                    report(&session, false);
                    ExitCode::FAILURE
                }
            }
        }
        Command::Compile { path, output, profile, workers } => {
            // saved programs get the MIR optimizations of `run --release`
            // (code generation then optimizes as `lang run` is told)
            let opts = lang_driver::RunOptions { profile, workers, optimize: true, ..Default::default() };
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
        Command::Task { name, project, profile, args } => {
            lang_driver::select_task(&name);
            let opts = lang_driver::RunOptions { profile, ..Default::default() };
            match session.run(&project, args, &opts) {
                Ok(code) => {
                    report(&session, false);
                    exit_now(code)
                }
                Err(_) => {
                    report(&session, false);
                    ExitCode::FAILURE
                }
            }
        }
        Command::Test { args, json } => {
            // `[path] [filter]`: a first argument naming an existing file or
            // directory is the program; anything else is the filter
            let mut args = args.into_iter();
            let (path, filter) = match args.next() {
                Some(a) if Path::new(&a).exists() => (PathBuf::from(a), args.next()),
                Some(a) => (PathBuf::from("."), Some(a)),
                None => (PathBuf::from("."), None),
            };
            let opts = lang_driver::RunOptions { tests: true, filter, json, ..Default::default() };
            if session.run(&path, vec![], &opts).is_err() {
                report(&session, json);
                return ExitCode::FAILURE;
            }
            report(&session, false);
            // a project's other entry points: tests in modules they reach
            for sel in lang_driver::project_entry_points(&path) {
                lang_driver::select_entry(Some(sel));
                let mut s = Session::new();
                if s.run(&path, vec![], &opts).is_err() {
                    report(&s, json);
                    return ExitCode::FAILURE;
                }
            }
            exit_now(lang_driver::finish_tests(json))
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
                    print!("{g}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Query { kind, path, line, col, source } => match source
            .map(|src| {
                let dir = lang_driver::private_temp_dir("lang-check").map_err(|e| e.to_string())?;
                let f = dir.join("source.lang");
                std::fs::write(&f, src).map_err(|e| e.to_string())?;
                Ok(f)
            })
            .unwrap_or_else(|| path.ok_or_else(|| "give a PATH or --source".to_string()))
            .and_then(|p| lsp::query(&kind, &p, line, col))
        {
            Ok(v) => {
                println!("{v}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
        Command::Format { paths, check } => {
            format(if paths.is_empty() { vec![PathBuf::from(".")] } else { paths }, check)
        }
        Command::Update { path } => match lang_driver::deps::update(&path) {
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
            match lang_driver::inspect::request(&addr, &what) {
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
                    let src = |s: &lang_driver::manifest::Source| match s {
                        lang_driver::manifest::Source::Base => "config".to_string(),
                        lang_driver::manifest::Source::Profile(p) => format!("profile.{p}"),
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
                    "error: `lang inspect {what}` reads a running program: start it with `lang run --inspect ADDR` and pass `--connect ADDR`"
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
                                n.refs.iter().map(|r| format!(":{r}")).collect::<Vec<_>>().join(", ")
                            )
                        };
                        println!("  :{} = {}{refs}", n.key, n.provider);
                    }
                    let order: Vec<String> = p.startup_order().iter().map(|k| format!(":{k}")).collect();
                    println!("  startup order: {}", order.join(" -> "));
                }
            }
            ExitCode::SUCCESS
        }
        Command::Explain { kind, target, path, json } => match kind.as_str() {
            "diagnostic" => match lang_base::catalog::explain(&target) {
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
        },
    }
}

fn lang_files(p: &Path, out: &mut Vec<PathBuf>) {
    if p.is_dir() {
        let mut entries: Vec<PathBuf> =
            std::fs::read_dir(p).into_iter().flatten().flatten().map(|e| e.path()).collect();
        entries.sort();
        for e in entries {
            lang_files(&e, out);
        }
    } else if p.extension().is_some_and(|x| x == "lang") {
        out.push(p.to_path_buf());
    }
}

fn format(paths: Vec<PathBuf>, check: bool) -> ExitCode {
    let mut files = vec![];
    for p in &paths {
        lang_files(p, &mut files);
    }
    let mut changed = 0;
    for f in files {
        let Ok(src) = std::fs::read_to_string(&f) else {
            eprintln!("error: cannot read {}", f.display());
            return ExitCode::FAILURE;
        };
        let out = lang_syntax::format::format(&src);
        if out != src {
            changed += 1;
            if check {
                println!("would reformat {}", f.display());
            } else if std::fs::write(&f, out).is_err() {
                eprintln!("error: cannot write {}", f.display());
                return ExitCode::FAILURE;
            }
        }
    }
    if check && changed > 0 { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}

/// Interactive session. Definitions (and imports) accumulate, a later
/// definition replaces an earlier one with the same name, and each
/// expression is checked and run against the current definitions.
/// Human-readable rendering of an inspection snapshot (JSON from the
/// running program's endpoint).
fn print_snapshot(what: &str, text: &str) {
    let field = |obj: &str, key: &str| -> String {
        // small flat-object reader: `"key":value`
        let pat = format!("\"{key}\":");
        match obj.find(&pat) {
            Some(i) => {
                let rest = &obj[i + pat.len()..];
                let end = if let Some(quoted) = rest.strip_prefix('"') {
                    quoted.find('"').map(|j| j + 2).unwrap_or(rest.len())
                } else if rest.starts_with('[') {
                    rest.find(']').map(|j| j + 1).unwrap_or(rest.len())
                } else {
                    rest.find([',', '}']).unwrap_or(rest.len())
                };
                rest[..end].trim_matches('"').to_string()
            }
            None => String::new(),
        }
    };
    let objects = |t: &str| -> Vec<String> {
        let inner = t.trim().trim_start_matches('[').trim_end_matches(']');
        let mut out = vec![];
        let (mut depth, mut start) = (0, None);
        for (i, c) in inner.char_indices() {
            match c {
                '{' => {
                    if depth == 0 {
                        start = Some(i);
                    }
                    depth += 1;
                }
                '}' => {
                    depth -= 1;
                    if depth == 0
                        && let Some(s) = start.take()
                    {
                        out.push(inner[s..=i].to_string());
                    }
                }
                _ => {}
            }
        }
        out
    };
    match what {
        "tasks" => {
            println!(
                "{:>5}  {:<9} {:<16} {:>6} {:>4} {:>8}  {:<18} name",
                "id", "state", "waiting on", "scope", "code", "stack", "stage"
            );
            for t in objects(text) {
                let wait = field(&t, "wait");
                let stage = field(&t, "stage");
                let stack = field(&t, "stack_resident").parse::<u64>().map(|b| format!("{}K", b / 1024));
                println!(
                    "{:>5}  {:<9} {:<16} {:>6} {:>4} {:>8}  {:<18} {}",
                    field(&t, "id"),
                    field(&t, "state"),
                    if wait == "null" { "-".to_string() } else { wait },
                    field(&t, "scope"),
                    format!("v{}", field(&t, "code_version")),
                    stack.unwrap_or_else(|_| "-".into()),
                    if stage == "null" { "-".to_string() } else { stage },
                    field(&t, "name")
                );
            }
        }
        "channels" => {
            println!(
                "{:>5}  {:<7} {:>8} {:>7} {:>7} {:>7}  waiting(s/r)  element",
                "id", "kind", "capacity", "queued", "staged", "closed"
            );
            for c in objects(text) {
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
            for s in objects(text) {
                println!("system {} ({})", field(&s, "id"), field(&s, "state"));
                let nodes = s.split_once("\"nodes\":").map(|x| x.1).unwrap_or("");
                for n in objects(nodes) {
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
            Some(what) => lang_driver::inspect::request(addr, what).map(|t| {
                print_snapshot(what, &t);
                String::new()
            }),
            None => lang_driver::inspect::eval(addr, trimmed),
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
/// same machinery as `lang repl --connect`). Definitions accumulate;
/// module-level `let` values are computed once and keep their identity;
/// expressions evaluate inside the session.
fn repl() -> ExitCode {
    let dir = match lang_driver::private_temp_dir("lang-repl") {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: cannot create a session directory: {e}");
            return ExitCode::FAILURE;
        }
    };
    let file = dir.join("session.lang");
    let session = "// REPL session program\nimport std.io\nimport std.task\n\npub fn main() -> Unit {\n    loop {\n        task.sleep(3600s)\n    }\n}\n";
    if std::fs::write(&file, session).is_err() {
        eprintln!("error: cannot write the session file");
        return ExitCode::FAILURE;
    }
    let port = match std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()) {
        Ok(a) => a.port(),
        Err(e) => {
            eprintln!("error: cannot reserve a local port: {e}");
            return ExitCode::FAILURE;
        }
    };
    let addr = format!("127.0.0.1:{port}");
    let client_addr = addr.clone();
    let cleanup = dir.clone();
    std::thread::spawn(move || {
        let start = std::time::Instant::now();
        while std::net::TcpStream::connect(&client_addr).is_err() {
            if start.elapsed() > std::time::Duration::from_secs(30) {
                eprintln!("error: the REPL session did not start");
                exit_now(1);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let code = repl_loop(&client_addr, "lang> ");
        let _ = std::fs::remove_dir_all(&cleanup);
        exit_now(code)
    });
    // session bindings are interactive state, not a program's module values
    // (which must be pure immutable data)
    lang_driver::set_repl_session();
    let mut s = Session::new();
    let opts = lang_driver::RunOptions { inspect: Some(addr), ..Default::default() };
    match s.run(&file, vec![], &opts) {
        Ok(code) => exit_now(code),
        Err(_) => {
            report(&s, false);
            ExitCode::FAILURE
        }
    }
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

/// Leave the process after a program ran: the runtime's task stacks, JIT
/// image, and heap live for the whole process, so skip their destructors.
pub(crate) fn exit_now(code: i32) -> ExitCode {
    lang_driver::finish();
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    // Abandoned task stacks may still be mid-call; process teardown hooks
    // must not run on their behalf.
    unsafe { libc::_exit(code) }
}

pub(crate) fn report(session: &Session, json: bool) {
    if json {
        println!("{}", session.diagnostics.render_json(&session.sources));
    } else {
        let color = std::io::stderr().is_terminal();
        let _ = session.diagnostics.render_with(&session.sources, &mut std::io::stderr(), color);
    }
}

/// `prog.lang` -> `prog.langmir`; a project directory -> `<package>.langmir`
/// in it (the directory's name when there is no manifest name).
fn default_binary_name(path: &Path) -> PathBuf {
    if path.is_dir() {
        let name = std::fs::read_to_string(path.join("lang.toml"))
            .ok()
            .and_then(|t| {
                t.lines().find_map(|l| {
                    let l = l.trim();
                    l.strip_prefix("name")
                        .map(|r| r.trim_start().trim_start_matches('=').trim().trim_matches('"').to_string())
                })
            })
            .filter(|n| !n.is_empty())
            .or_else(|| path.canonicalize().ok()?.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "program".into());
        return path.join(format!("{name}.langmir"));
    }
    path.with_extension("langmir")
}
