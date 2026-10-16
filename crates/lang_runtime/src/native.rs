//! Native modules (`extern "native NAME" fn`): finding and loading them,
//! calling their functions, and converting values to and from the wire
//! format (`lang_native`) by the declared types.
//!
//! A module is linked into the executable ([`register_builtin`]) or loaded
//! from `libNAME.so` in a directory of `LANG_NATIVE_PATH`, the executable's
//! directory, or `lib/lang` next to it. Calls pass owned bytes, so module
//! code never touches the heap: an inline function runs on the worker
//! without the runtime lock, a blocking one on the blocking pool while the
//! task waits (shielded, as for `extern "c blocking"`), and an async one
//! completes from any thread through the scheduler's external queue; a
//! cancelled task stops waiting for it (the module's `cancel` hook runs and
//! the late completion is discarded). A module's panic, a malformed result,
//! or a result that does not fit the declared type is a defect of the
//! calling task, never undefined behavior.

use std::ffi::{CStr, CString, c_char};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use lang_native::{self as nv, Call, Entry, Host, Mode, Module, Reader, Writer, status, tag};
use rustc_hash::FxHashMap;

use crate::desc::{Kind, TypeDesc};
use crate::gc::{self, Header, Roots, obj};
use crate::intrinsics::{Targs, rdesc, targ};
use crate::string::{bytes_of, new_bytes, new_string, str_of};
use crate::task::{self, TaskCtx};
use crate::{enum_parts, list, make_enum, map};

static HOST: Host = Host { abi_version: nv::ABI_VERSION, args: host_args, complete: host_complete };

struct Loaded {
    name: String,
    module: &'static Module,
    fns: FxHashMap<String, (usize, Mode)>,
}

unsafe impl Send for Loaded {}
unsafe impl Sync for Loaded {}

static BUILTINS: Mutex<Vec<(String, Entry)>> = Mutex::new(Vec::new());

/// Make module `name` available without loading a library (linked into
/// the executable).
pub fn register_builtin(name: &str, entry: Entry) {
    BUILTINS.lock().unwrap().push((name.to_string(), entry));
}

type Loading = Result<&'static Loaded, String>;

fn modules() -> &'static Mutex<FxHashMap<String, Loading>> {
    static M: std::sync::OnceLock<Mutex<FxHashMap<String, Loading>>> = std::sync::OnceLock::new();
    M.get_or_init(|| Mutex::new(FxHashMap::default()))
}

/// Where libraries are searched, in order.
pub fn search_path() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> =
        std::env::var("LANG_NATIVE_PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.to_path_buf())) {
        v.push(dir.join("lib/lang"));
        v.push(dir.join("../lib/lang"));
        v.push(dir.clone());
        // cargo builds: libraries of the workspace stay in `deps`
        v.push(dir.join("deps"));
    }
    v
}

fn file_name(name: &str) -> String {
    if cfg!(target_os = "macos") { format!("lib{name}.dylib") } else { format!("lib{name}.so") }
}

fn load(name: &str) -> Loading {
    let mut g = modules().lock().unwrap();
    if let Some(r) = g.get(name) {
        return r.clone();
    }
    let r = open(name);
    g.insert(name.to_string(), r.clone());
    r
}

