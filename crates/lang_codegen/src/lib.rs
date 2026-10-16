//! MIR → Cranelift IR, compiled in-process with `cranelift-jit`.
//!
//! Calling convention for every compiled function:
//! `fn(ctx: *mut TaskCtx, env: closure-or-null, params...) -> ret`.
//!
//! GC roots: a function allocates a shadow-stack frame (`prev`, `len`,
//! slots) in its native stack frame and links it into `ctx.shadow`, with a
//! slot for each pointer local live across a safe point (call, allocation,
//! poll; see `lang_mir::roots`). Every assignment to such a local also
//! stores the pointer into its slot, so the nonmoving collector sees
//! precise roots whenever it can run. A function without such locals
//! links no frame.
//!
//! Closures used as values get a word adapter
//! `fn(ctx, closure, args: *const u64) -> u64` so the runtime can call them.
#![allow(clippy::type_complexity)]

use cranelift_codegen::ir::ArgumentPurpose;
use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{
    AbiParam, Block as CBlock, InstBuilder, MemFlagsData, Signature, StackSlotData, StackSlotKind, TrapCode,
    Type, Value, types,
};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::Variable;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataDescription, DataId, FuncId as CFuncId, Linkage, Module, default_libcall_names};
use index_vec::IndexVec;
use lang_mir::{
    BinOp, BlockId, Callee, CmpOp, Const, FfiArg, Func, FuncId, Local, NumTy, Operand, Program, Repr, Rvalue,
    Stmt, Term,
};
use rustc_hash::FxHashMap;

pub use cranelift_codegen::Context;

pub mod debuginfo;
pub use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};

#[derive(Debug, thiserror::Error)]
pub enum CodegenError {
    #[error("unsupported host: {0}")]
    Host(String),
    #[error("{0}")]
    Module(String),
    #[error(transparent)]
    Settings(#[from] settings::SetError),
}

impl From<cranelift_module::ModuleError> for CodegenError {
    fn from(e: cranelift_module::ModuleError) -> Self {
        Self::Module(format!("{e:?}"))
    }
}

pub struct Jit {
    pub module: JITModule,
    pub ctx: Context,
    pub fn_ctx: FunctionBuilderContext,
}

impl Jit {
    pub fn new(optimize: bool) -> Result<Self, CodegenError> {
        let mut flags = settings::builder();
        flags.set("use_colocated_libcalls", "false")?;
        flags.set("is_pic", "false")?;
        flags.set("opt_level", if optimize { "speed" } else { "none" })?;
        // frame-pointer chains for profilers and debuggers
        flags.set("preserve_frame_pointers", "true")?;
        let isa = cranelift_native::builder()
            .map_err(|e| CodegenError::Host(e.to_string()))?
            .finish(settings::Flags::new(flags))
            .map_err(|e| CodegenError::Host(e.to_string()))?;
        let mut builder = JITBuilder::with_isa(isa, default_libcall_names());
        for (name, ptr) in lang_runtime::symbols() {
            builder.symbol(name, ptr);
        }
        let module = JITModule::new(builder);
        Ok(Self { ctx: module.make_context(), module, fn_ctx: FunctionBuilderContext::new() })
    }
}

/// A compiled program ready to run.
pub struct Compiled {
    pub jit: Jit,
    pub funcs: IndexVec<FuncId, CFuncId>,
    pub adapters: FxHashMap<FuncId, CFuncId>,
    pub globals: Option<DataId>,
    pub n_globals: u32,
    /// Code size and display name of each defined function.
    pub code: Vec<(CFuncId, usize, String)>,
}

impl Compiled {
    pub fn func_ptr(&self, f: FuncId) -> *const u8 {
        self.jit.module.get_finalized_function(self.funcs[f])
    }

    pub fn adapter_ptr(&self, f: FuncId) -> Option<*const u8> {
        self.adapters.get(&f).map(|id| self.jit.module.get_finalized_function(*id))
    }
}

fn cty(r: Repr) -> Type {
    match r {
        Repr::I8 => types::I8,
        Repr::I16 => types::I16,
        Repr::I32 => types::I32,
        Repr::I64 | Repr::Ptr => types::I64,
        Repr::F32 => types::F32,
        Repr::F64 => types::F64,
    }
}

const HDR: i32 = 16;

/// Whether `name` is a core runtime function code generation knows (used to
/// validate loaded programs).
pub fn is_runtime_function(name: &str) -> bool {
    runtime_sig_of(name).is_some()
}

/// Native signatures of core runtime functions: (param count after ctx,
/// returns a value).
fn runtime_sig(name: &str) -> (usize, bool) {
    runtime_sig_of(name).unwrap_or_else(|| panic!("unknown runtime function {name}"))
}

fn runtime_sig_of(name: &str) -> Option<(usize, bool)> {
    Some(match name {
        "lang_rt_alloc" => (2, true),
        "lang_rt_panic" | "lang_rt_defect" => (2, true),
        "lang_rt_precondition" => (3, true),
        "lang_rt_contract_site" => (1, false),
        "lang_rt_poll" => (0, false),
        "lang_rt_take_aux" => (0, true),
        "lang_rt_global_init" => (1, true),
        "lang_rt_global_done" => (1, false),
        "lang_rt_concat" => (2, true),
        "lang_rt_eq" | "lang_rt_cmp" => (3, true),
        "lang_rt_display" => (2, true),
        "lang_rt_list_new" | "lang_rt_list_len" | "lang_rt_map_new" | "lang_rt_list_page_cap" => (1, true),
        "lang_rt_list_page" => (2, true),
        "lang_rt_list_push" => (2, false),
        "lang_rt_list_get" | "lang_rt_map_index" => (3, true),
        "lang_rt_list_finish" => (2, true),
        "lang_rt_map_insert_mut" => (3, false),
        "lang_rt_caller_loc" => (1, false),
        "lang_rt_stack_overflow" => (0, true),
        "lang_rt_ffi_call" => (4, true),
        "lang_rt_err_origin" => (3, false),
        _ => return None,
    })
}

pub fn compile(prog: &Program, optimize: bool) -> Result<Compiled, CodegenError> {
    let mut jit = Jit::new(optimize)?;
    let call_conv = jit.module.target_config().default_call_conv;
    // declare functions
    let mut funcs: IndexVec<FuncId, CFuncId> = IndexVec::new();
    for (id, f) in prog.funcs.iter_enumerated() {
        let sig = func_sig(&jit, f);
        let name = format!("f{}_{}", id.index(), sanitize(&f.name));
        funcs.push(jit.module.declare_function(&name, Linkage::Local, &sig)?);
    }
    let mut adapters = FxHashMap::default();
    for (id, f) in prog.funcs.iter_enumerated() {
        if f.needs_adapter {
            let mut sig = jit.module.make_signature();
            sig.params.extend([AbiParam::new(types::I64); 3]);
            sig.returns.push(AbiParam::new(types::I64));
            let name = format!("a{}_{}", id.index(), sanitize(&f.name));
            adapters.insert(id, jit.module.declare_function(&name, Linkage::Local, &sig)?);
        }
    }
    let globals = if prog.n_globals > 0 {
        let d = jit.module.declare_data("lang_globals", Linkage::Local, true, false)?;
        let mut desc = DataDescription::new();
        desc.define_zeroinit(prog.n_globals as usize * 8);
        desc.set_align(8);
        jit.module.define_data(d, &desc)?;
        Some(d)
    } else {
        None
    };
    let mut imports: FxHashMap<String, CFuncId> = FxHashMap::default();
    let mut code = vec![];
    let mut debug: Vec<(CFuncId, debuginfo::FuncDebug)> = vec![];
    for (id, f) in prog.funcs.iter_enumerated() {
        let mut fx =
            FnCx { prog, funcs: &funcs, adapters: &adapters, imports: &mut imports, globals, call_conv };
        let (size, dbg) = fx.define(&mut jit, id, f)?;
        code.push((funcs[id], size, f.name.clone()));
        if let Some(d) = dbg {
            debug.push((funcs[id], d));
        }
    }
    for (id, f) in prog.funcs.iter_enumerated() {
        if let Some(&aid) = adapters.get(&id) {
            let (size, dbg) = define_adapter(&mut jit, funcs[id], aid, f)?;
            code.push((aid, size, format!("{} (adapter)", f.name)));
            if let Some(d) = dbg {
                debug.push((aid, d));
            }
        }
    }
    jit.module.finalize_definitions()?;
    for (id, size, name) in &code {
        lang_runtime::profile::register_code(jit.module.get_finalized_function(*id), *size, name);
    }
    if !debug.is_empty() {
        let funcs: Vec<(debuginfo::FuncDebug, u64)> = debug
            .into_iter()
            .map(|(id, d)| {
                let a = jit.module.get_finalized_function(id) as u64;
                (d, a)
            })
            .collect();
        if let Err(e) = debuginfo::register(&prog.debug_files, &funcs, jit.module.isa()) {
            eprintln!("warning: {e}");
        }
    }
    let compiled = Compiled { jit, funcs, adapters, globals, n_globals: prog.n_globals, code };
    // patch static closures with code and adapter addresses
    for &(obj, fid) in &prog.static_closures {
        let code = compiled.func_ptr(fid) as u64;
        let adapter = compiled.adapter_ptr(fid).unwrap_or(std::ptr::null()) as u64;
        unsafe {
            let p = lang_runtime::gc::obj(obj).payload();
            *(p as *mut u64) = code;
            *(p.add(8) as *mut u64) = adapter;
        }
    }
    for &(desc, fid) in &prog.display_hooks {
        if let Some(a) = compiled.adapter_ptr(fid) {
            desc.set_display(unsafe { std::mem::transmute::<*const u8, lang_runtime::Adapter>(a) });
        }
    }
    // global value slots holding managed pointers are GC roots (odd slots
    // are init flags; integer and float values are not pointers)
    if let Some(d) = globals {
        let (ptr, _) = compiled.jit.module.get_finalized_data(d);
        for &i in &prog.ptr_globals {
            lang_runtime::gc::register_global(unsafe { (ptr as *mut u64).add(i as usize) });
        }
    }
    Ok(compiled)
}

fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect()
}

