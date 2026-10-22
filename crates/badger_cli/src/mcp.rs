//! `badger mcp`: a Model Context Protocol server on stdio (newline-delimited
//! JSON-RPC) giving an agent the compiler and a live REPL through six tools:
//!
//! - `eval` ("code mode"): a persistent REPL session (the local `badger repl`
//!   machinery); definitions accumulate and expressions evaluate, and
//!   `stdx.compiler` exposes the compiler's analysis to the code itself, so
//!   one call can check, query, and post-process.
//! - `check`: diagnostics of a file or project, or of a source text.
//! - `query`: hover, definition, references, symbols, completion,
//!   diagnostic explanations, and standard module sources.
//! - `contracts`: each `requires`/`ensures` clause and whether it was proven,
//!   refuted, or is checked at run time (optionally run against the tests).
//! - `generate`: `badger generate feature`, checked (and its tests run) in a
//!   copy of the project before anything is written.
//! - `run`: run a program or its tests in a subprocess, with a time limit.
//!
//! Compiler questions run in subprocesses (`badger query`), isolated from the
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
definitions accumulate; an expression's value is shown). `import stdx.compiler` there to \
analyse files from code: compiler.diagnostics(path), compiler.check_source(src), \
compiler.hover(path, line, col), compiler.definition(...), compiler.references(...), \
compiler.symbols(path), compiler.completions(...), compiler.explain(code). Use `query` with \
kind `module` to read a standard module's API (e.g. `list`, `map`, `io`, `task`, `channel`), \
and `check` before `run`. `generate` adds a feature module that is checked (and its tests run) \
before it is written. Positions are 1-based.";

