//! Reproduction bundles. `badger run --record FILE` keeps the outcome of every
//! external operation a program performs (HTTP requests, subprocesses, file
//! reads and writes, environment variables, the clock, OS-seeded random
//! sources), its output, and how it ended, with the program's identity, in
//! one JSON file. `badger replay FILE` runs the program again with each
//! operation answered from the bundle instead of the world: nothing is
//! requested, run, or written. An operation the bundle does not hold, or
//! one that differs from the recording, ends the replay (it never falls
//! back to the real operation).
//!
//! Operations match by identity (kind and key, such as `fetch` and
//! `GET http://host/path`, in recorded order per identity), so tasks may
//! interleave differently; what an operation sends (a request or file body)
//! is compared as a digest. In comparison mode (`--compare`, for changed
//! code) differing payloads and unused operations are reported instead of
//! ending the run. Values of environment variables whose names look like
//! credentials are not stored; a replay that needs one stops and says so.
//! Databases and served HTTP are not captured: a replay that reaches them
//! stops.

use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};

const OFF: u8 = 0;
const RECORD: u8 = 1;
const REPLAY: u8 = 2;

/// Exit status of a replay that diverged from its bundle.
pub const DIVERGED: i32 = 3;
/// Bundle format identifier.
pub const FORMAT: &str = "badger-repro/1";
/// Recorded operations and output are bounded; beyond this the bundle is
/// marked incomplete.
const MAX_BYTES: usize = 64 << 20;

static MODE: AtomicU8 = AtomicU8::new(OFF);

struct Op {
    kind: String,
    key: String,
    digest: Option<String>,
    value: Value,
    redacted: bool,
}

