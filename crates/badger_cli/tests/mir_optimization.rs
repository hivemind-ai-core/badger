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

#[test]
fn constant_module_values_fold_and_fallible_ones_stay_lazy() {
    let src = "tests/run/constant_module_values.bdg";
    let expected = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/run/constant_module_values.out"),
    )
    .unwrap();
    let (out, mir) = ok(lang().args(["run", "--release", "--dump-mir", src]).output().unwrap());
    assert_eq!(out, expected);
    // `limit`, the derived `scale`, and the prelude's `none` are constants:
    // their getters have no callers left
    for getter in ["limit#global", "scale#global", "prelude.none#global"] {
        assert!(!mir.contains(getter), "{getter} survived:\n{mir}");
    }
    // a division by zero and an overflow keep their lazy initializers
    assert!(
        mir.contains("boom#global") && mir.contains("past#global"),
        "a fallible getter was folded:\n{mir}"
    );
    let (ctrl_out, ctrl) = ok(lang()
        .env("BADGER_NO_PASS", "globals")
        .args(["run", "--release", "--dump-mir", src])
        .output()
        .unwrap());
    assert_eq!(ctrl_out, expected);
    assert!(ctrl.contains("limit#global"), "control lost its getter:\n{ctrl}");
}

/// Run a program with `--release`, keeping its functions apart (no
/// inlining) so each can be inspected on its own.
fn release_mir(name: &str, source: &str) -> (String, String) {
    let dir = temp(name);
    let src = dir.join(format!("{name}.bdg"));
    std::fs::write(&src, source).unwrap();
    let r = ok(lang()
        .env("BADGER_NO_PASS", "inline")
        .args(["run", "--release", "--dump-mir"])
        .arg(&src)
        .output()
        .unwrap());
    let _ = std::fs::remove_dir_all(dir);
    r
}

#[test]
fn a_list_length_is_a_load() {
    let (out, mir) = release_mir(
        "list-len",
        "import std.io\nimport std.list\nfn wsum(xs: List<Int>) -> Int {\n    var s = 0\n    var i = 0\n    while i < xs.len() {\n        s = s + xs[i]\n        i = i + 1\n    }\n    s\n}\npub fn main() -> Unit {\n    let _ = io.println($\"{wsum(list.range(0, 100))} {[1, 2].len()}\")\n}\n",
    );
    assert_eq!(out, "4950 2\n");
    assert!(!mir.contains("badger_rt_i_list_len"), "a list length still calls the runtime:\n{mir}");
}

/// One function's text in a MIR dump (the first whose name contains `name`).
fn function<'a>(mir: &'a str, name: &str) -> &'a str {
    let start = mir
        .match_indices("fn ")
        .map(|(i, _)| i)
        .find(|i| {
            (*i == 0 || mir.as_bytes()[i - 1] == b'\n') && mir[*i..].lines().next().unwrap().contains(name)
        })
        .unwrap_or_else(|| panic!("no function {name}:\n{mir}"));
    let end = mir[start + 1..].find("\nfn ").map_or(mir.len(), |e| start + 1 + e);
    &mir[start..end]
}

#[test]
fn a_match_over_many_variants_is_one_switch() {
    let (out, mir) = release_mir(
        "big-match",
        "import std.io\nenum Op { add, sub, mul, div, neg, dup, pop, swap }\nfn step(op: Op, a: Int) -> Int {\n    match op {\n        .add => a + 1,\n        .sub => a - 1,\n        .mul => a * 3,\n        .div => a / 2,\n        .neg => 0 - a,\n        .dup => a + a,\n        .pop => 0,\n        .swap => a + 7,\n    }\n}\npub fn main() -> Unit {\n    var a = 1\n    for o in [Op.add, Op.mul, Op.swap, Op.pop, Op.dup, Op.neg, Op.sub, Op.div] { a = step(o, a) }\n    let _ = io.println($\"{a}\")\n}\n",
    );
    assert_eq!(out, "0\n");
    let step = function(&mir, "big-match.step#");
    assert_eq!(step.matches("Switch(").count(), 1, "the match is still a chain of tests:\n{step}");
}