fn tools() -> Value {
    json!([
        {
            "name": "eval",
            "description": "Evaluate code in a persistent REPL session. Items (fn, struct, enum, import, module-level let) are added to the session and replace earlier ones of the same name; an expression is evaluated and its value displayed. Output printed by the code is returned too. `import stdx.compiler` gives compiler services as functions (diagnostics, check_source, hover, definition, references, symbols, completions, explain) returning typed records, for querying and post-processing analysis results in code.",
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
            "description": "Type-check a file or project directory (`path`) or a source text (`source`), returning diagnostics as JSON (file, line, col, severity, code, message, notes, label, expected, actual, scope, related, declarations). An empty list means it checks. A project is checked as `badger check` does: its main entry, `[entries]`, and `[tasks]`. With `files` (an object from path, relative to the program's directory, to proposed text) the project is checked with those contents in place of the files on disk, without writing anything, and the result also lists the inputs checked (file, proposed, bytes, hash), proposed files no module reached (`unused_overlays`), and the roots covered (entry points, tests, task spawns).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "source": { "type": "string" },
                    "files": { "type": "object", "additionalProperties": { "type": "string" } }
                }
            }
        },
        {
            "name": "query",
            "description": "Ask about code or the language. kind `guide` gives the guide built into this toolchain (`name`: a chapter number or word, e.g. `3` or `errors`; without it, the orientation page and the chapter list); `examples` gives the guide's checked code examples containing `name` (with chapter and heading); `symbol` finds a definition by qualified `name` (`orders.service.create`) in `path`. kind `hover` (type or signature), `definition`, `references` (every use, across all of a project's entry points and tasks), `completion` need path, line, col (1-based); `rename` also takes the new name in `name` and returns the edits (file, line, col, end_line, end_col, text) without applying them; `symbols` and `diagnostics` need path; `explain` takes a diagnostic code in `name` (e.g. E0301); `module` returns a standard module's source (its API and documentation) for `name` (e.g. `list`), or the list of modules without a name; `graph` returns the program's `calls`, `modules`, `types`, or `systems` graph (`name`) as JSON nodes and edges; `api` returns the public interface (functions, types, methods with signatures, effects, contracts, and docs) of a standard module (`name`, e.g. `std.list`) or of the program at `path`. Inputs a kind cannot answer (a missing file, a directory where a source file is needed, a file that does not parse for `symbols`) are errors, not empty results.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "kind": { "type": "string", "enum": ["hover", "definition", "references", "completion", "rename", "symbols", "symbol", "diagnostics", "explain", "module", "graph", "api", "guide", "examples"] },
                    "path": { "type": "string" },
                    "line": { "type": "integer" },
                    "col": { "type": "integer" },
                    "name": { "type": "string" }
                },
                "required": ["kind"]
            }
        },
        {
            "name": "contracts",
            "description": "The program's contract clauses (`requires`, `ensures`) and what the compiler decided about each: `proven` (checked statically; its runtime check is compiled out), `refuted` (an error), or `runtime` (checked when the program runs), with the reason. With `tests: true` it also runs the `test_*` functions observing the program: each clause's test coverage (`tested.evaluated`, `tested.held`; 0 evaluated is a gap no test reaches) and `suggestions`, candidate clauses that held in every observed call. The loop: write tests for the gaps and for clauses the compiler cannot prove, state the suggestions that are really intended, and ask again; the compiler checks what is stated.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "tests": { "type": "boolean" }
                },
                "required": ["path"]
            }
        },
        {
            "name": "generate",
            "description": "Generate checked code into a project. `what: worker`: `NAME.bdg`, a durable job kind (stdx.jobs) with a supervised processor, `submit`, and a test. `what: feature`: `NAME/model.bdg` (a record type, its errors, the `Store` it needs, its `Operations`), `NAME/memory.bdg`, `NAME/service.bdg` (create/find/list operations and tests), and `NAME/http.bdg` (routes and a test), under the entry's directory. The files are checked and their tests run in a copy of the project first; existing files are never replaced. With `dry_run: true` it reports the plan and the check without writing. The generated rules and storage are placeholders to replace with the feature's own.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "what": { "type": "string", "enum": ["feature", "worker"] },
                    "name": { "type": "string", "description": "The feature (a lowercase identifier, e.g. `orders`)." },
                    "path": { "type": "string", "description": "The project directory." },
                    "item": { "type": "string", "description": "One record's name (default: `name` without a final `s`)." },
                    "dry_run": { "type": "boolean" }
                },
                "required": ["what", "name", "path"]
            }
        },
        {
            "name": "run",
            "description": "Run a program (file or project directory) or, with `test: true`, its `test_*` functions, in a fresh process. Returns the exit status, stdout, and stderr. With `breakpoints` (`file:line`, optionally `file:line if CONDITION`), it runs under gdb with debug information and reports, at each stop, the innermost frames, arguments, and locals (or the gdb `commands` given), up to `max_stops`. With `profile: true`, it also returns a CPU profile (functions by self and total time).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "args": { "type": "array", "items": { "type": "string" } },
                    "test": { "type": "boolean" },
                    "release": { "type": "boolean" },
                    "timeout_secs": { "type": "number", "description": "Kill the process after this long (default 60)." },
                    "breakpoints": { "type": "array", "items": { "type": "string" }, "description": "Stop at `file:line` or `file:line if CONDITION` (gdb syntax)." },
                    "commands": { "type": "array", "items": { "type": "string" }, "description": "gdb commands at each stop (default: bt 5, info args, info locals). Only with `breakpoints`." },
                    "max_stops": { "type": "integer", "description": "Stops to report before ending the run (default 10). Only with `breakpoints`." },
                    "profile": { "type": "boolean" }
                },
                "required": ["path"]
            }
        }
    ])
}

fn exe() -> String {
    std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|_| "badger".into())
}

/// Run `badger ARGS` (120 seconds at most): its standard output when it
/// succeeds, else its standard error as an error result.
fn subprocess_result(args: &[String]) -> Value {
    match badger_cmd(args, Duration::from_secs(120)) {
        Ok((0, out, _)) => text(out.trim(), false),
        Ok((_, _, err)) => text(err.trim(), true),
        Err(e) => text(e, true),
    }
}

/// Run `badger ARGS` with a time limit: (status, stdout, stderr).
fn badger_cmd(args: &[String], limit: Duration) -> Result<(i32, String, String), String> {
    badger_cmd_env(args, &[], limit)
}

/// `badger` with extra environment variables for the child.
fn badger_cmd_env(
    args: &[String],
    env: &[(&str, String)],
    limit: Duration,
) -> Result<(i32, String, String), String> {
    let mut cmd = std::process::Command::new(exe());
    cmd.args(args).envs(env.iter().map(|(k, v)| (*k, v.as_str())));
    run_limited(cmd, limit)
}

