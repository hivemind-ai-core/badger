//! `lang lsp`: a language server over stdio (JSON-RPC with
//! `Content-Length` framing). Each change re-checks the program the
//! document belongs to (its project, or the file alone) with every open
//! buffer overlaid, using the same checker and diagnostics as `lang check`
//! (`lang_driver::analysis`). Bursts of edits are coalesced into one check.
//!
//! Features: diagnostics, hover, definition, references, document
//! highlights, rename, document and workspace symbols, formatting, and
//! completion (names in scope, module members after `module.`, fields and
//! methods after `value.`). Positions are UTF-16 code units, as LSP
//! requires by default.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use lang_driver::analysis::{self, Analysis, FileIndex, Item, Key, SymbolInfo, SymbolKind};
use serde_json::{Value, json};

const KEYWORDS: &[&str] = &[
    "fn", "let", "var", "const", "if", "else", "match", "for", "in", "while", "loop", "break", "continue",
    "return", "struct", "enum", "protocol", "impl", "import", "pub", "type", "where", "derive", "true",
    "false", "unsafe", "extern", "dyn", "once", "system", "pipeline",
];

fn read_message(r: &mut impl BufRead) -> Option<Value> {
    let mut len = None;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(v) = line.strip_prefix("Content-Length:") {
            len = v.trim().parse::<usize>().ok();
        }
    }
    let mut buf = vec![0; len?];
    r.read_exact(&mut buf).ok()?;
    serde_json::from_slice(&buf).ok()
}

fn send(out: &mut impl Write, v: &Value) {
    let body = v.to_string();
    let _ = write!(out, "Content-Length: {}\r\n\r\n{body}", body.len());
    let _ = out.flush();
}

/// `file:///a/b%20c.lang` -> `/a/b c.lang`.
fn uri_to_path(uri: &str) -> PathBuf {
    let p = uri.strip_prefix("file://").unwrap_or(uri);
    let bytes = p.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(b) = u8::from_str_radix(&p[i + 1..i + 3], 16)
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    PathBuf::from(String::from_utf8_lossy(&out).into_owned())
}

fn path_to_uri(p: &str) -> String {
    let abs = std::fs::canonicalize(p).unwrap_or_else(|_| PathBuf::from(p));
    let mut s = String::from("file://");
    for b in abs.to_string_lossy().bytes() {
        if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) {
            s.push(b as char);
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
    s
}

/// Byte offset -> LSP position (line, UTF-16 column).
fn position(text: &str, offset: usize) -> Value {
    let offset = offset.min(text.len());
    let before = &text[..offset];
    let line = before.matches('\n').count();
    let start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let col: usize = text[start..offset].chars().map(char::len_utf16).sum();
    json!({ "line": line, "character": col })
}

/// LSP position -> byte offset (clamped to the line).
fn offset(text: &str, pos: &Value) -> usize {
    let line = pos["line"].as_u64().unwrap_or(0) as usize;
    let col = pos["character"].as_u64().unwrap_or(0) as usize;
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
        if units >= col {
            return start + i;
        }
        units += c.len_utf16();
    }
    end
}

fn range(text: &str, start: usize, end: usize) -> Value {
    json!({ "start": position(text, start), "end": position(text, end) })
}

fn symbol_kind(k: SymbolKind) -> u32 {
    match k {
        SymbolKind::Function => 12,
        SymbolKind::Method => 6,
        SymbolKind::Struct => 23,
        SymbolKind::Enum => 10,
        SymbolKind::Protocol => 11,
        SymbolKind::Type => 26,
        SymbolKind::Value => 13,
        SymbolKind::Impl => 5,
    }
}

fn completion_kind(k: SymbolKind) -> u32 {
    match k {
        SymbolKind::Function => 3,
        SymbolKind::Method => 2,
        SymbolKind::Struct => 22,
        SymbolKind::Enum => 13,
        SymbolKind::Protocol => 8,
        SymbolKind::Type => 7,
        SymbolKind::Value => 6,
        SymbolKind::Impl => 9,
    }
}

