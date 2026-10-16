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

/// The largest message accepted (bigger ones are skipped).
const MAX_MESSAGE: usize = 64 << 20;

/// The next message: `None` at the end of input, `Some(None)` for one that
/// cannot be used (no length, too large, not JSON), which is skipped.
fn read_message(r: &mut impl BufRead) -> Option<Option<Value>> {
    let mut len = None;
    loop {
        let mut line = Vec::new();
        if r.read_until(b'\n', &mut line).ok()? == 0 {
            return None;
        }
        let line = String::from_utf8_lossy(&line);
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':')
            && k.trim().eq_ignore_ascii_case("content-length")
        {
            len = v.trim().parse::<usize>().ok();
        }
    }
    let Some(len) = len else { return Some(None) };
    if len > MAX_MESSAGE {
        let _ = std::io::copy(&mut std::io::Read::take(&mut *r, len as u64), &mut std::io::sink());
        return Some(None);
    }
    let mut buf = vec![0; len];
    r.read_exact(&mut buf).ok()?;
    Some(serde_json::from_slice(&buf).ok())
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
            && let Ok(h) = std::str::from_utf8(&bytes[i + 1..i + 3])
            && let Ok(b) = u8::from_str_radix(h, 16)
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

/// The URI the client opened each file by (it may name a symlink).
static CLIENT_URIS: std::sync::Mutex<Option<HashMap<PathBuf, String>>> = std::sync::Mutex::new(None);

fn path_to_uri(p: &str) -> String {
    let abs = std::fs::canonicalize(p).unwrap_or_else(|_| PathBuf::from(p));
    if let Some(u) = CLIENT_URIS.lock().unwrap().as_ref().and_then(|m| m.get(&abs)) {
        return u.clone();
    }
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
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
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
    if path.is_dir() && path.join("lang.toml").exists() {
        return path.to_path_buf();
    }
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
    /// The program whose analysis covers each file.
    owner: HashMap<PathBuf, PathBuf>,
    dirty: HashSet<PathBuf>,
}

