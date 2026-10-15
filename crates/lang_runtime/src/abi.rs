//! Core entry points called directly by generated code (allocation, defects,
//! scheduling polls, strings, structural operations, collections).

use crate::desc::TypeDesc;
use crate::gc;
use crate::string::{new_string, str_of};
use crate::task::{self, TaskCtx};

fn desc(w: u64) -> &'static TypeDesc {
    unsafe { &*(w as *const TypeDesc) }
}

pub extern "C" fn lang_rt_alloc(_ctx: *mut TaskCtx, d: u64, size: u64) -> u64 {
    gc::alloc(desc(d), size as usize)
}

pub extern "C" fn lang_rt_panic(_ctx: *mut TaskCtx, msg: u64, loc: u64) -> u64 {
    let m = str_of(msg).to_string();
    let l = str_of(loc).to_string();
    task::panic_current(m, l)
}

/// Checked-operation failures with a source location.
pub extern "C" fn lang_rt_defect(_ctx: *mut TaskCtx, kind: u64, loc: u64) -> u64 {
    let m = match kind {
        0 => "integer overflow",
        1 => "division by zero",
        2 => "index out of bounds",
        3 => "assertion failed",
        4 => "unreachable code reached",
        5 => "invalid shift amount",
        6 => "lossy numeric conversion",
        7 => "missing map key",
        8 => "negative duration (use signed arithmetic on `time.as_nanos`)",
        _ => "runtime defect",
    };
    task::panic_current(m.to_string(), str_of(loc).to_string())
}

/// Scheduling/cancellation safe point (called when the budget runs out).
/// A foreign call with a declared contract (bit 0 blocking, bit 1 pinned):
/// run it on the blocking pool or the pinned thread while the task parks.
/// Arguments and the result are machine words (integers and pointers).
pub extern "C" fn lang_rt_ffi_call(_ctx: *mut TaskCtx, f: u64, args: u64, nargs: u64, contract: u64) -> u64 {
    let mut a = [0u64; 8];
    for (i, slot) in a.iter_mut().enumerate().take((nargs as usize).min(8)) {
        *slot = unsafe { *(args as *const u64).add(i) };
    }
    let call = move || {
        // integer/pointer arguments travel in registers; unused ones are ignored
        let fp: extern "C" fn(u64, u64, u64, u64, u64, u64, u64, u64) -> u64 =
            unsafe { std::mem::transmute(f as *const ()) };
        fp(a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7])
    };
    if contract & 2 != 0 { task::run_pinned(call) } else { task::run_blocking(call) }
}

/// Where the error now being propagated by `?` first came from: the
/// `Result` object returned and the location of its first propagation.
static ERR_ORIGIN: std::sync::Mutex<(u64, u64)> = std::sync::Mutex::new((0, 0));

/// `?` propagates `prev` (the failed `Result`) as `next`: a continuing
/// chain keeps its origin, a new one starts at `loc`.
pub extern "C" fn lang_rt_err_origin(_ctx: *mut TaskCtx, prev: u64, next: u64, loc: u64) {
    let mut g = ERR_ORIGIN.lock().unwrap();
    let origin = if g.0 == prev && g.1 != 0 { g.1 } else { loc };
    *g = (next, origin);
}

/// The source location where the error in `result` started propagating.
pub fn error_origin(result: u64) -> Option<String> {
    let g = ERR_ORIGIN.lock().unwrap();
    (g.0 == result && g.1 != 0).then(|| str_of(g.1).to_string())
}

/// The stack check in a function prologue failed.
pub extern "C" fn lang_rt_stack_overflow(_ctx: *mut TaskCtx) -> u64 {
    task::stack_overflow()
}

/// The source location of a call to a test helper (`std.test`), reported
/// when the helper fails the test.
pub extern "C" fn lang_rt_caller_loc(_ctx: *mut TaskCtx, loc: u64) {
    if let Some(t) = task::current() {
        t.caller_loc = loc;
    }
}

pub extern "C" fn lang_rt_poll(_ctx: *mut TaskCtx) {
    task::yield_now();
}

/// Tasks initializing (and waiting for) module-level values, by flag
/// address. Flags: 0 = uninitialized, 1 = initializing, 2 = ready.
static mut GLOBAL_INIT: Option<rustc_hash::FxHashMap<u64, (task::TaskId, Vec<task::TaskId>)>> = None;

/// Slow path of a module-level value getter: returns 1 when the caller must
/// run the initializer, 0 once the value is ready (waiting for another
/// task's initialization to finish).
#[allow(static_mut_refs)]
pub extern "C" fn lang_rt_global_init(_ctx: *mut TaskCtx, flag: u64) -> u64 {
    let f = flag as *mut u64;
    loop {
        let table = unsafe { GLOBAL_INIT.get_or_insert_with(Default::default) };
        match unsafe { *f } {
            2 => return 0,
            0 => {
                unsafe { *f = 1 };
                table.insert(flag, (task::current_id(), vec![]));
                return 1;
            }
            _ => {
                let me = task::current_id();
                match table.get_mut(&flag) {
                    Some((owner, _)) if *owner == me => {
                        task::panic_current(
                            "a module-level value was read during its own initialization".into(),
                            String::new(),
                        );
                    }
                    Some((_, waiters)) => waiters.push(me),
                    None => return 0,
                }
                task::park_for("module initialization");
            }
        }
    }
}