/// The program a document belongs to: the nearest enclosing project
/// directory (with `lang.toml`), else the file itself.
fn program_of(path: &Path) -> PathBuf {
    let mut dir = path.parent();
    while let Some(d) = dir {
        if d.join("lang.toml").exists() {
            return d.to_path_buf();
        }
        dir = d.parent();
    }
    path.to_path_buf()
}

fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// A file's index, the program's module members, and the file's own items.
type Indexed = (FileIndex, HashMap<String, Vec<Item>>, Vec<Item>);

struct Server {
    /// Open documents by canonical path.
    docs: HashMap<PathBuf, String>,
    /// The latest analysis of each program, by program path.
    programs: HashMap<PathBuf, Analysis>,
    /// The last index of each file that checked far enough to have one
    /// (completion keeps working while the current text does not parse).
    last_good: HashMap<PathBuf, Indexed>,
    /// Files that were sent diagnostics (to clear them when fixed).
    published: HashSet<String>,
    dirty: HashSet<PathBuf>,
}

impl Server {
    fn analysis_for(&mut self, file: &Path) -> Option<&Analysis> {
        let prog = self.program_path(file);
        self.programs.get(&prog)
    }

    fn program_path(&self, file: &Path) -> PathBuf {
        let p = program_of(file);
        // a file outside its project's module graph is analysed alone
        if p != file
            && let Some(a) = self.programs.get(&p)
            && a.file(&file.display().to_string()).is_none()
            && a.files.iter().all(|f| canonical(Path::new(&f.path)) != file)
        {
            return file.to_path_buf();
        }
        p
    }

    fn reanalyze(&mut self, out: &mut impl Write) {
        let dirty: Vec<PathBuf> = self.dirty.drain().collect();
        let mut done: HashSet<PathBuf> = HashSet::new();
        for file in dirty {
            let mut prog = program_of(&file);
            if !done.insert(prog.clone()) {
                continue;
            }
            let mut a = analysis::analyze(&prog, self.docs.clone());
            if prog != file && !a.files.iter().any(|f| canonical(Path::new(&f.path)) == file) {
                // not reachable from the project's entry: a module on its own
                prog = file.clone();
                a = analysis::analyze(&prog, self.docs.clone());
            }
            self.publish(&a, out);
            for f in &a.files {
                if !f.refs.is_empty() || !f.symbols.is_empty() {
                    let globals = a.globals.get(&f.path).cloned().unwrap_or_default();
                    self.last_good
                        .insert(canonical(Path::new(&f.path)), (f.clone(), a.modules.clone(), globals));
                }
            }
            self.programs.insert(prog, a);
        }
    }

    fn publish(&mut self, a: &Analysis, out: &mut impl Write) {
        let mut by_file: HashMap<String, Vec<Value>> = HashMap::new();
        let text_of = |p: &str| {
            a.file(p).map(|f| f.text.clone()).or_else(|| std::fs::read_to_string(p).ok()).unwrap_or_default()
        };
        for f in &a.files {
            by_file.entry(f.path.clone()).or_default();
        }
        for d in &a.diagnostics {
            if d.path.starts_with('<') {
                continue;
            }
            let text = text_of(&d.path);
            let mut message = d.message.clone();
            for n in &d.notes {
                message.push('\n');
                message.push_str(n);
            }
            let related: Vec<Value> = d
                .related
                .iter()
                .map(|(l, m)| {
                    json!({
                        "location": { "uri": path_to_uri(&l.path), "range": range(&text_of(&l.path), l.start, l.end) },
                        "message": if m.is_empty() { "related" } else { m.as_str() },
                    })
                })
                .collect();
            let mut v = json!({
                "range": range(&text, d.start, d.end),
                "severity": match d.severity { lang_base::Severity::Error => 1, lang_base::Severity::Warning => 2, lang_base::Severity::Note => 3 },
                "source": "lang",
                "message": message,
                "relatedInformation": related,
            });
            if let Some(c) = &d.code {
                v["code"] = json!(c);
            }
            by_file.entry(d.path.clone()).or_default().push(v);
        }
        for p in self.published.clone() {
            if !by_file.contains_key(&p) && a.files.iter().any(|f| f.path == p) {
                by_file.insert(p, vec![]);
            }
        }
        for (p, ds) in by_file {
            if ds.is_empty() {
                self.published.remove(&p);
            } else {
                self.published.insert(p.clone());
            }
            send(
                out,
                &json!({
                    "jsonrpc": "2.0",
                    "method": "textDocument/publishDiagnostics",
                    "params": { "uri": path_to_uri(&p), "diagnostics": ds },
                }),
            );
        }
    }

