//! Release directories (`lang release`): a self-contained deployment of a
//! program, started with `lang start DIR` (or the `run` launcher in it)
//! without the sources, a compiler installation, or network access.
//!
//! ```text
//! DIR/release.json       what is here, and the digests `start` checks
//! DIR/run                launcher: `bin/lang start DIR -- ARGS`
//! DIR/bin/lang           the executable that built the release
//! DIR/app/NAME.langmir   the program (binary MIR, settings included)
//! DIR/app/lang.lock      the project's lockfile, when it has one
//! DIR/lib/lang/libM.so   native modules not linked into `lang`
//! ```
//!
//! Starting checks the target, the program's digest, and each native
//! library's digest and ABI version before any program code runs, so a
//! replaced or incompatible library is refused rather than loaded at its
//! first call.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{RunOptions, Session};

/// The `release.json` layout version.
pub const FORMAT: u64 = 1;

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn file_digest(path: &Path) -> Result<String, String> {
    std::fs::read(path).map(|b| digest(&b)).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

/// What `build` wrote.
pub struct Built {
    pub name: String,
    pub program_bytes: usize,
    /// Native modules copied into `lib/lang`.
    pub libraries: Vec<String>,
}

/// The release's name: the package name, else the entry file's stem.
fn release_name(path: &Path) -> String {
    if path.is_dir()
        && let Ok(m) = crate::manifest::Manifest::load(&path.join("lang.toml"))
    {
        return m.name;
    }
    path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "app".to_string())
}

/// Compile the program at `path` and write a release directory at `out`.
/// `Err(None)`: the program has errors (in the session's diagnostics).
pub fn build(
    session: &mut Session,
    path: &Path,
    out: &Path,
    opts: &RunOptions,
) -> Result<Built, Option<String>> {
    if out.exists() {
        let empty = std::fs::read_dir(out).map(|mut d| d.next().is_none()).unwrap_or(false);
        if !empty && !out.join("release.json").is_file() {
            return Err(Some(format!("{} exists and is not a release directory", out.display())));
        }
    }
    let bytes = session.compile_binary(path, opts).map_err(|_| None)?;
    let name = release_name(path);
    // written next to its destination, then moved into place: a failure
    // leaves any earlier release as it was
    let mut tmp_name = out.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(format!(".partial-{}", std::process::id()));
    let staging = out.with_file_name(tmp_name);
    let r = write(session, path, &staging, &name, &bytes);
    if r.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
        return r.map_err(Some);
    }
    if out.exists() {
        std::fs::remove_dir_all(out).map_err(|e| Some(format!("cannot replace {}: {e}", out.display())))?;
    }
    std::fs::rename(&staging, out)
        .map_err(|e| Some(format!("cannot move the release to {}: {e}", out.display())))?;
    r.map_err(Some)
}