impl Server {
    fn analysis_for(&mut self, file: &Path) -> Option<&Analysis> {
        let prog = match self.owner.get(file) {
            Some(p) => p.clone(),
            None => self.program_path(file),
        };
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
            if prog != file
                && file.is_file()
                && !a.files.iter().any(|f| canonical(Path::new(&f.path)) == file)
            {
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
            for f in &a.files {
                self.owner.insert(canonical(Path::new(&f.path)), prog.clone());
            }
            self.owner.insert(file.clone(), prog.clone());
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
            let mut related: Vec<Value> = d
                .related
                .iter()
                .map(|(l, m)| {
                    json!({
                        "location": { "uri": path_to_uri(&l.path), "range": range(&text_of(&l.path), l.start, l.end) },
                        "message": if m.is_empty() { "related" } else { m.as_str() },
                    })
                })
                .collect();
            // related declarations (a called function, the expected type)
            related.extend(d.declarations.iter().map(|(n, l)| {
                json!({
                    "location": { "uri": path_to_uri(&l.path), "range": range(&text_of(&l.path), l.start, l.end) },
                    "message": format!("`{n}` is declared here"),
                })
            }));
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
        // library sources are not files
        if t.path.starts_with('<') {
            return Value::Null;
        }
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
    /// The name under the cursor and the exact ranges spelling it, each
    /// with whether it is a record-field shorthand (`{ name }`). Qualified
    /// uses (`util.helper`, `util.Point`) contribute their last segment;
    /// library definitions are not renamed.
    fn renamable(&mut self, params: &Value) -> Option<(String, Vec<(analysis::Loc, bool)>)> {
        let (_, locs) = self.occurrences(params)?;
        let (file, _, _) = self.locate(params)?;
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
        let name = a.file(&decl.path)?.text.get(decl.start..decl.end)?.trim().to_string();
        let ident = |c: char| c.is_alphanumeric() || c == '_';
        let mut out: Vec<(analysis::Loc, bool)> = vec![];
        for l in locs {
            let Some(f) = a.file(&l.path) else { continue };
            let Some(t) = f.text.get(l.start..l.end) else { continue };
            let t = t.trim_end();
            let start = if t == name {
                l.start
            } else if t.ends_with(&format!(".{name}")) {
                l.start + t.len() - name.len()
            } else if t.starts_with(&name) && !t[name.len()..].starts_with(ident) {
                // a type with arguments: `Point<T>`
                l.start
            } else {
                continue;
            };
            let loc = analysis::Loc { path: l.path.clone(), start, end: start + name.len() };
            let shorthand = f.shorthand.contains(&(loc.start, loc.end));
            if !out.iter().any(|(x, _)| *x == loc) {
                out.push((loc, shorthand));
            }
        }
        Some((name, out))
    }

    fn rename(&mut self, params: &Value) -> Result<Value, String> {
        let new = params["newName"].as_str().unwrap_or("");
        let Some((old, locs)) = self.renamable(params) else { return Err("nothing to rename here".into()) };
        let valid = new.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && new.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !KEYWORDS.contains(&new);
        // types are capitalized, values are not
        let class = |n: &str| n.chars().next().is_some_and(|c| c.is_ascii_uppercase());
        if !valid || class(new) != class(&old) {
            return Err(format!("`{new}` cannot replace `{old}`"));
        }
        // only where prepareRename would offer it
        let Some((_, path, off)) = self.locate(params) else { return Err("nothing to rename here".into()) };
        if !locs.iter().any(|(l, _)| l.path == path && l.start <= off && off <= l.end) {
            return Err("nothing to rename here".into());
        }
        let file = canonical(&uri_to_path(params["textDocument"]["uri"].as_str().unwrap_or("")));
        let mut changes: HashMap<String, Vec<Value>> = HashMap::new();
        for (l, shorthand) in &locs {
            let mut e = self.loc_json(&file, l);
            let uri = e["uri"].as_str().unwrap_or("").to_string();
            e["newText"] = json!(if *shorthand { format!("{old}: {new}") } else { new.to_string() });
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
        match locs.iter().find(|(l, _)| l.path == path && l.start <= off && off <= l.end) {
            Some((l, _)) => json!({ "range": range(&text, l.start, l.end), "placeholder": name }),
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
        // byte index after the last non-identifier character
        let after_last = |s: &str| {
            s.char_indices().rev().find(|(_, c)| !ident(*c)).map(|(i, c)| i + c.len_utf8()).unwrap_or(0)
        };
        let word_start = after_last(before);
        let prefix = &before[word_start..];
        let Some((index, modules, globals)) = self.last_good.get(&file).cloned() else {
            return json!(KEYWORDS.iter().map(|k| json!({"label": k, "kind": 14})).collect::<Vec<_>>());
        };
        let item = |i: &Item| json!({ "label": i.name, "detail": i.detail, "kind": completion_kind(i.kind) });
        // `receiver.`: module members, or fields and methods of a local
        if before[..word_start].ends_with('.') {
            let recv_end = word_start - 1;
            let recv_start = after_last(&before[..recv_end]);
            let recv = &before[recv_start..recv_end];
            // a member of a member, a call's result, a literal: not known here
            if recv.is_empty() || before[..recv_start].ends_with('.') {
                return json!([]);
            }
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
            if let Some(m) = m
                && tx.send(m).is_err()
            {
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
        owner: HashMap::new(),
        dirty: HashSet::new(),
    };
    *CLIENT_URIS.lock().unwrap() = Some(HashMap::new());
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
                    let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
                    let p = canonical(&uri_to_path(uri));
                    if let Some(m) = CLIENT_URIS.lock().unwrap().as_mut() {
                        m.insert(p.clone(), uri.to_string());
                    }
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
                                // an inverted range replaces nothing
                                text.replace_range(a..b.max(a), c["text"].as_str().unwrap_or(""));
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
pub fn query(
    kind: &str,
    path: &Path,
    line: Option<usize>,
    col: Option<usize>,
    overlays: HashMap<PathBuf, String>,
) -> Result<Value, String> {
    if kind == "api" {
        return api(path);
    }
    let file = canonical(path);
    let mut server = Server {
        docs: overlays.clone(),
        programs: HashMap::new(),
        last_good: HashMap::new(),
        published: HashSet::new(),
        owner: HashMap::new(),
        dirty: HashSet::from([file.clone()]),
    };
    if !matches!(kind, "diagnostics" | "check") {
        server.reanalyze(&mut std::io::sink());
    }
    let text_of = |p: &str| overlay_text(&overlays, p);
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
        "diagnostics" => json!(check_report(path, &overlays)["diagnostics"]),
        "check" => check_report(path, &overlays),
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
                "unknown query `{other}` (diagnostics, check, hover, definition, references, completion, symbols, api)"
            ));
        }
    })
}

/// A file's text: its proposed content, if any, else the file on disk.
fn overlay_text(overlays: &HashMap<PathBuf, String>, p: &str) -> String {
    match overlays.get(&lang_driver::overlay_key(Path::new(p))) {
        Some(t) => t.clone(),
        None => std::fs::read_to_string(p).unwrap_or_default(),
    }
}

/// Proposed file contents for `lang query --overlays`: a JSON object from
/// path to text, read from `spec` (a file, or `-` for standard input).
/// Relative paths are relative to the program's directory (`path` itself
/// for a project directory, else the file's directory).
pub fn read_overlays(spec: &str, path: &Path) -> Result<HashMap<PathBuf, String>, String> {
    let text = if spec == "-" {
        let mut t = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut t)
            .map_err(|e| format!("standard input: {e}"))?;
        t
    } else {
        std::fs::read_to_string(spec).map_err(|e| format!("cannot read {spec}: {e}"))?
    };
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("overlays: {e}"))?;
    let Some(obj) = v.as_object() else {
        return Err("overlays: expected an object from path to text".into());
    };
    let base = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent().unwrap_or(Path::new(".")).to_path_buf()
    };
    let mut out = HashMap::new();
    for (k, t) in obj {
        let Some(t) = t.as_str() else { return Err(format!("overlays: `{k}` is not a string")) };
        let p = Path::new(k);
        let p = if p.is_absolute() { p.to_path_buf() } else { base.join(p) };
        out.insert(lang_driver::overlay_key(&p), t.to_string());
    }
    Ok(out)
}