    fn text(&self, file: &Path) -> String {
        self.docs.get(file).cloned().unwrap_or_else(|| std::fs::read_to_string(file).unwrap_or_default())
    }

    /// The analysed file entry for `file` and its byte offset of `pos`.
    fn locate(&mut self, params: &Value) -> Option<(PathBuf, String, usize)> {
        let file = canonical(&uri_to_path(params["textDocument"]["uri"].as_str()?));
        let text = self.text(&file);
        let off = offset(&text, &params["position"]);
        let a = self.analysis_for(&file)?;
        let f = a.files.iter().find(|f| canonical(Path::new(&f.path)) == file)?;
        Some((file, f.path.clone(), off))
    }

    fn hover(&mut self, params: &Value) -> Value {
        let Some((file, path, off)) = self.locate(params) else { return Value::Null };
        let text = self.text(&file);
        let Some(a) = self.analysis_for(&file) else { return Value::Null };
        match a.ref_at(&path, off) {
            Some(r) if !r.hover.is_empty() => json!({
                "contents": { "kind": "markdown", "value": format!("```lang\n{}\n```", r.hover) },
                "range": range(&text, r.start, r.end),
            }),
            _ => Value::Null,
        }
    }

    fn definition(&mut self, params: &Value) -> Value {
        let Some((file, path, off)) = self.locate(params) else { return Value::Null };
        let Some(a) = self.analysis_for(&file) else { return Value::Null };
        let Some(t) = a.ref_at(&path, off).and_then(|r| r.target.clone()) else { return Value::Null };
        let text = a
            .file(&t.path)
            .map(|f| f.text.clone())
            .unwrap_or_else(|| std::fs::read_to_string(&t.path).unwrap_or_default());
        json!({ "uri": path_to_uri(&t.path), "range": range(&text, t.start, t.end) })
    }

    /// The key under the cursor and every location referring to it.
    fn occurrences(&mut self, params: &Value) -> Option<(Key, Vec<analysis::Loc>)> {
        let (file, path, off) = self.locate(params)?;
        let a = self.analysis_for(&file)?;
        let key = a
            .file(&path)?
            .refs
            .iter()
            .filter(|r| r.start <= off && off <= r.end && r.key.is_some())
            .min_by_key(|r| r.end - r.start)?
            .key?;
        Some((key, a.references(key)))
    }

    fn loc_json(&mut self, file: &Path, l: &analysis::Loc) -> Value {
        let text =
            self.analysis_for(file).and_then(|a| a.file(&l.path)).map(|f| f.text.clone()).unwrap_or_default();
        json!({ "uri": path_to_uri(&l.path), "range": range(&text, l.start, l.end) })
    }

    fn references(&mut self, params: &Value) -> Value {
        let file = canonical(&uri_to_path(params["textDocument"]["uri"].as_str().unwrap_or("")));
        let Some((_, locs)) = self.occurrences(params) else { return json!([]) };
        let v: Vec<Value> = locs.iter().map(|l| self.loc_json(&file, l)).collect();
        json!(v)
    }

