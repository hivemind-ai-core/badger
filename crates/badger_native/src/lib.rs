//! Native modules: functions written in Rust (or anything with a C ABI)
//! that programs call through `extern "native NAME" fn` declarations.
//!
//! A module is a shared library (`libNAME.so`) exporting
//! `badger_native_v1_NAME`, which returns a [`Module`]: its functions, each
//! with a [`Mode`], and one dispatcher. The host (the language runtime)
//! encodes a call's arguments in the [wire format](Value::encode), calls
//! the dispatcher, and receives the encoded result through
//! [`Host::complete`], exactly once, from any thread. Nothing managed
//! crosses the boundary: arguments and results are owned bytes, so a module
//! never sees the collector, a task, or a borrowed value.
//!
//! Modes:
//! - [`Mode::Inline`]: runs on the calling worker and must return promptly;
//! - [`Mode::Blocking`]: runs on the runtime's blocking pool while the
//!   calling task waits (cancellation waits for it to finish);
//! - [`Mode::Async`]: the dispatcher returns at once and the module
//!   completes the call later from any thread (an I/O event loop, say);
//!   cancelling the waiting task calls the module's `cancel` hook, and a
//!   completion after that is discarded.
//!
//! Handles ([`Value::Handle`]) name module-owned state (a connection, a
//! server); the program holds them as `native.Handle` values, and the host
//! calls the module's `drop_handle` once one is unreachable.
//!
//! Write a module with [`ModuleDef`] and [`export!`]:
//!
//! ```ignore
//! fn add(args: &Args) -> Outcome { Ok(Value::Int(args.int(0)? + args.int(1)?)) }
//! static DEF: ModuleDef = ModuleDef {
//!     name: "demo",
//!     functions: &[("add", Handler::Inline(add))],
//!     drop_handle: None,
//!     cancel: None,
//!     recorded: &[],
//!     pure: &["add"],
//! };
//! badger_native::export!("demo", DEF);
//! ```

use std::ffi::{CString, c_char};
use std::sync::{Arc, Mutex, OnceLock};

/// The ABI version a module and its host must agree on.
pub const ABI_VERSION: u32 = 1;

/// The symbol a library named `name` exports.
pub fn entry_symbol(name: &str) -> String {
    format!("badger_native_v1_{name}")
}

// ------------------------------------------------------------------ wire

/// Wire tags: each value is a tag byte and its payload (integers are
/// little-endian; lengths and counts are `u32`).
pub mod tag {
    pub const UNIT: u8 = 0;
    pub const NULL: u8 = 1;
    pub const FALSE: u8 = 2;
    pub const TRUE: u8 = 3;
    pub const INT: u8 = 4;
    pub const FLOAT: u8 = 5;
    pub const CHAR: u8 = 6;
    pub const STR: u8 = 7;
    pub const SYMBOL: u8 = 8;
    pub const BYTES: u8 = 9;
    pub const LIST: u8 = 10;
    pub const MAP: u8 = 11;
    pub const VARIANT: u8 = 12;
    pub const HANDLE: u8 = 13;
}

/// A call's result starts with one of these, then the value or the
/// defect's message.
pub mod status {
    pub const VALUE: u8 = 0;
    /// The module failed (a panic, a broken invariant): the calling task
    /// fails with this message, as a defect.
    pub const DEFECT: u8 = 1;
}

/// A value on the wire. Declared types decide how the host reads it:
/// records and structs are [`Value::Map`]s by field name, enum values
/// (including `Option` and `Result`) are [`Value::Variant`]s by variant
/// name, `Data` is any value, and a value returned where a `Result` is
/// declared is its `ok` (likewise `some` for an `Option`, with
/// [`Value::Null`] or [`Value::Unit`] for `none`).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Unit,
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Char(char),
    Str(String),
    Symbol(String),
    Bytes(Vec<u8>),
    List(Vec<Value>),
    Map(Vec<(String, Value)>),
    Variant(String, Vec<Value>),
    Handle(u64),
}

