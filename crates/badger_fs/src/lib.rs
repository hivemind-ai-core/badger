//! `badger_fs`: the filesystem operations of `std.fs` beyond the runtime's
//! own text reads and writes: bytes, appends, removal, renames, copies,
//! directories, metadata, recursive listings, and temporary directories.
//! Every call runs on the blocking pool (the calling task parks) and is
//! recorded by its path, so a replay answers it from the bundle.

use std::io::Write;
use std::path::Path;
use std::time::UNIX_EPOCH;

use std::sync::Mutex;

use badger_native::{Args, Error, Handler, Handles, ModuleDef, Outcome, Value};

fn io(e: std::io::Error) -> Error {
    Error::new(badger_native::io_error_kind(e.kind()), e.to_string())
}

fn at(path: &str, e: std::io::Error) -> Error {
    let mut err = io(e);
    err.message = format!("{path}: {}", err.message);
    err
}

fn read_bytes(a: &Args) -> Outcome {
    let p = a.str(0)?;
    Ok(Value::Bytes(std::fs::read(p).map_err(|e| at(p, e))?))
}

fn write_bytes(a: &Args) -> Outcome {
    let p = a.str(0)?;
    std::fs::write(p, a.bytes(1)?).map_err(|e| at(p, e))?;
    Ok(Value::Unit)
}

/// Append to a file, creating it if absent.
fn append(a: &Args) -> Outcome {
    let p = a.str(0)?;
    let data: &[u8] = match a.get(1) {
        Some(Value::Str(s)) => s.as_bytes(),
        _ => a.bytes(1)?,
    };
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(p).map_err(|e| at(p, e))?;
    f.write_all(data).map_err(|e| at(p, e))?;
    Ok(Value::Unit)
}

fn remove_file(a: &Args) -> Outcome {
    let p = a.str(0)?;
    std::fs::remove_file(p).map_err(|e| at(p, e))?;
    Ok(Value::Unit)
}

/// Remove an empty directory.
fn remove_dir(a: &Args) -> Outcome {
    let p = a.str(0)?;
    std::fs::remove_dir(p).map_err(|e| at(p, e))?;
    Ok(Value::Unit)
}

/// Remove a file, or a directory and everything in it (a symlink is
/// removed, not followed).
fn remove_all(a: &Args) -> Outcome {
    let p = a.str(0)?;
    let meta = std::fs::symlink_metadata(p).map_err(|e| at(p, e))?;
    if meta.is_dir() {
        std::fs::remove_dir_all(p).map_err(|e| at(p, e))?;
    } else {
        std::fs::remove_file(p).map_err(|e| at(p, e))?;
    }
    Ok(Value::Unit)
}

fn rename(a: &Args) -> Outcome {
    let (from, to) = (a.str(0)?, a.str(1)?);
    std::fs::rename(from, to).map_err(|e| at(from, e))?;
    Ok(Value::Unit)
}

/// Copy a file; the number of bytes copied.
fn copy(a: &Args) -> Outcome {
    let (from, to) = (a.str(0)?, a.str(1)?);
    Ok(Value::Int(std::fs::copy(from, to).map_err(|e| at(from, e))? as i64))
}

fn create_dir(a: &Args) -> Outcome {
    let p = a.str(0)?;
    std::fs::create_dir(p).map_err(|e| at(p, e))?;
    Ok(Value::Unit)
}

/// Create a directory and any missing parents (fine if it exists).
fn create_dir_all(a: &Args) -> Outcome {
    let p = a.str(0)?;
    std::fs::create_dir_all(p).map_err(|e| at(p, e))?;
    Ok(Value::Unit)
}

fn kind_of(t: std::fs::FileType) -> &'static str {
    if t.is_symlink() {
        "symlink"
    } else if t.is_dir() {
        "dir"
    } else if t.is_file() {
        "file"
    } else {
        "other"
    }
}

/// What is at a path (a symlink itself, not its target).
fn metadata(a: &Args) -> Outcome {
    let p = a.str(0)?;
    let m = std::fs::symlink_metadata(p).map_err(|e| at(p, e))?;
    let modified = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos().min(i64::MAX as u128) as i64);
    Ok(Value::Map(vec![
        ("kind".into(), Value::Symbol(kind_of(m.file_type()).into())),
        ("size".into(), Value::Int(m.len() as i64)),
        ("modified_nanos".into(), Value::Int(modified)),
        ("readonly".into(), Value::Bool(m.permissions().readonly())),
    ]))
}