    fn highlights(&mut self, params: &Value) -> Value {
        let file = canonical(&uri_to_path(params["textDocument"]["uri"].as_str().unwrap_or("")));
        let Some((_, locs)) = self.occurrences(params) else { return json!([]) };
        let text = self.text(&file);
        let v: Vec<Value> = locs
            .iter()
            .filter(|l| canonical(Path::new(&l.path)) == file)
            .map(|l| json!({ "range": range(&text, l.start, l.end) }))
            .collect();
        json!(v)
    }

    /// Occurrences spelled exactly as the name (a path like `io.println`
    /// is not renamed as a whole).
    fn renamable(&mut self, params: &Value) -> Option<(String, Vec<analysis::Loc>)> {
        let (key, locs) = self.occurrences(params)?;
        let (file, path, _) = self.locate(params)?;
        let a = self.analysis_for(&file)?;
        let decl = locs.iter().find(|l| {
            a.file(&l.path).is_some_and(|f| {
                f.refs.iter().any(|r| {
                    r.start == l.start
                        && r.end == l.end
                        && r.target.as_ref().is_some_and(|t| t.start == l.start && t.path == l.path)
                })
            })
        })?;
        let name = a.file(&decl.path)?.text.get(decl.start..decl.end)?.to_string();
        let _ = (key, path);
        let locs: Vec<analysis::Loc> = locs
            .into_iter()
            .filter(|l| a.file(&l.path).is_some_and(|f| f.text.get(l.start..l.end) == Some(name.as_str())))
            .collect();
        Some((name, locs))
    }