impl Value {
    pub fn str(s: impl Into<String>) -> Value {
        Value::Str(s.into())
    }

    pub fn ok(v: Value) -> Value {
        Value::Variant("ok".into(), vec![v])
    }

    pub fn some(v: Value) -> Value {
        Value::Variant("some".into(), vec![v])
    }

    pub fn none() -> Value {
        Value::Variant("none".into(), vec![])
    }

    /// The field `name` of a map.
    pub fn field(&self, name: &str) -> Option<&Value> {
        match self {
            Value::Map(fs) => fs.iter().find(|(k, _)| k == name).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn encode(&self, w: &mut Writer) {
        match self {
            Value::Unit => w.u8(tag::UNIT),
            Value::Null => w.u8(tag::NULL),
            Value::Bool(false) => w.u8(tag::FALSE),
            Value::Bool(true) => w.u8(tag::TRUE),
            Value::Int(i) => {
                w.u8(tag::INT);
                w.i64(*i);
            }
            Value::Float(f) => {
                w.u8(tag::FLOAT);
                w.f64(*f);
            }
            Value::Char(c) => {
                w.u8(tag::CHAR);
                w.u32(*c as u32);
            }
            Value::Str(s) => {
                w.u8(tag::STR);
                w.str(s);
            }
            Value::Symbol(s) => {
                w.u8(tag::SYMBOL);
                w.str(s);
            }
            Value::Bytes(b) => {
                w.u8(tag::BYTES);
                w.bytes(b);
            }
            Value::List(xs) => {
                w.u8(tag::LIST);
                w.count(xs.len());
                xs.iter().for_each(|x| x.encode(w));
            }
            Value::Map(fs) => {
                w.u8(tag::MAP);
                w.count(fs.len());
                for (k, v) in fs {
                    w.str(k);
                    v.encode(w);
                }
            }
            Value::Variant(name, xs) => {
                w.u8(tag::VARIANT);
                w.str(name);
                w.count(xs.len());
                xs.iter().for_each(|x| x.encode(w));
            }
            Value::Handle(h) => {
                w.u8(tag::HANDLE);
                w.u64(*h);
            }
        }
    }

    pub fn decode(r: &mut Reader<'_>) -> Result<Value, String> {
        Ok(match r.u8()? {
            tag::UNIT => Value::Unit,
            tag::NULL => Value::Null,
            tag::FALSE => Value::Bool(false),
            tag::TRUE => Value::Bool(true),
            tag::INT => Value::Int(r.i64()?),
            tag::FLOAT => Value::Float(r.f64()?),
            tag::CHAR => Value::Char(char::from_u32(r.u32()?).ok_or("invalid char")?),
            tag::STR => Value::Str(r.str()?.to_string()),
            tag::SYMBOL => Value::Symbol(r.str()?.to_string()),
            tag::BYTES => Value::Bytes(r.bytes()?.to_vec()),
            tag::LIST => {
                let n = r.count()?;
                let mut xs = Vec::with_capacity(n.min(1 << 16));
                for _ in 0..n {
                    xs.push(Value::decode(r)?);
                }
                Value::List(xs)
            }
            tag::MAP => {
                let n = r.count()?;
                let mut fs = Vec::with_capacity(n.min(1 << 16));
                for _ in 0..n {
                    let k = r.str()?.to_string();
                    fs.push((k, Value::decode(r)?));
                }
                Value::Map(fs)
            }
            tag::VARIANT => {
                let name = r.str()?.to_string();
                let n = r.count()?;
                let mut xs = Vec::with_capacity(n.min(1 << 16));
                for _ in 0..n {
                    xs.push(Value::decode(r)?);
                }
                Value::Variant(name, xs)
            }
            tag::HANDLE => Value::Handle(r.u64()?),
            t => return Err(format!("unknown wire tag {t}")),
        })
    }
}

/// Appends wire data.
#[derive(Default)]
pub struct Writer {
    pub buf: Vec<u8>,
}

impl Writer {
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i64(&mut self, v: i64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn f64(&mut self, v: f64) {
        self.buf.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    pub fn bytes(&mut self, b: &[u8]) {
        self.count(b.len());
        self.buf.extend_from_slice(b);
    }
    /// A length or count (the wire holds 32 bits: a value of 4 GiB, or
    /// 2^32 items, cannot cross; in a module that is a defect, not a
    /// silently truncated value).
    pub fn count(&mut self, n: usize) {
        let n = u32::try_from(n).unwrap_or_else(|_| panic!("a value of {n} bytes or items is too large to cross the native module boundary (limit 2^32 - 1)"));
        self.u32(n);
    }
    pub fn str(&mut self, s: &str) {
        self.bytes(s.as_bytes());
    }
}

/// Reads wire data, failing (not panicking) on malformed input.
pub struct Reader<'a> {
    pub buf: &'a [u8],
    pub pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Reader<'a> {
        Reader { buf, pos: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.pos.checked_add(n).filter(|e| *e <= self.buf.len()).ok_or("truncated wire data")?;
        let s = &self.buf[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    pub fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    pub fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn i64(&mut self) -> Result<i64, String> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn f64(&mut self) -> Result<f64, String> {
        Ok(f64::from_bits(self.u64()?))
    }
    /// A count of items that follow (each at least one byte).
    pub fn count(&mut self) -> Result<usize, String> {
        let n = self.u32()? as usize;
        if n > self.buf.len() - self.pos {
            return Err("truncated wire data".into());
        }
        Ok(n)
    }
    pub fn bytes(&mut self) -> Result<&'a [u8], String> {
        let n = self.u32()? as usize;
        self.take(n)
    }
    pub fn str(&mut self) -> Result<&'a str, String> {
        std::str::from_utf8(self.bytes()?).map_err(|_| "invalid UTF-8 in wire data".to_string())
    }
    pub fn at_end(&self) -> bool {
        self.pos == self.buf.len()
    }
}

// ------------------------------------------------------------------ C ABI

/// [`FnDesc::record`] of a function without effects outside the call.
pub const PURE: u32 = u32::MAX;

/// How the host runs a function.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Inline = 0,
    Blocking = 1,
    Async = 2,
}

/// One call in progress (opaque; owned by the host).
#[repr(C)]
pub struct Call {
    _private: [u8; 0],
}

/// What the host gives a module.
#[repr(C)]
pub struct Host {
    pub abi_version: u32,
    /// The call's encoded arguments (a list value), valid until it
    /// completes.
    pub args: unsafe extern "C" fn(call: *mut Call, len: *mut usize) -> *const u8,
    /// Complete the call with its encoded result (a status byte, then the
    /// value or a defect message). Exactly once per call, from any thread;
    /// the call must not be used afterwards.
    pub complete: unsafe extern "C" fn(call: *mut Call, result: *const u8, len: usize),
}

#[repr(C)]
pub struct FnDesc {
    pub name: *const c_char,
    pub mode: Mode,
    /// 0: the call's effects are not captured by reproduction bundles (a
    /// replay that reaches it stops). `k + 1`: the call is an external
    /// operation identified by its first `k` arguments (the rest are what
    /// it sends); a recording keeps its result and a replay answers it
    /// from there, as for the runtime's own HTTP requests and file reads.
    /// [`PURE`]: the call has no effect outside it (its result depends only
    /// on its arguments), so recordings and replays simply run it.
    pub record: u32,
    /// The kind of operation a recorded call is filed under in bundles
    /// (`fetch`), or null for `native MODULE.FUNCTION`.
    pub record_kind: *const c_char,
}

/// What a module gives the host.
#[repr(C)]
pub struct Module {
    pub abi_version: u32,
    pub name: *const c_char,
    pub fns: *const FnDesc,
    pub nfns: usize,
    /// Run function `index` for `call`.
    pub call: unsafe extern "C" fn(host: *const Host, call: *mut Call, index: usize),
    /// Release a handle the program can no longer reach.
    pub drop_handle: Option<unsafe extern "C" fn(handle: u64)>,
    /// The task waiting for an async call was cancelled: finish it soon
    /// (its completion is discarded).
    pub cancel: Option<unsafe extern "C" fn(call: *mut Call)>,
}

unsafe impl Sync for Module {}
unsafe impl Send for Module {}
unsafe impl Sync for FnDesc {}
unsafe impl Send for FnDesc {}

/// The entry point a library exports as `badger_native_v1_NAME`.
pub type Entry = unsafe extern "C" fn(host: *const Host) -> *const Module;

// ------------------------------------------------------------------ writing modules

/// An expected failure, returned to the program as `err` (or a defect
/// where the declared result is not a `Result`): a map with `kind`
/// (a symbol such as `:sqlite/constraint`, written without the colon) and
/// `message`.
#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    pub kind: String,
    pub message: String,
}

impl Error {
    pub fn new(kind: impl Into<String>, message: impl Into<String>) -> Error {
        Error { kind: kind.into(), message: message.into() }
    }

