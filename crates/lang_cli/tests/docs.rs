//! Documentation stays true to the toolchain: every `lang` code block in
//! the specification and guides checks (a `lang fragment` block is an
//! excerpt and is not checked), and the generated standard-library
//! reference in `docs/api` matches the checked interfaces.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn lang() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_lang"));
    c.current_dir(root());
    c
}

/// The Markdown files whose examples are checked.
fn documents() -> Vec<PathBuf> {
    let mut out = vec![root().join("SPEC.md"), root().join("ARCHITECTURE.md")];
    for dir in ["spec", "docs"] {
        let mut v: Vec<PathBuf> = std::fs::read_dir(root().join(dir))
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "md"))
            .collect();
        v.sort();
        out.extend(v);
    }
    out
}

/// `lang` blocks of a Markdown text: the line each starts on and its code.
fn blocks(text: &str) -> Vec<(usize, String)> {
    let mut out = vec![];
    // the open block: its first line, whether it is checked, and its code
    let mut cur: Option<(usize, bool, String)> = None;
    for (i, line) in text.lines().enumerate() {
        let fence = line.trim_start().starts_with("```");
        match &mut cur {
            None if fence => {
                // a block is checked when its info string is exactly `lang`
                let info = line.trim_start().trim_start_matches('`').trim();
                cur = Some((i + 1, info == "lang", String::new()));
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
    let dir = std::env::temp_dir().join(format!("lang-doc-examples-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut failures = vec![];
    let mut checked = 0;
    for doc in documents() {
        let text = std::fs::read_to_string(&doc).unwrap();
        for (line, code) in blocks(&text) {
            let f = dir.join(format!("example_{checked}.lang"));
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
        "documentation examples that do not check (fence excerpts as `lang fragment`):\n{}",
        failures.join("\n")
    );
}

#[test]
fn api_reference_current() {
    let out = lang().args(["doc", "std", "--out", "docs/api", "--check"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}
