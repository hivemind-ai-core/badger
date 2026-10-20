//! The tools that serve the toolchain itself: `badger completions`,
//! `badger doctor`, `badger init`, and `badger watch`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn lang() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_badger"));
    c.current_dir(root());
    c
}

/// A directory of its own for a test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("badger-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `badger completions SHELL`: a script for every shell, and a usage
/// error for an unknown one.
#[test]
fn completions() {
    for shell in ["bash", "zsh", "fish", "powershell"] {
        let out = lang().args(["completions", shell]).output().unwrap();
        assert!(out.status.success(), "{shell}");
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        assert!(text.contains("badger completion"), "{shell}: {text}");
        // the commands and their flags are completed
        assert!(text.contains("check"), "{shell}: {text}");
        assert!(text.contains("json"), "{shell}: {text}");
        // the script says how to install it
        assert!(text.contains("badger completions"), "{shell}: {text}");
    }
    // an unknown shell is a usage error
    let out = lang().args(["completions", "tcsh"]).output().unwrap();
    assert!(!out.status.success());
    let text = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(text.contains("possible values"), "{text}");
}

/// `badger doctor`: the report of the environment, as text and as JSON.
#[test]
fn doctor() {
    let dir = scratch("doctor");
    // a directory that is not a project
    let out = lang().current_dir(&dir).arg("doctor").output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(text.contains("cwd: "), "{text}");
    assert!(text.contains("project: none"), "{text}");
    assert!(text.contains("workers: "), "{text}");
    // a project
    std::fs::write(dir.join("badger.toml"), "[package]\nname = \"shop\"\nentry = \"src/main.bdg\"\n")
        .unwrap();
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/main.bdg"), "pub fn main() -> Unit {}\n").unwrap();
    let out = lang().current_dir(&dir).arg("doctor").output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(text.contains("name: shop"), "{text}");
    assert!(text.contains("source files: 1"), "{text}");
    // the JSON form
    let out = lang().current_dir(&dir).args(["doctor", "--json"]).output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["project"]["name"], "shop");
    assert_eq!(v["project"]["source_files"], 1);
    assert!(v["runtime"]["workers"].is_string());
    let _ = std::fs::remove_dir_all(&dir);
}

/// `badger init`: a project that checks, and never an existing file.
#[test]
fn init_project() {
    let dir = scratch("init");
    let project = dir.join("shop");
    let out = lang().args(["init", project.to_str().unwrap()]).output().unwrap();
    assert!(out.status.success());
    assert!(project.join("badger.toml").is_file());
    assert!(project.join("src/main.bdg").is_file());
    // the project checks
    let out = lang().arg("check").arg(&project).output().unwrap();
    assert!(out.status.success());
    // an existing manifest is never overwritten
    let out = lang().args(["init", project.to_str().unwrap()]).output().unwrap();
    assert!(!out.status.success());
    let text = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(text.contains("already exists"), "{text}");
    // a library project, too
    let lib = dir.join("lib");
    let out = lang().args(["init", "--template", "library", lib.to_str().unwrap()]).output().unwrap();
    assert!(out.status.success());
    assert!(lib.join("src/lib.bdg").is_file());
    let out = lang().arg("check").arg(&lib).output().unwrap();
    assert!(out.status.success());
    let _ = std::fs::remove_dir_all(&dir);
}

/// `badger watch`: the command runs again when a source file changes.
#[test]
fn watch_reruns() {
    let dir = scratch("watch");
    std::fs::write(dir.join("badger.toml"), "[package]\nname = \"shop\"\nentry = \"src/main.bdg\"\n")
        .unwrap();
    std::fs::create_dir_all(dir.join("src")).unwrap();
    let source = dir.join("src/main.bdg");
    std::fs::write(&source, "pub fn main() -> Unit {}\n").unwrap();
    // the output of the watch goes to a file, so it can be read while it runs
    let log = dir.join("watch.log");
    let file = std::fs::File::create(&log).unwrap();
    let mut child =
        lang().arg("watch").arg(&dir).stdout(Stdio::from(file)).stderr(Stdio::null()).spawn().unwrap();
    // the change is seen, and the command runs again
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut runs = 0;
    let mut changed = false;
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
        let text = std::fs::read_to_string(&log).unwrap_or_default();
        runs = text.lines().filter(|l| l.starts_with("running:")).count();
        if runs >= 1 && !changed {
            // the file changes: the watch runs the command again
            let mut src = std::fs::read_to_string(&source).unwrap();
            src.push_str("\n// a change\n");
            std::fs::write(&source, src).unwrap();
            changed = true;
        }
        if runs >= 2 && text.contains("changed:") {
            break;
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(text.contains("changed: "), "{text}");
    assert!(runs >= 2, "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}
