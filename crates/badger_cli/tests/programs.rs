//! End-to-end tests: `tests/run/*.bdg` must run and print `NAME.out`
//! (exit code `NAME.exit`, default 0); `tests/reject/*.bdg` must fail to
//! check with every diagnostic code listed in `// expect-error: CODE` lines.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn lang() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_badger"));
    c.current_dir(root());
    c
}

fn files(dir: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(root().join(dir))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "bdg"))
        .collect();
    v.sort();
    v
}

#[test]
fn run_programs() {
    run_all(&[]);
}

/// The same programs and expectations with four worker threads.
#[test]
fn run_programs_multicore() {
    run_all(&["--workers", "4"]);
}

/// Scenario 15: producers migrate between workers; per-producer order holds.
#[test]
fn multicore_migration() {
    let out = lang().args(["run", "tests/projects/multicore"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(
        text,
        "workers: 4\nper-producer order preserved: true\nmessages: 12000\nsome producer moved between workers: true\n"
    );
}

/// Run every `tests/run` program with `flags` (on a few threads) and
/// compare stdout and exit code with its expectations. `compiled` runs
/// each through `badger compile` and its binary MIR file instead.
fn run_corpus(flags: &[&str], compiled: bool) {
    let dir = std::env::temp_dir().join(format!(
        "badger-corpus-{}-{}{}",
        std::process::id(),
        flags.join(""),
        if compiled { "-mir" } else { "" }
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let jobs = std::sync::Mutex::new(files("tests/run"));
    let failures = std::sync::Mutex::new(vec![]);
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| {
                loop {
                    let Some(f) = jobs.lock().unwrap().pop() else { break };
                    let expected = std::fs::read_to_string(f.with_extension("out")).unwrap_or_default();
                    let code: i32 = std::fs::read_to_string(f.with_extension("exit"))
                        .map(|s| s.trim().parse().unwrap())
                        .unwrap_or(0);
                    let target = if compiled {
                        let bin = dir.join(f.file_stem().unwrap()).with_extension("bdgmir");
                        let c = lang().arg("compile").arg(&f).arg("-o").arg(&bin).output().unwrap();
                        if !c.status.success() {
                            failures.lock().unwrap().push(format!(
                                "{}: compile failed: {}",
                                f.display(),
                                String::from_utf8_lossy(&c.stderr)
                            ));
                            continue;
                        }
                        bin
                    } else {
                        f.clone()
                    };
                    let out = lang().arg("run").args(flags).arg(&target).output().unwrap();
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    if stdout != expected || out.status.code() != Some(code) {
                        failures.lock().unwrap().push(format!(
                            "{} ({flags:?}{}): exit {:?} (want {code})\n--- stdout\n{stdout}--- expected\n{expected}--- stderr\n{}",
                            f.display(),
                            if compiled { ", binary MIR" } else { "" },
                            out.status.code(),
                            String::from_utf8_lossy(&out.stderr)
                        ));
                    }
                }
            });
        }
    });
    let _ = std::fs::remove_dir_all(&dir);
    let failures = failures.into_inner().unwrap();
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

fn run_all(flags: &[&str]) {
    run_corpus(flags, false)
}

/// Optimized code must behave exactly as unoptimized code.
#[test]
fn run_programs_release() {
    run_all(&["--release"]);
}

/// Every program saved as binary MIR runs as from source.
#[test]
fn run_programs_compiled() {
    run_corpus(&[], true);
}

#[test]
fn reject_programs() {
    let mut failures = vec![];
    for f in files("tests/reject") {
        let src = std::fs::read_to_string(&f).unwrap();
        let want: Vec<&str> =
            src.lines().filter_map(|l| l.trim().strip_prefix("// expect-error:")).map(str::trim).collect();
        let out = lang().arg("check").arg("--json").arg(&f).output().unwrap();
        let json = String::from_utf8_lossy(&out.stdout);
        let missing: Vec<&&str> =
            want.iter().filter(|c| !json.contains(&format!("\"code\":\"{c}\""))).collect();
        if out.status.success() || !missing.is_empty() || want.is_empty() {
            failures.push(format!("{}: missing {missing:?}\n{json}", f.display()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// Standard input through `io.stdin()` (line reads), in a program that
/// also reads files through `io.Reader`.
#[test]
fn standard_input() {
    use std::io::Write;
    let mut child = lang()
        .args(["run", "tests/run/readers_writers.bdg"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"in one\r\nin two").unwrap();
    let out = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success() && text.ends_with("stdin: in one\nstdin: in two\n"), "{text}");
}

/// Programs that check with the warnings their `// expect-warning:` lines
/// name (and no errors).
#[test]
fn warning_programs() {
    let mut failures = vec![];
    for f in files("tests/warnings") {
        let src = std::fs::read_to_string(&f).unwrap();
        let want: Vec<&str> =
            src.lines().filter_map(|l| l.trim().strip_prefix("// expect-warning:")).map(str::trim).collect();
        let out = lang().arg("check").arg("--json").arg(&f).output().unwrap();
        let json = String::from_utf8_lossy(&out.stdout);
        let missing: Vec<&&str> = want
            .iter()
            .filter(|c| {
                !json.contains(&format!("\"code\":\"{c}\"")) || !json.contains("\"severity\":\"warning\"")
            })
            .collect();
        if !out.status.success()
            || !missing.is_empty()
            || want.is_empty()
            || json.contains("\"severity\":\"error\"")
        {
            failures.push(format!("{}: missing {missing:?}\n{json}", f.display()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// Every example project checks (they are not all run, so a checking
/// regression would otherwise go unnoticed).
#[test]
fn example_projects_check() {
    let mut failures = vec![];
    for dir in ["examples/design", "tests/sqlgen"] {
        for e in std::fs::read_dir(root().join(dir)).unwrap().flatten() {
            let p = e.path();
            if !p.join("badger.toml").is_file() {
                continue;
            }
            let out = lang().arg("check").arg(&p).output().unwrap();
            if !out.status.success() {
                failures.push(format!("{}:\n{}", p.display(), String::from_utf8_lossy(&out.stderr)));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Every example program runs and prints the expected output.
#[test]
fn examples_run() {
    let dir = std::env::temp_dir().join(format!("badger-examples-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("notes.txt"), "first note\n\n  second note  \n").unwrap();
    let cases: &[(&str, &[&str], &str)] = &[
        ("examples/01_basics.bdg", &[], "The answer is twelve; 5! is 120\n"),
        ("examples/02_records.bdg", &[], ""),
        (
            "examples/03_domain_types.bdg",
            &[],
            "Hello Ada, signed in as user-42\nRetry 1\nRetry budget exhausted\nStopping\n",
        ),
        ("examples/04_errors_and_use.bdg", &[], "shipment: 3 packages\n"),
        ("examples/05_collections.bdg", &[], ""),
        ("examples/06_schemas.bdg", &[], ""),
        ("examples/07_tasks.bdg", &["README.md", "README.md"], "true\n"),
        ("examples/09_batch_channels.bdg", &[], ""),
        ("examples/10_interceptors.bdg", &[], ""),
        ("examples/11_systems.bdg", &[], "Welcome, Ada\n"),
        ("examples/design/prefix_views.bdg", &[], ""),
        ("examples/smoke.bdg", &[], "12\n"),
    ];
    let mut failures = vec![];
    for (file, args, expected) in cases {
        let out = lang().arg("run").arg(file).arg("--").args(*args).output().unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        if stdout != *expected || !out.status.success() {
            failures.push(format!(
                "{file}: {:?}\n{stdout}\n{}",
                out.status.code(),
                String::from_utf8_lossy(&out.stderr)
            ));
        }
    }
    let channels = lang().args(["run", "examples/08_channels.bdg"]).output().unwrap();
    let text = String::from_utf8_lossy(&channels.stdout);
    if !channels.status.success() || text.lines().count() < 10 {
        failures.push(format!("08_channels: {text}"));
    }
    let notes = lang()
        .current_dir(&dir)
        .args(["run", root().join("examples/12_import_notes.bdg").to_str().unwrap(), "--", "notes.txt"])
        .output()
        .unwrap();
    if String::from_utf8_lossy(&notes.stdout) != "Imported 2 notes\n" {
        failures.push(format!("12_import_notes: {}", String::from_utf8_lossy(&notes.stderr)));
    }
    let jobs = lang().args(["run", "examples/design/batch_jobs.bdg"]).output().unwrap();
    let expected: String = (0..100).map(|i| format!("job-{i}\n")).collect();
    if !jobs.status.success() || String::from_utf8_lossy(&jobs.stdout) != expected {
        failures.push(format!("batch_jobs: {}", String::from_utf8_lossy(&jobs.stderr)));
    }
    let normalize =
        lang().args(["run", "examples/design/normalize.bdg", "--", "examples/README.md"]).output().unwrap();
    if !normalize.status.success() || normalize.stdout.is_empty() {
        failures.push("normalize".into());
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// `[config]` merged with `--profile NAME`, overlaid by the environment.
#[test]
fn profiles() {
    let run = |args: &[&str], env: &[(&str, &str)]| {
        let mut c = lang();
        c.args(["run", "tests/projects/profiles"]).args(args);
        for (k, v) in env {
            c.env(k, v);
        }
        let out = c.output().unwrap();
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        )
    };
    assert_eq!(
        run(&[], &[]).1,
        "port=8080 database=orders.db verbose=false\nlimits=100/10 pool=4 timeout=500\n"
    );
    assert_eq!(
        run(&["--profile", "test"], &[]).1,
        "port=8080 database=:memory: verbose=true\nlimits=100/10 pool=1 timeout=500\n"
    );
    assert_eq!(
        run(&["--profile", "prod"], &[("PROFILES_PORT", "9000"), ("PROFILES_DB_POOL", "9")]).1,
        "port=9000 database=orders.db verbose=false\nlimits=100/10 pool=9 timeout=500\n"
    );
    let (ok, _, err) = run(&["--profile", "nope"], &[]);
    assert!(!ok && err.contains("E0013"), "{err}");
    let out = lang()
        .args(["inspect", "config", "tests/projects/profiles", "--profile", "test", "--json"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains(r#"{"key":"database","value":":memory:","source":"profile.test"}"#), "{text}");
}

/// Scenario 16: `badger run --watch` replaces compatible functions while the
/// program runs; an executing frame keeps its version; an incompatible type
/// change is rejected and the running version continues.
#[test]
fn live_reload() {
    use std::io::{BufRead, BufReader, Read};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    let dir = std::env::temp_dir().join(format!("badger-live-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("live.bdg");
    let src = |name: &str| std::fs::read_to_string(root().join("tests/live").join(name)).unwrap();
    std::fs::write(&file, src("reload_v1.bdg")).unwrap();
    let mut child = common::Reaped::new(
        lang()
            .args(["run", "--watch", file.to_str().unwrap()])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let out = Arc::new(Mutex::new(String::new()));
    let err = Arc::new(Mutex::new(String::new()));
    let (o2, e2) = (out.clone(), err.clone());
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut stderr = child.stderr.take().unwrap();
    let t1 = std::thread::spawn(move || {
        let mut line = String::new();
        while stdout.read_line(&mut line).unwrap_or(0) > 0 {
            o2.lock().unwrap().push_str(&line);
            line.clear();
        }
    });
    let t2 = std::thread::spawn(move || {
        let mut buf = [0u8; 1024];
        while let Ok(n) = stderr.read(&mut buf) {
            if n == 0 {
                break;
            }
            e2.lock().unwrap().push_str(&String::from_utf8_lossy(&buf[..n]));
        }
    });
    let wait_for = |what: &Arc<Mutex<String>>, text: &str| {
        let start = Instant::now();
        while !what.lock().unwrap().contains(text) {
            assert!(start.elapsed() < Duration::from_secs(60), "timed out waiting for {text:?}");
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    wait_for(&out, "worker body v1 sees v1");
    std::fs::write(&file, src("reload_incompatible.bdg")).unwrap();
    wait_for(&err, "reload rejected");
    std::thread::sleep(Duration::from_millis(150));
    // an effect change is an incompatible contract change too
    std::fs::write(&file, src("reload_effect.bdg")).unwrap();
    wait_for(&err, "changed from `fn() -> String` to `fn() -> String !{Io}`");
    std::thread::sleep(Duration::from_millis(150));
    std::fs::write(&file, src("reload_v2.bdg")).unwrap();
    let status = child.wait().unwrap();
    t1.join().unwrap();
    t2.join().unwrap();
    let (out, err) = (out.lock().unwrap().clone(), err.lock().unwrap().clone());
    let _ = std::fs::remove_dir_all(&dir);
    assert!(status.success(), "{out}\n{err}");
    assert!(err.contains("type `live.Point` changed"), "{err}");
    assert!(err.contains("[live] version 2"), "{err}");
    assert!(out.contains("worker body v1 sees v2"), "{out}");
    assert!(!out.contains("worker body v2") && !out.contains("v3"), "{out}");
    // `main` itself was not restarted by the compatible reload
    assert!(
        out.matches("main started").count() == 1 && out.ends_with("v2 point 1\nmain finished\n"),
        "{out}"
    );
}

/// Benchmark programs stay valid (they are run manually with `--release`).
#[test]
fn bench_programs_check() {
    for f in files("bench") {
        let out = lang().arg("check").arg(&f).output().unwrap();
        assert!(out.status.success(), "{}: {}", f.display(), String::from_utf8_lossy(&out.stderr));
    }
}

/// `badger inspect WHAT --connect ADDR` reads a program started with
/// `badger run --inspect ADDR`: task wait reasons, channel occupancy, systems.
#[test]
fn inspect_running_program() {
    use std::time::{Duration, Instant};
    let port = 21000 + (std::process::id() % 1000) as u16;
    let addr = format!("127.0.0.1:{port}");
    let mut child = common::Reaped::new(
        lang()
            .args(["run", "--inspect", &addr, "tests/inspect/running.bdg"])
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let ask = |what: &str| {
        let out = lang().args(["inspect", what, "--connect", &addr, "--json"]).output().unwrap();
        String::from_utf8_lossy(&out.stdout).to_string()
    };
    // wait until the program is up and its tasks are parked
    let start = Instant::now();
    let mut tasks = String::new();
    while start.elapsed() < Duration::from_secs(10) {
        tasks = ask("tasks");
        if tasks.contains("\"channel send\"") && tasks.contains("\"channel receive\"") {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let channels = ask("channels");
    let systems = ask("system");
    let status = child.wait().unwrap();
    assert!(status.success());
    assert!(
        tasks.contains("\"wait\":\"channel send\"") && tasks.contains("\"wait\":\"channel receive\""),
        "{tasks}"
    );
    assert!(
        channels.contains("\"capacity\":1,\"queued\":1") && channels.contains("\"waiting_senders\":1"),
        "{channels}"
    );
    assert!(
        systems.contains(r#""nodes":[{"key":"base","provider":"test/value","generation":1},{"key":"total","provider":"test/value","generation":1}]"#),
        "{systems}"
    );
    // a task parked inside a pipeline stage, and every task's code version
    assert!(tasks.contains(r#""stage":"test/auth","stages":["test/auth"]"#), "{tasks}");
    assert!(tasks.contains(r#""code_version":1"#), "{tasks}");
}

/// `badger repl --connect ADDR` evaluates inside a running program: it reads the
/// program's module-level state, adds definitions, rejects an incompatible
/// replacement, and a compatible replacement changes the running loop.
#[test]
fn remote_repl() {
    use std::io::Write;
    use std::time::Duration;
    let port = 23000 + (std::process::id() % 1000) as u16;
    let addr = format!("127.0.0.1:{port}");
    let program = lang()
        .args(["run", "--inspect", &addr, "tests/inspect/counter.bdg"])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    // wait for the endpoint
    let start = std::time::Instant::now();
    while std::net::TcpStream::connect(&addr).is_err() {
        assert!(start.elapsed() < Duration::from_secs(10), "endpoint never came up");
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut repl = lang()
        .args(["repl", "--connect", &addr])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let input = "label == \"counter\"\nfn double(x: Int) -> Int { x * 2 }\ndouble(21)\nfn greeting() -> Int { 3 }\nfn greeting() -> String { \"bye\" }\n";
    repl.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let out = repl.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let prog = program.wait_with_output().unwrap();
    assert!(text.starts_with("true\n42\n"), "{text}");
    assert!(text.contains("E0300"), "{text}");
    assert!(prog.status.success());
    assert_eq!(String::from_utf8_lossy(&prog.stdout), "greeting is now bye\n");
}

/// Path dependencies (transitive) resolve through `badger.lock`: a changed
/// dependency is refused until `badger update`; version requirements hold.
#[test]
fn dependencies() {
    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                copy_dir(&p, &to.join(e.file_name()));
            } else {
                std::fs::copy(&p, to.join(e.file_name())).unwrap();
            }
        }
    }
    let dir = std::env::temp_dir().join(format!("badger-deps-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    copy_dir(&root().join("tests/projects/deps"), &dir);
    let app = dir.join("app");
    let run = || {
        let out = lang().args(["run", app.to_str().unwrap()]).output().unwrap();
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        )
    };
    let (ok, out, err) = run();
    assert!(ok, "{err}");
    assert_eq!(out, "HELLO!\n3\n3 items, 6 characters\n");
    // a changed dependency is refused until the lockfile is updated
    let lib = dir.join("mathlib/src/lib.bdg");
    let text = std::fs::read_to_string(&lib).unwrap();
    std::fs::write(&lib, text.replace("fold(0,", "fold(100,")).unwrap();
    let (ok, _, err) = run();
    assert!(!ok && err.contains("E0015") && err.contains("mathlib"), "{err}");
    let upd = lang().args(["update", app.to_str().unwrap()]).output().unwrap();
    assert!(upd.status.success());
    let (ok, out, err) = run();
    assert!(ok, "{err}");
    assert!(out.ends_with("3 items, 106 characters\n"), "{out}");
    // version requirements are checked
    let manifest = app.join("badger.toml");
    let m = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(&manifest, m.replace("version = \"0.2\"", "version = \"0.3\"")).unwrap();
    let (ok, _, err) = run();
    assert!(!ok && err.contains("requires version 0.3, found 0.2.1"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Structured logs on standard error: the level threshold, text and JSON
/// lines with fields in order, and `log.with` context inherited by a
/// spawned task.
#[test]
fn structured_logs() {
    let run = |env: &[(&str, &str)]| {
        let mut c = lang();
        c.args(["run", "tests/log/records.bdg"]);
        for (k, v) in env {
            c.env(k, v);
        }
        let out = c.output().unwrap();
        assert!(out.status.success());
        String::from_utf8_lossy(&out.stderr).to_string()
    };
    let text = run(&[]);
    let lines: Vec<&str> = text.lines().map(|l| l.split_once(' ').map(|x| x.1).unwrap_or(l)).collect();
    assert_eq!(
        lines,
        vec![
            "INFO  starting version=1.0 workers=4",
            "WARN  slow query request=7 ms=120 sql=\"select 1\"",
            "INFO  in child request=7 step=2",
            "ERROR done",
        ],
        "{text}"
    );
    let json = run(&[("BADGER_LOG_LEVEL", "debug"), ("BADGER_LOG_FORMAT", "json")]);
    let recs: Vec<serde_json::Value> = json.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(recs.len(), 5, "{json}");
    assert_eq!(recs[0]["level"], "debug");
    assert_eq!(
        (recs[3]["msg"].as_str(), recs[3]["request"].as_i64(), recs[3]["step"].as_i64()),
        (Some("in child"), Some(7), Some(2))
    );
    assert_ne!(recs[3]["task"], recs[0]["task"]);
    assert!(json.lines().nth(2).unwrap().starts_with("{\"ts\":"), "{json}");
    assert!(run(&[("BADGER_LOG_LEVEL", "off")]).is_empty());
}

/// OpenTelemetry export: a span, a log record inside it (with its trace
/// and span ids), and a counter reach an OTLP/HTTP collector as JSON.
#[test]
fn otel_export() {
    use std::io::{BufRead, BufReader, Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let collector = std::thread::spawn(move || {
        let mut got: Vec<(String, serde_json::Value)> = vec![];
        // never wait forever: report what arrived
        listener.set_nonblocking(true).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while got.len() < 3 && std::time::Instant::now() < deadline {
            let Ok((mut s, _)) = listener.accept() else {
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            };
            s.set_nonblocking(false).unwrap();
            s.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            r.read_line(&mut line).unwrap();
            let path = line.split(' ').nth(1).unwrap_or("").to_string();
            let mut len = 0usize;
            loop {
                let mut h = String::new();
                r.read_line(&mut h).unwrap();
                if h.trim().is_empty() {
                    break;
                }
                if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap();
                }
            }
            let mut body = vec![0u8; len];
            r.read_exact(&mut body).unwrap();
            s.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}").unwrap();
            got.push((path, serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null)));
        }
        got
    });
    let out = lang()
        .args(["run", "tests/log/otel.bdg"])
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", format!("http://127.0.0.1:{port}"))
        .env("OTEL_SERVICE_NAME", "svc")
        // records are exported, not written
        .env("BADGER_LOG_FORMAT", "none")
        .output()
        .unwrap();
    assert!(
        out.status.success() && String::from_utf8_lossy(&out.stdout) == "done\n",
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let got = collector.join().unwrap();
    let paths: Vec<&str> = got.iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(paths, vec!["/v1/traces", "/v1/logs", "/v1/metrics"]);
    let span = &got[0].1["resourceSpans"][0]["scopeSpans"][0]["spans"][0];
    assert_eq!(span["name"], "work");
    assert_eq!(got[0].1["resourceSpans"][0]["resource"]["attributes"][0]["value"]["stringValue"], "svc");
    let log = &got[1].1["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0];
    assert_eq!(log["body"]["stringValue"], "working");
    assert_eq!((&log["traceId"], &log["spanId"]), (&span["traceId"], &span["spanId"]));
    let metric = &got[2].1["resourceMetrics"][0]["scopeMetrics"][0]["metrics"][0];
    assert_eq!(
        (&metric["name"], &metric["sum"]["dataPoints"][0]["asDouble"]),
        (&serde_json::json!("jobs"), &serde_json::json!(1.0))
    );
}

/// Configuration from the environment: `[env] dotenv` files load before
/// the program starts (variables already set win), and `${NAME}` /
/// `${NAME:-default}` in `[config]` values read the environment.
#[test]
fn dotenv_and_env_references() {
    let dir = std::env::temp_dir().join(format!("badger-dotenv-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("badger.toml"),
        "[package]\nname = \"envdemo\"\n\n[env]\ndotenv = [\".env\", \".env.local\"]\n\n[config]\nurl = \"${DATABASE_URL}\"\nport = \"${PORT:-8080}\"\ngreeting = \"hi $${literal}\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join(".env"),
        "# local settings\nexport DATABASE_URL=\"postgres://u@h/db\"\nSECRET=abc # trailing\nQUOTED='a b'\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/main.bdg"),
        "import std.io\nimport std.config\nimport std.env\n\npub fn main() -> Result<Unit, String> {\n    let c = config.load<{ url: String, port: Int, greeting: String }>(\"APP_\").map_err(fn(e) { e.display() })?\n    let secret = env.get(\"SECRET\").unwrap_or(\"-\")\n    let quoted = env.get(\"QUOTED\").unwrap_or(\"-\")\n    let _ = io.println($\"{c.url} {c.port} {c.greeting} {secret} {quoted}\")\n    ok(())\n}\n",
    )
    .unwrap();
    let run = |env: &[(&str, &str)]| {
        let mut c = lang();
        c.arg("run")
            .arg(&dir)
            .env_remove("DATABASE_URL")
            .env_remove("PORT")
            .env_remove("SECRET")
            .env_remove("QUOTED");
        for (k, v) in env {
            c.env(k, v);
        }
        let out = c.output().unwrap();
        (String::from_utf8_lossy(&out.stdout).to_string(), String::from_utf8_lossy(&out.stderr).to_string())
    };
    assert_eq!(run(&[]).0, "postgres://u@h/db 8080 hi ${literal} abc a b\n");
    assert_eq!(run(&[("PORT", "9"), ("DATABASE_URL", "real")]).0, "real 9 hi ${literal} abc a b\n");
    std::fs::remove_file(dir.join(".env")).unwrap();
    let (out, err) = run(&[]);
    assert!(out.is_empty() && err.contains("url: environment variable `DATABASE_URL` is not set"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Workspaces and git dependencies: members share one `badger.lock`; a git
/// dependency builds from its locked commit (a changed `rev` is refused
/// until `badger update`, which resolves it again); `badger check` and
/// `badger test` at the root cover every member; a library and a workspace
/// root are not run.
#[test]
fn workspaces_and_git_dependencies() {
    if Command::new("git").arg("--version").output().is_err() {
        eprintln!("skipped: no git");
        return;
    }
    let dir = std::env::temp_dir().join(format!("badger-ws-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let write = |p: &str, text: &str| {
        let p = dir.join(p);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    };
    let git = |args: &[&str]| {
        let ok = Command::new("git")
            .args(["-c", "user.email=t@example.com", "-c", "user.name=t", "-c", "init.defaultBranch=main"])
            .args(args)
            .current_dir(dir.join("repo"))
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "git {args:?}");
    };
    write(
        "repo/badger.toml",
        "[package]\nname = \"shout\"\nversion = \"1.0.0\"\nkind = \"library\"\nentry = \"src/lib.bdg\"\n",
    );
    write("repo/src/lib.bdg", "pub fn shout(s: String) -> String { $\"{s}!\" }\n");
    git(&["init", "-q"]);
    git(&["add", "-A"]);
    git(&["commit", "-qm", "v1"]);
    git(&["tag", "v1"]);
    write("repo/src/lib.bdg", "pub fn shout(s: String) -> String { $\"{s}!!\" }\n");
    git(&["commit", "-qam", "v2"]);
    git(&["tag", "v2"]);
    write("ws/badger.toml", "[workspace]\nmembers = [\n    \"app\",\n    \"libs/*\",\n]\n");
    write(
        "ws/libs/util/badger.toml",
        "[package]\nname = \"util\"\nkind = \"library\"\nentry = \"src/lib.bdg\"\n",
    );
    write(
        "ws/libs/util/src/lib.bdg",
        "pub fn twice(n: Int) -> Int { n * 2 }\n\npub fn test_twice() -> Unit { assert(twice(2) == 4) }\n",
    );
    write(
        "ws/app/badger.toml",
        "[package]\nname = \"app\"\nentry = \"src/main.bdg\"\n\n[dependencies]\nutil = { path = \"../libs/util\" }\nshout = { git = \"../../repo\", rev = \"v1\" }\n",
    );
    write(
        "ws/app/src/main.bdg",
        "import std.io\nimport util\nimport shout\n\npub fn main() -> Result<Unit, io.Error> {\n    io.println(shout.shout($\"{util.twice(21)}\"))\n}\n\npub fn test_app() -> Unit { assert(util.twice(1) == 2) }\n",
    );
    let ws = dir.join("ws");
    let run = |args: &[&str]| {
        let out = lang().current_dir(&ws).args(args).env("BADGER_HOME", dir.join("cache")).output().unwrap();
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        )
    };
    let (ok, _, err) = run(&["check", "."]);
    assert!(ok, "{err}");
    let (ok, out, err) = run(&["run", "app"]);
    assert!(ok && out == "42!\n", "{out}{err}");
    let lock = std::fs::read_to_string(ws.join("badger.lock")).unwrap();
    assert!(
        lock.contains("shout 1.0.0 git+")
            && lock.contains("?rev=v1@")
            && lock.contains("util 0.0.0 libs/util"),
        "{lock}"
    );
    assert!(!ws.join("app/badger.lock").exists(), "members share the workspace lockfile");
    let (ok, out, err) = run(&["test", "."]);
    assert!(ok && out.contains("test_twice") && out.contains("test_app"), "{out}{err}");
    let (ok, _, err) = run(&["run", "libs/util"]);
    assert!(!ok && err.contains("is a library"), "{err}");
    let (ok, _, err) = run(&["run", "."]);
    assert!(!ok && err.contains("is a workspace") && err.contains("app, libs/util"), "{err}");
    // a changed `rev` is refused until the lockfile is updated
    let m = std::fs::read_to_string(ws.join("app/badger.toml")).unwrap();
    std::fs::write(ws.join("app/badger.toml"), m.replace("rev = \"v1\"", "rev = \"v2\"")).unwrap();
    let (ok, _, err) = run(&["run", "app"]);
    assert!(!ok && err.contains("E0015") && err.contains("run `badger update`"), "{err}");
    let (ok, _, err) = run(&["update", "."]);
    assert!(ok, "{err}");
    let (ok, out, err) = run(&["run", "app"]);
    assert!(ok && out == "42!!\n", "{out}{err}");
    // the checkout is cached: the repository can go
    std::fs::remove_dir_all(dir.join("repo")).unwrap();
    let (ok, out, err) = run(&["run", "app"]);
    assert!(ok && out == "42!!\n", "{out}{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Scenario 16, incompatible layout: with `--watch --reconstruct` a changed
/// type definition cancels the running program (its resource halts) and
/// restarts it on the new version with freshly built values.
#[test]
fn live_reconstruct() {
    use std::time::{Duration, Instant};
    for workers in ["1", "4"] {
        let dir = std::env::temp_dir().join(format!("badger-reconstruct-{}-{workers}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("app.bdg");
        let src = |name: &str| std::fs::read_to_string(root().join("tests/live").join(name)).unwrap();
        std::fs::write(&file, src("reconstruct_v1.bdg")).unwrap();
        let out_path = dir.join("out.txt");
        let child = common::Reaped::new(
            lang()
                .args(["run", "--workers", workers, "--watch", "--reconstruct", file.to_str().unwrap()])
                .stdout(std::fs::File::create(&out_path).unwrap())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let start = Instant::now();
        while !std::fs::read_to_string(&out_path).unwrap_or_default().contains("v1 point 1") {
            assert!(start.elapsed() < Duration::from_secs(20), "version 1 never started");
            std::thread::sleep(Duration::from_millis(20));
        }
        std::fs::write(&file, src("reconstruct_v2.bdg")).unwrap();
        let out = child.output();
        let text = std::fs::read_to_string(&out_path).unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(out.status.success(), "{err}");
        assert_eq!(
            text, "start v1\nv1 point 1\nhalt v1\nstart v2\nv2 point 1 2\nhalt v2\n",
            "workers {workers}: {err}"
        );
        assert!(err.contains("reconstructing the program"), "{err}");
    }
}

#[test]
fn explain_types() {
    let file = root().join("tests/explain/defs.bdg");
    let cases = [
        ("Point", "struct Point { x: Int, y: Int }"),
        ("Shape", "enum Shape { circle(r: Int), empty }"),
        ("Pair", "type Pair<T> = (T, T)"),
        ("Box2", "struct Box2<T> { pub value: T, count: Int } derive(Eq)"),
        ("Named", "protocol Named { name: fn(&Self) -> String }"),
        ("show_all", "fn<T, U>(T, U) -> String where T: Display, U: Display, U: Eq"),
    ];
    for (name, want) in cases {
        let out = lang().args(["explain", "type", name]).arg(&file).output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        assert_eq!(text.trim(), format!("{name}: {want}"), "explain type {name}");
    }
    let out = lang().args(["explain", "effect", "show_all"]).arg(&file).output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("show_all: !{}"));
}

/// Checking proposed edits in their project (`badger query check
/// --overlays`): an interface change and two consumers together, a failing
/// import and an incompatible signature, without touching the files on
/// disk; the result agrees with checking the same edits saved.
#[test]
fn query_check_overlays() {
    let dir = root().join("tests/projects/overlay");
    let files = [
        ("src/shapes.bdg", "pub fn area(w: Int, h: Int, d: Int) -> Int { w * h * d }\n"),
        (
            "src/report.bdg",
            "import shapes\nimport missing_mod\n\npub fn line(w: Int, h: Int) -> String { $\"{w}x{h} = {shapes.area(w, h, 1)}\" }\n",
        ),
        ("src/unreached.bdg", "pub fn x() -> Int { 1 }\n"),
    ];
    let before: Vec<String> = ["src/shapes.bdg", "src/report.bdg", "src/jobs.bdg", "src/main.bdg"]
        .iter()
        .map(|f| std::fs::read_to_string(dir.join(f)).unwrap())
        .collect();
    let overlays: serde_json::Map<String, serde_json::Value> =
        files.iter().map(|(k, v)| (k.to_string(), serde_json::Value::from(*v))).collect();
    let check = |path: &std::path::Path, overlays: Option<&str>| -> serde_json::Value {
        let mut c = lang();
        c.args(["query", "check"]).arg(path);
        if let Some(o) = overlays {
            c.args(["--overlays", "-"])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped());
            let mut child = c.spawn().unwrap();
            std::io::Write::write_all(&mut child.stdin.take().unwrap(), o.as_bytes()).unwrap();
            let out = child.wait_with_output().unwrap();
            assert!(out.status.success());
            return serde_json::from_slice(&out.stdout).unwrap();
        }
        let out = c.output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        serde_json::from_slice(&out.stdout).unwrap()
    };
    let r = check(&dir, Some(&serde_json::Value::Object(overlays).to_string()));
    let found: Vec<(String, String)> = r["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            let f = d["file"].as_str().unwrap();
            (f.rsplit('/').next().unwrap().to_string(), d["code"].as_str().unwrap_or("").to_string())
        })
        .collect();
    assert_eq!(
        found,
        vec![("report.bdg".to_string(), "E0114".to_string()), ("jobs.bdg".to_string(), "E0309".to_string())],
        "{r:#}"
    );
    assert_eq!(r["ok"], false);
    let proposed: Vec<&str> = r["inputs"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["proposed"] == true)
        .map(|i| i["file"].as_str().unwrap().rsplit('/').next().unwrap())
        .collect();
    assert_eq!(proposed, vec!["report.bdg", "shapes.bdg"]);
    let unused = r["unused_overlays"].as_array().unwrap();
    assert!(unused.len() == 1 && unused[0].as_str().unwrap().ends_with("src/unreached.bdg"), "{r:#}");
    let entries: Vec<&str> =
        r["roots"].as_array().unwrap().iter().map(|x| x["entry"].as_str().unwrap()).collect();
    assert_eq!(entries, vec!["main", "task:summary"]);
    // nothing written
    let after: Vec<String> = ["src/shapes.bdg", "src/report.bdg", "src/jobs.bdg", "src/main.bdg"]
        .iter()
        .map(|f| std::fs::read_to_string(dir.join(f)).unwrap())
        .collect();
    assert_eq!(before, after);
    assert!(!dir.join("src/unreached.bdg").exists());
    // the same edits saved in a copy check the same
    let copy = std::env::temp_dir().join(format!("badger-overlay-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&copy);
    std::fs::create_dir_all(copy.join("src")).unwrap();
    std::fs::copy(dir.join("badger.toml"), copy.join("badger.toml")).unwrap();
    for f in ["src/shapes.bdg", "src/report.bdg", "src/jobs.bdg", "src/main.bdg"] {
        std::fs::copy(dir.join(f), copy.join(f)).unwrap();
    }
    for (f, t) in &files[..2] {
        std::fs::write(copy.join(f), t).unwrap();
    }
    let saved = check(&copy, None);
    let strip = |v: &serde_json::Value, base: &std::path::Path| -> String {
        let b = std::fs::canonicalize(base).unwrap().display().to_string();
        v["diagnostics"].to_string().replace(&b, "")
    };
    assert_eq!(strip(&r, &dir), strip(&saved, &copy));
    let hashes = |v: &serde_json::Value| -> Vec<String> {
        v["inputs"].as_array().unwrap().iter().map(|i| i["hash"].as_str().unwrap().to_string()).collect()
    };
    assert_eq!(hashes(&r), hashes(&saved));
    // the saved project without edits checks, and `badger check` agrees
    assert_eq!(check(&dir, None)["ok"], true);
    assert!(!lang().arg("check").arg(&copy).output().unwrap().status.success());
    let _ = std::fs::remove_dir_all(&copy);
}

/// Typed SQL generation (`badger generate sql`) from a SQLite schema: the
/// committed module is what the generator writes, and the program using it
/// checks and runs.
#[test]
fn sql_generation_sqlite() {
    let out =
        lang().args(["generate", "sql", "--check", "tests/sqlgen/sqlite/queries.sql"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let run = lang().args(["run", "tests/sqlgen/sqlite"]).output().unwrap();
    let want = std::fs::read_to_string(root().join("tests/sqlgen/sqlite/main.out")).unwrap();
    assert_eq!(String::from_utf8_lossy(&run.stdout), want, "{}", String::from_utf8_lossy(&run.stderr));
    // a query the schema rejects is reported with its line
    let dir = std::env::temp_dir().join(format!("badger-sqlgen-bad-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let bad = dir.join("bad.sql");
    std::fs::write(&bad, "-- name: nope :many\nSELECT missing FROM users\n").unwrap();
    let out = lang()
        .args(["generate", "sql", "--schema", "tests/sqlgen/sqlite/schema.sql"])
        .arg(&bad)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success() && err.contains("line 1: `nope`") && err.contains("missing"), "{err}");
}

/// Native modules (`tests/native`, calling `crates/badger_native_demo`, which
/// cargo builds as a dev-dependency into `deps` next to the executable):
/// every mode, value shapes, defects, and handles, on one worker and four;
/// in a restricted project only the entry module may declare and call one.
#[test]
fn native_modules() {
    for f in files("tests/native") {
        let want = std::fs::read_to_string(f.with_extension("out")).unwrap();
        for flags in [&[][..], &["--release", "--workers", "4"][..]] {
            let out = lang().arg("run").args(flags).arg(&f).output().unwrap();
            assert_eq!(
                String::from_utf8_lossy(&out.stdout),
                want,
                "{} {flags:?}: {}",
                f.display(),
                String::from_utf8_lossy(&out.stderr)
            );
        }
    }
    // recorded async calls replay, including one abandoned by a timeout
    let bundle = std::env::temp_dir().join(format!("badger-native-replay-{}.json", std::process::id()));
    let rec = lang()
        .args(["run", "--record"])
        .arg(&bundle)
        .arg("tests/native/replay_timeout.bdg")
        .output()
        .unwrap();
    assert!(rec.status.success(), "{}", String::from_utf8_lossy(&rec.stderr));
    let started = std::time::Instant::now();
    let rep = lang().arg("replay").arg(&bundle).output().unwrap();
    let _ = std::fs::remove_file(&bundle);
    let err = String::from_utf8_lossy(&rep.stderr);
    assert!(rep.status.success() && err.contains("reproduced the recorded outcome"), "{err}");
    assert_eq!(String::from_utf8_lossy(&rep.stdout), "timed out: true\nlater: 2\n");
    assert!(started.elapsed() < std::time::Duration::from_secs(20), "the replay ran the abandoned call");
    let dir = std::env::temp_dir().join(format!("badger-native-restricted-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("badger.toml"),
        "[package]\nname = \"r\"\nentry = \"src/main.bdg\"\nrestricted = true\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/main.bdg"),
        "import helper\n\npub fn main() -> Unit {\n    let _ = helper.twice(2)\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/helper.bdg"),
        "extern \"native badger_native_demo\" fn add(a: Int, b: Int) -> Int\n\npub fn twice(x: Int) -> Int { add(x, x) }\n",
    )
    .unwrap();
    let out = lang().arg("check").arg(&dir).output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success()
            && err.contains("E0170")
            && err.contains("extern \"native badger_native_demo\" fn helper.add"),
        "{err}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// `badger query rename` returns edits (not applied) for every use, in each
/// of a project's entry points and tasks; inputs a query cannot answer are
/// errors rather than empty results.
#[test]
fn query_rename_and_unsupported_inputs() {
    let q = |args: &[&str]| {
        let out = lang().arg("query").args(args).output().unwrap();
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        )
    };
    let files = |out: &str| -> Vec<String> {
        let v: serde_json::Value = serde_json::from_str(out).unwrap();
        let mut f: Vec<String> = v["edits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["file"].as_str().unwrap().rsplit('/').next().unwrap().to_string())
            .collect();
        f.sort();
        f
    };
    let all = vec!["jobs.bdg".to_string(), "report.bdg".into(), "shapes.bdg".into()];
    // from the definition, from the task-only module, and from the main entry's module
    for (file, line, col) in [("shapes.bdg", "3", "8"), ("jobs.bdg", "5", "40"), ("report.bdg", "3", "62")] {
        let p = format!("tests/projects/overlay/src/{file}");
        let (ok, out, err) = q(&["rename", &p, "--line", line, "--col", col, "--name", "surface"]);
        assert!(ok, "{err}");
        assert_eq!(files(&out), all, "{file}: {out}");
    }
    assert_eq!(
        std::fs::read_to_string(root().join("tests/projects/overlay/src/shapes.bdg"))
            .unwrap()
            .matches("area")
            .count(),
        1
    );
    let (ok, _, err) = q(&[
        "rename",
        "tests/projects/overlay/src/shapes.bdg",
        "--line",
        "3",
        "--col",
        "8",
        "--name",
        "Surface",
    ]);
    assert!(!ok && err.contains("cannot replace"), "{err}");
    let (ok, _, err) = q(&["symbols", "tests/projects/overlay"]);
    assert!(!ok && err.contains("needs a source file"), "{err}");
    let (ok, _, err) = q(&["hover", "tests/projects/overlay/src/nope.bdg", "--line", "1", "--col", "1"]);
    assert!(!ok && err.contains("no such file"), "{err}");
    let (ok, _, err) = q(&["bogus", "tests/projects/overlay"]);
    assert!(!ok && err.contains("unknown query `bogus` (diagnostics, check,"), "{err}");
}

/// Project tooling: optional paths (the current project), named entries,
/// user tasks, runtime settings with profile overrides, the bounded trace
/// sink's overload report, and strict manifest keys.
#[test]
fn project_tooling() {
    let dir = root().join("tests/projects/tooling");
    let run = |args: &[&str], env: &[(&str, &str)]| {
        let mut c = lang();
        c.current_dir(&dir).args(args);
        for (k, v) in env {
            c.env(k, v);
        }
        let out = c.output().unwrap();
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        )
    };
    let (ok, out, err) = run(&["run"], &[]);
    assert!(ok, "{err}");
    assert_eq!(out, "capacity=4 recorded=10 dropped=6 policy=drop_oldest\nlast={\"i\":9}\n");
    assert!(err.contains("trace: sink overloaded: 6 of 10 events not retained"), "{err}");
    let (_, out, _) = run(&["run", "--profile", "strict"], &[]);
    assert_eq!(out, "capacity=2 recorded=10 dropped=8 policy=drop_newest\nlast={\"i\":1}\n");
    let (_, out, _) = run(&["run"], &[("BADGER_TRACE_CAPACITY", "16")]);
    assert!(out.starts_with("capacity=16 recorded=10 dropped=0"), "{out}");
    assert_eq!(run(&["run", "admin"], &[]).1, "admin entry\n");
    assert_eq!(run(&["task", "seed", "--", "a", "b", "c"], &[]).1, "seeding 3 records\n");
    assert_eq!(run(&["task", "greet", "--", "x", "y"], &[]).1, "hello x\nhello y\n");
    let (ok, _, err) = run(&["task", "nope"], &[]);
    assert!(!ok && err.contains("unknown task `nope` (declared in [tasks]: seed, greet)"), "{err}");
    let (ok, _, err) = run(&["run", "nope"], &[]);
    assert!(!ok && err.contains("unknown entry `nope`"), "{err}");
    assert!(run(&["check"], &[]).0);
    let (_, out, _) = run(&["inspect", "runtime", ".", "--profile", "strict"], &[]);
    assert!(out.contains("trace_capacity = 2    # from [profile.strict.runtime]"), "{out}");
    // strict manifests: unknown sections and keys are errors
    let tmp = std::env::temp_dir().join(format!("badger-strict-{}", std::process::id()));
    std::fs::create_dir_all(tmp.join("src")).unwrap();
    std::fs::write(tmp.join("src/main.bdg"), "pub fn main() {}\n").unwrap();
    for (toml, want) in [
        ("[package]\nname = \"x\"\n[runtime]\nwokers = 2\n", "unknown runtime setting `wokers`"),
        ("[package]\nname = \"x\"\n[pakage]\n", "unknown section `[pakage]`"),
        ("[package]\nname = \"x\"\nentrypoint = \"a\"\n", "unknown package key `entrypoint`"),
        ("[package]\nname = \"x\"\n[runtime]\ntrace_overflow = \"block\"\n", "trace_overflow must be"),
        ("[package]\nname = \"x\"\n[tasks]\nbad = \"src/main.bdg#nope\"\n", "no `nope` function"),
    ] {
        std::fs::write(tmp.join("badger.toml"), toml).unwrap();
        let out = lang().arg("check").arg(&tmp).output().unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success() && err.contains(want), "{toml}: {err}");
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

/// Machine diagnostics carry module, expected/actual shapes, the enclosing
/// definition, and related declarations; the human rendering shows the same.
#[test]
fn structured_diagnostics() {
    let file = root().join("tests/diagnostics/structured.bdg");
    let out = lang().args(["check", "--json"]).arg(&file).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    for want in [
        r#""module":"structured""#,
        r#""expected":"Point","actual":"Int","scope":"fn structured.main","related":[{"name":"Point","#,
        r#""expected":"Point","actual":"integer literal""#,
        r#""expected":"Int","actual":"String","scope":"fn structured.Point.norm""#,
        // receiver type, callee, and matched enum declarations
        r#""scope":"fn structured.more","related":[{"name":"Point","#,
        r#""scope":"fn structured.more","related":[{"name":"takes","#,
        r#""scope":"fn structured.more","related":[{"name":"Shape","#,
    ] {
        assert!(text.contains(want), "missing {want} in {text}");
    }
    let out = lang().arg("check").arg(&file).output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("in fn structured.Point.norm") && err.contains("expected: Int"), "{err}");
    // the query interface (MCP `check`, std.compiler) carries the same facts
    let out = lang().args(["query", "diagnostics"]).arg(&file).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    for want in [
        r#""scope":"fn structured.main""#,
        r#""expected":"Point""#,
        r#""actual":"Int""#,
        r#""scope":"fn structured.Point.norm""#,
        r#""declarations":[{"#,
        r#""name":"Point""#,
    ] {
        assert!(text.contains(want), "query diagnostics lacks {want} in {text}");
    }
}

/// A deadlock report names each blocked task and what it waits for.
#[test]
fn deadlock_report_names_waits() {
    for workers in ["1", "4"] {
        let out =
            lang().args(["run", "--workers", workers, "tests/runtime/deadlock_report.bdg"]).output().unwrap();
        assert_eq!(out.status.code(), Some(101));
        let err = String::from_utf8_lossy(&out.stderr);
        for want in [
            "deadlock: all tasks are blocked",
            "task 1 (main) waits on join (joining task 2)",
            "task 2 (child of task 1) waits on channel receive (channel #1)",
        ] {
            assert!(err.contains(want), "missing {want:?} ({workers} workers) in {err}");
        }
    }
    // the pattern is also found statically: a warning at the join
    let out = lang().args(["check", "--json", "tests/runtime/deadlock_report.bdg"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains(r#""code":"E0450""#) && text.contains("comes after this join"), "{text}");
}

/// Under `heap_max_bytes`, a task outgrowing the budget fails with a
/// defect its supervisor contains; without a budget it finishes.
#[test]
fn heap_budget_contains_a_task() {
    for workers in ["1", "4"] {
        let out = lang()
            .env("BADGER_HEAP_MAX_BYTES", "67108864")
            .args(["run", "--release", "--workers", workers, "tests/runtime/heap_budget.bdg"])
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        assert_eq!(
            text,
            "hog contained\nstill running true\n",
            "{workers} workers: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let out = lang().args(["run", "--release", "tests/runtime/heap_budget.bdg"]).output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("finished 10000000"));
}

/// An internal error on a worker thread ends the process at once with
/// status 70 and a report, rather than leaving the other workers waiting.
#[test]
fn internal_error_exits_promptly() {
    let out = lang()
        .env("BADGER_DEBUG_PANIC_WORKER", "1")
        .args(["run", "--workers", "4", "tests/run/rendezvous.bdg"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(70));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("internal error: requested by BADGER_DEBUG_PANIC_WORKER")
            && err.contains("please report"),
        "{err}"
    );
}

/// Name and syntax errors (found before type checking) also report their
/// module, and name errors their enclosing definition.
#[test]
fn diagnostics_early_phases() {
    let out = lang().args(["check", "--json", "tests/diagnostics/resolve_phase.bdg"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains(r#""module":"resolve_phase""#) && text.contains(r#""scope":"fn resolve_phase.bad""#),
        "{text}"
    );
    // a `.source` file is outside the formatting check (it does not parse)
    let out = lang().args(["check", "--json", "tests/diagnostics/syntax_phase.source"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains(r#""code":"E0001""#) && text.contains(r#""module":"syntax_phase""#), "{text}");
}

/// `badger test`: controlled clocks are per test and drive sleeps by virtual
/// time; assertion failures report the caller's location.
#[test]
fn test_runner_controlled_clock() {
    let out = lang().args(["test", "tests/testing/clock.bdg"]).current_dir(root()).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    for want in [
        "test test_a_manual_clock_sleep ... ok",
        "test test_b_clock_isolated ... ok",
        "test test_c_advance_wakes_sleeper ... ok",
        "test test_d_failure_location ... FAILED (assertion failed: expected 3, found 2 at tests/testing/clock.bdg:38:5)",
        "3 passed; 1 failed",
    ] {
        assert!(text.contains(want), "missing {want:?} in {text}");
    }
}

/// Simulation fixtures: seeded scheduling, record/replay of interleavings,
/// exploration reporting the failing seed, and property checks.
#[test]
fn test_runner_simulation() {
    for workers in ["1", "4"] {
        let out = lang()
            .args(["test", "tests/testing/simulation.bdg"])
            .env("BADGER_WORKERS", workers)
            .current_dir(root())
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        for want in [
            "test test_seeded_schedule_is_reproducible ... ok",
            "test test_seeds_explore_interleavings ... ok",
            "test test_record_and_replay ... ok",
            "test test_explore_reports_failing_seed ... FAILED (failed under schedule seed",
            "test test_property_holds ... ok",
            "test test_property_reports_counterexample ... FAILED (property failed for 95 (run 2, seed 3): too large",
            "4 passed; 2 failed",
        ] {
            assert!(text.contains(want), "missing {want:?} in {text}");
        }
    }
}

/// The local REPL is a live session: module-level values are computed once
/// and keep their identity, bare names show their values, new types can be
/// added, and changing an existing type is rejected.
#[test]
fn local_repl_session() {
    use std::io::Write;
    let mut child = common::Reaped::new(
        lang()
            .arg("repl")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let input = "let counter = Atom.new(0)\ncounter.set(5)\ncounter.get()\nlet r = { a: 1, b: \"s\" }\nr\nfn f(n: Int) -> Int { n * 2 }\nf(counter.get())\nstruct P { x: Int }\nlet p: P = { x: 3 }\np\nstruct Q { a: Int }\nstruct Q { a: Int, b: Int }\n:q\n";
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let out = child.output();
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        text.lines().take(4).collect::<Vec<_>>(),
        ["5", "{a: 1, b: \"s\"}", "10", "P { x: 3 }"],
        "{text}"
    );
    assert!(text.contains("rejected: type `session.Q` changed"), "{text}");
}

/// `main` returning an `Error` reports its code, causes, and origin.
#[test]
fn main_error_report() {
    let out = lang().args(["run", "tests/errors/main_error.bdg"]).current_dir(root()).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("error[wrap]: wrap: bad: 4\n  caused by: bad: 4\n  details: {\"wrap\":{\"source\":{\"bad\":{\"n\":4}}}}\n  at tests/errors/main_error.bdg:14:16"),
        "{err}"
    );
}

/// When a worker outlives the cleanup grace period the system escalates
/// with a defect and does not free the node's resources under it.
#[test]
fn grace_escalation_keeps_resources() {
    let out = lang()
        .args(["run", "tests/runtime/grace_keeps_resources.bdg"])
        .env("BADGER_CLEANUP_GRACE_MS", "200")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("failed: cleanup exceeded the grace period of 200ms"), "{text}");
    assert!(text.contains("worker cleanup uses resource alive=1"), "{text}");
    assert!(!text.contains("resource freed"), "{text}");
}

/// `badger compile` saves a project's settings in binary MIR, and damaged
/// files are refused.
#[test]
fn compiled_programs_run() {
    let dir = std::env::temp_dir().join(format!("badger-mir-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // (every `tests/run` program runs from binary MIR in `run_programs_compiled`)
    let src = root().join("tests/run/dyn_protocol.bdg");
    assert!(
        lang()
            .arg("compile")
            .arg(&src)
            .arg("-o")
            .arg(dir.join("dyn_protocol.bdgmir"))
            .status()
            .unwrap()
            .success()
    );
    // saved MIR is optimized as `run --release` optimizes it: a receive
    // whose result is only examined is split (after inlining)
    let bc = dir.join("broadcast.bdgmir");
    assert!(
        lang()
            .arg("compile")
            .arg(root().join("tests/run/broadcast.bdg"))
            .arg("-o")
            .arg(&bc)
            .status()
            .unwrap()
            .success()
    );
    let saved = std::fs::read(&bc).unwrap();
    assert!(saved.windows(27).any(|w| w == b"badger_rt_i_chan_recv_split"), "saved MIR is not MIR-optimized");
    // a project: its saved worker count and configuration are used
    let bin = dir.join("multicore.bdgmir");
    assert!(lang().args(["compile", "tests/projects/multicore", "-o"]).arg(&bin).status().unwrap().success());
    let out = lang().arg("run").arg(&bin).output().unwrap();
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "workers: 4\nper-producer order preserved: true\nmessages: 12000\nsome producer moved between workers: true\n"
    );
    // `badger compile --workers N` saves the worker count
    let bin2 = dir.join("multicore2.bdgmir");
    assert!(
        lang()
            .args(["compile", "tests/projects/multicore", "--workers", "2", "-o"])
            .arg(&bin2)
            .status()
            .unwrap()
            .success()
    );
    let out = lang().arg("run").arg(&bin2).output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("workers: 2\n"));
    // a file that is not a program is not mistaken for one
    let junk = dir.join("junk.bdgmir");
    std::fs::write(&junk, b"LANGMIR1\x05").unwrap();
    let out = lang().arg("run").arg(&junk).output().unwrap();
    assert!(!out.status.success());
    // a corrupted byte anywhere is refused cleanly, never run
    let good = std::fs::read(dir.join("dyn_protocol.bdgmir")).unwrap();
    for i in [good.len() / 3, good.len() / 2, good.len() - 5] {
        let mut bad = good.clone();
        bad[i] ^= 0x10;
        let p = dir.join("corrupt.bdgmir");
        std::fs::write(&p, &bad).unwrap();
        let out = lang().arg("run").arg(&p).output().unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(1), "{err}");
        assert!(err.contains("E0011") && !err.contains("panicked"), "{err}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// `badger run --bench` times the execution after compilation on stderr and
/// leaves the program's output alone.
#[test]
fn run_bench_reports_execution_time() {
    let out = lang().args(["run", "--bench", "examples/01_basics.bdg"]).output().unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "The answer is twelve; 5! is 120\n");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.starts_with("bench: run ") && err.contains("ms (compilation "), "{err}");
    // also when the program ends itself with `process.exit`
    let out = lang().args(["run", "--bench", "tests/runtime/exit_early.bdg"]).output().unwrap();
    assert_eq!(out.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&out.stderr).contains("bench: run "));
}

/// Receiving a batch allocates its lease object while the batch's
/// messages must stay reachable: collecting at every allocation
/// (`BADGER_GC_STRESS=1`) must not free them.
#[test]
fn batch_receive_under_gc_stress() {
    let out = lang()
        .args(["run", "tests/runtime/batch_gc_stress.bdg"])
        .env("BADGER_GC_STRESS", "1")
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "32 messages, checksum 48496\n");
}

/// Module-level integer values are not GC roots: collecting at every
/// allocation must not trace them as pointers.
#[test]
fn module_values_under_gc_stress() {
    for name in ["consts", "module_destructuring", "ch01_pos", "decode_scalars"] {
        let src = root().join(format!("tests/run/{name}.bdg"));
        let out = lang().arg("run").arg(&src).env("BADGER_GC_STRESS", "1").output().unwrap();
        let expected = std::fs::read_to_string(src.with_extension("out")).unwrap();
        assert_eq!(
            String::from_utf8_lossy(&out.stdout),
            expected,
            "{name}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// Map and set versions share managed trie nodes: collecting at every
/// allocation must keep every live version's nodes, keys, and values.
#[test]
fn map_versions_under_gc_stress() {
    let src = root().join("tests/run/map_versions.bdg");
    let out = lang().arg("run").arg(&src).env("BADGER_GC_STRESS", "1").output().unwrap();
    let expected = std::fs::read_to_string(src.with_extension("out")).unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), expected, "{}", String::from_utf8_lossy(&out.stderr));
}

/// `BADGER_PROFILE=FILE` writes collapsed stacks naming generated functions,
/// and `BADGER_PERF_MAP=1` writes a perf symbol map for them.
#[test]
fn sampling_profile() {
    let dir = std::env::temp_dir().join(format!("badger-profile-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("spin.bdg");
    std::fs::write(
        &src,
        "import std.time\n\
         fn spin(start: Instant) -> Int {\n\
         \x20   var n = 0\n\
         \x20   while time.now() - start < 300ms { n = n + 1 }\n\
         \x20   n\n\
         }\n\
         pub fn main() -> Unit { let _ = spin(time.now()) }\n",
    )
    .unwrap();
    let folded = dir.join("spin.folded");
    let child = common::Reaped::new(
        lang()
            .args(["run", "--release"])
            .arg(&src)
            .env("BADGER_PROFILE", &folded)
            .env("BADGER_PERF_MAP", "1")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let perf_map = std::path::PathBuf::from(format!("/tmp/perf-{}.map", child.id()));
    let out = child.output();
    let symbols = std::fs::read_to_string(&perf_map).unwrap_or_default();
    let _ = std::fs::remove_file(&perf_map);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    // Inlining followed by reachability pruning may remove spin's body.
    assert!(symbols.lines().any(|l| l.ends_with(" spin.spin") || l.ends_with(" spin.main")), "{symbols}");
    let stacks = std::fs::read_to_string(&folded).unwrap();
    // (optimized code may inline `spin` into `main`)
    assert!(stacks.lines().any(|l| l.contains("spin.spin") || l.contains("spin.main")), "{stacks}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("[profile]"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A program whose only task is busy still reaches step boundaries for
/// scheduler work: a remote evaluation answers while it computes.
#[test]
fn eval_in_busy_program() {
    use std::io::Write;
    use std::time::{Duration, Instant};
    let dir = std::env::temp_dir().join(format!("badger-busy-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("busy.bdg");
    std::fs::write(
        &src,
        "import std.time\npub fn main() -> Unit {\n    let start = time.now()\n    var n = 0\n    while time.now() - start < 20s { n = n + 1 }\n}\n",
    )
    .unwrap();
    let port = 25000 + (std::process::id() % 1000) as u16;
    let addr = format!("127.0.0.1:{port}");
    let mut child = common::Reaped::new(lang().args(["run", "--inspect", &addr]).arg(&src).spawn().unwrap());
    let start = Instant::now();
    let mut reply = String::new();
    while start.elapsed() < Duration::from_secs(15) {
        let mut repl = lang()
            .args(["repl", "--connect", &addr])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        repl.stdin.take().unwrap().write_all(b"40 + 2\n").unwrap();
        reply = String::from_utf8_lossy(&repl.wait_with_output().unwrap().stdout).to_string();
        if reply.contains("42") {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(reply.contains("42"), "{reply}");
    assert!(start.elapsed() < Duration::from_secs(15));
}

/// `badger run --debug` registers line tables, locals, and unwind information
/// for generated code: gdb breaks at a source line, shows arguments and
/// the caller's locals, and steps by line (skipped without gdb).
#[test]
fn debugger_sees_generated_code() {
    if Command::new("gdb").arg("--version").output().is_err() {
        eprintln!("gdb not found: skipped");
        return;
    }
    let dir = std::env::temp_dir().join(format!("badger-gdb-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("dbgprog.bdg");
    std::fs::write(
        &src,
        "import std.io\n\nfn area(w: Int, h: Int) -> Int {\n    let a = w * h\n    let b = a + 1\n    b - 1\n}\n\npub fn main() -> Unit {\n    var total = 0\n    for i in 0..3 {\n        let x = area(i, 10)\n        total = total + x\n    }\n    let _ = io.println($\"total {total}\")\n}\n",
    )
    .unwrap();
    let out = Command::new("gdb")
        .args(["-batch", "-nx", "-ex", "set debuginfod enabled off", "-ex", "set breakpoint pending on"])
        .args([
            "-ex",
            "break dbgprog.bdg:5 if w == 2",
            "-ex",
            "run",
            "-ex",
            "info locals",
            "-ex",
            "up",
            "-ex",
            "info locals",
        ])
        .args(["-ex", "down", "-ex", "next", "-ex", "p b", "-ex", "delete", "-ex", "continue", "--args"])
        .arg(env!("CARGO_BIN_EXE_badger"))
        .args(["run", "--debug"])
        .arg(&src)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(text.contains("dbgprog.area#1 (w=2, h=10)"), "{text}");
    assert!(text.contains("a = 20"), "{text}");
    assert!(text.contains("total = 10") && text.contains("i = 2"), "{text}");
    assert!(text.contains("$1 = 21"), "{text}");
    assert!(text.contains("total 30"), "{text}");
}

/// `badger graph`: calls between functions, types used by fields, and
/// imports, in each output format.
#[test]
fn program_graphs() {
    let dir = std::env::temp_dir().join(format!("badger-graph-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("shapes.bdg");
    std::fs::write(
        &src,
        "import std.io\n\npub struct Point { pub x: Int, pub y: Int }\npub struct Line { pub a: Point, pub b: Point }\n\nfn len2(l: Line) -> Int { dx(l) * dx(l) }\nfn dx(l: Line) -> Int { l.b.x - l.a.x }\n\npub fn main() -> Unit {\n    let _ = io.println($\"{len2({ a: { x: 0, y: 0 }, b: { x: 3, y: 4 } })}\")\n}\n",
    )
    .unwrap();
    let run = |args: &[&str]| {
        let out = lang().args(args).arg(&src).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).to_string()
    };
    let calls = run(&["graph", "calls", "--format", "json"]);
    assert!(calls.contains(r#"{"from":"shapes.main","to":"shapes.len2"}"#), "{calls}");
    assert!(calls.contains(r#"{"from":"shapes.len2","to":"shapes.dx"}"#), "{calls}");
    assert!(!calls.contains("println"), "{calls}");
    assert!(run(&["graph", "calls", "--std", "--format", "json"]).contains("std.io.println"));
    let types = run(&["graph", "types"]);
    assert!(types.contains(r#""shapes.Line" -> "shapes.Point";"#), "{types}");
    let modules = run(&["graph", "modules", "--std", "--format", "mermaid"]);
    assert!(modules.starts_with("graph LR\n") && modules.contains("std.io"), "{modules}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A failed precondition blames the call site and names the contract; a
/// failed postcondition is the function's own defect.
#[test]
fn contract_violations() {
    let dir = std::env::temp_dir().join(format!("badger-contracts-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("acct.bdg");
    let program = |call: &str| {
        format!(
            "requires amount > 0, \"a positive amount\"\nfn withdraw(balance: Int, amount: Int) -> Int {{ balance - amount }}\n\nensures result == x + 1\nfn inc(x: Int) -> Int {{ x + 2 }}\n\npub fn main() -> Unit {{\n    let _ = {call}\n}}\n// what a function without `ensures` returns is not known statically\nfn unknown(n: Int) -> Int {{ n }}\n"
        )
    };
    // a violation the compiler can see is an error before the program runs
    std::fs::write(&src, program("withdraw(10, -5)")).unwrap();
    let out = lang().arg("run").arg(&src).output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(!out.status.success() && err.contains("E0145") && err.contains("amount > 0"), "{err}");
    // one it cannot is a defect when the program runs
    std::fs::write(&src, program("withdraw(10, unknown(-5))")).unwrap();
    let out = lang().arg("run").arg(&src).output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert_eq!(out.status.code(), Some(101), "{err}");
    assert!(err.contains("precondition of `withdraw` failed: amount > 0 (a positive amount)"), "{err}");
    assert!(err.contains("acct.bdg:1:1, violated by the call") && err.contains("acct.bdg:8:13"), "{err}");
    std::fs::write(&src, program("inc(1)")).unwrap();
    let out = lang().arg("run").arg(&src).output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        err.contains("postcondition of `inc` failed: result == x + 1") && err.contains("acct.bdg:4:1"),
        "{err}"
    );
    std::fs::write(&src, program("withdraw(10, 5) + inc(-2) * 0")).unwrap();
    let out = lang().arg("run").arg(&src).output().unwrap();
    assert!(!out.status.success());
    let _ = std::fs::remove_dir_all(&dir);
}

/// `badger contracts`: each clause's static status, and what proven ones
/// leave out of the compiled program.
#[test]
fn contract_report() {
    let out = lang().args(["contracts", "--json", "tests/run/contracts.bdg"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let status = |f: &str, kind: &str| {
        v["clauses"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["function"] == f && c["kind"] == kind)
            .map(|c| c["status"].as_str().unwrap_or("").to_string())
            .unwrap_or_default()
    };
    // abs returns -x only when x < 0, and x after that guard
    assert_eq!(status("abs", "ensures"), "proven");
    // the recursive call is in the n > 1 branch
    assert_eq!(status("factorial", "requires"), "proven");
    assert_eq!(status("factorial", "ensures"), "proven");
    // withdraw_twice passes an amount it knows nothing about
    assert_eq!(status("withdraw", "requires"), "runtime");
    assert_eq!(status("largest", "ensures"), "runtime");
}

/// `badger contracts --tests`: which clauses the tests exercise, and
/// candidate clauses from what held in every observed call.
#[test]
fn contract_test_report() {
    let out =
        lang().args(["contracts", "--tests", "--json", "tests/contracts/inventory.bdg"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["tests"]["ran"], 3);
    let clause =
        |f: &str| v["clauses"].as_array().unwrap().iter().find(|c| c["function"] == f).cloned().unwrap();
    assert_eq!(clause("restock")["tested"]["evaluated"], 3);
    assert_eq!(clause("restock")["tested"]["held"], 3);
    // a clause no test reaches is a gap
    assert_eq!(clause("reorder_after")["tested"]["evaluated"], 0);
    let suggested: Vec<String> = v["suggestions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| format!("{} {}", s["function"].as_str().unwrap(), s["clause"].as_str().unwrap()))
        .collect();
    assert!(suggested.contains(&"take ensures result >= 0".to_string()), "{suggested:?}");
    // nothing is suggested about what a clause already states
    assert!(!suggested.iter().any(|s| s.starts_with("restock requires qty")), "{suggested:?}");
}

/// `badger query api`: a standard module's public functions, types, and
/// methods with signatures and documentation, and a program's own.
#[test]
fn api_export() {
    let out = lang().args(["query", "api", "std.list"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let api = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        api.contains(r#""name":"List.map","signature":"map: fn<U>(&List<T>, fn(T) -> U) -> List<U>""#),
        "{api}"
    );
    assert!(api.contains(r#""doc":"Collect a sequence into a list.","kind":"function","module":"std.list","name":"collect""#), "{api}");
    let out = lang().args(["query", "api", "tests/run/contracts.bdg"]).output().unwrap();
    let api = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(api.contains(r#""name":"withdraw""#) && api.contains("requires amount > 0"), "{api}");
    assert!(api.contains(r#""name":"Range.width""#), "{api}");
    assert!(!api.contains(r#""name":"abs""#), "private functions are not part of the API: {api}");
}

/// Diagnostics never disappear: a missing file, and a module its project
/// does not import, are still reported by `badger query diagnostics`.
#[test]
fn query_reports_unloadable_and_unreachable_files() {
    let out = lang().args(["query", "diagnostics", "/nonexistent/x.bdg"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(text.contains("E0011"), "{text}");
    let dir = std::env::temp_dir().join(format!("badger-orphan-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("badger.toml"), "[package]\nname = \"orph\"\nversion = \"0.1.0\"\n").unwrap();
    std::fs::write(dir.join("src/main.bdg"), "pub fn main() -> Unit {}\n").unwrap();
    std::fs::write(dir.join("src/orphan.bdg"), "fn lonely() -> Int { \"s\" }\n").unwrap();
    let out = lang().args(["query", "diagnostics"]).arg(dir.join("src/orphan.bdg")).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(text.contains("E0300"), "{text}");
    // a missing file inside a project is reported, not replaced by a
    // (passing) check of the project
    let out = lang().args(["query", "diagnostics"]).arg(dir.join("src/typo.bdg")).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(text.contains("E0011") && text.contains("typo.bdg"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Optimized builds inline small helpers and then remove the record built
/// only to be passed to one (`BADGER_NO_INLINE=1` keeps the call).
#[test]
fn inlining_removes_helper_calls_and_allocations() {
    let dir = std::env::temp_dir().join(format!("badger-inline-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("inl.bdg");
    std::fs::write(
        &src,
        "import std.io\n\nfn area(r: { w: Int, h: Int }) -> Int { r.w * r.h }\n\npub fn main() -> Unit {\n    var total = 0\n    for i in 0..1000 {\n        total = total + area({ w: i, h: 2 })\n    }\n    let _ = io.println($\"{total}\")\n}\n",
    )
    .unwrap();
    let main_mir = |inline: bool| {
        let mut c = lang();
        c.args(["run", "--release", "--dump-mir"]).arg(&src);
        if !inline {
            c.env("BADGER_NO_INLINE", "1");
        }
        let out = c.output().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), "999000\n");
        let mir = String::from_utf8_lossy(&out.stderr).to_string();
        mir.split("\nfn 1 ").next().unwrap_or("").to_string()
    };
    let plain = main_mir(false);
    assert!(plain.contains("Alloc {") && plain.contains("Direct(1)"), "{plain}");
    let inlined = main_mir(true);
    assert!(!inlined.contains("Alloc {") && !inlined.contains("Direct(1)"), "{inlined}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Restricted projects: modules other than the entry work through the
/// capabilities they are given; ambient authority reached directly or
/// through a wrapper (even one in the entry module) is rejected with its path.
#[test]
fn restricted_projects() {
    let out =
        lang().args(["run", "tests/projects/restricted"]).env("INVENTORY_LABEL", "spring").output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "stock: 3 apples\nlabel spring, other variables false, other programs refused true\n"
    );
    // checking one of its files is checking it as part of the project
    let file =
        lang().args(["check", "--json", "tests/projects/restricted_bypass/src/main.bdg"]).output().unwrap();
    assert!(!file.status.success() && String::from_utf8_lossy(&file.stdout).contains("\"code\":\"E0170\""));
    let out = lang().args(["check", "--json", "tests/projects/restricted_bypass"]).output().unwrap();
    assert!(!out.status.success());
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(text.contains("`worker.direct` uses ambient authority `env.get`"), "{text}");
    assert!(
        text.contains("`worker.indirect` uses ambient authority `fs.read_text` through `main.load`"),
        "{text}"
    );
    assert!(
        text.contains("`worker.foreign` uses ambient authority `extern")
            && text.contains("fn worker.getpid`"),
        "{text}"
    );
    assert!(text.contains("`worker.fetching` uses ambient authority `fetch.get`"), "{text}");
    assert!(text.contains("`worker.settings` uses ambient authority `config.from_env`"), "{text}");
    assert!(text.contains("`worker.database` uses ambient authority `sqlite.pool`"), "{text}");
    assert_eq!(text.matches("E0170").count(), 6, "{text}");
}

/// A local HTTP server answering each request with `reply(method, path,
/// body) -> (status line, extra headers, body)`; returns its port.
fn http_server(reply: impl Fn(&str, &str, &str) -> (String, String, String) + Send + 'static) -> u16 {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            let mut conn = conn;
            // the head, then as much body as it declares
            let (mut buf, mut chunk) = (vec![], [0u8; 4096]);
            let req = loop {
                let n = conn.read(&mut chunk).unwrap_or(0);
                buf.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&buf).to_string();
                let complete = text.split_once("\r\n\r\n").is_some_and(|(head, body)| {
                    let len = head
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse().unwrap_or(0))
                        })
                        .unwrap_or(0);
                    body.len() >= len
                });
                if n == 0 || complete {
                    break text;
                }
            };
            let mut words = req.split_whitespace();
            let (method, path) =
                (words.next().unwrap_or("").to_string(), words.next().unwrap_or("/").to_string());
            let body = req.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
            let (status, extra, text) = reply(&method, &path, &body);
            let _ = write!(
                conn,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{text}",
                text.len()
            );
        }
    });
    port
}

/// `std.fetch` against a local server: replies of every status, an
/// origin-bound client that refuses other origins, unfollowed redirects.
#[test]
fn outbound_http() {
    let port = http_server(|_, path, body| {
        let (status, text, extra) = match path {
            "/hello" => ("200 OK", "hi".to_string(), ""),
            "/echo" => ("200 OK", format!("got {body}"), ""),
            "/moved" => ("302 Found", "moved".to_string(), "Location: http://example.com/\r\n"),
            _ => ("404 Not Found", "none".to_string(), ""),
        };
        (status.to_string(), extra.to_string(), text)
    });
    let out = lang()
        .args(["run", "tests/net/fetch.bdg"])
        .env("FETCH_BASE", format!("http://127.0.0.1:{port}"))
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "get 200 hi\npost 200 got ping\nclient 200 hi\nclient post 200 got pong\nredirect 302 moved [content-length=5,connection=close,location=http://example.com/]\nmissing 404 none\nother origin refused (io/permission_denied)\nno slash refused (io/permission_denied)\nbad origin true true\nunreachable refused (io/connection_failed)\n"
    );
}

/// `std.fetch.send` and the JSON helpers against a local server.
#[test]
fn outbound_http_send() {
    let port = http_server(|method, path, body| {
        let json = "Content-Type: application/json\r\n";
        let (status, extra, text) = match path {
            "/json" => ("200 OK", json.to_string(), r#"{"n":1,"tags":["a","b"]}"#.to_string()),
            "/echo" => ("200 OK", json.to_string(), body.to_string()),
            "/method" => ("200 OK", "Content-Type: text/plain\r\n".to_string(), method.to_string()),
            "/hop" => ("302 Found", "Location: /json\r\n".to_string(), String::new()),
            "/big" => ("200 OK", String::new(), "x".repeat(100)),
            _ => ("500 Internal Server Error", String::new(), "no".to_string()),
        };
        (status.to_string(), extra, text)
    });
    let out = lang()
        .args(["run", "tests/net/fetch_send.bdg"])
        .env("FETCH_BASE", format!("http://127.0.0.1:{port}"))
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "get_json 1 [\"a\", \"b\"]\npost_json 7 [\"x\", \"y\"]\npatch 200 PATCH type text/plain\nfollowed 200 true {\"n\":1,\"tags\":[\"a\",\"b\"]}\nnot followed 302 /json\nlimit io/too_large\nstatus io/http_status\nclient 200 DELETE success true\n"
    );
}

/// An inlined lambda reports a defect at its own source position.
#[test]
fn inlined_lambda_defect_location() {
    for flags in [&[][..], &["--release"][..]] {
        let out = lang().arg("run").args(flags).arg("tests/run/known_closures.bdg").output().unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("integer overflow at tests/run/known_closures.bdg:81:28"), "{flags:?}: {err}");
    }
}

/// Reproduction bundles (`badger run --record`, `badger replay`): an injected
/// timeout is reproduced offline, an exact replay refuses changed code, a
/// fixed transformation is compared against the recording without writing,
/// a changed operation sequence diverges, and credentials are not stored.
#[test]
fn reproduction_bundles() {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    let requests = Arc::new(AtomicUsize::new(0));
    let slow = Arc::new(AtomicBool::new(false));
    let orders = Arc::new(Mutex::new("10, 20, x30".to_string()));
    let (r, sl, o) = (requests.clone(), slow.clone(), orders.clone());
    let port = http_server(move |_, path, _| {
        r.fetch_add(1, Ordering::SeqCst);
        match path {
            "/orders" => {
                if sl.load(Ordering::SeqCst) {
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                }
                ("200 OK".into(), String::new(), o.lock().unwrap().clone())
            }
            "/report" => ("201 Created".into(), String::new(), "ok".into()),
            _ => ("404 Not Found".into(), String::new(), String::new()),
        }
    });
    let base = format!("http://127.0.0.1:{port}");
    let dir = std::env::temp_dir().join(format!("badger-repro-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let source = std::fs::read_to_string(root().join("tests/net/reconcile.bdg")).unwrap();
    std::fs::write(dir.join("reconcile.bdg"), &source).unwrap();
    let run = |args: &[&str], env: &[(&str, &str)]| {
        let mut c = Command::new(env!("CARGO_BIN_EXE_badger"));
        c.current_dir(&dir).args(args);
        for (k, v) in env {
            c.env(k, v);
        }
        let out = c.output().unwrap();
        (
            out.status.code(),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        )
    };
    // a run with a bug in its transformation, recorded
    let (code, out, err) = run(&["run", "--record", "bug.json", "reconcile.bdg"], &[("FETCH_BASE", &base)]);
    assert_eq!((code, out.as_str()), (Some(0), "total 30\nreported 201\n"), "{err}");
    assert_eq!(requests.load(Ordering::SeqCst), 2);
    // replayed exactly: the same outcome, and nothing reaches the server
    let (code, out, err) = run(&["replay", "bug.json"], &[]);
    assert_eq!((code, out.as_str()), (Some(0), "total 30\nreported 201\n"), "{err}");
    assert!(err.contains("reproduced the recorded outcome (exit 0)"), "{err}");
    assert_eq!(requests.load(Ordering::SeqCst), 2);
    // fixed: an exact replay refuses the changed program; a comparison runs
    // it against the recording, still without writing
    let fixed =
        source.replace("text.parse_int(s.trim())", "text.parse_int(text.replace(s.trim(), \"x\", \"\"))");
    assert_ne!(fixed, source);
    std::fs::write(dir.join("reconcile.bdg"), &fixed).unwrap();
    let (code, _, err) = run(&["replay", "bug.json"], &[]);
    assert_eq!(code, Some(1), "{err}");
    assert!(err.contains("E0016") && err.contains("reconcile.bdg changed"), "{err}");
    let (code, out, err) = run(&["replay", "--compare", "bug.json"], &[]);
    assert_eq!((code, out.as_str()), (Some(0), "total 60\nreported 201\n"), "{err}");
    assert!(
        err.contains("`fetch POST http://127.0.0.1:") && err.contains("sends different content"),
        "{err}"
    );
    assert!(err.contains("the outcome differs from the recording"), "{err}");
    assert_eq!(requests.load(Ordering::SeqCst), 2);
    // a changed operation sequence diverges instead of reaching the network
    std::fs::write(dir.join("reconcile.bdg"), fixed.replace("\"/orders\"", "\"/orders/all\"")).unwrap();
    let (code, _, err) = run(&["replay", "--compare", "bug.json"], &[]);
    assert_eq!(code, Some(3), "{err}");
    assert!(
        err.contains("replay diverged: `fetch GET http://127.0.0.1:")
            && err.contains("/orders/all` was not recorded"),
        "{err}"
    );
    assert_eq!(requests.load(Ordering::SeqCst), 2);
    // an injected timeout, recorded, reproduces offline
    std::fs::write(dir.join("reconcile.bdg"), &source).unwrap();
    slow.store(true, Ordering::SeqCst);
    let (code, _, err) = run(&["run", "--record", "timeout.json", "reconcile.bdg"], &[("FETCH_BASE", &base)]);
    assert_eq!(code, Some(1), "{err}");
    assert!(err.contains("io/timed_out"), "{err}");
    let bundle = std::fs::read_to_string(dir.join("timeout.json")).unwrap();
    assert!(bundle.contains("let reply = c.get(\\\"/orders\\\")?"), "the failing line is kept: {bundle}");
    let before = requests.load(Ordering::SeqCst);
    let (code, _, err) = run(&["replay", "timeout.json"], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        err.contains("reproduced the recorded outcome (exit 1)") && err.contains("io/timed_out"),
        "{err}"
    );
    assert_eq!(requests.load(Ordering::SeqCst), before, "the replay does not reach the server");
    // other operations: random seeds, clocks, a subprocess, files, exit
    let recorded = std::fs::read_to_string(root().join("tests/net/recorded.bdg")).unwrap();
    std::fs::write(dir.join("recorded.bdg"), recorded).unwrap();
    let (code, first, err) = run(&["run", "--record", "ops.json", "recorded.bdg"], &[("OUT", "out.txt")]);
    assert_eq!(code, Some(4), "{err}");
    std::fs::remove_file(dir.join("out.txt")).unwrap();
    let (code, again, err) = run(&["replay", "ops.json"], &[]);
    assert_eq!((code, again), (Some(0), first), "{err}");
    assert!(!dir.join("out.txt").exists(), "a replay does not write files");
    // credentials are not stored, and a replay that needs one stops
    let (code, _, err) = run(
        &["run", "--record", "token.json", "recorded.bdg"],
        &[("VERBOSE", "1"), ("API_TOKEN", "s3cret-value")],
    );
    assert_eq!(code, Some(4), "{err}");
    assert!(!std::fs::read_to_string(dir.join("token.json")).unwrap().contains("s3cret-value"));
    let (code, _, err) = run(&["replay", "token.json"], &[]);
    assert_eq!(code, Some(3), "{err}");
    assert!(err.contains("`env API_TOKEN` was redacted"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `std.jobs` across crashes: a worker that dies before committing is
/// fenced out and its job retaken after the lease; an event is ingested
/// once; a dispatcher that dies after the destination accepted a charge
/// leaves it uncertain, so it is reconciled, not charged twice.
#[test]
fn durable_jobs_survive_crashes() {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    // the destination: charges by key, each POST counted
    let charges: Arc<Mutex<HashMap<String, (String, usize)>>> = Arc::default();
    let ch = charges.clone();
    let port = http_server(move |method, path, body| {
        let mut c = ch.lock().unwrap();
        match (method, path.strip_prefix("/charges")) {
            ("POST", Some("")) => {
                let key = body.split(' ').next().unwrap_or("").to_string();
                let n = c.len() + 1;
                let entry = c.entry(key).or_insert((format!("charge-{n}"), 0));
                entry.1 += 1;
                ("201 Created".into(), String::new(), entry.0.clone())
            }
            ("GET", Some(key)) => match c.get(key.trim_start_matches('/')) {
                Some((receipt, _)) => ("200 OK".into(), String::new(), receipt.clone()),
                None => ("404 Not Found".into(), String::new(), String::new()),
            },
            _ => ("404 Not Found".into(), String::new(), String::new()),
        }
    });
    let dir = std::env::temp_dir().join(format!("badger-jobs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("jobs.db");
    let run = |args: &[&str]| {
        let out = lang()
            .args(["run", "tests/jobs/charge.bdg", "--"])
            .args(args)
            .env("JOBS_DB", &db)
            .env("CHARGES", format!("http://127.0.0.1:{port}"))
            .output()
            .unwrap();
        (
            out.status.code(),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        )
    };
    let ok = |r: (Option<i32>, String, String)| {
        assert_eq!(r.0, Some(0), "{}{}", r.1, r.2);
        r.1
    };
    assert_eq!(ok(run(&["submit", "e1", "100"])), "created: order:e1\n");
    assert_eq!(ok(run(&["submit", "e1", "100"])), "duplicate: order:e1\n");
    // a worker dies holding the job: nobody else gets it until the lease
    // expires, then it is retaken as a failed attempt
    assert_eq!(run(&["work", "crash"]).0, Some(9));
    assert_eq!(ok(run(&["work"])), "nothing due\n");
    std::thread::sleep(std::time::Duration::from_millis(4200));
    assert_eq!(ok(run(&["work"])), "order:e1 decided after 1 failed attempt(s)\n");
    assert_eq!(ok(run(&["submit", "e2", "50"])), "created: order:e2\n");
    assert_eq!(ok(run(&["work"])), "order:e2 decided after 0 failed attempt(s)\n");
    // the dispatcher dies after the first charge was accepted
    assert_eq!(run(&["dispatch", "crash"]).0, Some(9));
    assert_eq!(charges.lock().unwrap()["order:e1/charge"].1, 1);
    // after its deadline it is uncertain and not sent again; the other is
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert_eq!(ok(run(&["dispatch"])), "order:e2/charge: confirmed charge-2\n");
    let status = ok(run(&["status"]));
    assert!(
        status.contains("operation order:e1/charge uncertain sending was interrupted sent 1x"),
        "{status}"
    );
    // reconciliation asks the destination by key
    assert_eq!(ok(run(&["reconcile"])), "order:e1/charge: confirmed charge-1\n");
    let status = ok(run(&["status"]));
    assert_eq!(
        status,
        "job order:e1 done charged attempts 0\njob order:e2 done charged attempts 0\noperation order:e1/charge confirmed charge-1 sent 1x\noperation order:e2/charge confirmed charge-2 sent 1x\n"
    );
    // each charge reached the destination exactly once
    let c = charges.lock().unwrap();
    assert_eq!((c.len(), c.values().map(|v| v.1).sum::<usize>()), (2, 2));
    drop(c);
    assert_eq!(ok(run(&["dispatch"])), "");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Typed SQL against a project's `[database] schema` file: literal
/// statements and row types are checked when the program is.
#[test]
fn typed_sql_schema() {
    let out = lang().args(["run", "tests/projects/typed_sql"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "ann 15 -\n");
    // a copy whose statements no longer match the schema
    let dir = std::env::temp_dir().join(format!("badger-typed-sql-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    let src = root().join("tests/projects/typed_sql");
    for f in ["badger.toml", "schema.sql"] {
        std::fs::copy(src.join(f), dir.join(f)).unwrap();
    }
    let main = std::fs::read_to_string(src.join("src/main.bdg")).unwrap();
    let broken = main
        .replace("SET balance = balance + ?", "SET balanse = balanse + ?")
        .replace("note: Option<String>,", "note: String,")
        .replace("a.note.unwrap_or(\"-\")", "a.note");
    assert_ne!(broken, main);
    std::fs::write(dir.join("src/main.bdg"), broken).unwrap();
    let out = lang().args(["check", "--json"]).arg(&dir).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(!out.status.success(), "{text}");
    assert!(text.contains("E0180") && text.contains("no such column: balanse"), "{text}");
    assert!(text.contains("E0182") && text.contains("column `note` can be NULL"), "{text}");
    // a schema that is missing is reported
    std::fs::remove_file(dir.join("schema.sql")).unwrap();
    let out = lang().args(["check"]).arg(&dir).output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr).to_string() + &String::from_utf8_lossy(&out.stdout);
    assert!(err.contains("cannot read the database schema"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Updating a struct outside its module needs every field to be public.
#[test]
fn private_struct_update() {
    let out = lang().args(["check", "--json", "tests/projects/private_update"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(!out.status.success());
    assert_eq!(text.matches("\"code\":\"E0308\"").count(), 1, "{text}");
    assert!(text.contains("field `secret` is private") && text.contains("\"line\":12"), "{text}");
}

/// Inherent methods are private to their module unless `pub`, by method
/// call and by path; protocol methods stay usable.
#[test]
fn private_method_call() {
    let out = lang().args(["check", "--json", "tests/projects/private_method"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(!out.status.success());
    assert_eq!(text.matches("\"code\":\"E0112\"").count(), 2, "{text}");
    assert!(text.contains("\"line\":8,\"column\":18") && text.contains("\"line\":9,\"column\":26"), "{text}");
    assert_eq!(text.matches("\"code\"").count(), 2, "{text}");
}

/// Cleanup that never finishes is escalated after one grace period, not
/// waited for forever (or once per waiter).
#[test]
fn stuck_cleanup_is_bounded() {
    let started = std::time::Instant::now();
    let out = lang()
        .args(["run", "tests/cleanup/stuck_timeout.bdg"])
        .env("BADGER_CLEANUP_GRACE_MS", "1500")
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(101), "{err}");
    assert_eq!(err.matches("cleanup exceeded the grace period of 1500ms").count(), 1, "{err}");
    assert!(started.elapsed() < std::time::Duration::from_secs(20));
}

/// A release runs from anywhere with an empty environment (no sources, no
/// search path), and refuses to start once a file it records has changed.
#[test]
fn release_starts_without_sources() {
    let base = std::env::temp_dir().join(format!("badger-release-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let built = base.join("built");
    let out = lang().args(["release", "tests/native/demo.bdg", "-o"]).arg(&built).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    // moved away from where it was built, started with nothing inherited
    let moved = base.join("moved");
    std::fs::rename(&built, &moved).unwrap();
    let run = Command::new(moved.join("run")).env_clear().current_dir("/").output().unwrap();
    let want = std::fs::read_to_string(root().join("tests/native/demo.out")).unwrap();
    assert_eq!(String::from_utf8_lossy(&run.stdout), want, "{}", String::from_utf8_lossy(&run.stderr));
    // a changed native library is refused before the program runs
    let lib = moved.join("lib/badger/libbadger_native_demo.so");
    let mut bytes = std::fs::read(&lib).unwrap();
    bytes.push(0);
    std::fs::write(&lib, &bytes).unwrap();
    let run = Command::new(moved.join("run")).env_clear().output().unwrap();
    let err = String::from_utf8_lossy(&run.stderr);
    assert!(!run.status.success() && err.contains("has changed since the release was built"), "{err}");
    assert!(run.stdout.is_empty());
    // something that is not a release is never overwritten
    let other = base.join("other");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join("keep.txt"), "mine").unwrap();
    let out = lang().args(["release", "tests/native/demo.bdg", "-o"]).arg(&other).output().unwrap();
    assert!(
        !out.status.success() && String::from_utf8_lossy(&out.stderr).contains("not a release directory")
    );
    assert!(other.join("keep.txt").is_file());
    let _ = std::fs::remove_dir_all(&base);
}

/// Deeply nested programs get a diagnostic, not a crash or a hang: a long
/// operator chain past the expression depth limit, brackets past the
/// nesting limit; just under the limits they compile (even in debug
/// builds, on the CLI's large stack), and nested parentheses parse in
/// linear time.
#[test]
fn deep_nesting_is_refused_cleanly() {
    let dir = std::env::temp_dir().join(format!("badger-deep-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let check = |name: &str, src: String| {
        let f = dir.join(name);
        std::fs::write(&f, src).unwrap();
        let started = std::time::Instant::now();
        let out = lang().arg("check").arg(&f).output().unwrap();
        (out.status.code(), String::from_utf8_lossy(&out.stderr).into_owned(), started.elapsed())
    };
    let chain = |n: usize| {
        format!("pub fn main() {{\n    let x = {}\n    assert(x == {n})\n}}\n", vec!["1"; n].join(" + "))
    };
    let (code, err, _) = check("long.bdg", chain(1999));
    assert_eq!(code, Some(0), "{err}");
    let (code, err, _) = check("longer.bdg", chain(5000));
    assert!(code == Some(1) && err.contains("nested more than 2000 deep"), "{code:?} {err}");
    let parens =
        |n: usize| format!("pub fn main() {{\n    let x = {}1{}\n}}\n", "(".repeat(n), ")".repeat(n));
    let (code, err, took) = check("parens.bdg", parens(900));
    assert!(code == Some(0) && took < std::time::Duration::from_secs(20), "{code:?} {took:?} {err}");
    let (code, err, _) = check("deeper.bdg", parens(1500));
    assert!(code == Some(1) && err.contains("brackets nested more than 1000 deep"), "{code:?} {err}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Two imports binding one name to different items are an error, not a
/// silent choice of the later one.
#[test]
fn conflicting_imports_are_reported() {
    let out = lang().args(["check", "--json", "tests/projects/import_conflict"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(!out.status.success());
    assert!(text.contains("\"code\":\"E0141\"") && text.contains("`greet` is imported twice"), "{text}");
}