    fn rename(&mut self, params: &Value) -> Result<Value, String> {
        let new = params["newName"].as_str().unwrap_or("");
        if new.is_empty() || !new.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(format!("`{new}` is not a name"));
        }
        let Some((_, locs)) = self.renamable(params) else { return Err("nothing to rename here".into()) };
        let file = canonical(&uri_to_path(params["textDocument"]["uri"].as_str().unwrap_or("")));
        let mut changes: HashMap<String, Vec<Value>> = HashMap::new();
        for l in &locs {
            let mut e = self.loc_json(&file, l);
            let uri = e["uri"].as_str().unwrap_or("").to_string();
            e["newText"] = json!(new);
            e.as_object_mut().map(|o| o.remove("uri"));
            changes.entry(uri).or_default().push(e);
        }
        Ok(json!({ "changes": changes }))
    }

    fn prepare_rename(&mut self, params: &Value) -> Value {
        let file = canonical(&uri_to_path(params["textDocument"]["uri"].as_str().unwrap_or("")));
        let Some((_, path, off)) = self.locate(params) else { return Value::Null };
        let Some((name, locs)) = self.renamable(params) else { return Value::Null };
        let text = self.text(&file);
        match locs.iter().find(|l| l.path == path && l.start <= off && off <= l.end) {
            Some(l) => json!({ "range": range(&text, l.start, l.end), "placeholder": name }),
            None => Value::Null,
        }
    }

    fn symbols(&mut self, params: &Value) -> Value {
        let file = canonical(&uri_to_path(params["textDocument"]["uri"].as_str().unwrap_or("")));
        let text = self.text(&file);
        let Some(a) = self.analysis_for(&file) else { return json!([]) };
        let Some(f) = a.files.iter().find(|f| canonical(Path::new(&f.path)) == file) else {
            return json!([]);
        };
        fn conv(text: &str, s: &SymbolInfo) -> Value {
            json!({
                "name": s.name,
                "detail": s.detail,
                "kind": symbol_kind(s.kind),
                "range": range(text, s.start, s.end),
                "selectionRange": range(text, s.name_start, s.name_end),
                "children": s.children.iter().map(|c| conv(text, c)).collect::<Vec<_>>(),
            })
        }
        json!(f.symbols.iter().map(|s| conv(&text, s)).collect::<Vec<_>>())
    }

    fn workspace_symbols(&mut self, params: &Value) -> Value {
        let q = params["query"].as_str().unwrap_or("").to_lowercase();
        let mut out = vec![];
        for a in self.programs.values() {
            for f in &a.files {
                let mut stack: Vec<&SymbolInfo> = f.symbols.iter().collect();
                while let Some(s) = stack.pop() {
                    stack.extend(s.children.iter());
                    if s.kind != SymbolKind::Impl && s.name.to_lowercase().contains(&q) {
                        out.push(json!({
                            "name": s.name,
                            "kind": symbol_kind(s.kind),
                            "location": { "uri": path_to_uri(&f.path), "range": range(&f.text, s.name_start, s.name_end) },
                        }));
                    }
                }
            }
        }
        json!(out)
    }

    fn format(&mut self, params: &Value) -> Value {
        let file = canonical(&uri_to_path(params["textDocument"]["uri"].as_str().unwrap_or("")));
        let text = self.text(&file);
        let formatted = lang_syntax::format::format(&text);
        if formatted == text {
            return json!([]);
        }
        json!([{ "range": range(&text, 0, text.len()), "newText": formatted }])
    }

    fn completion(&mut self, params: &Value) -> Value {
        let file = canonical(&uri_to_path(params["textDocument"]["uri"].as_str().unwrap_or("")));
        let text = self.text(&file);
        let off = offset(&text, &params["position"]);
        let line_start = text[..off].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let before = &text[line_start..off];
        let ident = |c: char| c.is_alphanumeric() || c == '_';
        let word_start = before.rfind(|c: char| !ident(c)).map(|i| i + 1).unwrap_or(0);
        let prefix = &before[word_start..];
        let Some((index, modules, globals)) = self.last_good.get(&file).cloned() else {
            return json!(KEYWORDS.iter().map(|k| json!({"label": k, "kind": 14})).collect::<Vec<_>>());
        };
        let item = |i: &Item| json!({ "label": i.name, "detail": i.detail, "kind": completion_kind(i.kind) });
        // `receiver.`: module members, or fields and methods of a local
        if before[..word_start].ends_with('.') {
            let recv_end = word_start - 1;
            let recv_start = before[..recv_end].rfind(|c: char| !ident(c)).map(|i| i + 1).unwrap_or(0);
            let recv = &before[recv_start..recv_end];
            if let Some(ms) = modules.get(recv)
                && !index.locals.iter().any(|l| l.name == recv)
            {
                return json!(ms.iter().filter(|m| m.name.starts_with(prefix)).map(item).collect::<Vec<_>>());
            }
            // the nearest declaration of that name before the cursor
            let local = index
                .locals
                .iter()
                .filter(|l| l.name == recv && l.start <= line_start + recv_start)
                .max_by_key(|l| l.start)
                .or_else(|| index.locals.iter().find(|l| l.name == recv));
            return match local {
                Some(l) => json!(
                    l.members.iter().filter(|m| m.name.starts_with(prefix)).map(item).collect::<Vec<_>>()
                ),
                None => json!([]),
            };
        }
        let mut seen = HashSet::new();
        let mut items = vec![];
        let mut locals: Vec<&analysis::LocalInfo> =
            index.locals.iter().filter(|l| l.start < off && l.scope.0 <= off && off <= l.scope.1).collect();
        locals.sort_by_key(|l| std::cmp::Reverse(l.start));
        for l in locals {
            if l.name.starts_with(prefix) && seen.insert(l.name.clone()) {
                items.push(json!({ "label": l.name, "detail": l.ty, "kind": 6 }));
            }
        }
        for g in &globals {
            if g.name.starts_with(prefix) && seen.insert(g.name.clone()) {
                items.push(item(g));
            }
        }
        // modules this file imports, by the name code uses
        let imported: Vec<&str> = text
            .lines()
            .filter_map(|l| l.trim().strip_prefix("import "))
            .map(|p| p.trim().rsplit('.').next().unwrap_or(p))
            .collect();
        for m in modules.keys() {
            if m.starts_with(prefix) && imported.contains(&m.as_str()) && seen.insert(m.clone()) {
                items.push(json!({ "label": m, "kind": 9 }));
            }
        }
        for k in KEYWORDS {
            if k.starts_with(prefix) && seen.insert(k.to_string()) {
                items.push(json!({ "label": k, "kind": 14 }));
            }
        }
        json!(items)
    }
}

