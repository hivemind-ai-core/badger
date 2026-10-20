//! `badger lsp`: a language server over stdio (JSON-RPC with
//! `Content-Length` framing). Each change re-checks the program the
//! document belongs to (its project, or the file alone) with every open
//! buffer overlaid, using the same checker and diagnostics as `badger check`
//! (`badger_driver::analysis`). Bursts of edits are coalesced into one check.
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

use badger_driver::analysis::{self, Analysis, FileIndex, Item, SymbolInfo, SymbolKind};
use serde_json::{Value, json};

/// Keywords offered as completions: the reserved words and the ones that
/// open their own constructs.
const KEYWORDS: &[&str] = &[
    "fn", "let", "var", "const", "if", "else", "match", "for", "in", "while", "loop", "break", "continue",
    "return", "struct", "enum", "protocol", "impl", "import", "as", "pub", "type", "where", "derive", "true",
    "false", "unsafe", "extern", "dyn", "once", "use", "mut", "requires", "ensures", "system", "pipeline",
    "map", "set", "ref",
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

/// `file:///a/b%20c.bdg` -> `/a/b c.bdg`.
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
/// directory (with `badger.toml`), else the file itself.
fn program_of(path: &Path) -> PathBuf {
    if path.is_dir() && path.join("badger.toml").exists() {
        return path.to_path_buf();
    }
    let mut dir = path.parent();
    while let Some(d) = dir {
        if d.join("badger.toml").exists() {
            return d.to_path_buf();
        }
        dir = d.parent();
    }
    path.to_path_buf()
}

fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// `canonical` of an analysed file's path, cached: requests compare every
/// analysed file against the request's document.
fn canonical_cached(p: &str) -> PathBuf {
    thread_local! {
        static CACHE: std::cell::RefCell<HashMap<String, PathBuf>> = Default::default();
    }
    CACHE.with(|c| c.borrow_mut().entry(p.to_string()).or_insert_with(|| canonical(Path::new(p))).clone())
}

/// The request's document, as a canonical path.
fn doc_path(params: &Value) -> PathBuf {
    canonical(&uri_to_path(params["textDocument"]["uri"].as_str().unwrap_or("")))
}

/// The analysed entry of `file` (a canonical path) in `a`.
fn file_of<'a>(a: &'a Analysis, file: &Path) -> Option<&'a analysis::FileIndex> {
    a.files.iter().find(|f| canonical_cached(&f.path) == file)
}

/// Analyse the program `prog` that `file` belongs to; when the file is not
/// reachable from the program's entry, analyse the file on its own. Returns
/// the program analysed.
fn analyze_owning(prog: &Path, file: &Path, overlays: &HashMap<PathBuf, String>) -> (PathBuf, Analysis) {
    let a = analysis::analyze(prog, overlays.clone());
    if prog != file && file.is_file() && file_of(&a, file).is_none() {
        // not reachable from the project's entry: a module on its own
        return (file.to_path_buf(), analysis::analyze(file, overlays.clone()));
    }
    (prog.to_path_buf(), a)
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
    /// For a project with `[entries]` or `[tasks]`, the analysis of each
    /// other entry point (made when references or a rename first need
    /// them, dropped at the next change): a module reachable only from a
    /// task still has its uses found and renamed.
    entries: HashMap<PathBuf, Vec<Analysis>>,
}

impl Server {
    fn new() -> Server {
        Server {
            docs: HashMap::new(),
            programs: HashMap::new(),
            last_good: HashMap::new(),
            published: HashSet::new(),
            owner: HashMap::new(),
            dirty: HashSet::new(),
            entries: HashMap::new(),
        }
    }

    fn analysis_for(&self, file: &Path) -> Option<&Analysis> {
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
            && file_of(a, file).is_none()
        {
            return file.to_path_buf();
        }
        p
    }

    fn reanalyze(&mut self, out: &mut impl Write) {
        let dirty: Vec<PathBuf> = self.dirty.drain().collect();
        if !dirty.is_empty() {
            self.entries.clear();
        }
        let mut done: HashSet<PathBuf> = HashSet::new();
        for file in dirty {
            let prog = program_of(&file);
            if !done.insert(prog.clone()) {
                continue;
            }
            let (prog, a) = analyze_owning(&prog, &file, &self.docs);
            self.publish(&a, out);
            for f in &a.files {
                if !f.refs.is_empty() || !f.symbols.is_empty() {
                    let globals = a.globals.get(&f.path).cloned().unwrap_or_default();
                    self.last_good.insert(canonical_cached(&f.path), (f.clone(), a.modules.clone(), globals));
                }
            }
            for f in &a.files {
                self.owner.insert(canonical_cached(&f.path), prog.clone());
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
                "severity": match d.severity { badger_base::Severity::Error => 1, badger_base::Severity::Warning => 2, badger_base::Severity::Note => 3 },
                "source": "badger",
                "message": message,
                "relatedInformation": related,
            });
            if let Some(c) = &d.code {
                v["code"] = json!(c);
            }
            // the structured facts (also in the message as prose), for
            // clients and agents that act on them
            v["data"] = json!({
                "schema_version": 1,
                "label": d.label,
                "expected": d.expected,
                "actual": d.actual,
                "scope": d.scope,
                "notes": d.notes,
            });
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
        params["textDocument"]["uri"].as_str()?;
        let file = doc_path(params);
        let text = self.text(&file);
        let off = offset(&text, &params["position"]);
        let a = self.analysis_for(&file)?;
        let f = file_of(a, &file)?;
        Some((file, f.path.clone(), off))
    }

