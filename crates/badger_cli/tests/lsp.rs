//! `badger lsp` over its stdio protocol: diagnostics follow edits, and hover,
//! definition, references, rename, symbols, completion, and formatting
//! answer from the checker.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

struct Client {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    notes: Vec<Value>,
    next: i64,
}

impl Client {
    fn start() -> Client {
        let mut child = Command::new(env!("CARGO_BIN_EXE_badger"))
            .arg("lsp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Client { child, stdin, stdout, notes: vec![], next: 1 }
    }

    fn send(&mut self, v: Value) {
        let b = v.to_string();
        write!(self.stdin, "Content-Length: {}\r\n\r\n{b}", b.len()).unwrap();
        self.stdin.flush().unwrap();
    }

    fn recv(&mut self) -> Value {
        let mut len = 0;
        loop {
            let mut line = String::new();
            self.stdout.read_line(&mut line).unwrap();
            let line = line.trim();
            if line.is_empty() {
                break;
            }
            if let Some(v) = line.strip_prefix("Content-Length:") {
                len = v.trim().parse().unwrap();
            }
        }
        let mut buf = vec![0; len];
        self.stdout.read_exact(&mut buf).unwrap();
        serde_json::from_slice(&buf).unwrap()
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let m = self.recv();
            if m["id"] == json!(id) {
                return m;
            }
            self.notes.push(m);
        }
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    /// The latest diagnostics published for `uri` (waiting for one more
    /// publication with a round trip).
    fn diagnostics(&mut self, uri: &str) -> Vec<Value> {
        self.request("workspace/symbol", json!({ "query": "" }));
        self.notes
            .iter()
            .rev()
            .find(|n| n["method"] == "textDocument/publishDiagnostics" && n["params"]["uri"] == uri)
            .map(|n| n["params"]["diagnostics"].as_array().unwrap().clone())
            .unwrap_or_default()
    }
}

const SRC: &str = "import std.io

pub struct Point { pub x: Int, pub y: Int }

fn norm(p: Point) -> Int {
    p.x * p.x + p.y * p.y
}

pub fn main() -> Result<Unit, io.Error> {
    let origin: Point = { x: 3, y: 4 }
    let total = norm(origin) + 1
    io.println($\"total {total}\")?
    ok(())
}
";

fn pos(uri: &str, line: u32, character: u32) -> Value {
    json!({ "textDocument": { "uri": uri }, "position": { "line": line, "character": character } })
}

#[test]
fn language_server() {
    let dir = std::env::temp_dir().join(format!("badger-lsp-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("demo.bdg");
    std::fs::write(&file, SRC).unwrap();
    let uri = format!("file://{}", std::fs::canonicalize(&file).unwrap().display());
    let mut c = Client::start();
    let init = c.request("initialize", json!({ "capabilities": {} }));
    assert_eq!(init["result"]["capabilities"]["hoverProvider"], json!(true));
    c.notify("initialized", json!({}));
    c.notify(
        "textDocument/didOpen",
        json!({ "textDocument": { "uri": uri, "languageId": "badger", "version": 1, "text": SRC } }),
    );
    assert!(c.diagnostics(&uri).is_empty());

    // hover: a call's signature, a local's type
    let h = c.request("textDocument/hover", pos(&uri, 10, 17));
    assert_eq!(h["result"]["contents"]["value"], "```badger\nnorm: fn(Point) -> Int\n```");
    let h = c.request("textDocument/hover", pos(&uri, 10, 24));
    assert_eq!(h["result"]["contents"]["value"], "```badger\norigin: Point\n```");
    let h = c.request("textDocument/hover", pos(&uri, 11, 25));
    assert_eq!(h["result"]["contents"]["value"], "```badger\ntotal: Int\n```");

    // definition: of a function, and of a type used in a signature
    let d = c.request("textDocument/definition", pos(&uri, 10, 17));
    assert_eq!(d["result"]["range"]["start"], json!({ "line": 4, "character": 3 }));
    let d = c.request("textDocument/definition", pos(&uri, 4, 12));
    assert_eq!(d["result"]["range"]["start"], json!({ "line": 2, "character": 11 }));

    // references and rename of a local
    let r = c.request("textDocument/references", pos(&uri, 9, 9));
    let starts: Vec<Value> =
        r["result"].as_array().unwrap().iter().map(|l| l["range"]["start"].clone()).collect();
    assert_eq!(starts, vec![json!({ "line": 9, "character": 8 }), json!({ "line": 10, "character": 21 })]);
    let rn = c.request("textDocument/rename", json!({ "textDocument": { "uri": uri }, "position": { "line": 4, "character": 4 }, "newName": "size" }));
    let edits = rn["result"]["changes"][&uri].as_array().unwrap();
    assert_eq!(edits.len(), 2);
    assert!(edits.iter().all(|e| e["newText"] == "size"));
    // library definitions are not renamed
    let pr = c.request("textDocument/prepareRename", pos(&uri, 11, 8));
    assert_eq!(pr["result"], Value::Null);

    // symbols
    let s = c.request("textDocument/documentSymbol", json!({ "textDocument": { "uri": uri } }));
    let names: Vec<&str> =
        s["result"].as_array().unwrap().iter().map(|x| x["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["Point", "norm", "main"]);

    // an edit with a type error publishes it; fixing it clears it
    let bad = SRC.replace("norm(origin)", "norm(origin, 2)");
    c.notify(
        "textDocument/didChange",
        json!({ "textDocument": { "uri": uri, "version": 2 }, "contentChanges": [{ "text": bad }] }),
    );
    let ds = c.diagnostics(&uri);
    assert_eq!(ds.len(), 1, "{ds:?}");
    assert_eq!(ds[0]["code"], "E0309");
    assert_eq!(ds[0]["range"]["start"]["line"], 10);
    // the structured facts travel in `data` too
    assert_eq!(ds[0]["data"]["schema_version"], 1, "{ds:?}");
    assert!(ds[0]["data"].get("expected").is_some() && ds[0]["data"]["label"].is_string(), "{ds:?}");
    c.notify(
        "textDocument/didChange",
        json!({ "textDocument": { "uri": uri, "version": 3 }, "contentChanges": [{ "text": SRC }] }),
    );
    assert!(c.diagnostics(&uri).is_empty());

    // completion: members after `p.` (while the text does not parse), and
    // module members after `io.`
    let typing = SRC.replace("    p.x * p.x + p.y * p.y\n", "    p.x * p.x + p.y * p.y\n    p.\n");
    c.notify(
        "textDocument/didChange",
        json!({ "textDocument": { "uri": uri, "version": 4 }, "contentChanges": [{ "text": typing }] }),
    );
    let cm = c.request("textDocument/completion", pos(&uri, 6, 6));
    let labels: Vec<&str> =
        cm["result"].as_array().unwrap().iter().map(|x| x["label"].as_str().unwrap()).collect();
    assert_eq!(labels, vec!["x", "y"]);
    let typing = SRC.replace("    ok(())\n", "    io.pri\n    ok(())\n");
    c.notify(
        "textDocument/didChange",
        json!({ "textDocument": { "uri": uri, "version": 5 }, "contentChanges": [{ "text": typing }] }),
    );
    let cm = c.request("textDocument/completion", pos(&uri, 12, 10));
    let labels: Vec<&str> =
        cm["result"].as_array().unwrap().iter().map(|x| x["label"].as_str().unwrap()).collect();
    assert!(labels.contains(&"println") && labels.contains(&"print"), "{labels:?}");

    // formatting returns the canonical layout
    let messy = SRC.replace("    let total", "        let total");
    c.notify(
        "textDocument/didChange",
        json!({ "textDocument": { "uri": uri, "version": 6 }, "contentChanges": [{ "text": messy }] }),
    );
    let f = c.request(
        "textDocument/formatting",
        json!({ "textDocument": { "uri": uri }, "options": { "tabSize": 4, "insertSpaces": true } }),
    );
    assert_eq!(f["result"][0]["newText"], SRC);

    c.request("shutdown", Value::Null);
    c.notify("exit", Value::Null);
    assert!(c.child.wait().unwrap().success());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Hostile and unusual input does not stop the server: bad framing and
/// JSON are skipped, an inverted edit range replaces nothing, completion
/// next to multi-byte text works, and renames keep the program valid
/// (qualified uses rename their last segment, record shorthand keeps its
/// label, invalid names are refused).
#[test]
fn language_server_robustness() {
    let dir = std::env::temp_dir().join(format!("badger-lsp-robust-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("r.bdg");
    let src = "import std.io\n\nfn make(name: String, n: Int) -> { name: String, n: Int } {\n    { name, n }\n}\n\n// a→b\npub fn main() -> Unit {\n    let r = make(\"x\", 1)\n    let _ = io.println(r.name)\n}\n";
    std::fs::write(&file, src).unwrap();
    let uri = format!("file://{}", std::fs::canonicalize(&file).unwrap().display());
    let mut c = Client::start();
    // garbage first: no length, bad JSON, an oversized length marker
    write!(c.stdin, "X-Junk: 1\r\n\r\n").unwrap();
    write!(c.stdin, "Content-Length: 5\r\n\r\n{{oops").unwrap();
    c.stdin.flush().unwrap();
    c.request("initialize", json!({ "capabilities": {} }));
    c.notify(
        "textDocument/didOpen",
        json!({ "textDocument": { "uri": uri, "languageId": "badger", "version": 1, "text": src } }),
    );
    assert!(c.diagnostics(&uri).is_empty());
    // completion right after a multi-byte character
    let cm = c.request("textDocument/completion", pos(&uri, 6, 7));
    assert!(cm["error"].is_null(), "{cm}");
    // an inverted range in an incremental edit
    c.notify(
        "textDocument/didChange",
        json!({ "textDocument": { "uri": uri, "version": 2 }, "contentChanges": [{ "range": { "start": { "line": 3, "character": 5 }, "end": { "line": 3, "character": 1 } }, "text": "" }] }),
    );
    assert!(c.diagnostics(&uri).is_empty());
    // renaming a parameter used as a record shorthand keeps the field label
    let rn = c.request("textDocument/rename", json!({ "textDocument": { "uri": uri }, "position": { "line": 2, "character": 9 }, "newName": "label" }));
    let edits = rn["result"]["changes"][&uri].as_array().unwrap().clone();
    let texts: Vec<&str> = edits.iter().map(|e| e["newText"].as_str().unwrap()).collect();
    assert!(texts.contains(&"label") && texts.contains(&"name: label"), "{rn}");
    // invalid replacement names are refused
    for bad in ["fn", "9x", "Label"] {
        let rn = c.request("textDocument/rename", json!({ "textDocument": { "uri": uri }, "position": { "line": 2, "character": 9 }, "newName": bad }));
        assert!(!rn["error"].is_null(), "{bad}: {rn}");
    }
    c.request("shutdown", Value::Null);
    c.notify("exit", Value::Null);
    assert!(c.child.wait().unwrap().success());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A project's other entry points count: references and a rename from a
/// module reachable only from a `[tasks]` function cover the main entry's
/// modules too, and the other way around.
#[test]
fn language_server_project_entries() {
    let dir =
        std::fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/projects/overlay"))
            .unwrap();
    let jobs = dir.join("src/jobs.bdg");
    let shapes = dir.join("src/shapes.bdg");
    let uri = |p: &Path| format!("file://{}", p.display());
    let mut c = Client::start();
    c.request("initialize", json!({ "capabilities": {} }));
    let src = std::fs::read_to_string(&jobs).unwrap();
    c.notify(
        "textDocument/didOpen",
        json!({ "textDocument": { "uri": uri(&jobs), "languageId": "badger", "version": 1, "text": src } }),
    );
    assert!(c.diagnostics(&uri(&jobs)).is_empty());
    // `shapes.area(1, 1)` in the task module
    let rn = c.request(
        "textDocument/rename",
        pos(&uri(&jobs), 4, 39)
            .as_object()
            .cloned()
            .map(|mut o| {
                o.insert("newName".into(), json!("surface"));
                Value::Object(o)
            })
            .unwrap(),
    );
    let changes = rn["result"]["changes"].as_object().unwrap();
    let mut files: Vec<&str> = changes.keys().map(|k| k.rsplit('/').next().unwrap()).collect();
    files.sort();
    assert_eq!(files, vec!["jobs.bdg", "report.bdg", "shapes.bdg"], "{rn}");
    let refs = c.request("textDocument/references", pos(&uri(&shapes), 2, 7));
    let n = refs["result"].as_array().map(|v| v.len()).unwrap_or(0);
    assert_eq!(n, 3, "{refs}");
    c.request("shutdown", Value::Null);
    c.notify("exit", Value::Null);
    assert!(c.child.wait().unwrap().success());
}

/// A quick fix inserts the import of a standard module used without one.
#[test]
fn code_action_inserts_import() {
    let dir = std::env::temp_dir().join(format!("badger-lsp-action-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("demo.bdg");
    let src = "import std.io\n\npub fn main() -> Unit {\n    let t = text.trim(\" a \")\n}\n";
    std::fs::write(&file, src).unwrap();
    let uri = format!("file://{}", std::fs::canonicalize(&file).unwrap().display());
    let mut c = Client::start();
    c.request("initialize", json!({ "capabilities": {} }));
    c.notify("initialized", json!({}));
    c.notify(
        "textDocument/didOpen",
        json!({ "textDocument": { "uri": uri, "languageId": "badger", "version": 1, "text": src } }),
    );
    let ds = c.diagnostics(&uri);
    assert_eq!(ds.len(), 1, "{ds:?}");
    let r = c.request(
        "textDocument/codeAction",
        json!({ "textDocument": { "uri": uri }, "range": ds[0]["range"], "context": { "diagnostics": ds } }),
    );
    let actions = r["result"].as_array().unwrap();
    assert_eq!(actions.len(), 1, "{r}");
    assert_eq!(actions[0]["title"], "Import std.text");
    let edit = &actions[0]["edit"]["changes"][&uri][0];
    assert_eq!(edit["newText"], "import std.text\n");
    assert_eq!(edit["range"]["start"]["line"], 1, "after the last import");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A quick fix adds the arm a non-exhaustive `match` misses, with a body
/// that panics until it is written; the result checks.
#[test]
fn code_action_adds_missing_arm() {
    let dir = std::env::temp_dir().join(format!("badger-lsp-arm-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("demo.bdg");
    let src = "enum Shape {\n    circle(r: Int),\n    square(s: Int),\n    point,\n}\n\nfn size(s: Shape) -> Int {\n    match s {\n        .circle(r) => r,\n        .point => 0,\n    }\n}\n\npub fn main() -> Unit {}\n";
    std::fs::write(&file, src).unwrap();
    let uri = format!("file://{}", std::fs::canonicalize(&file).unwrap().display());
    let mut c = Client::start();
    c.request("initialize", json!({ "capabilities": {} }));
    c.notify("initialized", json!({}));
    c.notify(
        "textDocument/didOpen",
        json!({ "textDocument": { "uri": uri, "languageId": "badger", "version": 1, "text": src } }),
    );
    let ds = c.diagnostics(&uri);
    assert_eq!(ds.len(), 1, "{ds:?}");
    let r = c.request(
        "textDocument/codeAction",
        json!({ "textDocument": { "uri": uri }, "range": ds[0]["range"], "context": { "diagnostics": ds } }),
    );
    let edit = &r["result"][0]["edit"]["changes"][&uri][0];
    assert_eq!(edit["newText"], "        .square(_) => panic(\"not handled yet: .square(_)\"),\n", "{r}");
    // apply it: the program checks
    let line = edit["range"]["start"]["line"].as_u64().unwrap() as usize;
    let mut lines: Vec<String> = src.lines().map(str::to_string).collect();
    lines.insert(line, edit["newText"].as_str().unwrap().trim_end_matches('\n').to_string());
    let fixed = lines.join("\n") + "\n";
    c.notify(
        "textDocument/didChange",
        json!({ "textDocument": { "uri": uri, "version": 2 }, "contentChanges": [{ "text": fixed }] }),
    );
    assert!(c.diagnostics(&uri).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Quick fixes for common mistakes: a near-miss name is replaced by the
/// suggestion, and an ignored `Result` is propagated with `?` or discarded
/// with `let _ =`.
#[test]
fn code_actions_common_fixes() {
    let dir = std::env::temp_dir().join(format!("badger-lsp-fixes-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("demo.bdg");
    let src = "import std.io\n\nfn total(items: List<Int>) -> Int => items.len()\n\npub fn main() -> Result<Unit, io.Error> {\n    let n = totl([1])\n    io.println(\"x\")\n    ok(())\n}\n";
    std::fs::write(&file, src).unwrap();
    let uri = format!("file://{}", std::fs::canonicalize(&file).unwrap().display());
    let mut c = Client::start();
    c.request("initialize", json!({ "capabilities": {} }));
    c.notify("initialized", json!({}));
    c.notify(
        "textDocument/didOpen",
        json!({ "textDocument": { "uri": uri, "languageId": "badger", "version": 1, "text": src } }),
    );
    let ds = c.diagnostics(&uri);
    let r = c.request(
        "textDocument/codeAction",
        json!({ "textDocument": { "uri": uri }, "range": ds[0]["range"], "context": { "diagnostics": ds } }),
    );
    let titles: Vec<String> =
        r["result"].as_array().unwrap().iter().map(|a| a["title"].as_str().unwrap().to_string()).collect();
    assert!(titles.contains(&"Change to `total`".to_string()), "{r}");
    // fixed name: the ignored result remains, with its two fixes
    let fixed = src.replace("totl(", "total(");
    c.notify(
        "textDocument/didChange",
        json!({ "textDocument": { "uri": uri, "version": 2 }, "contentChanges": [{ "text": fixed }] }),
    );
    let ds = c.diagnostics(&uri);
    let r = c.request(
        "textDocument/codeAction",
        json!({ "textDocument": { "uri": uri }, "range": ds[0]["range"], "context": { "diagnostics": ds } }),
    );
    let actions = r["result"].as_array().unwrap();
    let titles: Vec<&str> = actions.iter().map(|a| a["title"].as_str().unwrap()).collect();
    assert_eq!(titles, vec!["Propagate the error with `?`", "Discard the result with `let _ =`"], "{r}");
    assert_eq!(actions[0]["edit"]["changes"][&uri][0]["newText"], "?");
    // where `?` would not check (the function returns `Int`), only
    // discarding is offered
    let plain = "import std.io\n\nfn count() -> Int {\n    io.println(\"x\")\n    3\n}\n\npub fn main() -> Unit { let _ = count() }\n";
    c.notify(
        "textDocument/didChange",
        json!({ "textDocument": { "uri": uri, "version": 3 }, "contentChanges": [{ "text": plain }] }),
    );
    let ds = c.diagnostics(&uri);
    let r = c.request(
        "textDocument/codeAction",
        json!({ "textDocument": { "uri": uri }, "range": ds[0]["range"], "context": { "diagnostics": ds } }),
    );
    let titles: Vec<&str> =
        r["result"].as_array().unwrap().iter().map(|a| a["title"].as_str().unwrap()).collect();
    assert_eq!(titles, vec!["Discard the result with `let _ =`"], "{r}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The missing arm goes before the match's own closing brace, whatever
/// braces its arms hold in chars and comments.
#[test]
fn code_action_missing_arm_skips_braces_in_chars_and_comments() {
    let dir = std::env::temp_dir().join(format!("badger-lsp-arm-chars-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("demo.bdg");
    let src = "enum Tok { open, close, word(s: String) }\nfn show(t: Tok) -> Char {\n    match t {\n        // a } in a comment\n        .open => '{',\n        .close => '}',\n    }\n}\npub fn main() -> Unit {}\n";
    std::fs::write(&file, src).unwrap();
    let uri = format!("file://{}", std::fs::canonicalize(&file).unwrap().display());
    let mut c = Client::start();
    c.request("initialize", json!({ "capabilities": {} }));
    c.notify("initialized", json!({}));
    c.notify(
        "textDocument/didOpen",
        json!({ "textDocument": { "uri": uri, "languageId": "badger", "version": 1, "text": src } }),
    );
    let ds = c.diagnostics(&uri);
    let r = c.request(
        "textDocument/codeAction",
        json!({ "textDocument": { "uri": uri }, "range": ds[0]["range"], "context": { "diagnostics": ds } }),
    );
    let edit = &r["result"][0]["edit"]["changes"][&uri][0];
    assert_eq!(edit["range"]["start"], json!({ "line": 6, "character": 0 }), "{r}");
    assert_eq!(edit["newText"], "        .word(_) => panic(\"not handled yet: .word(_)\"),\n", "{r}");
    let _ = std::fs::remove_dir_all(&dir);
}