fn open(name: &str) -> Loading {
    let builtin = BUILTINS.lock().unwrap().iter().find(|(n, _)| n == name).map(|(_, e)| *e);
    let entry: Entry = match builtin {
        Some(e) => e,
        None => {
            let paths = search_path();
            let file =
                paths.iter().map(|d| d.join(file_name(name))).find(|p| p.is_file()).ok_or_else(|| {
                    let dirs: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
                    format!("native module `{name}` not found ({} in {})", file_name(name), dirs.join(", "))
                })?;
            let c = CString::new(file.display().to_string()).map_err(|_| "bad library path".to_string())?;
            let h = unsafe { libc::dlopen(c.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
            if h.is_null() {
                let e = unsafe { CStr::from_ptr(libc::dlerror()) }.to_string_lossy().into_owned();
                return Err(format!("cannot load native module `{name}` from {}: {e}", file.display()));
            }
            let sym = CString::new(nv::entry_symbol(name)).unwrap();
            let f = unsafe { libc::dlsym(h, sym.as_ptr()) };
            if f.is_null() {
                return Err(format!("{} does not export `{}`", file.display(), nv::entry_symbol(name)));
            }
            unsafe { std::mem::transmute::<*mut libc::c_void, Entry>(f) }
        }
    };
    let m = unsafe { entry(&HOST) };
    if m.is_null() {
        return Err(format!("native module `{name}` refused ABI version {}", nv::ABI_VERSION));
    }
    let module: &'static Module = unsafe { &*m };
    if module.abi_version != nv::ABI_VERSION {
        return Err(format!(
            "native module `{name}` has ABI version {}, not {}",
            module.abi_version,
            nv::ABI_VERSION
        ));
    }
    let mut fns = FxHashMap::default();
    for i in 0..module.nfns {
        let d = unsafe { &*module.fns.add(i) };
        let n = unsafe { CStr::from_ptr(d.name) }.to_string_lossy().into_owned();
        fns.insert(n, (i, d.mode));
    }
    Ok(Box::leak(Box::new(Loaded { name: name.to_string(), module, fns })))
}

/// A call site: its module and function index (by the site's immortal
/// `"module\0function"` string).
type Site = (&'static Loaded, usize, Mode);

fn site(s: u64) -> Result<Site, String> {
    static SITES: Mutex<Option<FxHashMap<u64, Result<Site, String>>>> = Mutex::new(None);
    if let Some(r) = SITES.lock().unwrap().get_or_insert_default().get(&s) {
        return r.clone();
    }
    let text = str_of(s);
    let (lib, func) = text.split_once('\0').unwrap_or((text, ""));
    let r = load(lib).and_then(|m| match m.fns.get(func) {
        Some(&(i, mode)) => Ok((m, i, mode)),
        None => Err(format!("native module `{lib}` has no function `{func}`")),
    });
    SITES.lock().unwrap().get_or_insert_default().insert(s, r.clone());
    r
}

fn defect(msg: String) -> ! {
    task::panic_current(msg, task::caller_location())
}

// ------------------------------------------------------------------ calls

const PENDING: u8 = 0;
const DONE: u8 = 1;
const ABANDONED: u8 = 2;

struct CallState {
    args: Vec<u8>,
    result: Mutex<Option<Vec<u8>>>,
    state: AtomicU8,
    /// Async calls: who to wake, and through which queue.
    waiter: Option<(task::TaskId, Arc<task::ExternalQueue>)>,
}

unsafe extern "C" fn host_args(call: *mut Call, len: *mut usize) -> *const u8 {
    let st = unsafe { &*(call as *const CallState) };
    unsafe { *len = st.args.len() };
    st.args.as_ptr()
}

unsafe extern "C" fn host_complete(call: *mut Call, result: *const u8, len: usize) {
    // the module's reference to the call ends here
    let st = unsafe { Arc::from_raw(call as *const CallState) };
    let bytes =
        if result.is_null() { vec![] } else { unsafe { std::slice::from_raw_parts(result, len) }.to_vec() };
    *st.result.lock().unwrap() = Some(bytes);
    if st.state.compare_exchange(PENDING, DONE, Ordering::AcqRel, Ordering::Acquire).is_ok()
        && let Some((t, q)) = &st.waiter
    {
        task::post_completion(q, *t, Box::new(()));
    }
}

/// Call `site` with `args` (of the declared types `targs`), returning a
/// value of type `ret`.
fn call(targs: Targs, ret: &'static TypeDesc, s: u64, args: &[u64]) -> u64 {
    let (m, index, mode) = site(s).unwrap_or_else(|e| defect(e));
    let what = || str_of(s).replace('\0', ".");
    let mut w = Writer::default();
    w.u8(tag::LIST);
    w.u32(args.len() as u32);
    for (i, a) in args.iter().enumerate() {
        if let Err(e) = encode(targ(targs, i), *a, &mut w, m) {
            defect(format!("argument {} of native `{}`: {e}", i + 1, what()));
        }
    }
    let st = Arc::new(CallState {
        args: w.buf,
        result: Mutex::new(None),
        state: AtomicU8::new(PENDING),
        waiter: if mode == Mode::Async {
            Some((task::current_id(), with_lock(task::external_queue)))
        } else {
            None
        },
    });
    let raw = Arc::into_raw(st.clone()) as *mut Call;
    let module = m.module;
    match mode {
        Mode::Inline => unsafe { (module.call)(&HOST, raw, index) },
        Mode::Blocking => {
            let raw = raw as usize;
            let module = module as *const Module as usize;
            with_lock(|| {
                task::run_blocking(move || unsafe {
                    let module = &*(module as *const Module);
                    (module.call)(&HOST, raw as *mut Call, index)
                })
            });
        }
        Mode::Async => with_lock(|| wait_async(&st, module, raw, index)),
    }
    let result = st.result.lock().unwrap().take();
    let Some(bytes) = result else {
        defect(format!("native `{}` returned without completing the call", what()));
    };
    let _r = Roots::new();
    match decode_result(ret, &bytes, m) {
        Ok(v) => v,
        Err(e) => defect(format!("native `{}`: {e}", what())),
    }
}

fn with_lock<R>(f: impl FnOnce() -> R) -> R {
    if task::holds_lock() {
        return f();
    }
    task::enter_runtime_for(task::reason::SAFE_END);
    let r = f();
    task::leave_runtime();
    r
}

/// Start an async call and wait for it (lock held). Cancelling the task
/// withdraws: the module's `cancel` hook runs and the completion, when it
/// comes, is discarded.
fn wait_async(st: &Arc<CallState>, module: &'static Module, raw: *mut Call, index: usize) {
    task::begin_external();
    let guard = {
        let st = st.clone();
        let cancel = module.cancel;
        let raw = raw as usize;
        task::push_cleanup(task::Cleanup::Native(Box::new(move |_| {
            if st.state.compare_exchange(PENDING, ABANDONED, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                task::withdraw_external();
                if let Some(c) = cancel {
                    unsafe { c(raw as *mut Call) };
                }
            }
        })))
    };
    if let Some(t) = task::current() {
        t.wait = "native call";
    }
    unsafe { (module.call)(&HOST, raw, index) };
    loop {
        if task::take_completion().is_some() {
            break;
        }
        task::park();
    }
    if let Some(t) = task::current() {
        t.wait = "";
    }
    task::remove_cleanup(guard);
}

macro_rules! native_calls {
    ($( $name:ident ($($a:ident),*) ),* $(,)?) => {
        $(
            #[allow(unused_variables)]
            pub extern "C" fn $name(_ctx: *mut TaskCtx, targs: Targs, ret: *const TypeDesc, site: u64 $(, $a: u64)*) -> u64 {
                call(targs, rdesc(ret), site, &[$($a),*])
            }
        )*
        pub fn symbols() -> Vec<(&'static str, *const u8)> {
            vec![$((concat!("lang_rt_i_", stringify!($name)), $name as *const u8)),*]
        }
    };
}

native_calls! {
    native_call0(),
    native_call1(a),
    native_call2(a, b),
    native_call3(a, b, c),
    native_call4(a, b, c, d),
    native_call5(a, b, c, d, e),
    native_call6(a, b, c, d, e, f),
    native_call7(a, b, c, d, e, f, g),
    native_call8(a, b, c, d, e, f, g, h),
}

// ------------------------------------------------------------------ handles

/// A module's handle as a managed value: the module and its id.
fn handle_desc() -> &'static TypeDesc {
    static D: std::sync::OnceLock<&'static TypeDesc> = std::sync::OnceLock::new();
    D.get_or_init(|| {
        TypeDesc::new("Handle", Kind::Opaque { trace: None, finalize: Some(finalize_handle), args: vec![] })
            .leak()
    })
}

fn is_handle_type(d: &TypeDesc) -> bool {
    d.name == "Handle" && matches!(d.kind, Kind::Opaque { .. })
}

fn new_handle(m: &'static Loaded, id: u64) -> u64 {
    let h = gc::alloc(handle_desc(), 16);
    unsafe {
        let p = obj(h).payload() as *mut u64;
        *p = m as *const Loaded as u64;
        *p.add(1) = id;
    }
    h
}

/// Handles the collector found unreachable, released off the collector
/// (a module's `drop_handle` may take its own locks or do I/O).
fn finalize_handle(o: *mut Header) {
    let (m, id) = unsafe {
        let p = (*o).payload() as *const u64;
        (*p as *const Loaded, *p.add(1))
    };
    let Some(drop) = (unsafe { (*m).module.drop_handle }) else { return };
    static DROPS: std::sync::OnceLock<Option<std::sync::mpsc::Sender<(usize, u64)>>> =
        std::sync::OnceLock::new();
    let tx = DROPS.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<(usize, u64)>();
        std::thread::Builder::new()
            .name("lang-native-drop".into())
            .spawn(move || {
                for (f, id) in rx {
                    let f: unsafe extern "C" fn(u64) = unsafe { std::mem::transmute(f) };
                    unsafe { f(id) };
                }
            })
            .ok()
            .map(|_| tx)
    });
    if let Some(tx) = tx {
        let _ = tx.send((drop as usize, id));
    }
}