/// Run a command to completion within `limit` (killed after it).
fn run_limited(mut cmd: std::process::Command, limit: Duration) -> Result<(i32, String, String), String> {
    use std::os::unix::process::CommandExt;
    let program = cmd.get_program().to_string_lossy().into_owned();
    // its own process group, so a time limit stops whatever it started
    cmd.process_group(0);
    let mut child = cmd
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot start {program}: {e}"))?;
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
            unsafe { libc::killpg(child.id() as i32, libc::SIGKILL) };
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("timed out after {}s", limit.as_secs_f64()));
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    Ok((status, ro.join().unwrap_or_default(), re.join().unwrap_or_default()))
}

/// A path argument for a subprocess (never read as an option).
fn arg_path(p: &str) -> String {
    if p.starts_with('-') { format!("./{p}") } else { p.to_string() }
}

/// The guide, as built into this toolchain (so an agent reads the version
/// it is talking to): chapters by file name, and the orientation page.
const GUIDE: &[(&str, &str)] = &[
    ("01-values-and-functions", include_str!("../../../docs/guide/01-values-and-functions.md")),
    ("02-types-and-records", include_str!("../../../docs/guide/02-types-and-records.md")),
    ("03-errors", include_str!("../../../docs/guide/03-errors.md")),
    ("04-collections", include_str!("../../../docs/guide/04-collections.md")),
    ("05-modules-and-projects", include_str!("../../../docs/guide/05-modules-and-projects.md")),
    ("06-tasks-and-channels", include_str!("../../../docs/guide/06-tasks-and-channels.md")),
    ("07-effects-and-ownership", include_str!("../../../docs/guide/07-effects-and-ownership.md")),
    ("08-systems-and-pipelines", include_str!("../../../docs/guide/08-systems-and-pipelines.md")),
    ("09-testing-and-contracts", include_str!("../../../docs/guide/09-testing-and-contracts.md")),
    ("10-services", include_str!("../../../docs/guide/10-services.md")),
    ("collections", include_str!("../../../docs/collections.md")),
];
const LLMS: &str = include_str!("../../../llms.txt");

/// A guide chapter by number or a word of its name (`3`, `errors`), or the
/// list of chapters and the orientation page.
fn guide(name: Option<&str>) -> Value {
    let Some(want) = name.map(str::to_lowercase).filter(|w| !w.is_empty()) else {
        let names: Vec<&str> = GUIDE.iter().map(|(n, _)| *n).collect();
        return text(format!("{LLMS}\nchapters: {}", names.join(", ")), false);
    };
    // a chapter number (`3`), or a word of its name (`errors`)
    let number = want.parse::<u32>().ok();
    let hit = GUIDE.iter().find(|(n, _)| match number {
        Some(k) => n.split('-').next().and_then(|p| p.parse::<u32>().ok()) == Some(k),
        None => n.contains(want.as_str()),
    });
    match hit {
        Some((_, body)) => text(*body, false),
        None => text(
            format!(
                "no chapter `{want}`; chapters: {}",
                GUIDE.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")
            ),
            true,
        ),
    }
}

/// The guide's checked examples (its `badger` code blocks, which the
/// documentation tests check) that contain `term`, with the chapter and
/// heading each comes from.
fn examples(term: &str) -> Value {
    let mut out = vec![];
    for (chapter, body) in GUIDE {
        let mut heading = "";
        let mut lines = body.lines();
        while let Some(l) = lines.next() {
            if l.starts_with('#') {
                heading = l.trim_start_matches('#').trim();
            }
            if l.trim_start() == "```badger" || l.trim_start() == "```badger fragment" {
                let fragment = l.contains("fragment");
                let mut code = vec![];
                for c in lines.by_ref() {
                    if c.trim_start() == "```" {
                        break;
                    }
                    code.push(c);
                }
                let code = code.join("\n");
                if term.is_empty() || code.contains(term) {
                    out.push(json!({ "chapter": chapter, "heading": heading, "complete": !fragment, "code": code }));
                }
            }
        }
    }
    let n = out.len();
    out.truncate(20);
    text(json!({ "matches": n, "examples": out }).to_string(), false)
}

