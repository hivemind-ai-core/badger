//! HTTP settings of the runtime configuration (the server itself is the
//! `lang_http` native module), and process shutdown signals and
//! running-system registration for `std.system`.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use crate::desc::TypeDesc;
use crate::intrinsics::{Targs, rdesc};
use crate::string::{bytes_of, new_string, str_of};
use crate::task::{self, TaskCtx, TaskId};
use crate::{list, make_record};

/// Explicit request limits: body and header sizes, and bounded admission
/// (requests in flight; more are answered 503 without running a handler).
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_body_bytes: usize,
    pub max_header_bytes: usize,
    pub max_in_flight: usize,
}

static DEFAULT_BODY: AtomicU64 = AtomicU64::new(16 << 20);
static DEFAULT_HEADER: AtomicU64 = AtomicU64::new(64 << 10);
static DEFAULT_IN_FLIGHT: AtomicU64 = AtomicU64::new(1024);
/// Bytes of a streamed reply that may wait for a slow client before the
/// streaming task waits (`[runtime] http_max_stream_buffer_bytes`).
static STREAM_BUFFER: AtomicU64 = AtomicU64::new(1 << 20);

/// Runtime configuration defaults (`[runtime] http_max_body_bytes`,
/// `http_max_header_bytes`, `http_max_in_flight`).
pub fn set_default_limit(key: &str, v: u64) -> bool {
    match key {
        "http_max_body_bytes" => DEFAULT_BODY.store(v, Ordering::SeqCst),
        "http_max_header_bytes" => DEFAULT_HEADER.store(v, Ordering::SeqCst),
        "http_max_in_flight" => DEFAULT_IN_FLIGHT.store(v.max(1), Ordering::SeqCst),
        "http_max_stream_buffer_bytes" => STREAM_BUFFER.store(v.max(1), Ordering::SeqCst),
        _ => return false,
    }
    true
}

pub fn default_limits() -> Limits {
    Limits {
        max_body_bytes: DEFAULT_BODY.load(Ordering::SeqCst) as usize,
        max_header_bytes: DEFAULT_HEADER.load(Ordering::SeqCst) as usize,
        max_in_flight: DEFAULT_IN_FLIGHT.load(Ordering::SeqCst) as usize,
    }
}

struct Registry {
    shutdown_waiters: Vec<TaskId>,
    signal_installed: bool,
}

struct RegCell(UnsafeCell<Option<Registry>>);
unsafe impl Sync for RegCell {}
static REG: RegCell = RegCell(UnsafeCell::new(None));

fn reg() -> &'static mut Registry {
    unsafe {
        (*REG.0.get()).get_or_insert_with(|| Registry { shutdown_waiters: vec![], signal_installed: false })
    }
}

macro_rules! http_intrinsics {
    ($( fn $name:ident($ctx:ident, $targs:ident, $ret:ident $(, $arg:ident)*) $body:block )*) => {
        $(
            #[allow(unused_variables)]
            pub extern "C" fn $name($ctx: *mut TaskCtx, $targs: Targs, $ret: *const TypeDesc $(, $arg: u64)*) -> u64 $body
        )*
        pub fn symbols() -> Vec<(&'static str, *const u8)> {
            vec![$((concat!("lang_rt_i_", stringify!($name)), $name as *const u8)),*]
        }
    };
}

http_intrinsics! {
    fn http_default_limits(ctx, targs, ret) {
        let lim = default_limits();
        make_record(rdesc(ret), &[
            ("max_body_bytes", lim.max_body_bytes as u64),
            ("max_header_bytes", lim.max_header_bytes as u64),
            ("max_in_flight", lim.max_in_flight as u64),
        ])
    }
    fn http_stream_buffer_default(ctx, targs, ret) { STREAM_BUFFER.load(Ordering::SeqCst) }
    // Park until SIGINT/SIGTERM (or `LANG_SHUTDOWN_AFTER_MS` elapses).
    fn sys_wait_shutdown(ctx, targs, ret) {
        install_signals();
        let r = reg();
        r.shutdown_waiters.push(task::current_id());
        task::set_listener(1);
        if let Some(ms) = std::env::var("LANG_SHUTDOWN_AFTER_MS").ok().and_then(|v| v.parse::<u64>().ok()) {
            task::sleep(Duration::from_millis(ms));
        } else {
            while !SIGNALLED.load(Ordering::SeqCst) {
                task::park_for("shutdown signal");
            }
        }
        task::set_listener(-1);
        0
    }
    fn http_body_text(ctx, targs, ret, b) { new_string(&String::from_utf8_lossy(bytes_of(b))) }
    // running-system registration for inspection
    fn sys_started(ctx, targs, ret, labels, providers) {
        let nodes = (0..list::len(labels))
            .map(|i| (str_of(list::get(labels, i)).to_string(), str_of(list::get(providers, i)).to_string()))
            .collect();
        crate::inspect::system_started(nodes)
    }
    fn sys_stopping(ctx, targs, ret, id, done) {
        crate::inspect::system_stopping(id, done & 1 == 1);
        0
    }
}

static SIGNALLED: AtomicBool = AtomicBool::new(false);
static RAISED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_: libc::c_int) {
    RAISED.store(true, Ordering::SeqCst);
}

fn install_signals() {
    let r = reg();
    if r.signal_installed {
        return;
    }
    r.signal_installed = true;
    unsafe {
        libc::signal(libc::SIGINT, on_signal as *const () as libc::sighandler_t);
        libc::signal(libc::SIGTERM, on_signal as *const () as libc::sighandler_t);
    }
    let q = task::rt().external.clone();
    std::thread::spawn(move || {
        loop {
            if RAISED.load(Ordering::SeqCst) {
                task::post(
                    &q,
                    task::External::Callback(Box::new(|| {
                        SIGNALLED.store(true, Ordering::SeqCst);
                        for w in std::mem::take(&mut reg().shutdown_waiters) {
                            task::wake(w, task::Wake::Normal);
                        }
                    })),
                );
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    });
}
