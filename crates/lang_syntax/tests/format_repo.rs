//! Formatting every source in the repository preserves its parse tree (up to
//! a dropped trailing comma) and is idempotent, and the sources are kept in
//! canonical form.

use std::path::{Path, PathBuf};

fn lang_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "target" || n == ".git") {
                continue;
            }
            lang_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "lang") {
            out.push(p);
        }
    }
}

#[test]
fn formatting_preserves_structure() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = vec![];
    for d in ["examples", "tests", "bench", "crates/lang_hir/std"] {
        lang_files(&root.join(d), &mut files);
    }
    assert!(files.len() > 100, "found {} files", files.len());
    let mut checked = 0;
    for f in files {
        let src = std::fs::read_to_string(&f).unwrap();
        let Ok(before) = lang_syntax::parser::leaf_tokens(&src) else { continue };
        let once = lang_syntax::format::format(&src);
        assert_eq!(lang_syntax::format::format(&once), once, "not idempotent: {}", f.display());
        // the repository's own sources are kept in canonical form
        assert_eq!(once, src, "not canonically formatted (run `lang format`): {}", f.display());
        let after = lang_syntax::parser::leaf_tokens(&once)
            .unwrap_or_else(|e| panic!("{} no longer parses: {e}", f.display()));
        assert_eq!(before, after, "structure changed: {}", f.display());
        checked += 1;
    }
    assert!(checked > 100);
}