// ------------------------------------------------------------------ to the wire

fn encode(d: &TypeDesc, v: u64, w: &mut Writer, m: &'static Loaded) -> Result<(), String> {
    if matches!(d.kind, Kind::Dynamic) && v != 0 {
        return encode(unsafe { obj(v).desc() }, v, w, m);
    }
    match &d.kind {
        Kind::Unit => w.u8(tag::UNIT),
        Kind::Bool => w.u8(if v & 1 != 0 { tag::TRUE } else { tag::FALSE }),
        Kind::Int { signed: false, bits } => {
            let mask = if *bits == 64 { u64::MAX } else { (1u64 << bits) - 1 };
            if *bits == 64 && v > i64::MAX as u64 {
                return Err(format!("{v} does not fit a signed 64-bit integer"));
            }
            w.u8(tag::INT);
            w.i64((v & mask) as i64);
        }
        Kind::Int { .. } | Kind::Duration | Kind::Instant | Kind::Timestamp => {
            w.u8(tag::INT);
            w.i64(crate::value::as_i64(d, v));
        }
        Kind::Float { .. } => {
            w.u8(tag::FLOAT);
            w.f64(crate::value::as_f64(d, v));
        }
        Kind::Char => {
            w.u8(tag::CHAR);
            w.u32(v as u32);
        }
        Kind::String => {
            w.u8(tag::STR);
            w.str(str_of(v));
        }
        Kind::Symbol => {
            w.u8(tag::SYMBOL);
            w.str(str_of(v));
        }
        Kind::Bytes => {
            w.u8(tag::BYTES);
            w.bytes(bytes_of(v));
        }
        Kind::Record { fields, tuple, .. } => {
            let p = unsafe { obj(v).payload() };
            w.u8(if *tuple { tag::LIST } else { tag::MAP });
            w.u32(fields.len() as u32);
            for f in fields {
                if !*tuple {
                    w.str(&f.name);
                }
                encode(f.desc, unsafe { f.repr.load(p.add(f.offset as usize)) }, w, m)?;
            }
        }
        Kind::Enum { variants, .. } => {
            let (t, vals) = enum_parts(v);
            let var = &variants[t as usize];
            if d.name == "Data" {
                return encode_data(v, w);
            }
            if d.name == "Option" {
                if var.fields.is_empty() {
                    w.u8(tag::NULL);
                } else {
                    encode(var.fields[0].desc, vals[0], w, m)?;
                }
                return Ok(());
            }
            w.u8(tag::VARIANT);
            w.str(&var.name);
            w.u32(var.fields.len() as u32);
            for (f, x) in var.fields.iter().zip(vals) {
                encode(f.desc, x, w, m)?;
            }
        }
        Kind::List { elem } => {
            let n = list::len(v);
            w.u8(tag::LIST);
            w.u32(n as u32);
            for i in 0..n {
                encode(elem, list::get(v, i), w, m)?;
            }
        }
        Kind::Set { elem } => {
            let es = map::entries(v);
            w.u8(tag::LIST);
            w.u32(es.len() as u32);
            for (k, _) in es {
                encode(elem, k, w, m)?;
            }
        }
        Kind::Map { key, val } if matches!(key.kind, Kind::String | Kind::Symbol) => {
            let es = map::entries(v);
            w.u8(tag::MAP);
            w.u32(es.len() as u32);
            for (k, x) in es {
                w.str(str_of(k));
                encode(val, x, w, m)?;
            }
        }
        Kind::Opaque { .. } if crate::buffer::array_elem(d).is_some() => {
            let elem = crate::buffer::array_elem(d).expect("array");
            let items = crate::buffer::items(v);
            w.u8(tag::LIST);
            w.u32(items.len() as u32);
            for x in items.iter() {
                encode(elem, *x, w, m)?;
            }
        }
        Kind::Opaque { .. } if is_handle_type(d) => {
            if !std::ptr::eq(unsafe { obj(v).desc() }, handle_desc()) {
                return Err("not a native handle".into());
            }
            let (owner, id) = unsafe {
                let p = obj(v).payload() as *const u64;
                (*p as *const Loaded, *p.add(1))
            };
            if !std::ptr::eq(owner, m) {
                let other = unsafe { &(*owner).name };
                return Err(format!("a handle of module `{other}` passed to module `{}`", m.name));
            }
            w.u8(tag::HANDLE);
            w.u64(id);
        }
        _ => return Err(format!("a value of type `{}` cannot cross a native boundary", d.name)),
    }
    Ok(())
}

