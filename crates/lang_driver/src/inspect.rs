//! Inspection endpoint of a running program (`lang run --inspect ADDR`) and
//! its clients (`lang inspect WHAT --connect ADDR`, `lang repl --connect`).
//! The protocol is one request line per connection and one reply:
//!
//! - a snapshot name (`tasks`, `channels`, `system`, `heap`, `code`, `all`)
//!   answers with its JSON;
//! - `eval <json string>` evaluates REPL input inside the running program.
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
    Running(lang_runtime::task::TaskId),
}

fn item_name(src: &str) -> Option<String> {
    let words: Vec<&str> =
        src.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|w| !w.is_empty()).collect();
    let kw = ["pub", "fn", "type", "struct", "enum", "protocol", "let", "const", "extern", "c", "impl"];
    words.into_iter().find(|w| !kw.contains(w)).map(str::to_string)
}

/// Compile the program plus `input` as a new version (scheduler thread).
fn eval_start(input: &str) -> Eval {
    let mut g = REPL.lock().unwrap();
    let Some(repl) = g.as_mut() else { return Eval::Done("error: no program".into()) };
    let trimmed = input.trim();
    let is_import = trimmed.starts_with("import ");
    let is_item = ["fn ", "pub ", "type ", "struct ", "enum ", "protocol ", "impl", "extern ", "const "]
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
        workers: Some(lang_runtime::task::workers()),
        ..Default::default()
    };
    let built = match session.build(&repl.entry, &opts) {
        Ok(b) => b,
        Err(_) => {
            let mut out = Vec::new();
            let _ = session.diagnostics.render_with(&session.sources, &mut out, false);
            return Eval::Done(String::from_utf8_lossy(&out).trim_end().to_string());
        }
    };
    let entries = crate::live::entries(&built);
    if let Some(why) = lang_runtime::live::incompatibility(&built.types, &entries) {
        return Eval::Done(format!("rejected: {why}; the running version continues"));
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
                return Eval::Done(format!(
                    "rejected: `{name}` changed from `{a}` to `{b}`; the running version continues"
                ));
            }
            new_sig = Some((name, sig));
        }
    }
    lang_runtime::live::publish(&built.types, &entries);
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
                .map(|(obj, _)| lang_runtime::task::spawn(*obj, None, "repl".into()))
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
fn eval_poll(id: lang_runtime::task::TaskId) -> Option<String> {
    use lang_runtime::task::Exit;
    if !lang_runtime::task::is_done(id) {
        return None;
    }
    let exit = lang_runtime::task::exit_of(id);
    lang_runtime::task::remove_task(id);
    Some(match exit {
        Some(Exit::Returned(w)) => {
            let s = lang_runtime::string::str_of(w).to_string();
            if s == "()" { String::new() } else { s }
        }
        Some(Exit::Failed(p)) => format!("panic: {} {}", p.message, p.location).trim_end().to_string(),
        _ => "cancelled".into(),
    })
}

fn on_scheduler<T: Send + 'static>(
    q: &std::sync::Arc<lang_runtime::task::ExternalQueue>,
    f: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    let (tx, rx) = mpsc::channel::<T>();
    lang_runtime::task::post(
        q,
        lang_runtime::task::External::Scheduler(Box::new(move || {
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

fn evaluate(q: &std::sync::Arc<lang_runtime::task::ExternalQueue>, input: String) -> String {
    let started = on_scheduler(q, move || match eval_start(&input) {
        Eval::Done(s) => Ok(s),
        Eval::Running(id) => Err(id),
    });
    match started {
        None => "error: the program did not respond".into(),
        Some(Ok(s)) => s,
        Some(Err(id)) => {
            let start = std::time::Instant::now();
            loop {
                if let Some(Some(r)) = on_scheduler(q, move || eval_poll(id)) {
                    return r;
                }
                if start.elapsed() > Duration::from_secs(60) {
                    return "error: evaluation timed out".into();
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

/// Serve snapshots; each is computed on the scheduler thread between task
/// steps (running programs answer at their next poll).
pub fn serve(addr: &str, entry: PathBuf, optimize: bool) -> Result<(), String> {
    let listener = TcpListener::bind(addr).map_err(|e| format!("cannot listen on {addr}: {e}"))?;
    let q = lang_runtime::task::external_queue();
    *REPL.lock().unwrap() =
        Some(Repl { entry, optimize, items: vec![], imports: vec![], counter: 0, sigs: vec![] });
    std::thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            let mut line = String::new();
            let mut reader = BufReader::new(match conn.try_clone() {
                Ok(c) => c,
                Err(_) => continue,
            });
            if reader.read_line(&mut line).is_err() {
                continue;
            }
            let what = line.trim().to_string();
            if let Some(input) = what.strip_prefix("eval ") {
                let reply = evaluate(&q, json_unquote(input));
                let mut conn = conn;
                let _ = conn.write_all(reply.as_bytes());
                continue;
            }
            let (tx, rx) = mpsc::channel::<String>();
            lang_runtime::task::post(
                &q,
                lang_runtime::task::External::Callback(Box::new(move || {
                    let reply = lang_runtime::inspect::snapshot(&what)
                        .unwrap_or_else(|| format!("{{\"error\":\"unknown snapshot `{what}`\"}}"));
                    let _ = tx.send(reply);
                })),
            );
            let reply = rx
                .recv_timeout(Duration::from_secs(10))
                .unwrap_or_else(|_| "{\"error\":\"no response\"}".into());
            let mut conn = conn;
            let _ = conn.write_all(reply.as_bytes());
            let _ = conn.write_all(b"\n");
        }
    });
    Ok(())
}

/// Request one snapshot from a running program.
pub fn request(addr: &str, what: &str) -> Result<String, String> {
    let mut s = TcpStream::connect(addr).map_err(|e| format!("cannot connect to {addr}: {e}"))?;
    s.set_read_timeout(Some(Duration::from_secs(15))).ok();
    s.write_all(format!("{what}\n").as_bytes()).map_err(|e| e.to_string())?;
    let mut out = String::new();
    s.read_to_string(&mut out).map_err(|e| e.to_string())?;
    Ok(out.trim().to_string())
}

/// Evaluate REPL input in a running program.
pub fn eval(addr: &str, input: &str) -> Result<String, String> {
    let quoted = lang_runtime::intrinsics_io::json_quote(input);
    let mut s = TcpStream::connect(addr).map_err(|e| format!("cannot connect to {addr}: {e}"))?;
    s.write_all(format!("eval {quoted}\n").as_bytes()).map_err(|e| e.to_string())?;
    let mut out = String::new();
    s.read_to_string(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}
