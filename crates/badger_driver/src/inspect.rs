//! Inspection endpoint of a running program (`badger run --inspect ADDR`) and
//! its clients (`badger inspect WHAT --connect ADDR`, `badger repl --connect`).
//! The protocol is one request line per connection and one reply:
//!
//! - a snapshot name (`tasks`, `channels`, `system`, `heap`, `code`, `all`)
//!   answers with its JSON;
//! - `eval <json string>` evaluates REPL input inside the running program
//!   (`evalt MILLIS <json string>` cancels it after a time limit).
//!
//! Every request line starts with the endpoint's token: a random secret
//! made when it starts, kept in memory for in-process clients (the local
//! REPL, `badger mcp`) and written to a file only the user can read
//! (`inspect_token_path`) for other processes of the same user.
//!   Definitions join the program as a new code version (accepted only when
//!   compatible, exactly like `--watch`); an expression runs as a task of
//!   the new version and the reply is its displayed value. Module-level
//!   values are the running program's own.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Mutex, mpsc};
use std::time::Duration;

use crate::{RunOptions, Session};

struct Repl {
    entry: PathBuf,
    optimize: bool,
    /// Accepted definitions: (name, source).
    items: Vec<(String, String)>,
    imports: Vec<String>,
    counter: u32,
    /// Type and effect of each accepted definition, by name.
    sigs: Vec<(String, (String, String))>,
}

static REPL: Mutex<Option<Repl>> = Mutex::new(None);

/// Source of the definitions made through connected REPLs.
pub fn repl_definitions() -> String {
    let g = REPL.lock().unwrap();
    match &*g {
        Some(r) => {
            let mut s = r.imports.join("\n");
            for (_, src) in &r.items {
                s.push('\n');
                s.push_str(src);
            }
            s
        }
        None => String::new(),
    }
}

enum Eval {
    Done(String),
    /// Rejected (diagnostics, incompatible change) without running.
    Failed(String),
    Running(badger_runtime::task::TaskId),
}

/// The key a session item is replaced by: its name, or for an `impl` its
/// header (`impl Display for Point`). Contract clauses and doc comments
/// before a function are skipped.
fn item_name(src: &str) -> Option<String> {
    let body: Vec<&str> = src
        .lines()
        .skip_while(|l| {
            let t = l.trim_start();
            t.starts_with("requires ") || t.starts_with("ensures ") || t.starts_with("//") || t.is_empty()
        })
        .collect();
    let body = body.join("\n");
    let t = body.trim_start();
    let t = t.strip_prefix("pub ").unwrap_or(t);
    if t.starts_with("impl ") || t.starts_with("impl<") {
        let header = t.split('{').next()?;
        return Some(header.split_whitespace().collect::<Vec<_>>().join(" "));
    }
    let src = body.as_str();
    let words: Vec<&str> =
        src.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|w| !w.is_empty()).collect();
    let kw = ["pub", "fn", "type", "struct", "enum", "protocol", "let", "const", "extern", "c", "impl"];
    words.into_iter().find(|w| !kw.contains(w)).map(str::to_string)
}

