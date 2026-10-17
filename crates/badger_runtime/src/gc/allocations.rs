//! Managed allocation census. Diagnostic storage is native and is not counted.
//! Side metadata follows OBJECTS' stable compaction, without changing headers.
use std::panic::Location;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use rustc_hash::FxHashMap;
use serde_json::json;

use super::{HEADER, Header, MARK, cell_size, layout, small_max};
use crate::desc::TypeDesc;

fn path() -> Option<&'static str> {
    static PATH: OnceLock<Option<String>> = OnceLock::new();
    PATH.get_or_init(|| std::env::var("BADGER_ALLOC_PROFILE").ok()).as_deref()
}

/// Items storage only: allocator calls, Vec adoption and owned capacity.
/// Inline bytes are a subset of managed payloads, never extra native bytes.
pub(crate) enum BufferStorageEvent {
    NativeAlloc,
    NativeRealloc,
    NativeAdopt,
    NativeFree,
    Inline,
    InlineGrow,
}
const BUFFER_EVENTS: [&str; 6] =
    ["native_alloc", "native_realloc_growth", "native_adopt", "native_free", "inline", "inline_to_native"];
static BUFFER_CALLS: [AtomicU64; 6] = [const { AtomicU64::new(0) }; 6];
static BUFFER_BYTES: [AtomicU64; 6] = [const { AtomicU64::new(0) }; 6];

pub(crate) fn record_buffer_storage(event: BufferStorageEvent, bytes: usize) {
    if path().is_none() {
        return;
    }
    let event = event as usize;
    BUFFER_CALLS[event].fetch_add(1, Ordering::Relaxed);
    BUFFER_BYTES[event].fetch_add(bytes as u64, Ordering::Relaxed);
}

