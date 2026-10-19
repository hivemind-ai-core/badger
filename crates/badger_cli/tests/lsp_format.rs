//! `badger lsp` formatting: the server advertises `textDocument/formatting`
//! (so a client can offer format on save) and answers it with the standard
//! `TextEdit[]` over the whole document, turning it into the same canonical
//! layout `badger format` writes.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
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

    fn stop(mut self) {
        self.request("shutdown", Value::Null);
        self.notify("exit", Value::Null);
        assert!(self.child.wait().unwrap().success());
    }
}

/// A temporary directory of this test's own.
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("badger-lsp-format-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The canonical layout of `src`, as `badger format` writes it to a file.
fn canonical_form(src: &str) -> String {
    let dir = temp_dir("canon");
    let file = dir.join("c.bdg");
    std::fs::write(&file, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_badger")).arg("format").arg(&file).output().unwrap();
    assert!(out.status.success(), "badger format failed: {}", String::from_utf8_lossy(&out.stderr));
    let canon = std::fs::read_to_string(&file).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    canon
}

/// LSP position (line, UTF-16 column) -> byte offset in `text`.
fn offset_of(text: &str, pos: &Value) -> usize {
    let line = pos["line"].as_u64().unwrap_or(0) as usize;
    let column = pos["character"].as_u64().unwrap_or(0) as usize;
    let mut start = 0;
    for _ in 0..line {
        match text[start..].find('\n') {
            Some(i) => start += i + 1,
            None => return text.len(),
        }
    }
    let end = text[start..].find('\n').map(|i| start + i).unwrap_or(text.len());
    let mut units = 0;
    for (i, c) in text[start..end].char_indices() {
        if units >= column {
            return start + i;
        }
        units += c.len_utf16();
    }
    end
}

/// The position of the end of `text`.
fn end_of(text: &str) -> Value {
    let line = text.matches('\n').count();
    let last = text.rsplit('\n').next().unwrap_or("");
    let character: usize = last.chars().map(char::len_utf16).sum();
    json!({ "line": line, "character": character })
}

/// Apply a `TextEdit` (a range and a new text) to `text`.
fn apply_edit(text: &str, edit: &Value) -> String {
    let start = offset_of(text, &edit["range"]["start"]);
    let end = offset_of(text, &edit["range"]["end"]);
    let new = edit["newText"].as_str().unwrap_or("");
    let mut out = String::with_capacity(text.len() + new.len());
    out.push_str(&text[..start]);
    out.push_str(new);
    out.push_str(&text[end..]);
    out
}

/// A document that is not in the canonical layout: the indentation is wrong,
/// there are blank lines inside the body, the spacing is off, and the file
/// does not end with a newline.
const MESSY: &str = "import std.io

pub struct Point { pub x: Int, pub y: Int }

pub fn main() -> Result<Unit, io.Error> {
        let p: Point = { x: 3, y: 4 }

    let t = p.x * p.x + p.y * p.y
    io.println($\"total {t}\")?
    ok(())
}";

/// The canonical layout of [`MESSY`].
const CANON: &str = "import std.io

pub struct Point { pub x: Int, pub y: Int }

pub fn main() -> Result<Unit, io.Error> {
    let p: Point = { x: 3, y: 4 }

    let t = p.x * p.x + p.y * p.y
    io.println($\"total {t}\")?
    ok(())
}
";

fn uri_of(file: &Path) -> String {
    format!("file://{}", std::fs::canonicalize(file).unwrap().display())
}

/// The server advertises the formatting capability, and answers a
/// `textDocument/formatting` request with the standard `TextEdit[]`: one
/// edit over the whole document that turns it into the canonical layout
/// `badger format` writes.
#[test]
fn formatting_is_advertised_and_answers_with_text_edits() {
    let dir = temp_dir("edits");
    let file = dir.join("demo.bdg");
    std::fs::write(&file, MESSY).unwrap();
    let uri = uri_of(&file);
    let mut c = Client::start();

    let init = c.request("initialize", json!({ "capabilities": {} }));
    assert_eq!(init["result"]["capabilities"]["documentFormattingProvider"], json!(true), "{init}");
    c.notify("initialized", json!({}));

    c.notify(
        "textDocument/didOpen",
        json!({ "textDocument": { "uri": uri, "languageId": "badger", "version": 1, "text": MESSY } }),
    );
    assert!(c.diagnostics(&uri).is_empty());

    let f = c.request(
        "textDocument/formatting",
        json!({ "textDocument": { "uri": uri }, "options": { "tabSize": 4, "insertSpaces": true } }),
    );
    assert!(f["error"].is_null(), "{f}");
    let edits = f["result"].as_array().unwrap();
    assert_eq!(edits.len(), 1, "{f}");
    // the edit is a `TextEdit`: a range and a new text
    let edit = &edits[0];
    assert!(edit["range"]["start"].is_object() && edit["range"]["end"].is_object(), "{f}");
    assert!(edit["newText"].is_string(), "{f}");
    // the range covers the whole document
    assert_eq!(edit["range"]["start"], json!({ "line": 0, "character": 0 }), "{f}");
    assert_eq!(edit["range"]["end"], end_of(MESSY), "{f}");
    // applying it yields the canonical layout
    let formatted = apply_edit(MESSY, edit);
    assert_eq!(formatted, CANON, "{f}");
    // which is what `badger format` writes to the file
    assert_eq!(canonical_form(MESSY), CANON);

    // a document already in the canonical layout needs no edits
    c.notify(
        "textDocument/didChange",
        json!({ "textDocument": { "uri": uri, "version": 2 }, "contentChanges": [{ "text": CANON }] }),
    );
    let f = c.request(
        "textDocument/formatting",
        json!({ "textDocument": { "uri": uri }, "options": { "tabSize": 4, "insertSpaces": true } }),
    );
    assert_eq!(f["result"], json!([]), "{f}");

    c.stop();
    let _ = std::fs::remove_dir_all(&dir);
}

/// A save re-checks the document, but the server does not rewrite the
/// user's file: the client asks for the formatting, and applies the edits
/// the server answers with.
#[test]
fn save_does_not_reformat_the_file() {
    let dir = temp_dir("save");
    let file = dir.join("demo.bdg");
    std::fs::write(&file, MESSY).unwrap();
    let uri = uri_of(&file);
    let mut c = Client::start();
    c.request("initialize", json!({ "capabilities": {} }));
    c.notify("initialized", json!({}));
    c.notify(
        "textDocument/didOpen",
        json!({ "textDocument": { "uri": uri, "languageId": "badger", "version": 1, "text": MESSY } }),
    );
    assert!(c.diagnostics(&uri).is_empty());

    c.notify("textDocument/didSave", json!({ "textDocument": { "uri": uri } }));
    assert!(c.diagnostics(&uri).is_empty());
    // the file on disk is untouched: nothing is formatted behind the user's back
    assert_eq!(std::fs::read_to_string(&file).unwrap(), MESSY);

    // the client asks for the formatting, and gets the edit to apply
    let f = c.request(
        "textDocument/formatting",
        json!({ "textDocument": { "uri": uri }, "options": { "tabSize": 4, "insertSpaces": true } }),
    );
    let edit = &f["result"].as_array().unwrap()[0];
    assert_eq!(apply_edit(MESSY, edit), CANON, "{f}");
    // and the file on disk is still untouched: the client applies the edit
    assert_eq!(std::fs::read_to_string(&file).unwrap(), MESSY);

    c.stop();
    let _ = std::fs::remove_dir_all(&dir);
}