/// A `Data` value as its natural wire value.
fn encode_data(v: u64, w: &mut Writer) -> Result<(), String> {
    let (t, vals) = enum_parts(v);
    let d = unsafe { obj(v).desc() };
    let name = d.variant(t).map(|x| x.name.as_str()).unwrap_or("");
    match name {
        "null" => w.u8(tag::NULL),
        "bool" => w.u8(if vals[0] & 1 != 0 { tag::TRUE } else { tag::FALSE }),
        "int" => {
            w.u8(tag::INT);
            w.i64(vals[0] as i64);
        }
        "float" => {
            w.u8(tag::FLOAT);
            w.f64(f64::from_bits(vals[0]));
        }
        "string" => {
            w.u8(tag::STR);
            w.str(str_of(vals[0]));
        }
        "list" => {
            let n = list::len(vals[0]);
            w.u8(tag::LIST);
            w.u32(n as u32);
            for i in 0..n {
                encode_data(list::get(vals[0], i), w)?;
            }
        }
        "map" => {
            let es = map::entries(vals[0]);
            w.u8(tag::MAP);
            w.u32(es.len() as u32);
            for (k, x) in es {
                w.str(str_of(k));
                encode_data(x, w)?;
            }
        }
        other => return Err(format!("`Data.{other}` cannot cross a native boundary")),
    }
    Ok(())
}

