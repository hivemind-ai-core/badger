//! `lang run --watch`: recompile when sources change and replace compatible
//! functions in the running program (dispatch slots in `lang_runtime::live`).
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
pub fn type_fingerprint(g: &lang_typeck::Global<'_>) -> String {
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
                .map(|(l, t, _)| format!("{}: {}", g.interner.resolve(*l), lang_typeck::show_ty(g, t)))
                .collect();
            out.push(format!("struct {name} {{ {} }}", fields.join(", ")));
        }
        if let Some(e) = g.env.enums.get(&id) {
            let vs: Vec<String> = e
                .variants
                .iter()
                .map(|v| {
                    let fs: Vec<String> = v.fields.iter().map(|(_, t)| lang_typeck::show_ty(g, t)).collect();
                    format!("{}({})", g.interner.resolve(v.name), fs.join(", "))
                })
                .collect();
            out.push(format!("enum {name} {{ {} }}", vs.join(", ")));
        }
    }
    out.sort();
    out.join("\n")
}

pub(crate) fn entries(b: &Built) -> Vec<lang_runtime::live::Entry> {
    b.prog
        .funcs
        .iter_enumerated()
        .filter_map(|(id, f)| {
            f.live_key.as_ref().map(|key| lang_runtime::live::Entry {
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
    lang_runtime::live::publish(&b.types, &entries(b));
}

/// Poll the sources; on a change, recompile on the scheduler thread (between
/// task steps, so no task observes a half-replaced version).
pub fn watch(entry: PathBuf, files: Vec<PathBuf>, optimize: bool, reconstruct: bool) {
    lang_runtime::task::add_listener();
    let q = lang_runtime::task::external_queue();
    let stamp = |files: &[PathBuf]| -> Vec<Option<SystemTime>> {
        files.iter().map(|f| std::fs::metadata(f).and_then(|m| m.modified()).ok()).collect()
    };
    std::thread::spawn(move || {
        let mut last = stamp(&files);
        loop {
            std::thread::sleep(Duration::from_millis(100));
            let now = stamp(&files);
            if now == last {
                continue;
            }
            // let a writer finish
            std::thread::sleep(Duration::from_millis(50));
            last = stamp(&files);
            let entry = entry.clone();
            lang_runtime::task::post(
                &q,
                lang_runtime::task::External::Scheduler(Box::new(move || {
                    reload(entry, optimize, reconstruct)
                })),
            );
        }
    });
}

fn reload(entry: PathBuf, optimize: bool, reconstruct: bool) {
    let mut s = Session::new();
    // definitions made through a connected REPL stay part of the program
    s.entry_suffix = crate::inspect::repl_definitions();
    let opts = RunOptions {
        optimize,
        watch: true,
        workers: Some(lang_runtime::task::workers()),
        ..Default::default()
    };
    match s.build(&entry, &opts) {
        Ok(built) => {
            let es = entries(&built);
            if std::env::var_os("LANG_DEBUG_LIVE").is_some() {
                for e in &es {
                    eprintln!("[live] entry {} : {}", e.key, e.sig);
                }
            }
            match lang_runtime::live::incompatibility(&built.types, &es) {
                Some(why) if reconstruct => {
                    let main = built.prog.entry.and_then(|f| {
                        built.prog.static_closures.iter().find(|(_, g)| *g == f).map(|(o, _)| *o)
                    });
                    let Some(main) = main else {
                        eprintln!("[live] reload rejected: the new version has no `main`");
                        return;
                    };
                    let v = lang_runtime::live::publish_reconstructed(&built.types, &es);
                    eprintln!("[live] version {v}: {why}; reconstructing the program");
                    lang_runtime::task::reconstruct(main);
                    std::mem::forget(built);
                }
                Some(why) => eprintln!(
                    "[live] reload rejected: {why}; the running version continues (use --reconstruct to restart on it)"
                ),
                None => {
                    let (v, n) = lang_runtime::live::publish(&built.types, &es);
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