/// Serve the protocol on stdin/stdout until `exit`.
pub fn serve() -> i32 {
    let (tx, rx) = mpsc::channel::<Value>();
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut r = stdin.lock();
        while let Some(m) = read_message(&mut r) {
            if tx.send(m).is_err() {
                break;
            }
        }
    });
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut server = Server {
        docs: HashMap::new(),
        programs: HashMap::new(),
        last_good: HashMap::new(),
        published: HashSet::new(),
        dirty: HashSet::new(),
    };
    let mut shutdown = false;
    while let Ok(first) = rx.recv() {
        // take everything already queued, apply edits, then check once
        let mut batch = vec![first];
        while let Ok(m) = rx.try_recv() {
            batch.push(m);
        }
        let mut requests = vec![];
        for m in batch {
            let method = m["method"].as_str().unwrap_or("").to_string();
            let params = &m["params"];
            match method.as_str() {
                "textDocument/didOpen" => {
                    let p = canonical(&uri_to_path(params["textDocument"]["uri"].as_str().unwrap_or("")));
                    server
                        .docs
                        .insert(p.clone(), params["textDocument"]["text"].as_str().unwrap_or("").to_string());
                    server.dirty.insert(p);
                }
                "textDocument/didChange" => {
                    let p = canonical(&uri_to_path(params["textDocument"]["uri"].as_str().unwrap_or("")));
                    if let Some(changes) = params["contentChanges"].as_array() {
                        for c in changes {
                            let text = server.docs.entry(p.clone()).or_default();
                            if c.get("range").is_some_and(|r| !r.is_null()) {
                                let (a, b) =
                                    (offset(text, &c["range"]["start"]), offset(text, &c["range"]["end"]));
                                text.replace_range(a..b, c["text"].as_str().unwrap_or(""));
                            } else {
                                *text = c["text"].as_str().unwrap_or("").to_string();
                            }
                        }
                    }
                    server.dirty.insert(p);
                }
                "textDocument/didSave" => {
                    let p = canonical(&uri_to_path(params["textDocument"]["uri"].as_str().unwrap_or("")));
                    server.dirty.insert(p);
                }
                "textDocument/didClose" => {
                    let p = canonical(&uri_to_path(params["textDocument"]["uri"].as_str().unwrap_or("")));
                    server.docs.remove(&p);
                    server.dirty.insert(p);
                }
                "exit" => return if shutdown { 0 } else { 1 },
                _ => requests.push(m),
            }
        }
        if !server.dirty.is_empty() {
            server.reanalyze(&mut out);
        }
        for m in requests {
            let Some(id) = m.get("id").cloned() else { continue };
            let params = &m["params"];
            let result: Result<Value, (i64, String)> = match m["method"].as_str().unwrap_or("") {
                "initialize" => Ok(json!({
                    "capabilities": {
                        "textDocumentSync": { "openClose": true, "change": 2, "save": true },
                        "hoverProvider": true,
                        "definitionProvider": true,
                        "referencesProvider": true,
                        "documentHighlightProvider": true,
                        "documentSymbolProvider": true,
                        "workspaceSymbolProvider": true,
                        "documentFormattingProvider": true,
                        "renameProvider": { "prepareProvider": true },
                        "completionProvider": { "triggerCharacters": ["."] },
                    },
                    "serverInfo": { "name": "lang", "version": env!("CARGO_PKG_VERSION") },
                })),
                "shutdown" => {
                    shutdown = true;
                    Ok(Value::Null)
                }
                "textDocument/hover" => Ok(server.hover(params)),
                "textDocument/definition" => Ok(server.definition(params)),
                "textDocument/references" => Ok(server.references(params)),
                "textDocument/documentHighlight" => Ok(server.highlights(params)),
                "textDocument/documentSymbol" => Ok(server.symbols(params)),
                "workspace/symbol" => Ok(server.workspace_symbols(params)),
                "textDocument/formatting" => Ok(server.format(params)),
                "textDocument/completion" => Ok(server.completion(params)),
                "textDocument/prepareRename" => Ok(server.prepare_rename(params)),
                "textDocument/rename" => server.rename(params).map_err(|e| (-32602, e)),
                other => Err((-32601, format!("method not found: {other}"))),
            };
            let reply = match result {
                Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
                Err((code, message)) => {
                    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
                }
            };
            send(&mut out, &reply);
        }
    }
    0
}

