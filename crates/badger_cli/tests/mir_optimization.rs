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

fn ok(out: Output) -> (String, String) {
    assert!(
        out.status.success(),
        "status {:?}\nstdout {}\nstderr {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
}

fn temp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("badger-mir-opt-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn large_helper(capture: bool) -> String {
    let mut s =
        String::from("import std.io\n\nfn apply_many(f: fn(Int) -> Int, x: Int) -> Int {\n    var v = x\n");
    for i in 0..90 {
        s.push_str(&format!("    v = v + {}\n", i % 7 + 1));
    }
    s.push_str("    f(v)\n}\n\npub fn main() -> Unit {\n");
    if capture {
        s.push_str("    let k = 2\n    let r = apply_many(fn(x) { x * k }, 3)\n");
    } else {
        s.push_str("    let r = apply_many(fn(x) { x * 2 }, 3)\n");
    }
    s.push_str("    let _ = io.println($\"{r}\")\n}\n");
    s
}

#[test]
fn a_large_helper_called_with_a_known_closure_is_specialized() {
    let dir = temp("specialize");
    let src = dir.join("specialize.bdg");
    std::fs::write(&src, large_helper(true)).unwrap();
    let (stdout, mir) = ok(lang().args(["run", "--release", "--dump-mir"]).arg(&src).output().unwrap());
    assert_eq!(stdout, "720\n");
    assert!(mir.contains(".spec"), "no specialized clone:\n{mir}");
    assert!(mir.contains("callee: Lambda("), "no direct lambda call:\n{mir}");
    assert!(!mir.contains("callee: Closure("), "indirect closure call remains:\n{mir}");
    // the capturing environment is still required
    assert!(mir.contains("Alloc {"), "capturing environment was dropped:\n{mir}");

    // the un-specialized form remains available as a control
    let (stdout, control) = ok(lang()
        .env("BADGER_NO_PASS", "specialize")
        .args(["run", "--release", "--dump-mir"])
        .arg(&src)
        .output()
        .unwrap());
    assert_eq!(stdout, "720\n");
    assert!(control.contains("callee: Closure("), "control lost its closure call:\n{control}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_non_capturing_closure_environment_is_eliminated() {
    let dir = temp("nocapture");
    let src = dir.join("nocapture.bdg");
    std::fs::write(&src, large_helper(false)).unwrap();
    let (stdout, mir) = ok(lang().args(["run", "--release", "--dump-mir"]).arg(&src).output().unwrap());
    assert_eq!(stdout, "720\n");
    assert!(!mir.contains("callee: Closure("), "indirect closure call remains:\n{mir}");
    assert!(mir.contains("callee: Lambda("), "no direct lambda call:\n{mir}");
    assert!(!mir.contains("Alloc {"), "closure environment was not eliminated:\n{mir}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn superopt_only_runs_when_requested_and_preserves_results() {
    let dir = temp("superopt");
    let src = dir.join("superopt.bdg");
    std::fs::write(
        &src,
        "import std.io\npub fn main(args: List<String>) -> Unit {\n    let n = args.len()\n    let k = n.wrapping_add(1).wrapping_sub(1)\n    let _ = io.println($\"{k}\")\n}\n",
    )
    .unwrap();
    let (plain_out, plain) =
        ok(lang().args(["run", "--release", "--dump-mir"]).arg(&src).arg("--").arg("x").output().unwrap());
    let (super_out, superopt) = ok(lang()
        .args(["run", "--release", "--opt", "superopt", "--dump-mir"])
        .arg(&src)
        .arg("--")
        .arg("x")
        .output()
        .unwrap());
    assert_eq!(plain_out, "1\n");
    assert_eq!(super_out, plain_out);
    assert!(plain.contains("op: Sub"), "expected the cancellation to be visible before superopt:\n{plain}");
    assert!(!superopt.contains("op: Sub"), "superopt kept the cancellable subtraction:\n{superopt}");

    // the same level on saved MIR sees the same optimization
    let saved = dir.join("superopt.bdgmir");
    ok(lang().args(["compile", "--opt", "superopt"]).arg(&src).arg("-o").arg(&saved).output().unwrap());
    let (saved_out, saved_mir) =
        ok(lang().args(["run", "--release", "--dump-mir"]).arg(&saved).arg("--").arg("x").output().unwrap());
    assert_eq!(saved_out, "1\n");
    assert!(!saved_mir.contains("op: Sub"), "saved MIR lost the superopt result:\n{saved_mir}");
    let _ = std::fs::remove_dir_all(dir);
}
