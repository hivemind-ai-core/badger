//! The commands the ergonomics work added: `rename`, `repl --load`,
//! `bench`, `doc --open` and `test --watch`.

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

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("badger-ergonomics-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

/// A rename changes the definition and every use of it.
#[test]
fn rename_changes_the_definition_and_its_uses() {
    let dir = scratch("rename");
    let file = dir.join("main.bdg");
    std::fs::write(
        &file,
        "import std.io\n\nfn helper(x: Int) -> Int { x + 1 }\n\npub fn main() -> Result<Unit, io.Error> {\n    io.println($\"{helper(1)}\")?\n    ok(())\n}\n",
    )
    .expect("the program");
    let out = lang()
        .args(["rename", file.to_str().unwrap(), "--line", "3", "--col", "4", "--name", "assist"])
        .output()
        .expect("badger rename");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = std::fs::read_to_string(&file).expect("the program");
    assert!(text.contains("fn assist(x: Int)"), "{text}");
    assert!(text.contains("assist(1)"), "{text}");
    assert!(!text.contains("helper"), "{text}");
    // the renamed program still runs
    let run = lang().args(["run", file.to_str().unwrap()]).output().expect("badger run");
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "2");
}

/// A file's definitions can be loaded into a REPL session.
#[test]
fn repl_loads_a_file() {
    let dir = scratch("repl");
    let file = dir.join("load.bdg");
    std::fs::write(&file, "fn twice(x: Int) -> Int { x * 2 }\n").expect("the file");
    let mut child = lang()
        .args(["repl", "--load", file.to_str().unwrap()])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("badger repl");
    {
        use std::io::Write;
        let mut stdin = child.stdin.take().expect("stdin");
        stdin.write_all(b"twice(21)\n").expect("the input");
        stdin.write_all(b":quit\n").expect("the input");
    }
    let out = child.wait_with_output().expect("the session");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(text.contains("42"), "{text}");
}

/// The benchmark programs are run and their measurements reported, with a
/// baseline to compare against.
#[test]
fn bench_reports_measurements() {
    let dir = scratch("bench");
    let program = dir.join("tiny.bdg");
    std::fs::write(
        &program,
        "import std.io\nimport std.time\n\npub fn main() -> Result<Unit, io.Error> {\n    let t = time.now()\n    var total = 0\n    for i in 0..100 { total = total + i }\n    io.println($\"sum: {total} items\")?\n    io.println($\"loop: {time.as_nanos(time.now() - t)} ns\")?\n    ok(())\n}\n",
    )
    .expect("the program");
    let baseline = dir.join("baseline.json");
    let out = lang()
        .args(["bench", dir.to_str().unwrap(), "--out", baseline.to_str().unwrap()])
        .output()
        .expect("badger bench");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(text.contains("sum 4950 items"), "{text}");
    assert!(text.contains("loop "), "{text}");
    // the baseline is JSON, and holds the measurements
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&baseline).expect("the baseline"))
            .expect("json");
    let program = json
        .as_object()
        .expect("an object")
        .keys()
        .next()
        .expect("the program")
        .clone();
    assert!(json[&program]["sum"]["value"].as_f64().unwrap() > 0.0);
    // a comparison against the baseline reports the change
    let out = lang()
        .args(["bench", dir.to_str().unwrap(), "--compare", baseline.to_str().unwrap()])
        .output()
        .expect("badger bench");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(text.contains("against"), "{text}");
}

/// The reference documentation can be written as one HTML page.
#[test]
fn doc_open_writes_one_html_page() {
    let dir = scratch("doc");
    let out = lang()
        .args(["doc", "std.list", "--open", "--out", dir.to_str().unwrap()])
        .output()
        .expect("badger doc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let page = dir.join("index.html");
    let html = std::fs::read_to_string(&page).expect("the page");
    assert!(html.starts_with("<!doctype html>"), "{}", &html[..40]);
    assert!(html.contains("<h1"), "{html}");
}