fn write(session: &Session, path: &Path, out: &Path, name: &str, bytes: &[u8]) -> Result<Built, String> {
    let io = |e: std::io::Error, what: &Path| format!("cannot write {}: {e}", what.display());
    for d in ["bin", "app", "lib/lang"] {
        let d = out.join(d);
        std::fs::create_dir_all(&d).map_err(|e| io(e, &d))?;
    }
    let program = out.join("app").join(format!("{name}.langmir"));
    std::fs::write(&program, bytes).map_err(|e| io(e, &program))?;
    let exe = std::env::current_exe().map_err(|e| format!("cannot find the running executable: {e}"))?;
    let bin = out.join("bin/lang");
    std::fs::copy(&exe, &bin).map_err(|e| io(e, &bin))?;
    let mut natives = vec![];
    let mut libraries = vec![];
    for m in &session.natives {
        match lang_runtime::native::probe(m) {
            Ok(None) => natives.push(json!({ "name": m, "linked": true })),
            Ok(Some(file)) => {
                let fname = lang_runtime::native::file_name(m);
                let dest = out.join("lib/lang").join(&fname);
                std::fs::copy(&file, &dest).map_err(|e| io(e, &dest))?;
                natives.push(
                    json!({ "name": m, "file": format!("lib/lang/{fname}"), "sha256": file_digest(&dest)? }),
                );
                libraries.push(m.clone());
            }
            Err(e) => return Err(e),
        }
    }
    let mut lock = Value::Null;
    if path.is_dir() && path.join("lang.lock").is_file() {
        let dest = out.join("app/lang.lock");
        std::fs::copy(path.join("lang.lock"), &dest).map_err(|e| io(e, &dest))?;
        lock = json!({ "file": "app/lang.lock", "sha256": file_digest(&dest)? });
    }
    let manifest = json!({
        "format": FORMAT,
        "name": name,
        "lang": env!("CARGO_PKG_VERSION"),
        "native_abi": lang_runtime::native::ABI_VERSION,
        "target": { "arch": std::env::consts::ARCH, "os": std::env::consts::OS },
        "program": { "file": format!("app/{name}.langmir"), "sha256": digest(bytes), "bytes": bytes.len() },
        "executable": { "file": "bin/lang", "sha256": file_digest(&bin)? },
        "natives": natives,
        "lock": lock,
    });
    let mpath = out.join("release.json");
    std::fs::write(&mpath, serde_json::to_string_pretty(&manifest).unwrap() + "\n")
        .map_err(|e| io(e, &mpath))?;
    let run = out.join("run");
    std::fs::write(
        &run,
        "#!/bin/sh\n# Start this release (arguments go to the program).\nhere=$(cd \"$(dirname \"$0\")\" && pwd)\nexec \"$here/bin/lang\" start \"$here\" -- \"$@\"\n",
    )
    .map_err(|e| io(e, &run))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&run, std::fs::Permissions::from_mode(0o755)).map_err(|e| io(e, &run))?;
    }
    Ok(Built { name: name.to_string(), program_bytes: bytes.len(), libraries })
}

/// Check the release at `dir` before starting it: the program file to run.
pub fn verify(dir: &Path) -> Result<PathBuf, String> {
    let mpath = dir.join("release.json");
    let text =
        std::fs::read_to_string(&mpath).map_err(|e| format!("{} is not a release: {e}", dir.display()))?;
    let m: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", mpath.display()))?;
    if m["format"].as_u64() != Some(FORMAT) {
        return Err(format!("{}: release format {} is not {FORMAT}", mpath.display(), m["format"]));
    }
    let (arch, os) = (m["target"]["arch"].as_str().unwrap_or(""), m["target"]["os"].as_str().unwrap_or(""));
    if arch != std::env::consts::ARCH || os != std::env::consts::OS {
        return Err(format!(
            "the release is built for {arch}-{os}, not {}-{}",
            std::env::consts::ARCH,
            std::env::consts::OS
        ));
    }
    if m["lang"].as_str() != Some(env!("CARGO_PKG_VERSION")) {
        return Err(format!(
            "the release was built by lang {}, and this is lang {} (start it with its own `bin/lang`)",
            m["lang"],
            env!("CARGO_PKG_VERSION")
        ));
    }
    let checked = |rel: &Value, what: &str| -> Result<PathBuf, String> {
        let file =
            dir.join(rel["file"].as_str().ok_or_else(|| format!("release.json lacks the {what} file"))?);
        let have = file_digest(&file)?;
        if Some(have.as_str()) != rel["sha256"].as_str() {
            return Err(format!("{} has changed since the release was built", file.display()));
        }
        Ok(file)
    };
    let program = checked(&m["program"], "program")?;
    if !m["lock"].is_null() {
        checked(&m["lock"], "lockfile")?;
    }
    for n in m["natives"].as_array().map(|v| v.as_slice()).unwrap_or(&[]) {
        let name = n["name"].as_str().unwrap_or_default();
        if n["linked"].as_bool() == Some(true) {
            match lang_runtime::native::probe(name) {
                Ok(None) => continue,
                _ => return Err(format!("native module `{name}` is not linked into this executable")),
            }
        }
        let file = checked(n, &format!("`{name}` library"))?;
        // the release's own copy is the one loaded, at its ABI version
        match lang_runtime::native::probe(name)? {
            Some(found) if same_file(&found, &file) => {}
            Some(found) => {
                return Err(format!(
                    "native module `{name}` would load from {} instead of the release's {} (check LANG_NATIVE_PATH)",
                    found.display(),
                    file.display()
                ));
            }
            None => return Err(format!("native module `{name}` is linked into this executable")),
        }
    }
    Ok(program)
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}
