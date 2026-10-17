//! Binary MIR files (`lang compile`): a lowered program saved so `lang run`
//! can generate code and run it without parsing or checking the sources.
//!
//! MIR refers to runtime objects by address (descriptors, immortal strings,
//! symbols, static closures, nullary enum values, type-argument arrays).
//! Saving replaces every address with an index into a constant pool that
//! records how the object was made; loading makes fresh objects and patches
//! the program with their new addresses.

use index_vec::IndexVec;
use lang_runtime::desc::{Field, Kind, TypeDesc, Variant, builtin};
use rustc_hash::FxHashMap;

use crate::mir::*;

const MAGIC: &[u8; 8] = b"LANGMIR1";
/// Layout of the file format; files of another version are refused.
const FORMAT: u32 = 5;

/// The toolchain that wrote a file must be the one that loads it: MIR and
/// the runtime's ABI change together.
fn toolchain() -> String {
    format!("lang {} (mir format {FORMAT})", env!("CARGO_PKG_VERSION"))
}

/// FNV-1a over the payload: detects corrupted or truncated files.
fn checksum(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Whether `bytes` start like a saved program.
pub fn is_binary(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}

// ------------------------------------------------------------------ pool

#[derive(Debug, Clone)]
enum Entry {
    Null,
    Desc(usize),
    Builtin(String),
    Str(String),
    Sym(String),
    Closure { desc: u32, size: u32, fid: u32 },
    Nullary { desc: u32, idx: u32 },
    Targs(Vec<u32>),
}

fn builtins() -> Vec<(&'static str, &'static TypeDesc)> {
    vec![
        ("unit", builtin::unit()),
        ("bool", builtin::bool_()),
        ("int", builtin::int()),
        ("u8", builtin::u8_()),
        ("u16", builtin::u16_()),
        ("float", builtin::float()),
        ("char", builtin::char_()),
        ("symbol", builtin::symbol()),
        ("string", builtin::string()),
        ("bytes", builtin::bytes()),
        ("duration", builtin::duration()),
        ("instant", builtin::instant()),
        ("fn", builtin::fn_()),
    ]
}

struct Pool<'p> {
    origins: &'p Origins,
    desc_index: FxHashMap<u64, usize>,
    builtins: Vec<(&'static str, &'static TypeDesc)>,
    entries: Vec<Entry>,
    index: FxHashMap<u64, u32>,
    descs: Vec<&'static TypeDesc>,
}

impl Pool<'_> {
    fn addr(&mut self, w: u64) -> Result<u32, String> {
        if let Some(&i) = self.index.get(&w) {
            return Ok(i);
        }
        let entry = if w == 0 {
            Entry::Null
        } else if let Some(&d) = self.desc_index.get(&w) {
            Entry::Desc(d)
        } else if let Some((name, _)) = self.builtins.iter().find(|(_, d)| *d as *const TypeDesc as u64 == w)
        {
            Entry::Builtin(name.to_string())
        } else if let Some(s) = self.origins.strings.get(&w) {
            Entry::Str(s.clone())
        } else if let Some(s) = self.origins.symbols.get(&w) {
            Entry::Sym(s.clone())
        } else if let Some(&(desc, size, fid)) = self.origins.closures.get(&w) {
            let desc = self.addr(desc)?;
            Entry::Closure { desc, size, fid: fid.raw() }
        } else if let Some(&(desc, idx)) = self.origins.nullary.get(&w) {
            let desc = self.addr(desc)?;
            Entry::Nullary { desc, idx }
        } else if let Some(ds) = self.origins.targs.get(&w).cloned() {
            let mut v = vec![];
            for d in ds {
                v.push(self.addr(d)?);
            }
            Entry::Targs(v)
        } else {
            return Err(format!("the program refers to a runtime object at {w:#x} that cannot be saved"));
        };
        let i = self.entries.len() as u32;
        self.entries.push(entry);
        self.index.insert(w, i);
        Ok(i)
    }

    fn desc(&mut self, d: &'static TypeDesc) -> Result<u32, String> {
        self.addr(d as *const TypeDesc as u64)
    }
}

// ------------------------------------------------------------------ writer

struct W {
    out: Vec<u8>,
}

impl W {
    fn u8(&mut self, v: u8) {
        self.out.push(v);
    }
    fn u32(&mut self, v: u32) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    fn bool(&mut self, v: bool) {
        self.u8(v as u8);
    }
    fn str(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.out.extend_from_slice(s.as_bytes());
    }
    fn opt_str(&mut self, s: &Option<String>) {
        match s {
            Some(s) => {
                self.u8(1);
                self.str(s);
            }
            None => self.u8(0),
        }
    }
}

fn repr_code(r: Repr) -> u8 {
    match r {
        Repr::I8 => 0,
        Repr::I16 => 1,
        Repr::I32 => 2,
        Repr::I64 => 3,
        Repr::F32 => 4,
        Repr::F64 => 5,
        Repr::Ptr => 6,
    }
}

fn repr_of(c: u8) -> Result<Repr, String> {
    Ok(match c {
        0 => Repr::I8,
        1 => Repr::I16,
        2 => Repr::I32,
        3 => Repr::I64,
        4 => Repr::F32,
        5 => Repr::F64,
        6 => Repr::Ptr,
        _ => return Err(format!("bad representation {c}")),
    })
}