    pub fn value(&self) -> Value {
        Value::Variant(
            "err".into(),
            vec![Value::Map(vec![
                ("kind".into(), Value::Symbol(self.kind.clone())),
                ("message".into(), Value::Str(self.message.clone())),
            ])],
        )
    }
}

/// A function's result: a value, or an expected failure.
pub type Outcome = Result<Value, Error>;

/// A call's arguments, with typed accessors that fail with an
/// `:native/argument` error.
pub struct Args {
    pub values: Vec<Value>,
}

fn arg_err(i: usize, want: &str, got: Option<&Value>) -> Error {
    Error::new("native/argument", format!("argument {i}: expected {want}, got {got:?}"))
}

impl Args {
    pub fn len(&self) -> usize {
        self.values.len()
    }
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
    pub fn get(&self, i: usize) -> Option<&Value> {
        self.values.get(i)
    }
    pub fn int(&self, i: usize) -> Result<i64, Error> {
        match self.get(i) {
            Some(Value::Int(v)) => Ok(*v),
            v => Err(arg_err(i, "an integer", v)),
        }
    }
    pub fn float(&self, i: usize) -> Result<f64, Error> {
        match self.get(i) {
            Some(Value::Float(v)) => Ok(*v),
            v => Err(arg_err(i, "a float", v)),
        }
    }
    pub fn bool(&self, i: usize) -> Result<bool, Error> {
        match self.get(i) {
            Some(Value::Bool(v)) => Ok(*v),
            v => Err(arg_err(i, "a boolean", v)),
        }
    }
    pub fn str(&self, i: usize) -> Result<&str, Error> {
        match self.get(i) {
            Some(Value::Str(v) | Value::Symbol(v)) => Ok(v),
            v => Err(arg_err(i, "a string", v)),
        }
    }
    pub fn bytes(&self, i: usize) -> Result<&[u8], Error> {
        match self.get(i) {
            Some(Value::Bytes(v)) => Ok(v),
            v => Err(arg_err(i, "bytes", v)),
        }
    }
    pub fn list(&self, i: usize) -> Result<&[Value], Error> {
        match self.get(i) {
            Some(Value::List(v)) => Ok(v),
            v => Err(arg_err(i, "a list", v)),
        }
    }
    pub fn handle(&self, i: usize) -> Result<u64, Error> {
        match self.get(i) {
            Some(Value::Handle(v)) => Ok(*v),
            v => Err(arg_err(i, "a handle", v)),
        }
    }
}

/// Completes an async call once, from any thread. Dropping it without
/// completing fails the call with a defect.
pub struct Completion {
    host: *const Host,
    call: *mut Call,
    done: bool,
}

unsafe impl Send for Completion {}

impl Completion {
    /// The call this completes (to match a `cancel` hook's argument).
    pub fn id(&self) -> CallId {
        CallId(self.call as usize)
    }

