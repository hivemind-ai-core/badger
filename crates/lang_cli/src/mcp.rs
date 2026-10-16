//! `lang mcp`: a Model Context Protocol server on stdio (newline-delimited
//! JSON-RPC) giving an agent the compiler and a live REPL through four tools:
//!
//! - `eval` ("code mode"): a persistent REPL session (the local `lang repl`
//!   machinery); definitions accumulate and expressions evaluate, and
//!   `std.compiler` exposes the compiler's analysis to the code itself, so
//!   one call can check, query, and post-process.
//! - `check`: diagnostics of a file or project, or of a source text.
//! - `query`: hover, definition, references, symbols, completion,
//!   diagnostic explanations, and standard module sources.
//! - `run`: run a program or its tests in a subprocess, with a time limit.
//!
//! Compiler questions run in subprocesses (`lang query`), isolated from the
//! session. The session program's standard output is captured (stdout is
//! the protocol channel) and returned with each evaluation.

use std::io::{BufRead, Read, Write};
use std::os::fd::FromRawFd;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

const PROTOCOL: &str = "2025-06-18";

const INSTRUCTIONS: &str = "Tools for a statically typed language with records, effects, \
affine ownership, structured tasks, and channels. Write code in `eval` (a persistent REPL: \
definitions accumulate; an expression's value is shown). `import std.compiler` there to \
analyse files from code: compiler.diagnostics(path), compiler.check_source(src), \
compiler.hover(path, line, col), compiler.definition(...), compiler.references(...), \
compiler.symbols(path), compiler.completions(...), compiler.explain(code). Use `query` with \
kind `module` to read a standard module's API (e.g. `list`, `map`, `io`, `task`, `channel`), \
and `check` before `run`. Positions are 1-based.";

fn tools() -> Value {
    json!([
        {
            "name": "eval",
            "description": "Evaluate code in a persistent REPL session. Items (fn, struct, enum, import, module-level let) are added to the session and replace earlier ones of the same name; an expression is evaluated and its value displayed. Output printed by the code is returned too. `import std.compiler` gives compiler services as functions (diagnostics, check_source, hover, definition, references, symbols, completions, explain) returning typed records, for querying and post-processing analysis results in code.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "code": { "type": "string", "description": "Items or an expression." },
                    "timeout_secs": { "type": "number", "description": "Give up waiting after this long (default 30)." }
                },
                "required": ["code"]
            }
        },
        {
            "name": "check",
            "description": "Type-check a file or project directory (`path`) or a source text (`source`), returning diagnostics as JSON (file, line, col, severity, code, message, notes). An empty list means it checks.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "source": { "type": "string" }
                }
            }
        },
        {
            "name": "query",
            "description": "Ask about code. kind `hover` (type or signature), `definition`, `references`, `completion` need path, line, col (1-based); `symbols` and `diagnostics` need path; `explain` takes a diagnostic code in `name` (e.g. E0301); `module` returns a standard module's source (its API and documentation) for `name` (e.g. `list`), or the list of modules without a name.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "kind": { "type": "string", "enum": ["hover", "definition", "references", "completion", "symbols", "diagnostics", "explain", "module"] },
                    "path": { "type": "string" },
                    "line": { "type": "integer" },
                    "col": { "type": "integer" },
                    "name": { "type": "string" }
                },
                "required": ["kind"]
            }
        },
        {
            "name": "run",
            "description": "Run a program (file or project directory) or, with `test: true`, its `test_*` functions, in a fresh process. Returns the exit status, stdout, and stderr.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "args": { "type": "array", "items": { "type": "string" } },
                    "test": { "type": "boolean" },
                    "release": { "type": "boolean" },
                    "timeout_secs": { "type": "number", "description": "Kill the process after this long (default 60)." }
                },
                "required": ["path"]
            }
        }
    ])
}

fn exe() -> String {
    std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|_| "lang".into())
}

/// Run `lang ARGS` with a time limit: (status, stdout, stderr).
fn lang(args: &[String], limit: Duration) -> Result<(i32, String, String), String> {
    let mut child = std::process::Command::new(exe())
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot start lang: {e}"))?;
    let (mut out, mut err) = (child.stdout.take().unwrap(), child.stderr.take().unwrap());
    let ro = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = out.read_to_string(&mut s);
        s
    });
    let re = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = err.read_to_string(&mut s);
        s
    });
    let start = std::time::Instant::now();
    let status = loop {
        if let Some(st) = child.try_wait().map_err(|e| e.to_string())? {
            break st.code().unwrap_or(-1);
        }
        if start.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("timed out after {}s", limit.as_secs_f64()));
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    Ok((status, ro.join().unwrap_or_default(), re.join().unwrap_or_default()))
}