const BIN_OPS: [BinOp; 10] = [
    BinOp::Add,
    BinOp::Sub,
    BinOp::Mul,
    BinOp::Div,
    BinOp::Rem,
    BinOp::And,
    BinOp::Or,
    BinOp::Xor,
    BinOp::Shl,
    BinOp::Shr,
];
const CMP_OPS: [CmpOp; 6] = [CmpOp::Eq, CmpOp::Ne, CmpOp::Lt, CmpOp::Le, CmpOp::Gt, CmpOp::Ge];

struct Enc<'p> {
    w: W,
    pool: Pool<'p>,
}

impl Enc<'_> {
    fn addr(&mut self, a: u64) -> Result<(), String> {
        let i = self.pool.addr(a)?;
        self.w.u32(i);
        Ok(())
    }

    fn num(&mut self, t: NumTy) {
        match t {
            NumTy::I(bits, signed) => {
                self.w.u8(0);
                self.w.u8(bits);
                self.w.bool(signed);
            }
            NumTy::F(bits) => {
                self.w.u8(1);
                self.w.u8(bits);
            }
        }
    }

    fn operand(&mut self, o: &Operand) -> Result<(), String> {
        match o {
            Operand::Local(l) => {
                self.w.u8(0);
                self.w.u32(l.raw());
            }
            Operand::Const(Const::Int(v, r)) => {
                self.w.u8(1);
                self.w.u64(*v as u64);
                self.w.u8(repr_code(*r));
            }
            Operand::Const(Const::F32(f)) => {
                self.w.u8(2);
                self.w.u32(f.to_bits());
            }
            Operand::Const(Const::F64(f)) => {
                self.w.u8(3);
                self.w.u64(f.to_bits());
            }
            Operand::Const(Const::Addr(a)) => {
                self.w.u8(4);
                self.addr(*a)?;
            }
        }
        Ok(())
    }

    fn operands(&mut self, os: &[Operand]) -> Result<(), String> {
        self.w.u32(os.len() as u32);
        for o in os {
            self.operand(o)?;
        }
        Ok(())
    }

    fn ffi(&mut self, a: &FfiArg) -> Result<(), String> {
        match a {
            FfiArg::Scalar(r) => {
                self.w.u8(0);
                self.w.u8(repr_code(*r));
            }
            FfiArg::Regs { desc, size, sse } => {
                self.w.u8(1);
                self.addr(*desc)?;
                self.w.u32(*size);
                self.w.u32(sse.len() as u32);
                for s in sse {
                    self.w.bool(*s);
                }
            }
            FfiArg::Memory { desc, size } => {
                self.w.u8(2);
                self.addr(*desc)?;
                self.w.u32(*size);
            }
        }
        Ok(())
    }

    fn callee(&mut self, c: &Callee) -> Result<(), String> {
        match c {
            Callee::Direct(f) => {
                self.w.u8(0);
                self.w.u32(f.raw());
            }
            Callee::Closure(o) => {
                self.w.u8(1);
                self.operand(o)?;
            }
            Callee::Runtime(name) => {
                self.w.u8(2);
                self.w.str(name);
            }
            Callee::Intrinsic { name, targs, ret } => {
                self.w.u8(3);
                self.w.str(name);
                self.addr(*targs)?;
                self.addr(*ret)?;
            }
            Callee::Foreign { name, params, ret, contract } => {
                self.w.u8(4);
                self.w.str(name);
                self.w.u32(params.len() as u32);
                for p in params {
                    self.ffi(p)?;
                }
                match ret {
                    Some(r) => {
                        self.w.u8(1);
                        self.ffi(r)?;
                    }
                    None => self.w.u8(0),
                }
                self.w.u8(*contract);
            }
        }
        Ok(())
    }

    fn rvalue(&mut self, r: &Rvalue) -> Result<(), String> {
        match r {
            Rvalue::Use(o) => {
                self.w.u8(0);
                self.operand(o)?;
            }
            Rvalue::Bin { op, a, b, ty, loc } => {
                self.w.u8(1);
                self.w.u8(BIN_OPS.iter().position(|x| x == op).unwrap() as u8);
                self.operand(a)?;
                self.operand(b)?;
                self.num(*ty);
                self.addr(*loc)?;
            }
            Rvalue::Neg { a, ty, loc } => {
                self.w.u8(2);
                self.operand(a)?;
                self.num(*ty);
                self.addr(*loc)?;
            }
            Rvalue::Not(o) => {
                self.w.u8(3);
                self.operand(o)?;
            }
            Rvalue::Cmp { op, a, b, ty } => {
                self.w.u8(4);
                self.w.u8(CMP_OPS.iter().position(|x| x == op).unwrap() as u8);
                self.operand(a)?;
                self.operand(b)?;
                self.num(*ty);
            }
            Rvalue::Convert { a, from, to, checked, loc } => {
                self.w.u8(5);
                self.operand(a)?;
                self.num(*from);
                self.num(*to);
                self.w.bool(*checked);
                self.addr(*loc)?;
            }
            Rvalue::Fits { a, from, to } => {
                self.w.u8(6);
                self.operand(a)?;
                self.num(*from);
                self.num(*to);
            }
            Rvalue::Overflows { op, a, b, ty } => {
                self.w.u8(7);
                self.w.u8(BIN_OPS.iter().position(|x| x == op).unwrap() as u8);
                self.operand(a)?;
                self.operand(b)?;
                self.num(*ty);
            }
            Rvalue::Load { base, offset, repr } => {
                self.w.u8(8);
                self.operand(base)?;
                self.w.u32(*offset as u32);
                self.w.u8(repr_code(*repr));
            }
            Rvalue::FromWord { a, to } => {
                self.w.u8(9);
                self.operand(a)?;
                self.w.u8(repr_code(*to));
            }
            Rvalue::ToWord(o) => {
                self.w.u8(10);
                self.operand(o)?;
            }
            Rvalue::Tag(o) => {
                self.w.u8(11);
                self.operand(o)?;
            }
            Rvalue::Alloc { desc, size } => {
                self.w.u8(12);
                self.addr(*desc)?;
                self.w.u32(*size);
            }
            Rvalue::Call { callee, args, ret } => {
                self.w.u8(13);
                self.callee(callee)?;
                self.operands(args)?;
                self.w.u8(repr_code(*ret));
            }
            Rvalue::Concat(os) => {
                self.w.u8(14);
                self.operands(os)?;
            }
            Rvalue::GlobalLoad(s) => {
                self.w.u8(15);
                self.w.u32(*s);
            }
            Rvalue::GlobalAddr(s) => {
                self.w.u8(16);
                self.w.u32(*s);
            }
            Rvalue::FuncAddr(f) => {
                self.w.u8(17);
                self.w.u32(f.raw());
            }
            Rvalue::AdapterAddr(f) => {
                self.w.u8(18);
                self.w.u32(f.raw());
            }
        }
        Ok(())
    }

    fn stmt(&mut self, s: &Stmt) -> Result<(), String> {
        match s {
            Stmt::Assign(l, r) => {
                self.w.u8(0);
                self.w.u32(l.raw());
                self.rvalue(r)?;
            }
            Stmt::Store { base, offset, repr, value } => {
                self.w.u8(1);
                self.operand(base)?;
                self.w.u32(*offset as u32);
                self.w.u8(repr_code(*repr));
                self.operand(value)?;
            }
            Stmt::GlobalStore { slot, value } => {
                self.w.u8(2);
                self.w.u32(*slot);
                self.operand(value)?;
            }
            Stmt::Poll => self.w.u8(3),
            Stmt::Loc { file, line, col } => {
                self.w.u8(4);
                self.w.u32(*file);
                self.w.u32(*line);
                self.w.u32(*col);
            }
        }
        Ok(())
    }

    fn term(&mut self, t: &Option<Term>) -> Result<(), String> {
        match t {
            None => self.w.u8(0),
            Some(Term::Goto(b)) => {
                self.w.u8(1);
                self.w.u32(b.raw());
            }
            Some(Term::Branch(o, a, b)) => {
                self.w.u8(2);
                self.operand(o)?;
                self.w.u32(a.raw());
                self.w.u32(b.raw());
            }
            Some(Term::Switch(o, cases, default)) => {
                self.w.u8(3);
                self.operand(o)?;
                self.w.u32(cases.len() as u32);
                for (v, b) in cases {
                    self.w.u64(*v as u64);
                    self.w.u32(b.raw());
                }
                self.w.u32(default.raw());
            }
            Some(Term::Return(o)) => {
                self.w.u8(4);
                self.operand(o)?;
            }
            Some(Term::Unreachable) => self.w.u8(5),
        }
        Ok(())
    }

    fn func(&mut self, f: &Func) -> Result<(), String> {
        self.w.str(&f.name);
        self.w.u8(match f.kind {
            FuncKind::Normal => 0,
            FuncKind::Lambda => 1,
            FuncKind::DisplayHook => 2,
        });
        self.w.u32(f.env.raw());
        self.w.u32(f.params.len() as u32);
        for p in &f.params {
            self.w.u32(p.raw());
        }
        self.w.u8(repr_code(f.ret));
        self.w.u32(f.locals.len() as u32);
        for l in &f.locals {
            self.w.u8(repr_code(l.repr));
            self.w.opt_str(&l.name);
        }
        self.w.u32(f.blocks.len() as u32);
        for b in &f.blocks {
            self.w.u32(b.stmts.len() as u32);
            for s in &b.stmts {
                self.stmt(s)?;
            }
            self.term(&b.term)?;
        }
        self.w.bool(f.needs_adapter);
        self.w.opt_str(&f.live_key);
        self.w.str(&f.live_sig);
        self.w.bool(f.is_global);
        Ok(())
    }
}

