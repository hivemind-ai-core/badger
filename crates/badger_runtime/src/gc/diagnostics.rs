//! Opt-in collector phase measurements and experimental policy controls.
//! Compiled only with `gc-profile`; normal builds contain none of this code.
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use serde_json::{Value, json};

struct Settings {
    path: Option<String>,
    young_bytes: Option<usize>,
    max_minors: Option<usize>,
    trim: bool,
    pool_max: Option<usize>,
    pool_refill: Option<usize>,
    buffer_inline: Option<usize>,
    reuse_sweep: Option<bool>,
    sweep_prefetch: Option<usize>,
}
fn settings() -> &'static Settings {
    static SETTINGS: OnceLock<Settings> = OnceLock::new();
    SETTINGS.get_or_init(|| {
        let number = |key| {
            std::env::var(key).ok().map(|v| {
                v.parse::<usize>().unwrap_or_else(|_| panic!("invalid diagnostic setting {key}={v}"))
            })
        };
        let pool_max = number("BADGER_GC_POOL_MAX");
        assert!(
            matches!(pool_max, None | Some(256 | 512 | 1024)),
            "BADGER_GC_POOL_MAX must be 256, 512 or 1024"
        );
        let pool_refill = number("BADGER_GC_POOL_REFILL_KIB");
        let buffer_inline = number("BADGER_BUFFER_INLINE_BYTES");
        let sweep_prefetch = number("BADGER_GC_SWEEP_PREFETCH");
        assert!(
            matches!(sweep_prefetch, None | Some(0 | 8 | 16 | 32 | 64 | 128)),
            "BADGER_GC_SWEEP_PREFETCH must be 0, 8, 16, 32, 64 or 128"
        );
        assert!(
            matches!(buffer_inline, None | Some(0 | 128 | 256)),
            "BADGER_BUFFER_INLINE_BYTES must be 0, 128 or 256"
        );
        assert!(
            matches!(pool_refill, None | Some(256 | 512 | 2048)),
            "BADGER_GC_POOL_REFILL_KIB must be 256, 512 or 2048"
        );
        Settings {
            path: std::env::var("BADGER_GC_PROFILE").ok(),
            young_bytes: number("BADGER_GC_MINOR_BYTES"),
            max_minors: number("BADGER_GC_MAX_MINORS"),
            trim: !std::env::var("BADGER_GC_TRIM").is_ok_and(|v| v == "0"),
            pool_max,
            pool_refill: pool_refill.map(|kib| kib << 10),
            buffer_inline,
            sweep_prefetch,
            reuse_sweep: std::env::var("BADGER_GC_SWEEP_REUSE").ok().map(|v| match v.as_str() {
                "0" => false,
                "1" => true,
                _ => panic!("BADGER_GC_SWEEP_REUSE must be 0 or 1"),
            }),
        }
    })
}
pub(super) fn young_bytes(default: usize) -> usize {
    settings().young_bytes.unwrap_or(default).max(1)
}
pub(super) fn max_minors(default: usize) -> usize {
    settings().max_minors.unwrap_or(default)
}
pub(super) fn trim() -> bool {
    settings().trim
}
pub(super) fn pool_max(default: usize) -> usize {
    settings().pool_max.unwrap_or(default)
}
pub(super) fn pool_refill(default: usize) -> usize {
    settings().pool_refill.unwrap_or(default)
}
pub(crate) fn buffer_inline_bytes(default: usize) -> usize {
    settings().buffer_inline.unwrap_or(default)
}
pub(super) fn reuse_sweep_helpers(default: bool) -> bool {
    settings().reuse_sweep.unwrap_or(default)
}
pub(super) fn sweep_prefetch(default: usize) -> usize {
    settings().sweep_prefetch.unwrap_or(default)
}

pub(super) struct Collection {
    clock: Option<Instant>,
    phases: BTreeMap<&'static str, u64>,
    pub fields: BTreeMap<&'static str, Value>,
}
impl Collection {
    pub fn new() -> Self {
        Self {
            clock: settings().path.as_ref().map(|_| Instant::now()),
            phases: BTreeMap::new(),
            fields: BTreeMap::new(),
        }
    }
    pub fn phase(&mut self, name: &'static str) {
        if let Some(clock) = &mut self.clock {
            let now = Instant::now();
            self.phases.insert(name, now.duration_since(*clock).as_nanos() as u64);
            *clock = now;
        }
    }
    pub fn snapshot(&mut self, name: &'static str, value: usize) {
        if self.clock.is_some() {
            self.fields.insert(name, json!(value));
        }
    }
    pub fn snapshot_counts(&mut self, name: &'static str, values: impl Iterator<Item = usize>) {
        if self.clock.is_some() {
            self.fields.insert(name, json!(values.collect::<Vec<_>>()));
        }
    }
    pub fn finish(self) {
        if self.clock.is_some() {
            RECORDS.lock().unwrap().push(json!({"phases_ns": self.phases, "heap": self.fields}));
        }
    }
}
static RECORDS: Mutex<Vec<Value>> = Mutex::new(Vec::new());

pub fn finish() {
    super::allocations::finish();
    let Some(path) = &settings().path else { return };
    static FINISHED: AtomicBool = AtomicBool::new(false);
    if FINISHED.swap(true, Ordering::Relaxed) {
        return;
    }
    let records = std::mem::take(&mut *RECORDS.lock().unwrap());
    let output = json!({"schema": 1, "collections": records,
        "policy": {"minor_bytes": settings().young_bytes, "max_minors": settings().max_minors,
                   "trim": settings().trim, "pool_max_bytes": pool_max(super::SMALL_MAX),
                   "pool_refill_bytes": pool_refill(super::BIG_CHUNK),
                   "buffer_inline_bytes": buffer_inline_bytes(crate::buffer::INLINE_BYTES),
                   "sweep_prefetch_objects": sweep_prefetch(super::SWEEP_PREFETCH),
                   "sweep_prefetch_supported": cfg!(target_arch = "x86_64"),
                   "reuse_sweep_helpers": reuse_sweep_helpers(super::REUSE_SWEEP_HELPERS)},
        "note": "Diagnostic wall times. Phases are disjoint within each collection; stop_world includes rendezvous. Sampling/lock stats perturb execution."});
    if let Err(e) = std::fs::write(path, serde_json::to_vec_pretty(&output).unwrap()) {
        eprintln!("cannot write GC diagnostics {path}: {e}");
    }
}
