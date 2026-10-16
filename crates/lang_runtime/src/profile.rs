//! Sampling profiler and JIT symbol registry.
//!
//! `LANG_PROFILE=<file>` samples every running thread at 1 kHz of CPU time
//! (`SIGPROF`). The handler records the interrupted program counter and the
//! frame-pointer chain above it; frames are read with `process_vm_readv`,
//! so a broken chain ends the walk instead of faulting. At exit the samples
//! are symbolized (generated code through the registry below, native code
//! through the binary's debug info) and written as collapsed stacks, one
//! `outer;...;leaf count` line per distinct stack: the input format of
//! flamegraph tools and speedscope. A short self/total summary goes to
//! stderr. Build with `RUSTFLAGS="-C force-frame-pointers=yes"` for callers
//! of runtime functions; generated code always keeps frame pointers.
//!
//! `LANG_PERF_MAP=1` writes `/tmp/perf-<pid>.map`, which lets `perf`
//! attribute samples in generated code.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};

const DEPTH: usize = 64;
const CAPACITY: usize = 1 << 18;

struct Code {
    start: usize,
    len: usize,
    name: String,
}

static CODE: Mutex<Vec<Code>> = Mutex::new(Vec::new());

/// Record a generated function's code range (for profiles and perf maps).
pub fn register_code(start: *const u8, len: usize, name: &str) {
    if std::env::var_os("LANG_PERF_MAP").is_some() {
        use std::io::Write;
        let path = format!("/tmp/perf-{}.map", std::process::id());
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "{:x} {:x} {name}", start as usize, len);
        }
    }
    CODE.lock().unwrap().push(Code { start: start as usize, len, name: name.to_string() });
}

/// Name of the generated function containing `pc`, if any.
pub fn code_name(pc: usize) -> Option<String> {
    let code = CODE.lock().unwrap();
    code.iter().find(|c| pc >= c.start && pc < c.start + c.len).map(|c| c.name.clone())
}

// one sample: frame count, then frames (leaf first)
type Sample = [usize; DEPTH + 1];

static SAMPLES: AtomicPtr<Sample> = AtomicPtr::new(std::ptr::null_mut());
static NEXT: AtomicUsize = AtomicUsize::new(0);
static STARTED: AtomicBool = AtomicBool::new(false);
static OUT: Mutex<Option<String>> = Mutex::new(None);

/// Start sampling if `LANG_PROFILE` is set (once per process; the profile
/// is written at exit).
pub fn start_from_env() {
    let Some(path) = std::env::var_os("LANG_PROFILE") else { return };
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    *OUT.lock().unwrap() = Some(path.to_string_lossy().into_owned());
    let buf: Box<[Sample]> = vec![[0usize; DEPTH + 1]; CAPACITY].into_boxed_slice();
    SAMPLES.store(Box::leak(buf).as_mut_ptr(), Ordering::SeqCst);
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = on_sample as *const () as usize;
        sa.sa_flags = libc::SA_SIGINFO | libc::SA_RESTART;
        libc::sigemptyset(&mut sa.sa_mask);
        libc::sigaction(libc::SIGPROF, &sa, std::ptr::null_mut());
        let tick = libc::timeval { tv_sec: 0, tv_usec: 1000 };
        let it = libc::itimerval { it_interval: tick, it_value: tick };
        libc::setitimer(libc::ITIMER_PROF, &it, std::ptr::null_mut());
        libc::atexit(finish_at_exit);
    }
}