// ------------------------------------------------------------------ from the wire

fn decode_result(d: &'static TypeDesc, bytes: &[u8], m: &'static Loaded) -> Result<u64, String> {
    let mut r = Reader::new(bytes);
    match r.u8()? {
        status::VALUE => {}
        status::DEFECT => return Err(r.str()?.to_string()),
        s => return Err(format!("unknown result status {s}")),
    }
    let v = decode(d, &mut r, m)?;
    if !r.at_end() {
        return Err("trailing bytes after the result".into());
    }
    Ok(v)
}

fn root(w: u64) -> u64 {
    gc::push_root(w);
    w
}

/// The variant name and field count at a VARIANT tag.
fn peek_variant<'a>(r: &Reader<'a>) -> Option<&'a str> {
    let mut p = Reader { buf: r.buf, pos: r.pos };
    if p.u8().ok()? != tag::VARIANT {
        return None;
    }
    p.str().ok()
}

fn want(what: &str, d: &TypeDesc, t: u8) -> String {
    let got = match t {
        tag::UNIT => "unit",
        tag::NULL => "null",
        tag::FALSE | tag::TRUE => "a boolean",
        tag::INT => "an integer",
        tag::FLOAT => "a float",
        tag::CHAR => "a character",
        tag::STR => "a string",
        tag::SYMBOL => "a symbol",
        tag::BYTES => "bytes",
        tag::LIST => "a list",
        tag::MAP => "a map",
        tag::VARIANT => "a variant",
        tag::HANDLE => "a handle",
        _ => "malformed data",
    };
    format!("returned {got} where {what} of type `{}` was declared", d.name)
}