#[test]
fn an_exclusive_range_step_does_not_test_the_end() {
    let (out, mir) = release_mir(
        "range-step",
        "import std.io\nfn sum(n: Int) -> Int {\n    var s = 0\n    for i in 0..n { s = s + i }\n    s\n}\nfn sum_to(n: Int) -> Int {\n    var s = 0\n    for i in 0..=n { s = s + i }\n    s\n}\npub fn main(args: List<String>) -> Unit {\n    let n = args.len() + 9\n    let _ = io.println($\"{sum(n)} {sum_to(n)}\")\n}\n",
    );
    assert_eq!(out, "36 45\n");
    let eqs = |f: &str| function(&mir, f).matches("op: Eq").count();
    assert_eq!(eqs("range-step.sum#"), 0, "the exclusive loop tests its end twice:\n{mir}");
    assert!(eqs("range-step.sum_to#") >= 1, "the inclusive loop lost its endpoint test:\n{mir}");
}

#[test]
fn a_repeated_checked_computation_is_computed_once() {
    let (out, mir) = release_mir(
        "dup",
        "import std.io\nfn dup(a: Int, b: Int) -> Int {\n    let u = a * b + 1\n    let v = a * b + 1\n    u + v\n}\npub fn main(args: List<String>) -> Unit {\n    let n = args.len() + 5\n    let _ = io.println($\"{dup(n, n + 1)}\")\n}\n",
    );
    assert_eq!(out, "62\n");
    let dup = function(&mir, "dup.dup#");
    assert_eq!(dup.matches("op: Mul").count(), 1, "the second product is recomputed:\n{dup}");
    // when the shared check fails, the first product's location reports it
    let dir = temp("dup-overflow");
    let src = dir.join("overflow.bdg");
    std::fs::write(
        &src,
        "import std.io\nfn dup(a: Int, b: Int) -> Int {\n    let u = a * b + 1\n    let v = a * b + 1\n    u + v\n}\npub fn main(args: List<String>) -> Unit {\n    let n = 4611686018427387904 + args.len()\n    let _ = io.println($\"{dup(n, 4)}\")\n}\n",
    )
    .unwrap();
    for flags in [&[][..], &["--release"][..]] {
        let out = lang().env("BADGER_NO_PASS", "inline").arg("run").args(flags).arg(&src).output().unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            err.contains("integer overflow at ") && err.contains("overflow.bdg:3:13"),
            "{flags:?}: {err}"
        );
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_question_mark_on_a_known_result_skips_building_it() {
    let dir = temp("try-thread");
    let src = dir.join("try.bdg");
    std::fs::write(
        &src,
        "import std.io\nfn check(x: Int) -> Result<Int, String> { if x >= 0 { ok(x) } else { err(\"neg\") } }\nfn run(n: Int, k: Int) -> Result<Int, String> {\n    var s = 0\n    for i in 0..n {\n        let v = check(i - k)?\n        s = s + v\n    }\n    ok(s)\n}\npub fn main(args: List<String>) -> Unit {\n    let n = args.len() + 10\n    let _ = io.println($\"{run(n, 0).unwrap_or(-1)} {run(n, 3).unwrap_or(-1)}\")\n}\n",
    )
    .unwrap();
    let (out, mir) = ok(lang().args(["run", "--release", "--dump-mir"]).arg(&src).output().unwrap());
    assert_eq!(out, "45 -1\n");
    let main = function(&mir, "try.main");
    // the `ok` results are tested where they are built: none is allocated
    assert!(!main.contains("value: Const(Int(0, I32))"), "an `ok` is still built and tested:\n{main}");
    assert!(main.contains("value: Const(Int(1, I32))"), "the `err` path disappeared:\n{main}");
    let _ = std::fs::remove_dir_all(dir);
}