/// A result whose text is JSON (or a summary line and then JSON, as
/// `contracts` gives) also carries it as `structuredContent` (an object; a
/// list is `{"result": [...]}`), so clients need not parse the text.
fn structured(mut r: Value) -> Value {
    let Some(t) = r["content"][0]["text"].as_str() else { return r };
    let json = serde_json::from_str::<Value>(t)
        .ok()
        .or_else(|| t.split_once('\n').and_then(|(_, rest)| serde_json::from_str::<Value>(rest).ok()));
    match json {
        Some(v @ Value::Object(_)) => r["structuredContent"] = v,
        Some(v @ Value::Array(_)) => r["structuredContent"] = json!({ "result": v }),
        _ => {}
    }
    r
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
                let reply = badger_driver::inspect::eval_within(
                    &self.addr,
                    code,
                    Some(secs(&a["timeout_secs"], 30.0)),
                );
                // output written by the evaluation reaches the pipe promptly
                std::thread::sleep(Duration::from_millis(20));
                let printed =
                    String::from_utf8_lossy(&std::mem::take(&mut *self.output.lock().unwrap())).into_owned();
                match reply {
                    Ok(r) => {
                        let (status, r) = r.split_once('\n').unwrap_or((r.as_str(), ""));
                        let failed = status != "ok";
                        let r = r.trim_end();
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
            "check" if !a["files"].is_null() && !a["files"].is_object() => {
                text("`files` maps file paths to their unsaved text (an object)", true)
            }
            "check" if a["files"].is_object() => {
                let Some(path) = a["path"].as_str() else {
                    return text("`files` needs the project's `path`", true);
                };
                let p = self.scratch.join("overlays.json");
                if let Err(e) = std::fs::write(&p, a["files"].to_string()) {
                    return text(format!("cannot write {}: {e}", p.display()), true);
                }
                let args = [
                    "query".into(),
                    "check".into(),
                    arg_path(path),
                    "--overlays".into(),
                    p.display().to_string(),
                ];
                match badger_cmd(&args, Duration::from_secs(120)) {
                    Ok((0, out, _)) => {
                        let r: Value = serde_json::from_str(&out).unwrap_or(Value::Null);
                        let n = r["diagnostics"].as_array().map(|d| d.len()).unwrap_or(0);
                        let summary = if n == 0 {
                            "no diagnostics: it checks".to_string()
                        } else {
                            format!("{n} diagnostic(s)")
                        };
                        text(format!("{summary}\n{out}"), !r["ok"].as_bool().unwrap_or(false))
                    }
                    Ok((_, _, err)) => text(err, true),
                    Err(e) => text(e, true),
                }
            }
            "generate" => {
                let Some(what) = a["what"].as_str().filter(|w| matches!(*w, "feature" | "worker")) else {
                    return text("`what` must be `feature` or `worker`", true);
                };
                let (Some(name), Some(path)) = (a["name"].as_str(), a["path"].as_str()) else {
                    return text("give `name` and `path`", true);
                };
                // a name or item is a word, never an option of the command
                let word = |s: &str| {
                    s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                };
                if !word(name) {
                    return text(format!("`name` must be a word (letters, digits, `_`): `{name}`"), true);
                }
                // `dry_run` writes nothing: only a real boolean `false` (or no
                // value) writes files
                let dry_run = match &a["dry_run"] {
                    Value::Null => false,
                    Value::Bool(b) => *b,
                    other => return text(format!("`dry_run` must be true or false, not {other}"), true),
                };
                let mut args =
                    vec!["generate".to_string(), what.to_string(), name.to_string(), arg_path(path)];
                match (&a["item"], what) {
                    (Value::Null, _) => {}
                    (_, "worker") => return text("`item` applies only to `feature`", true),
                    (Value::String(item), _) if word(item) => {
                        args.push("--item".into());
                        args.push(item.clone());
                    }
                    (other, _) => return text(format!("`item` must be a word, not {other}"), true),
                }
                if dry_run {
                    args.push("--dry-run".into());
                }
                match badger_cmd(&args, Duration::from_secs(300)) {
                    Ok((code, out, err)) => text(format!("{out}{err}"), code != 0),
                    Err(e) => text(e, true),
                }
            }
            "contracts" => {
                let Some(path) = a["path"].as_str() else { return text("give `path`", true) };
                let mut args = vec!["contracts".to_string(), "--json".into()];
                if a["tests"].as_bool().unwrap_or(false) {
                    args.push("--tests".into());
                }
                args.push(arg_path(path));
                match badger_cmd(&args, Duration::from_secs(300)) {
                    Ok((_, out, err)) => match serde_json::from_str::<Value>(&out) {
                        Ok(v) => {
                            let clauses = v["clauses"].as_array().cloned().unwrap_or_default();
                            let count = |s: &str| clauses.iter().filter(|c| c["status"] == s).count();
                            let gaps: Vec<&Value> = clauses
                                .iter()
                                .filter(|c| c["tested"]["evaluated"] == 0 && c["status"] != "proven")
                                .collect();
                            let mut summary = format!(
                                "{} clause(s): {} proven, {} checked at run time, {} refuted",
                                clauses.len(),
                                count("proven"),
                                count("runtime"),
                                count("refuted")
                            );
                            if v.get("tests").is_some() {
                                summary.push_str(&format!(
                                    "; {} test(s) ran, {} failed; {} unproven clause(s) no test reaches; {} suggestion(s)",
                                    v["tests"]["ran"],
                                    v["tests"]["failed"],
                                    gaps.len(),
                                    v["suggestions"].as_array().map_or(0, |s| s.len())
                                ));
                            }
                            text(format!("{summary}\n{out}"), !v["ok"].as_bool().unwrap_or(false))
                        }
                        Err(_) => text(format!("{out}{err}"), true),
                    },
                    Err(e) => text(e, true),
                }
            }
            "check" => {
                let path = match (a["path"].as_str(), a["source"].as_str()) {
                    (Some(p), _) => arg_path(p),
                    (None, Some(src)) => {
                        let p = self.scratch.join("check.bdg");
                        if let Err(e) = std::fs::write(&p, src) {
                            return text(format!("cannot write {}: {e}", p.display()), true);
                        }
                        p.display().to_string()
                    }
                    _ => return text("give `path` or `source`", true),
                };
                match badger_cmd(&["query".into(), "diagnostics".into(), path], Duration::from_secs(120)) {
                    Ok((0, out, _)) => {
                        let ds = serde_json::from_str::<Value>(&out).ok().and_then(|v| v.as_array().cloned());
                        // only errors make it fail (warnings are reported, the program checks)
                        let errors =
                            ds.as_ref().map(|d| d.iter().filter(|x| x["severity"] == "error").count());
                        let summary = match ds.as_ref().map(Vec::len) {
                            Some(0) => "no diagnostics: it checks".to_string(),
                            Some(n) => format!("{n} diagnostic(s):\n{out}"),
                            None => out,
                        };
                        text(summary, errors.unwrap_or(1) > 0)
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
                        match badger_base::catalog::explain(code) {
                            Some((title, body)) => {
                                text(format!("{} {title}\n\n{body}", code.to_uppercase()), false)
                            }
                            None => text(format!("no explanation for `{code}`"), true),
                        }
                    }
                    "guide" => guide(a["name"].as_str()),
                    "examples" => examples(a["name"].as_str().unwrap_or("")),
                    "module" => match a["name"].as_str() {
                        Some(n) => {
                            // a bare name (`http`) resolves to its namespace
                            let qualified = match badger_driver::std_root_of(n) {
                                Some(root) => format!("{root}.{n}"),
                                None => n.to_string(),
                            };
                            match badger_driver::std_source(&qualified) {
                                Some(src) => text(src, false),
                                None => text(
                                    format!(
                                        "no standard module `{n}`; modules: {}",
                                        badger_driver::std_modules().join(", ")
                                    ),
                                    true,
                                ),
                            }
                        }
                        None => text(badger_driver::std_modules().join(", "), false),
                    },
                    "api" => {
                        let Some(target) = a["name"].as_str().or(a["path"].as_str()) else {
                            return text("give `name` (e.g. std.list) or `path`", true);
                        };
                        let args = vec!["query".into(), "api".into(), arg_path(target)];
                        subprocess_result(&args)
                    }
                    "graph" => {
                        let Some(path) = a["path"].as_str() else { return text("`path` is required", true) };
                        let which = a["name"].as_str().unwrap_or("calls").to_string();
                        let args =
                            vec!["graph".into(), which, arg_path(path), "--format".into(), "json".into()];
                        subprocess_result(&args)
                    }
                    "hover" | "definition" | "references" | "completion" | "symbols" | "symbol"
                    | "diagnostics" => {
                        let Some(path) = a["path"].as_str() else { return text("`path` is required", true) };
                        let mut args = vec!["query".to_string(), kind.to_string(), arg_path(path)];
                        for k in ["line", "col"] {
                            match &a[k] {
                                Value::Null => {}
                                v => match v.as_u64() {
                                    Some(n) if n >= 1 => {
                                        args.push(format!("--{k}"));
                                        args.push(n.to_string());
                                    }
                                    _ => return text(format!("`{k}` is a position from 1, not {v}"), true),
                                },
                            }
                        }
                        if kind == "symbol"
                            && let Some(n) = a["name"].as_str()
                        {
                            args.push(format!("--name={n}"));
                        }
                        subprocess_result(&args)
                    }
                    // the rename computes the edits; `--json` writes nothing
                    "rename" => {
                        let Some(path) = a["path"].as_str() else { return text("`path` is required", true) };
                        let mut args = vec!["rename".to_string(), arg_path(path)];
                        for k in ["line", "col"] {
                            if let Some(v) = a[k].as_u64() {
                                args.push(format!("--{k}"));
                                args.push(v.to_string());
                            }
                        }
                        if let Some(n) = a["name"].as_str() {
                            args.push("--name".into());
                            args.push(n.to_string());
                        }
                        args.push("--json".into());
                        subprocess_result(&args)
                    }
                    // `check` has its own tool
                    other => text(
                        format!(
                            "unknown query kind `{other}` (hover, definition, references, completion, rename, symbols, symbol, diagnostics, guide, examples, explain, module, graph, api; diagnostics of proposed contents: the `check` tool)"
                        ),
                        true,
                    ),
                }
            }
            "run" => {
                let Some(path) = a["path"].as_str() else { return text("`path` is required", true) };
                // a wrongly typed option is refused, not read as absent
                for (opt, want, ok) in [
                    ("test", "true or false", a["test"].is_boolean()),
                    ("release", "true or false", a["release"].is_boolean()),
                    ("profile", "true or false", a["profile"].is_boolean()),
                    ("args", "a list of strings", a["args"].is_array()),
                    ("breakpoints", "a list", a["breakpoints"].is_array()),
                    ("timeout_secs", "a number", a["timeout_secs"].is_number()),
                ] {
                    if !a[opt].is_null() && !ok {
                        return text(format!("`{opt}` is {want}, not {}", a[opt]), true);
                    }
                }
                // options that would be ignored are refused, not dropped
                let debugging = a["breakpoints"].as_array().is_some_and(|b| !b.is_empty());
                for opt in ["commands", "max_stops"] {
                    if !a[opt].is_null() && !debugging {
                        return text(format!("`{opt}` applies only with `breakpoints`"), true);
                    }
                }
                if debugging && a["profile"] == json!(true) {
                    return text(
                        "`profile` cannot be used with `breakpoints`: a debugged run is not profiled",
                        true,
                    );
                }
                if a["test"] == json!(true) {
                    for (opt, why) in [
                        ("breakpoints", "breakpoints debug a program run, not its tests"),
                        ("args", "tests take no program arguments"),
                        ("release", "tests run unoptimized"),
                    ] {
                        if !a[opt].is_null() && a[opt] != json!(false) && a[opt] != json!([]) {
                            return text(format!("`{opt}` cannot be used with `test`: {why}"), true);
                        }
                    }
                }
                let mut args: Vec<String> =
                    vec![if a["test"] == json!(true) { "test" } else { "run" }.into()];
                if a["release"] == json!(true) && args[0] == "run" {
                    args.push("--release".into());
                }
                args.push(arg_path(path));
                if let Some(xs) = a["args"].as_array()
                    && args[0] == "run"
                {
                    // program arguments are strings (not dropped silently)
                    if let Some(bad) = xs.iter().find(|x| !x.is_string()) {
                        return text(format!("`args` are strings; {bad} is not"), true);
                    }
                    args.push("--".into());
                    args.extend(xs.iter().filter_map(|x| x.as_str().map(str::to_string)));
                }
                if let Some(bps) = a["breakpoints"].as_array().filter(|b| !b.is_empty()) {
                    return self.debug_run(path, bps, a);
                }
                let mut env = vec![];
                if a["profile"] == json!(true) {
                    // the summary is printed on stderr
                    env.push(("BADGER_PROFILE", self.scratch.join("profile.folded").display().to_string()));
                }
                match badger_cmd_env(&args, &env, secs(&a["timeout_secs"], 60.0)) {
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

    /// Run a program under gdb with breakpoints, reporting each stop.
    fn debug_run(&self, path: &str, bps: &[Value], a: &Value) -> Value {
        let commands: Vec<String> = match a["commands"].as_array() {
            Some(cs) => cs.iter().filter_map(|c| c.as_str().map(str::to_string)).collect(),
            None => vec!["bt 5".into(), "info args".into(), "info locals".into()],
        };
        let max = a["max_stops"].as_u64().unwrap_or(10);
        let mut script = String::from(
            "set debuginfod enabled off\nset breakpoint pending on\nset pagination off\nset confirm off\nset $stops = 0\n",
        );
        for (i, bp) in bps.iter().filter_map(|b| b.as_str()).enumerate() {
            script.push_str(&format!("break {bp}\ncommands {}\nset $stops = $stops + 1\n", i + 1));
            for c in &commands {
                script.push_str(c);
                script.push('\n');
            }
            script.push_str(&format!("if $stops < {max}\ncontinue\nend\nend\n"));
        }
        script.push_str("run\n");
        let file = self.scratch.join("debug.gdb");
        if let Err(e) = std::fs::write(&file, script) {
            return text(format!("cannot write {}: {e}", file.display()), true);
        }
        let mut args = vec!["-batch", "-nx", "-x"].into_iter().map(String::from).collect::<Vec<_>>();
        args.push(file.display().to_string());
        args.extend(["--args".to_string(), exe(), "run".into(), "--debug".into()]);
        if a["release"] == json!(true) {
            args.push("--release".into());
        }
        args.push(arg_path(path));
        if let Some(xs) = a["args"].as_array() {
            args.push("--".into());
            args.extend(xs.iter().filter_map(|x| x.as_str().map(str::to_string)));
        }
        let mut cmd = std::process::Command::new("gdb");
        cmd.args(&args);
        match run_limited(cmd, secs(&a["timeout_secs"], 60.0)) {
            Err(e) => text(format!("debugging with gdb: {e}"), true),
            Ok((_, out, err)) => {
                // keep the debugger's report, not its thread chatter
                let keep = |l: &&str| {
                    !(l.starts_with("[New Thread")
                        || l.starts_with("[Thread")
                        || l.contains("libthread_db")
                        || l.starts_with("Using host")
                        || l.contains("auto-load")
                        || l.starts_with("of file")
                        || l.starts_with("Use `info auto-load"))
                };
                let report: Vec<&str> = out.lines().chain(err.lines()).filter(keep).collect();
                // the program's failure, not gdb's status, makes it an error
                let failed = report.iter().any(|l| {
                    l.contains("exited with code") || l.starts_with("Program terminated with signal")
                });
                text(report.join("\n"), failed)
            }
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
                    let mark = b"[earlier output dropped]\n";
                    o.splice(0..0, mark.iter().copied());
                }
                o.extend_from_slice(&buf[..n]);
            }
        });
        (std::fs::File::from_raw_fd(proto), output)
    };
    let Some(setup) = crate::prepare_session("badger-mcp") else { return std::process::ExitCode::FAILURE };
    let mcp = Mcp { addr: setup.addr.clone(), output, scratch: setup.dir.clone() };
    std::thread::spawn(move || {
        let mut proto = std::io::LineWriter::new(proto);
        let mut send = |v: Value| {
            let _ = writeln!(proto, "{v}");
            let _ = proto.flush();
        };
        crate::wait_for_session(&mcp.addr);
        // lines as bytes: invalid UTF-8 is an unparsable request, not the end
        for line in std::io::stdin().lock().split(b'\n') {
            let Ok(line) = line else { break };
            let line = String::from_utf8_lossy(&line).into_owned();
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
                    "serverInfo": { "name": "badger", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": INSTRUCTIONS,
                })),
                "ping" => Ok(json!({})),
                "tools/list" => Ok(json!({ "tools": tools() })),
                "tools/call" => {
                    let name = params["name"].as_str().unwrap_or("");
                    Ok(structured(mcp.call(name, &params["arguments"])))
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
    badger_driver::forbid_exit();
    crate::run_session(&setup)
}