/// Compile the program plus `input` as a new version (scheduler thread).
fn eval_start(input: &str) -> Eval {
    let mut g = REPL.lock().unwrap();
    let Some(repl) = g.as_mut() else { return Eval::Failed("error: no program".into()) };
    let trimmed = input.trim();
    let is_import = trimmed.starts_with("import ");
    let is_item = [
        "fn ",
        "pub ",
        "type ",
        "struct ",
        "enum ",
        "protocol ",
        "impl",
        "extern ",
        "const ",
        "requires ",
        "ensures ",
        "///",
    ]
    .iter()
    .any(|k| trimmed.starts_with(k))
        || (trimmed.starts_with("let ") && !trimmed.contains('\n'));
    let mut items = repl.items.clone();
    let mut imports = repl.imports.clone();
    repl.counter += 1;
    let eval_fn = format!("test_repl_eval_{}", repl.counter);
    let mut extra = String::new();
    if is_import {
        if !imports.iter().any(|i| i == trimmed) {
            imports.push(trimmed.to_string());
        }
    } else if is_item {
        let name = item_name(trimmed).unwrap_or_default();
        items.retain(|(n, _)| *n != name);
        items.push((name, trimmed.to_string()));
    } else {
        // a leading statement keeps `{ name }` a block, not a record
        extra = format!(
            "\nimport std.inspect\nfn {eval_fn}() -> String {{\n    let value = {{\n    let _ = ()\n{trimmed}\n    }}\n    inspect.show(value)\n}}\n"
        );
    }
    let mut session = Session::new();
    let mut suffix = imports.join("\n");
    for (_, src) in &items {
        suffix.push('\n');
        suffix.push_str(src);
    }
    suffix.push_str(&extra);
    session.entry_suffix = suffix;
    let opts = RunOptions {
        optimize: repl.optimize,
        watch: true,
        tests: true,
        workers: Some(badger_runtime::task::workers()),
        ..Default::default()
    };
    let built = match session.build(&repl.entry, &opts) {
        Ok(b) => b,
        Err(_) => {
            let mut out = Vec::new();
            let _ = session.diagnostics.render_with(&session.sources, &mut out, false);
            return Eval::Failed(String::from_utf8_lossy(&out).trim_end().to_string());
        }
    };
    let entries = crate::live::entries(&built);
    if let Some(why) = badger_runtime::live::incompatibility(&built.types, &entries) {
        return Eval::Failed(format!("rejected: {why}; the running version continues"));
    }
    // a redefinition keeps its type and effects (compatible replacement)
    let mut new_sig = None;
    if is_item && !is_import {
        let name = item_name(trimmed).unwrap_or_default();
        let mut probe = Session::new();
        probe.entry_suffix = session.entry_suffix.clone();
        if let Some(sig) = probe.explain_def(&repl.entry, &name) {
            // the previous definition: an earlier REPL one, or the program's
            let previous = match repl.sigs.iter().find(|(n, _)| *n == name) {
                Some((_, old)) => Some(old.clone()),
                None => {
                    let mut base = Session::new();
                    let mut prev = repl.imports.join("\n");
                    for (_, src) in &repl.items {
                        prev.push('\n');
                        prev.push_str(src);
                    }
                    base.entry_suffix = prev;
                    base.explain_def(&repl.entry, &name)
                }
            };
            if let Some(old) = previous
                && old != sig
            {
                let (a, b) = if old.0 != sig.0 { (&old.0, &sig.0) } else { (&old.1, &sig.1) };
                return Eval::Failed(format!(
                    "rejected: `{name}` changed from `{a}` to `{b}`; the running version continues"
                ));
            }
            new_sig = Some((name, sig));
        }
    }
    badger_runtime::live::publish(&built.types, &entries);
    repl.items = items;
    repl.imports = imports;
    if let Some((name, sig)) = new_sig {
        repl.sigs.retain(|(n, _)| *n != name);
        repl.sigs.push((name, sig));
    }
    let task = if extra.is_empty() {
        None
    } else {
        built.prog.tests.iter().find(|(n, _)| *n == eval_fn).and_then(|(_, f)| {
            built
                .prog
                .static_closures
                .iter()
                .find(|(_, g)| g == f)
                .map(|(obj, _)| badger_runtime::task::spawn(*obj, None, "repl".into()))
        })
    };
    // this version's code stays mapped for the life of the process
    std::mem::forget(built);
    match task {
        Some(id) => Eval::Running(id),
        None => Eval::Done(String::new()),
    }
}

/// Result of an evaluation task, once it has finished (scheduler thread).
/// The evaluation's reply and whether it failed.
fn eval_poll(id: badger_runtime::task::TaskId) -> Option<(String, bool)> {
    use badger_runtime::task::Exit;
    if !badger_runtime::task::is_done(id) {
        return None;
    }
    let exit = badger_runtime::task::exit_of(id);
    badger_runtime::task::remove_task(id);
    Some(match exit {
        Some(Exit::Returned(w)) => {
            let s = badger_runtime::string::str_of(w).to_string();
            (if s == "()" { String::new() } else { s }, false)
        }
        Some(Exit::Failed(p)) => {
            (format!("panic: {} {}", p.message, p.location).trim_end().to_string(), true)
        }
        _ => ("cancelled".into(), true),
    })
}

