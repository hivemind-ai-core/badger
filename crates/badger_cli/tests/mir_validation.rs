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
    let plain = out(lang().args(["run", "--release", "--opt", "superopt"]).arg(&src).args(["--", "a", "b", "c"]));
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