/// The initializer stored the value: mark it ready and wake waiters.
#[allow(static_mut_refs)]
pub extern "C" fn lang_rt_global_done(_ctx: *mut TaskCtx, flag: u64) {
    unsafe { *(flag as *mut u64) = 2 };
    if let Some((_, waiters)) = unsafe { GLOBAL_INIT.get_or_insert_with(Default::default).remove(&flag) } {
        for w in waiters {
            task::wake(w, task::Wake::Normal);
        }
    }
}

/// Concatenate `n` strings (interpolation).
pub extern "C" fn lang_rt_concat(_ctx: *mut TaskCtx, parts: *const u64, n: u64) -> u64 {
    let mut s = String::new();
    for i in 0..n as usize {
        s.push_str(str_of(unsafe { *parts.add(i) }));
    }
    new_string(&s)
}

pub extern "C" fn lang_rt_eq(_ctx: *mut TaskCtx, d: u64, a: u64, b: u64) -> u64 {
    crate::value::eq(desc(d), a, b) as u64
}

pub extern "C" fn lang_rt_cmp(_ctx: *mut TaskCtx, d: u64, a: u64, b: u64) -> u64 {
    crate::value::cmp(desc(d), a, b) as i8 as i64 as u64
}

pub extern "C" fn lang_rt_display(ctx: *mut TaskCtx, d: u64, w: u64) -> u64 {
    let s = crate::value::display(ctx, desc(d), w, true);
    new_string(&s)
}

pub extern "C" fn lang_rt_list_new(_ctx: *mut TaskCtx, d: u64) -> u64 {
    let d = desc(d);
    if d.name == "Buffer" || d.name == "Array" {
        return crate::buffer::new(crate::buffer::elem_of(d), 0);
    }
    crate::list::new(d)
}

pub extern "C" fn lang_rt_list_push(_ctx: *mut TaskCtx, list: u64, w: u64) {
    if crate::buffer::is_buffer(list) {
        return crate::buffer::data(list).items.push(w);
    }
    crate::list::push(list, w)
}

pub extern "C" fn lang_rt_list_len(_ctx: *mut TaskCtx, list: u64) -> u64 {
    if crate::buffer::is_buffer(list) {
        return crate::buffer::data(list).items.len() as u64;
    }
    crate::list::len(list) as u64
}

/// Checked element access; `loc` reports the defect location.
pub extern "C" fn lang_rt_list_get(_ctx: *mut TaskCtx, list: u64, i: u64, loc: u64) -> u64 {
    if crate::buffer::is_buffer(list) {
        let items = &crate::buffer::data(list).items;
        if i as usize >= items.len() {
            task::panic_current(
                format!("index out of bounds: the length is {} but the index is {}", items.len(), i as i64),
                str_of(loc).into(),
            );
        }
        return items[i as usize];
    }
    let n = crate::list::len(list);
    if (i as i64) < 0 || i as usize >= n {
        task::panic_current(
            format!("index out of bounds: the length is {n} but the index is {}", i as i64),
            str_of(loc).into(),
        );
    }
    crate::list::get(list, i as usize)
}

pub extern "C" fn lang_rt_list_finish(_ctx: *mut TaskCtx, builder: u64, d: u64) -> u64 {
    crate::list::finish(builder, desc(d))
}

pub extern "C" fn lang_rt_map_new(_ctx: *mut TaskCtx, d: u64) -> u64 {
    crate::map::new(desc(d))
}

pub extern "C" fn lang_rt_map_insert_mut(_ctx: *mut TaskCtx, m: u64, k: u64, v: u64) {
    crate::map::insert_mut(m, k, v)
}

/// Checked map access `map[key]`.
pub extern "C" fn lang_rt_map_index(_ctx: *mut TaskCtx, m: u64, k: u64, loc: u64) -> u64 {
    match crate::map::get(m, k) {
        Some(v) => v,
        None => task::panic_current("missing map key".into(), str_of(loc).into()),
    }
}

pub fn symbols() -> Vec<(&'static str, *const u8)> {
    vec![
        ("lang_rt_alloc", lang_rt_alloc as *const u8),
        ("lang_rt_panic", lang_rt_panic as *const u8),
        ("lang_rt_defect", lang_rt_defect as *const u8),
        ("lang_rt_poll", lang_rt_poll as *const u8),
        ("lang_rt_caller_loc", lang_rt_caller_loc as *const u8),
        ("lang_rt_stack_overflow", lang_rt_stack_overflow as *const u8),
        ("lang_rt_ffi_call", lang_rt_ffi_call as *const u8),
        ("lang_rt_err_origin", lang_rt_err_origin as *const u8),
        ("lang_rt_global_init", lang_rt_global_init as *const u8),
        ("lang_rt_global_done", lang_rt_global_done as *const u8),
        ("lang_rt_concat", lang_rt_concat as *const u8),
        ("lang_rt_eq", lang_rt_eq as *const u8),
        ("lang_rt_cmp", lang_rt_cmp as *const u8),
        ("lang_rt_display", lang_rt_display as *const u8),
        ("lang_rt_list_new", lang_rt_list_new as *const u8),
        ("lang_rt_list_push", lang_rt_list_push as *const u8),
        ("lang_rt_list_len", lang_rt_list_len as *const u8),
        ("lang_rt_list_get", lang_rt_list_get as *const u8),
        ("lang_rt_list_finish", lang_rt_list_finish as *const u8),
        ("lang_rt_map_new", lang_rt_map_new as *const u8),
        ("lang_rt_map_insert_mut", lang_rt_map_insert_mut as *const u8),
        ("lang_rt_map_index", lang_rt_map_index as *const u8),
    ]
}
