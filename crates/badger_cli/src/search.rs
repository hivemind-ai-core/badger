//! `badger search QUERY`: find a definition in the standard library and
//! in the project's modules.
//!
//! The standard library is searched in its sources (a name, or a word of
//! a name, matched case-insensitively); the project is searched in its
//! checked interface, the same one `badger query api` prints.

use std::path::Path;

/// One thing the query found.
pub struct Found {
    /// Where it is: `std.list` for the standard library, the module's
    /// name for the project.
    pub module: String,
    /// Its name.
    pub name: String,
    /// Its signature (or its kind).
    pub signature: String,
    /// Its documentation, the first line of it.
    pub doc: String,
}

/// The name of a definition: the word after its kind.
fn item_name(rest: &str) -> Option<&str> {
    let rest = rest.trim_start();
    let end = rest
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    if end == 0 { None } else { Some(&rest[..end]) }
}

/// The public items of a standard module, read from its source: the
/// kind, the name, the signature, and the doc comment above it.
fn std_items(module: &str) -> Vec<Found> {
    let mut out = vec![];
    let Some(source) = badger_driver::std_source(module) else { return out };
    // the doc comment above an item: its lines, until the item itself
    let mut doc: Vec<String> = vec![];
    for line in source.lines() {
        let t = line.trim();
        if let Some(text) = t.strip_prefix("///") {
            let text = text.trim();
            if text.is_empty() {
                doc.clear();
            } else {
                doc.push(text.to_string());
            }
            continue;
        }
        if t.starts_with("//") {
            continue;
        }
        let Some(rest) = t.strip_prefix("pub ") else {
            doc.clear();
            continue;
        };
        let Some((kind, rest)) = rest.split_once(' ') else {
            doc.clear();
            continue;
        };
        if !matches!(kind, "fn" | "struct" | "enum" | "type" | "let" | "protocol" | "const") {
            doc.clear();
            continue;
        }
        let Some(name) = item_name(rest) else {
            doc.clear();
            continue;
        };
        out.push(Found {
            module: format!("std.{module}"),
            name: name.to_string(),
            signature: t.to_string(),
            doc: std::mem::take(&mut doc).join(" "),
        });
    }
    out
}

/// The public items of a program, from its checked interface.
fn project_items(path: &Path) -> Vec<Found> {
    let Ok(items) = crate::lsp::api(path) else { return vec![] };
    let Some(list) = items.as_array() else { return vec![] };
    list.iter()
        .map(|i| Found {
            module: i["module"].as_str().unwrap_or("").to_string(),
            name: i["name"].as_str().unwrap_or("").to_string(),
            signature: i["signature"].as_str().unwrap_or("").to_string(),
            doc: i["doc"].as_str().unwrap_or("").to_string(),
        })
        .collect()
}

/// Whether a thing matches the query: its name holds it (ignoring case),
/// or its documentation does.
fn matches(f: &Found, query: &str) -> bool {
    let query = query.to_lowercase();
    f.name.to_lowercase().contains(&query) || f.doc.to_lowercase().contains(&query)
}

/// Everything the query finds: the standard library first, then the
/// project.
pub fn search(query: &str, path: &Path) -> Vec<Found> {
    let mut out = vec![];
    for m in badger_driver::std_modules() {
        for f in std_items(m) {
            if matches(&f, query) {
                out.push(f);
            }
        }
    }
    for f in project_items(path) {
        if matches(&f, query) {
            out.push(f);
        }
    }
    out
}

/// The matches as plain text: one per line, with the module, the name,
/// and the documentation.
pub fn text(found: &[Found]) -> String {
    let mut s = String::new();
    for f in found {
        s.push_str(&format!("{}.{}", f.module, f.name));
        if !f.signature.is_empty() {
            s.push_str(&format!("  {}", f.signature));
        }
        if !f.doc.is_empty() {
            s.push_str(&format!("\n    {}", f.doc));
        }
        s.push('\n');
    }
    s
}

/// The matches as JSON.
pub fn json(found: &[Found]) -> serde_json::Value {
    serde_json::Value::Array(
        found
            .iter()
            .map(|f| {
                serde_json::json!({
                    "module": f.module,
                    "name": f.name,
                    "signature": f.signature,
                    "doc": f.doc,
                })
            })
            .collect(),
    )
}