fn kind_out(w: &mut W, pool: &mut Pool<'_>, k: &Kind) -> Result<(), String> {
    let fields = |w: &mut W, pool: &mut Pool<'_>, fs: &[Field]| -> Result<(), String> {
        w.u32(fs.len() as u32);
        for f in fs {
            w.str(&f.name);
            w.u32(f.offset);
            w.u8(repr_code(f.repr));
            let i = pool.desc(f.desc)?;
            w.u32(i);
        }
        Ok(())
    };
    match k {
        Kind::Unit => w.u8(0),
        Kind::Bool => w.u8(1),
        Kind::Int { bits, signed } => {
            w.u8(2);
            w.u8(*bits);
            w.bool(*signed);
        }
        Kind::Float { bits } => {
            w.u8(3);
            w.u8(*bits);
        }
        Kind::Char => w.u8(4),
        Kind::Symbol => w.u8(5),
        Kind::Duration => w.u8(6),
        Kind::Instant => w.u8(7),
        Kind::Timestamp => w.u8(8),
        Kind::String => w.u8(9),
        Kind::Bytes => w.u8(10),
        Kind::Record { size, fields: fs, nominal, tuple } => {
            w.u8(11);
            w.u32(*size);
            fields(w, pool, fs)?;
            w.bool(*nominal);
            w.bool(*tuple);
        }
        Kind::Enum { size, variants } => {
            w.u8(12);
            w.u32(*size);
            w.u32(variants.len() as u32);
            for v in variants {
                w.str(&v.name);
                fields(w, pool, &v.fields)?;
            }
        }
        Kind::Closure { size, captures } => {
            w.u8(13);
            w.u32(*size);
            fields(w, pool, captures)?;
        }
        Kind::Fn => w.u8(14),
        Kind::Dynamic => w.u8(15),
        Kind::List { elem } => {
            w.u8(16);
            let i = pool.desc(elem)?;
            w.u32(i);
        }
        Kind::Map { key, val } => {
            w.u8(17);
            let (a, b) = (pool.desc(key)?, pool.desc(val)?);
            w.u32(a);
            w.u32(b);
        }
        Kind::Set { elem } => {
            w.u8(18);
            let i = pool.desc(elem)?;
            w.u32(i);
        }
        Kind::Range { elem } => {
            w.u8(19);
            let i = pool.desc(elem)?;
            w.u32(i);
        }
        Kind::Opaque { trace, finalize, args } => {
            if trace.is_some() || finalize.is_some() {
                return Err("a runtime-defined descriptor cannot be saved".into());
            }
            w.u8(20);
            w.u32(args.len() as u32);
            for a in args {
                let i = pool.desc(a)?;
                w.u32(i);
            }
        }
    }
    Ok(())
}