/// Read one word of this process's memory; `None` if it is not mapped.
fn read_word(addr: usize) -> Option<usize> {
    let mut out = 0usize;
    let local = libc::iovec { iov_base: &mut out as *mut usize as *mut libc::c_void, iov_len: 8 };
    let remote = libc::iovec { iov_base: addr as *mut libc::c_void, iov_len: 8 };
    let n = unsafe { libc::process_vm_readv(libc::getpid(), &local, 1, &remote, 1, 0) };
    (n == 8).then_some(out)
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
unsafe fn registers(uc: *mut libc::c_void) -> (usize, usize) {
    let uc = uc as *mut libc::ucontext_t;
    let g = unsafe { &(*uc).uc_mcontext.gregs };
    (g[libc::REG_RIP as usize] as usize, g[libc::REG_RBP as usize] as usize)
}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
unsafe fn registers(uc: *mut libc::c_void) -> (usize, usize) {
    let uc = uc as *mut libc::ucontext_t;
    let m = unsafe { &(*uc).uc_mcontext };
    (m.pc as usize, m.regs[29] as usize)
}

#[cfg(not(all(target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64"))))]
unsafe fn registers(_uc: *mut libc::c_void) -> (usize, usize) {
    (0, 0)
}

extern "C" fn on_sample(_sig: libc::c_int, _info: *mut libc::siginfo_t, uc: *mut libc::c_void) {
    let errno = unsafe { *libc::__errno_location() };
    let base = SAMPLES.load(Ordering::Relaxed);
    let i = NEXT.fetch_add(1, Ordering::Relaxed);
    if base.is_null() || i >= CAPACITY {
        return;
    }
    let s = unsafe { &mut *base.add(i) };
    let (pc, mut fp) = unsafe { registers(uc) };
    let mut n = 0;
    if pc != 0 {
        s[1] = pc;
        n = 1;
    }
    // walk [fp] = caller's fp, [fp + 8] = return address
    while n < DEPTH && fp != 0 && fp % 8 == 0 {
        let (Some(next), Some(ret)) = (read_word(fp), read_word(fp + 8)) else { break };
        if ret == 0 {
            break;
        }
        s[1 + n] = ret - 1;
        n += 1;
        if next <= fp || next - fp > (64 << 20) {
            break;
        }
        fp = next;
    }
    s[0] = n;
    unsafe { *libc::__errno_location() = errno };
}

extern "C" fn finish_at_exit() {
    finish();
}

/// Stop sampling and write the profile (idempotent).
pub fn finish() {
    let Some(path) = OUT.lock().unwrap().take() else { return };
    unsafe {
        let zero = libc::timeval { tv_sec: 0, tv_usec: 0 };
        let it = libc::itimerval { it_interval: zero, it_value: zero };
        libc::setitimer(libc::ITIMER_PROF, &it, std::ptr::null_mut());
    }
    let base = SAMPLES.load(Ordering::SeqCst);
    let taken = NEXT.load(Ordering::SeqCst).min(CAPACITY);
    let samples = unsafe { std::slice::from_raw_parts(base, taken) };
    let mut names: rustc_hash::FxHashMap<usize, String> = Default::default();
    let mut stacks: rustc_hash::FxHashMap<Vec<String>, usize> = Default::default();
    let mut own: rustc_hash::FxHashMap<String, usize> = Default::default();
    let mut total: rustc_hash::FxHashMap<String, usize> = Default::default();
    for s in samples {
        let n = s[0];
        if n == 0 {
            continue;
        }
        let mut frames: Vec<String> =
            s[1..=n].iter().map(|&pc| names.entry(pc).or_insert_with(|| symbolize(pc)).clone()).collect();
        frames.dedup();
        *own.entry(frames[0].clone()).or_default() += 1;
        let mut seen = rustc_hash::FxHashSet::default();
        for f in &frames {
            if seen.insert(f.clone()) {
                *total.entry(f.clone()).or_default() += 1;
            }
        }
        frames.reverse();
        *stacks.entry(frames).or_default() += 1;
    }
    let mut lines: Vec<String> = stacks.iter().map(|(k, v)| format!("{} {v}", k.join(";"))).collect();
    lines.sort();
    if let Err(e) = std::fs::write(&path, lines.join("\n") + "\n") {
        eprintln!("[profile] cannot write {path}: {e}");
        return;
    }
    let count = samples.iter().filter(|s| s[0] > 0).count().max(1);
    let top = |m: &rustc_hash::FxHashMap<String, usize>, title: &str| {
        let mut v: Vec<(&String, &usize)> = m.iter().collect();
        v.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        eprintln!("[profile] {title}");
        for (name, c) in v.into_iter().take(20) {
            eprintln!("[profile] {:6.2}% {:6} {name}", *c as f64 * 100.0 / count as f64, c);
        }
    };
    eprintln!("[profile] {count} samples (1 ms of CPU each) -> {path}");
    top(&own, "self");
    top(&total, "total");
}

fn symbolize(pc: usize) -> String {
    if let Some(n) = code_name(pc) {
        return n;
    }
    let mut out = None;
    backtrace::resolve(pc as *mut libc::c_void, |sym| {
        if out.is_none()
            && let Some(n) = sym.name()
        {
            out = Some(format!("{n:#}"));
        }
    });
    out.unwrap_or_else(|| mapping_of(pc).unwrap_or_else(|| format!("{pc:#x}")))
}

/// `[file+offset]` for an address in a mapped file without symbols.
fn mapping_of(pc: usize) -> Option<String> {
    let maps = std::fs::read_to_string("/proc/self/maps").ok()?;
    maps.lines().find_map(|l| {
        let mut parts = l.split_whitespace();
        let (lo, hi) = parts.next()?.split_once('-')?;
        let (lo, hi) = (usize::from_str_radix(lo, 16).ok()?, usize::from_str_radix(hi, 16).ok()?);
        if pc < lo || pc >= hi {
            return None;
        }
        let off = usize::from_str_radix(parts.nth(1)?, 16).ok()?;
        let file = parts.nth(2).unwrap_or("anonymous");
        let file = file.rsplit('/').next().unwrap_or(file);
        Some(format!("[{file}+{:#x}]", pc - lo + off))
    })
}