fn text(t: impl Into<String>, is_error: bool) -> Value {
    json!({ "content": [{ "type": "text", "text": t.into() }], "isError": is_error })
}

fn secs(v: &Value, default: f64) -> Duration {
    Duration::from_secs_f64(v.as_f64().unwrap_or(default).clamp(0.1, 3600.0))
}

struct Mcp {
    addr: String,
    output: Arc<Mutex<Vec<u8>>>,
    scratch: PathBuf,
}

impl Mcp {
    fn call(&self, name: &str, a: &Value) -> Value {
        match name {
            "eval" => {
                let code = a["code"].as_str().unwrap_or("").trim();
                if code.is_empty() {
                    return text("nothing to evaluate", true);
                }
                self.output.lock().unwrap().clear();
                let reply =
                    lang_driver::inspect::eval_within(&self.addr, code, Some(secs(&a["timeout_secs"], 30.0)));
                // output written by the evaluation reaches the pipe promptly
                std::thread::sleep(Duration::from_millis(20));
                let printed =
                    String::from_utf8_lossy(&std::mem::take(&mut *self.output.lock().unwrap())).into_owned();
                match reply {
                    Ok(r) => {
                        let r = r.trim_end();
                        let failed = r.starts_with("error") || r.contains("\nerror") || r.starts_with('[');
                        let mut out = printed;
                        if !r.is_empty() {
                            if !out.is_empty() && !out.ends_with('\n') {
                                out.push('\n');
                            }
                            out.push_str(r);
                        }
                        if out.is_empty() {
                            out = "(no value)".into();
                        }
                        text(out, failed)
                    }
                    Err(e) => text(format!("{printed}evaluation did not finish: {e}"), true),
                }
            }
            "check" => {
                let path = match (a["path"].as_str(), a["source"].as_str()) {
                    (Some(p), _) => p.to_string(),
                    (None, Some(src)) => {
                        let p = self.scratch.join("check.lang");
                        if let Err(e) = std::fs::write(&p, src) {
                            return text(format!("cannot write {}: {e}", p.display()), true);
                        }
                        p.display().to_string()
                    }
                    _ => return text("give `path` or `source`", true),
                };
                match lang(&["query".into(), "diagnostics".into(), path], Duration::from_secs(120)) {
                    Ok((0, out, _)) => {
                        let n = serde_json::from_str::<Value>(&out)
                            .ok()
                            .and_then(|v| v.as_array().map(|a| a.len()));
                        let summary = match n {
                            Some(0) => "no diagnostics: it checks".to_string(),
                            Some(n) => format!("{n} diagnostic(s):\n{out}"),
                            None => out,
                        };
                        text(summary, n.unwrap_or(1) > 0)
                    }
                    Ok((_, _, err)) => text(err, true),
                    Err(e) => text(e, true),
                }
            }
            "query" => {
                let kind = a["kind"].as_str().unwrap_or("");
                match kind {
                    "explain" => {
                        let code = a["name"].as_str().unwrap_or("");
                        match lang_base::catalog::explain(code) {
                            Some((title, body)) => {
                                text(format!("{} {title}\n\n{body}", code.to_uppercase()), false)
                            }
                            None => text(format!("no explanation for `{code}`"), true),
                        }
                    }
                    "module" => match a["name"].as_str() {
                        Some(n) => match lang_driver::std_source(n.trim_start_matches("std.")) {
                            Some(src) => text(src, false),
                            None => text(
                                format!(
                                    "no standard module `{n}`; modules: {}",
                                    lang_driver::std_modules().join(", ")
                                ),
                                true,
                            ),
                        },
                        None => text(lang_driver::std_modules().join(", "), false),
                    },
                    _ => {
                        let Some(path) = a["path"].as_str() else { return text("`path` is required", true) };
                        let mut args = vec!["query".to_string(), kind.to_string(), path.to_string()];
                        for k in ["line", "col"] {
                            if let Some(v) = a[k].as_u64() {
                                args.push(format!("--{k}"));
                                args.push(v.to_string());
                            }
                        }
                        match lang(&args, Duration::from_secs(120)) {
                            Ok((0, out, _)) => text(out.trim(), false),
                            Ok((_, _, err)) => text(err.trim(), true),
                            Err(e) => text(e, true),
                        }
                    }
                }
            }
            "run" => {
                let Some(path) = a["path"].as_str() else { return text("`path` is required", true) };
                let mut args: Vec<String> =
                    vec![if a["test"] == json!(true) { "test" } else { "run" }.into()];
                if a["release"] == json!(true) && args[0] == "run" {
                    args.push("--release".into());
                }
                args.push(path.into());
                if let Some(xs) = a["args"].as_array()
                    && args[0] == "run"
                {
                    args.push("--".into());
                    args.extend(xs.iter().filter_map(|x| x.as_str().map(str::to_string)));
                }
                match lang(&args, secs(&a["timeout_secs"], 60.0)) {
                    Ok((status, out, err)) => {
                        let mut t = format!("exit status {status}\n");
                        if !out.is_empty() {
                            t.push_str(&format!("--- stdout\n{out}"));
                        }
                        if !err.is_empty() {
                            t.push_str(&format!("--- stderr\n{err}"));
                        }
                        text(t, status != 0)
                    }
                    Err(e) => text(e, true),
                }
            }
            other => text(format!("unknown tool `{other}`"), true),
        }
    }
}