/// Save a lowered program (and `meta`, the run configuration) as bytes.
pub fn encode(prog: &Program, meta: &[(String, String)]) -> Result<Vec<u8>, String> {
    let desc_index =
        prog.origins.descs.iter().enumerate().map(|(i, d)| (*d as *const TypeDesc as u64, i)).collect();
    let pool = Pool {
        origins: &prog.origins,
        desc_index,
        builtins: builtins(),
        entries: vec![],
        index: FxHashMap::default(),
        descs: prog.origins.descs.clone(),
    };
    let mut e = Enc { w: W { out: vec![] }, pool };
    // the program body first: it fills the pool
    e.w.u32(prog.funcs.len() as u32);
    for f in &prog.funcs {
        e.func(f)?;
    }
    let opt_fid = |w: &mut W, f: Option<FuncId>| match f {
        Some(f) => {
            w.u8(1);
            w.u32(f.raw());
        }
        None => w.u8(0),
    };
    opt_fid(&mut e.w, prog.entry);
    for d in [prog.entry_ret, prog.entry_args] {
        match d {
            Some(d) => {
                e.w.u8(1);
                let i = e.pool.desc(d)?;
                e.w.u32(i);
            }
            None => e.w.u8(0),
        }
    }
    e.w.opt_str(&prog.entry_error);
    opt_fid(&mut e.w, prog.error_report);
    e.w.u32(prog.n_globals);
    e.w.u32(prog.ptr_globals.len() as u32);
    for s in &prog.ptr_globals {
        e.w.u32(*s);
    }
    e.w.u32(prog.static_closures.len() as u32);
    for (o, f) in &prog.static_closures {
        e.addr(*o)?;
        e.w.u32(f.raw());
    }
    e.w.u32(prog.display_hooks.len() as u32);
    for (d, f) in &prog.display_hooks {
        let i = e.pool.desc(d)?;
        e.w.u32(i);
        e.w.u32(f.raw());
    }
    e.w.u32(prog.tests.len() as u32);
    for (n, f) in &prog.tests {
        e.w.str(n);
        e.w.u32(f.raw());
    }
    e.w.u32(prog.debug_files.len() as u32);
    for f in &prog.debug_files {
        e.w.str(f);
    }
    let body = std::mem::take(&mut e.w.out);
    // descriptors reachable from the pool (their fields may add entries)
    let mut descs_out = W { out: vec![] };
    let mut written = 0;
    let mut desc_count = 0u32;
    while written < e.pool.entries.len() {
        if let Entry::Desc(d) = e.pool.entries[written].clone() {
            let desc = e.pool.descs[d];
            descs_out.u32(written as u32);
            descs_out.str(&desc.name);
            kind_out(&mut descs_out, &mut e.pool, &desc.kind)?;
            desc_count += 1;
        }
        written += 1;
    }
    // payload: meta, pool entries, descriptor kinds, body
    let mut w = W { out: vec![] };
    w.u32(meta.len() as u32);
    for (k, v) in meta {
        w.str(k);
        w.str(v);
    }
    w.u32(e.pool.entries.len() as u32);
    for en in &e.pool.entries {
        match en {
            Entry::Null => w.u8(0),
            Entry::Desc(_) => w.u8(1),
            Entry::Builtin(n) => {
                w.u8(2);
                w.str(n);
            }
            Entry::Str(s) => {
                w.u8(3);
                w.str(s);
            }
            Entry::Sym(s) => {
                w.u8(4);
                w.str(s);
            }
            Entry::Closure { desc, size, fid } => {
                w.u8(5);
                w.u32(*desc);
                w.u32(*size);
                w.u32(*fid);
            }
            Entry::Nullary { desc, idx } => {
                w.u8(6);
                w.u32(*desc);
                w.u32(*idx);
            }
            Entry::Targs(v) => {
                w.u8(7);
                w.u32(v.len() as u32);
                for x in v {
                    w.u32(*x);
                }
            }
        }
    }
    w.u32(desc_count);
    w.out.extend_from_slice(&descs_out.out);
    w.out.extend_from_slice(&body);
    // file: magic, toolchain, checksum of the payload, payload
    let mut file = W { out: MAGIC.to_vec() };
    file.str(&toolchain());
    file.u64(checksum(&w.out));
    file.out.extend_from_slice(&w.out);
    Ok(file.out)
}

