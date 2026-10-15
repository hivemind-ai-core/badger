//! End-to-end tests: `tests/run/*.lang` must run and print `NAME.out`
//! (exit code `NAME.exit`, default 0); `tests/reject/*.lang` must fail to
//! check with every diagnostic code listed in `// expect-error: CODE` lines.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn lang() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_lang"));
    c.current_dir(root());
    c
}

fn files(dir: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(root().join(dir))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "lang"))
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

fn run_all(flags: &[&str]) {
    let mut failures = vec![];
    for f in files("tests/run") {
        let expected = std::fs::read_to_string(f.with_extension("out")).unwrap_or_default();
        let code: i32 =
            std::fs::read_to_string(f.with_extension("exit")).map(|s| s.trim().parse().unwrap()).unwrap_or(0);
        let out = lang().arg("run").args(flags).arg(&f).output().unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        if stdout != expected || out.status.code() != Some(code) {
            failures.push(format!(
                "{}: exit {:?} (want {code})\n--- stdout\n{stdout}--- expected\n{expected}--- stderr\n{}",
                f.display(),
                out.status.code(),
                String::from_utf8_lossy(&out.stderr)
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
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

/// Every example program runs and prints the expected output.
#[test]
fn examples_run() {
    let dir = std::env::temp_dir().join(format!("lang-examples-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("notes.txt"), "first note\n\n  second note  \n").unwrap();
    let cases: &[(&str, &[&str], &str)] = &[
        ("examples/01_basics.lang", &[], "The answer is twelve; 5! is 120\n"),
        ("examples/02_records.lang", &[], ""),
        (
            "examples/03_domain_types.lang",
            &[],
            "Hello Ada, signed in as user-42\nRetry 1\nRetry budget exhausted\nStopping\n",
        ),
        ("examples/04_errors_and_use.lang", &[], "shipment: 3 packages\n"),
        ("examples/05_collections.lang", &[], ""),
        ("examples/06_schemas.lang", &[], ""),
        ("examples/07_tasks.lang", &["README.md", "README.md"], "true\n"),
        ("examples/09_batch_channels.lang", &[], ""),
        ("examples/10_interceptors.lang", &[], ""),
        ("examples/11_systems.lang", &[], "Welcome, Ada\n"),
        ("examples/design/prefix_views.lang", &[], ""),
        ("examples/smoke.lang", &[], "12\n"),
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
    let channels = lang().args(["run", "examples/08_channels.lang"]).output().unwrap();
    let text = String::from_utf8_lossy(&channels.stdout);
    if !channels.status.success() || text.lines().count() < 10 {
        failures.push(format!("08_channels: {text}"));
    }
    let notes = lang()
        .current_dir(&dir)
        .args(["run", root().join("examples/12_import_notes.lang").to_str().unwrap(), "--", "notes.txt"])
        .output()
        .unwrap();
    if String::from_utf8_lossy(&notes.stdout) != "Imported 2 notes\n" {
        failures.push(format!("12_import_notes: {}", String::from_utf8_lossy(&notes.stderr)));
    }
    let jobs = lang().args(["run", "examples/design/batch_jobs.lang"]).output().unwrap();
    let expected: String = (0..100).map(|i| format!("job-{i}\n")).collect();
    if !jobs.status.success() || String::from_utf8_lossy(&jobs.stdout) != expected {
        failures.push(format!("batch_jobs: {}", String::from_utf8_lossy(&jobs.stderr)));
    }
    let normalize =
        lang().args(["run", "examples/design/normalize.lang", "--", "examples/README.md"]).output().unwrap();
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

/// Scenario 16: `lang run --watch` replaces compatible functions while the
/// program runs; an executing frame keeps its version; an incompatible type
/// change is rejected and the running version continues.
#[test]
fn live_reload() {
    use std::io::{BufRead, BufReader, Read};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    let dir = std::env::temp_dir().join(format!("lang-live-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("live.lang");
    let src = |name: &str| std::fs::read_to_string(root().join("tests/live").join(name)).unwrap();
    std::fs::write(&file, src("reload_v1.lang")).unwrap();
    let mut child = lang()
        .args(["run", "--watch", file.to_str().unwrap()])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
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
    std::fs::write(&file, src("reload_incompatible.lang")).unwrap();
    wait_for(&err, "reload rejected");
    std::thread::sleep(Duration::from_millis(150));
    // an effect change is an incompatible contract change too
    std::fs::write(&file, src("reload_effect.lang")).unwrap();
    wait_for(&err, "changed from `fn() -> String` to `fn() -> String !{Io}`");
    std::thread::sleep(Duration::from_millis(150));
    std::fs::write(&file, src("reload_v2.lang")).unwrap();
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

/// `lang inspect WHAT --connect ADDR` reads a program started with
/// `lang run --inspect ADDR`: task wait reasons, channel occupancy, systems.
#[test]
fn inspect_running_program() {
    use std::time::{Duration, Instant};
    let port = 21000 + (std::process::id() % 1000) as u16;
    let addr = format!("127.0.0.1:{port}");
    let mut child = lang()
        .args(["run", "--inspect", &addr, "tests/inspect/running.lang"])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
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

/// `lang repl --connect ADDR` evaluates inside a running program: it reads the
/// program's module-level state, adds definitions, rejects an incompatible
/// replacement, and a compatible replacement changes the running loop.
#[test]
fn remote_repl() {
    use std::io::Write;
    use std::time::Duration;
    let port = 23000 + (std::process::id() % 1000) as u16;
    let addr = format!("127.0.0.1:{port}");
    let program = lang()
        .args(["run", "--inspect", &addr, "tests/inspect/counter.lang"])
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

/// Path dependencies (transitive) resolve through `lang.lock`: a changed
/// dependency is refused until `lang update`; version requirements hold.
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
    let dir = std::env::temp_dir().join(format!("lang-deps-{}", std::process::id()));
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
    let lib = dir.join("mathlib/src/lib.lang");
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
    let manifest = app.join("lang.toml");
    let m = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(&manifest, m.replace("version = \"0.2\"", "version = \"0.3\"")).unwrap();
    let (ok, _, err) = run();
    assert!(!ok && err.contains("requires version 0.3, found 0.2.1"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Scenario 16, incompatible layout: with `--watch --reconstruct` a changed
/// type definition cancels the running program (its resource halts) and
/// restarts it on the new version with freshly built values.
#[test]
fn live_reconstruct() {
    use std::time::{Duration, Instant};
    for workers in ["1", "4"] {
        let dir = std::env::temp_dir().join(format!("lang-reconstruct-{}-{workers}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("app.lang");
        let src = |name: &str| std::fs::read_to_string(root().join("tests/live").join(name)).unwrap();
        std::fs::write(&file, src("reconstruct_v1.lang")).unwrap();
        let out_path = dir.join("out.txt");
        let child = lang()
            .args(["run", "--workers", workers, "--watch", "--reconstruct", file.to_str().unwrap()])
            .stdout(std::fs::File::create(&out_path).unwrap())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let start = Instant::now();
        while !std::fs::read_to_string(&out_path).unwrap_or_default().contains("v1 point 1") {
            assert!(start.elapsed() < Duration::from_secs(20), "version 1 never started");
            std::thread::sleep(Duration::from_millis(20));
        }
        std::fs::write(&file, src("reconstruct_v2.lang")).unwrap();
        let out = child.wait_with_output().unwrap();
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
    let file = root().join("tests/explain/defs.lang");
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
    let (_, out, _) = run(&["run"], &[("LANG_TRACE_CAPACITY", "16")]);
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
    let tmp = std::env::temp_dir().join(format!("lang-strict-{}", std::process::id()));
    std::fs::create_dir_all(tmp.join("src")).unwrap();
    std::fs::write(tmp.join("src/main.lang"), "pub fn main() {}\n").unwrap();
    for (toml, want) in [
        ("[package]\nname = \"x\"\n[runtime]\nwokers = 2\n", "unknown runtime setting `wokers`"),
        ("[package]\nname = \"x\"\n[pakage]\n", "unknown section `[pakage]`"),
        ("[package]\nname = \"x\"\nentrypoint = \"a\"\n", "unknown package key `entrypoint`"),
        ("[package]\nname = \"x\"\n[runtime]\ntrace_overflow = \"block\"\n", "trace_overflow must be"),
        ("[package]\nname = \"x\"\n[tasks]\nbad = \"src/main.lang#nope\"\n", "no `nope` function"),
    ] {
        std::fs::write(tmp.join("lang.toml"), toml).unwrap();
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
    let file = root().join("tests/diagnostics/structured.lang");
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
}

/// `lang test`: controlled clocks are per test and drive sleeps by virtual
/// time; assertion failures report the caller's location.
#[test]
fn test_runner_controlled_clock() {
    let out = lang().args(["test", "tests/testing/clock.lang"]).current_dir(root()).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    for want in [
        "test test_a_manual_clock_sleep ... ok",
        "test test_b_clock_isolated ... ok",
        "test test_c_advance_wakes_sleeper ... ok",
        "test test_d_failure_location ... FAILED (assertion failed: expected 3, found 2 at tests/testing/clock.lang:38:5)",
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
            .args(["test", "tests/testing/simulation.lang"])
            .env("LANG_WORKERS", workers)
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
    let mut child = lang()
        .arg("repl")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let input = "let counter = Atom.new(0)\ncounter.set(5)\ncounter.get()\nlet r = { a: 1, b: \"s\" }\nr\nfn f(n: Int) -> Int { n * 2 }\nf(counter.get())\nstruct P { x: Int }\nlet p: P = { x: 3 }\np\nstruct Q { a: Int }\nstruct Q { a: Int, b: Int }\n:q\n";
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
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
    let out = lang().args(["run", "tests/errors/main_error.lang"]).current_dir(root()).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("error[wrap]: wrap: bad: 4\n  caused by: bad: 4\n  details: {\"wrap\":{\"source\":{\"bad\":{\"n\":4}}}}\n  at tests/errors/main_error.lang:14:16"),
        "{err}"
    );
}

/// When a worker outlives the cleanup grace period the system escalates
/// with a defect and does not free the node's resources under it.
#[test]
fn grace_escalation_keeps_resources() {
    let out = lang()
        .args(["run", "tests/runtime/grace_keeps_resources.lang"])
        .env("LANG_CLEANUP_GRACE_MS", "200")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("failed: cleanup exceeded the grace period of 200ms"), "{text}");
    assert!(text.contains("worker cleanup uses resource alive=1"), "{text}");
    assert!(!text.contains("resource freed"), "{text}");
}

/// `lang compile` saves binary MIR that `lang run` executes without the
/// sources: outputs and exit codes match the source runs.
#[test]
fn compiled_programs_run() {
    let dir = std::env::temp_dir().join(format!("lang-mir-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut failures = vec![];
    for name in [
        "channel_choose",
        "closure_capture_modes",
        "conditional_display",
        "dyn_protocol",
        "ffi_structs",
        "let_polymorphism",
        "pipeline_dynamic",
        "polymorphic_recursion_dynamic",
        "record_contextual",
        "schema_visit",
        "select_rendezvous_oversize",
        "system_lifecycle",
        "scenario05_startup_failure",
        "batch_self_receive",
    ] {
        let src = root().join(format!("tests/run/{name}.lang"));
        let bin = dir.join(format!("{name}.langmir"));
        let c = lang().arg("compile").arg(&src).arg("-o").arg(&bin).output().unwrap();
        if !c.status.success() {
            failures.push(format!("{name}: compile failed: {}", String::from_utf8_lossy(&c.stderr)));
            continue;
        }
        let expected = std::fs::read_to_string(src.with_extension("out")).unwrap_or_default();
        let code: i32 = std::fs::read_to_string(src.with_extension("exit"))
            .map(|s| s.trim().parse().unwrap())
            .unwrap_or(0);
        let out = lang().arg("run").arg(&bin).output().unwrap();
        if String::from_utf8_lossy(&out.stdout) != expected || out.status.code() != Some(code) {
            failures.push(format!(
                "{name}: exit {:?} (want {code})\n{}\n{}",
                out.status.code(),
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ));
        }
    }
    // a project: its saved worker count and configuration are used
    let bin = dir.join("multicore.langmir");
    assert!(lang().args(["compile", "tests/projects/multicore", "-o"]).arg(&bin).status().unwrap().success());
    let out = lang().arg("run").arg(&bin).output().unwrap();
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "workers: 4\nper-producer order preserved: true\nmessages: 12000\nsome producer moved between workers: true\n"
    );
    // a file that is not a program is not mistaken for one
    let junk = dir.join("junk.langmir");
    std::fs::write(&junk, b"LANGMIR1\x05").unwrap();
    let out = lang().arg("run").arg(&junk).output().unwrap();
    assert!(!out.status.success() && String::from_utf8_lossy(&out.stderr).contains("truncated"));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// `lang run --bench` times the execution after compilation on stderr and
/// leaves the program's output alone.
#[test]
fn run_bench_reports_execution_time() {
    let out = lang().args(["run", "--bench", "examples/01_basics.lang"]).output().unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "The answer is twelve; 5! is 120\n");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.starts_with("bench: run ") && err.contains("ms (compilation "), "{err}");
}