fn decode(d: &'static TypeDesc, r: &mut Reader<'_>, m: &'static Loaded) -> Result<u64, String> {
    // `err` where something else was declared: the module's failure
    if peek_variant(r) == Some("err")
        && !(matches!(d.kind, Kind::Enum { .. }) && d.variant_index("err").is_some())
    {
        let mut p = Reader { buf: r.buf, pos: r.pos };
        let msg = match nv::Value::decode(&mut p) {
            Ok(nv::Value::Variant(_, xs)) => xs
                .first()
                .map(|e| match (e.field("kind"), e.field("message")) {
                    (Some(nv::Value::Symbol(k) | nv::Value::Str(k)), Some(nv::Value::Str(msg))) => {
                        format!("{k}: {msg}")
                    }
                    _ => format!("{e:?}"),
                })
                .unwrap_or_default(),
            _ => "an error".into(),
        };
        return Err(format!("failed: {msg}"));
    }
    match &d.kind {
        Kind::Enum { variants, .. } => return decode_enum(d, variants, r, m),
        Kind::Opaque { .. } if is_handle_type(d) => {
            return match r.u8()? {
                tag::HANDLE => Ok(root(new_handle(m, r.u64()?))),
                t => Err(want("a handle", d, t)),
            };
        }
        _ => {}
    }
    let t = r.u8()?;
    Ok(match (&d.kind, t) {
        (Kind::Unit, tag::UNIT | tag::NULL) => 0,
        (Kind::Bool, tag::TRUE) => 1,
        (Kind::Bool, tag::FALSE) => 0,
        (Kind::Int { bits, signed }, tag::INT) => {
            let v = r.i64()?;
            let fits = match (bits, signed) {
                (64, true) => true,
                (64, false) => v >= 0,
                (b, true) => v >= -(1i64 << (b - 1)) && v < (1i64 << (b - 1)),
                (b, false) => v >= 0 && v < (1i64 << b),
            };
            if !fits {
                return Err(format!("{v} does not fit `{}`", d.name));
            }
            v as u64
        }
        (Kind::Duration | Kind::Instant | Kind::Timestamp, tag::INT) => r.i64()? as u64,
        (Kind::Float { bits }, tag::FLOAT | tag::INT) => {
            let f = if t == tag::INT { r.i64()? as f64 } else { r.f64()? };
            if *bits == 32 { (f as f32).to_bits() as u64 } else { f.to_bits() }
        }
        (Kind::Char, tag::CHAR) => r.u32()? as u64,
        (Kind::String, tag::STR | tag::SYMBOL) => root(new_string(r.str()?)),
        (Kind::Symbol, tag::SYMBOL | tag::STR) => crate::string::symbol(r.str()?),
        (Kind::Bytes, tag::BYTES) => root(new_bytes(r.bytes()?)),
        (Kind::Bytes, tag::STR) => root(new_bytes(r.str()?.as_bytes())),
        (Kind::Record { fields, tuple: true, size, .. }, tag::LIST) => {
            let n = r.count()?;
            if n != fields.len() {
                return Err(format!("expected {} elements for `{}`, got {n}", fields.len(), d.name));
            }
            let mut vals = vec![];
            for f in fields {
                vals.push(decode(f.desc, r, m)?);
            }
            record(d, *size, fields, vals)
        }
        (Kind::Record { fields, tuple: false, size, .. }, tag::MAP) => {
            let n = r.count()?;
            let mut got: Vec<Option<u64>> = vec![None; fields.len()];
            for _ in 0..n {
                let k = r.str()?;
                match fields.iter().position(|f| f.name == k) {
                    Some(i) => got[i] = Some(decode(fields[i].desc, r, m)?),
                    // a field the program does not declare is skipped
                    None => {
                        nv::Value::decode(r)?;
                    }
                }
            }
            let mut vals = vec![];
            for (f, v) in fields.iter().zip(got) {
                match v {
                    Some(v) => vals.push(v),
                    None if f.desc.name == "Option" => vals.push(root(make_enum(f.desc, 0, &[]))),
                    None => return Err(format!("missing field `{}` of `{}`", f.name, d.name)),
                }
            }
            record(d, *size, fields, vals)
        }
        (Kind::List { elem }, tag::LIST) => {
            let n = r.count()?;
            let l = root(list::new(d));
            for _ in 0..n {
                let v = decode(elem, r, m)?;
                list::push(l, v);
            }
            l
        }
        (Kind::Map { key, val }, tag::MAP) if matches!(key.kind, Kind::String | Kind::Symbol) => {
            let n = r.count()?;
            let out = root(map::new(d));
            for _ in 0..n {
                let k = r.str()?;
                let kw = if matches!(key.kind, Kind::Symbol) {
                    crate::string::symbol(k)
                } else {
                    root(new_string(k))
                };
                let vw = decode(val, r, m)?;
                map::insert_mut(out, kw, vw);
            }
            out
        }
        (Kind::Opaque { .. }, tag::LIST) if crate::buffer::array_elem(d).is_some() => {
            let elem = crate::buffer::array_elem(d).expect("array");
            let n = r.count()?;
            let mut items = Vec::with_capacity(n);
            for _ in 0..n {
                items.push(decode(elem, r, m)?);
            }
            root(crate::buffer::frozen(elem, items))
        }
        _ => return Err(want("a value", d, t)),
    })
}