// ------------------------------------------------------------------ reader

struct R<'b> {
    b: &'b [u8],
    pos: usize,
}

impl R<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        if self.pos + n > self.b.len() {
            return Err("truncated program file".into());
        }
        let s = &self.b[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn bool(&mut self) -> Result<bool, String> {
        Ok(self.u8()? != 0)
    }
    fn str(&mut self) -> Result<String, String> {
        let n = self.u32()? as usize;
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| "bad text in program file".to_string())
    }
    fn opt_str(&mut self) -> Result<Option<String>, String> {
        Ok(if self.u8()? == 1 { Some(self.str()?) } else { None })
    }
}

struct Dec<'b> {
    r: R<'b>,
    /// New address of each pool entry.
    addrs: Vec<u64>,
}

impl Dec<'_> {
    fn addr(&mut self) -> Result<u64, String> {
        let i = self.r.u32()? as usize;
        self.addrs.get(i).copied().ok_or_else(|| "bad constant reference".to_string())
    }
    fn desc(&mut self) -> Result<&'static TypeDesc, String> {
        let a = self.addr()?;
        if a == 0 {
            return Err("missing descriptor".into());
        }
        Ok(unsafe { &*(a as *const TypeDesc) })
    }
    fn num(&mut self) -> Result<NumTy, String> {
        Ok(match self.r.u8()? {
            0 => NumTy::I(self.r.u8()?, self.r.bool()?),
            _ => NumTy::F(self.r.u8()?),
        })
    }
    fn repr(&mut self) -> Result<Repr, String> {
        repr_of(self.r.u8()?)
    }
    fn local(&mut self) -> Result<Local, String> {
        Ok(Local::from_raw(self.r.u32()?))
    }
    fn block(&mut self) -> Result<BlockId, String> {
        Ok(BlockId::from_raw(self.r.u32()?))
    }
    fn fid(&mut self) -> Result<FuncId, String> {
        Ok(FuncId::from_raw(self.r.u32()?))
    }
    fn operand(&mut self) -> Result<Operand, String> {
        Ok(match self.r.u8()? {
            0 => Operand::Local(self.local()?),
            1 => {
                let v = self.r.u64()? as i64;
                Operand::Const(Const::Int(v, self.repr()?))
            }
            2 => Operand::Const(Const::F32(f32::from_bits(self.r.u32()?))),
            3 => Operand::Const(Const::F64(f64::from_bits(self.r.u64()?))),
            4 => Operand::Const(Const::Addr(self.addr()?)),
            t => return Err(format!("bad operand {t}")),
        })
    }
    fn operands(&mut self) -> Result<Vec<Operand>, String> {
        let n = self.r.u32()?;
        (0..n).map(|_| self.operand()).collect()
    }
    fn ffi(&mut self) -> Result<FfiArg, String> {
        Ok(match self.r.u8()? {
            0 => FfiArg::Scalar(self.repr()?),
            1 => {
                let desc = self.addr()?;
                let size = self.r.u32()?;
                let n = self.r.u32()?;
                let sse = (0..n).map(|_| self.r.bool()).collect::<Result<_, _>>()?;
                FfiArg::Regs { desc, size, sse }
            }
            _ => FfiArg::Memory { desc: self.addr()?, size: self.r.u32()? },
        })
    }
    fn callee(&mut self) -> Result<Callee, String> {
        Ok(match self.r.u8()? {
            0 => Callee::Direct(self.fid()?),
            1 => Callee::Closure(self.operand()?),
            2 => {
                let name = self.r.str()?;
                Callee::Runtime(Box::leak(name.into_boxed_str()))
            }
            3 => Callee::Intrinsic { name: self.r.str()?, targs: self.addr()?, ret: self.addr()? },
            4 => {
                let name = self.r.str()?;
                let n = self.r.u32()?;
                let params = (0..n).map(|_| self.ffi()).collect::<Result<_, _>>()?;
                let ret = if self.r.u8()? == 1 { Some(self.ffi()?) } else { None };
                Callee::Foreign { name, params, ret, contract: self.r.u8()? }
            }
            t => return Err(format!("bad callee {t}")),
        })
    }
    fn bin(&mut self) -> Result<BinOp, String> {
        BIN_OPS.get(self.r.u8()? as usize).copied().ok_or_else(|| "bad operator".to_string())
    }
    fn rvalue(&mut self) -> Result<Rvalue, String> {
        Ok(match self.r.u8()? {
            0 => Rvalue::Use(self.operand()?),
            1 => {
                let op = self.bin()?;
                Rvalue::Bin { op, a: self.operand()?, b: self.operand()?, ty: self.num()?, loc: self.addr()? }
            }
            2 => Rvalue::Neg { a: self.operand()?, ty: self.num()?, loc: self.addr()? },
            3 => Rvalue::Not(self.operand()?),
            4 => {
                let op = CMP_OPS.get(self.r.u8()? as usize).copied().ok_or("bad comparison")?;
                Rvalue::Cmp { op, a: self.operand()?, b: self.operand()?, ty: self.num()? }
            }
            5 => Rvalue::Convert {
                a: self.operand()?,
                from: self.num()?,
                to: self.num()?,
                checked: self.r.bool()?,
                loc: self.addr()?,
            },
            6 => Rvalue::Fits { a: self.operand()?, from: self.num()?, to: self.num()? },
            7 => {
                let op = self.bin()?;
                Rvalue::Overflows { op, a: self.operand()?, b: self.operand()?, ty: self.num()? }
            }
            8 => Rvalue::Load { base: self.operand()?, offset: self.r.u32()? as i32, repr: self.repr()? },
            9 => Rvalue::FromWord { a: self.operand()?, to: self.repr()? },
            10 => Rvalue::ToWord(self.operand()?),
            11 => Rvalue::Tag(self.operand()?),
            12 => Rvalue::Alloc { desc: self.addr()?, size: self.r.u32()? },
            13 => Rvalue::Call { callee: self.callee()?, args: self.operands()?, ret: self.repr()? },
            14 => Rvalue::Concat(self.operands()?),
            15 => Rvalue::GlobalLoad(self.r.u32()?),
            16 => Rvalue::GlobalAddr(self.r.u32()?),
            17 => Rvalue::FuncAddr(self.fid()?),
            18 => Rvalue::AdapterAddr(self.fid()?),
            t => return Err(format!("bad value {t}")),
        })
    }
    fn stmt(&mut self) -> Result<Stmt, String> {
        Ok(match self.r.u8()? {
            0 => Stmt::Assign(self.local()?, self.rvalue()?),
            1 => Stmt::Store {
                base: self.operand()?,
                offset: self.r.u32()? as i32,
                repr: self.repr()?,
                value: self.operand()?,
            },
            2 => Stmt::GlobalStore { slot: self.r.u32()?, value: self.operand()? },
            4 => Stmt::Loc { file: self.r.u32()?, line: self.r.u32()?, col: self.r.u32()? },
            _ => Stmt::Poll,
        })
    }
    fn term(&mut self) -> Result<Option<Term>, String> {
        Ok(match self.r.u8()? {
            0 => None,
            1 => Some(Term::Goto(self.block()?)),
            2 => Some(Term::Branch(self.operand()?, self.block()?, self.block()?)),
            3 => {
                let o = self.operand()?;
                let n = self.r.u32()?;
                let mut cases = vec![];
                for _ in 0..n {
                    cases.push((self.r.u64()? as i64, self.block()?));
                }
                Some(Term::Switch(o, cases, self.block()?))
            }
            4 => Some(Term::Return(self.operand()?)),
            _ => Some(Term::Unreachable),
        })
    }
    fn func(&mut self) -> Result<Func, String> {
        let name = self.r.str()?;
        let kind = match self.r.u8()? {
            0 => FuncKind::Normal,
            1 => FuncKind::Lambda,
            _ => FuncKind::DisplayHook,
        };
        let env = self.local()?;
        let n = self.r.u32()?;
        let params = (0..n).map(|_| self.local()).collect::<Result<_, _>>()?;
        let ret = self.repr()?;
        let n = self.r.u32()?;
        let mut locals = IndexVec::new();
        for _ in 0..n {
            locals.push(LocalDecl { repr: self.repr()?, name: self.r.opt_str()? });
        }
        let n = self.r.u32()?;
        let mut blocks = IndexVec::new();
        for _ in 0..n {
            let k = self.r.u32()?;
            let stmts = (0..k).map(|_| self.stmt()).collect::<Result<_, _>>()?;
            blocks.push(Block { stmts, term: self.term()? });
        }
        Ok(Func {
            name,
            kind,
            env,
            params,
            ret,
            locals,
            blocks,
            needs_adapter: self.r.bool()?,
            live_key: self.r.opt_str()?,
            live_sig: self.r.str()?,
            is_global: self.r.bool()?,
        })
    }
}