fn fnv(text: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in text.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{h:016x}")
}

/// One diagnostic as JSON, with 1-based lines and columns.
fn diagnostic_json(a: &Analysis, d: &analysis::FileDiag, text_of: &dyn Fn(&str) -> String) -> Value {
    let t = a.file(&d.path).map(|f| f.text.clone()).unwrap_or_else(|| text_of(&d.path));
    let (l, c) = line_col(&t, d.start);
    let (el, ec) = line_col(&t, d.end);
    let at = |l: &lang_driver::analysis::Loc| {
        let t = a.file(&l.path).map(|f| f.text.clone()).unwrap_or_else(|| text_of(&l.path));
        let (sl, sc) = line_col(&t, l.start);
        let (el, ec) = line_col(&t, l.end);
        json!({ "file": l.path, "line": sl, "col": sc, "end_line": el, "end_col": ec })
    };
    let related: Vec<Value> = d
        .related
        .iter()
        .map(|(l, m)| {
            let mut v = at(l);
            v["message"] = json!(m);
            v
        })
        .collect();
    let declarations: Vec<Value> = d
        .declarations
        .iter()
        .map(|(n, l)| {
            let mut v = at(l);
            v["name"] = json!(n);
            v
        })
        .collect();
    json!({
        "file": d.path, "line": l, "col": c, "end_line": el, "end_col": ec,
        "severity": match d.severity { lang_base::Severity::Error => "error", lang_base::Severity::Warning => "warning", lang_base::Severity::Note => "note" },
        "code": d.code, "message": d.message, "notes": d.notes,
        "label": d.label, "expected": d.expected, "actual": d.actual, "scope": d.scope,
        "related": related, "declarations": declarations,
    })
}

