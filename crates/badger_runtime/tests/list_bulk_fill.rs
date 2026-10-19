use badger_runtime::{
    buffer,
    desc::{Kind, TypeDesc},
    gc, list,
};

#[test]
fn bulk_filled_pages_preserve_directory_capacity_and_persistent_appends() {
    // The standalone runtime heap is process-global. Isolate each GC policy.
    if std::env::var_os("BADGER_BULK_FILL_CHILD").is_none() {
        for generation in ["0", "1"] {
            for verify in ["0", "1"] {
                let mut command = std::process::Command::new(std::env::current_exe().unwrap());
                for (name, _) in std::env::vars_os() {
                    if name.to_string_lossy().starts_with("BADGER_") {
                        command.env_remove(name);
                    }
                }
                let output = command
                    .env("BADGER_BULK_FILL_CHILD", "1")
                    .env("BADGER_GC_GENERATIONAL", generation)
                    .env("BADGER_GC_VERIFY", verify)
                    .env("BADGER_GC_STRESS", "1")
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "gen={generation} verify={verify}: {}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
        return;
    }
    for elem in [
        TypeDesc::new("U8", Kind::Int { bits: 8, signed: false }).leak(),
        badger_runtime::desc::builtin::int(),
    ] {
        let desc = TypeDesc::new("List", Kind::List { elem }).leak();
        let cap = list::page_cap_for(elem.repr().size() as usize);
        for n in [0, 1, 15, 16, 17, cap - 1, cap, cap + 1, cap * 2 + 1] {
            let roots = gc::Roots::new();
            let original = roots.add(list::fill(desc, n, |i| (i % 251) as u64));
            if n != 0 {
                // Directory ABI: the second aligned word is capacity, distinct
                // from the initialized-slot count in the first word.
                let directory = unsafe { *(gc::obj(original).payload().add(8) as *const u64) };
                let capacity = unsafe { *(gc::obj(directory).payload().add(8) as *const u64) };
                assert_eq!(capacity as usize, n.div_ceil(cap));
            }
            let first = roots.add(list::append(original, 252, desc));
            let fork = roots.add(list::append(original, 253, desc));
            gc::collect();
            assert_eq!(list::len(original), n);
            assert_eq!(list::get(first, n), 252);
            assert_eq!(list::get(fork, n), 253);
            let array = roots.add(list::to_array(first, elem));
            for i in 0..n {
                assert_eq!(list::get(original, i), (i % 251) as u64);
                assert_eq!(list::get(first, i), (i % 251) as u64);
                assert_eq!(list::get(fork, i), (i % 251) as u64);
                assert_eq!(buffer::data(array).items.get(i), Some((i % 251) as u64));
            }
            assert_eq!(buffer::data(array).items.get(n), Some(252));
        }
    }
}
