//! The HTTP/SQLite service design example, driven by a real HTTP client.

mod common;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn request(port: u16, method: &str, path: &str, body: &str) -> (u16, String) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let req = format!("{method} {path} HTTP/1.1\r\nhost: x\r\ncontent-length: {}\r\n\r\n{body}", body.len());
    s.write_all(req.as_bytes()).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    let status = out[9..12].parse().unwrap();
    let body = out.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    (status, body)
}

#[test]
fn orders_service() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join(format!("badger-service-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("orders.db");
    let _ = std::fs::remove_file(&db);
    let port = 18000 + (std::process::id() % 1000) as u16;
    let mut child = common::Reaped::new(
        Command::new(env!("CARGO_BIN_EXE_badger"))
            .current_dir(&root)
            .args(["run", "examples/design/service"])
            .env("ORDERS_DATABASE_PATH", &db)
            .env("ORDERS_ADDRESS", "127.0.0.1")
            .env("ORDERS_PORT", port.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let start = Instant::now();
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(start.elapsed() < Duration::from_secs(20), "service did not start");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(
        request(port, "POST", "/orders", r#"{"title":"Prepare shipment"}"#),
        (201, r#"{"id":1,"title":"Prepare shipment"}"#.into())
    );
    assert_eq!(
        request(port, "POST", "/orders", r#"{"title":"Second"}"#),
        (201, r#"{"id":2,"title":"Second"}"#.into())
    );
    assert_eq!(
        request(port, "POST", "/orders", r#"{"title":"#),
        (400, r#"{"error":"invalid JSON request"}"#.into())
    );
    assert_eq!(
        request(port, "POST", "/orders", r#"{"title":""}"#),
        (400, r#"{"error":"title must not be empty"}"#.into())
    );
    assert_eq!(
        request(port, "POST", "/orders", r#"{"name":"x"}"#),
        (400, r#"{"error":"invalid JSON request"}"#.into())
    );
    assert_eq!(request(port, "GET", "/missing", ""), (404, r#"{"error":"not found"}"#.into()));
    unsafe { libc_kill(child.id() as i32, 15) };
    let status = child.wait().unwrap();
    assert!(status.success(), "service exit: {status:?}");
}

/// Concurrent clients against the service on four workers, under chaos
/// scheduling and heap verification: every order is created exactly once
/// and receives a distinct id.
#[test]
fn orders_service_concurrent() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join(format!("badger-service-conc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("orders.db");
    let _ = std::fs::remove_file(&db);
    let port = 21000 + (std::process::id() % 1000) as u16;
    let child = common::Reaped::new(
        Command::new(env!("CARGO_BIN_EXE_badger"))
            .current_dir(&root)
            .args(["run", "--workers", "4", "examples/design/service"])
            .env("ORDERS_DATABASE_PATH", &db)
            .env("ORDERS_ADDRESS", "127.0.0.1")
            .env("ORDERS_PORT", port.to_string())
            .env("BADGER_CHAOS", "11")
            .env("BADGER_GC_VERIFY", "1")
            .env("BADGER_TRACE", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let start = Instant::now();
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(start.elapsed() < Duration::from_secs(30), "service did not start");
        std::thread::sleep(Duration::from_millis(50));
    }
    let clients: Vec<_> = (0..8)
        .map(|c| {
            std::thread::spawn(move || {
                (0..25)
                    .map(|i| request(port, "POST", "/orders", &format!(r#"{{"title":"c{c} n{i}"}}"#)))
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let mut ids = vec![];
    let mut failed = vec![];
    for c in clients {
        for (status, body) in c.join().unwrap() {
            if status != 201 {
                failed.push(body.clone());
                continue;
            }
            let id: u64 =
                body.split("\"id\":").nth(1).and_then(|r| r.split(',').next()).unwrap().parse().unwrap();
            ids.push(id);
        }
    }
    if !failed.is_empty() {
        unsafe { libc_kill(child.id() as i32, 15) };
        let out = child.output();
        let err = String::from_utf8_lossy(&out.stderr);
        let lines: Vec<&str> = err
            .lines()
            .filter(|l| {
                !l.contains("orders/completed")
                    || l.split("\"elapsed\":")
                        .nth(1)
                        .and_then(|x| x.trim_end_matches('}').parse::<u64>().ok())
                        .is_some_and(|ns| ns > 500_000_000)
            })
            .collect();
        panic!("{failed:?}\n{}", lines.join("\n"));
    }
    ids.sort_unstable();
    assert_eq!(ids, (1..=200).collect::<Vec<u64>>(), "ids lost or duplicated");
    unsafe { libc_kill(child.id() as i32, 15) };
    let out = child.output();
    assert!(out.status.success(), "service exit: {:?}\n{}", out.status, String::from_utf8_lossy(&out.stderr));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn request_isolation() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let port = 19000 + (std::process::id() % 1000) as u16;
    let mut child = common::Reaped::new(
        Command::new(env!("CARGO_BIN_EXE_badger"))
            .current_dir(&root)
            .args(["run", "--workers", "4", "tests/http/isolation.bdg"])
            .env("ISO_PORT", port.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let start = Instant::now();
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(start.elapsed() < Duration::from_secs(20), "server did not start");
        std::thread::sleep(Duration::from_millis(50));
    }
    // B waits on the database while A fails
    let slow = std::thread::spawn(move || request(port, "GET", "/slow", ""));
    std::thread::sleep(Duration::from_millis(50));
    let failed = request(port, "GET", "/fail", "");
    assert_eq!(failed.0, 500);
    let slow = slow.join().unwrap();
    // A's transaction was rolled back: its insert is not visible
    assert_eq!(slow, (200, r#"{"hits":{"n":0}}"#.into()));
    // the server keeps serving after a failed request
    assert_eq!(request(port, "GET", "/nothing", "").0, 404);
    assert_eq!(request(port, "GET", "/slow", "").0, 200);
    unsafe { libc_kill(child.id() as i32, 15) };
    let status = child.wait().unwrap();
    assert!(status.success(), "server exit: {status:?}");
}

unsafe extern "C" {
    #[link_name = "kill"]
    fn libc_kill(pid: i32, sig: i32) -> i32;
}

#[test]
fn http_limits_and_streaming() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let port = 20000 + (std::process::id() % 1000) as u16 * 3;
    let child = common::Reaped::new(
        Command::new(env!("CARGO_BIN_EXE_badger"))
            .current_dir(&root)
            .args(["run", "tests/http/limits.bdg"])
            .env("LIM_PORT", port.to_string())
            .env("BADGER_SHUTDOWN_AFTER_MS", "4000")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let start = Instant::now();
    while TcpStream::connect(("127.0.0.1", port + 1)).is_err() {
        assert!(start.elapsed() < Duration::from_secs(20), "server did not start");
        std::thread::sleep(Duration::from_millis(50));
    }
    // an oversized body is refused before the handler runs
    let big = "x".repeat(100);
    assert_eq!(request(port, "POST", "/echo", &big).0, 413);
    assert_eq!(request(port, "POST", "/echo", "small"), (200, r#"{"path":"/echo","size":5}"#.into()));
    // bounded admission: one request in flight, the next is answered 503
    let slow = std::thread::spawn(move || request(port, "GET", "/slow", ""));
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(request(port, "GET", "/other", "").0, 503);
    assert_eq!(slow.join().unwrap().0, 200);
    // a streamed reply arrives chunked and its source is closed
    let mut s = TcpStream::connect(("127.0.0.1", port + 1)).unwrap();
    s.write_all(b"GET /s HTTP/1.1\r\nhost: x\r\n\r\n").unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    assert!(out.contains("transfer-encoding: chunked"), "{out}");
    assert!(out.contains("chunk 0\n") && out.contains("chunk 2\n") && out.ends_with("0\r\n\r\n"), "{out}");
    // a request-scoped handler streams inside its scope
    let mut s = TcpStream::connect(("127.0.0.1", port + 2)).unwrap();
    s.write_all(b"GET /s HTTP/1.1\r\nhost: x\r\n\r\n").unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    assert!(out.contains("chunk 0\n") && out.contains("chunk 1\n") && out.ends_with("0\r\n\r\n"), "{out}");
    let output = child.output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("stream closed after 3 chunks"), "{stdout}");
    assert!(stdout.contains("stream closed after 2 chunks"), "{stdout}");
    assert!(stdout.contains("limits: body 64, in flight 1, rejected 2"), "{stdout}");
}

/// A streamed reply waits for a client that stops reading instead of
/// buffering all of it (`http_max_stream_buffer_bytes`).
#[test]
fn stream_backpressure() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let port = 20000 + (std::process::id() % 1000) as u16 * 2;
    let child = common::Reaped::new(
        Command::new(env!("CARGO_BIN_EXE_badger"))
            .current_dir(&root)
            .args(["run", "tests/http/backpressure.bdg"])
            .env("BP_PORT", port.to_string())
            .env("BADGER_HTTP_MAX_STREAM_BUFFER_BYTES", "65536")
            .env("BADGER_SHUTDOWN_AFTER_MS", "8000")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let start = Instant::now();
    while TcpStream::connect(("127.0.0.1", port + 1)).is_err() {
        assert!(start.elapsed() < Duration::from_secs(20), "server did not start");
        std::thread::sleep(Duration::from_millis(50));
    }
    let produced = || -> u64 {
        let body = request(port + 1, "GET", "/count", "").1;
        body.trim_start_matches("{\"produced\":").trim_end_matches('}').parse().unwrap()
    };
    // connect with a small receive buffer and stop reading
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.write_all(b"GET /s HTTP/1.1\r\nhost: x\r\n\r\n").unwrap();
    std::thread::sleep(Duration::from_millis(1500));
    let stalled = produced();
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(produced(), stalled, "the producer kept going while the client was not reading");
    assert!(stalled < 1280, "all 1280 chunks were produced without the client reading ({stalled})");
    // reading resumes: everything arrives
    let mut total = 0usize;
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = s.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        total += n;
    }
    assert!(total > 1280 * 16384, "got {total} bytes");
    assert_eq!(produced(), 1280);
    drop(child.output());
}

/// Server-sent events to a client that disconnects: the request task is
/// cancelled while its producer is parked, so the event source is closed.
#[test]
fn events_stop_when_the_client_leaves() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let port = 24000 + (std::process::id() % 1000) as u16 * 3;
    let child = common::Reaped::new(
        Command::new(env!("CARGO_BIN_EXE_badger"))
            .current_dir(&root)
            .args(["run", "tests/http/events.bdg"])
            .env("SSE_PORT", port.to_string())
            .env("BADGER_SHUTDOWN_AFTER_MS", "6000")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let start = Instant::now();
    while TcpStream::connect(("127.0.0.1", port + 2)).is_err() {
        assert!(start.elapsed() < Duration::from_secs(20), "server did not start");
        std::thread::sleep(Duration::from_millis(50));
    }
    let closed = || -> u64 {
        let body = request(port + 2, "GET", "/count", "").1;
        body.trim_start_matches("{\"closed\":").trim_end_matches('}').parse().unwrap()
    };
    for (left, p) in [port, port + 1].into_iter().enumerate() {
        let mut s = TcpStream::connect(("127.0.0.1", p)).unwrap();
        s.write_all(b"GET /events HTTP/1.1\r\nhost: x\r\n\r\n").unwrap();
        let mut got = Vec::new();
        let mut buf = [0u8; 1024];
        while !String::from_utf8_lossy(&got).contains("data: tick 1\n\n") {
            let n = s.read(&mut buf).unwrap();
            assert!(n > 0, "stream ended early: {}", String::from_utf8_lossy(&got));
            got.extend_from_slice(&buf[..n]);
        }
        let head = String::from_utf8_lossy(&got).to_string();
        assert!(head.contains("content-type: text/event-stream"), "{head}");
        assert!(head.contains("event: tick\nid: 0\ndata: tick 0\n\n"), "{head}");
        assert_eq!(closed(), left as u64, "closed before the client left");
        drop(s);
        let start = Instant::now();
        while closed() == left as u64 {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "the event source was not closed after the client left"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    let output = child.output();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}

/// WebSockets: an echo server and clients in one program (text, binary, a
/// reader task beside a writer, a server-side close, and a plain request
/// answered 426).
#[test]
fn websockets() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let port = 27000 + (std::process::id() % 1000) as u16 * 2;
    let out = Command::new(env!("CARGO_BIN_EXE_badger"))
        .current_dir(&root)
        .args(["run", "tests/http/websocket.bdg"])
        .env("WS_PORT", port.to_string())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let round = "echo: hello\n5 bytes\n[\"echo: m0\", \"echo: m1\", \"echo: m2\"]\nclosed\n";
    assert_eq!(
        stdout,
        format!("{round}{round}plain request: 426\n"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success());
}

/// The framework example as a service (framework F11): it becomes ready
/// only after startup; concurrent writes through its admission gate land
/// in SQLite exactly once while malformed and oversized requests fail on
/// their own and a slow client holds a connection; the running system can
/// be inspected; SIGINT drains and stops it cleanly.
#[test]
fn framework_service() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let app = root.join("examples/framework");
    let dir = std::env::temp_dir().join(format!("badger-framework-service-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("tasks.db");
    let _ = std::fs::remove_file(&db);
    let migrate = Command::new(env!("CARGO_BIN_EXE_badger"))
        .current_dir(&app)
        .args(["run", "migrate"])
        .env("TASKS_DATABASE", &db)
        .output()
        .unwrap();
    assert!(migrate.status.success(), "{}", String::from_utf8_lossy(&migrate.stderr));
    let port = 24000 + (std::process::id() % 1000) as u16;
    let inspect = format!("127.0.0.1:{}", port + 1);
    let child = common::Reaped::new(
        Command::new(env!("CARGO_BIN_EXE_badger"))
            .current_dir(&app)
            .args(["run", "--workers", "4", "--inspect", &inspect, "--", &format!("port={port}")])
            .env("TASKS_DATABASE", &db)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let start = Instant::now();
    loop {
        if TcpStream::connect(("127.0.0.1", port)).is_ok()
            && request(port, "GET", "/health/ready", "").0 == 200
        {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(30), "service did not become ready");
        std::thread::sleep(Duration::from_millis(50));
    }
    // a slow client: half a request, held open while others are served
    let mut slow = TcpStream::connect(("127.0.0.1", port)).unwrap();
    slow.write_all(b"POST /tasks HTTP/1.1\r\nhost: x\r\ncontent-length: 40\r\n\r\n{\"title\"").unwrap();
    let clients: Vec<_> = (0..8)
        .map(|c| {
            std::thread::spawn(move || {
                (0..25)
                    .map(|i| {
                        let body = match i % 10 {
                            3 => "{\"title\": ".to_string(),
                            7 => format!(r#"{{"title":"{}"}}"#, "x".repeat(100)),
                            _ => format!(r#"{{"title":"c{c} n{i}"}}"#),
                        };
                        request(port, "POST", "/tasks", &body).0
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let mut statuses: Vec<u16> = vec![];
    for c in clients {
        statuses.extend(c.join().unwrap());
    }
    let count = |s: u16| statuses.iter().filter(|x| **x == s).count();
    assert_eq!((count(201), count(400)), (160, 40), "{statuses:?}");
    let (status, body) = request(port, "GET", "/tasks", "");
    assert_eq!(status, 200);
    assert_eq!(body.matches("\"id\":").count(), 160, "each write landed exactly once");
    // the running system, inspected from outside
    let sys = Command::new(env!("CARGO_BIN_EXE_badger"))
        .args(["inspect", "system", "--connect", &inspect])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&sys.stdout).to_string();
    assert!(sys.status.success() && text.contains("server") && text.contains("store"), "{text}");
    drop(slow);
    unsafe { libc_kill(child.id() as i32, 2) };
    let stopped = Instant::now();
    let out = child.output();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "service exit: {:?}\n{err}", out.status);
    assert!(stopped.elapsed() < Duration::from_secs(10), "shutdown took {:?}", stopped.elapsed());
    assert!(!err.contains("panic") && !err.contains("leak"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}
