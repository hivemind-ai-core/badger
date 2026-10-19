//! `badger run --watch`: recompile when sources change and replace compatible
//! functions in the running program (dispatch slots in `badger_runtime::live`).
//!
//! A new version is accepted when it checks, its user type definitions are
//! unchanged, and every function present in both versions keeps its type.
//! Accepted: new calls run new code; frames already executing and closures
//! already created keep their version; module-level values keep their state.
//! An incompatible version (changed type definition or function type) needs
//! controlled reconstruction: with `--reconstruct`, the running program is
//! cancelled (tasks settle, resources halt in reverse order) and restarted on
//! the new version, whose values are built fresh; without it the change is
//! rejected. A version that fails to check is always rejected; the running
//! version continues.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use crate::{Built, RunOptions, Session};

/// Text of every user struct and enum definition (layout-relevant shape).
pub fn type_fingerprint(g: &badger_typeck::Global<'_>) -> String {
    let mut out = vec![];
    for (id, def) in g.hir.defs.iter() {
        let m = &g.hir.modules[def.module];
        if m.is_std {
            continue;
        }
        let path: Vec<&str> = m.path.iter().map(|s| g.interner.resolve(*s)).collect();
        let name = format!("{}.{}", path.join("."), g.interner.resolve(def.name));
        if let Some(s) = g.env.structs.get(&id) {
            let fields: Vec<String> = s
                .fields
                .iter()
                .map(|(l, t, _)| format!("{}: {}", g.interner.resolve(*l), badger_typeck::show_ty(g, t)))
                .collect();
            out.push(format!("struct {name} {{ {} }}", fields.join(", ")));
        }
        if let Some(e) = g.env.enums.get(&id) {
            let vs: Vec<String> = e
                .variants
                .iter()
                .map(|v| {
                    let fs: Vec<String> =
                        v.fields.iter().map(|(_, t)| badger_typeck::show_ty(g, t)).collect();
                    format!("{}({})", g.interner.resolve(v.name), fs.join(", "))
                })
                .collect();
            out.push(format!("enum {name} {{ {} }}", vs.join(", ")));
        }
    }
    out.sort();
    out.join("\n")
}

pub(crate) fn entries(b: &Built) -> Vec<badger_runtime::live::Entry> {
    b.prog
        .funcs
        .iter_enumerated()
        .filter_map(|(id, f)| {
            f.live_key.as_ref().map(|key| badger_runtime::live::Entry {
                key: key.clone(),
                sig: f.live_sig.clone(),
                code: b.compiled.func_ptr(id) as u64,
                keep_existing: f.is_global,
            })
        })
        .collect()
}

/// Make the initial version current.
pub fn publish_version(b: &Built) {
    badger_runtime::live::publish(&b.types, &entries(b));
}

/// Poll the sources; on a change, recompile on the scheduler thread (between
/// task steps, so no task observes a half-replaced version).
/// `read_at` is when the running version's sources were read: a file
/// modified since then (while it compiled) is already a change.
pub fn watch(entry: PathBuf, files: Vec<PathBuf>, optimize: bool, reconstruct: bool, read_at: SystemTime) {
    badger_runtime::task::add_listener();
    let q = badger_runtime::task::external_queue();
    let stamp = |files: &[PathBuf]| -> Vec<Option<SystemTime>> {
        files.iter().map(|f| std::fs::metadata(f).and_then(|m| m.modified()).ok()).collect()
    };
    // the baseline is taken now, not whenever the thread starts running
    let mut last = stamp(&files);
    let mut changed = last.iter().any(|t| t.is_some_and(|t| t > read_at));
    // it recompiles: the compiler's room for deeply nested programs
    let watcher = std::thread::Builder::new().name("watch".into()).stack_size(512 << 20);
    let _ = watcher.spawn(move || {
        loop {
            if !changed {
                std::thread::sleep(Duration::from_millis(100));
                let now = stamp(&files);
                if now == last {
                    continue;
                }
            }
            changed = false;
            // let a writer finish
            std::thread::sleep(Duration::from_millis(50));
            last = stamp(&files);
            let entry = entry.clone();
            badger_runtime::task::post(
                &q,
                badger_runtime::task::External::Scheduler(Box::new(move || {
                    reload_session(entry, optimize, reconstruct)
                })),
            );
        }
    });
}

thread_local! {
    /// The watch session, kept across reloads: a rebuild then re-parses and
    /// re-checks only the files that changed, reusing the parsed and checked
    /// standard library. `Session` is deliberately single-threaded (its
    /// cache holds `Rc`s) and reloads run on the scheduler thread, so a
    /// thread-local is where it can live. Its `SourceMap` accumulates one
    /// copy of each re-read file per reload.
    static SESSION: std::cell::RefCell<Session> = std::cell::RefCell::new(Session::new());
}

fn reload_session(entry: PathBuf, optimize: bool, reconstruct: bool) {
    SESSION.with(|s| reload(&mut s.borrow_mut(), entry, optimize, reconstruct));
}

fn reload(s: &mut Session, entry: PathBuf, optimize: bool, reconstruct: bool) {
    s.reset_for_build();
    // definitions made through a connected REPL stay part of the program
    s.entry_suffix = crate::inspect::repl_definitions();
    let opts = RunOptions {
        optimize,
        watch: true,
        workers: Some(badger_runtime::task::workers()),
        ..Default::default()
    };
    match s.build(&entry, &opts) {
        Ok(built) => {
            let es = entries(&built);
            if std::env::var_os("BADGER_DEBUG_LIVE").is_some() {
                for e in &es {
                    eprintln!("[live] entry {} : {}", e.key, e.sig);
                }
            }
            match badger_runtime::live::incompatibility(&built.types, &es) {
                Some(why) if reconstruct => {
                    let main = built.prog.entry.and_then(|f| {
                        built.prog.static_closures.iter().find(|(_, g)| *g == f).map(|(o, _)| *o)
                    });
                    let Some(main) = main else {
                        eprintln!("[live] reload rejected: the new version has no `main`");
                        return;
                    };
                    let v = badger_runtime::live::publish_reconstructed(&built.types, &es);
                    eprintln!("[live] version {v}: {why}; reconstructing the program");
                    badger_runtime::task::reconstruct(main);
                    std::mem::forget(built);
                }
                Some(why) => eprintln!(
                    "[live] reload rejected: {why}; the running version continues (use --reconstruct to restart on it)"
                ),
                None => {
                    let (v, n) = badger_runtime::live::publish(&built.types, &es);
                    eprintln!("[live] version {v}: {n} function(s) replaced");
                    // old and new code stay mapped for the life of the process
                    std::mem::forget(built);
                }
            }
        }
        Err(_) => {
            let _ = s.diagnostics.render_with(&s.sources, &mut std::io::stderr(), false);
            eprintln!("[live] reload failed checks; the running version continues");
        }
    }
}
