use badger_runtime::{buffer, desc, gc, list, string};

// One test owns this process's global runtime heap. Invoke this test binary
// separately under each generational/verification setting.
#[test]
fn shared_prefix_old_objects_and_empty_pool_chunks() {
    let d = desc::TypeDesc::new("List<String>", desc::Kind::List { elem: desc::builtin::string() }).leak();
    {
        let roots = gc::Roots::new();
        let mut latest = list::new(d);
        for i in 0..512 {
            let step = gc::Roots::new();
            step.add(latest);
            let s = step.add(string::new_string(&format!("value-{i}")));
            latest = list::append(latest, s, d);
        }
        let prefix = roots.add(latest);
        // Keep an interior element address stable while a longer shared
        // version is dropped and its invisible pages are collected.
        list::stabilize(prefix, 0);
        let address = list::addr(prefix, 0);
        for i in 512..2048 {
            let step = gc::Roots::new();
            step.add(latest);
            let s = step.add(string::new_string(&format!("value-{i}")));
            latest = list::append(latest, s, d);
        }
        for _ in 0..=gc::MAX_CONSECUTIVE_MINORS {
            gc::collect();
        }
        let mut count = 0;
        gc::for_each_object_of(desc::builtin::string(), &mut |_| count += 1);
        assert_eq!(count, 512);
        assert_eq!(list::addr(prefix, 0), address);
        assert_eq!(string::str_of(unsafe { *(address as *const u64) }), "value-0");
        let extra = roots.add(string::new_string("branch"));
        let branch = roots.add(list::append(prefix, extra, d));
        assert_eq!(string::str_of(list::get(branch, 512)), "branch");
        assert_eq!(string::str_of(list::get(prefix, 511)), "value-511");
        assert_eq!(list::len(prefix), 512);
        for _ in 0..=gc::MAX_CONSECUTIVE_MINORS {
            gc::collect();
        }
        assert_eq!(string::str_of(list::get(branch, 512)), "branch");
        let b = roots.add(buffer::new(desc::builtin::u8_(), 1_000_000));
        buffer::data(b).items.resize(1_000_000, 42);
        assert!(gc::memory_bytes().1 >= 1_000_000);
    }
    for _ in 0..=gc::MAX_CONSECUTIVE_MINORS {
        gc::collect();
    }
    assert_eq!(gc::stats().0, 0);
    assert_eq!(gc::memory_bytes().1, 0);
    {
        let roots = gc::Roots::new();
        for _ in 0..if std::env::var_os("BADGER_GC_STRESS").is_some() { 1000 } else { 100_000 } {
            roots.add(string::new_string("temporary"));
        }
        gc::collect();
        assert!(gc::pool_reserved_bytes() > 0);
    }
    for _ in 0..=gc::MAX_CONSECUTIVE_MINORS {
        gc::collect();
    }
    assert_eq!(gc::stats().0, 0, "dead old objects must eventually be collected");
    if !gc::verify() {
        assert_eq!(gc::pool_reserved_bytes(), 0, "empty pool chunks must be released");
    }
    // Allocate again after free-list purging and chunk release.
    let roots = gc::Roots::new();
    let s = roots.add(string::new_string("reused"));
    gc::collect();
    assert_eq!(string::str_of(s), "reused");
}