/// Load a saved program, making its runtime objects anew. Returns the
/// program and the run configuration saved with it.
pub fn decode(bytes: &[u8]) -> Result<(Program, Vec<(String, String)>), String> {
    if !is_binary(bytes) {
        return Err("not a compiled program (missing LANGMIR1 header)".into());
    }
    let mut h = R { b: bytes, pos: MAGIC.len() };
    let made_by = h.str().ok().filter(|s| s.starts_with("lang ") && s.len() < 64);
    if made_by.as_deref() != Some(toolchain().as_str()) {
        let by = made_by.unwrap_or_else(|| "another toolchain".into());
        return Err(format!("compiled by {by}, but this is {}: compile it again", toolchain()));
    }
    let sum = h.u64()?;
    let payload = &bytes[h.pos..];
    if checksum(payload) != sum {
        return Err("the compiled program is corrupted (checksum mismatch): compile it again".into());
    }
    let mut r = R { b: payload, pos: 0 };
    let n = r.u32()?;
    let mut meta = vec![];
    for _ in 0..n {
        meta.push((r.str()?, r.str()?));
    }
    // pool: descriptors get placeholders first (kinds refer to each other)
    let n = r.u32()? as usize;
    #[derive(Debug)]
    enum Raw {
        Null,
        Desc,
        Builtin(String),
        Str(String),
        Sym(String),
        Closure(u32, u32),
        Nullary(u32, u32),
        Targs(Vec<u32>),
    }
    let mut raw = vec![];
    for _ in 0..n {
        raw.push(match r.u8()? {
            0 => Raw::Null,
            1 => Raw::Desc,
            2 => Raw::Builtin(r.str()?),
            3 => Raw::Str(r.str()?),
            4 => Raw::Sym(r.str()?),
            5 => {
                // (descriptor, size, function): the function is listed with
                // the program's static closures
                let (d, s, _) = (r.u32()?, r.u32()?, r.u32()?);
                Raw::Closure(d, s)
            }
            6 => Raw::Nullary(r.u32()?, r.u32()?),
            7 => {
                let k = r.u32()?;
                Raw::Targs((0..k).map(|_| r.u32()).collect::<Result<_, _>>()?)
            }
            t => return Err(format!("bad constant kind {t}")),
        });
    }
    let mut addrs = vec![0u64; n];
    let bs = builtins();
    for (i, e) in raw.iter().enumerate() {
        match e {
            Raw::Desc => {
                let p: *mut TypeDesc = Box::into_raw(Box::new(TypeDesc::new("", Kind::Unit)));
                addrs[i] = p as u64;
            }
            Raw::Builtin(name) => {
                let d = bs
                    .iter()
                    .find(|(b, _)| b == name)
                    .ok_or_else(|| format!("unknown descriptor `{name}`"))?;
                addrs[i] = d.1 as *const TypeDesc as u64;
            }
            Raw::Str(s) => addrs[i] = lang_runtime::string::immortal_string(s),
            Raw::Sym(s) => addrs[i] = lang_runtime::string::symbol(s),
            _ => {}
        }
    }
    let mut dec = Dec { r, addrs };
    // descriptor kinds
    let count = dec.r.u32()?;
    for _ in 0..count {
        let i = dec.r.u32()? as usize;
        let name = dec.r.str()?;
        let kind = read_kind(&mut dec)?;
        let p = dec.addrs[i] as *mut TypeDesc;
        unsafe {
            (*p).name = name;
            (*p).kind = kind;
            (*p).leaf.store(0, std::sync::atomic::Ordering::Relaxed);
        }
    }
    // objects made from descriptors
    for (i, e) in raw.iter().enumerate() {
        match e {
            Raw::Closure(desc, size) => {
                let d = unsafe { &*(dec.addrs[*desc as usize] as *const TypeDesc) };
                dec.addrs[i] = lang_runtime::gc::alloc_immortal(d, *size as usize);
            }
            Raw::Nullary(desc, idx) => {
                let d = unsafe { &*(dec.addrs[*desc as usize] as *const TypeDesc) };
                let w = lang_runtime::gc::alloc_immortal(d, d.payload_size() as usize);
                unsafe { *(lang_runtime::gc::obj(w).payload() as *mut u32) = *idx };
                dec.addrs[i] = w;
            }
            Raw::Targs(v) => {
                let ds: Vec<&'static TypeDesc> =
                    v.iter().map(|x| unsafe { &*(dec.addrs[*x as usize] as *const TypeDesc) }).collect();
                dec.addrs[i] = Box::leak(ds.into_boxed_slice()).as_ptr() as u64;
            }
            Raw::Null => dec.addrs[i] = 0,
            _ => {}
        }
    }
    // the program
    let mut prog = Program::default();
    let n = dec.r.u32()?;
    for _ in 0..n {
        let f = dec.func()?;
        prog.funcs.push(f);
    }
    prog.entry = if dec.r.u8()? == 1 { Some(dec.fid()?) } else { None };
    prog.entry_ret = if dec.r.u8()? == 1 { Some(dec.desc()?) } else { None };
    prog.entry_args = if dec.r.u8()? == 1 { Some(dec.desc()?) } else { None };
    prog.entry_error = dec.r.opt_str()?;
    prog.error_report = if dec.r.u8()? == 1 { Some(dec.fid()?) } else { None };
    prog.n_globals = dec.r.u32()?;
    let n = dec.r.u32()?;
    for _ in 0..n {
        let s = dec.r.u32()?;
        if s >= prog.n_globals {
            return Err("invalid compiled program: a global slot out of range".into());
        }
        prog.ptr_globals.push(s);
    }
    let n = dec.r.u32()?;
    for _ in 0..n {
        let o = dec.addr()?;
        prog.static_closures.push((o, dec.fid()?));
    }
    let n = dec.r.u32()?;
    for _ in 0..n {
        let d = dec.desc()?;
        prog.display_hooks.push((d, dec.fid()?));
    }
    let n = dec.r.u32()?;
    for _ in 0..n {
        let name = dec.r.str()?;
        prog.tests.push((name, dec.fid()?));
    }
    let n = dec.r.u32()?;
    for _ in 0..n {
        prog.debug_files.push(dec.r.str()?);
    }
    if dec.r.pos != dec.r.b.len() {
        return Err("the compiled program has trailing data".into());
    }
    validate(&prog)?;
    Ok((prog, meta))
}

