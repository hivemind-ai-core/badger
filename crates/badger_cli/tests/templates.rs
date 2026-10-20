//! The project templates of `badger init --template=NAME`: every template
//! must produce a project that checks, runs, and passes its tests.

#[path = "../src/templates.rs"]
mod templates;

use std::path::{Path, PathBuf};
use std::process::Command;

fn badger() -> Command {
    Command::new(env!("CARGO_BIN_EXE_badger"))
}

/// A fresh temporary directory for a project.
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("badger-template-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Write the plan of `template` into `dir`, as `badger init` would.
fn write(dir: &Path, template: &str, name: &str) {
    for (path, text) in templates::plan(name, template).unwrap() {
        let file = dir.join(&path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, text).unwrap();
    }
}

fn report(output: &std::process::Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr))
}

#[test]
fn the_templates_are_planned() {
    assert_eq!(templates::names(), vec!["cli", "service", "framework", "library", "worker"]);
    for template in templates::names() {
        let files = templates::plan("inventory", template).unwrap();
        assert!(!files.is_empty(), "{template} has no files");
        let manifest = files
            .iter()
            .find(|(path, _)| path == "badger.toml")
            .unwrap_or_else(|| panic!("{template} has no badger.toml"));
        assert!(manifest.1.contains("name = \"inventory\""), "{template}: {}", manifest.1);
        for (path, text) in &files {
            assert!(!Path::new(path).is_absolute(), "{template}: {path} is absolute");
            assert!(!path.contains(".."), "{template}: {path} leaves the project");
            assert!(!text.contains("@name@"), "{template}: {path} keeps the placeholder");
        }
    }
}

#[test]
fn an_unknown_template_is_an_error() {
    let error = templates::plan("inventory", "nope").unwrap_err();
    for template in templates::names() {
        assert!(error.contains(template), "{error}");
    }
}

#[test]
fn every_template_checks_and_runs() {
    for template in templates::names() {
        let dir = temp_dir(template);
        write(&dir, template, "inventory");
        let path = dir.to_str().unwrap();

        let check = badger().args(["check", path]).output().unwrap();
        assert!(check.status.success(), "{template} does not check:\n{}", report(&check));

        let test = badger().args(["test", path]).output().unwrap();
        assert!(test.status.success(), "{template} does not pass its tests:\n{}", report(&test));

        let run = badger().args(["run", path]).output().unwrap();
        if template == "library" {
            // a library package has no `main` to run
            assert!(!run.status.success(), "{template} should not run");
            assert!(report(&run).contains("is a library"), "{}", report(&run));
        } else {
            assert!(run.status.success(), "{template} does not run:\n{}", report(&run));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