struct State {
    path: std::path::PathBuf,
    meta: Value,
    ops: Vec<Op>,
    used: Vec<bool>,
    /// Replay: recorded indices in the order they were used.
    order: Vec<usize>,
    stdout: String,
    bytes: usize,
    incomplete: bool,
    limitations: Vec<String>,
    notes: Vec<String>,
    compare: bool,
    /// Replay: the recorded outcome.
    expected: Value,
    finished: bool,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

#[inline]
pub fn active() -> bool {
    MODE.load(Ordering::Relaxed) != OFF
}

fn fnv(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{h:016x}")
}

/// A digest of a payload (for identities and comparisons).
pub fn digest(bytes: &[u8]) -> String {
    fnv(bytes)
}

/// Whether a name looks like it holds a credential.
pub fn secret_name(name: &str) -> bool {
    let n = name.to_ascii_uppercase();
    ["SECRET", "TOKEN", "PASSWORD", "PASSWD", "CREDENTIAL", "API_KEY", "APIKEY", "PRIVATE_KEY", "AUTH"]
        .iter()
        .any(|s| n.contains(s))
}

/// A URL with the values of credential-like query parameters and any user
/// information masked.
pub fn mask_url(url: &str) -> String {
    let (base, query) = match url.split_once('?') {
        Some((b, q)) => (b.to_string(), Some(q)),
        None => (url.to_string(), None),
    };
    let base = match base.split_once("://") {
        Some((s, rest)) => match rest.split_once('/') {
            Some((auth, path)) if auth.contains('@') => {
                format!("{s}://***@{}/{path}", auth.rsplit('@').next().unwrap())
            }
            None if rest.contains('@') => format!("{s}://***@{}", rest.rsplit('@').next().unwrap()),
            _ => base,
        },
        None => base,
    };
    match query {
        None => base,
        Some(q) => {
            let parts: Vec<String> = q
                .split('&')
                .map(|kv| match kv.split_once('=') {
                    Some((k, _)) if secret_name(k) => format!("{k}=***"),
                    _ => kv.to_string(),
                })
                .collect();
            format!("{base}?{}", parts.join("&"))
        }
    }
}

/// Start recording into `path`; `meta` identifies the program.
pub fn start_record(path: std::path::PathBuf, meta: Value) {
    *STATE.lock().unwrap() = Some(State {
        path,
        meta,
        ops: vec![],
        used: vec![],
        order: vec![],
        stdout: String::new(),
        bytes: 0,
        incomplete: false,
        limitations: vec![],
        notes: vec![],
        compare: false,
        expected: Value::Null,
        finished: false,
    });
    MODE.store(RECORD, Ordering::SeqCst);
}

/// Start replaying `bundle` (already checked against the program by the
/// caller); `compare` relaxes payload and completeness checks.
pub fn start_replay(path: std::path::PathBuf, bundle: &Value, compare: bool) -> Result<(), String> {
    let mut ops = vec![];
    for o in bundle["operations"].as_array().ok_or("the bundle has no operations list")? {
        ops.push(Op {
            kind: o["kind"].as_str().unwrap_or("").to_string(),
            key: o["key"].as_str().unwrap_or("").to_string(),
            digest: o["digest"].as_str().map(str::to_string),
            value: o["value"].clone(),
            redacted: o["redacted"].as_bool().unwrap_or(false),
        });
    }
    let n = ops.len();
    *STATE.lock().unwrap() = Some(State {
        path,
        meta: bundle["program"].clone(),
        ops,
        used: vec![false; n],
        order: vec![],
        stdout: String::new(),
        bytes: 0,
        incomplete: bundle["incomplete"].as_bool().unwrap_or(false),
        limitations: vec![],
        notes: vec![],
        compare,
        expected: bundle["outcome"].clone(),
        finished: false,
    });
    MODE.store(REPLAY, Ordering::SeqCst);
    Ok(())
}

/// End a replay that cannot continue faithfully.
fn diverge(msg: String) -> ! {
    use std::io::Write;
    let _ = std::io::stdout().flush();
    eprintln!("replay diverged: {msg}");
    eprintln!("  (the replay stops instead of performing the operation for real)");
    if let Some(s) = STATE.lock().unwrap().as_mut() {
        s.finished = true;
    }
    unsafe { libc::_exit(DIVERGED) }
}

/// Perform (or, in a replay, answer) one external operation. `digest`
/// summarizes what it sends, if anything.
pub fn external<T: Serialize + DeserializeOwned>(
    kind: &str,
    key: &str,
    digest: Option<String>,
    redact: bool,
    real: impl FnOnce() -> T,
) -> T {
    match MODE.load(Ordering::Relaxed) {
        OFF => real(),
        RECORD => {
            let v = real();
            let value = if redact { Value::Null } else { serde_json::to_value(&v).unwrap_or(Value::Null) };
            let mut g = STATE.lock().unwrap();
            let s = g.as_mut().unwrap();
            let size = kind.len() + key.len() + value.to_string().len();
            if s.bytes + size > MAX_BYTES {
                s.incomplete = true;
            } else {
                s.bytes += size;
                s.ops.push(Op { kind: kind.into(), key: key.into(), digest, value, redacted: redact });
            }
            v
        }
        _ => answer(kind, key, replayed(kind, key, &digest)),
    }
}

/// The recorded outcome of an operation the task stopped waiting for.
const CANCELLED: &str = "$cancelled";

/// Like [`external`], for an operation the task can be cancelled while it
/// waits (an async native call). A recording files the operation when it
/// starts, so one the task stopped waiting for is noted as such; a replay
/// of that one calls `wait_cancelled`, which waits until the task is
/// cancelled again (by the same timeout, say) and never returns.
pub fn external_cancellable<T: Serialize + DeserializeOwned>(
    kind: &str,
    key: &str,
    digest: Option<String>,
    real: impl FnOnce() -> T,
    wait_cancelled: impl FnOnce() -> T,
) -> T {
    match MODE.load(Ordering::Relaxed) {
        OFF => real(),
        RECORD => {
            let slot = {
                let mut g = STATE.lock().unwrap();
                let s = g.as_mut().unwrap();
                let size = kind.len() + key.len();
                if s.bytes + size > MAX_BYTES {
                    s.incomplete = true;
                    None
                } else {
                    s.bytes += size;
                    let value = serde_json::json!({ CANCELLED: true });
                    s.ops.push(Op { kind: kind.into(), key: key.into(), digest, value, redacted: false });
                    Some(s.ops.len() - 1)
                }
            };
            let v = real();
            if let Some(i) = slot {
                let value = serde_json::to_value(&v).unwrap_or(Value::Null);
                let mut g = STATE.lock().unwrap();
                let s = g.as_mut().unwrap();
                let size = value.to_string().len();
                if s.bytes + size > MAX_BYTES {
                    // too large to keep: the operation stays unanswerable
                    s.incomplete = true;
                    s.ops[i].value = Value::Null;
                    s.ops[i].redacted = true;
                } else {
                    s.bytes += size;
                    s.ops[i].value = value;
                }
            }
            v
        }
        _ => {
            let value = replayed(kind, key, &digest);
            if value.get(CANCELLED) == Some(&Value::Bool(true)) {
                return wait_cancelled();
            }
            answer(kind, key, value)
        }
    }
}

fn answer<T: DeserializeOwned>(kind: &str, key: &str, value: Value) -> T {
    match serde_json::from_value(value) {
        Ok(v) => v,
        Err(e) => diverge(format!("`{kind} {key}`: the recorded outcome does not fit ({e})")),
    }
}

/// The next unused recorded outcome of `kind key` (a replay).
fn replayed(kind: &str, key: &str, digest: &Option<String>) -> Value {
    let mut g = STATE.lock().unwrap();
    let s = g.as_mut().unwrap();
    // the same operation sending the same content first: concurrent tasks
    // may reach operations with one key in another order than recorded
    let unused = |i: &usize| !s.used[*i] && s.ops[*i].kind == kind && s.ops[*i].key == key;
    let found = (0..s.ops.len())
        .find(|i| unused(i) && s.ops[*i].digest == *digest)
        .or_else(|| (0..s.ops.len()).find(unused));
    let Some(i) = found else {
        let done = s.ops.iter().zip(&s.used).filter(|(o, u)| **u && o.kind == kind && o.key == key).count();
        let why = if s.incomplete {
            " (the bundle is incomplete: recording stopped at its size limit)"
        } else if done > 0 {
            " again (the recording has it fewer times)"
        } else {
            ""
        };
        drop(g);
        diverge(format!("`{kind} {key}` was not recorded{why}"));
    };
    let op = &s.ops[i];
    if op.redacted {
        drop(g);
        diverge(format!(
            "`{kind} {key}` was redacted from the bundle (it looks like a credential), so it cannot be replayed"
        ));
    }
    if op.digest != *digest {
        if s.compare {
            s.notes.push(format!("`{kind} {key}` sends different content than the recording"));
        } else {
            drop(g);
            diverge(format!("`{kind} {key}` sends different content than the recording"));
        }
    }
    s.used[i] = true;
    s.order.push(i);
    op.value.clone()
}

/// The program reached something bundles do not capture.
pub fn uncaptured(what: &str) {
    match MODE.load(Ordering::Relaxed) {
        OFF => {}
        RECORD => {
            let mut g = STATE.lock().unwrap();
            let s = g.as_mut().unwrap();
            let l = format!("{what} is not captured; a replay that reaches it stops");
            if !s.limitations.contains(&l) {
                s.limitations.push(l);
            }
        }
        _ => diverge(format!("{what} is not captured in bundles")),
    }
}

/// Program output (standard output), kept with the bundle.
pub fn output(text: &str, newline: bool) {
    if !active() {
        return;
    }
    let mut g = STATE.lock().unwrap();
    let s = g.as_mut().unwrap();
    if s.stdout.len() + text.len() < MAX_BYTES / 4 {
        s.stdout.push_str(text);
        if newline {
            s.stdout.push('\n');
        }
    } else if MODE.load(Ordering::Relaxed) == RECORD {
        s.incomplete = true;
    }
}

/// The source line a failure location (`... at PATH:LINE:COL`) names.
fn context(failure: &str) -> Value {
    let Some(at) = failure.rsplit(" at ").next() else { return Value::Null };
    let mut it = at.trim().rsplitn(3, ':');
    let (Some(col), Some(line), Some(path)) = (it.next(), it.next(), it.next()) else { return Value::Null };
    let (Ok(line), Ok(col)) = (line.parse::<usize>(), col.parse::<usize>()) else { return Value::Null };
    let Ok(text) = std::fs::read_to_string(path) else { return Value::Null };
    match text.lines().nth(line.wrapping_sub(1)) {
        Some(src) => json!({ "file": path, "line": line, "col": col, "source": src }),
        None => Value::Null,
    }
}

/// The program ended with `code` (and `failure`, what it reported). A
/// recording writes its bundle; a replay compares the outcome with the
/// recorded one and reports. Returns the exit status to use.
pub fn finish(code: i32, failure: Option<String>) -> i32 {
    let mode = MODE.load(Ordering::Relaxed);
    if mode == OFF {
        return code;
    }
    let mut g = STATE.lock().unwrap();
    let Some(s) = g.as_mut() else { return code };
    if s.finished {
        return code;
    }
    s.finished = true;
    if mode == RECORD {
        let ops: Vec<Value> = s
            .ops
            .iter()
            .enumerate()
            .map(|(i, o)| {
                let mut v = json!({ "seq": i, "kind": o.kind, "key": o.key, "value": o.value });
                if let Some(d) = &o.digest {
                    v["digest"] = json!(d);
                }
                if o.redacted {
                    v["redacted"] = json!(true);
                }
                v
            })
            .collect();
        let ctx = failure.as_deref().map(context).unwrap_or(Value::Null);
        let bundle = json!({
            "format": FORMAT,
            "program": s.meta,
            "operations": ops,
            "incomplete": s.incomplete,
            "limitations": s.limitations,
            "outcome": { "exit": code, "stdout": s.stdout, "failure": failure, "context": ctx },
        });
        let text = serde_json::to_string_pretty(&bundle).unwrap_or_default();
        match std::fs::write(&s.path, text + "\n") {
            Ok(()) => {
                eprintln!("recorded {} operations to {}", s.ops.len(), s.path.display());
                for l in &s.limitations {
                    eprintln!("  note: {l}");
                }
                if s.incomplete {
                    eprintln!("  note: the bundle is incomplete (size limit reached)");
                }
            }
            Err(e) => eprintln!("error: cannot write the bundle {}: {e}", s.path.display()),
        }
        return code;
    }
    // a replay: compare with the recording
    let unused: Vec<String> = s
        .ops
        .iter()
        .zip(&s.used)
        .filter(|(_, u)| !**u)
        .map(|(o, _)| format!("`{} {}`", o.kind, o.key))
        .collect();
    let in_order = s.order.windows(2).all(|w| w[0] < w[1]);
    let e = &s.expected;
    let (want_code, want_out, want_fail) =
        (e["exit"].as_i64().unwrap_or(0) as i32, e["stdout"].as_str().unwrap_or(""), e["failure"].as_str());
    let same = code == want_code && s.stdout == want_out && failure.as_deref() == want_fail;
    let label = if s.compare { "comparison" } else { "replay" };
    let used = s.order.len();
    if !unused.is_empty() && !s.compare {
        eprintln!(
            "replay diverged: the run ended without {} recorded operation(s), first {}",
            unused.len(),
            unused[0]
        );
        return DIVERGED;
    }
    eprintln!("{label}: {used} recorded operation(s) answered from the bundle");
    for n in &s.notes {
        eprintln!("  note: {n}");
    }
    if !unused.is_empty() {
        eprintln!("  note: {} recorded operation(s) not performed, first {}", unused.len(), unused[0]);
    }
    if !in_order {
        eprintln!("  note: operations ran in a different order than recorded");
    }
    if same {
        match want_fail {
            Some(f) => eprintln!("{label}: reproduced the recorded outcome (exit {code}): {f}"),
            None => eprintln!("{label}: reproduced the recorded outcome (exit {code})"),
        }
    } else {
        eprintln!("{label}: the outcome differs from the recording");
        if code != want_code {
            eprintln!("  exit {code} (recorded {want_code})");
        }
        if failure.as_deref() != want_fail {
            eprintln!(
                "  failure: {} (recorded: {})",
                failure.as_deref().unwrap_or("none"),
                want_fail.unwrap_or("none")
            );
        }
        if s.stdout != want_out {
            let line = s.stdout.lines().zip(want_out.lines()).take_while(|(a, b)| a == b).count() + 1;
            eprintln!("  output differs from line {line}");
        }
    }
    if s.compare {
        code
    } else if same {
        0
    } else {
        1
    }
}
