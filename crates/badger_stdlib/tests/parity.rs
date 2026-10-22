//! The embedded module list and the sources on disk agree, and a module
//! name names one module.

use std::collections::BTreeSet;
use std::path::Path;

/// Every `.bdg` file in a namespace's directory is listed in `STD_MODULES`,
/// and every listed module has its file: the compiler loads only what is
/// listed, so a file without an entry is a module nobody can import.
#[test]
fn the_list_and_the_files_agree() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut on_disk: BTreeSet<(String, String)> = BTreeSet::new();
    for root in badger_stdlib::ROOTS {
        let ns = dir.join(root);
        for entry in std::fs::read_dir(&ns).unwrap_or_else(|e| panic!("{}: {e}", ns.display())) {
            let path = entry.expect("readable directory entry").path();
            if path.extension().is_some_and(|x| x == "bdg") {
                let name = path.file_stem().expect("a named file").to_string_lossy().into_owned();
                on_disk.insert(((*root).to_string(), name));
            }
        }
    }
    let listed: BTreeSet<(String, String)> =
        badger_stdlib::names().map(|(r, n)| (r.to_string(), n.to_string())).collect();
    let missing_from_list: Vec<&(String, String)> = on_disk.difference(&listed).collect();
    let missing_on_disk: Vec<&(String, String)> = listed.difference(&on_disk).collect();
    assert!(
        missing_from_list.is_empty() && missing_on_disk.is_empty(),
        "module list and sources differ:\n  on disk, not listed: {missing_from_list:?}\n  listed, not on disk: {missing_on_disk:?}"
    );
}

/// A module name is its last path segment, so two modules with the same name
/// in different namespaces would make `import stdx.http` and lang items
/// (`system.provider`) ambiguous.
#[test]
fn module_names_are_unique() {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (root, name) in badger_stdlib::names() {
        assert!(seen.insert(name), "`{root}.{name}` reuses a module name");
    }
}
