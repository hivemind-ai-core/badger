//! Read-only performance probes; see reports/performance-audit-2026-10-16.
use lang_runtime::{buffer, desc, gc, list, string};

fn rss_kib() -> usize {
    std::fs::read_to_string("/proc/self/status")
        .unwrap()
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:").and_then(|s| s.split_whitespace().next()?.parse().ok()))
        .unwrap()
}

fn snapshot(label: &str) {
    let (objects, pressure, collections) = gc::stats();
    println!(
        "{label}: objects={objects} pressure_bytes={pressure} collections={collections} rss_kib={}",
        rss_kib()
    );
}

fn main() {
    let mode = std::env::args().nth(1).expect("buffer | prefix | pool | old");
    match mode.as_str() {
        "buffer" => {
            snapshot("before");
            let roots = gc::Roots::new();
            let w = roots.add(buffer::new(desc::builtin::u8_(), 8_000_000));
            buffer::data(w).items.resize(8_000_000, 42);
            snapshot("8m_byte_elements");
            assert_eq!(buffer::items(w).len(), 8_000_000);
            assert_eq!(buffer::items(w)[7_999_999], 42);
        }
        "prefix" => {
            let d = desc::TypeDesc::new("List<String>", desc::Kind::List { elem: desc::builtin::string() })
                .leak();
            let roots = gc::Roots::new();
            let mut latest = list::new(d);
            for i in 0..512 {
                gc::push_root(latest);
                let value = string::new_string(&format!("value-{i}"));
                latest = list::append(latest, value, d);
                gc::pop_root();
            }
            let prefix = roots.add(latest);
            for i in 512..2048 {
                gc::push_root(latest);
                let value = string::new_string(&format!("value-{i}"));
                latest = list::append(latest, value, d);
                gc::pop_root();
            }
            assert_eq!(list::len(prefix), 512);
            assert_eq!(list::len(latest), 2048);
            gc::collect();
            let mut strings = 0;
            gc::for_each_object_of(desc::builtin::string(), &mut |_| {
                strings += 1;
            });
            println!("prefix_len=512 retained_strings={strings}");
            assert_eq!(strings, 2048);
            drop(roots);
            gc::collect();
            snapshot("after_drop_prefix");
        }
        "pool" => {
            snapshot("before");
            {
                let roots = gc::Roots::new();
                for _ in 0..500_000 {
                    roots.add(gc::alloc(desc::builtin::string(), 32));
                }
                snapshot("live_peak");
            }
            gc::collect();
            snapshot("after_collection");
            assert_eq!(gc::stats().0, 0);
        }
        "old" => {
            {
                let roots = gc::Roots::new();
                for _ in 0..100_000 {
                    roots.add(string::new_string("temporary"));
                }
                gc::collect();
                snapshot("promoted");
            }
            for _ in 0..5 {
                gc::collect();
            }
            snapshot("after_drop_and_five_collections");
        }
        _ => panic!("unknown mode"),
    }
}
