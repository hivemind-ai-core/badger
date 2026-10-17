//! Documentation stays true to the toolchain: every `badger` code block in
//! the specification and guides checks (a `badger fragment` block is an
//! excerpt and is not checked), and the generated standard-library
//! reference in `docs/api` matches the checked interfaces.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn lang() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_badger"));
    c.current_dir(root());
    c
}

/// The Markdown files whose examples are checked: the specification, the
/// architecture notes, and the guides (every `docs` directory but the
/// generated `docs/api`).
fn documents() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let mut v: Vec<PathBuf> =
            std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok().map(|e| e.path())).collect();
        v.sort();
        for p in v {
            if p.is_dir() {
                if !p.ends_with("docs/api") {
                    walk(&p, out);
                }
            } else if p.extension().is_some_and(|x| x == "md") {
                out.push(p);
            }
        }
    }
    let mut out = vec![root().join("SPEC.md"), root().join("ARCHITECTURE.md")];
    for dir in ["spec", "docs"] {
        walk(&root().join(dir), &mut out);
    }
    out
}

/// `badger` blocks of a Markdown text: the line each starts on and its code.
fn blocks(text: &str) -> Vec<(usize, String)> {
    let mut out = vec![];
    // the open block: its first line, whether it is checked, and its code
    let mut cur: Option<(usize, bool, String)> = None;
    for (i, line) in text.lines().enumerate() {
        let fence = line.trim_start().starts_with("```");
        match &mut cur {
            None if fence => {
                // a block is checked when its info string is exactly `badger`
                let info = line.trim_start().trim_start_matches('`').trim();
                cur = Some((i + 1, info == "badger", String::new()));
            }
            Some((start, checked, code)) if fence => {
                if *checked {
                    out.push((*start, std::mem::take(code)));
                }
                cur = None;
            }
            Some((_, true, code)) => {
                code.push_str(line);
                code.push('\n');
            }
            _ => {}
        }
    }
    out
}

#[test]
fn doc_examples_check() {
    let dir = std::env::temp_dir().join(format!("badger-doc-examples-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut failures = vec![];
    let mut checked = 0;
    for doc in documents() {
        let text = std::fs::read_to_string(&doc).unwrap();
        for (line, code) in blocks(&text) {
            let f = dir.join(format!("example_{checked}.bdg"));
            std::fs::write(&f, &code).unwrap();
            checked += 1;
            let out = lang().arg("check").arg(&f).output().unwrap();
            if !out.status.success() {
                let name = doc.strip_prefix(root()).unwrap_or(&doc).display().to_string();
                let err = String::from_utf8_lossy(&out.stderr);
                let first: Vec<&str> = err.lines().take(3).collect();
                failures.push(format!("{name}:{line}: {}", first.join(" / ")));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(checked > 0, "no examples found");
    assert!(
        failures.is_empty(),
        "documentation examples that do not check (fence excerpts as `badger fragment`):\n{}",
        failures.join("\n")
    );
}

#[test]
fn api_reference_current() {
    let out = lang().args(["doc", "std", "--out", "docs/api", "--check"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

/// Every diagnostic code the compiler emits has a catalog entry
/// (`badger explain diagnostic CODE`), and every entry is still used.
#[test]
fn diagnostic_codes_catalogued() {
    fn codes(text: &str, out: &mut std::collections::BTreeSet<String>) {
        let b = text.as_bytes();
        for i in 0..b.len().saturating_sub(6) {
            if b[i] == b'"'
                && b[i + 1] == b'E'
                && b[i + 6] == b'"'
                && b[i + 2..i + 6].iter().all(u8::is_ascii_digit)
            {
                out.insert(text[i + 1..i + 6].to_string());
            }
        }
    }
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let mut files = vec![];
    for c in std::fs::read_dir(root().join("crates")).unwrap().flatten() {
        if c.path().join("src").is_dir() {
            walk(&c.path().join("src"), &mut files);
        }
    }
    let (mut used, mut catalogued) = (Default::default(), Default::default());
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        if f.ends_with("badger_base/src/catalog.rs") {
            codes(&text, &mut catalogued);
        } else {
            codes(&text, &mut used);
        }
    }
    let missing: Vec<&String> = used.difference(&catalogued).collect();
    let stale: Vec<&String> = catalogued.difference(&used).collect();
    assert!(
        missing.is_empty(),
        "codes without a catalog entry (crates/badger_base/src/catalog.rs): {missing:?}"
    );
    assert!(stale.is_empty(), "catalog entries no code uses: {stale:?}");
}