/// Every reference in a loaded program points at something that exists:
/// functions, locals, blocks, global slots, and runtime entry points.
fn validate(prog: &Program) -> Result<(), String> {
    let nf = prog.funcs.len();
    let bad = |what: &str, f: &str| Err(format!("invalid compiled program: {what} in `{f}`"));
    let intrinsics: rustc_hash::FxHashSet<&str> =
        lang_runtime::symbols().into_iter().map(|(n, _)| n).collect();
    let fid_ok = |f: FuncId| f.index() < nf;
    for f in prog.funcs.iter() {
        let nl = f.locals.len();
        let nb = f.blocks.len();
        let op_ok = |o: &Operand| match o {
            Operand::Local(l) => l.index() < nl,
            Operand::Const(_) => true,
        };
        if f.env.index() >= nl || f.params.iter().any(|p| p.index() >= nl) {
            return bad("a parameter out of range", &f.name);
        }
        for b in f.blocks.iter() {
            for st in &b.stmts {
                let ok = match st {
                    Stmt::Assign(l, rv) => {
                        l.index() < nl
                            && match rv {
                                Rvalue::Use(a) | Rvalue::Not(a) | Rvalue::ToWord(a) | Rvalue::Tag(a) => {
                                    op_ok(a)
                                }
                                Rvalue::Neg { a, .. }
                                | Rvalue::Convert { a, .. }
                                | Rvalue::Fits { a, .. } => op_ok(a),
                                Rvalue::FromWord { a, .. } => op_ok(a),
                                Rvalue::Bin { a, b, .. }
                                | Rvalue::Cmp { a, b, .. }
                                | Rvalue::Overflows { a, b, .. } => op_ok(a) && op_ok(b),
                                Rvalue::Load { base, .. } => op_ok(base),
                                Rvalue::Alloc { .. } => true,
                                Rvalue::Concat(os) => os.iter().all(&op_ok),
                                Rvalue::GlobalLoad(s) | Rvalue::GlobalAddr(s) => *s < prog.n_globals,
                                Rvalue::FuncAddr(fid) | Rvalue::AdapterAddr(fid) => fid_ok(*fid),
                                Rvalue::Call { callee, args, .. } => {
                                    args.iter().all(&op_ok)
                                        && match callee {
                                            Callee::Direct(fid) => fid_ok(*fid),
                                            Callee::Closure(o) => op_ok(o),
                                            Callee::Runtime(name) => {
                                                lang_runtime::symbols().iter().any(|(n, _)| n == name)
                                            }
                                            Callee::Intrinsic { name, .. } => {
                                                intrinsics.contains(name.as_str())
                                            }
                                            Callee::Foreign { .. } => true,
                                        }
                                }
                            }
                    }
                    Stmt::Store { base, value, .. } => op_ok(base) && op_ok(value),
                    Stmt::GlobalStore { slot, value } => *slot < prog.n_globals && op_ok(value),
                    Stmt::Poll => true,
                    Stmt::Loc { file, .. } => (*file as usize) < prog.debug_files.len(),
                };
                if !ok {
                    return bad("an operation with an invalid reference", &f.name);
                }
            }
            let ok = match &b.term {
                None => true,
                Some(Term::Goto(t)) => t.index() < nb,
                Some(Term::Branch(o, x, y)) => op_ok(o) && x.index() < nb && y.index() < nb,
                Some(Term::Switch(o, cases, d)) => {
                    op_ok(o) && d.index() < nb && cases.iter().all(|(_, t)| t.index() < nb)
                }
                Some(Term::Return(o)) => op_ok(o),
                Some(Term::Unreachable) => true,
            };
            if !ok {
                return bad("a branch with an invalid target", &f.name);
            }
        }
    }
    let entries =
        prog.entry.iter().chain(prog.error_report.iter()).chain(prog.static_closures.iter().map(|c| &c.1));
    if entries
        .chain(prog.display_hooks.iter().map(|d| &d.1))
        .chain(prog.tests.iter().map(|t| &t.1))
        .any(|f| !fid_ok(*f))
    {
        return Err("invalid compiled program: a function reference out of range".into());
    }
    Ok(())
}