    pub fn complete(mut self, outcome: Outcome) {
        self.done = true;
        finish(self.host, self.call, &encode_outcome(&outcome));
    }
}

impl Drop for Completion {
    fn drop(&mut self) {
        if !self.done {
            let why = if std::thread::panicking() {
                "the native module failed (it panicked) before completing the call"
            } else {
                "the module dropped an async call without completing it"
            };
            finish(self.host, self.call, &defect(why));
        }
    }
}

/// An async call's identity, as given to a module's `cancel` hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CallId(pub usize);

pub enum Handler {
    Inline(fn(&Args) -> Outcome),
    Blocking(fn(&Args) -> Outcome),
    Async(fn(Args, Completion)),
}

/// A module written in Rust; [`export!`] turns it into the C entry point.
pub struct ModuleDef {
    pub name: &'static str,
    pub functions: &'static [(&'static str, Handler)],
    pub drop_handle: Option<fn(u64)>,
    pub cancel: Option<fn(CallId)>,
    /// Functions reproduction bundles record (see [`FnDesc::record`]):
    /// each name, the kind of operation it is filed under (empty for
    /// `native MODULE.FUNCTION`), and how many leading arguments identify a
    /// call.
    pub recorded: &'static [(&'static str, &'static str, u32)],
    /// Functions without effects outside the call, which reproduction
    /// bundles need not capture.
    pub pure: &'static [&'static str],
}