fn func_sig(jit: &Jit, f: &Func) -> Signature {
    let mut sig = jit.module.make_signature();
    sig.params.push(AbiParam::new(types::I64)); // ctx
    sig.params.push(AbiParam::new(types::I64)); // env
    for p in &f.params {
        sig.params.push(AbiParam::new(cty(f.locals[*p].repr)));
    }
    sig.returns.push(AbiParam::new(cty(f.ret)));
    sig
}

fn define_adapter(
    jit: &mut Jit,
    target: CFuncId,
    aid: CFuncId,
    f: &Func,
) -> Result<(usize, Option<debuginfo::FuncDebug>), CodegenError> {
    let mut sig = jit.module.make_signature();
    sig.params.extend([AbiParam::new(types::I64); 3]);
    sig.returns.push(AbiParam::new(types::I64));
    jit.ctx.func.signature = sig;
    let fref = jit.module.declare_func_in_func(target, &mut jit.ctx.func);
    let fe = jit.module.isa().frontend_config();
    {
        let mut b = FunctionBuilder::new(&mut jit.ctx.func, &mut jit.fn_ctx);
        let entry = b.create_block();
        b.append_block_params_for_function_params(entry);
        b.switch_to_block(entry);
        let ps = b.block_params(entry).to_vec();
        let mut args = vec![ps[0], ps[1]];
        for (i, p) in f.params.iter().enumerate() {
            let w = b.ins().load(types::I64, MemFlagsData::trusted(), ps[2], (i * 8) as i32);
            args.push(from_word(&mut b, w, f.locals[*p].repr));
        }
        let call = b.ins().call(fref, &args);
        let r = b.inst_results(call)[0];
        let w = to_word(&mut b, r, f.ret);
        b.ins().return_(&[w]);
        b.seal_all_blocks();
        b.finalize(fe);
    }
    jit.module.define_function(aid, &mut jit.ctx)?;
    let size = code_size(&jit.ctx);
    // a named frame with unwind information, so backtraces pass through
    let dbg = debug_info().then(|| debuginfo::FuncDebug {
        name: format!("{} (adapter)", f.name),
        size: size as u32,
        lines: vec![],
        vars: vec![],
        unwind: jit.ctx.compiled_code().and_then(|c| c.create_unwind_info(jit.module.isa()).ok().flatten()),
    });
    jit.module.clear_context(&mut jit.ctx);
    Ok((size, dbg))
}

/// `LANG_DUMP_ASM=name,...` selects functions whose name contains `name`.
fn f_matches(func: &str, pat: &str) -> bool {
    !pat.is_empty() && func.contains(pat)
}

/// Line rows, named locals' locations, and unwind information of the
/// function just compiled in `jit.ctx`.
fn func_debug(
    jit: &Jit,
    f: &Func,
    size: usize,
    locs: &[(u32, u32, u32)],
    layout: Option<&(cranelift_codegen::ir::StackSlot, FxHashMap<Local, i32>)>,
) -> debuginfo::FuncDebug {
    let isa = jit.module.isa();
    let Some(code) = jit.ctx.compiled_code() else {
        return debuginfo::FuncDebug {
            name: f.name.clone(),
            size: size as u32,
            lines: vec![],
            vars: vec![],
            unwind: None,
        };
    };
    let mut lines = vec![];
    for sl in code.buffer.get_srclocs_sorted() {
        if sl.loc.is_default() {
            continue;
        }
        if let Some(&(file, line, col)) = locs.get(sl.loc.bits() as usize - 1)
            && lines.last().is_none_or(|l: &(u32, u32, u32, u32)| (l.1, l.2, l.3) != (file, line, col))
        {
            lines.push((sl.start, file, line, col));
        }
    }
    // named copies of the parameters, made on entry, stand for them
    let mut params: Vec<Local> = f.params.clone();
    if let Some(entry) = f.blocks.iter().next() {
        for st in &entry.stmts {
            if let Stmt::Assign(d, Rvalue::Use(Operand::Local(src))) = st
                && f.params.contains(src)
            {
                params.push(*d);
            }
        }
    }
    // each named local's slot, as an offset from the frame pointer
    let mut vars = vec![];
    if let (Some((slot, offsets)), Some(fl)) = (layout, code.buffer.frame_layout()) {
        let base = fl.stackslots[*slot].offset as i64 - fl.frame_to_fp_offset as i64;
        for (l, off) in offsets {
            let decl = &f.locals[*l];
            let Some(name) = decl.name.clone() else { continue };
            vars.push(debuginfo::VarDebug {
                name,
                repr: decl.repr,
                param: params.contains(l),
                fp_offset: base + *off as i64,
            });
        }
    }
    vars.sort_by_key(|v| (!v.param, v.fp_offset));
    let unwind = code.create_unwind_info(isa).ok().flatten();
    debuginfo::FuncDebug { name: f.name.clone(), size: size as u32, lines, vars, unwind }
}

fn code_size(ctx: &Context) -> usize {
    ctx.compiled_code().map_or(0, |c| c.code_info().total_size as usize)
}

fn from_word(b: &mut FunctionBuilder, w: Value, r: Repr) -> Value {
    match r {
        Repr::I64 | Repr::Ptr => w,
        Repr::I8 | Repr::I16 | Repr::I32 => b.ins().ireduce(cty(r), w),
        Repr::F64 => b.ins().bitcast(types::F64, MemFlagsData::new(), w),
        Repr::F32 => {
            let n = b.ins().ireduce(types::I32, w);
            b.ins().bitcast(types::F32, MemFlagsData::new(), n)
        }
    }
}

fn to_word(b: &mut FunctionBuilder, v: Value, r: Repr) -> Value {
    let t = b.func.dfg.value_type(v);
    match r {
        _ if t == types::I64 => v,
        Repr::F64 => b.ins().bitcast(types::I64, MemFlagsData::new(), v),
        Repr::F32 => {
            let n = b.ins().bitcast(types::I32, MemFlagsData::new(), v);
            b.ins().uextend(types::I64, n)
        }
        _ if t == types::F64 => b.ins().bitcast(types::I64, MemFlagsData::new(), v),
        _ if t == types::F32 => {
            let n = b.ins().bitcast(types::I32, MemFlagsData::new(), v);
            b.ins().uextend(types::I64, n)
        }
        _ => b.ins().uextend(types::I64, v),
    }
}

