//! Crash resistance on invalid input: programs from the corpus are mutated
//! (byte ranges deleted, lines duplicated or swapped, tokens inserted, text
//! truncated) and checked; those that still check are also compiled to
//! MIR. The compiler must answer with diagnostics or success, never panic
//! or die from a signal. `BADGER_FUZZ_SECS=N` runs for N seconds (default: a
//! fixed number of cases); `BADGER_FUZZ_SEED` picks the seed. A failing input
//! is saved under the system temp directory and named in the report.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn corpus() -> Vec<String> {
    let mut files = vec![];
    for dir in ["tests/run", "tests/reject", "tests/stress", "bench"] {
        if let Ok(rd) = std::fs::read_dir(root().join(dir)) {
            for e in rd.flatten() {
                if e.path().extension().is_some_and(|x| x == "bdg") {
                    files.push(e.path());
                }
            }
        }
    }
    files.sort();
    files.iter().filter_map(|p| std::fs::read_to_string(p).ok()).collect()
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next() % n as u64) as usize }
    }
}

const TOKENS: &[&str] = &[
    "{",
    "}",
    "(",
    ")",
    "[",
    "]",
    "<",
    ">",
    ",",
    ".",
    ":",
    "?",
    "!",
    "=",
    "=>",
    "->",
    "|>",
    "..",
    "fn ",
    "let ",
    "var ",
    "match ",
    "if ",
    "else ",
    "for ",
    "in ",
    "return ",
    "break",
    "loop ",
    "impl ",
    "struct ",
    "enum ",
    "pub ",
    "import std.io\n",
    "\"",
    "$\"",
    "{x}",
    "//",
    "/*",
    "*/",
    "0",
    "-1",
    "9999999999999999999999",
    "1.5",
    "'a'",
    "x",
    "self",
    "Self",
    "T",
    "Int",
    "String",
    "List<",
    "&mut ",
    "once ",
    "dyn ",
    "\n",
    "requires x > 0\n",
    "ensures result\n",
    "unsafe ",
    "system {",
    "pipeline [",
    ":sym",
    "100ms",
    "_",
    "|",
    "&",
    "%",
    "\\",
];

/// One mutation of `src`, at char boundaries.
fn mutate(src: &str, r: &mut Rng) -> String {
    let at = |r: &mut Rng, s: &str| {
        let mut i = r.below(s.len() + 1);
        while !s.is_char_boundary(i) {
            i -= 1;
        }
        i
    };
    let mut s = src.to_string();
    for _ in 0..1 + r.below(3) {
        match r.below(6) {
            0 => {
                let a = at(r, &s);
                let b = (a + 1 + r.below(40)).min(s.len());
                let mut b = b;
                while !s.is_char_boundary(b) {
                    b -= 1;
                }
                s.replace_range(a..b.max(a), "");
            }
            1 | 2 => {
                let a = at(r, &s);
                s.insert_str(a, TOKENS[r.below(TOKENS.len())]);
            }
            3 => {
                let lines: Vec<&str> = s.lines().collect();
                if !lines.is_empty() {
                    let i = r.below(lines.len());
                    let mut v: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
                    v.insert(i, lines[r.below(lines.len())].to_string());
                    s = v.join("\n");
                }
            }
            4 => {
                let mut v: Vec<String> = s.lines().map(str::to_string).collect();
                if v.len() > 1 {
                    let (i, j) = (r.below(v.len()), r.below(v.len()));
                    v.swap(i, j);
                    s = v.join("\n");
                }
            }
            _ => {
                let a = at(r, &s);
                s.truncate(a);
            }
        }
    }
    s
}

/// Problems with the compiler's answer to `args` (none: diagnostics or
/// success).
fn crashed(args: &[&str], file: &Path) -> Option<String> {
    let out = Command::new(env!("CARGO_BIN_EXE_badger")).args(args).arg(file).output().ok()?;
    let err = String::from_utf8_lossy(&out.stderr);
    match out.status.code() {
        Some(0) | Some(1) if !err.contains("panicked at") => None,
        code => Some(format!(
            "`badger {}` exited with {code:?}:\n{}",
            args.join(" "),
            err.lines().take(12).collect::<Vec<_>>().join("\n")
        )),
    }
}

