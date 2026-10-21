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
    // more statements than the inliner takes, copies aside
    for i in 0..180 {
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

/// `let f = same; f(x)`: the local holds a *static* closure — a function
/// passed by name, whose object the caller never allocates — and the
/// program records which function that object calls, so the call is seen
/// through like a direct one. A single-definition copy of the local is too.
#[test]
fn a_function_held_in_a_local_is_inlined() {
    let dir = temp("static-local");
    let src = dir.join("static_local.bdg");
    std::fs::write(
        &src,
        "import std.io\n\nfn same(x: Int) -> Int { x }\n\
         fn by_local(x: Int) -> Int { let f = same; f(x) }\n\
         fn by_copy(x: Int) -> Int { let f = same; let g = f; g(x) }\n\n\
         pub fn main() -> Unit {\n    let _ = io.println($\"{by_local(3)} {by_copy(4)}\")\n}\n",
    )
    .unwrap();
    let (stdout, mir) = ok(lang().args(["run", "--release", "--dump-mir"]).arg(&src).output().unwrap());
    assert_eq!(stdout, "3 4\n");
    assert!(!mir.contains("callee: Closure"), "an indirect call remains:\n{mir}");
}

/// A local set to the same callable through more than one branch is known
/// too (`var f = same; if p { f = same }`): a declaration always gives a
/// value, so every path to a use holds that callable. A local that can hold
/// two *different* functions must stay indirect.
#[test]
fn a_local_set_only_to_the_same_function_is_known() {
    let dir = temp("same-function");
    let src = dir.join("same_function.bdg");
    // past the inliner's size limit, so each shape keeps its own function
    let pad: String = (0..45).map(|i| format!("    v = v + {}\n", i % 3 + 1)).collect();
    let mut s = String::from(
        "import std.io\n\nfn same(x: Int) -> Int { x }\nfn other(x: Int) -> Int { x + 1 }\n\n",
    );
    for (name, held) in [
        ("twice", "    var f = same\n    if x > 0 { f = same }"),
        ("copied", "    let one = same\n    var f = one\n    if x > 0 { f = one }"),
        ("varies", "    var f = same\n    if x > 0 { f = other }"),
    ] {
        s.push_str(&format!("fn {name}(x: Int) -> Int {{\n    var v = 0\n{pad}{held}\n    f(v + x)\n}}\n\n"));
    }
    s.push_str("pub fn main() -> Unit {\n    let _ = io.println($\"{twice(1)} {copied(1)} {varies(1)}\")\n}\n");
    std::fs::write(&src, s).unwrap();
    let (stdout, mir) = ok(lang().args(["run", "--release", "--dump-mir"]).arg(&src).output().unwrap());
    assert_eq!(stdout, "91 91 92\n");
    let sect = |name: &str| mir.split("\nfn ").find(|p| p.contains(name)).unwrap_or_default().to_string();
    for name in ["twice", "copied"] {
        assert!(!sect(name).contains("callee: Closure"), "{name} should be known:\n{}", sect(name));
    }
    assert!(
        sect("varies").contains("callee: Closure"),
        "a varying callee must stay indirect:\n{}",
        sect("varies")
    );
}

#[test]
fn a_large_helper_called_with_a_known_closure_is_specialized() {
    let dir = temp("specialize");
    let src = dir.join("specialize.bdg");
    std::fs::write(&src, large_helper(true)).unwrap();
    let (stdout, mir) = ok(lang().args(["run", "--release", "--dump-mir"]).arg(&src).output().unwrap());
    assert_eq!(stdout, "1436\n");
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
    assert_eq!(stdout, "1436\n");
    assert!(control.contains("callee: Closure("), "control lost its closure call:\n{control}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_non_capturing_closure_environment_is_eliminated() {
    let dir = temp("nocapture");
    let src = dir.join("nocapture.bdg");
    std::fs::write(&src, large_helper(false)).unwrap();
    let (stdout, mir) = ok(lang().args(["run", "--release", "--dump-mir"]).arg(&src).output().unwrap());
    assert_eq!(stdout, "1436\n");
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

#[test]
fn eager_filter_then_map_is_one_pass_when_the_filter_is_pure() {
    let dir = temp("eager-fusion");
    let src = dir.join("fusion.bdg");
    std::fs::write(
        &src,
        "import std.io\nimport std.list\npub fn main() -> Unit {\n    let xs = [1, 2, 3, 4, 5]\n    let ys = xs.filter(fn(x) { x < 4 }).map(fn(x) { x * 10 })\n    let _ = io.println($\"{ys}\")\n}\n",
    )
    .unwrap();
    let (out, fused) = ok(lang().args(["run", "--release", "--dump-mir"]).arg(&src).output().unwrap());
    assert_eq!(out, "[10, 20, 30]\n");
    let (ctrl_out, ctrl) = ok(lang()
        .env("BADGER_NO_LIST_FUSION", "1")
        .args(["run", "--release", "--dump-mir"])
        .arg(&src)
        .output()
        .unwrap());
    assert_eq!(ctrl_out, out);
    let main = |mir: &str| mir.split("\nfn 1 ").next().unwrap_or("").to_string();
    let builders = |mir: &str| main(mir).matches("list_builder").count();
    assert!(
        builders(&fused) < builders(&ctrl),
        "fused {} builders, control {}:\n{fused}",
        builders(&fused),
        builders(&ctrl)
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_effectful_filter_keeps_eager_order() {
    let dir = temp("eager-order");
    let src = dir.join("order.bdg");
    std::fs::write(
        &src,
        "import std.io\nimport std.list\npub fn main() -> Unit {\n    let xs = [1, 2, 3]\n    let ys = xs.filter(fn(x) { let _ = io.println($\"f{x}\"); x < 3 }).map(fn(x) { let _ = io.println($\"m{x}\"); x * 10 })\n    let _ = io.println($\"{ys}\")\n}\n",
    )
    .unwrap();
    let (out, _) = ok(lang().args(["run", "--release", "--dump-mir"]).arg(&src).output().unwrap());
    assert_eq!(out, "f1\nf2\nf3\nm1\nm2\n[10, 20]\n");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_may_fail_filter_is_not_fused() {
    let dir = temp("eager-rem");
    let src = dir.join("rem.bdg");
    std::fs::write(
        &src,
        "import std.io\nimport std.list\npub fn main() -> Unit {\n    let xs = [1, 2, 3, 4]\n    let ys = xs.filter(fn(x) { x % 2 == 0 }).map(fn(x) { x * 10 })\n    let _ = io.println($\"{ys}\")\n}\n",
    )
    .unwrap();
    let (out, fused) = ok(lang().args(["run", "--release", "--dump-mir"]).arg(&src).output().unwrap());
    let (ctrl_out, ctrl) = ok(lang()
        .env("BADGER_NO_LIST_FUSION", "1")
        .args(["run", "--release", "--dump-mir"])
        .arg(&src)
        .output()
        .unwrap());
    assert_eq!(out, ctrl_out);
    let main = |mir: &str| mir.split("\nfn 1 ").next().unwrap_or("").to_string();
    assert_eq!(
        main(&fused).matches("list_builder").count(),
        main(&ctrl).matches("list_builder").count(),
        "a possibly failing filter must not be fused"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn map_entries_chains_do_not_build_entry_lists() {
    let dir = temp("map-entries");
    let src = dir.join("entries.bdg");
    std::fs::write(
        &src,
        "import std.io\nimport std.map\nimport std.list\npub fn main() -> Unit {\n    let m = map.from_list([(1, 10), (2, 20), (3, 30)])\n    let xs = m.entries().map(fn((k, v)) { k + v })\n    let _ = io.println($\"{xs}\")\n    let total = m.entries().fold(0, fn(acc, (k, v)) { acc + k * v })\n    let _ = io.println($\"{total}\")\n}\n",
    )
    .unwrap();
    let (out, fused) = ok(lang().args(["run", "--release", "--dump-mir"]).arg(&src).output().unwrap());
    let (ctrl_out, ctrl) = ok(lang()
        .env("BADGER_NO_MAP_FUSION", "1")
        .args(["run", "--release", "--dump-mir"])
        .arg(&src)
        .output()
        .unwrap());
    assert_eq!(out, ctrl_out);
    assert!(ctrl.matches("map_entries").count() >= 2, "control did not materialize entries:\n{ctrl}");
    assert_eq!(fused.matches("map_entries").count(), 0, "fused MIR still builds entry lists:\n{fused}");
    let _ = std::fs::remove_dir_all(dir);
}