/// Check a program as `lang check` does (a project's main entry, its
/// `[entries]`, and its `[tasks]`) with `overlays` in place of the files
/// they name, without writing anything. The report: diagnostics, the
/// inputs checked (path, whether proposed, size, and hash), overlays no
/// checked module reached, and the roots covered (each entry point, the
/// tests, and the task spawns).
fn check_report(path: &Path, overlays: &HashMap<PathBuf, String>) -> Value {
    let file = canonical(path);
    let mut prog = program_of(&file);
    let text_of = |p: &str| overlay_text(overlays, p);
    let mut selections: Vec<Option<String>> = vec![None];
    selections.extend(lang_driver::project_entry_points(&prog).into_iter().map(Some));
    let mut diagnostics: Vec<Value> = vec![];
    let mut inputs: std::collections::BTreeMap<String, Value> = Default::default();
    let mut roots: Vec<Value> = vec![];
    let mut tests: Vec<String> = vec![];
    let mut tasks: Vec<Value> = vec![];
    let mut errors = false;
    let mut i = 0;
    while i < selections.len() {
        let sel = selections[i].clone();
        lang_driver::select_entry(sel.clone());
        let mut a = analysis::analyze(&prog, overlays.clone());
        if i == 0
            && prog != file
            && file.is_file()
            && !a.files.iter().any(|f| canonical(Path::new(&f.path)) == file)
        {
            // not reachable from the project's entry: a module on its own
            prog = file.clone();
            selections.truncate(1);
            a = analysis::analyze(&prog, overlays.clone());
        }
        for d in &a.diagnostics {
            errors |= d.severity == lang_base::Severity::Error;
            let v = diagnostic_json(&a, d, &text_of);
            if !diagnostics.contains(&v) {
                diagnostics.push(v);
            }
        }
        for f in &a.files {
            inputs.entry(f.path.clone()).or_insert_with(|| {
                let proposed = overlays.contains_key(&lang_driver::overlay_key(Path::new(&f.path)));
                json!({ "file": f.path, "proposed": proposed, "bytes": f.text.len(), "hash": fnv(&f.text) })
            });
        }
        roots.push(json!({
            "entry": sel.unwrap_or_else(|| "main".into()),
            "module": a.roots.entry, "function": a.roots.main,
        }));
        for t in &a.roots.tests {
            if !tests.contains(t) {
                tests.push(t.clone());
            }
        }
        for (f, l) in &a.roots.tasks {
            let (line, col) = line_col(&text_of(&l.path), l.start);
            let v = json!({ "function": f, "file": l.path, "line": line, "col": col });
            if !tasks.contains(&v) {
                tasks.push(v);
            }
        }
        i += 1;
    }
    lang_driver::select_entry(None);
    if prog.is_dir() {
        let m = prog.join("lang.toml");
        let t = text_of(&m.display().to_string());
        let proposed = overlays.contains_key(&lang_driver::overlay_key(&m));
        inputs.insert(
            m.display().to_string(),
            json!({ "file": m.display().to_string(), "proposed": proposed, "bytes": t.len(), "hash": fnv(&t) }),
        );
    }
    let checked: HashSet<PathBuf> = inputs.keys().map(|p| lang_driver::overlay_key(Path::new(p))).collect();
    let mut unused: Vec<String> =
        overlays.keys().filter(|k| !checked.contains(*k)).map(|k| k.display().to_string()).collect();
    unused.sort();
    json!({
        "ok": !errors, "diagnostics": diagnostics, "inputs": inputs.into_values().collect::<Vec<_>>(),
        "unused_overlays": unused, "roots": roots, "tests": tests, "tasks": tasks,
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
            let dir = lang_driver::private_temp_dir("lang-api").map_err(|e| e.to_string())?;
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