#[test]
fn invalid_input_never_crashes() {
    let corpus = corpus();
    assert!(!corpus.is_empty());
    let seed: u64 =
        std::env::var("BADGER_FUZZ_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or_else(|| {
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
        });
    let secs = std::env::var("BADGER_FUZZ_SECS").ok().and_then(|v| v.parse::<u64>().ok());
    let cases = if secs.is_some() { usize::MAX } else { 240 };
    let deadline = Instant::now() + Duration::from_secs(secs.unwrap_or(600));
    let dir = std::env::temp_dir().join(format!("badger-fuzz-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let failures: Mutex<Vec<String>> = Mutex::new(vec![]);
    let next_case = std::sync::atomic::AtomicUsize::new(0);
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
    std::thread::scope(|sc| {
        for t in 0..threads {
            let (corpus, failures, next_case, dir) = (&corpus, &failures, &next_case, &dir);
            sc.spawn(move || {
                loop {
                    let case = next_case.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if case >= cases || Instant::now() > deadline || failures.lock().unwrap().len() >= 5 {
                        break;
                    }
                    let mut r = Rng(seed ^ (case as u64).wrapping_mul(0x2545_F491_4F6C_DD1D));
                    let src = &corpus[r.below(corpus.len())];
                    let text = mutate(src, &mut r);
                    let file = dir.join(format!("case{t}.bdg"));
                    std::fs::write(&file, &text).unwrap();
                    let mut problem = crashed(&["check"], &file);
                    if problem.is_none() {
                        let out = dir.join(format!("case{t}.bdgmir"));
                        problem = crashed(&["compile", "-o", out.to_str().unwrap()], &file);
                    }
                    if let Some(p) = problem {
                        let keep =
                            std::env::temp_dir().join(format!("badger-fuzz-failure-{seed}-{case}.bdg"));
                        let _ = std::fs::write(&keep, &text);
                        failures
                            .lock()
                            .unwrap()
                            .push(format!("case {case} (saved as {}):\n{p}", keep.display()));
                    }
                }
            });
        }
    });
    let _ = std::fs::remove_dir_all(&dir);
    let f = failures.into_inner().unwrap();
    assert!(f.is_empty(), "BADGER_FUZZ_SEED={seed}: {} crash(es)\n\n{}", f.len(), f.join("\n\n"));
}

/// The formatter on arbitrary text: it never panics, and formatting its
/// output again changes nothing.
#[test]
fn formatter_is_idempotent() {
    let corpus = corpus();
    let seed: u64 = std::env::var("BADGER_FUZZ_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(7);
    let mut r = Rng(seed);
    let mut inputs: Vec<String> = corpus.clone();
    for _ in 0..2000 {
        let src = &corpus[r.below(corpus.len())];
        inputs.push(mutate(src, &mut r));
    }
    for (i, text) in inputs.iter().enumerate() {
        let once = std::panic::catch_unwind(|| badger_syntax::format::format(text))
            .unwrap_or_else(|_| panic!("formatter panicked on input {i}:\n{text}"));
        let twice = badger_syntax::format::format(&once);
        assert_eq!(once, twice, "formatting is not idempotent on input {i}:\n{text}");
    }
}

/// Formatting a program does not change what it means: the same
/// diagnostics (or none) before and after, for every corpus program.
#[test]
fn formatting_preserves_meaning() {
    let dir = std::env::temp_dir().join(format!("badger-fmt-meaning-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let codes = |text: &str, name: &str| {
        let f = dir.join(name);
        std::fs::write(&f, text).unwrap();
        let out =
            Command::new(env!("CARGO_BIN_EXE_badger")).args(["check", "--json"]).arg(&f).output().unwrap();
        let s = String::from_utf8_lossy(&out.stdout).to_string();
        let mut c: Vec<String> =
            s.split("\"code\":").skip(1).map(|x| x.split(',').next().unwrap_or("").to_string()).collect();
        c.sort();
        (out.status.code(), c)
    };
    for (i, src) in corpus().iter().enumerate() {
        // squash indentation and spacing, then let the formatter restore it
        let messy: String = src
            .lines()
            .map(|l| if l.trim_start().starts_with("//") { l.to_string() } else { format!("  {}", l.trim()) })
            .collect::<Vec<_>>()
            .join("\n");
        let formatted = badger_syntax::format::format(&messy);
        assert_eq!(
            codes(src, "a.bdg"),
            codes(&formatted, "a.bdg"),
            "corpus file {i} changed meaning:\n{formatted}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
