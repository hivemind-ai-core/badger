//! The HTTP/SQLite service design example, driven by a real HTTP client.

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
    let dir = std::env::temp_dir().join(format!("lang-service-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("orders.db");
    let _ = std::fs::remove_file(&db);
    let port = 18000 + (std::process::id() % 1000) as u16;
    let mut child = Command::new(env!("CARGO_BIN_EXE_lang"))
        .current_dir(&root)
        .args(["run", "examples/design/service"])
        .env("ORDERS_DATABASE_PATH", &db)
        .env("ORDERS_ADDRESS", "127.0.0.1")
        .env("ORDERS_PORT", port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
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

#[test]
fn request_isolation() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let port = 19000 + (std::process::id() % 1000) as u16;
    let mut child = Command::new(env!("CARGO_BIN_EXE_lang"))
        .current_dir(&root)
        .args(["run", "--workers", "4", "tests/http/isolation.lang"])
        .env("ISO_PORT", port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
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
    let child = Command::new(env!("CARGO_BIN_EXE_lang"))
        .current_dir(&root)
        .args(["run", "tests/http/limits.lang"])
        .env("LIM_PORT", port.to_string())
        .env("LANG_SHUTDOWN_AFTER_MS", "4000")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
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
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("stream closed after 3 chunks"), "{stdout}");
    assert!(stdout.contains("stream closed after 2 chunks"), "{stdout}");
    assert!(stdout.contains("limits: body 64, in flight 1, rejected 2"), "{stdout}");
}