fn read_kind(d: &mut Dec<'_>) -> Result<Kind, String> {
    fn fields(d: &mut Dec<'_>) -> Result<Vec<Field>, String> {
        let n = d.r.u32()?;
        let mut v = vec![];
        for _ in 0..n {
            let name = d.r.str()?;
            let offset = d.r.u32()?;
            let repr = d.repr()?;
            let desc = d.desc()?;
            v.push(Field { name, offset, repr, desc });
        }
        Ok(v)
    }
    Ok(match d.r.u8()? {
        0 => Kind::Unit,
        1 => Kind::Bool,
        2 => Kind::Int { bits: d.r.u8()?, signed: d.r.bool()? },
        3 => Kind::Float { bits: d.r.u8()? },
        4 => Kind::Char,
        5 => Kind::Symbol,
        6 => Kind::Duration,
        7 => Kind::Instant,
        8 => Kind::Timestamp,
        9 => Kind::String,
        10 => Kind::Bytes,
        11 => {
            let size = d.r.u32()?;
            let fs = fields(d)?;
            Kind::Record { size, fields: fs, nominal: d.r.bool()?, tuple: d.r.bool()? }
        }
        12 => {
            let size = d.r.u32()?;
            let n = d.r.u32()?;
            let mut variants = vec![];
            for _ in 0..n {
                let name = d.r.str()?;
                variants.push(Variant { name, fields: fields(d)? });
            }
            Kind::Enum { size, variants }
        }
        13 => {
            let size = d.r.u32()?;
            Kind::Closure { size, captures: fields(d)? }
        }
        14 => Kind::Fn,
        15 => Kind::Dynamic,
        16 => Kind::List { elem: d.desc()? },
        17 => Kind::Map { key: d.desc()?, val: d.desc()? },
        18 => Kind::Set { elem: d.desc()? },
        19 => Kind::Range { elem: d.desc()? },
        20 => {
            let n = d.r.u32()?;
            let args = (0..n).map(|_| d.desc()).collect::<Result<_, _>>()?;
            Kind::Opaque { trace: None, finalize: None, args }
        }
        t => return Err(format!("bad descriptor kind {t}")),
    })
}
