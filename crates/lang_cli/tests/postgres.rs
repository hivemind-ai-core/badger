//! `std.postgres` against a live server: `LANG_TEST_POSTGRES_URL` (a
//! database the test may change), else a throwaway cluster made with the
//! `initdb` and `pg_ctl` on the path; skipped without either.

use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// A cluster stopped (and removed) when the test ends.
struct Cluster(PathBuf);

impl Drop for Cluster {
    fn drop(&mut self) {
        let _ = Command::new("pg_ctl")
            .arg("-D")
            .arg(self.0.join("data"))
            .args(["-m", "immediate", "stop"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn server() -> Option<(String, Option<Cluster>)> {
    if let Ok(url) = std::env::var("LANG_TEST_POSTGRES_URL") {
        return Some((url, None));
    }
    let dir = std::env::temp_dir().join(format!("lang-pg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let init = Command::new("initdb")
        .arg("-D")
        .arg(dir.join("data"))
        .args(["-U", "lang", "--auth=trust", "-E", "UTF8"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if !init.is_ok_and(|s| s.success()) {
        let _ = std::fs::remove_dir_all(&dir);
        return None;
    }
    let cluster = Cluster(dir.clone());
    let port = 25000 + (std::process::id() % 1000) as u16;
    let started = Command::new("pg_ctl")
        .arg("-D")
        .arg(dir.join("data"))
        .arg("-o")
        .arg(format!("-p {port} -c unix_socket_directories= -c listen_addresses=127.0.0.1 -c fsync=off"))
        .arg("-l")
        .arg(dir.join("log"))
        .args(["-w", "start"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(started.success(), "{}", std::fs::read_to_string(dir.join("log")).unwrap_or_default());
    let start = Instant::now();
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(start.elapsed() < Duration::from_secs(20), "postgres did not start");
        std::thread::sleep(Duration::from_millis(20));
    }
    Some((format!("postgres://lang@127.0.0.1:{port}/postgres?sslmode=disable"), Some(cluster)))
}

#[test]
fn postgres_client() {
    let Some((url, _cluster)) = server() else {
        eprintln!("skipped: set LANG_TEST_POSTGRES_URL or install postgres (initdb, pg_ctl)");
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let want = std::fs::read_to_string(root.join("tests/postgres/client.out")).unwrap();
    let bundle = std::env::temp_dir().join(format!("lang-pg-{}.json", std::process::id()));
    let out = Command::new(env!("CARGO_BIN_EXE_lang"))
        .current_dir(&root)
        .args(["run", "--record"])
        .arg(&bundle)
        .arg("tests/postgres/client.lang")
        .env("POSTGRES_URL", &url)
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), want, "{}", String::from_utf8_lossy(&out.stderr));
    // the recording replays without the server
    drop(_cluster);
    let rep = Command::new(env!("CARGO_BIN_EXE_lang"))
        .current_dir(&root)
        .arg("replay")
        .arg(&bundle)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&bundle);
    assert_eq!(String::from_utf8_lossy(&rep.stdout), want, "{}", String::from_utf8_lossy(&rep.stderr));
    assert!(rep.status.success(), "{}", String::from_utf8_lossy(&rep.stderr));
}