/// Every file under a directory, as paths relative to it, sorted; symlinks
/// are listed but not followed.
fn walk(a: &Args) -> Outcome {
    let root = a.str(0)?;
    let mut out = vec![];
    let mut stack = vec![std::path::PathBuf::new()];
    while let Some(rel) = stack.pop() {
        let dir = Path::new(root).join(&rel);
        for entry in std::fs::read_dir(&dir).map_err(|e| at(&dir.to_string_lossy(), e))? {
            let entry = entry.map_err(io)?;
            let path = rel.join(entry.file_name());
            if entry.file_type().map_err(io)?.is_dir() {
                stack.push(path);
            } else {
                out.push(path.to_string_lossy().into_owned());
            }
        }
    }
    out.sort();
    Ok(Value::List(out.into_iter().map(Value::Str).collect()))
}

/// The system's directory for temporary files.
fn temp_dir(_: &Args) -> Outcome {
    Ok(Value::Str(std::env::temp_dir().to_string_lossy().into_owned()))
}

/// A new, empty, private directory under the temporary directory.
fn make_temp_dir(a: &Args) -> Outcome {
    let prefix = a.str(0)?;
    // the directory is made directly in the temporary directory
    if prefix.contains(['/', '\\']) || prefix.contains("..") {
        return Err(Error::new(
            "io/invalid_input",
            format!("a temporary directory prefix cannot contain a path: {prefix:?}"),
        ));
    }
    let base = std::env::temp_dir();
    let seed = std::time::SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    for attempt in 0..64u128 {
        let mut x = (seed ^ (std::process::id() as u128) << 64)
            .wrapping_add(attempt.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let mut name = String::from(prefix);
        for _ in 0..10 {
            name.push(char::from(b"abcdefghijklmnopqrstuvwxyz0123456789"[(x % 36) as usize]));
            x /= 36;
        }
        let p = base.join(&name);
        // created private (owner only), with no window where it is not
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        match builder.create(&p) {
            Ok(()) => return Ok(Value::Str(p.to_string_lossy().into_owned())),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(at(&p.to_string_lossy(), e)),
        }
    }
    Err(Error::new("io/already_exists", "no free temporary directory name"))
}

// ---------------------------------------------------------------- handles

/// An open file (dropped, closing it, when its handle is released).
static FILES: Handles<Mutex<Option<std::fs::File>>> = Handles::new();

fn file(a: &Args) -> Result<std::sync::Arc<Mutex<Option<std::fs::File>>>, Error> {
    FILES.get(a.handle(0)?)
}

/// Open `path`: `read`, `write` (create or truncate), `append` (create or
/// add), or `create` (a new file; fails if it exists).
fn open(a: &Args) -> Outcome {
    let (p, mode) = (a.str(0)?, a.str(1)?);
    let mut o = std::fs::OpenOptions::new();
    match mode {
        "read" => o.read(true),
        "write" => o.write(true).create(true).truncate(true),
        "append" => o.append(true).create(true),
        "create" => o.write(true).create_new(true),
        other => return Err(Error::new("io/invalid_input", format!("unknown mode `{other}`"))),
    };
    let f = o.open(p).map_err(|e| at(p, e))?;
    Ok(FILES.insert(Mutex::new(Some(f))))
}

fn closed() -> Error {
    Error::new("io/closed", "the file is closed")
}

/// Up to `max` bytes; `none` at the end of the file.
fn read(a: &Args) -> Outcome {
    use std::io::Read;
    let f = file(a)?;
    let max = a.int(1)?.clamp(1, 16 << 20) as usize;
    let mut g = f.lock().unwrap();
    let file = g.as_mut().ok_or_else(closed)?;
    let mut buf = vec![0u8; max];
    let n = file.read(&mut buf).map_err(io)?;
    if n == 0 {
        return Ok(Value::Null);
    }
    buf.truncate(n);
    Ok(Value::Bytes(buf))
}

fn write(a: &Args) -> Outcome {
    let f = file(a)?;
    let data = a.bytes(1)?;
    let mut g = f.lock().unwrap();
    g.as_mut().ok_or_else(closed)?.write_all(data).map_err(io)?;
    Ok(Value::Unit)
}

fn flush(a: &Args) -> Outcome {
    let f = file(a)?;
    let mut g = f.lock().unwrap();
    g.as_mut().ok_or_else(closed)?.flush().map_err(io)?;
    Ok(Value::Unit)
}

/// Close (later reads and writes fail with `:io/closed`).
fn close_file(a: &Args) -> Outcome {
    let f = file(a)?;
    let taken = f.lock().unwrap().take();
    if let Some(mut file) = taken {
        file.flush().map_err(io)?;
    }
    Ok(Value::Unit)
}

/// Up to `max` bytes of standard input; `none` at its end.
fn stdin_read(a: &Args) -> Outcome {
    use std::io::{BufRead, Read};
    let max = a.int(0)?.clamp(1, 16 << 20) as usize;
    let mut lock = std::io::stdin().lock();
    // what is buffered first, so line and byte reads interleave in order
    let available = lock.fill_buf().map_err(io)?;
    if available.is_empty() {
        return Ok(Value::Null);
    }
    let n = available.len().min(max);
    let mut buf = vec![0u8; n];
    lock.read_exact(&mut buf).map_err(io)?;
    Ok(Value::Bytes(buf))
}

/// The next line of standard input without its line break; `none` at its end.
fn stdin_line(_: &Args) -> Outcome {
    use std::io::BufRead;
    let mut line = String::new();
    let n = std::io::stdin().lock().read_line(&mut line).map_err(io)?;
    if n == 0 {
        return Ok(Value::Null);
    }
    if line.ends_with('\n') {
        line.pop();
        if line.ends_with('\r') {
            line.pop();
        }
    }
    Ok(Value::Str(line))
}

fn drop_handle(h: u64) {
    FILES.remove(h);
}

pub static DEF: ModuleDef = ModuleDef {
    name: "badger_fs",
    functions: &[
        ("read_bytes", Handler::Blocking(read_bytes)),
        ("write_bytes", Handler::Blocking(write_bytes)),
        ("append", Handler::Blocking(append)),
        ("remove_file", Handler::Blocking(remove_file)),
        ("remove_dir", Handler::Blocking(remove_dir)),
        ("remove_all", Handler::Blocking(remove_all)),
        ("rename", Handler::Blocking(rename)),
        ("copy", Handler::Blocking(copy)),
        ("create_dir", Handler::Blocking(create_dir)),
        ("create_dir_all", Handler::Blocking(create_dir_all)),
        ("metadata", Handler::Blocking(metadata)),
        ("walk", Handler::Blocking(walk)),
        ("temp_dir", Handler::Inline(temp_dir)),
        ("make_temp_dir", Handler::Blocking(make_temp_dir)),
        ("open", Handler::Blocking(open)),
        ("read", Handler::Blocking(read)),
        ("write", Handler::Blocking(write)),
        ("flush", Handler::Blocking(flush)),
        ("close", Handler::Blocking(close_file)),
        ("stdin_read", Handler::Blocking(stdin_read)),
        ("stdin_line", Handler::Blocking(stdin_line)),
    ],
    drop_handle: Some(drop_handle),
    cancel: None,
    recorded: &[
        ("read_bytes", "read", 1),
        ("write_bytes", "write", 1),
        ("append", "append", 1),
        ("remove_file", "remove", 1),
        ("remove_dir", "remove", 1),
        ("remove_all", "remove", 1),
        ("rename", "rename", 2),
        ("copy", "copy", 2),
        ("create_dir", "mkdir", 1),
        ("create_dir_all", "mkdir", 1),
        ("metadata", "metadata", 1),
        ("walk", "list", 1),
        ("temp_dir", "temp", 0),
        ("make_temp_dir", "temp", 1),
        ("open", "open", 2),
        ("read", "file read", 1),
        ("write", "file write", 1),
        ("flush", "file flush", 1),
        ("close", "file close", 1),
        ("stdin_read", "stdin", 0),
        ("stdin_line", "stdin", 0),
    ],
    pure: &[],
};

badger_native::export!("badger_fs", DEF);