/// Convert a value to type `to` (integer widening/narrowing, or bitcast for
/// equal sizes). Used to reconcile constants and unit placeholders.
fn coerce(b: &mut FunctionBuilder, v: Value, to: Type) -> Value {
    let from = b.func.dfg.value_type(v);
    if from == to {
        return v;
    }
    if from.is_int() && to.is_int() {
        return if from.bits() > to.bits() { b.ins().ireduce(to, v) } else { b.ins().uextend(to, v) };
    }
    if from.bits() == to.bits() {
        return b.ins().bitcast(to, MemFlagsData::new(), v);
    }
    if from.is_int() && to.is_float() {
        let n = coerce(b, v, if to == types::F32 { types::I32 } else { types::I64 });
        return b.ins().bitcast(to, MemFlagsData::new(), n);
    }
    if from.is_float() && to.is_int() {
        let n =
            b.ins().bitcast(if from == types::F32 { types::I32 } else { types::I64 }, MemFlagsData::new(), v);
        return coerce(b, n, to);
    }
    if to.is_float() {
        return if to == types::F32 { b.ins().f32const(0.0) } else { b.ins().f64const(0.0) };
    }
    b.ins().iconst(to, 0)
}

struct FnCx<'p> {
    prog: &'p Program,
    funcs: &'p IndexVec<FuncId, CFuncId>,
    adapters: &'p FxHashMap<FuncId, CFuncId>,
    imports: &'p mut FxHashMap<String, CFuncId>,
    globals: Option<DataId>,
    call_conv: cranelift_codegen::isa::CallConv,
}

struct Body<'f> {
    f: &'f Func,
    vars: IndexVec<Local, Variable>,
    slots: FxHashMap<Local, i32>,
    frame: cranelift_codegen::ir::StackSlot,
    ctx: Value,
    blocks: IndexVec<BlockId, CBlock>,
    /// Debug info: the source positions met (`SourceLoc` n is entry
    /// n - 1), and a stack slot holding every named local (each
    /// assignment also stores there), so a debugger finds them at fixed
    /// frame offsets throughout the function.
    debug: bool,
    locs: Vec<(u32, u32, u32)>,
    dbg_slot: Option<cranelift_codegen::ir::StackSlot>,
    dbg_offsets: FxHashMap<Local, i32>,
}

static DEBUG_INFO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Generate debug information (line tables, locals, unwind) and register
/// it with debuggers (see [`debuginfo`]).
pub fn set_debug_info(on: bool) {
    DEBUG_INFO.store(on, std::sync::atomic::Ordering::SeqCst);
}

fn debug_info() -> bool {
    DEBUG_INFO.load(std::sync::atomic::Ordering::SeqCst) || std::env::var_os("LANG_DEBUG_INFO").is_some()
}

