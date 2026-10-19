use std::collections::HashMap;

use badger_runtime::{
    desc::{Kind, TypeDesc, builtin},
    gc, hamt, map, string, value,
};

/// Run `body` in child processes under each collector policy: the
/// standalone runtime heap is process-global.
fn in_children(name: &str, flag: &str, body: fn()) {
    if std::env::var_os(flag).is_some() {
        return body();
    }
    for generation in ["0", "1"] {
        for verify in ["0", "1"] {
            for stress in ["0", "1"] {
                let mut command = std::process::Command::new(std::env::current_exe().unwrap());
                for (key, _) in std::env::vars_os() {
                    if key.to_string_lossy().starts_with("BADGER_") {
                        command.env_remove(key);
                    }
                }
                let output = command
                    .args(["--exact", name])
                    .env(flag, "1")
                    .env("BADGER_GC_GENERATIONAL", generation)
                    .env("BADGER_GC_VERIFY", verify)
                    .env("BADGER_GC_STRESS", stress)
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "gen={generation} verify={verify} stress={stress}: {}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
    }
}

fn s(w: u64) -> String {
    string::str_of(w).to_string()
}

fn contents(m: u64) -> HashMap<String, String> {
    map::entries(m).into_iter().map(|(k, v)| (s(k), s(v))).collect()
}

fn keys(m: u64) -> Vec<String> {
    map::entries(m).into_iter().map(|(k, _)| s(k)).collect()
}

/// The entries a cursor visits, in order.
fn walk(m: u64) -> Vec<(u64, u64)> {
    let r = gc::Roots::new();
    let c = r.add(map::cursor(m));
    let mut out = vec![];
    while hamt::cursor_next(c) == 1 {
        let entry = unsafe { *(gc::obj(c).payload() as *const u64) } as *const u64;
        out.push(unsafe { (*entry.add(1), *entry.add(2)) });
    }
    out
}

#[test]
fn small_and_large_maps_keep_versions_order_and_gc_edges() {
    in_children(
        "small_and_large_maps_keep_versions_order_and_gc_edges",
        "BADGER_COLLECTIONS_TEST_CHILD",
        small_and_large_maps,
    );
}

fn small_and_large_maps() {
    let desc = TypeDesc::new("Map", Kind::Map { key: builtin::string(), val: builtin::string() }).leak();
    let set_desc = TypeDesc::new("Set", Kind::Set { elem: builtin::string() }).leak();
    let roots = gc::Roots::new();
    // Grow across the flat/trie boundary, keeping every version.
    let mut m = roots.add(map::new(desc));
    let mut model = HashMap::new();
    let mut versions = vec![(m, model.clone())];
    // (an inner `Roots` unroots everything added after it when dropped:
    // outer roots are added only once inner ones are gone)
    for i in 0..24u64 {
        m = {
            let r = gc::Roots::new();
            let k = r.add(string::new_string(&format!("key{}", (i * 7) % 13)));
            let v = r.add(string::new_string(&format!("value{i}")));
            map::insert(m, k, v)
        };
        roots.add(m);
        model.insert(format!("key{}", (i * 7) % 13), format!("value{i}"));
        versions.push((m, model.clone()));
        gc::collect();
    }
    // Shrink across it again, one removal at a time.
    for i in 0..13u64 {
        m = {
            let r = gc::Roots::new();
            let k = r.add(string::new_string(&format!("key{}", (i * 5) % 13)));
            map::remove(m, k)
        };
        roots.add(m);
        model.remove(&format!("key{}", (i * 5) % 13));
        versions.push((m, model.clone()));
        gc::collect();
    }
    for &(version, ref reference) in &versions {
        assert_eq!(map::len(version), reference.len());
        assert_eq!(&contents(version), reference);
        assert_eq!(walk(version), map::entries(version));
        for (k, v) in reference {
            let r = gc::Roots::new();
            let kw = r.add(string::new_string(k));
            assert_eq!(map::get(version, kw).map(s).as_deref(), Some(v.as_str()));
        }
        let r = gc::Roots::new();
        let absent = r.add(string::new_string("absent"));
        assert_eq!(map::get(version, absent), None);
        let unchanged = r.add(map::remove(version, absent));
        assert_eq!(keys(unchanged), keys(version));
        assert!(map::eq(unchanged, version));
    }
    // A map's iteration order and equality depend on its content alone,
    // whatever the insertion order, builder or persistent.
    for &(version, ref reference) in &versions {
        let mut pairs: Vec<_> = reference.iter().collect();
        for round in 0..3 {
            let r = gc::Roots::new();
            let built = r.add(map::new(desc));
            let mut persistent = r.add(map::new(desc));
            match round {
                1 => pairs.reverse(),
                2 => pairs.sort(),
                _ => {}
            }
            for (k, v) in &pairs {
                persistent = {
                    let t = gc::Roots::new();
                    let kw = t.add(string::new_string(k));
                    let vw = t.add(string::new_string(v));
                    map::insert_mut(built, kw, vw);
                    map::insert(persistent, kw, vw)
                };
                r.add(persistent);
            }
            gc::collect();
            assert_eq!(keys(built), keys(version));
            assert_eq!(keys(persistent), keys(version));
            assert!(map::eq(built, version) && map::eq(version, built));
            assert!(value::eq(desc, persistent, version));
            if let Some((k, _)) = pairs.first() {
                let t = gc::Roots::new();
                let kw = t.add(string::new_string(k));
                let vw = t.add(string::new_string("other"));
                let changed = t.add(map::insert(built, kw, vw));
                assert!(!map::eq(changed, version) && !map::eq(version, changed));
                let removed = t.add(map::remove(built, kw));
                let kw2 = t.add(string::new_string("fresh"));
                let swapped = t.add(map::insert(removed, kw2, vw));
                assert!(!map::eq(swapped, version) && !map::eq(version, swapped));
            }
        }
    }
    // Sets share the representation; their value is the unit.
    let mut set = roots.add(map::new(set_desc));
    let mut members = vec![];
    for i in 0..20u64 {
        let next = {
            let r = gc::Roots::new();
            let k = r.add(string::new_string(&format!("m{i}")));
            let next = map::insert(set, k, 0);
            assert!(map::get(next, k).is_some() && map::get(set, k).is_none());
            next
        };
        roots.add(next);
        members.push(format!("m{i}"));
        assert_eq!(map::len(next), members.len());
        assert_eq!(map::len(set), members.len() - 1);
        set = next;
        let mut got = keys(set);
        got.sort();
        let mut want = members.clone();
        want.sort();
        assert_eq!(got, want);
        gc::collect();
    }
}

#[test]
fn colliding_and_owned_small_maps() {
    in_children("colliding_and_owned_small_maps", "BADGER_COLLECTIONS_OWNED_CHILD", colliding_and_owned);
}

fn colliding_and_owned() {
    let int = builtin::int();
    let float = builtin::float();
    let desc = TypeDesc::new("Map", Kind::Map { key: int, val: int }).leak();
    let roots = gc::Roots::new();
    // Ownership epochs edit flat roots in place only while unshared.
    for interval in [1, 3, 8, 64] {
        let mut m = roots.add(map::new(desc));
        let mut model = HashMap::new();
        let mut versions = vec![];
        for step in 0..40u64 {
            let k = (step * 5) % 11;
            m = roots.add(map::insert_owned(m, k, step, false));
            model.insert(k, step);
            if (step + 1) % interval == 0 {
                map::freeze(m);
                versions.push((m, model.clone()));
            }
            if step % 7 == 0 {
                gc::collect();
            }
        }
        for (version, reference) in versions {
            assert_eq!(map::len(version), reference.len());
            for k in 0..11 {
                assert_eq!(map::get(version, k), reference.get(&k).copied());
            }
        }
    }
    // Floats are not reflexive (NaN): equality of a map with itself still
    // compares every entry.
    let fdesc = TypeDesc::new("Map", Kind::Map { key: int, val: float }).leak();
    for n in [3u64, 20] {
        let mut m = roots.add(map::new(fdesc));
        for k in 0..n {
            m = roots.add(map::insert(m, k, (k as f64).to_bits()));
        }
        assert!(map::eq(m, m));
        let nan = roots.add(map::insert(m, 1, f64::NAN.to_bits()));
        assert!(!map::eq(nan, nan));
        assert!(!map::eq(nan, m));
    }
}