fn buffer_storage_report() -> serde_json::Value {
    let calls = BUFFER_CALLS.each_ref().map(|v| v.load(Ordering::Relaxed));
    let bytes = BUFFER_BYTES.each_ref().map(|v| v.load(Ordering::Relaxed));
    let charged = bytes[0] + bytes[1] + bytes[2];
    let blocks = calls[0] + calls[2];
    assert!(charged >= bytes[3] && blocks >= calls[3], "buffer storage ownership underflow");
    let events: serde_json::Map<_, _> = BUFFER_EVENTS
        .iter()
        .enumerate()
        .map(|(i, name)| ((*name).to_string(), json!({"calls": calls[i], "bytes": bytes[i]})))
        .collect();
    json!({
        "events": events,
        "native_owned_bytes_remaining": charged - bytes[3],
        "native_owned_blocks_remaining": blocks - calls[3],
        "note": "Items storage only, process-wide. Native alloc/realloc are successful allocator API calls; realloc bytes are capacity growth, not all bytes internally moved. Adoption transfers existing Vec storage and is not a new malloc call. Temporary Vec construction and other native allocations are excluded. Inline bytes are already included in managed totals. No timing/RSS inference from diagnostic counts."
    })
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct Key {
    descriptor: usize,
    file: &'static str,
    line: u32,
    column: u32,
}

#[derive(Clone, Copy, Default)]
struct Volume {
    objects: u64,
    requested_bytes: u64,
    cell_bytes: u64,
}
impl Volume {
    fn object(payload: usize) -> Self {
        Self {
            objects: 1,
            requested_bytes: (HEADER + payload) as u64,
            cell_bytes: if cell_size(payload) <= small_max() {
                cell_size(payload) as u64
            } else {
                layout(payload).size() as u64
            },
        }
    }
    fn add(&mut self, other: Self) {
        self.objects += other.objects;
        self.requested_bytes += other.requested_bytes;
        self.cell_bytes += other.cell_bytes;
    }
    fn minus(self, other: Self) -> Self {
        Self {
            objects: self.objects - other.objects,
            requested_bytes: self.requested_bytes - other.requested_bytes,
            cell_bytes: self.cell_bytes - other.cell_bytes,
        }
    }
    fn report(self) -> serde_json::Value {
        json!({"objects": self.objects, "requested_bytes": self.requested_bytes, "cell_bytes": self.cell_bytes})
    }
}

#[derive(Default)]
struct Counts {
    allocated: Volume,
    reclaimed_before_survival: Volume,
    reclaimed_after_survival: Volume,
    first_survival: Volume,
    minor_survivor_observations: Volume,
    full_survivor_observations: Volume,
}
impl Counts {
    fn add(&mut self, other: &Self) {
        self.allocated.add(other.allocated);
        self.reclaimed_before_survival.add(other.reclaimed_before_survival);
        self.reclaimed_after_survival.add(other.reclaimed_after_survival);
        self.first_survival.add(other.first_survival);
        self.minor_survivor_observations.add(other.minor_survivor_observations);
        self.full_survivor_observations.add(other.full_survivor_observations);
    }
    fn report(&self) -> serde_json::Value {
        let reclaimed = {
            let mut n = self.reclaimed_before_survival;
            n.add(self.reclaimed_after_survival);
            n
        };
        json!({
            "allocated": self.allocated.report(),
            "reclaimed_before_survival": self.reclaimed_before_survival.report(),
            "reclaimed_after_survival": self.reclaimed_after_survival.report(),
            "first_survival": self.first_survival.report(),
            "minor_survivor_observations": self.minor_survivor_observations.report(),
            "full_survivor_observations": self.full_survivor_observations.report(),
            "remaining": self.allocated.minus(reclaimed).report(),
            "remaining_after_survival": self.first_survival.minus(self.reclaimed_after_survival).report(),
        })
    }
}

struct Site {
    key: Key,
    name: String,
    counts: Counts,
}
#[derive(Clone, Copy)]
struct Object {
    site: usize,
    survived: bool,
}
struct Worker {
    sites: Vec<Site>,
    by_key: FxHashMap<Key, usize>,
    objects: Vec<Object>,
    // Exact small-cell classes, followed by all system-allocated large cells.
    sizes: [Volume; super::CLASSES + 1],
    requested_sizes: [Volume; 65],
}
impl Default for Worker {
    fn default() -> Self {
        Self {
            sites: Vec::new(),
            by_key: FxHashMap::default(),
            objects: Vec::new(),
            sizes: [Volume::default(); super::CLASSES + 1],
            requested_sizes: [Volume::default(); 65],
        }
    }
}
static WORKERS: [Mutex<Worker>; crate::task::MAX_WORKERS] = [const {
    Mutex::new(Worker {
        sites: Vec::new(),
        by_key: FxHashMap::with_hasher(rustc_hash::FxBuildHasher),
        objects: Vec::new(),
        sizes: [Volume { objects: 0, requested_bytes: 0, cell_bytes: 0 }; super::CLASSES + 1],
        requested_sizes: [Volume { objects: 0, requested_bytes: 0, cell_bytes: 0 }; 65],
    })
}; crate::task::MAX_WORKERS];

pub(super) fn record(
    worker: usize,
    desc: &'static TypeDesc,
    payload: usize,
    length: usize,
    caller: &'static Location<'static>,
) {
    if path().is_none() {
        return;
    }
    let key = Key { descriptor: desc.id, file: caller.file(), line: caller.line(), column: caller.column() };
    let mut w = WORKERS[worker].lock().unwrap();
    assert_eq!(w.objects.len() + 1, length, "allocation census lost object alignment");
    let site = if let Some(&site) = w.by_key.get(&key) {
        site
    } else {
        let site = w.sites.len();
        w.sites.push(Site { key, name: desc.name.clone(), counts: Counts::default() });
        w.by_key.insert(key, site);
        site
    };
    let volume = Volume::object(payload);
    w.sites[site].counts.allocated.add(volume);
    let class = if cell_size(payload) <= small_max() { cell_size(payload) / 16 - 1 } else { super::CLASSES };
    w.sizes[class].add(volume);
    w.requested_sizes[(cell_size(payload) / 16 - 1).min(64)].add(volume);
    w.objects.push(Object { site, survived: false });
}

impl Worker {
    // Called only with mutators stopped, before marks are cleared or objects
    // finalized. Minor collections deliberately leave the old prefix alone.
    fn sweep(&mut self, list: &[*mut Header], start: usize, minor: bool) {
        assert_eq!(self.objects.len(), list.len(), "allocation census lost sweep alignment");
        let mut j = start;
        for (i, &p) in list.iter().enumerate().skip(start) {
            let mut object = self.objects[i];
            let counts = &mut self.sites[object.site].counts;
            let volume = Volume::object(unsafe { (*p).payload_size() });
            if unsafe { (*p).meta } & MARK != 0 {
                if !object.survived {
                    counts.first_survival.add(volume);
                    object.survived = true;
                }
                if minor {
                    counts.minor_survivor_observations.add(volume);
                } else {
                    counts.full_survivor_observations.add(volume);
                }
                self.objects[j] = object;
                j += 1;
            } else if object.survived {
                counts.reclaimed_after_survival.add(volume);
            } else {
                counts.reclaimed_before_survival.add(volume);
            }
        }
        self.objects.truncate(j);
    }
}

pub(super) fn before_sweep(worker: usize, list: &[*mut Header], start: usize, minor: bool) {
    if path().is_some() {
        WORKERS[worker].lock().unwrap().sweep(list, start, minor);
    }
}

pub(super) fn finish() {
    let Some(path) = path() else { return };
    static FINISHED: AtomicBool = AtomicBool::new(false);
    if FINISHED.swap(true, Ordering::Relaxed) {
        return;
    }
    let mut sites: FxHashMap<Key, (String, Counts)> = FxHashMap::default();
    let mut metadata_capacity = 0;
    let mut remaining_objects = 0;
    let mut sizes = [Volume::default(); super::CLASSES + 1];
    let mut requested_sizes = [Volume::default(); 65];
    for worker in &WORKERS {
        let worker = worker.lock().unwrap();
        metadata_capacity += worker.objects.capacity() * std::mem::size_of::<Object>();
        remaining_objects += worker.objects.len();
        for (total, volume) in sizes.iter_mut().zip(worker.sizes) {
            total.add(volume);
        }
        for (total, volume) in requested_sizes.iter_mut().zip(worker.requested_sizes) {
            total.add(volume);
        }
        for site in &worker.sites {
            sites
                .entry(site.key)
                .or_insert_with(|| (site.name.clone(), Counts::default()))
                .1
                .add(&site.counts);
        }
    }
    let mut sites: Vec<_> = sites.into_iter().collect();
    sites.sort_by(|(a, (an, _)), (b, (bn, _))| {
        (an, a.file, a.line, a.column).cmp(&(bn, b.file, b.line, b.column))
    });
    let mut total = Counts::default();
    let rows: Vec<_> = sites.into_iter().map(|(key, (name, counts))| {
        total.add(&counts);
        json!({"type": name, "type_id": key.descriptor, "runtime_source": {"file": key.file, "line": key.line, "column": key.column}, "counts": counts.report()})
    }).collect();
    let output = json!({
        "schema": 1,
        "scope": "process-managed-allocations",
        "buffer_storage": buffer_storage_report(),
        "note": "Runtime Rust call sites, not language source locations. Includes startup/output allocation. Managed totals exclude immortals, external buffers, task stacks and other native allocations; buffer_storage separately reports Items allocator API calls and owned capacity. Reclaimed means removed by sweep, including quarantined cells; remaining is unswept, not proven reachable. Minor collections do not examine old objects. Survivor observations can count an object repeatedly. Survival is not escape/uniqueness proof. Diagnostic counters, mutexes and side metadata perturb timing and memory.",
        "object_metadata_capacity_bytes": metadata_capacity,
        "remaining_metadata_objects": remaining_objects,
        "allocation_sizes": {
            "small_pool_classes": sizes[..small_max()/16].iter().enumerate().map(|(i, volume)| json!({"cell_size": (i + 1) * 16, "allocated": volume.report()})).collect::<Vec<_>>(),
            "large_system_cells": sizes[super::CLASSES].report(),
        },
        "requested_size_classes": {
            "through_1024": requested_sizes[..64].iter().enumerate().map(|(i, volume)| json!({"rounded_total_bytes": (i + 1) * 16, "allocated": volume.report()})).collect::<Vec<_>>(),
            "over_1024": requested_sizes[64].report(),
        },
        "totals": total.report(), "sites": rows,
    });
    if let Err(e) = std::fs::write(path, serde_json::to_vec_pretty(&output).unwrap()) {
        eprintln!("cannot write allocation diagnostics {path}: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn census_follows_minor_and_full_sweep_compaction_and_cell_reuse() {
        let mut worker = Worker::default();
        for descriptor in 0..2 {
            worker.sites.push(Site {
                key: Key { descriptor, file: "test", line: 1, column: 1 },
                name: format!("type{descriptor}"),
                counts: Counts::default(),
            });
        }
        let mut headers: Vec<_> = [0, 239, 240, 257]
            .into_iter()
            .map(|payload| Box::new(Header { desc: std::ptr::null(), meta: payload << 32 }))
            .collect();
        let mut list: Vec<_> = headers.iter_mut().map(|h| &mut **h as *mut Header).collect();
        for (i, h) in headers.iter().enumerate() {
            worker.sites[i % 2].counts.allocated.add(Volume::object(h.payload_size()));
            worker.objects.push(Object { site: i % 2, survived: false });
        }
        headers[1].meta |= MARK;
        headers[2].meta |= MARK;
        worker.sweep(&list, 0, false);
        super::super::sweep_from(&mut list, 0, true, |_| {}, |_| {});
        assert_eq!(list, vec![&mut *headers[1] as *mut Header, &mut *headers[2] as *mut Header]);
        assert_eq!(worker.objects.iter().map(|o| o.site).collect::<Vec<_>>(), vec![1, 0]);
        assert!(worker.objects.iter().all(|o| o.survived));

        // Reuse an already dead cell. A new young object survives a minor;
        // a second minor must not count the old prefix as newly surviving.
        let p = &mut *headers[0] as *mut Header;
        list.push(p);
        headers[0].meta |= MARK;
        worker.objects.push(Object { site: 0, survived: false });
        worker.sites[0].counts.allocated.add(Volume::object(0));
        worker.sweep(&list, 2, true);
        super::super::sweep_from(&mut list, 2, true, |_| {}, |_| {});
        worker.sweep(&list, 3, true);
        assert_eq!(worker.sites[0].counts.minor_survivor_observations.objects, 1);
        assert_eq!(worker.sites[0].counts.first_survival.objects, 2);

        // All old objects become unreachable; only a full sweep reclaims them.
        for h in &mut headers {
            h.meta &= !MARK;
        }
        worker.sweep(&list, 0, false);
        super::super::sweep_from(&mut list, 0, true, |_| {}, |_| {});
        assert!(list.is_empty());
        assert!(worker.objects.is_empty());
        for site in &worker.sites {
            let report = site.counts.report();
            assert_eq!(report["remaining"], Volume::default().report());
            assert_eq!(report["remaining_after_survival"], Volume::default().report());
        }
        assert_eq!(worker.sites[0].counts.reclaimed_before_survival.objects, 1);
        assert_eq!(worker.sites[0].counts.reclaimed_after_survival.objects, 2);
        assert_eq!(worker.sites[1].counts.reclaimed_before_survival.objects, 1);
        assert_eq!(worker.sites[1].counts.reclaimed_after_survival.objects, 1);
    }

    #[test]
    fn bytes_follow_actual_small_pool_and_large_layout_boundaries() {
        for (payload, requested, cell) in [
            (0, 16, 32),
            (1, 17, 32),
            (240, 256, 256),
            (241, 257, if small_max() >= 512 { 272 } else { 257 }),
            (497, 513, if small_max() >= 1024 { 528 } else { 513 }),
            (1009, 1025, 1025),
        ] {
            let volume = Volume::object(payload);
            assert_eq!(volume.objects, 1);
            assert_eq!(volume.requested_bytes, requested);
            assert_eq!(volume.cell_bytes, cell);
        }
    }
}
