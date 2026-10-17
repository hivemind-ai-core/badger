//! Observations for `lang contracts --tests`: while tests run, compiled
//! code reports the integer arguments and results of the program's
//! functions and each contract clause it evaluates. The compiler reads the
//! table afterwards to report which clauses tests exercised and what
//! invariants held in every observed call.

use std::collections::HashMap;
use std::sync::Mutex;

/// Observed range of one integer slot (a parameter or a result).
#[derive(Debug, Clone, Copy)]
pub struct Range {
    pub count: u64,
    pub min: i64,
    pub max: i64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Clause {
    pub evaluated: u64,
    pub held: u64,
}

#[derive(Default)]
struct Table {
    slots: HashMap<u64, Range>,
    clauses: HashMap<u64, Clause>,
}

static TABLE: Mutex<Option<Table>> = Mutex::new(None);

/// Start (or restart) observing.
pub fn reset() {
    *TABLE.lock().unwrap() = Some(Table::default());
}

/// What was observed: integer slots and clause counts, by their ids.
pub fn take() -> (HashMap<u64, Range>, HashMap<u64, Clause>) {
    match TABLE.lock().unwrap().take() {
        Some(t) => (t.slots, t.clauses),
        None => Default::default(),
    }
}

pub extern "C" fn lang_rt_observe_int(_ctx: *mut crate::task::TaskCtx, slot: u64, value: u64) -> u64 {
    if let Some(t) = TABLE.lock().unwrap().as_mut() {
        let v = value as i64;
        let r = t.slots.entry(slot).or_insert(Range { count: 0, min: v, max: v });
        r.count += 1;
        r.min = r.min.min(v);
        r.max = r.max.max(v);
    }
    0
}

pub extern "C" fn lang_rt_observe_clause(_ctx: *mut crate::task::TaskCtx, id: u64, held: u64) -> u64 {
    if let Some(t) = TABLE.lock().unwrap().as_mut() {
        let c = t.clauses.entry(id).or_default();
        c.evaluated += 1;
        if held & 1 != 0 {
            c.held += 1;
        }
    }
    0
}

pub fn symbols() -> Vec<(&'static str, *const u8)> {
    vec![
        ("lang_rt_observe_int", lang_rt_observe_int as *const u8),
        ("lang_rt_observe_clause", lang_rt_observe_clause as *const u8),
    ]
}
