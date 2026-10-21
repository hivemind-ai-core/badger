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
fn a_point_free_callback_is_called_directly() {
    // A function passed by name lowers to a *static* closure (a null
    // environment with that function's code), which the caller did not
    // allocate. The program records which function such an object calls, so
    // the inliner sees through the call and the function is pruned when
    // nothing else uses it — no `callee: Closure` survives.
    let dir = std::env::temp_dir().join(format!("badger-pointfree-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("pointfree.bdg");
    std::fs::write(
        &source,
        "import std.list\nimport std.seq\nfn same(x: Int) -> Int { x }\n\
         fn add(a: Int, x: Int) -> Int { a + x }\n\
         pub fn main() -> Int {\n    let xs = [1, 2, 3, 4, 5]\n    \
         let n = xs.seq().map(same).fold(0, add)\n    assert(n == 15)\n    0\n}\n",
    )
    .unwrap();
    let out = lang().args(["run", "--release", "--dump-mir"]).arg(&source).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let mir = String::from_utf8_lossy(&out.stderr);
    assert!(!mir.contains("callee: Closure"), "{mir}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn count_of_a_counted_source_skips_the_loop_only_when_skipping_is_unobservable() {
    let dir = std::env::temp_dir().join(format!("badger-count-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let case = |name: &str, callback: &str| {
        let source = dir.join(format!("{name}.bdg"));
        std::fs::write(
            &source,
            format!(
                "import std.list\nimport std.seq\nfn double(x: Int) -> Int {{ x * 2 }}\n\
                 fn same(x: Int) -> Int {{ x }}\n\
                 pub fn main() -> Int {{\n    let xs = [1, 2, 3, 4, 5]\n    \
                 let n = xs.seq().map({callback}).count()\n    assert(n == 5)\n    0\n}}\n"
            ),
        )
        .unwrap();
        let out = lang().args(["run", "--release", "--dump-mir"]).arg(&source).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let mir = String::from_utf8_lossy(&out.stderr);
        let main = mir.split("\nfn 1 ").next().unwrap().to_string();
        main
    };
    // A counted source answers `count` from its length: no loop, no per-item
    // `list_get`.
    let pure = case("pure", "fn(x: Int) { x }");
    assert!(!pure.contains("badger_rt_list_get"), "{pure}");
    // Checked arithmetic can defect, so every item must still be visited.
    let checked = case("checked", "fn(x: Int) { x * 2 }");
    assert!(checked.contains("badger_rt_list_get"), "{checked}");
    // A named callback is read too: `same`'s body is pure and infallible, so
    // the count is still the list's length…
    let named_pure = case("named_pure", "same");
    assert!(!named_pure.contains("badger_rt_list_get"), "{named_pure}");
    // …while a named callback with checked arithmetic is not.
    let named = case("named", "double");
    assert!(named.contains("badger_rt_list_get"), "{named}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_search_terminal_fuses_into_the_loop() {
    let dir = std::env::temp_dir().join(format!("badger-search-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("search.bdg");
    std::fs::write(
        &source,
        "import std.seq\npub fn main() -> Int {\n    \
         let found = seq.range(0, 100).map(fn(x: Int) { x }).any(fn(x: Int) { x == 3 })\n    \
         let first = seq.range(0, 100).map(fn(x: Int) { x }).find(fn(x: Int) { x == 3 })\n    \
         assert(found && first == Option.some(3))\n    0\n}\n",
    )
    .unwrap();
    let out = lang().args(["run", "--release", "--dump-mir"]).arg(&source).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let mir = String::from_utf8_lossy(&out.stderr);
    let main = mir.split("\nfn 1 ").next().unwrap().to_string();
    assert!(!main.contains("callee: Closure"), "{main}");
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
