use std::path::Path;
use std::process::Command;

fn lang() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_lang"));
    c.current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."));
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("LANG_") {
            c.env_remove(key);
        }
    }
    c
}

#[test]
fn packed_array_bits_codecs_and_roots() {
    for workers in [1, 4, 8] {
        for release in [false, true] {
            let mut c = lang();
            c.args(["run", "--workers", &workers.to_string()]);
            if release {
                c.arg("--release");
            }
            let out = c
                .arg("tests/run/packed_arrays.lang")
                .env("LANG_GC_STRESS", "1")
                .env("LANG_GC_VERIFY", "1")
                .env("LANG_CHAOS", "20261016")
                .output()
                .unwrap();
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        }
    }
}

#[test]
fn array_get_and_iteration_use_typed_loads() {
    let dir = std::env::temp_dir().join(format!("lang-array-load-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("arrays.lang");
    std::fs::write(&source, "import std.array\npub fn main() -> Int {\nlet a = array.from_list([7U8, 9U8])\nassert(a.get(1).unwrap_or(0U8) == 9U8)\nvar sum = 0U8\nfor x in a { sum = sum + x }\nassert(sum == 16U8)\n0\n}\n").unwrap();
    let out = lang().args(["run", "--release", "--dump-mir"]).arg(&source).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stderr);
    let main = text.split("\nfn 1 ").next().unwrap();
    assert!(
        !main.contains("lang_rt_i_buffer_get")
            && !main.contains("lang_rt_list_get")
            && !main.contains("Tag("),
        "{main}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn buffer_capacity_obeys_heap_budget() {
    let dir = std::env::temp_dir().join(format!("lang-buffer-budget-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("budget.lang");
    std::fs::write(&source, "import std.buffer\npub fn main() -> Int { let b = buffer.with_capacity<U8>(10000000)\nb.push(1U8)\n0 }\n").unwrap();
    let out =
        lang().args(["run", "--release"]).arg(source).env("LANG_HEAP_MAX_BYTES", "1000000").output().unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("heap budget exceeded"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn dead_array_roots_release_storage_before_the_next_allocation() {
    let dir = std::env::temp_dir().join(format!("lang-array-lifetime-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("lifetime.lang");
    std::fs::write(&source, "import std.array\npub fn main() -> Int {\nlet a = array.build(2000000, fn(_: Int) { 7U8 })\nassert(a.get(1999999).unwrap_or(0U8) == 7U8)\nlet b = array.build(2000000, fn(_: Int) { 9U8 })\nassert(b.get(1999999).unwrap_or(0U8) == 9U8)\n0\n}\n").unwrap();
    for workers in [1, 4] {
        let out = lang()
            .args(["run", "--release", "--workers", &workers.to_string()])
            .arg(&source)
            .env("LANG_HEAP_MAX_BYTES", "3000000")
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }
    std::fs::remove_dir_all(dir).unwrap();
}
