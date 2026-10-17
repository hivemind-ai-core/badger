//! Independent regressions retained for ADV-001 through ADV-007, plus controls.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixtures(expectation: &str) -> Vec<PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(root().join("tests/adversarial/fixtures"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "bdg" || extension == "source"))
        .filter(|path| path.with_extension(expectation).is_file())
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no adversarial fixtures with .{expectation} expectations");
    files
}

/// Preserve the retained suite's per-process timeout and drain both pipes so
/// even a compiler producing a large diagnostic cannot block the runner.
fn execute(args: &[&str], file: &Path) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_badger"))
        .current_dir(root())
        .args(args)
        .arg(file)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = thread::spawn(move || {
        let mut bytes = vec![];
        stdout.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let err = thread::spawn(move || {
        let mut bytes = vec![];
        stderr.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let start = Instant::now();
    let (status, timed_out) = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break (status, false);
        }
        if start.elapsed() >= Duration::from_secs(20) {
            child.kill().unwrap();
            break (child.wait().unwrap(), true);
        }
        thread::sleep(Duration::from_millis(20));
    };
    let output = Output { status, stdout: out.join().unwrap(), stderr: err.join().unwrap() };
    assert!(
        !timed_out,
        "{} {args:?} timed out after 20 seconds\n{}",
        file.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn run_all(flags: &[&str]) {
    let mut failures = vec![];
    let args: Vec<_> = std::iter::once("run").chain(flags.iter().copied()).collect();
    for file in fixtures("out") {
        let expected = std::fs::read(file.with_extension("out")).unwrap();
        let output = execute(&args, &file);
        if output.status.code() != Some(0) || output.stdout != expected || !output.stderr.is_empty() {
            failures.push(format!(
                "{} {flags:?}: exit {:?}\nstdout: {:?}\nexpected: {:?}\nstderr: {}",
                file.display(),
                output.status.code(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&expected),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn run_single_worker() {
    run_all(&["--workers", "1"]);
}

#[test]
fn run_four_workers() {
    run_all(&["--workers", "4"]);
}

#[test]
fn run_optimized() {
    run_all(&["--release", "--workers", "1"]);
}

#[test]
fn reject_programs() {
    let mut failures = vec![];
    for file in fixtures("code") {
        let expected = std::fs::read_to_string(file.with_extension("code")).unwrap();
        let output = execute(&["check", "--json"], &file);
        let json = String::from_utf8_lossy(&output.stdout);
        if output.status.code() != Some(1)
            || !json.contains(&format!("\"code\":\"{}\"", expected.trim()))
            || !output.stderr.is_empty()
        {
            failures.push(format!(
                "{}: exit {:?}, expected diagnostic {}\n{json}\n{}",
                file.display(),
                output.status.code(),
                expected.trim(),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
