use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
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
    /// Compile a program with the JIT and run it.
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
        /// Arguments passed to the program (after `--`).
        #[arg(last = true)]
        args: Vec<String>,
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

fn main() -> ExitCode {
    let mut cli = Cli::parse();
    // a missing path means the current directory's project
    if let Command::Inspect { path, connect: None, .. } = &mut cli.command
        && path.is_none()
    {
        *path = Some(PathBuf::from("."));
    }
    let mut session = Session::new();
    match cli.command {
        Command::Run { entry, release, dump_mir, profile, watch, inspect, workers, reconstruct, args } => {
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
                "{:>5}  {:<9} {:<16} {:>6} {:>4}  {:<18} name",
                "id", "state", "waiting on", "scope", "code", "stage"
            );
            for t in objects(text) {
                let wait = field(&t, "wait");
                let stage = field(&t, "stage");
                println!(
                    "{:>5}  {:<9} {:<16} {:>6} {:>4}  {:<18} {}",
                    field(&t, "id"),
                    field(&t, "state"),
                    if wait == "null" { "-".to_string() } else { wait },
                    field(&t, "scope"),
                    format!("v{}", field(&t, "code_version")),
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
    let dir = std::env::temp_dir().join(format!("lang-repl-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
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
    unsafe { std::env::set_var("LANG_REPL_SESSION", &file) };
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
fn exit_now(code: i32) -> ExitCode {
    lang_driver::finish();
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    // Abandoned task stacks may still be mid-call; process teardown hooks
    // must not run on their behalf.
    unsafe { libc::_exit(code) }
}

fn report(session: &Session, json: bool) {
    if json {
        println!("{}", session.diagnostics.render_json(&session.sources));
    } else {
        let color = std::io::stderr().is_terminal();
        let _ = session.diagnostics.render_with(&session.sources, &mut std::io::stderr(), color);
    }
}
