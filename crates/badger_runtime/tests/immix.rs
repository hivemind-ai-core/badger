use badger_runtime::{gc, string};

// One test owns this process's global runtime heap; it reruns itself under
// each generational/verification setting.
#[test]
fn mixed_lifetimes_reuse_dead_lines_in_place() {
    if std::env::var_os("BADGER_IMMIX_TEST_CHILD").is_none() {
        for generation in ["0", "1"] {
            for verify in ["0", "1"] {
                let mut command = std::process::Command::new(std::env::current_exe().unwrap());
                for (key, _) in std::env::vars_os() {
                    if key.to_string_lossy().starts_with("BADGER_") {
                        command.env_remove(key);
                    }
                }
                let output = command
                    .args(["--exact", "mixed_lifetimes_reuse_dead_lines_in_place"])
                    .env("BADGER_IMMIX_TEST_CHILD", "1")
                    .env("BADGER_GC_GENERATIONAL", generation)
                    .env("BADGER_GC_VERIFY", verify)
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "generational {generation} verify {verify}: {} {}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
        return;
    }
    const COUNT: usize = 60_000;
    {
        // Every eighth string lives on: the survivors are scattered over
        // every block, so no block is reclaimed whole and only its dead
        // lines can be reused.
        let roots = gc::Roots::new();
        let mut kept = vec![];
        for i in 0..COUNT {
            let s = string::new_string(&format!("survivor-{i:06}"));
            if i % 8 == 0 {
                kept.push(roots.add(s));
            }
        }
        for _ in 0..=gc::MAX_CONSECUTIVE_MINORS {
            gc::collect();
        }
        let mut settled = 0;
        for round in 0..8 {
            {
                let churn = gc::Roots::new();
                for j in 0..COUNT {
                    let s = string::new_string(&format!("temporary-{round}-{j}"));
                    if j % 1000 == 0 {
                        churn.add(s);
                    }
                }
                gc::collect();
            }
            gc::collect();
            // nothing moved, and no reused line overlapped a survivor
            for (i, &s) in kept.iter().enumerate() {
                assert_eq!(string::str_of(s), format!("survivor-{:06}", i * 8));
            }
            if round == 1 {
                settled = gc::pool_reserved_bytes();
            } else if round > 1 && !gc::verify() {
                assert_eq!(
                    gc::pool_reserved_bytes(),
                    settled,
                    "round {round}: dead lines must be reused in place"
                );
            }
        }
        // (old churn survivors are found by a full collection)
        for _ in 0..=gc::MAX_CONSECUTIVE_MINORS {
            gc::collect();
        }
        assert_eq!(gc::stats().0, kept.len());
    }
    for _ in 0..=gc::MAX_CONSECUTIVE_MINORS {
        gc::collect();
    }
    assert_eq!(gc::stats().0, 0);
    if !gc::verify() {
        assert_eq!(gc::pool_reserved_bytes(), 0, "blocks with no live line must be reclaimed whole");
    }
}