pub fn encode_outcome(o: &Outcome) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(status::VALUE);
    match o {
        Ok(v) => v.encode(&mut w),
        Err(e) => e.value().encode(&mut w),
    }
    w.buf
}

pub fn defect(message: &str) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(status::DEFECT);
    w.str(message);
    w.buf
}

fn finish(host: *const Host, call: *mut Call, bytes: &[u8]) {
    unsafe { ((*host).complete)(call, bytes.as_ptr(), bytes.len()) }
}

fn panic_message(p: &(dyn std::any::Any + Send)) -> String {
    p.downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| p.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a panic".into())
}

thread_local! {
    static IN_DISPATCH: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Panics inside a dispatched function are caught and reported to the
/// program as defects; keep them off the process's panic hook (which, for
/// a module linked into the `badger` executable, reports internal errors).
fn quiet_panics() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if !IN_DISPATCH.with(|f| f.get()) {
                prev(info)
            }
        }));
    });
}

/// Run a module hook (handle release, cancellation) with its panics caught
/// and kept off the process's panic hook.
#[doc(hidden)]
pub fn module_thread_call(f: impl FnOnce()) {
    let _ = dispatched(f);
}

/// The `io.Error` kind of an operating-system error (one mapping for the
/// runtime and every module, so the same failure has the same kind).
pub fn io_error_kind(k: std::io::ErrorKind) -> &'static str {
    use std::io::ErrorKind::*;
    match k {
        NotFound => "io/not_found",
        PermissionDenied => "io/permission_denied",
        AlreadyExists => "io/already_exists",
        InvalidData => "io/invalid_data",
        InvalidInput => "io/invalid_input",
        DirectoryNotEmpty => "io/not_empty",
        IsADirectory => "io/is_a_directory",
        NotADirectory => "io/not_a_directory",
        TimedOut => "io/timed_out",
        BrokenPipe => "io/broken_pipe",
        Interrupted => "io/interrupted",
        UnexpectedEof => "io/unexpected_eof",
        _ => "io/other",
    }
}

/// Mark the current thread as the module's own (a tokio worker, a
/// connection thread). Its panics stay off the process's panic hook, which
/// in the `badger` executable would end the whole process: a panicking
/// tokio task becomes its `JoinError`, and an async call the panic leaves
/// unfinished is reported to the program as a defect (its [`Completion`]
/// is dropped).
pub fn module_thread() {
    quiet_panics();
    IN_DISPATCH.with(|c| c.set(true));
}

/// Run `f` as dispatched module code (its panics are caught by the caller).
fn dispatched<R>(f: impl FnOnce() -> R) -> std::thread::Result<R> {
    quiet_panics();
    let was = IN_DISPATCH.with(|c| c.replace(true));
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    IN_DISPATCH.with(|c| c.set(was));
    r
}

