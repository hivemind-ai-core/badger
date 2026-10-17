//! Versioned code dispatch for live reload (`badger run --watch`).
//!
//! In live mode, calls between user functions load their target from a
//! per-instance slot instead of calling a fixed address. Publishing a new
//! compatible version stores its code addresses in the slots: new dispatches
//! use new code, while frames already executing (and closures already
//! created) keep the code version they started with. Old code is never
//! unmapped.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

struct State {
    slots: HashMap<String, &'static AtomicU64>,
    /// Signature text of the published instance behind each slot.
    sigs: HashMap<String, String>,
    /// Fingerprint of the published version's user type definitions.
    types: String,
    version: u32,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut g = STATE.lock().unwrap();
    let s = g.get_or_insert_with(|| State {
        slots: HashMap::new(),
        sigs: HashMap::new(),
        types: String::new(),
        version: 0,
    });
    f(s)
}

/// The dispatch slot of an instance key (created empty on first use).
pub fn slot(key: &str) -> &'static AtomicU64 {
    with(|s| *s.slots.entry(key.to_string()).or_insert_with(|| Box::leak(Box::new(AtomicU64::new(0)))))
}

/// A version's functions: (key, signature text, code address, keep the
/// published one if present — module-level value getters keep their state).
pub struct Entry {
    pub key: String,
    pub sig: String,
    pub code: u64,
    pub keep_existing: bool,
}

/// Why a new version cannot replace the running one.
pub fn incompatibility(types: &str, entries: &[Entry]) -> Option<String> {
    with(|s| {
        if s.version == 0 {
            return None;
        }
        // adding a type is compatible; changing or removing one is not
        let name = |line: &str| line.split_whitespace().nth(1).map(str::to_string);
        let new: Vec<&str> = types.lines().collect();
        for old in s.types.lines() {
            let Some(n) = name(old) else { continue };
            match new.iter().find(|l| name(l).as_deref() == Some(n.as_str())) {
                Some(l) if *l == old => {}
                Some(_) => return Some(format!("type `{n}` changed")),
                None => return Some(format!("type `{n}` was removed")),
            }
        }
        for e in entries {
            if let Some(old) = s.sigs.get(&e.key)
                && *old != e.sig
            {
                return Some(format!("`{}` changed from `{old}` to `{}`", e.key, e.sig));
            }
        }
        None
    })
}

/// Make a version current; returns its number and how many slots changed.
pub fn publish(types: &str, entries: &[Entry]) -> (u32, usize) {
    let mut changed = 0;
    for e in entries {
        let slot = slot(&e.key);
        if e.keep_existing && slot.load(Ordering::Acquire) != 0 {
            continue;
        }
        if slot.swap(e.code, Ordering::AcqRel) != e.code {
            changed += 1;
        }
    }
    with(|s| {
        s.types = types.to_string();
        for e in entries {
            s.sigs.insert(e.key.clone(), e.sig.clone());
        }
        s.version += 1;
        publish_version(s.version);
        (s.version, changed)
    })
}

/// Number of the current code version (0 before any publication; 1 for a
/// program not run with `--watch`).
pub fn version() -> u32 {
    VERSION.load(std::sync::atomic::Ordering::Acquire).max(1)
}

/// The current version, read without the state lock (every spawn reads it).
static VERSION: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn publish_version(v: u32) {
    VERSION.store(v, std::sync::atomic::Ordering::Release);
}

/// Make a version current although it is incompatible with the running one
/// (controlled reconstruction restarts the program on it): every slot,
/// including module-level value getters, moves to the new code.
pub fn publish_reconstructed(types: &str, entries: &[Entry]) -> u32 {
    for e in entries {
        slot(&e.key).store(e.code, Ordering::Release);
    }
    with(|s| {
        s.types = types.to_string();
        s.sigs.clear();
        for e in entries {
            s.sigs.insert(e.key.clone(), e.sig.clone());
        }
        s.version += 1;
        publish_version(s.version);
        s.version
    })
}
