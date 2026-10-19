use badger_runtime::{desc::Kind, desc::TypeDesc, gc, list};

#[test]
fn presized_builder_directory_keeps_pages_across_minor_collections() {
    // The standalone runtime heap is process-global: run the scenario in a
    // child with collection stress, generations, and heap verification.
    if std::env::var_os("BADGER_BUILDER_DIR_CHILD").is_none() {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("BADGER_") {
                command.env_remove(name);
            }
        }
        // (uncaptured: a heap verification failure aborts the child)
        let output = command
            .arg("--nocapture")
            .env("BADGER_BUILDER_DIR_CHILD", "1")
            .env("BADGER_GC_GENERATIONAL", "1")
            .env("BADGER_GC_VERIFY", "1")
            .env("BADGER_GC_STRESS", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let desc = TypeDesc::new("List", Kind::List { elem: badger_runtime::desc::builtin::int() }).leak();
    {
        // Allocating the page collects (stress), so the directory is old
        // when the young page is stored into it.
        let roots = gc::Roots::new();
        let builder = roots.add(list::mark_builder(list::with_capacity(desc, 1)));
        list::push(builder, 7);
        // Finish without a collection: the consumed builder no longer
        // reaches the directory, and the finished list dies unseen.
        let _no_gc = gc::NoGc::new();
        list::finish(builder, desc);
    }
    // Minor collections, then a full one that clears weak directory slots:
    // no slot of the (old, dead) directory may name a freed page.
    for _ in 0..32 {
        gc::collect();
    }
}
