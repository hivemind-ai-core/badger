//! `badger init`: turn a directory into a Badger project.
//!
//! The project it writes is the one `badger new` writes: a manifest and
//! an entry module with a program (or a library module, with `--lib`).
//! An existing file is never overwritten.

use std::path::Path;

/// The package name of the project in `dir`: the directory's name, as a
/// package name (lowercase, `_` between words).
fn package_name(dir: &Path) -> String {
    let name = dir
        .canonicalize()
        .ok()
        .as_deref()
        .and_then(|d| d.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "app".to_string());
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    let out = out.trim_matches('_').to_string();
    // a name `import` could not refer to (a reserved word, `std`) gets a
    // suffix
    if out.is_empty() {
        "app".to_string()
    } else if badger_syntax::RESERVED.contains(&out.as_str()) || out == "std" {
        format!("{out}_pkg")
    } else {
        out
    }
}

/// The manifest of the new project.
fn manifest(name: &str, entry: &str, lib: bool) -> String {
    let mut s = format!("[package]\nname = \"{name}\"\nentry = \"{entry}\"\n");
    if lib {
        s.push_str("kind = \"library\"\n");
    }
    s
}

/// The entry module of the new project.
fn entry_source(name: &str, lib: bool) -> String {
    if lib {
        format!(
            "// {name}: a library package: `badger test` runs the `test_*` functions here.\n\
             \n\
             /// The greeting the library offers.\n\
             pub fn greeting(name: String) -> String {{\n    $\"Hello, {{name}}!\"\n}}\n\
             \n\
             fn test_greeting() -> Unit {{\n    assert(greeting(\"you\") == \"Hello, you!\")\n}}\n"
        )
    } else {
        format!(
            "// {name}: `badger run` runs `main`; `badger test` runs the `test_*` functions.\n\
             import std.io\n\
             \n\
             fn greeting(name: String) -> String {{\n    $\"Hello, {{name}}!\"\n}}\n\
             \n\
             pub fn main() -> Result<Unit, io.Error> {{\n    io.println(greeting(\"world\"))?\n    ok(())\n}}\n\
             \n\
             fn test_greeting() -> Unit {{\n    assert(greeting(\"you\") == \"Hello, you!\")\n}}\n"
        )
    }
}

/// Turn `path` into a project: `badger.toml` and the entry module.
///
/// The directory is created when it does not exist. An existing
/// `badger.toml` (or entry module) is never overwritten: the command
/// fails and says so.
pub fn init(path: &Path, lib: bool) -> Result<(), String> {
    if path.exists() && !path.is_dir() {
        return Err(format!("{} is not a directory", path.display()));
    }
    let config = path.join("badger.toml");
    if config.exists() {
        return Err(format!("{} already exists", config.display()));
    }
    let entry = if lib { "src/lib.bdg" } else { "src/main.bdg" };
    let source = path.join(entry);
    if source.exists() {
        return Err(format!("{} already exists", source.display()));
    }
    // the directory is created first: the package's name is the
    // directory's name
    if let Err(e) = std::fs::create_dir_all(path) {
        return Err(format!("cannot create {}: {e}", path.display()));
    }
    let name = package_name(path);
    let files = [(config, manifest(&name, entry, lib)), (source, entry_source(&name, lib))];
    for (file, text) in &files {
        if let Some(parent) = file.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            return Err(format!("cannot create {}: {e}", parent.display()));
        }
        if let Err(e) = std::fs::write(file, text) {
            return Err(format!("cannot write {}: {e}", file.display()));
        }
    }
    Ok(())
}

/// What the command prints after it wrote the project.
pub fn report(path: &Path, lib: bool) -> String {
    let entry = if lib { "src/lib.bdg" } else { "src/main.bdg" };
    let last = if lib { "    badger test" } else { "    badger run" };
    format!(
        "created {}/badger.toml and {}/{entry}\n\nnext:\n    cd {}\n    badger check\n{last}",
        path.display(),
        path.display(),
        path.display()
    )
}