/// 1-based line and column (in characters) of byte `offset`.
fn line_col(text: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(text.len());
    let before = &text[..offset];
    let line = before.matches('\n').count() + 1;
    let start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
    (line, text[start..offset].chars().count() + 1)
}

/// LSP range JSON (0-based, UTF-16) -> `{file, line, col, end_line, end_col}`
/// (1-based, characters).
fn plain_range(file: &str, text: &str, r: &Value) -> Value {
    let a = offset(text, &r["start"]);
    let b = offset(text, &r["end"]);
    let (l, c) = line_col(text, a);
    let (el, ec) = line_col(text, b);
    json!({ "file": file, "line": l, "col": c, "end_line": el, "end_col": ec })
}

/// `lang query`: one question about a program, answered as JSON with
/// 1-based lines and columns (for scripts, agents, and `std.compiler`).
pub fn query(kind: &str, path: &Path, line: Option<usize>, col: Option<usize>) -> Result<Value, String> {
    if kind == "api" {
        return api(path);
    }
    let file = canonical(path);
    let mut server = Server {
        docs: HashMap::new(),
        programs: HashMap::new(),
        last_good: HashMap::new(),
        published: HashSet::new(),
        dirty: HashSet::from([file.clone()]),
    };
    server.reanalyze(&mut std::io::sink());
    let text_of = |p: &str| std::fs::read_to_string(p).unwrap_or_default();
    let uri = path_to_uri(&file.display().to_string());
    let text = text_of(&file.display().to_string());
    let at = || -> Result<Value, String> {
        let (Some(l), Some(c)) = (line, col) else { return Err(format!("`{kind}` needs --line and --col")) };
        // characters -> UTF-16 units on that line
        let line_text = text.lines().nth(l.saturating_sub(1)).unwrap_or("");
        let units: usize = line_text.chars().take(c.saturating_sub(1)).map(char::len_utf16).sum();
        Ok(
            json!({ "textDocument": { "uri": uri }, "position": { "line": l.saturating_sub(1), "character": units } }),
        )
    };
    let loc = |v: &Value| -> Value {
        let p = uri_to_path(v["uri"].as_str().unwrap_or("")).display().to_string();
        plain_range(&p, &text_of(&p), &v["range"])
    };
    Ok(match kind {
        "diagnostics" => {
            let Some(a) = server.analysis_for(&file) else { return Ok(json!([])) };
            let ds: Vec<Value> = a
                .diagnostics
                .iter()
                .map(|d| {
                    let t = a.file(&d.path).map(|f| f.text.clone()).unwrap_or_else(|| text_of(&d.path));
                    let (l, c) = line_col(&t, d.start);
                    let (el, ec) = line_col(&t, d.end);
                    json!({
                        "file": d.path, "line": l, "col": c, "end_line": el, "end_col": ec,
                        "severity": match d.severity { lang_base::Severity::Error => "error", lang_base::Severity::Warning => "warning", lang_base::Severity::Note => "note" },
                        "code": d.code, "message": d.message, "notes": d.notes,
                    })
                })
                .collect();
            json!(ds)
        }
        "hover" => {
            let h = server.hover(&at()?);
            match h["contents"]["value"].as_str() {
                Some(v) => json!({ "text": v.trim_start_matches("```lang\n").trim_end_matches("\n```") }),
                None => Value::Null,
            }
        }
        "definition" => {
            let d = server.definition(&at()?);
            if d.is_null() { d } else { loc(&d) }
        }
        "references" => json!(
            server
                .references(&at()?)
                .as_array()
                .map(|v| v.iter().map(loc).collect::<Vec<_>>())
                .unwrap_or_default()
        ),
        "completion" => {
            let c = server.completion(&at()?);
            json!(
                c.as_array()
                    .map(|v| v
                        .iter()
                        .map(|i| json!({ "name": i["label"], "detail": i["detail"] }))
                        .collect::<Vec<_>>())
                    .unwrap_or_default()
            )
        }
        "symbols" => {
            let Some(a) = server.analysis_for(&file) else { return Ok(json!([])) };
            let Some(f) = a.files.iter().find(|f| canonical(Path::new(&f.path)) == file) else {
                return Ok(json!([]));
            };
            let mut out = vec![];
            fn walk(s: &SymbolInfo, container: Option<&str>, text: &str, out: &mut Vec<Value>) {
                let (l, c) = line_col(text, s.name_start);
                let kind = format!("{:?}", s.kind).to_lowercase();
                out.push(json!({ "name": s.name, "kind": kind, "detail": s.detail, "line": l, "col": c, "container": container }));
                for ch in &s.children {
                    walk(ch, Some(&s.name), text, out);
                }
            }
            for s in &f.symbols {
                walk(s, None, &f.text, &mut out);
            }
            json!(out)
        }
        other => {
            return Err(format!(
                "unknown query `{other}` (diagnostics, hover, definition, references, completion, symbols)"
            ));
        }
    })
}

