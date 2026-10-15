//! Concurrency stress: `tests/stress/*.lang` are self-checking programs
//! (conservation, exactly-once, cleanup invariants) that must print
//! `NAME.out` under every scheduling configuration.
//!
//! The quick tier runs each program on 1, 4, and 8 workers with chaos
//! scheduling (`LANG_CHAOS`), heap verification (`LANG_GC_VERIFY`), and
//! optimized code. Set `LANG_STRESS_SECS=N` for a soak run that cycles
//! through randomized configurations (also `LANG_GC_STRESS` and binary
//! MIR) for N seconds; `LANG_STRESS_SEED` picks its starting seed. Every
//! failure names the program and configuration that reproduces it.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn programs() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(root().join("tests/stress"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "lang"))
        .collect();
    v.sort();
    assert!(!v.is_empty(), "no stress programs");
    v
}

#[derive(Clone, Debug)]
struct Config {
    workers: u32,
    chaos: Option<u64>,
    verify: bool,
    gc_stress: bool,
    release: bool,
    /// Run through `lang compile` and the binary MIR file.
    compiled: bool,
}

impl Config {
    fn describe(&self) -> String {
        let mut env = vec![];
        if let Some(s) = self.chaos {
            env.push(format!("LANG_CHAOS={s}"));
        }
        if self.verify {
            env.push("LANG_GC_VERIFY=1".into());
        }
        if self.gc_stress {
            env.push("LANG_GC_STRESS=1".into());
        }
        let flags = format!("--workers {}{}", self.workers, if self.release { " --release" } else { "" });
        let how = if self.compiled { " (via lang compile)" } else { "" };
        format!("{} lang run {flags}{how}", env.join(" "))
    }
}

const TIMEOUT: Duration = Duration::from_secs(180);

/// Run one program under one configuration: `Err` describes the failure.
fn run(program: &Path, cfg: &Config, scratch: &Path) -> Result<(), String> {
    let expected = std::fs::read_to_string(program.with_extension("out")).unwrap_or_default();
    let target = if cfg.compiled {
        let out = scratch.join(program.file_stem().unwrap()).with_extension("langmir");
        let st = Command::new(env!("CARGO_BIN_EXE_lang"))
            .current_dir(root())
            .args(["compile"])
            .arg(program)
            .arg("-o")
            .arg(&out)
            .output()
            .unwrap();
        if !st.status.success() {
            return Err(format!("compile failed: {}", String::from_utf8_lossy(&st.stderr)));
        }
        out
    } else {
        program.to_path_buf()
    };
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_lang"));
    cmd.current_dir(root()).args(["run", "--workers", &cfg.workers.to_string()]);
    if cfg.release {
        cmd.arg("--release");
    }
    cmd.arg(&target).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(s) = cfg.chaos {
        cmd.env("LANG_CHAOS", s.to_string());
    }
    if cfg.verify {
        cmd.env("LANG_GC_VERIFY", "1");
    }
    if cfg.gc_stress {
        cmd.env("LANG_GC_STRESS", "1");
    }
    let mut child = cmd.spawn().unwrap();
    let mut so = child.stdout.take().unwrap();
    let mut se = child.stderr.take().unwrap();
    let out_t = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = so.read_to_string(&mut s);
        s
    });
    let err_t = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = se.read_to_string(&mut s);
        s
    });
    let start = Instant::now();
    let status = loop {
        if let Some(st) = child.try_wait().unwrap() {
            break Some(st);
        }
        if start.elapsed() > TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let stdout = out_t.join().unwrap();
    let stderr = err_t.join().unwrap();
    match status {
        None => Err(format!("hung for {}s\n--- stderr\n{stderr}", TIMEOUT.as_secs())),
        Some(st) if !st.success() || stdout != expected => Err(format!(
            "exit {:?}\n--- stdout\n{stdout}--- expected\n{expected}--- stderr\n{stderr}",
            st.code()
        )),
        Some(_) => Ok(()),
    }
}

/// Run every (program, configuration) pair on a few threads.
fn run_matrix(jobs: Vec<(PathBuf, Config)>) -> Vec<String> {
    let scratch = std::env::temp_dir().join(format!("lang-stress-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let jobs = std::sync::Mutex::new(jobs);
    let failures = std::sync::Mutex::new(vec![]);
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(2, 4);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let Some((p, cfg)) = jobs.lock().unwrap().pop() else { break };
                    // compiled programs get their own output directory
                    let dir =
                        scratch.join(format!("{:?}", std::thread::current().id()).replace(['(', ')'], ""));
                    std::fs::create_dir_all(&dir).unwrap();
                    if let Err(e) = run(&p, &cfg, &dir) {
                        failures.lock().unwrap().push(format!(
                            "{}\n  reproduce: {} {}\n{e}",
                            p.display(),
                            cfg.describe(),
                            p.display()
                        ));
                    }
                }
            });
        }
    });
    let _ = std::fs::remove_dir_all(&scratch);
    failures.into_inner().unwrap()
}

#[test]
fn stress_programs() {
    let base =
        Config { workers: 1, chaos: None, verify: false, gc_stress: false, release: false, compiled: false };
    let configs = vec![
        base.clone(),
        Config { chaos: Some(1), ..base.clone() },
        Config { workers: 4, chaos: Some(2), ..base.clone() },
        Config { workers: 4, chaos: Some(3), ..base.clone() },
        Config { workers: 8, chaos: Some(4), ..base.clone() },
        Config { workers: 4, chaos: Some(5), verify: true, ..base.clone() },
        Config { workers: 4, chaos: Some(6), release: true, ..base.clone() },
        // ordinary multi-worker scheduling (no chaos), plain and optimized
        Config { workers: 4, ..base.clone() },
        Config { workers: 8, release: true, ..base.clone() },
    ];
    let mut jobs = vec![];
    for p in programs() {
        for c in &configs {
            jobs.push((p.clone(), c.clone()));
        }
    }
    let failures = run_matrix(jobs);
    assert!(failures.is_empty(), "{} stress failure(s):\n\n{}", failures.len(), failures.join("\n\n"));
}

/// `LANG_STRESS_SECS=N`: randomized configurations for N seconds.
#[test]
fn stress_soak() {
    let Some(secs) = std::env::var("LANG_STRESS_SECS").ok().and_then(|v| v.parse::<u64>().ok()) else {
        return;
    };
    let mut seed: u64 =
        std::env::var("LANG_STRESS_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or_else(|| {
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
        });
    let mut next = || {
        seed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = seed;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    let deadline = Instant::now() + Duration::from_secs(secs);
    let progs = programs();
    let mut failures = vec![];
    let mut runs = 0;
    while Instant::now() < deadline && failures.len() < 10 {
        let mut jobs = vec![];
        for p in &progs {
            let r = next();
            let cfg = Config {
                workers: [1, 2, 3, 4, 8][(r % 5) as usize],
                chaos: if r & 0x10 == 0 { Some(next() % 1_000_000) } else { None },
                verify: r & 0x20 == 0,
                gc_stress: r & 0x3c0 == 0,
                release: r & 0x400 == 0,
                compiled: r & 0x800 == 0,
            };
            jobs.push((p.clone(), cfg));
        }
        runs += jobs.len();
        failures.extend(run_matrix(jobs));
    }
    assert!(
        failures.is_empty(),
        "{} soak failure(s) in {runs} runs:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
    eprintln!("stress soak: {runs} runs, no failures");
}