impl FnCx<'_> {
    fn import(
        &mut self,
        jit: &mut Jit,
        name: &str,
        nparams: usize,
        ret: bool,
    ) -> Result<CFuncId, CodegenError> {
        if let Some(id) = self.imports.get(name) {
            return Ok(*id);
        }
        let mut sig = jit.module.make_signature();
        sig.params.extend(std::iter::repeat_n(AbiParam::new(types::I64), nparams + 1));
        if ret {
            sig.returns.push(AbiParam::new(types::I64));
        }
        let id = jit.module.declare_function(name, Linkage::Import, &sig)?;
        self.imports.insert(name.to_string(), id);
        Ok(id)
    }

    fn define(
        &mut self,
        jit: &mut Jit,
        id: FuncId,
        f: &Func,
    ) -> Result<(usize, Option<debuginfo::FuncDebug>), CodegenError> {
        jit.ctx.func.signature = func_sig(jit, f);
        let debug = debug_info();
        if debug {
            jit.ctx.func.collect_debug_info();
        }
        let mut src_locs: Vec<(u32, u32, u32)> = vec![];
        let dbg_layout: Option<(cranelift_codegen::ir::StackSlot, FxHashMap<Local, i32>)>;
        // pre-declare imports used by this function (needs &mut jit before the builder)
        let mut foreign: FxHashMap<String, cranelift_codegen::ir::FuncRef> = FxHashMap::default();
        let mut used: Vec<(String, usize, bool)> = vec![
            ("lang_rt_poll".into(), 0, false),
            ("lang_rt_defect".into(), 2, true),
            ("lang_rt_alloc".into(), 2, true),
            ("lang_rt_concat".into(), 2, true),
            ("lang_rt_stack_overflow".into(), 0, true),
        ];
        if self.prog.workers > 1 {
            used.push(("lang_rt_enter".into(), 1, false));
            for n in ["lang_rt_leave", "lang_rt_safe_begin", "lang_rt_safe_end"] {
                used.push((n.into(), 0, false));
            }
        }
        for b in &f.blocks {
            for s in &b.stmts {
                if let Stmt::Assign(_, Rvalue::Call { callee, args, .. }) = s {
                    match callee {
                        Callee::Runtime(n) => {
                            let (np, r) = runtime_sig(n);
                            used.push((n.to_string(), np, r));
                        }
                        Callee::Intrinsic { name, .. } => used.push((name.clone(), args.len() + 2, true)),
                        Callee::Foreign { name, params, ret, contract } => {
                            if *contract != 0 {
                                used.push(("lang_rt_ffi_call".into(), 4, true));
                            }
                            if !foreign.contains_key(name) {
                                let mut sig = jit.module.make_signature();
                                if let Some(FfiArg::Memory { .. }) = ret {
                                    sig.params
                                        .push(AbiParam::special(types::I64, ArgumentPurpose::StructReturn));
                                }
                                for (i, a) in args.iter().enumerate() {
                                    match params.get(i) {
                                        Some(FfiArg::Regs { sse, .. }) => {
                                            for x in sse {
                                                sig.params.push(AbiParam::new(if *x {
                                                    types::F64
                                                } else {
                                                    types::I64
                                                }));
                                            }
                                        }
                                        Some(FfiArg::Memory { size, .. }) => {
                                            sig.params.push(AbiParam::special(
                                                types::I64,
                                                ArgumentPurpose::StructArgument(*size),
                                            ));
                                        }
                                        _ => {
                                            let r = match a {
                                                Operand::Local(l) => f.locals[*l].repr,
                                                Operand::Const(Const::Int(_, r)) => *r,
                                                Operand::Const(Const::F32(_)) => Repr::F32,
                                                Operand::Const(Const::F64(_)) => Repr::F64,
                                                Operand::Const(Const::Addr(_)) => Repr::I64,
                                            };
                                            let r = match params.get(i) {
                                                Some(FfiArg::Scalar(r)) => *r,
                                                _ => r,
                                            };
                                            sig.params.push(AbiParam::new(cty(r)));
                                        }
                                    }
                                }
                                match ret {
                                    Some(FfiArg::Regs { sse, .. }) => {
                                        for x in sse {
                                            sig.returns.push(AbiParam::new(if *x {
                                                types::F64
                                            } else {
                                                types::I64
                                            }));
                                        }
                                    }
                                    Some(FfiArg::Memory { .. }) => {}
                                    _ => {
                                        if let Stmt::Assign(dst, _) = s {
                                            sig.returns.push(AbiParam::new(cty(f.locals[*dst].repr)));
                                        }
                                    }
                                }
                                let id = jit.module.declare_function(name, Linkage::Import, &sig)?;
                                let fref = jit.module.declare_func_in_func(id, &mut jit.ctx.func);
                                foreign.insert(name.clone(), fref);
                                if params.iter().chain(ret.iter()).any(|p| !matches!(p, FfiArg::Scalar(_))) {
                                    used.push(("lang_rt_ffi_pack".into(), 3, false));
                                    used.push(("lang_rt_ffi_unpack".into(), 2, true));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        let mut import_refs: FxHashMap<String, cranelift_codegen::ir::FuncRef> = FxHashMap::default();
        for (n, np, r) in used {
            let cid = self.import(jit, &n, np, r)?;
            let fref = jit.module.declare_func_in_func(cid, &mut jit.ctx.func);
            import_refs.insert(n, fref);
        }
        for (n, r) in foreign {
            import_refs.insert(format!("foreign:{n}"), r);
        }
        let mut func_refs: FxHashMap<FuncId, cranelift_codegen::ir::FuncRef> = FxHashMap::default();
        let mut adapter_refs: FxHashMap<FuncId, cranelift_codegen::ir::FuncRef> = FxHashMap::default();
        for b in &f.blocks {
            for s in &b.stmts {
                if let Stmt::Assign(_, rv) = s {
                    match rv {
                        Rvalue::Call { callee: Callee::Direct(t), .. } | Rvalue::FuncAddr(t) => {
                            if !func_refs.contains_key(t) {
                                let r = jit.module.declare_func_in_func(self.funcs[*t], &mut jit.ctx.func);
                                func_refs.insert(*t, r);
                            }
                        }
                        Rvalue::AdapterAddr(t) => {
                            if let Some(a) = self.adapters.get(t)
                                && !adapter_refs.contains_key(t)
                            {
                                let r = jit.module.declare_func_in_func(*a, &mut jit.ctx.func);
                                adapter_refs.insert(*t, r);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        let globals_gv = self.globals.map(|d| jit.module.declare_data_in_func(d, &mut jit.ctx.func));
        let fe = jit.module.isa().frontend_config();
        // root slots only for pointers live across a safe point; no frame
        // at all when there are none
        let ptr_locals: Vec<Local> = lang_mir::roots::rooted_locals(f);
        let framed = !ptr_locals.is_empty();
        {
            let mut b = FunctionBuilder::new(&mut jit.ctx.func, &mut jit.fn_ctx);
            let frame_size = (2 + ptr_locals.len() as u32) * 8;
            let frame =
                b.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, frame_size, 3));
            let mut vars = IndexVec::new();
            for (_, d) in f.locals.iter_enumerated() {
                vars.push(b.declare_var(cty(d.repr)));
            }
            let mut slots = FxHashMap::default();
            for (i, l) in ptr_locals.iter().enumerate() {
                slots.insert(*l, 16 + 8 * i as i32);
            }
            let blocks: IndexVec<BlockId, CBlock> = f.blocks.iter().map(|_| b.create_block()).collect();
            let entry = b.create_block();
            b.append_block_params_for_function_params(entry);
            b.switch_to_block(entry);
            let ps = b.block_params(entry).to_vec();
            let ctx = ps[0];
            // initialize every variable (Cranelift requires definitions)
            for (l, d) in f.locals.iter_enumerated() {
                let z = match d.repr {
                    Repr::F32 => b.ins().f32const(0.0),
                    Repr::F64 => b.ins().f64const(0.0),
                    r => b.ins().iconst(cty(r), 0),
                };
                b.def_var(vars[l], z);
            }
            // stack check: past the task's limit is a stack overflow defect
            // (a limit of 0 means unchecked)
            {
                let limit = b.ins().load(types::I64, MemFlagsData::trusted(), ctx, 32);
                let sp = b.ins().get_stack_pointer(types::I64);
                let over = b.ins().icmp(IntCC::UnsignedLessThan, sp, limit);
                let (fail, cont) = (b.create_block(), b.create_block());
                b.ins().brif(over, fail, &[], cont, &[]);
                b.set_cold_block(fail);
                b.switch_to_block(fail);
                b.seal_block(fail);
                b.ins().call(import_refs["lang_rt_stack_overflow"], &[ctx]);
                b.ins().trap(TrapCode::unwrap_user(1));
                b.switch_to_block(cont);
                b.seal_block(cont);
            }
            // link the shadow frame
            if framed {
                let prev = b.ins().load(types::I64, MemFlagsData::trusted(), ctx, 0);
                b.ins().stack_store(types::I64, prev, frame, 0);
                let n = b.ins().iconst(types::I64, ptr_locals.len() as i64);
                b.ins().stack_store(types::I64, n, frame, 8);
                let zero = b.ins().iconst(types::I64, 0);
                for i in 0..ptr_locals.len() {
                    b.ins().stack_store(types::I64, zero, frame, 16 + 8 * i as i32);
                }
                let faddr = b.ins().stack_addr(types::I64, frame, 0);
                b.ins().store(MemFlagsData::trusted(), faddr, ctx, 0);
            }
            let debug = debug_info();
            let mut dbg_offsets = FxHashMap::default();
            let dbg_slot = if debug {
                for (l, d) in f.locals.iter_enumerated() {
                    if d.name.as_ref().is_some_and(|n| !n.starts_with('$')) && l != f.env {
                        dbg_offsets.insert(l, 8 * dbg_offsets.len() as i32);
                    }
                }
                let size = (8 * dbg_offsets.len() as u32).max(8);
                Some(b.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, size, 3)))
            } else {
                None
            };
            dbg_layout = dbg_slot.map(|s| (s, dbg_offsets.clone()));
            let mut body =
                Body { f, vars, slots, frame, ctx, blocks, debug, locs: vec![], dbg_slot, dbg_offsets };
            // debug slots start zeroed (a local not yet assigned reads 0)
            if let Some(slot) = body.dbg_slot {
                let zero = b.ins().iconst(types::I64, 0);
                for off in body.dbg_offsets.values() {
                    b.ins().stack_store(types::I64, zero, slot, *off);
                }
            }
            // parameters
            body.set(&mut b, f.env, ps[1]);
            for (i, p) in f.params.iter().enumerate() {
                body.set(&mut b, *p, ps[2 + i]);
            }
            b.ins().jump(body.blocks[BlockId::new(0)], &[]);
            for (bid, blk) in f.blocks.iter_enumerated() {
                b.switch_to_block(body.blocks[bid]);
                for s in &blk.stmts {
                    self.stmt(&mut b, &mut body, s, &import_refs, &func_refs, &adapter_refs, globals_gv);
                }
                match blk.term.as_ref().unwrap_or(&Term::Unreachable) {
                    Term::Goto(t) => {
                        b.ins().jump(body.blocks[*t], &[]);
                    }
                    Term::Branch(c, t, e) => {
                        let v = body.op(&mut b, c);
                        b.ins().brif(v, body.blocks[*t], &[], body.blocks[*e], &[]);
                    }
                    Term::Switch(v, cases, default) => {
                        let x = body.op(&mut b, v);
                        let ty = b.func.dfg.value_type(x);
                        let mut sw = cranelift_frontend::Switch::new();
                        for (k, t) in cases {
                            let k = if ty.bits() < 64 {
                                (*k as u64) & ((1u64 << ty.bits()) - 1)
                            } else {
                                *k as u64
                            };
                            sw.set_entry(k as u128, body.blocks[*t]);
                        }
                        sw.emit(&mut b, x, body.blocks[*default]);
                    }
                    Term::Return(v) => {
                        let x = body.op(&mut b, v);
                        let x = coerce(&mut b, x, cty(f.ret));
                        if framed {
                            let prev = b.ins().stack_load(types::I64, types::I64, body.frame, 0);
                            b.ins().store(MemFlagsData::trusted(), prev, body.ctx, 0);
                        }
                        b.ins().return_(&[x]);
                    }
                    Term::Unreachable => {
                        b.ins().trap(TrapCode::unwrap_user(1));
                    }
                }
            }
            b.seal_all_blocks();
            b.finalize(fe);
            src_locs = std::mem::take(&mut body.locs);
        }
        if std::env::var("LANG_DUMP_CLIF").is_ok() {
            eprintln!("{}", jit.ctx.func.display());
        }
        let asm = std::env::var("LANG_DUMP_ASM")
            .ok()
            .filter(|pat| pat.is_empty() || pat == "1" || pat.split(',').any(|n| f_matches(&f.name, n)));
        jit.ctx.set_disasm(asm.is_some());
        jit.module
            .define_function(self.funcs[id], &mut jit.ctx)
            .map_err(|e| CodegenError::Module(format!("{}: {e:?}\n{}", f.name, jit.ctx.func.display())))?;
        let size = code_size(&jit.ctx);
        if asm.is_some()
            && let Some(code) = jit.ctx.compiled_code()
            && let Some(text) = &code.vcode
        {
            eprintln!("; {} ({size} bytes)\n{text}", f.name);
        }
        let dbg = if debug { Some(func_debug(jit, f, size, &src_locs, dbg_layout.as_ref())) } else { None };
        jit.module.clear_context(&mut jit.ctx);
        let _ = self.call_conv;
        Ok((size, dbg))
    }

    #[allow(clippy::too_many_arguments)]
    fn stmt(
        &mut self,
        b: &mut FunctionBuilder,
        body: &mut Body,
        s: &Stmt,
        imports: &FxHashMap<String, cranelift_codegen::ir::FuncRef>,
        funcs: &FxHashMap<FuncId, cranelift_codegen::ir::FuncRef>,
        adapters: &FxHashMap<FuncId, cranelift_codegen::ir::FuncRef>,
        globals: Option<cranelift_codegen::ir::GlobalValue>,
    ) {
        match s {
            Stmt::Assign(l, rv) => {
                let ty = cty(body.f.locals[*l].repr);
                let v = self.rvalue(b, body, rv, ty, imports, funcs, adapters, globals);
                let v = coerce(b, v, ty);
                body.set(b, *l, v);
            }
            Stmt::Store { base, offset, repr, value } => {
                let p = body.op(b, base);
                let v = body.op(b, value);
                let v = coerce(b, v, cty(*repr));
                b.ins().store(MemFlagsData::trusted(), v, p, HDR + offset);
            }
            Stmt::GlobalStore { slot, value } => {
                let v = body.op(b, value);
                let w = to_word(b, v, Repr::I64);
                let g = b.ins().symbol_value(types::I64, globals.unwrap());
                b.ins().store(MemFlagsData::trusted(), w, g, (*slot * 8) as i32);
            }
            Stmt::Loc { file, line, col } => {
                if body.debug {
                    body.locs.push((*file, *line, *col));
                    b.set_srcloc(cranelift_codegen::ir::SourceLoc::new(body.locs.len() as u32));
                }
            }
            Stmt::Poll => {
                let budget = b.ins().load(types::I64, MemFlagsData::trusted(), body.ctx, 8);
                let nb = b.ins().iadd_imm_s(budget, -1);
                b.ins().store(MemFlagsData::trusted(), nb, body.ctx, 8);
                let neg = b.ins().icmp_imm_s(IntCC::SignedLessThan, nb, 0);
                let (slow, cont) = (b.create_block(), b.create_block());
                b.set_cold_block(slow);
                b.ins().brif(neg, slow, &[], cont, &[]);
                b.switch_to_block(slow);
                self.rt_call(b, imports, "lang_rt_poll", &[body.ctx], body.ctx);
                b.ins().jump(cont, &[]);
                b.switch_to_block(cont);
            }
        }
    }

    fn defect_if(
        &self,
        b: &mut FunctionBuilder,
        body: &Body,
        cond: Value,
        kind: i64,
        loc: u64,
        imports: &FxHashMap<String, cranelift_codegen::ir::FuncRef>,
    ) {
        let (bad, ok) = (b.create_block(), b.create_block());
        b.set_cold_block(bad);
        b.ins().brif(cond, bad, &[], ok, &[]);
        b.switch_to_block(bad);
        let k = b.ins().iconst(types::I64, kind);
        let l = b.ins().iconst(types::I64, loc as i64);
        self.rt_call(b, imports, "lang_rt_defect", &[body.ctx, k, l], body.ctx);
        b.ins().trap(TrapCode::unwrap_user(2));
        b.switch_to_block(ok);
    }

    #[allow(clippy::too_many_arguments)]
    fn rvalue(
        &mut self,
        b: &mut FunctionBuilder,
        body: &mut Body,
        rv: &Rvalue,
        dest: Type,
        imports: &FxHashMap<String, cranelift_codegen::ir::FuncRef>,
        funcs: &FxHashMap<FuncId, cranelift_codegen::ir::FuncRef>,
        adapters: &FxHashMap<FuncId, cranelift_codegen::ir::FuncRef>,
        globals: Option<cranelift_codegen::ir::GlobalValue>,
    ) -> Value {
        match rv {
            Rvalue::Use(o) => body.op(b, o),
            Rvalue::Bin { op, a, b: rhs, ty, loc } => {
                let x = body.op(b, a);
                let y = body.op(b, rhs);
                match ty {
                    NumTy::F(bits) => {
                        let t = if *bits == 32 { types::F32 } else { types::F64 };
                        let (x, y) = (coerce(b, x, t), coerce(b, y, t));
                        match op {
                            BinOp::Add => b.ins().fadd(x, y),
                            BinOp::Sub => b.ins().fsub(x, y),
                            BinOp::Mul => b.ins().fmul(x, y),
                            BinOp::Div => b.ins().fdiv(x, y),
                            _ => {
                                // remainder: x - trunc(x / y) * y
                                let q = b.ins().fdiv(x, y);
                                let t = b.ins().trunc(q);
                                let m = b.ins().fmul(t, y);
                                b.ins().fsub(x, m)
                            }
                        }
                    }
                    NumTy::I(bits, signed) => {
                        let t = Type::int(*bits as u16).unwrap();
                        let (x, y) = (coerce(b, x, t), coerce(b, y, t));
                        let checked = *loc != 0;
                        self.int_bin(b, body, *op, x, y, *bits, *signed, checked, *loc, imports)
                    }
                }
            }
            Rvalue::Neg { a, ty, loc } => {
                let x = body.op(b, a);
                match ty {
                    NumTy::F(_) => b.ins().fneg(x),
                    NumTy::I(bits, signed) => {
                        let t = Type::int(*bits as u16).unwrap();
                        let x = coerce(b, x, t);
                        let z = b.ins().iconst(t, 0);
                        self.int_bin(b, body, BinOp::Sub, z, x, *bits, *signed, *loc != 0, *loc, imports)
                    }
                }
            }
            Rvalue::Not(a) => {
                let x = body.op(b, a);
                let x = coerce(b, x, types::I8);

                b.ins().icmp_imm_s(IntCC::Equal, x, 0)
            }
            Rvalue::Cmp { op, a, b: rhs, ty } => {
                let x = body.op(b, a);
                let y = body.op(b, rhs);
                match ty {
                    NumTy::F(bits) => {
                        let t = if *bits == 32 { types::F32 } else { types::F64 };
                        let (x, y) = (coerce(b, x, t), coerce(b, y, t));
                        let cc = match op {
                            CmpOp::Eq => FloatCC::Equal,
                            CmpOp::Ne => FloatCC::NotEqual,
                            CmpOp::Lt => FloatCC::LessThan,
                            CmpOp::Le => FloatCC::LessThanOrEqual,
                            CmpOp::Gt => FloatCC::GreaterThan,
                            CmpOp::Ge => FloatCC::GreaterThanOrEqual,
                        };
                        b.ins().fcmp(cc, x, y)
                    }
                    NumTy::I(bits, signed) => {
                        let t = Type::int(*bits as u16).unwrap();
                        let (x, y) = (coerce(b, x, t), coerce(b, y, t));
                        let cc = match (op, signed) {
                            (CmpOp::Eq, _) => IntCC::Equal,
                            (CmpOp::Ne, _) => IntCC::NotEqual,
                            (CmpOp::Lt, true) => IntCC::SignedLessThan,
                            (CmpOp::Le, true) => IntCC::SignedLessThanOrEqual,
                            (CmpOp::Gt, true) => IntCC::SignedGreaterThan,
                            (CmpOp::Ge, true) => IntCC::SignedGreaterThanOrEqual,
                            (CmpOp::Lt, false) => IntCC::UnsignedLessThan,
                            (CmpOp::Le, false) => IntCC::UnsignedLessThanOrEqual,
                            (CmpOp::Gt, false) => IntCC::UnsignedGreaterThan,
                            (CmpOp::Ge, false) => IntCC::UnsignedGreaterThanOrEqual,
                        };
                        b.ins().icmp(cc, x, y)
                    }
                }
            }
            Rvalue::Convert { a, from, to, checked, loc } => {
                let x = body.op(b, a);
                self.convert(b, body, x, *from, *to, *checked, *loc, imports)
            }
            Rvalue::Fits { a, from, to } => {
                let x = body.op(b, a);
                match (from, to) {
                    (NumTy::I(fb, fs), NumTy::I(tb, ts)) => {
                        let x = coerce(b, x, Type::int(*fb as u16).unwrap());
                        let (_, bad) = int_convert(b, x, *fb, *fs, *tb, *ts);
                        b.ins().icmp_imm_u(IntCC::Equal, bad, 0)
                    }
                    _ => b.ins().iconst(types::I8, 1),
                }
            }
            Rvalue::Overflows { op, a, b: rhs, ty } => {
                let x = body.op(b, a);
                let y = body.op(b, rhs);
                let NumTy::I(bits, signed) = *ty else {
                    return b.ins().iconst(types::I8, 0);
                };
                let t = Type::int(bits as u16).unwrap();
                let (x, y) = (coerce(b, x, t), coerce(b, y, t));
                match op {
                    BinOp::Add | BinOp::Sub | BinOp::Mul => int_overflow(b, *op, x, y, bits, signed).1,
                    _ => {
                        let z = b.ins().icmp_imm_s(IntCC::Equal, y, 0);
                        if signed && *op == BinOp::Div {
                            let min =
                                b.ins().iconst(t, if bits == 64 { i64::MIN } else { -(1i64 << (bits - 1)) });
                            let is_min = b.ins().icmp(IntCC::Equal, x, min);
                            let neg1 = b.ins().icmp_imm_s(IntCC::Equal, y, -1);
                            let both = b.ins().band(is_min, neg1);
                            b.ins().bor(z, both)
                        } else {
                            z
                        }
                    }
                }
            }
            Rvalue::Load { base, offset, repr } => {
                let p = body.op(b, base);
                b.ins().load(cty(*repr), MemFlagsData::trusted(), p, HDR + offset)
            }
            Rvalue::FromWord { a, to } => {
                let x = body.op(b, a);
                let x = coerce(b, x, types::I64);
                from_word(b, x, *to)
            }
            Rvalue::ToWord(a) => {
                let x = body.op(b, a);
                let t = b.func.dfg.value_type(x);
                let r = if t == types::F64 {
                    Repr::F64
                } else if t == types::F32 {
                    Repr::F32
                } else {
                    Repr::I64
                };
                to_word(b, x, r)
            }
            Rvalue::Tag(a) => {
                let p = body.op(b, a);
                b.ins().load(types::I32, MemFlagsData::trusted(), p, HDR)
            }
            Rvalue::Alloc { desc, size } => {
                let d = b.ins().iconst(types::I64, *desc as i64);
                let s = b.ins().iconst(types::I64, *size as i64);
                // allocation needs no runtime lock (per-worker object lists;
                // a due collection takes the lock itself)
                let call = b.ins().call(imports["lang_rt_alloc"], &[body.ctx, d, s]);
                b.inst_results(call)[0]
            }
            Rvalue::Concat(parts) => {
                let slot = b.create_sized_stack_slot(StackSlotData::new(
                    StackSlotKind::ExplicitSlot,
                    (parts.len().max(1) * 8) as u32,
                    3,
                ));
                for (i, p) in parts.iter().enumerate() {
                    let v = body.op(b, p);
                    let v = coerce(b, v, types::I64);
                    b.ins().stack_store(types::I64, v, slot, (i * 8) as i32);
                }
                let addr = b.ins().stack_addr(types::I64, slot, 0);
                let n = b.ins().iconst(types::I64, parts.len() as i64);
                let call = self.rt_call(b, imports, "lang_rt_concat", &[body.ctx, addr, n], body.ctx);
                b.inst_results(call)[0]
            }
            Rvalue::GlobalAddr(slot) => {
                let g = b.ins().symbol_value(types::I64, globals.unwrap());
                b.ins().iadd_imm_s(g, (*slot * 8) as i64)
            }
            Rvalue::GlobalLoad(slot) => {
                let g = b.ins().symbol_value(types::I64, globals.unwrap());
                let w = b.ins().load(types::I64, MemFlagsData::trusted(), g, (*slot * 8) as i32);
                let r = if dest == types::F64 {
                    Repr::F64
                } else if dest == types::F32 {
                    Repr::F32
                } else if dest == types::I64 {
                    Repr::I64
                } else if dest == types::I32 {
                    Repr::I32
                } else if dest == types::I16 {
                    Repr::I16
                } else {
                    Repr::I8
                };
                from_word(b, w, r)
            }
            Rvalue::FuncAddr(fid) => b.ins().func_addr(types::I64, funcs[fid]),
            Rvalue::AdapterAddr(fid) => match adapters.get(fid) {
                Some(r) => b.ins().func_addr(types::I64, *r),
                None => b.ins().iconst(types::I64, 0),
            },
            Rvalue::Call { callee, args, ret } => {
                let mut vals: Vec<Value> = args.iter().map(|a| body.op(b, a)).collect();
                match callee {
                    Callee::Direct(fid) => {
                        let target = &self.prog.funcs[*fid];
                        let mut cargs = vec![body.ctx, b.ins().iconst(types::I64, 0)];
                        for (v, p) in vals.iter().zip(&target.params) {
                            cargs.push(coerce(b, *v, cty(target.locals[*p].repr)));
                        }
                        let call = match (&target.live_key, self.prog.live) {
                            // live mode: dispatch through the instance's slot
                            (Some(key), true) => {
                                let slot = lang_runtime::live::slot(key) as *const _ as i64;
                                let mut sig = Signature::new(self.call_conv);
                                sig.params.push(AbiParam::new(types::I64));
                                sig.params.push(AbiParam::new(types::I64));
                                for p in &target.params {
                                    sig.params.push(AbiParam::new(cty(target.locals[*p].repr)));
                                }
                                sig.returns.push(AbiParam::new(cty(target.ret)));
                                let sref = b.import_signature(sig);
                                let addr = b.ins().iconst(types::I64, slot);
                                let code = b.ins().load(types::I64, MemFlagsData::trusted(), addr, 0);
                                b.ins().call_indirect(sref, code, &cargs)
                            }
                            _ => b.ins().call(funcs[fid], &cargs),
                        };
                        b.inst_results(call)[0]
                    }
                    Callee::Closure(c) => {
                        let clo = body.op(b, c);
                        let code = b.ins().load(types::I64, MemFlagsData::trusted(), clo, HDR);
                        let mut sig = Signature::new(self.call_conv);
                        sig.params.push(AbiParam::new(types::I64));
                        sig.params.push(AbiParam::new(types::I64));
                        for v in &vals {
                            sig.params.push(AbiParam::new(b.func.dfg.value_type(*v)));
                        }
                        sig.returns.push(AbiParam::new(cty(*ret)));
                        let sref = b.import_signature(sig);
                        let mut cargs = vec![body.ctx, clo];
                        cargs.append(&mut vals);
                        let call = b.ins().call_indirect(sref, code, &cargs);
                        b.inst_results(call)[0]
                    }
                    Callee::Runtime(name) => {
                        let (_, has_ret) = runtime_sig(name);
                        let mut cargs = vec![body.ctx];
                        for v in vals {
                            let t = b.func.dfg.value_type(v);
                            let r = if t == types::F64 {
                                Repr::F64
                            } else if t == types::F32 {
                                Repr::F32
                            } else {
                                Repr::I64
                            };
                            cargs.push(to_word(b, v, r));
                        }
                        let call = self.rt_call(b, imports, name, &cargs, body.ctx);
                        if has_ret { b.inst_results(call)[0] } else { b.ins().iconst(types::I64, 0) }
                    }
                    Callee::Foreign { name, params: _, ret: _, contract } if *contract != 0 => {
                        // a declared blocking/pinned contract: the call runs on
                        // the blocking pool or the pinned thread while this
                        // task parks (word-sized arguments and result)
                        let fref = imports[&format!("foreign:{name}")];
                        let addr = b.ins().func_addr(types::I64, fref);
                        let n = vals.len().max(1) as u32;
                        let ss = b.create_sized_stack_slot(StackSlotData::new(
                            StackSlotKind::ExplicitSlot,
                            n * 8,
                            3,
                        ));
                        for (i, v) in vals.iter().enumerate() {
                            let t = b.func.dfg.value_type(*v);
                            let w = if t == types::I64 {
                                *v
                            } else if t.is_int() {
                                b.ins().sextend(types::I64, *v)
                            } else {
                                b.ins().bitcast(types::I64, MemFlagsData::new(), *v)
                            };
                            b.ins().stack_store(types::I64, w, ss, (i * 8) as i32);
                        }
                        let args_ptr = b.ins().stack_addr(types::I64, ss, 0);
                        let nargs = b.ins().iconst(types::I64, vals.len() as i64);
                        let flags = b.ins().iconst(types::I64, *contract as i64);
                        let call = self.rt_call(
                            b,
                            imports,
                            "lang_rt_ffi_call",
                            &[body.ctx, addr, args_ptr, nargs, flags],
                            body.ctx,
                        );
                        let w = b.inst_results(call)[0];
                        let t = cty(*ret);
                        if t == types::I64 {
                            w
                        } else if t.is_int() {
                            b.ins().ireduce(t, w)
                        } else {
                            b.ins().bitcast(t, MemFlagsData::new(), w)
                        }
                    }
                    Callee::Foreign { name, params, ret: fret, .. } => {
                        let multi = self.prog.workers > 1;
                        let slot = |b: &mut FunctionBuilder, size: u32| {
                            let ss = b.create_sized_stack_slot(StackSlotData::new(
                                StackSlotKind::ExplicitSlot,
                                size.div_ceil(16) * 16,
                                4,
                            ));
                            b.ins().stack_addr(types::I64, ss, 0)
                        };
                        let mut cargs = vec![];
                        // a large struct result is written through a hidden pointer
                        let sret = match fret {
                            Some(FfiArg::Memory { size, .. }) => {
                                let a = slot(b, *size);
                                cargs.push(a);
                                Some(a)
                            }
                            _ => None,
                        };
                        for (i, v) in vals.iter().enumerate() {
                            match params.get(i) {
                                Some(FfiArg::Regs { desc, size, sse }) => {
                                    let a = slot(b, *size);
                                    let d = b.ins().iconst(types::I64, *desc as i64);
                                    self.rt_call(
                                        b,
                                        imports,
                                        "lang_rt_ffi_pack",
                                        &[body.ctx, *v, d, a],
                                        body.ctx,
                                    );
                                    for (k, x) in sse.iter().enumerate() {
                                        let t = if *x { types::F64 } else { types::I64 };
                                        cargs.push(b.ins().load(
                                            t,
                                            MemFlagsData::trusted(),
                                            a,
                                            (k * 8) as i32,
                                        ));
                                    }
                                }
                                Some(FfiArg::Memory { desc, size }) => {
                                    let a = slot(b, *size);
                                    let d = b.ins().iconst(types::I64, *desc as i64);
                                    self.rt_call(
                                        b,
                                        imports,
                                        "lang_rt_ffi_pack",
                                        &[body.ctx, *v, d, a],
                                        body.ctx,
                                    );
                                    cargs.push(a);
                                }
                                Some(FfiArg::Scalar(r)) => cargs.push(coerce(b, *v, cty(*r))),
                                None => cargs.push(*v),
                            }
                        }
                        if multi {
                            b.ins().call(imports["lang_rt_safe_begin"], &[body.ctx]);
                        }
                        let call = b.ins().call(imports[&format!("foreign:{name}")], &cargs);
                        if multi {
                            b.ins().call(imports["lang_rt_safe_end"], &[body.ctx]);
                        }
                        let results: Vec<Value> = b.inst_results(call).to_vec();
                        match fret {
                            Some(FfiArg::Regs { desc, size, .. }) => {
                                let a = slot(b, *size);
                                for (k, r) in results.iter().enumerate() {
                                    b.ins().store(MemFlagsData::trusted(), *r, a, (k * 8) as i32);
                                }
                                let d = b.ins().iconst(types::I64, *desc as i64);
                                let c = self.rt_call(
                                    b,
                                    imports,
                                    "lang_rt_ffi_unpack",
                                    &[body.ctx, d, a],
                                    body.ctx,
                                );
                                b.inst_results(c)[0]
                            }
                            Some(FfiArg::Memory { desc, .. }) => {
                                let d = b.ins().iconst(types::I64, *desc as i64);
                                let c = self.rt_call(
                                    b,
                                    imports,
                                    "lang_rt_ffi_unpack",
                                    &[body.ctx, d, sret.unwrap()],
                                    body.ctx,
                                );
                                b.inst_results(c)[0]
                            }
                            _ => match results.first() {
                                Some(v) => *v,
                                None => b.ins().iconst(types::I8, 0),
                            },
                        }
                    }
                    Callee::Intrinsic { name, targs, ret: rd } => {
                        let mut cargs = vec![
                            body.ctx,
                            b.ins().iconst(types::I64, *targs as i64),
                            b.ins().iconst(types::I64, *rd as i64),
                        ];
                        for v in vals {
                            let t = b.func.dfg.value_type(v);
                            let r = if t == types::F64 {
                                Repr::F64
                            } else if t == types::F32 {
                                Repr::F32
                            } else {
                                Repr::I64
                            };
                            cargs.push(to_word(b, v, r));
                        }
                        let call = self.rt_call(b, imports, name.as_str(), &cargs, body.ctx);
                        b.inst_results(call)[0]
                    }
                }
            }
        }
    }

    /// Call a runtime function; with several workers the call holds the
    /// runtime lock (`lang_rt_enter` / `lang_rt_leave`), except for the
    /// entries in [`UNLOCKED`].
    fn rt_call(
        &self,
        b: &mut FunctionBuilder,
        imports: &FxHashMap<String, cranelift_codegen::ir::FuncRef>,
        name: &str,
        args: &[Value],
        ctx: Value,
    ) -> cranelift_codegen::ir::Inst {
        let multi = self.prog.workers > 1 && !UNLOCKED.contains(&name);
        if multi {
            // attributed to the runtime function (`LANG_RUNTIME_STATS`)
            let why = b.ins().iconst(types::I64, lang_runtime::task::lock_reason_id(name) as i64);
            b.ins().call(imports["lang_rt_enter"], &[ctx, why]);
        }
        let call = b.ins().call(imports[name], args);
        if multi {
            b.ins().call(imports["lang_rt_leave"], &[ctx]);
        }
        call
    }

    #[allow(clippy::too_many_arguments)]
    fn int_bin(
        &self,
        b: &mut FunctionBuilder,
        body: &Body,
        op: BinOp,
        x: Value,
        y: Value,
        bits: u8,
        signed: bool,
        checked: bool,
        loc: u64,
        imports: &FxHashMap<String, cranelift_codegen::ir::FuncRef>,
    ) -> Value {
        let t = Type::int(bits as u16).unwrap();
        match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul if checked => {
                let (r, of) = int_overflow(b, op, x, y, bits, signed);
                self.defect_if(b, body, of, 0, loc, imports);
                r
            }
            BinOp::Add => b.ins().iadd(x, y),
            BinOp::Sub => b.ins().isub(x, y),
            BinOp::Mul => b.ins().imul(x, y),
            BinOp::Div | BinOp::Rem => {
                let z = b.ins().icmp_imm_s(IntCC::Equal, y, 0);
                self.defect_if(b, body, z, 1, loc, imports);
                if signed {
                    // MIN / -1 overflows
                    let min = b.ins().iconst(t, if bits == 64 { i64::MIN } else { -(1i64 << (bits - 1)) });
                    let is_min = b.ins().icmp(IntCC::Equal, x, min);
                    let neg1 = b.ins().icmp_imm_s(IntCC::Equal, y, -1);
                    let both = b.ins().band(is_min, neg1);
                    if op == BinOp::Div {
                        self.defect_if(b, body, both, 0, loc, imports);
                        b.ins().sdiv(x, y)
                    } else {
                        // MIN % -1 == 0
                        let one = b.ins().iconst(t, 1);
                        let y2 = b.ins().select(both, one, y);
                        b.ins().srem(x, y2)
                    }
                } else if op == BinOp::Div {
                    b.ins().udiv(x, y)
                } else {
                    b.ins().urem(x, y)
                }
            }
            BinOp::And => b.ins().band(x, y),
            BinOp::Or => b.ins().bor(x, y),
            BinOp::Xor => b.ins().bxor(x, y),
            BinOp::Shl | BinOp::Shr => {
                let bad = b.ins().icmp_imm_s(IntCC::UnsignedGreaterThanOrEqual, y, bits as i64);
                self.defect_if(b, body, bad, 5, loc, imports);
                match (op, signed) {
                    (BinOp::Shl, _) => b.ins().ishl(x, y),
                    (_, true) => b.ins().sshr(x, y),
                    _ => b.ins().ushr(x, y),
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn convert(
        &self,
        b: &mut FunctionBuilder,
        body: &Body,
        x: Value,
        from: NumTy,
        to: NumTy,
        checked: bool,
        loc: u64,
        imports: &FxHashMap<String, cranelift_codegen::ir::FuncRef>,
    ) -> Value {
        match (from, to) {
            (NumTy::I(fb, fs), NumTy::I(tb, ts)) => {
                let ft = Type::int(fb as u16).unwrap();
                let x = coerce(b, x, ft);
                let (r, bad) = int_convert(b, x, fb, fs, tb, ts);
                if checked && (tb < fb || fs != ts) {
                    self.defect_if(b, body, bad, 6, loc, imports);
                }
                r
            }
            (NumTy::I(fb, fs), NumTy::F(tb)) => {
                let mut x = coerce(b, x, Type::int(fb as u16).unwrap());
                // the backend converts from 32- and 64-bit integers only
                if fb < 32 {
                    x = if fs { b.ins().sextend(types::I32, x) } else { b.ins().uextend(types::I32, x) };
                }
                let tt = if tb == 32 { types::F32 } else { types::F64 };
                if fs { b.ins().fcvt_from_sint(tt, x) } else { b.ins().fcvt_from_uint(tt, x) }
            }
            (NumTy::F(_), NumTy::I(tb, ts)) => {
                // the backend converts to 32- and 64-bit integers only;
                // narrower targets go through I32 (callers check the range)
                let wide = if tb < 32 { types::I32 } else { Type::int(tb as u16).unwrap() };
                let r =
                    if ts { b.ins().fcvt_to_sint_sat(wide, x) } else { b.ins().fcvt_to_uint_sat(wide, x) };
                if tb < 32 { b.ins().ireduce(Type::int(tb as u16).unwrap(), r) } else { r }
            }
            (NumTy::F(fb), NumTy::F(tb)) => {
                if fb == tb {
                    x
                } else if tb > fb {
                    b.ins().fpromote(types::F64, x)
                } else {
                    b.ins().fdemote(types::F32, x)
                }
            }
        }
    }
}

impl Body<'_> {
    fn set(&mut self, b: &mut FunctionBuilder, l: Local, v: Value) {
        b.def_var(self.vars[l], v);
        if let (Some(slot), Some(off)) = (self.dbg_slot, self.dbg_offsets.get(&l)) {
            b.ins().stack_store(types::I64, v, slot, *off);
        }
        if let Some(&off) = self.slots.get(&l) {
            b.ins().stack_store(types::I64, v, self.frame, off);
        }
    }

    fn op(&mut self, b: &mut FunctionBuilder, o: &Operand) -> Value {
        match o {
            Operand::Local(l) => b.use_var(self.vars[*l]),
            Operand::Const(c) => match c {
                Const::Int(v, r) => b.ins().iconst(
                    cty(*r),
                    if cty(*r).bits() < 64 { *v & ((1i64 << cty(*r).bits()) - 1) } else { *v },
                ),
                Const::F32(f) => b.ins().f32const(*f),
                Const::F64(f) => b.ins().f64const(*f),
                Const::Addr(a) => b.ins().iconst(types::I64, *a as i64),
            },
        }
    }
}

/// Runtime entries generated code calls without the runtime lock, with
/// several workers: they read immutable values, use builders or fresh
/// objects the caller owns, allocate (a due collection takes the lock),
/// and root through the running task. They never touch scheduler state,
/// call back into compiled code, or mutate shared objects (a shared list
/// tail is extended by an atomic slot claim); a defect takes the lock.
pub const UNLOCKED: &[&str] = &[
    "lang_rt_alloc",
    "lang_rt_concat",
    "lang_rt_eq",
    "lang_rt_cmp",
    "lang_rt_list_new",
    "lang_rt_list_push",
    "lang_rt_list_len",
    "lang_rt_list_get",
    "lang_rt_list_page_cap",
    "lang_rt_contract_site",
    "lang_rt_list_page",
    "lang_rt_list_finish",
    "lang_rt_map_new",
    "lang_rt_map_insert_mut",
    "lang_rt_i_list_len",
    "lang_rt_i_list_get",
    "lang_rt_i_list_builder",
    "lang_rt_i_list_builder_append",
    "lang_rt_i_list_builder_len",
    "lang_rt_i_list_finish",
    "lang_rt_i_list_append",
    "lang_rt_i_list_concat",
    "lang_rt_i_list_reverse",
    "lang_rt_i_list_slice",
    "lang_rt_i_list_contains",
    "lang_rt_i_list_to_array",
    // persistent maps and sets: immutable tries, fresh nodes, structural
    // key equality and hashing
    "lang_rt_i_map_empty",
    "lang_rt_i_map_insert",
    "lang_rt_i_map_remove",
    "lang_rt_i_map_get",
    "lang_rt_i_map_contains",
    "lang_rt_i_map_len",
    "lang_rt_i_map_entries",
    "lang_rt_i_map_insert_mut",
    "lang_rt_i_set_insert",
    "lang_rt_i_set_insert_mut",
    "lang_rt_i_set_remove",
    "lang_rt_i_set_items",
    // buffers are affine (one task uses one at a time) and frozen arrays
    // are immutable; a defect or due collection takes the lock
    "lang_rt_i_buffer_new",
    "lang_rt_i_buffer_push",
    "lang_rt_i_buffer_pop",
    "lang_rt_i_buffer_get",
    "lang_rt_i_buffer_set",
    "lang_rt_i_buffer_len",
    "lang_rt_i_buffer_capacity",
    "lang_rt_i_buffer_address",
    "lang_rt_i_buffer_freeze",
    "lang_rt_i_buffer_to_list",
    // reading a batch lease the task holds (its items are the task's until
    // it releases them; release changes the channel and stays locked)
    "lang_rt_i_batch_len",
    "lang_rt_i_batch_get",
    // the payload a split receive left with the task
    "lang_rt_take_aux",
    "lang_rt_i_chan_recv_split",
    "lang_rt_i_chan_try_recv_split",
    "lang_rt_i_batch_recv_one_split",
    "lang_rt_i_batch_try_recv_split",
    // channel fast paths; they take the lock themselves when needed
    "lang_rt_i_chan_send",
    "lang_rt_i_chan_try_send",
    "lang_rt_i_chan_recv",
    "lang_rt_i_chan_try_recv",
    "lang_rt_i_batch_append",
    "lang_rt_i_batch_try_append",
    "lang_rt_i_batch_recv_one",
    "lang_rt_i_batch_try_recv",
];

/// Run the compiled program's `main` on the runtime scheduler.
pub fn run_main(c: &Compiled, prog: &Program) -> lang_runtime::task::Exit {
    let entry = prog.entry.expect("program has no main");
    let closure =
        prog.static_closures.iter().find(|(_, f)| *f == entry).map(|(o, _)| *o).expect("main closure");
    let _ = c;
    let exit = lang_runtime::task::run_main(closure);
    lang_runtime::task::print_stats();
    exit
}

/// Integer conversion and a flag that is set when the value does not survive
/// it (round-trip or sign mismatch).
fn int_convert(b: &mut FunctionBuilder, x: Value, fb: u8, fs: bool, tb: u8, ts: bool) -> (Value, Value) {
    let ft = Type::int(fb as u16).unwrap();
    let tt = Type::int(tb as u16).unwrap();
    let r = if tb < fb {
        b.ins().ireduce(tt, x)
    } else if tb > fb {
        if fs { b.ins().sextend(tt, x) } else { b.ins().uextend(tt, x) }
    } else {
        x
    };
    let back = if tb < fb {
        if ts { b.ins().sextend(ft, r) } else { b.ins().uextend(ft, r) }
    } else if tb > fb {
        b.ins().ireduce(ft, r)
    } else {
        r
    };
    let lossy = b.ins().icmp(IntCC::NotEqual, back, x);
    let neg_src =
        if fs { b.ins().icmp_imm_s(IntCC::SignedLessThan, x, 0) } else { b.ins().iconst(types::I8, 0) };
    let neg_dst =
        if ts { b.ins().icmp_imm_s(IntCC::SignedLessThan, r, 0) } else { b.ins().iconst(types::I8, 0) };
    let sign_flip = b.ins().icmp(IntCC::NotEqual, neg_src, neg_dst);
    let bad = b.ins().bor(lossy, sign_flip);
    (r, bad)
}

/// Wrapping result of `+ - *` and its overflow flag.
fn int_overflow(
    b: &mut FunctionBuilder,
    op: BinOp,
    x: Value,
    y: Value,
    bits: u8,
    signed: bool,
) -> (Value, Value) {
    let t = Type::int(bits as u16).unwrap();
    if op == BinOp::Mul && bits < 64 {
        // widen, multiply, and check the result fits
        let (wx, wy) = if signed {
            (b.ins().sextend(types::I64, x), b.ins().sextend(types::I64, y))
        } else {
            (b.ins().uextend(types::I64, x), b.ins().uextend(types::I64, y))
        };
        let w = b.ins().imul(wx, wy);
        let r = b.ins().ireduce(t, w);
        let back = if signed { b.ins().sextend(types::I64, r) } else { b.ins().uextend(types::I64, r) };
        let of = b.ins().icmp(IntCC::NotEqual, back, w);
        (r, of)
    } else {
        match (op, signed) {
            (BinOp::Add, true) => b.ins().sadd_overflow(x, y),
            (BinOp::Add, false) => b.ins().uadd_overflow(x, y),
            (BinOp::Sub, true) => b.ins().ssub_overflow(x, y),
            (BinOp::Sub, false) => b.ins().usub_overflow(x, y),
            (_, true) => b.ins().smul_overflow(x, y),
            _ => b.ins().umul_overflow(x, y),
        }
    }
}