fn record(d: &'static TypeDesc, size: u32, fields: &[crate::desc::Field], vals: Vec<u64>) -> u64 {
    let w = root(gc::alloc(d, size as usize));
    let pl = unsafe { obj(w).payload() };
    for (f, v) in fields.iter().zip(vals) {
        unsafe { f.repr.store(pl.add(f.offset as usize), v) };
    }
    w
}

fn decode_enum(
    d: &'static TypeDesc,
    variants: &[crate::desc::Variant],
    r: &mut Reader<'_>,
    m: &'static Loaded,
) -> Result<u64, String> {
    if d.name == "Data" {
        return decode_data(d, r);
    }
    let named = peek_variant(r).and_then(|n| d.variant_index(n));
    let Some(i) = named else {
        // a plain value where an `Option` or `Result` was declared
        let wrap = match d.name.as_str() {
            "Option" => {
                let mut p = Reader { buf: r.buf, pos: r.pos };
                if matches!(p.u8()?, tag::NULL | tag::UNIT) {
                    r.u8()?;
                    return Ok(root(make_enum(d, 0, &[])));
                }
                d.variant_index("some")
            }
            "Result" => d.variant_index("ok"),
            _ => None,
        };
        let Some(i) = wrap else {
            let mut p = Reader { buf: r.buf, pos: r.pos };
            let t = p.u8()?;
            return Err(want("a variant", d, t));
        };
        let v = decode(variants[i as usize].fields[0].desc, r, m)?;
        return Ok(root(make_enum(d, i, &[v])));
    };
    r.u8()?;
    r.str()?;
    let n = r.count()?;
    let var = &variants[i as usize];
    if n != var.fields.len() {
        return Err(format!("`{}.{}` takes {} fields, got {n}", d.name, var.name, var.fields.len()));
    }
    let mut vals = vec![];
    for f in &var.fields {
        vals.push(decode(f.desc, r, m)?);
    }
    Ok(root(make_enum(d, i, &vals)))
}

fn decode_data(d: &'static TypeDesc, r: &mut Reader<'_>) -> Result<u64, String> {
    let v = nv::Value::decode(r)?;
    Ok(root(data_value(d, &v)?))
}

fn data_value(d: &'static TypeDesc, v: &nv::Value) -> Result<u64, String> {
    use nv::Value as V;
    let rt = Roots::new();
    Ok(match v {
        V::Null | V::Unit => make_enum(d, 0, &[]),
        V::Bool(b) => make_enum(d, 1, &[*b as u64]),
        V::Int(i) => make_enum(d, 2, &[*i as u64]),
        V::Float(f) => make_enum(d, 3, &[f.to_bits()]),
        V::Str(s) | V::Symbol(s) => {
            let w = rt.add(new_string(s));
            make_enum(d, 4, &[w])
        }
        V::Char(c) => {
            let w = rt.add(new_string(&c.to_string()));
            make_enum(d, 4, &[w])
        }
        V::Bytes(b) => {
            let w = rt.add(new_string(&String::from_utf8_lossy(b)));
            make_enum(d, 4, &[w])
        }
        V::List(xs) => {
            let list_desc = d.variant(5).unwrap().fields[0].desc;
            let l = rt.add(list::new(list_desc));
            for x in xs {
                let w = rt.add(data_value(d, x)?);
                list::push(l, w);
            }
            make_enum(d, 5, &[l])
        }
        V::Map(fs) => {
            let map_desc = d.variant(6).unwrap().fields[0].desc;
            let mw = rt.add(map::new(map_desc));
            for (k, x) in fs {
                let kw = rt.add(new_string(k));
                let vw = rt.add(data_value(d, x)?);
                map::insert_mut(mw, kw, vw);
            }
            make_enum(d, 6, &[mw])
        }
        V::Variant(..) | V::Handle(_) => return Err(format!("{v:?} is not `Data`")),
    })
}

/// For inspection: the modules loaded so far and where libraries are
/// searched.
pub fn describe() -> String {
    let g = modules().lock().unwrap();
    let mut names: Vec<String> = g
        .iter()
        .map(|(n, r)| match r {
            Ok(m) => format!("{n} ({} functions)", m.fns.len()),
            Err(e) => format!("{n}: {e}"),
        })
        .collect();
    names.sort();
    names.join("\n")
}

#[allow(dead_code)]
fn c_name(p: *const c_char) -> String {
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}