/// The public interface of a program's modules, or of a standard module
/// (`std.list`): each item's module, name, kind, signature (with effects
/// and contracts), and documentation.
fn api(target: &Path) -> Result<Value, String> {
    let t = target.display().to_string();
    let (a, wanted): (Analysis, Option<String>) = match t.strip_prefix("std.") {
        Some(m) => {
            if lang_driver::std_source(m).is_none() {
                return Err(format!("no standard module `{t}`"));
            }
            let dir = std::env::temp_dir().join(format!("lang-api-{}", std::process::id()));
            let _ = std::fs::create_dir_all(&dir);
            let f = dir.join("api.lang");
            std::fs::write(&f, format!("import std.{m}\npub fn main() -> Unit {{}}\n"))
                .map_err(|e| e.to_string())?;
            let a = analysis::analyze(&f, HashMap::new());
            let _ = std::fs::remove_dir_all(&dir);
            (a, Some(m.rsplit('.').next().unwrap_or(m).to_string()))
        }
        None => (analysis::analyze(&program_of(&canonical(target)), HashMap::new()), None),
    };
    let mut out = vec![];
    let item = |module: &str, i: &Item| {
        json!({
            "module": module, "name": i.name, "kind": format!("{:?}", i.kind).to_lowercase(),
            "signature": i.detail, "doc": i.doc,
        })
    };
    match wanted {
        Some(m) => {
            let items = a.modules.get(&m).ok_or(format!("`std.{m}` has no public items"))?;
            out.extend(items.iter().filter(|i| i.public).map(|i| item(&format!("std.{m}"), i)));
            for i in a.methods.get(&m).into_iter().flatten().filter(|i| i.public) {
                out.push(item(&format!("std.{m}"), i));
            }
        }
        None => {
            for f in &a.files {
                let module = f.path.clone();
                for i in a.globals.get(&f.path).into_iter().flatten().filter(|i| i.public) {
                    out.push(item(&module, i));
                }
                let stem =
                    std::path::Path::new(&f.path).file_stem().map(|x| x.to_string_lossy().into_owned());
                for i in stem.and_then(|st| a.methods.get(&st)).into_iter().flatten().filter(|i| i.public) {
                    out.push(item(&module, i));
                }
            }
        }
    }
    Ok(json!(out))
}
