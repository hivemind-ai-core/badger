//! `badger watch`: re-run a command when a source file changes.
//!
//! The files are polled (their size and modification time), so the
//! command needs no file-watching dependency.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

/// How often the files are looked at.
const INTERVAL: Duration = Duration::from_millis(200);

/// Set when the user interrupts the watch (Ctrl-C).
static INTERRUPTED: AtomicBool = AtomicBool::new(false);

/// The interrupt handler: it only sets a flag (the watch stops at the
/// next poll).
extern "C" fn on_interrupt(_: libc::c_int) {
    INTERRUPTED.store(true, Ordering::Relaxed);
}

/// Ask for the interrupt to end the watch instead of killing the
/// process.
fn catch_interrupts() {
    // SAFETY: the handler only sets an atomic flag
    unsafe {
        libc::signal(libc::SIGINT, on_interrupt as *const () as libc::sighandler_t);
    }
}

/// The state of a watched file: its size and modification time.
type Stamp = (u64, SystemTime);

/// The stamp of a file (`None` when it cannot be read).
fn stamp(file: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(file).ok()?;
    Some((meta.len(), meta.modified().ok()?))
}

/// The watched files and their stamps.
fn snapshot(files: &[PathBuf]) -> BTreeMap<PathBuf, Option<Stamp>> {
    files.iter().map(|f| (f.clone(), stamp(f))).collect()
}

/// The files of the project: every `.bdg` file under it.
fn sources(dir: &Path) -> Vec<PathBuf> {
    let mut files = vec![];
    crate::source_files(dir, &mut files);
    files
}

/// The command to run on a change: the given one, or `badger check` of
/// the project.
struct Watched {
    /// The program to run.
    program: PathBuf,
    /// Its arguments.
    args: Vec<String>,
    /// How the command is shown to the user.
    shown: String,
}

/// The command of the watch: the one the user gave, or the default
/// `badger check PATH`.
fn command(project: &Path, given: &[String]) -> Result<Watched, String> {
    if given.is_empty() {
        let exe = std::env::current_exe().map_err(|e| format!("cannot find the badger executable: {e}"))?;
        let path = project.to_string_lossy().into_owned();
        Ok(Watched {
            shown: format!("badger check {path}"),
            program: exe,
            args: vec!["check".to_string(), path],
        })
    } else {
        let program = PathBuf::from(&given[0]);
        let args = given[1..].to_vec();
        let shown =
            std::iter::once(given[0].clone()).chain(args.iter().cloned()).collect::<Vec<_>>().join(" ");
        Ok(Watched { program, args, shown })
    }
}

/// Watch the project's sources and run the command on every change.
///
/// The command's exit status is reported, but does not end the watch.
pub fn watch(path: &Path, given: Vec<String>) -> ExitCode {
    let project = match std::path::absolute(path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: cannot resolve {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
    };
    if !project.is_dir() {
        eprintln!("error: no such directory: {}", project.display());
        return ExitCode::FAILURE;
    }
    let cmd = match command(&project, &given) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    catch_interrupts();
    let mut files = sources(&project);
    println!("watching {} ({} source file(s))", project.display(), files.len());
    println!("running: {}", cmd.shown);
    let mut last = snapshot(&files);
    let status = run(&cmd, &project);
    println!("exit status: {status}");
    loop {
        if INTERRUPTED.load(Ordering::Relaxed) {
            println!("stopped");
            return ExitCode::SUCCESS;
        }
        std::thread::sleep(INTERVAL);
        if INTERRUPTED.load(Ordering::Relaxed) {
            println!("stopped");
            return ExitCode::SUCCESS;
        }
        // a file that appeared or disappeared is a change too
        files = sources(&project);
        let now = snapshot(&files);
        let mut changed = false;
        for (file, st) in &now {
            match last.get(file) {
                None => {
                    println!("changed: {} (new)", file.display());
                    changed = true;
                }
                Some(prev) if prev != st => {
                    println!("changed: {}", file.display());
                    changed = true;
                }
                _ => {}
            }
        }
        for file in last.keys() {
            if !now.contains_key(file) {
                println!("changed: {} (removed)", file.display());
                changed = true;
            }
        }
        last = now;
        if !changed {
            continue;
        }
        println!("running: {}", cmd.shown);
        let status = run(&cmd, &project);
        println!("exit status: {status}");
    }
}

/// Run the command in the project's directory; the status it ended with.
fn run(cmd: &Watched, project: &Path) -> i32 {
    match std::process::Command::new(&cmd.program).args(&cmd.args).current_dir(project).status() {
        Ok(s) => s.code().unwrap_or(-1),
        Err(e) => {
            eprintln!("error: cannot run {}: {e}", cmd.program.display());
            -1
        }
    }
}