/// The dispatcher [`export!`] installs: decode the arguments, run the
/// function (catching panics), complete the call.
///
/// # Safety
/// `host` and `call` come from the host, for one call in progress.
pub unsafe fn dispatch(def: &'static ModuleDef, host: *const Host, call: *mut Call, index: usize) {
    let args = unsafe {
        let mut len = 0usize;
        let p = ((*host).args)(call, &mut len);
        let bytes = if p.is_null() { &[][..] } else { std::slice::from_raw_parts(p, len) };
        let mut r = Reader::new(bytes);
        match Value::decode(&mut r) {
            Ok(Value::List(values)) => Args { values },
            Ok(other) => {
                finish(host, call, &defect(&format!("arguments are not a list: {other:?}")));
                return;
            }
            Err(e) => {
                finish(host, call, &defect(&format!("malformed arguments: {e}")));
                return;
            }
        }
    };
    let Some((_, handler)) = def.functions.get(index) else {
        finish(host, call, &defect(&format!("module `{}` has no function {index}", def.name)));
        return;
    };
    match handler {
        Handler::Inline(f) | Handler::Blocking(f) => {
            let bytes = match dispatched(|| f(&args)) {
                Ok(o) => encode_outcome(&o),
                Err(p) => defect(&format!("panicked: {}", panic_message(&*p))),
            };
            finish(host, call, &bytes);
        }
        Handler::Async(f) => {
            let c = Completion { host, call, done: false };
            // a panic drops the completion while unwinding, which completes
            // the call with a defect (unless the function completed first)
            let _ = dispatched(|| f(args, c));
        }
    }
}

/// The C module table for `def` (built once).
pub fn module_table(
    def: &'static ModuleDef,
    call: unsafe extern "C" fn(*const Host, *mut Call, usize),
) -> *const Module {
    struct Table {
        module: Module,
        _fns: Vec<FnDesc>,
        _names: Vec<CString>,
        _kinds: Vec<Option<CString>>,
    }
    unsafe impl Send for Table {}
    unsafe impl Sync for Table {}
    static TABLES: Mutex<Vec<(&'static str, &'static Table)>> = Mutex::new(Vec::new());
    let mut g = TABLES.lock().unwrap();
    if let Some((_, t)) = g.iter().find(|(n, _)| *n == def.name) {
        return &t.module;
    }
    let names: Vec<CString> = std::iter::once(def.name)
        .chain(def.functions.iter().map(|(n, _)| *n))
        .map(|n| CString::new(n).expect("module and function names have no NUL"))
        .collect();
    let kinds: Vec<Option<CString>> = def
        .functions
        .iter()
        .map(|(f, _)| {
            def.recorded
                .iter()
                .find(|(r, k, _)| r == f && !k.is_empty())
                .map(|(_, k, _)| CString::new(*k).expect("kinds have no NUL"))
        })
        .collect();
    let fns: Vec<FnDesc> = def
        .functions
        .iter()
        .zip(&names[1..])
        .zip(&kinds)
        .map(|(((f, h), n), kind)| FnDesc {
            name: n.as_ptr(),
            mode: match h {
                Handler::Inline(_) => Mode::Inline,
                Handler::Blocking(_) => Mode::Blocking,
                Handler::Async(_) => Mode::Async,
            },
            record: if def.pure.contains(f) {
                PURE
            } else {
                def.recorded.iter().find(|(r, _, _)| r == f).map_or(0, |(_, _, k)| k + 1)
            },
            record_kind: kind.as_ref().map_or(std::ptr::null(), |k| k.as_ptr()),
        })
        .collect();
    let module = Module {
        abi_version: ABI_VERSION,
        name: names[0].as_ptr(),
        fns: fns.as_ptr(),
        nfns: fns.len(),
        call,
        drop_handle: None,
        cancel: None,
    };
    let t: &'static Table = Box::leak(Box::new(Table { module, _fns: fns, _names: names, _kinds: kinds }));
    g.push((def.name, t));
    &t.module
}

