//! `stdx.valkey` against a live server: `BADGER_TEST_VALKEY_URL` (a server
//! whose database 0 the test may flush), else a `valkey-server` or
//! `redis-server` on the path started for the test; skipped without one.

mod common;

use std::net::TcpStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn server() -> Option<(String, Option<common::Reaped>)> {
    if let Ok(url) = std::env::var("BADGER_TEST_VALKEY_URL") {
        return Some((url, None));
    }
    let port = 26000 + (std::process::id() % 1000) as u16;
    for exe in ["valkey-server", "redis-server"] {
        let Ok(child) = Command::new(exe)
            .args(["--port", &port.to_string(), "--bind", "127.0.0.1", "--save", "", "--appendonly", "no"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            continue;
        };
        let child = common::Reaped::new(child);
        let start = Instant::now();
        while TcpStream::connect(("127.0.0.1", port)).is_err() {
            assert!(start.elapsed() < Duration::from_secs(10), "{exe} did not start");
            std::thread::sleep(Duration::from_millis(20));
        }
        return Some((format!("valkey://127.0.0.1:{port}"), Some(child)));
    }
    None
}

#[test]
fn valkey_client() {
    let Some((url, _server)) = server() else {
        eprintln!("skipped: set BADGER_TEST_VALKEY_URL or install valkey-server");
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let want = std::fs::read_to_string(root.join("tests/valkey/client.out")).unwrap();
    let bundle = std::env::temp_dir().join(format!("badger-valkey-{}.json", std::process::id()));
    let out = Command::new(env!("CARGO_BIN_EXE_badger"))
        .current_dir(&root)
        .args(["run", "--record"])
        .arg(&bundle)
        .arg("tests/valkey/client.bdg")
        .env("VALKEY_URL", &url)
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), want, "{}", String::from_utf8_lossy(&out.stderr));
    // the recording replays without the server
    let rep = Command::new(env!("CARGO_BIN_EXE_badger"))
        .current_dir(&root)
        .arg("replay")
        .arg(&bundle)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&bundle);
    assert_eq!(String::from_utf8_lossy(&rep.stdout), want, "{}", String::from_utf8_lossy(&rep.stderr));
    assert!(rep.status.success(), "{}", String::from_utf8_lossy(&rep.stderr));
}
