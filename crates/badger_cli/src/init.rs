//! `badger init`: turn a directory into a Badger project.
//!
//! The project it writes is a manifest and an entry module with a
//! program; a template gives it another shape (a service, a worker, a
//! library). The directory is created when it does not exist, and an
//! existing file is never overwritten.

use std::path::Path;

/// The package name of the project in `dir`: the directory's name, as a
/// package name (lowercase, `_` between words).
fn package_name(dir: &Path) -> String {
    // the directory may not exist yet: its name is then the last part
    // of the path
    let name = dir
        .canonicalize()
        .ok()
        .as_deref()
        .and_then(|d| d.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .or_else(|| dir.file_name().map(|n| n.to_string_lossy().into_owned()))
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
    // a name `import` could not refer to (a reserved word, a standard
    // namespace) gets a suffix
    if out.is_empty() {
        "app".to_string()
    } else if badger_syntax::RESERVED.contains(&out.as_str()) || badger_stdlib::is_root(&out) {
        format!("{out}_pkg")
    } else {
        out
    }
}

/// The manifest of the new project.
fn manifest(name: &str, entry: &str) -> String {
    format!("[package]\nname = \"{name}\"\nentry = \"{entry}\"\n")
}

/// The entry module of the new project: a program with a test.
fn entry_source(name: &str) -> String {
    format!(
        "// {name}: `badger run` runs `main`; `badger test` runs the `test_*` functions.\n\
         import std.io\n\
         \n\
         fn greeting(name: String) -> String {{\n    $\"Hello, {{name}}!\"\n}}\n\
         \n\
         pub fn main() -> Result<Unit, io.Error> {{\n    io.println(greeting(\"{name}\"))?\n    ok(())\n}}\n\
         \n\
         fn test_greeting() -> Unit {{\n    assert(greeting(\"you\") == \"Hello, you!\")\n}}\n"
    )
}

/// The files of the project: the template's, or a manifest and an entry
/// module with a program.
fn files(name: &str, template: Option<&str>) -> Result<Vec<(String, String)>, String> {
    match template {
        Some(t) => crate::templates::plan(name, t),
        None => Ok(vec![
            ("badger.toml".to_string(), manifest(name, "src/main.bdg")),
            ("src/main.bdg".to_string(), entry_source(name)),
        ]),
    }
}

/// Whether the project is a library package (it has no `main` to run).
fn is_library(files: &[(String, String)]) -> bool {
    files
        .iter()
        .find(|(path, _)| path == "badger.toml")
        .is_some_and(|(_, text)| text.contains("kind = \"library\""))
}

/// Turn `path` into a project: `badger.toml` and the entry module (or
/// the template's files). The files it wrote are returned.
///
/// The directory is created when it does not exist. An existing file of
/// the project is never overwritten: the command fails and says so.
pub fn init(path: &Path, template: Option<&str>) -> Result<Vec<(String, String)>, String> {
    if path.exists() && !path.is_dir() {
        return Err(format!("{} is not a directory", path.display()));
    }
    let name = package_name(path);
    let files = files(&name, template)?;
    for (file, _) in &files {
        if path.join(file).exists() {
            return Err(format!("{} already exists", path.join(file).display()));
        }
    }
    // the directory is created first: the package's name is the
    // directory's name
    if let Err(e) = std::fs::create_dir_all(path) {
        return Err(format!("cannot create {}: {e}", path.display()));
    }
    for (file, text) in &files {
        let f = path.join(file);
        if let Some(parent) = f.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            return Err(format!("cannot create {}: {e}", parent.display()));
        }
        if let Err(e) = std::fs::write(&f, text) {
            return Err(format!("cannot write {}: {e}", f.display()));
        }
    }
    Ok(files)
}

/// A path as a shell word: in single quotes when it holds a space or
/// another character the shell would read (`cd 'My App'`).
fn shell_word(p: &str) -> String {
    let plain = |c: char| c.is_alphanumeric() || "/._-+,:@%=".contains(c);
    if !p.is_empty() && p.chars().all(plain) {
        p.to_string()
    } else {
        format!("'{}'", p.replace('\'', r"'\''"))
    }
}

/// What the command prints after it wrote the project.
pub fn report(path: &Path, files: &[(String, String)]) -> String {
    let made = files
        .iter()
        .map(|(file, _)| format!("{}/{}", path.display(), file))
        .collect::<Vec<_>>()
        .join(" and ");
    let mut s = format!(
        "created {made}\n\nnext:\n    cd {}\n    badger check\n",
        shell_word(&path.display().to_string())
    );
    if !is_library(files) {
        s.push_str("    badger run\n");
    }
    s.push_str("    badger test");
    s
}