/// Serve MCP on the process's stdin/stdout while the REPL session program
/// runs on this thread (it returns only when the client disconnects).
pub fn serve() -> std::process::ExitCode {
    // the protocol keeps the real stdout; everything else written to fd 1
    // (the session's program output) is captured
    let (proto, output) = unsafe {
        let proto = libc::dup(1);
        let mut fds = [0; 2];
        if proto < 0 || libc::pipe(fds.as_mut_ptr()) != 0 {
            eprintln!("error: cannot redirect output");
            return std::process::ExitCode::FAILURE;
        }
        libc::dup2(fds[1], 1);
        libc::close(fds[1]);
        let output = Arc::new(Mutex::new(Vec::new()));
        let sink = output.clone();
        let mut reader = std::fs::File::from_raw_fd(fds[0]);
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let mut o = sink.lock().unwrap();
                // keep the most recent output if a program floods it
                if o.len() > 1 << 20 {
                    let cut = o.len() - (1 << 19);
                    o.drain(..cut);
                }
                o.extend_from_slice(&buf[..n]);
            }
        });
        (std::fs::File::from_raw_fd(proto), output)
    };
    let dir = std::env::temp_dir().join(format!("lang-mcp-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join("session.lang");
    let session = "// MCP session program\nimport std.io\nimport std.task\n\npub fn main() -> Unit {\n    loop {\n        task.sleep(3600s)\n    }\n}\n";
    if std::fs::write(&file, session).is_err() {
        eprintln!("error: cannot write the session file");
        return std::process::ExitCode::FAILURE;
    }
    let port = match std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()) {
        Ok(a) => a.port(),
        Err(e) => {
            eprintln!("error: cannot reserve a local port: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let addr = format!("127.0.0.1:{port}");
    let mcp = Mcp { addr: addr.clone(), output, scratch: dir.clone() };
    std::thread::spawn(move || {
        let mut proto = std::io::LineWriter::new(proto);
        let mut send = |v: Value| {
            let _ = writeln!(proto, "{v}");
            let _ = proto.flush();
        };
        let start = std::time::Instant::now();
        while std::net::TcpStream::connect(&mcp.addr).is_err() {
            if start.elapsed() > Duration::from_secs(30) {
                eprintln!("error: the session did not start");
                crate::exit_now(1);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if line.trim().is_empty() {
                continue;
            }
            let Ok(m) = serde_json::from_str::<Value>(&line) else {
                send(
                    json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": "parse error" } }),
                );
                continue;
            };
            let Some(id) = m.get("id").cloned() else { continue };
            let params = &m["params"];
            let result = match m["method"].as_str().unwrap_or("") {
                "initialize" => Ok(json!({
                    "protocolVersion": params["protocolVersion"].as_str().unwrap_or(PROTOCOL),
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": "lang", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": INSTRUCTIONS,
                })),
                "ping" => Ok(json!({})),
                "tools/list" => Ok(json!({ "tools": tools() })),
                "tools/call" => {
                    let name = params["name"].as_str().unwrap_or("");
                    Ok(mcp.call(name, &params["arguments"]))
                }
                other => Err(json!({ "code": -32601, "message": format!("method not found: {other}") })),
            };
            match result {
                Ok(r) => send(json!({ "jsonrpc": "2.0", "id": id, "result": r })),
                Err(e) => send(json!({ "jsonrpc": "2.0", "id": id, "error": e })),
            }
        }
        let _ = std::fs::remove_dir_all(&mcp.scratch);
        crate::exit_now(0)
    });
    lang_driver::set_repl_session();
    let mut s = lang_driver::Session::new();
    let opts = lang_driver::RunOptions { inspect: Some(addr), ..Default::default() };
    match s.run(&file, vec![], &opts) {
        Ok(code) => crate::exit_now(code),
        Err(_) => {
            crate::report(&s, false);
            std::process::ExitCode::FAILURE
        }
    }
}
