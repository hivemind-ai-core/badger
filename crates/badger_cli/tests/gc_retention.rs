use std::{path::Path, process::Command};

#[test]
fn full_sweeps_keep_workers_stopped_until_task_results_and_lists_are_safe() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for generation in ["0", "1"] {
        for case in ["try_all_finishing", "channel_conservation"] {
            let source = root.join(format!("tests/stress/{case}.bdg"));
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_badger"));
            for (key, _) in std::env::vars_os() {
                if key.to_string_lossy().starts_with("BADGER_") {
                    cmd.env_remove(key);
                }
            }
            let out = cmd
                .current_dir(&root)
                .args(["run", "--release", "--workers", "4"])
                .arg(&source)
                .env("BADGER_GC_GENERATIONAL", generation)
                .env("BADGER_GC_VERIFY", "1")
                .env("BADGER_CHAOS", "5")
                .output()
                .unwrap();
            assert!(out.status.success(), "{case} gen{generation}: {}", String::from_utf8_lossy(&out.stderr));
            assert_eq!(
                String::from_utf8_lossy(&out.stdout),
                std::fs::read_to_string(source.with_extension("out")).unwrap()
            );
        }
    }
}
