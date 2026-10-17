use std::path::Path;
use std::process::Command;

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
fn local_scalar_enums_have_no_wrapper_allocations_before_dump_or_save() {
    let dir = std::env::temp_dir().join(format!("badger-scalar-enums-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("scalar.bdg");
    std::fs::write(
        &source,
        r#"
pub fn main() -> Int {
    var sum = 0
    for i in 0..100 {
        var a = Option.some(i)
        let old = a
        a = if i % 2 == 0 { Option.some(i + 1) } else { Option.none }
        sum = sum + old.unwrap_or(-1) + a.unwrap_or(0)
    }
    assert(sum == 7450)
    0
}
"#,
    )
    .unwrap();
    let out = lang().args(["run", "--release", "--dump-mir"]).arg(&source).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stderr);
    let main = text.split("\nfn 1 ").next().unwrap();
    assert!(!main.contains("Alloc {") && !main.contains("Tag("), "{main}");
    let saved = dir.join("scalar.bdgmir");
    assert!(lang().arg("compile").arg(&source).arg("-o").arg(&saved).status().unwrap().success());
    let out = lang().args(["run", "--release", "--dump-mir"]).arg(saved).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(!String::from_utf8_lossy(&out.stderr).split("\nfn 1 ").next().unwrap().contains("Alloc {"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn mixed_payloads_aliases_and_gc() {
    for workers in [1, 4, 8] {
        for release in [false, true] {
            let mut c = lang();
            c.args(["run", "--workers", &workers.to_string()]);
            if release {
                c.arg("--release");
            }
            let out = c
                .arg("tests/run/scalar_enums.bdg")
                .env("BADGER_GC_STRESS", "1")
                .env("BADGER_GC_VERIFY", "1")
                .env("BADGER_CHAOS", "20261016")
                .output()
                .unwrap();
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        }
    }
}
