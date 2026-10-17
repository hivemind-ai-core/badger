use std::{path::Path, process::Command};

fn lang() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_badger"));
    command.current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."));
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("BADGER_") {
            command.env_remove(key);
        }
    }
    command
}

#[test]
fn builder_nodes_keep_pointer_fields_and_finished_snapshots_alive() {
    for generation in ["0", "1"] {
        for workers in [1, 4, 8] {
            for release in [false, true] {
                let mut command = lang();
                command.args(["run", "--workers", &workers.to_string()]);
                if release {
                    command.arg("--release");
                }
                let output = command
                    .arg("tests/run/transient_maps.bdg")
                    .env("BADGER_GC_GENERATIONAL", generation)
                    .env("BADGER_GC_STRESS", "1")
                    .env("BADGER_GC_VERIFY", "1")
                    .env("BADGER_CHAOS", "20261017")
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "gen{generation} workers{workers} release{release}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                assert_eq!(output.stdout, b"transient maps: ok\n");
            }
        }
    }
}

#[test]
fn finished_builder_maps_survive_saved_mir() {
    let scratch = std::env::temp_dir().join(format!("lang-transient-maps-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let saved = scratch.join("maps.bdgmir");
    let output = lang().args(["compile", "tests/run/transient_maps.bdg", "-o"]).arg(&saved).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let output = lang()
        .args(["run", "--release", "--workers", "4"])
        .arg(&saved)
        .env("BADGER_GC_STRESS", "1")
        .env("BADGER_GC_VERIFY", "1")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(output.stdout, b"transient maps: ok\n");
    std::fs::remove_dir_all(scratch).unwrap();
}