fn on_scheduler<T: Send + 'static>(
    q: &std::sync::Arc<badger_runtime::task::ExternalQueue>,
    f: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    let (tx, rx) = mpsc::channel::<T>();
    badger_runtime::task::post(
        q,
        badger_runtime::task::External::Scheduler(Box::new(move || {
            let _ = tx.send(f());
        })),
    );
    rx.recv_timeout(Duration::from_secs(60)).ok()
}

fn json_unquote(s: &str) -> String {
    let s = s.trim();
    let s = s.strip_prefix('"').and_then(|x| x.strip_suffix('"')).unwrap_or(s);
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some(o) => out.push(o),
            None => {}
        }
    }
    out
}

/// Evaluate `input`: the reply and whether the evaluation failed.
fn evaluate(
    q: &std::sync::Arc<badger_runtime::task::ExternalQueue>,
    input: String,
    limit: Duration,
) -> (String, bool) {
    let started = on_scheduler(q, move || match eval_start(&input) {
        Eval::Done(s) => Ok((s, false)),
        Eval::Failed(s) => Ok((s, true)),
        Eval::Running(id) => Err(id),
    });
    match started {
        None => ("error: the program did not respond".into(), true),
        Some(Ok(r)) => r,
        Some(Err(id)) => {
            let start = std::time::Instant::now();
            loop {
                if let Some(Some(r)) = on_scheduler(q, move || eval_poll(id)) {
                    return r;
                }
                if start.elapsed() > limit {
                    // stop it, so the session stays usable
                    on_scheduler(q, move || {
                        badger_runtime::task::cancel(id);
                    });
                    return (
                        format!(
                            "error: evaluation timed out after {}s and was cancelled",
                            limit.as_secs_f64()
                        ),
                        true,
                    );
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

static TOKENS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// Where the token of the endpoint at `addr` is kept for the user's other
/// processes: a private (0700) per-user directory.
pub fn inspect_token_path(addr: &str) -> Option<PathBuf> {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    let dir = base.join("badger-inspect");
    let name: String = addr.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    Some(dir.join(format!("{name}.token")))
}

/// The endpoint's secret: without one it does not serve (it can evaluate
/// code), rather than serve under a guessable one.
fn random_token() -> std::io::Result<String> {
    crate::random_hex(16)
}

fn publish_token(addr: &str, token: &str) {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
    let Some(path) = inspect_token_path(addr) else { return };
    let dir = path.parent().unwrap();
    let _ = std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir);
    // only a directory of our own, not readable by others
    let Ok(meta) = std::fs::symlink_metadata(dir) else { return };
    if !meta.is_dir() || meta.permissions().mode() & 0o077 != 0 {
        return;
    }
    let _ = std::fs::remove_file(&path);
    if let Ok(mut f) = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path) {
        let _ = f.write_all(token.as_bytes());
    }
}

/// Remove the token files this process published (when its program ends),
/// unless another process has since published its own for the address.
pub fn withdraw_tokens() {
    for (addr, token) in TOKENS.lock().unwrap().iter() {
        if let Some(path) = inspect_token_path(addr)
            && std::fs::read_to_string(&path).is_ok_and(|t| t == *token)
        {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// The token for `addr`: this process's own endpoint, or the user's file.
fn token_for(addr: &str) -> String {
    if let Some((_, t)) = TOKENS.lock().unwrap().iter().find(|(a, _)| a == addr) {
        return t.clone();
    }
    inspect_token_path(addr)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Serve snapshots; each is computed on the scheduler thread between task
/// steps (running programs answer at their next poll).
pub fn serve(addr: &str, entry: PathBuf, optimize: bool) -> Result<(), String> {
    let listener = TcpListener::bind(addr).map_err(|e| format!("cannot listen on {addr}: {e}"))?;
    let token = random_token().map_err(|e| format!("cannot make an access token: {e}"))?;
    TOKENS.lock().unwrap().push((addr.to_string(), token.clone()));
    publish_token(addr, &token);
    let q = badger_runtime::task::external_queue();
    *REPL.lock().unwrap() =
        Some(Repl { entry, optimize, items: vec![], imports: vec![], counter: 0, sigs: vec![] });
    // evaluations change the program: one at a time
    let evaluating = std::sync::Arc::new(Mutex::new(()));
    std::thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            let (q, token, evaluating) = (q.clone(), token.clone(), evaluating.clone());
            std::thread::spawn(move || serve_one(conn, &q, &token, &evaluating));
        }
    });
    Ok(())
}

fn serve_one(
    conn: TcpStream,
    q: &std::sync::Arc<badger_runtime::task::ExternalQueue>,
    token: &str,
    evaluating: &Mutex<()>,
) {
    {
        conn.set_read_timeout(Some(Duration::from_secs(10))).ok();
        let mut line = String::new();
        let mut reader = BufReader::new(match conn.try_clone() {
            Ok(c) => c,
            Err(_) => return,
        });
        if reader.read_line(&mut line).is_err() {
            return;
        }
        let line = line.trim();
        let Some(what) = line.strip_prefix(token).and_then(|r| r.strip_prefix(' ')) else {
            let mut conn = conn;
            let _ = conn.write_all(b"error: unauthorized (the endpoint's token is missing or wrong)\n");
            return;
        };
        let what = what.to_string();
        // `eval INPUT`, or `evalt MILLIS INPUT` with a time limit
        let timed = what.strip_prefix("evalt ").and_then(|r| {
            let (ms, input) = r.split_once(' ')?;
            Some((Duration::from_millis(ms.parse().ok()?), input))
        });
        let untimed = what.strip_prefix("eval ").map(|i| (Duration::from_secs(60), i));
        let is_timed = timed.is_some();
        if let Some((limit, input)) = timed.or(untimed) {
            let _one = evaluating.lock().unwrap_or_else(|e| e.into_inner());
            let (reply, failed) = evaluate(q, json_unquote(input), limit);
            let mut conn = conn;
            // `evalt` replies start with a status line
            if is_timed {
                let _ = conn.write_all(if failed { b"failed\n" } else { b"ok\n" });
            }
            let _ = conn.write_all(reply.as_bytes());
            return;
        }
        let (tx, rx) = mpsc::channel::<String>();
        badger_runtime::task::post(
            q,
            badger_runtime::task::External::Callback(Box::new(move || {
                let reply = badger_runtime::inspect::snapshot(&what)
                    .unwrap_or_else(|| format!("{{\"error\":\"unknown snapshot `{what}`\"}}"));
                let _ = tx.send(reply);
            })),
        );
        let reply =
            rx.recv_timeout(Duration::from_secs(10)).unwrap_or_else(|_| "{\"error\":\"no response\"}".into());
        let mut conn = conn;
        let _ = conn.write_all(reply.as_bytes());
        let _ = conn.write_all(b"\n");
    }
}

/// Request one snapshot from a running program.
pub fn request(addr: &str, what: &str) -> Result<String, String> {
    let mut s = TcpStream::connect(addr).map_err(|e| format!("cannot connect to {addr}: {e}"))?;
    s.set_read_timeout(Some(Duration::from_secs(15))).ok();
    s.write_all(format!("{} {what}\n", token_for(addr)).as_bytes()).map_err(|e| e.to_string())?;
    let mut out = String::new();
    s.read_to_string(&mut out).map_err(|e| e.to_string())?;
    Ok(out.trim().to_string())
}

/// Evaluate REPL input in a running program.
pub fn eval(addr: &str, input: &str) -> Result<String, String> {
    eval_within(addr, input, None)
}

/// Evaluate REPL input; with a `limit`, the program cancels an evaluation
/// still running after it (and the reply starts with an `ok` or `failed`
/// status line).
pub fn eval_within(addr: &str, input: &str, limit: Option<Duration>) -> Result<String, String> {
    let quoted = badger_runtime::intrinsics_io::json_quote(input);
    let mut s = TcpStream::connect(addr).map_err(|e| format!("cannot connect to {addr}: {e}"))?;
    let token = token_for(addr);
    if let Some(l) = limit {
        // the program cancels at the limit; allow it time to answer
        s.set_read_timeout(Some(l + Duration::from_secs(30))).ok();
        s.write_all(format!("{token} evalt {} {quoted}\n", l.as_millis()).as_bytes())
            .map_err(|e| e.to_string())?;
        let mut out = String::new();
        s.read_to_string(&mut out).map_err(|e| e.to_string())?;
        return Ok(out);
    }
    s.write_all(format!("{token} eval {quoted}\n").as_bytes()).map_err(|e| e.to_string())?;
    let mut out = String::new();
    s.read_to_string(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}