    fn hover(&mut self, params: &Value) -> Value {
        let Some((file, path, off)) = self.locate(params) else { return Value::Null };
        let text = self.text(&file);
        let Some(a) = self.analysis_for(&file) else { return Value::Null };
        match a.ref_at(&path, off) {
            Some(r) if !r.hover.is_empty() => json!({
                "contents": { "kind": "markdown", "value": format!("```badger\n{}\n```", r.hover) },
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

    /// For a project with `[entries]` or `[tasks]`: analyse each other
    /// entry point (once per change), and the main one too when only a
    /// module on its own was analysed.
    fn ensure_entries(&mut self, prog: &Path) {
        if prog.is_dir() && !self.entries.contains_key(prog) {
            let mut v = vec![];
            if !self.programs.contains_key(prog) {
                v.push(analysis::analyze(prog, self.docs.clone()));
            }
            for sel in badger_driver::project_entry_points(prog) {
                badger_driver::select_entry(Some(sel));
                v.push(analysis::analyze(prog, self.docs.clone()));
            }
            badger_driver::select_entry(None);
            self.entries.insert(prog.to_path_buf(), v);
        }
    }

    /// The key under the cursor in `a` (whose index of the file is `path`):
    /// every location referring to it, and its declaration.
    fn occurrences_in(
        a: &Analysis,
        path: &str,
        off: usize,
    ) -> Option<(Vec<analysis::Loc>, Option<analysis::Loc>)> {
        let r = a
            .file(path)?
            .refs
            .iter()
            .filter(|r| r.start <= off && off <= r.end && r.key.is_some())
            .min_by_key(|r| r.end - r.start)?;
        Some((a.references(r.key?), r.target.clone()))
    }

    /// What is under the cursor, in each analysis that knows it: those
    /// covering the file (its program's, or the file alone, and each entry
    /// point reaching it), then every other entry point's program holding
    /// its declaration (uses of a definition from modules this file's
    /// program does not reach). Each with the locations referring to it.
    fn per_analysis(&mut self, params: &Value) -> Vec<(&Analysis, Vec<analysis::Loc>)> {
        let Some((file, _, off)) = self.locate(params) else { return vec![] };
        let prog = program_of(&file);
        self.ensure_entries(&prog);
        let index_of = |a: &Analysis, p: &Path| file_of(a, p).map(|f| f.path.clone());
        let mut all: Vec<&Analysis> = self.analysis_for(&file).into_iter().collect();
        if let Some(a) = self.programs.get(&prog)
            && !all.iter().any(|x| std::ptr::eq(*x, a))
        {
            all.push(a);
        }
        all.extend(self.entries.get(&prog).into_iter().flatten());
        let mut out: Vec<(&Analysis, Vec<analysis::Loc>)> = vec![];
        let mut decl: Option<analysis::Loc> = None;
        for a in &all {
            let Some(path) = index_of(a, &file) else { continue };
            if let Some((locs, d)) = Self::occurrences_in(a, &path, off) {
                decl = decl.or(d);
                out.push((a, locs));
            }
        }
        let Some(d) = decl.filter(|d| !d.path.starts_with('<')) else { return out };
        let dfile = canonical(Path::new(&d.path));
        for a in all {
            if out.iter().any(|(x, _)| std::ptr::eq(*x, a)) {
                continue;
            }
            let Some(path) = index_of(a, &dfile) else { continue };
            if let Some((locs, _)) = Self::occurrences_in(a, &path, d.start) {
                out.push((a, locs));
            }
        }
        out
    }

    /// Every location referring to what is under the cursor, in every
    /// entry point's program.
    fn occurrences(&mut self, params: &Value) -> Option<Vec<analysis::Loc>> {
        let mut out: Vec<analysis::Loc> = vec![];
        for (_, locs) in self.per_analysis(params) {
            for l in locs {
                if !out.contains(&l) {
                    out.push(l);
                }
            }
        }
        if out.is_empty() { None } else { Some(out) }
    }

    fn loc_json(&mut self, l: &analysis::Loc) -> Value {
        let text = self.text(&canonical_cached(&l.path));
        json!({ "uri": path_to_uri(&l.path), "range": range(&text, l.start, l.end) })
    }

    fn references(&mut self, params: &Value) -> Value {
        let Some(locs) = self.occurrences(params) else { return json!([]) };
        let v: Vec<Value> = locs.iter().map(|l| self.loc_json(l)).collect();
        json!(v)
    }

    fn highlights(&mut self, params: &Value) -> Value {
        let file = doc_path(params);
        let Some(locs) = self.occurrences(params) else { return json!([]) };
        let text = self.text(&file);
        let v: Vec<Value> = locs
            .iter()
            .filter(|l| canonical_cached(&l.path) == file)
            .map(|l| json!({ "range": range(&text, l.start, l.end) }))
            .collect();
        json!(v)
    }

    /// The name under the cursor and the exact ranges spelling it, each
    /// with whether it is a record-field shorthand (`{ name }`), in every
    /// entry point's program. Qualified uses (`util.helper`, `util.Point`)
    /// contribute their last segment, so a path like `io.println` is not
    /// renamed as a whole; library definitions are not renamed.
    fn renamable(&mut self, params: &Value) -> Option<(String, Vec<(analysis::Loc, bool)>)> {
        let mut name: Option<String> = None;
        let mut out: Vec<(analysis::Loc, bool)> = vec![];
        for (a, locs) in self.per_analysis(params) {
            let Some((n, found)) = Self::spelled(a, locs) else { continue };
            if name.as_ref().is_some_and(|m| *m != n) {
                continue;
            }
            name = Some(n);
            for x in found {
                if !out.iter().any(|(y, _)| *y == x.0) {
                    out.push(x);
                }
            }
        }
        Some((name?, out))
    }

    /// In analysis `a`: the declared name `locs` refer to (found at its
    /// declaration) and the ranges spelling it.
    fn spelled(a: &Analysis, locs: Vec<analysis::Loc>) -> Option<(String, Vec<(analysis::Loc, bool)>)> {
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

    /// Quick fixes for the diagnostics in the request's context: a missing
    /// `match` arm (added with a `panic` body, so it stays visibly
    /// unfinished), and for an unknown
    /// name that is a standard module gets an action inserting its import
    /// after the file's last import.
    fn code_actions(&mut self, params: &Value) -> Value {
        let uri = params["textDocument"]["uri"].as_str().unwrap_or("").to_string();
        let file = canonical(&uri_to_path(&uri));
        let text = self.text(&file);
        let modules = badger_driver::std_modules();
        let mut actions = vec![];
        for d in params["context"]["diagnostics"].as_array().cloned().unwrap_or_default() {
            if d["code"] == "E0342"
                && let Some(action) = missing_arm(&uri, &text, &d)
            {
                actions.push(action);
                continue;
            }
            // an ignored `Result`: propagate it, or discard it explicitly
            if d["code"] == "E0337" {
                let (start, end) = (d["range"]["start"].clone(), d["range"]["end"].clone());
                for (title, at, new_text) in [
                    ("Propagate the error with `?`", end, "?"),
                    ("Discard the result with `let _ =`", start, "let _ = "),
                ] {
                    actions.push(json!({
                        "title": title,
                        "kind": "quickfix",
                        "diagnostics": [d.clone()],
                        "edit": { "changes": { uri.clone(): [{ "range": { "start": at, "end": at }, "newText": new_text }] } },
                    }));
                }
                continue;
            }
            if d["code"] != "E0127" {
                continue;
            }
            // a near miss: replace the name with the suggestion
            let notes = d["data"]["notes"].as_array().cloned().unwrap_or_default();
            if let Some(near) = notes
                .iter()
                .filter_map(|n| {
                    n.as_str()?.strip_prefix("did you mean `")?.strip_suffix("`?").map(str::to_string)
                })
                .next()
            {
                actions.push(json!({
                    "title": format!("Change to `{near}`"),
                    "kind": "quickfix",
                    "diagnostics": [d.clone()],
                    "isPreferred": true,
                    "edit": { "changes": { uri.clone(): [{ "range": d["range"].clone(), "newText": near }] } },
                }));
            }
            let msg = d["message"].as_str().unwrap_or("");
            let Some(name) = msg.strip_prefix("unknown name `").and_then(|r| r.split('`').next()) else {
                continue;
            };
            if name == "prelude" || !modules.contains(&name) {
                continue;
            }
            let import = format!("import std.{name}");
            if text.lines().any(|l| l.trim() == import) {
                continue;
            }
            // after the last import, else at the top
            let line = text
                .lines()
                .enumerate()
                .filter(|(_, l)| l.starts_with("import "))
                .map(|(i, _)| i + 1)
                .last()
                .unwrap_or(0);
            let at = json!({ "line": line, "character": 0 });
            actions.push(json!({
                "title": format!("Import std.{name}"),
                "kind": "quickfix",
                "diagnostics": [d],
                "isPreferred": true,
                "edit": { "changes": { uri.clone(): [{ "range": { "start": at, "end": at }, "newText": format!("{import}\n") }] } },
            }));
        }
        json!(actions)
    }

    fn rename(&mut self, params: &Value) -> Result<Value, String> {
        let new = params["newName"].as_str().unwrap_or("");
        let Some((old, locs)) = self.renamable(params) else { return Err("nothing to rename here".into()) };
        let valid = new.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && new.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !badger_syntax::RESERVED.contains(&new);
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
        let file = doc_path(params);
        self.check_rename(&file, &locs, &old, new)?;
        let mut changes: HashMap<String, Vec<Value>> = HashMap::new();
        for (l, shorthand) in &locs {
            let mut e = self.loc_json(l);
            let uri = e["uri"].as_str().unwrap_or("").to_string();
            e["newText"] = json!(if *shorthand { format!("{old}: {new}") } else { new.to_string() });
            e.as_object_mut().map(|o| o.remove("uri"));
            changes.entry(uri).or_default().push(e);
        }
        Ok(json!({ "changes": changes }))
    }

    /// Refuse a rename that changes what the program means: the renamed
    /// text is analysed again, and every reference must resolve as before
    /// (no use captured by, or captured from, another `new`), with no new
    /// errors (a second definition of `new`).
    fn check_rename(
        &self,
        file: &Path,
        locs: &[(analysis::Loc, bool)],
        old: &str,
        new: &str,
    ) -> Result<(), String> {
        let Some(before) = self.analysis_for(file) else { return Ok(()) };
        let prog = self.owner.get(file).cloned().unwrap_or_else(|| self.program_path(file));
        // the edits of each file, sorted: (old start, old end, new length)
        let mut edits: HashMap<String, Vec<(usize, usize, usize)>> = HashMap::new();
        for (l, shorthand) in locs {
            let len = if *shorthand { old.len() + 2 + new.len() } else { new.len() };
            edits
                .entry(canonical(Path::new(&l.path)).display().to_string())
                .or_default()
                .push((l.start, l.end, len));
        }
        let mut overlays = self.docs.clone();
        for (path, es) in &mut edits {
            es.sort();
            let Some(f) =
                before.files.iter().find(|f| canonical(Path::new(&f.path)).display().to_string() == *path)
            else {
                continue;
            };
            let mut text = f.text.clone();
            let mut mine: Vec<&(analysis::Loc, bool)> = locs
                .iter()
                .filter(|(l, _)| canonical(Path::new(&l.path)).display().to_string() == *path)
                .collect();
            mine.sort_by_key(|(l, _)| std::cmp::Reverse(l.start));
            for (l, shorthand) in mine {
                let t = if *shorthand { format!("{old}: {new}") } else { new.to_string() };
                text.replace_range(l.start..l.end, &t);
            }
            overlays.insert(PathBuf::from(path), text);
        }
        let after = if prog.is_dir() || prog == file {
            analysis::analyze(&prog, overlays)
        } else {
            analyze_owning(&prog, file, &overlays).1
        };
        // a position in either text, as one in the text before: inside an
        // edit it is that edit (its spelling changed), elsewhere shifted back
        #[derive(PartialEq, Eq, Hash, Clone, Debug)]
        enum Pos {
            Plain(String, usize),
            Edit(String, usize),
        }
        let canon = |path: &str, off: usize, renamed: bool| -> Pos {
            let key = canonical(Path::new(path)).display().to_string();
            let mut shift: isize = 0;
            for (i, (s, e, n)) in edits.get(&key).map(|v| v.as_slice()).unwrap_or(&[]).iter().enumerate() {
                let (s2, e2) = if renamed {
                    ((*s as isize + shift) as usize, (*s as isize + shift) as usize + n)
                } else {
                    (*s, *e)
                };
                if off >= s2 && off < e2.max(s2 + 1) {
                    return Pos::Edit(key, i);
                }
                if off < s2 {
                    break;
                }
                shift += *n as isize - (*e - *s) as isize;
            }
            let back = if renamed { (off as isize - shift) as usize } else { off };
            Pos::Plain(key, back)
        };
        let resolutions = |a: &Analysis, renamed: bool| -> HashMap<Pos, HashSet<Pos>> {
            let mut m: HashMap<Pos, HashSet<Pos>> = HashMap::new();
            for f in &a.files {
                for r in &f.refs {
                    let Some(t) = &r.target else { continue };
                    if t.start == r.start && t.path == f.path {
                        continue;
                    }
                    let to = if t.path.starts_with('<') {
                        Pos::Plain(t.path.clone(), t.start)
                    } else {
                        canon(&t.path, t.start, renamed)
                    };
                    m.entry(canon(&f.path, r.start, renamed)).or_default().insert(to);
                }
            }
            m
        };
        let errors = |a: &Analysis| {
            a.diagnostics.iter().filter(|d| d.severity == badger_base::Severity::Error).count()
        };
        if errors(&after) > errors(before) {
            let d = after.diagnostics.iter().find(|d| d.severity == badger_base::Severity::Error);
            let why = d.map(|d| format!(": {}", d.message)).unwrap_or_default();
            return Err(format!("renaming `{old}` to `{new}` would not check{why}"));
        }
        let (was, now) = (resolutions(before, false), resolutions(&after, true));
        let line = |p: &Pos| -> String {
            let (Pos::Plain(path, off) | Pos::Edit(path, off)) = p;
            let off = match p {
                Pos::Edit(path, i) => edits[path][*i].0,
                _ => *off,
            };
            let text = before
                .files
                .iter()
                .find(|f| canonical(Path::new(&f.path)).display().to_string() == *path)
                .map(|f| f.text.as_str())
                .unwrap_or("");
            let n = text.get(..off.min(text.len())).map(|t| t.matches('\n').count() + 1).unwrap_or(0);
            format!("{}:{n}", Path::new(path).file_name().and_then(|f| f.to_str()).unwrap_or(path))
        };
        for (at, targets) in &was {
            let moved = match now.get(at) {
                Some(t) => !targets.is_subset(t),
                None => true,
            };
            if moved {
                return Err(format!(
                    "renaming `{old}` to `{new}` would change what the name at {} refers to (another `{new}` is in scope)",
                    line(at)
                ));
            }
        }
        Ok(())
    }

    fn prepare_rename(&mut self, params: &Value) -> Value {
        let file = doc_path(params);
        let Some((_, path, off)) = self.locate(params) else { return Value::Null };
        let Some((name, locs)) = self.renamable(params) else { return Value::Null };
        let text = self.text(&file);
        match locs.iter().find(|(l, _)| l.path == path && l.start <= off && off <= l.end) {
            Some((l, _)) => json!({ "range": range(&text, l.start, l.end), "placeholder": name }),
            None => Value::Null,
        }
    }

    fn symbols(&mut self, params: &Value) -> Value {
        let file = doc_path(params);
        let text = self.text(&file);
        let Some(a) = self.analysis_for(&file) else { return json!([]) };
        let Some(f) = file_of(a, &file) else {
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
        let file = doc_path(params);
        let text = self.text(&file);
        let formatted = badger_syntax::format::format(&text);
        if formatted == text {
            return json!([]);
        }
        json!([{ "range": range(&text, 0, text.len()), "newText": formatted }])
    }

    fn completion(&mut self, params: &Value) -> Value {
        let file = doc_path(params);
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
    let mut server = Server::new();
    *CLIENT_URIS.lock().unwrap() = Some(HashMap::new());
    let mut shutdown = false;
    while let Ok(first) = rx.recv() {
        // everything already queued, handled in order: edits apply as they
        // come, a request first brings the analysis up to date with the
        // edits before it, and the batch's diagnostics publish at its end
        let mut batch = vec![first];
        while let Ok(m) = rx.try_recv() {
            batch.push(m);
        }
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
                    let p = doc_path(params);
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
                    let p = doc_path(params);
                    server.dirty.insert(p);
                }
                "textDocument/didClose" => {
                    let p = doc_path(params);
                    server.docs.remove(&p);
                    server.dirty.insert(p);
                }
                "exit" => return if shutdown { 0 } else { 1 },
                _ => {
                    let Some(id) = m.get("id").cloned() else { continue };
                    if !server.dirty.is_empty() && !matches!(method.as_str(), "initialize" | "shutdown") {
                        server.reanalyze(&mut out);
                    }
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
                                "codeActionProvider": { "codeActionKinds": ["quickfix"] },
                                "completionProvider": { "triggerCharacters": ["."] },
                            },
                            "serverInfo": { "name": "badger", "version": env!("CARGO_PKG_VERSION") },
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
                        "textDocument/codeAction" => Ok(server.code_actions(params)),
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
        }
        if !server.dirty.is_empty() {
            server.reanalyze(&mut out);
        }
    }
    0
}

/// The action adding an arm for the pattern a non-exhaustive `match`
/// misses (E0342 names it): before the match's closing brace, with a body
/// that panics until it is written.
fn missing_arm(uri: &str, text: &str, d: &Value) -> Option<Value> {
    let label = d["data"]["label"].as_str()?;
    let pattern = label.strip_prefix("pattern `")?.strip_suffix("` is not covered")?;
    // the scrutinee's end, then the match's braces
    let line = d["range"]["end"]["line"].as_u64()? as usize;
    let character = d["range"]["end"]["character"].as_u64()? as usize;
    let line_start: usize = text.split_inclusive('\n').take(line).map(str::len).sum();
    let line_text = text[line_start..].lines().next().unwrap_or("");
    let mut offset = line_start;
    let mut units = 0;
    for c in line_text.chars() {
        if units >= character {
            break;
        }
        units += c.len_utf16();
        offset += c.len_utf8();
    }
    let bytes = text.as_bytes();
    let open = offset + text[offset..].find('{')?;
    let (mut depth, mut i, mut in_string) = (0usize, open, false);
    let close = loop {
        let c = *bytes.get(i)?;
        if in_string {
            if c == b'\\' {
                i += 1;
            } else if c == b'"' {
                in_string = false;
            }
        } else if c == b'"' {
            in_string = true;
        } else if c == b'{' {
            depth += 1;
        } else if c == b'}' {
            depth -= 1;
            if depth == 0 {
                break i;
            }
        }
        i += 1;
    };
    let close_line_start = text[..close].rfind('\n').map(|p| p + 1).unwrap_or(0);
    let before = &text[close_line_start..close];
    let message = pattern.replace('\\', "\\\\").replace('"', "\\\"");
    let (at, new_text) = if before.trim().is_empty() {
        // `}` on its own line: a new line above it, indented one level more
        (close_line_start, format!("{before}    {pattern} => panic(\"not handled yet: {message}\"),\n"))
    } else {
        // one line: `match x { .a => 1 }`
        let prev = text[..close].trim_end();
        let sep = if prev.ends_with(',') || prev.ends_with('{') { " " } else { ", " };
        (prev.len(), format!("{sep}{pattern} => panic(\"not handled yet: {message}\"),"))
    };
    let pos = |o: usize| {
        let l = text[..o].matches('\n').count();
        let ls = text[..o].rfind('\n').map(|p| p + 1).unwrap_or(0);
        json!({ "line": l, "character": text[ls..o].chars().map(char::len_utf16).sum::<usize>() })
    };
    Some(json!({
        "title": format!("Add an arm for `{pattern}`"),
        "kind": "quickfix",
        "diagnostics": [d],
        "edit": { "changes": { uri: [{ "range": { "start": pos(at), "end": pos(at) }, "newText": new_text }] } },
    }))
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

/// The questions `badger query` answers.
pub const QUERY_KINDS: [&str; 9] = [
    "diagnostics",
    "check",
    "hover",
    "definition",
    "references",
    "completion",
    "symbols",
    "symbol",
    "api",
];

/// The rename, which `badger rename` asks for: it is not a question
/// `badger query` answers (the rename command computes the edits and
/// applies them).
pub const RENAME: &str = "rename";

/// The parser of the question `badger query` answers: the questions are
/// listed in the help and completed by the shell, and an unknown one is
/// named in the tool's own words.
#[derive(Clone)]
pub struct QueryKind;

impl clap::builder::TypedValueParser for QueryKind {
    type Value = String;

    fn parse_ref(
        &self,
        _cmd: &clap::Command,
        _arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let s = value.to_string_lossy().into_owned();
        if QUERY_KINDS.contains(&s.as_str()) {
            Ok(s)
        } else {
            Err(clap::Error::raw(
                clap::error::ErrorKind::ValueValidation,
                format!("unknown query `{s}` ({})\n", QUERY_KINDS.join(", ")),
            ))
        }
    }

    fn possible_values(&self) -> Option<Box<dyn Iterator<Item = clap::builder::PossibleValue> + '_>> {
        Some(Box::new(QUERY_KINDS.iter().map(|v| clap::builder::PossibleValue::new(*v))))
    }
}

/// `badger query`: one question about a program, answered as JSON with
/// 1-based lines and columns (for scripts, agents, and `std.compiler`).
pub fn query(
    kind: &str,
    path: &Path,
    line: Option<usize>,
    col: Option<usize>,
    name: Option<&str>,
    overlays: HashMap<PathBuf, String>,
) -> Result<Value, String> {
    if kind == "api" {
        return api(path);
    }
    const KINDS: &[&str] = &QUERY_KINDS;
    // the rename is the rename command's, not a question `badger query`
    // answers
    if !KINDS.contains(&kind) && kind != RENAME {
        return Err(format!("unknown query `{kind}` ({})", KINDS.join(", ")));
    }
    // inputs a kind cannot answer are errors, not empty answers (a check
    // reports an unreadable program as a diagnostic)
    let positional = matches!(kind, "hover" | "definition" | "references" | "completion" | "rename");
    let exists = path.exists() || overlays.contains_key(&badger_driver::overlay_key(path));
    if (positional || kind == "symbols") && !exists {
        return Err(format!("no such file or directory: {}", path.display()));
    }
    if (positional || kind == "symbols") && path.is_dir() {
        return Err(format!("`{kind}` needs a source file, not a directory"));
    }
    // `symbol` reads a project from its entry module
    let entry = if kind == "symbol" && path.is_dir() {
        badger_driver::Session::entry_of(path).map(|(f, _, _)| f).map_err(|e| e.to_string())?
    } else {
        path.to_path_buf()
    };
    let path = entry.as_path();
    let file = canonical(path);
    let mut server = Server { docs: overlays.clone(), dirty: HashSet::from([file.clone()]), ..Server::new() };
    if !matches!(kind, "diagnostics" | "check") {
        server.reanalyze(&mut std::io::sink());
    }
    let text_of = |p: &str| overlay_text(&overlays, p);
    let uri = path_to_uri(&file.display().to_string());
    let text = text_of(&file.display().to_string());
    let at = || -> Result<Value, String> {
        let (Some(l), Some(c)) = (line, col) else { return Err(format!("`{kind}` needs --line and --col")) };
        // positions are 1-based, and on a line of the file
        let lines = text.lines().count().max(1);
        if l == 0 || c == 0 {
            return Err(format!("--line and --col start at 1 (got {l}:{c})"));
        }
        if l > lines {
            return Err(format!("line {l} is past the end of the file ({lines} lines)"));
        }
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
                Some(v) => json!({ "text": v.trim_start_matches("```badger\n").trim_end_matches("\n```") }),
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
        "rename" => {
            let Some(new) = name else { return Err("`rename` needs --name NEW".into()) };
            let mut params = at()?;
            params["newName"] = json!(new);
            let r = server.rename(&params)?;
            let mut edits = vec![];
            for (u, es) in r["changes"].as_object().into_iter().flatten() {
                let p = uri_to_path(u).display().to_string();
                let t = text_of(&p);
                for e in es.as_array().into_iter().flatten() {
                    let mut v = plain_range(&p, &t, &e["range"]);
                    v["text"] = e["newText"].clone();
                    edits.push(v);
                }
            }
            edits.sort_by(|a, b| {
                let k = |v: &Value| {
                    (v["file"].as_str().unwrap_or("").to_string(), v["line"].as_u64(), v["col"].as_u64())
                };
                k(a).cmp(&k(b))
            });
            json!({ "edits": edits })
        }
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
        // a definition by its qualified name (`orders.service.create`,
        // `Order.total`, or just `create`): where it is, its signature and
        // type, and every reference to it
        "symbol" => {
            let Some(want) = name else {
                return Err("`symbol` needs --name (e.g. `orders.service.create`)".into());
            };
            let Some(a) = server.analysis_for(&file) else {
                return Err(format!("{} does not parse and resolve (see `diagnostics`)", path.display()));
            };
            let parts: Vec<&str> = want.split('.').collect();
            let (last, scope) = parts.split_last().map(|(l, s)| (*l, s.to_vec())).unwrap_or((want, vec![]));
            // every definition named `last`
            struct Hit {
                path: String,
                name: String,
                start: usize,
                kind: String,
                detail: String,
                container: Option<String>,
            }
            let mut found: Vec<Hit> = vec![];
            fn walk(
                f: &badger_driver::analysis::FileIndex,
                s: &SymbolInfo,
                container: Option<&str>,
                last: &str,
                out: &mut Vec<Hit>,
            ) {
                if s.name == last {
                    out.push(Hit {
                        path: f.path.clone(),
                        name: s.name.clone(),
                        start: s.name_start,
                        kind: format!("{:?}", s.kind).to_lowercase(),
                        detail: s.detail.clone(),
                        container: container.map(str::to_string),
                    });
                }
                for ch in &s.children {
                    walk(f, ch, Some(&s.name), last, out);
                }
            }
            for f in &a.files {
                for s in &f.symbols {
                    walk(f, s, None, last, &mut found);
                }
            }
            // the qualifying segments name the container, or end the
            // file's path (`orders.service` is `.../orders/service.bdg`)
            let matches = |h: &Hit| {
                if scope.is_empty() {
                    return true;
                }
                let mut rest = scope.clone();
                // a method's container is its impl (`impl Order`, or
                // `impl Display for Order`): `Order.total` names it
                let owner = h.container.as_deref().map(|c| {
                    let c = c.strip_prefix("impl ").unwrap_or(c);
                    let c = c.rsplit(" for ").next().unwrap_or(c);
                    c.split('<').next().unwrap_or(c).trim()
                });
                if owner.is_some() && owner == rest.last().copied() {
                    rest.pop();
                } else if h.container.is_some() {
                    return false;
                }
                if rest.is_empty() {
                    return true;
                }
                let stem = h.path.trim_end_matches(".bdg");
                stem.ends_with(&format!("/{}", rest.join("/"))) || stem == rest.join("/")
            };
            let hits: Vec<_> = found.into_iter().filter(|h| matches(h)).collect();
            let texts: HashMap<String, String> =
                a.files.iter().map(|f| (f.path.clone(), f.text.clone())).collect();
            let mut out = vec![];
            for Hit { path: fpath, name: sname, start, kind, detail, container } in hits {
                let text = texts.get(&fpath).cloned().unwrap_or_default();
                let (l, c) = line_col(&text, start);
                let line_text = text.lines().nth(l.saturating_sub(1)).unwrap_or("");
                let units: usize = line_text.chars().take(c.saturating_sub(1)).map(char::len_utf16).sum();
                let at = json!({ "textDocument": { "uri": path_to_uri(&fpath) }, "position": { "line": l - 1, "character": units } });
                let refs: Vec<Value> = server
                    .references(&at)
                    .as_array()
                    .map(|v| v.iter().map(loc).collect())
                    .unwrap_or_default();
                let hover = server.hover(&at)["contents"]["value"]
                    .as_str()
                    .map(|v| v.trim_start_matches("```badger\n").trim_end_matches("\n```").to_string());
                out.push(json!({
                    "name": sname, "kind": kind, "container": container, "detail": detail, "hover": hover,
                    "file": fpath, "line": l, "col": c, "references": refs,
                }));
            }
            json!(out)
        }
        "symbols" => {
            let unindexed = || {
                format!(
                    "{} was not indexed: it does not parse and resolve (see `diagnostics`)",
                    path.display()
                )
            };
            let Some(a) = server.analysis_for(&file) else { return Err(unindexed()) };
            let Some(f) = file_of(a, &file) else {
                return Err(unindexed());
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
        _ => unreachable!("checked against KINDS"),
    })
}

/// A file's text: its proposed content, if any, else the file on disk.
fn overlay_text(overlays: &HashMap<PathBuf, String>, p: &str) -> String {
    match overlays.get(&badger_driver::overlay_key(Path::new(p))) {
        Some(t) => t.clone(),
        None => std::fs::read_to_string(p).unwrap_or_default(),
    }
}

/// Proposed file contents for `badger query --overlays`: a JSON object from
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
        out.insert(badger_driver::overlay_key(&p), t.to_string());
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
    let at = |l: &badger_driver::analysis::Loc| {
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
        "severity": match d.severity { badger_base::Severity::Error => "error", badger_base::Severity::Warning => "warning", badger_base::Severity::Note => "note" },
        "code": d.code, "message": d.message, "notes": d.notes,
        "label": d.label, "expected": d.expected, "actual": d.actual, "scope": d.scope,
        "related": related, "declarations": declarations,
        "schema_version": 1,
    })
}

/// Check a program as `badger check` does (a project's main entry, its
/// `[entries]`, and its `[tasks]`) with `overlays` in place of the files
/// they name, without writing anything. The report: diagnostics, the
/// inputs checked (path, whether proposed, size, and hash), overlays no
/// checked module reached, and the roots covered (each entry point, the
/// tests, and the task spawns).
fn check_report(path: &Path, overlays: &HashMap<PathBuf, String>) -> Value {
    let file = canonical(path);
    // a file that is not there is reported itself (E0011), never replaced
    // by a check of the project around it (a mistyped path would "check")
    let exists = file.exists() || overlays.contains_key(&badger_driver::overlay_key(&file));
    let mut prog = if exists { program_of(&file) } else { file.clone() };
    let text_of = |p: &str| overlay_text(overlays, p);
    let mut selections: Vec<Option<String>> = vec![None];
    selections.extend(badger_driver::project_entry_points(&prog).into_iter().map(Some));
    let mut diagnostics: Vec<Value> = vec![];
    let mut inputs: std::collections::BTreeMap<String, Value> = Default::default();
    let mut roots: Vec<Value> = vec![];
    let mut tests: Vec<String> = vec![];
    let mut tasks: Vec<Value> = vec![];
    let mut errors = false;
    let mut i = 0;
    while i < selections.len() {
        let sel = selections[i].clone();
        badger_driver::select_entry(sel.clone());
        let a = if i == 0 {
            let (owner, a) = analyze_owning(&prog, &file, overlays);
            if owner != prog {
                prog = owner;
                selections.truncate(1);
            }
            a
        } else {
            analysis::analyze(&prog, overlays.clone())
        };
        for d in &a.diagnostics {
            errors |= d.severity == badger_base::Severity::Error;
            let v = diagnostic_json(&a, d, &text_of);
            if !diagnostics.contains(&v) {
                diagnostics.push(v);
            }
        }
        for f in &a.files {
            inputs.entry(f.path.clone()).or_insert_with(|| {
                let proposed = overlays.contains_key(&badger_driver::overlay_key(Path::new(&f.path)));
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
    badger_driver::select_entry(None);
    if prog.is_dir() {
        let m = prog.join("badger.toml");
        let t = text_of(&m.display().to_string());
        let proposed = overlays.contains_key(&badger_driver::overlay_key(&m));
        inputs.insert(
            m.display().to_string(),
            json!({ "file": m.display().to_string(), "proposed": proposed, "bytes": t.len(), "hash": fnv(&t) }),
        );
    }
    let checked: HashSet<PathBuf> = inputs.keys().map(|p| badger_driver::overlay_key(Path::new(p))).collect();
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
pub fn api(target: &Path) -> Result<Value, String> {
    let t = target.display().to_string();
    let (a, wanted): (Analysis, Option<String>) = match t.strip_prefix("std.") {
        Some(m) => {
            if badger_driver::std_source(m).is_none() {
                return Err(format!("no standard module `{t}`"));
            }
            let dir = badger_driver::private_temp_dir("badger-api").map_err(|e| e.to_string())?;
            let f = dir.join("api.bdg");
            std::fs::write(&f, format!("import std.{m}\npub fn main() -> Unit {{}}\n"))
                .map_err(|e| e.to_string())?;
            let a = analysis::analyze(&f, HashMap::new());
            let _ = std::fs::remove_dir_all(&dir);
            (a, Some(m.rsplit('.').next().unwrap_or(m).to_string()))
        }
        None => {
            if !target.exists() {
                return Err(format!("no such file or directory: {t}"));
            }
            let a = analysis::analyze(&program_of(&canonical(target)), HashMap::new());
            // an interface is the checked program's (not of one that fails)
            let errors = a.diagnostics.iter().filter(|d| d.severity == badger_base::Severity::Error).count();
            if errors > 0 {
                return Err(format!("{t} does not check ({errors} error(s)): run `badger check {t}`"));
            }
            (a, None)
        }
    };
    let mut out = vec![];
    let item = |module: &str, i: &Item| {
        json!({
            "module": module, "name": i.name, "kind": format!("{:?}", i.kind).to_lowercase(),
            "signature": i.detail, "declaration": i.decl, "doc": i.doc,
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
