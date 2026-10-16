use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn command() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_lang"));
    cmd.current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."));
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("LANG_") {
            cmd.env_remove(key);
        }
    }
    cmd
}

fn success(out: Output) -> String {
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stderr).unwrap()
}

fn temp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("lang-niche-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn local_pointer_options_use_null_and_payload_before_dump_and_save() {
    let dir = temp("local");
    let source = dir.join("local.lang");
    std::fs::write(
        &source,
        r#"
pub fn main() -> Int {
    var total = 0
    for i in 0..100 {
        var value: Option<String> = Option.none
        if i % 2 == 0 { value = Option.some("hello") }
        total = total + value.unwrap_or("").len()
    }
    assert(total == 250)
    0
}
"#,
    )
    .unwrap();
    let mir = success(command().args(["run", "--release", "--dump-mir"]).arg(&source).output().unwrap());
    let main = mir.split("\nfn 1 ").next().unwrap();
    assert!(!main.contains("Alloc {"), "{main}");
    assert!(!main.contains("Tag("), "{main}");
    assert!(main.contains("Cmp { op: Ne") && main.contains("Int(0, Ptr)"), "{main}");
    let boxed = success(
        command()
            .env("LANG_NO_INLINE", "1")
            .args(["run", "--release", "--dump-mir"])
            .arg(&source)
            .output()
            .unwrap(),
    );
    assert!(boxed.split("\nfn 1 ").next().unwrap().contains("Alloc {"));
    let saved = dir.join("local.langmir");
    success(command().arg("compile").arg(&source).arg("-o").arg(&saved).output().unwrap());
    let loaded = success(command().args(["run", "--release", "--dump-mir"]).arg(saved).output().unwrap());
    assert!(!loaded.split("\nfn 1 ").next().unwrap().contains("Alloc {"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn pointer_option_roots_survive_gc_suspension_and_multiple_workers() {
    for release in [false, true] {
        for workers in [1, 4] {
            let mut cmd = command();
            cmd.arg("run").args(["--workers", &workers.to_string()]);
            if release {
                cmd.arg("--release");
            }
            success(
                cmd.arg("tests/run/option_niche.lang")
                    .env("LANG_GC_STRESS", "1")
                    .env("LANG_GC_VERIFY", "1")
                    .env("LANG_CHAOS", "20261016")
                    .output()
                    .unwrap(),
            );
        }
    }
}

#[test]
fn scalar_nested_and_returned_options_keep_boxed_layout() {
    let dir = temp("guards");
    let source = dir.join("guards.lang");
    std::fs::write(
        &source,
        r#"
fn returned() -> Option<String> { Option.some("payload") }
pub fn main() -> Int {
    let scalar = Option.some(0)
    assert(scalar.is_some())
    let absent: Option<String> = Option.none
    let nested = Option.some(absent)
    assert(nested.is_some())
    match nested { .some(v) => assert(v.is_none()), .none => assert(false) }
    assert(returned().unwrap_or("") == "payload")
    0
}
"#,
    )
    .unwrap();
    let mir = success(command().args(["run", "--release", "--dump-mir"]).arg(source).output().unwrap());
    let main = mir.split("\nfn 1 ").next().unwrap();
    assert!(main.matches("Alloc {").count() >= 2, "{main}");
    let returned = mir.split("\nfn ").find(|s| s.contains("guards.returned") && s.contains("Alloc {"));
    assert!(returned.is_some(), "{mir}");
    std::fs::remove_dir_all(dir).unwrap();
}
