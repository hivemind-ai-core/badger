//! `BADGER_MIR_VALIDATE=1`: the translation-validation gate for the MIR
//! optimizer.
//!
//! The optimizer restructures the program by design, so the gate compares
//! the *behaviour* of the optimized and unoptimized programs: it runs the
//! program twice (`run --release` and the same run with the MIR pipeline
//! off) and fails when their stdout or exit status differs. The driver's
//! structural check (`badger_mir::validate`) covers the pipeline's
//! determinism in the same mode. A deliberately broken rewrite must be
//! caught; the correct pipeline must pass and its output must not change.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn lang() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_badger"));
    c.current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."));
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("BADGER_") {
            c.env_remove(key);
        }
    }
    c
}

fn out(c: &mut Command) -> Output {
    c.output().unwrap()
}

fn temp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("badger-mir-validate-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// A program whose `n * 4` reaches the superoptimizer as a wrapping
/// multiplication by a power of two (an unchecked `Mul`, which the
/// superoptimizer turns into a left shift).
const PROGRAM: &str = "import std.io\npub fn main(args: List<String>) -> Unit {\n    let n = args.len()\n    let y = n.wrapping_mul(4)\n    let _ = io.println($\"y={y}\")\n}\n";

#[test]
fn the_check_passes_the_correct_pipeline_and_does_not_change_the_output() {
    let dir = temp("ok");
    let src = dir.join("ok.bdg");
    std::fs::write(&src, PROGRAM).unwrap();
    let plain =
        out(lang().args(["run", "--release", "--opt", "superopt"]).arg(&src).args(["--", "a", "b", "c"]));
    let checked = out(lang()
        .env("BADGER_MIR_VALIDATE", "1")
        .args(["run", "--release", "--opt", "superopt"])
        .arg(&src)
        .args(["--", "a", "b", "c"]));
    assert!(plain.status.success() && checked.status.success(), "{plain:?} {checked:?}");
    assert_eq!(plain.stdout, b"y=12\n");
    assert_eq!(checked.stdout, plain.stdout, "the check changed the program's output");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_deliberately_broken_rewrite_is_caught_by_the_check() {
    let dir = temp("broken");
    let src = dir.join("broken.bdg");
    std::fs::write(&src, PROGRAM).unwrap();
    // The broken rewrite shifts by one bit too many, so `n * 4` becomes
    // `n << 3`. Without the check it silently miscompiles.
    let broken = out(lang()
        .env("BADGER_MIR_BREAK", "superopt")
        .args(["run", "--release", "--opt", "superopt"])
        .arg(&src)
        .args(["--", "a", "b", "c"]));
    assert!(broken.status.success(), "{broken:?}");
    assert_eq!(broken.stdout, b"y=24\n", "the broken rewrite did not change the result");
    // With the check the same miscompile fails the compilation instead.
    let caught = out(lang()
        .env("BADGER_MIR_BREAK", "superopt")
        .env("BADGER_MIR_VALIDATE", "1")
        .args(["run", "--release", "--opt", "superopt"])
        .arg(&src)
        .args(["--", "a", "b", "c"]));
    assert!(!caught.status.success(), "the check passed a miscompile: {caught:?}");
    let stderr = String::from_utf8_lossy(&caught.stderr);
    assert!(stderr.contains("MIR translation validation failed"), "{stderr}");
    assert!(stderr.contains("y=24") && stderr.contains("y=12"), "no first difference: {stderr}");
    let _ = std::fs::remove_dir_all(dir);
}

/// `BADGER_MIR_TV=1`: the SMT translation validator (`badger_mir::opt::tv`)
/// refutes the same broken rewrite before the program runs, naming the pass,
/// the function, and the values the two versions compute.
#[test]
fn the_smt_validator_refutes_the_broken_rewrite() {
    let dir = temp("tv-broken");
    let src = dir.join("broken.bdg");
    std::fs::write(&src, PROGRAM).unwrap();
    let caught = out(lang()
        .env("BADGER_MIR_BREAK", "superopt")
        .env("BADGER_MIR_TV", "1")
        .args(["run", "--release", "--opt", "superopt"])
        .arg(&src)
        .args(["--", "a", "b", "c"]));
    assert!(!caught.status.success(), "the validator passed a miscompile: {caught:?}");
    assert!(caught.stdout.is_empty(), "the refuted program ran: {caught:?}");
    let stderr = String::from_utf8_lossy(&caught.stderr);
    assert!(stderr.contains("the `superopt` pass changed the meaning of"), "{stderr}");
    assert!(stderr.contains("refuted"), "{stderr}");
    let _ = std::fs::remove_dir_all(dir);
}

/// The gate of the SMT validator: every `tests/run` program is compiled with
/// `BADGER_MIR_TV=1`, no function a pass in the model changed is refuted, and
/// the saved MIR is byte for byte the one compiled with the check off -- the
/// check reads the passes and never changes the compiler's output.
#[test]
fn every_corpus_program_passes_the_smt_validator_with_its_output_unchanged() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files: Vec<PathBuf> = std::fs::read_dir(root.join("tests/run"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "bdg"))
        .collect();
    files.sort();
    let dir = temp("tv-corpus");
    let jobs = std::sync::Mutex::new(files);
    let failures = std::sync::Mutex::new(vec![]);
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| {
                loop {
                    let Some(f) = jobs.lock().unwrap().pop() else { break };
                    let stem = f.file_stem().unwrap().to_string_lossy().to_string();
                    let (plain, checked) =
                        (dir.join(format!("{stem}.bdgmir")), dir.join(format!("{stem}-tv.bdgmir")));
                    let off = out(lang().arg("compile").arg(&f).arg("-o").arg(&plain));
                    let on =
                        out(lang().env("BADGER_MIR_TV", "1").arg("compile").arg(&f).arg("-o").arg(&checked));
                    let stderr = String::from_utf8_lossy(&on.stderr);
                    if off.status.success() != on.status.success() || !stderr.contains("BADGER_MIR_TV:") {
                        failures.lock().unwrap().push(format!("{}: the check failed\n{stderr}", f.display()));
                    } else if std::fs::read(&plain).ok() != std::fs::read(&checked).ok() {
                        failures
                            .lock()
                            .unwrap()
                            .push(format!("{}: the check changed the saved MIR", f.display()));
                    }
                }
            });
        }
    });
    let _ = std::fs::remove_dir_all(dir);
    let failures = failures.into_inner().unwrap();
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
