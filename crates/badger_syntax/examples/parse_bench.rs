//! Parser benchmark: parse every file of a list repeatedly and report the
//! time per pass (min and median of the timed rounds) and throughput.
//!
//! `cargo run --release -p badger_syntax --example parse_bench -- FILE_LIST [ROUNDS]`
use badger_base::{Interner, SourceMap};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let list = std::fs::read_to_string(&args[0]).expect("a file listing one source path per line");
    let rounds: usize = args.get(1).and_then(|r| r.parse().ok()).unwrap_or(30);
    let mut sm = SourceMap::default();
    let files: Vec<_> = list
        .lines()
        .filter(|l| !l.is_empty())
        .map(|p| (p.to_string(), sm.add(p.to_string(), std::fs::read_to_string(p).unwrap())))
        .collect();
    let bytes: usize = files.iter().map(|(_, f)| sm.get(*f).text.len()).sum();
    let pass = |filter: &dyn Fn(&str) -> bool| {
        let mut int = Interner::new();
        let mut diags = vec![];
        let (mut ok, mut failed) = (0, 0);
        let t = std::time::Instant::now();
        for (p, f) in &files {
            if !filter(p) {
                continue;
            }
            match badger_syntax::parse(*f, &sm.get(*f).text, &mut int, &mut diags) {
                Ok(_) => ok += 1,
                Err(_) => failed += 1,
            }
        }
        (t.elapsed(), ok, failed)
    };
    for (label, filter) in [
        ("all", &(|_: &str| true) as &dyn Fn(&str) -> bool),
        ("std", &(|p: &str| p.contains("badger_hir/std/")) as &dyn Fn(&str) -> bool),
    ] {
        for _ in 0..3 {
            pass(filter);
        }
        let mut times = vec![];
        let mut counts = (0, 0);
        for _ in 0..rounds {
            let (t, ok, failed) = pass(filter);
            times.push(t);
            counts = (ok, failed);
        }
        times.sort();
        let n_bytes: usize =
            files.iter().filter(|(p, _)| filter(p)).map(|(_, f)| sm.get(*f).text.len()).sum();
        let median = times[times.len() / 2];
        println!(
            "{label}: {} files ({} parse, {} errors), {} KB: min {:.2} ms, median {:.2} ms, {:.1} MB/s",
            counts.0 + counts.1,
            counts.0,
            counts.1,
            n_bytes / 1024,
            times[0].as_secs_f64() * 1e3,
            median.as_secs_f64() * 1e3,
            n_bytes as f64 / median.as_secs_f64() / 1e6
        );
    }
    let _ = bytes;
}
