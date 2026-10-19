use std::{path::Path, process::Command};

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

#[test]
fn fusion_preserves_construction_effect_order_limits_and_roots() {
    for w in [1, 2, 4, 8] {
        for release in [false, true] {
            let mut c = lang();
            c.args(["run", "--workers", &w.to_string()]);
            if release {
                c.arg("--release");
            }
            let out = c
                .arg("tests/run/sequence_fusion.bdg")
                .env("BADGER_GC_STRESS", "1")
                .env("BADGER_GC_VERIFY", "1")
                .env("BADGER_CHAOS", "20261016")
                .output()
                .unwrap();
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        }
    }
}

#[test]
fn stored_pipeline_is_reusable_and_preserves_snapshots_and_escape_boundaries() {
    for w in [1, 2, 4, 8] {
        for release in [false, true] {
            let mut c = lang();
            c.args(["run", "--workers", &w.to_string()]);
            if release {
                c.arg("--release");
            }
            let out = c
                .arg("tests/run/sequence_stored.bdg")
                .env("BADGER_GC_STRESS", "1")
                .env("BADGER_GC_VERIFY", "1")
                .env("BADGER_CHAOS", "2026101943")
                .output()
                .unwrap();
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            assert_eq!(String::from_utf8_lossy(&out.stdout), "stored sequences: ok\n");
        }
    }
}

#[test]
fn stored_pipeline_is_a_loop_before_dump_and_save() {
    let dir = std::env::temp_dir().join(format!("badger-stored-fusion-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("stored.bdg");
    std::fs::write(&source, "import std.seq\npub fn main() -> Int {\nlet s = seq.range(0, 100).map(fn(x: Int) { x * 2 }).filter(fn(x: Int) { x % 3 == 0 }).take(4)\nassert(s.fold(0, fn(a, x) { a + x }) == 36)\nassert(s.count() == 4)\n0\n}\n").unwrap();
    let out = lang().args(["run", "--release", "--dump-mir"]).arg(&source).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let mir = String::from_utf8_lossy(&out.stderr);
    let main = mir.split("\nfn 1 ").next().unwrap();
    assert!(
        !main.contains("Alloc {") && !main.contains("callee: Closure") && !main.contains("Tag("),
        "{main}"
    );
    let saved = dir.join("stored.bdgmir");
    assert!(lang().arg("compile").arg(&source).arg("-o").arg(&saved).status().unwrap().success());
    assert!(lang().args(["run", "--release"]).arg(saved).status().unwrap().success());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn visible_pipeline_is_a_loop_before_dump_and_save() {
    let dir = std::env::temp_dir().join(format!("badger-fusion-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("fusion.bdg");
    std::fs::write(&source, "import std.seq\npub fn main() -> Int {\nlet n = seq.range(0, 100).map(fn(x: Int) { x * 2 }).filter(fn(x: Int) { x % 3 == 0 }).fold(0, fn(a: Int, x: Int) { a + x })\nassert(n == 3366)\n0\n}\n").unwrap();
    let out = lang().args(["run", "--release", "--dump-mir"]).arg(&source).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let mir = String::from_utf8_lossy(&out.stderr);
    let main = mir.split("\nfn 1 ").next().unwrap();
    assert!(
        !main.contains("Alloc {") && !main.contains("callee: Closure") && !main.contains("Tag("),
        "{main}"
    );
    let saved = dir.join("fusion.bdgmir");
    assert!(lang().arg("compile").arg(&source).arg("-o").arg(&saved).status().unwrap().success());
    assert!(lang().args(["run", "--release"]).arg(saved).status().unwrap().success());
    std::fs::remove_dir_all(dir).unwrap();
}