/// Export `DEF` (a `static` [`ModuleDef`]) as native module `NAME`: the
/// C entry point `badger_native_v1_NAME`, and `native_entry` for linking
/// the module into a program's executable instead of loading it.
#[macro_export]
macro_rules! export {
    ($name:literal, $def:path) => {
        unsafe extern "C" fn __lang_native_call(
            host: *const $crate::Host,
            call: *mut $crate::Call,
            index: usize,
        ) {
            unsafe { $crate::dispatch(&$def, host, call, index) }
        }
        unsafe extern "C" fn __lang_native_drop(h: u64) {
            if let Some(f) = $def.drop_handle {
                $crate::module_thread_call(|| f(h));
            }
        }
        unsafe extern "C" fn __lang_native_cancel(c: *mut $crate::Call) {
            if let Some(f) = $def.cancel {
                $crate::module_thread_call(|| f($crate::CallId(c as usize)));
            }
        }
        /// The module's entry point.
        ///
        /// # Safety
        /// Called by the host with its function table.
        #[unsafe(export_name = concat!("badger_native_v1_", $name))]
        pub unsafe extern "C" fn native_entry(host: *const $crate::Host) -> *const $crate::Module {
            if host.is_null() || unsafe { (*host).abi_version } != $crate::ABI_VERSION {
                return std::ptr::null();
            }
            let m = $crate::module_table(&$def, __lang_native_call) as *mut $crate::Module;
            unsafe {
                if $def.drop_handle.is_some() {
                    (*m).drop_handle = Some(__lang_native_drop);
                }
                if $def.cancel.is_some() {
                    (*m).cancel = Some(__lang_native_cancel);
                }
            }
            m
        }
    };
}

// ------------------------------------------------------------------ handles

/// The last handle number issued, and the live entries.
type Table<T> = Mutex<(u64, std::collections::HashMap<u64, Arc<T>>)>;

/// A thread-safe table of module-owned state named by handles.
pub struct Handles<T> {
    inner: OnceLock<Table<T>>,
}

impl<T> Default for Handles<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Handles<T> {
    pub const fn new() -> Handles<T> {
        Handles { inner: OnceLock::new() }
    }
    fn table(&self) -> &Table<T> {
        self.inner.get_or_init(|| Mutex::new((0, std::collections::HashMap::new())))
    }
    /// Store `v`; its handle value.
    pub fn insert(&self, v: T) -> Value {
        let mut g = self.table().lock().unwrap();
        g.0 += 1;
        let id = g.0;
        g.1.insert(id, Arc::new(v));
        Value::Handle(id)
    }
    pub fn get(&self, id: u64) -> Result<Arc<T>, Error> {
        self.table()
            .lock()
            .unwrap()
            .1
            .get(&id)
            .cloned()
            .ok_or_else(|| Error::new("native/closed", "the handle was closed"))
    }
    pub fn remove(&self, id: u64) -> Option<Arc<T>> {
        self.table().lock().unwrap().1.remove(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_round_trip() {
        let v = Value::List(vec![
            Value::Unit,
            Value::Null,
            Value::Bool(true),
            Value::Int(-7),
            Value::Float(1.5),
            Value::Char('é'),
            Value::str("hi"),
            Value::Symbol("a/b".into()),
            Value::Bytes(vec![0, 255]),
            Value::Map(vec![("k".into(), Value::Int(1))]),
            Value::Variant("some".into(), vec![Value::Int(2)]),
            Value::Handle(9),
        ]);
        let mut w = Writer::default();
        v.encode(&mut w);
        let mut r = Reader::new(&w.buf);
        assert_eq!(Value::decode(&mut r).unwrap(), v);
        assert!(r.at_end());
    }

    #[test]
    fn malformed_wire_data_is_an_error() {
        for bad in [&[tag::STR, 5, 0, 0, 0, b'a'][..], &[tag::LIST, 255, 255, 255, 255], &[99], &[]] {
            assert!(Value::decode(&mut Reader::new(bad)).is_err(), "{bad:?}");
        }
    }
}
