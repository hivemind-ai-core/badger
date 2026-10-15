//! MIR: explicit control-flow graphs over typed locals.

use index_vec::IndexVec;
pub use lang_runtime::Repr;

index_vec::define_index_type! {
    pub struct Local = u32;
}

index_vec::define_index_type! {
    pub struct BlockId = u32;
}

index_vec::define_index_type! {
    pub struct FuncId = u32;
}

#[derive(Debug, Default)]
pub struct Program {
    pub funcs: IndexVec<FuncId, Func>,
    /// Instance of `main`.
    pub entry: Option<FuncId>,
    /// Descriptor of `main`'s result type.
    pub entry_ret: Option<&'static lang_runtime::TypeDesc>,
    /// `List<String>` descriptor when the entry takes the arguments.
    pub entry_args: Option<&'static lang_runtime::TypeDesc>,
    /// Why the entry function's signature cannot be invoked by the tool.
    pub entry_error: Option<String>,
    /// Static closure of `describe_error<E>` for an entry returning
    /// `Result<T, E>` with `E: Error`.
    pub error_report: Option<FuncId>,
    /// Global value slots (count).
    pub n_globals: u32,
    /// Static closure objects to patch with code/adapter addresses.
    pub static_closures: Vec<(u64, FuncId)>,
    /// Display implementations to register on descriptors.
    pub display_hooks: Vec<(&'static lang_runtime::TypeDesc, FuncId)>,
    /// Test functions: (name, function).
    pub tests: Vec<(String, FuncId)>,
    /// Emit calls between user functions through live-reload slots.
    pub live: bool,
    /// Worker threads; above one, runtime calls take the runtime lock.
    pub workers: usize,
    /// Where each runtime address in the program came from (for saving a
    /// program as a relocatable binary file; see `serial`).
    pub origins: Origins,
}

/// The runtime objects a lowered program refers to by address.
#[derive(Debug, Default)]
pub struct Origins {
    /// Every descriptor the compiler created.
    pub descs: Vec<&'static lang_runtime::TypeDesc>,
    /// Immortal strings (constants and source locations).
    pub strings: rustc_hash::FxHashMap<u64, String>,
    /// Interned symbols.
    pub symbols: rustc_hash::FxHashMap<u64, String>,
    /// Static closure objects: (descriptor, payload size, function).
    pub closures: rustc_hash::FxHashMap<u64, (u64, u32, FuncId)>,
    /// Nullary enum values: (descriptor, variant).
    pub nullary: rustc_hash::FxHashMap<u64, (u64, u32)>,
    /// Type-argument descriptor arrays.
    pub targs: rustc_hash::FxHashMap<u64, Vec<u64>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuncKind {
    Normal,
    /// Lowered lambda; `env` is the closure object.
    Lambda,
    /// Display hook: `(ctx, word) -> String` called by the runtime.
    DisplayHook,
}

#[derive(Debug)]
pub struct Func {
    pub name: String,
    pub kind: FuncKind,
    pub env: Local,
    pub params: Vec<Local>,
    pub ret: Repr,
    pub locals: IndexVec<Local, LocalDecl>,
    pub blocks: IndexVec<BlockId, Block>,
    /// Used as a first-class closure: needs a word adapter.
    pub needs_adapter: bool,
    /// Live-reload identity of a user instance (`path<type args>`) and its
    /// signature text; `None` for library code, lambdas, and hooks.
    pub live_key: Option<String>,
    pub live_sig: String,
    /// A module-level value getter (keeps its state across reloads).
    pub is_global: bool,
}

#[derive(Debug, Clone)]
pub struct LocalDecl {
    pub repr: Repr,
    pub name: Option<String>,
}

#[derive(Debug, Default)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub term: Option<Term>,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Assign(Local, Rvalue),
    /// Store into a managed object's payload at `offset`.
    Store {
        base: Operand,
        offset: i32,
        repr: Repr,
        value: Operand,
    },
    /// Store into a global slot.
    GlobalStore {
        slot: u32,
        value: Operand,
    },
    /// Scheduling / cancellation safe point.
    Poll,
}

#[derive(Debug, Clone, Copy)]
pub enum Operand {
    Local(Local),
    Const(Const),
}

#[derive(Debug, Clone, Copy)]
pub enum Const {
    Int(i64, Repr),
    F32(f32),
    F64(f64),
    /// Immortal object, descriptor, or other static address.
    Addr(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumTy {
    I(u8, bool),
    F(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    And,
    Or,
    Xor,
    Shl,
    Shr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone)]
pub enum Callee {
    Direct(FuncId),
    /// Call a closure object (code pointer at payload offset 0).
    Closure(Operand),
    /// Core runtime function with a native signature.
    Runtime(&'static str),
    /// Standard-library intrinsic with the uniform ABI.
    Intrinsic {
        name: String,
        targs: u64,
        ret: u64,
    },
    /// Foreign C function: scalar arguments use their representation;
    /// `repr c` structs are passed by value per the C calling convention.
    /// `contract`: bit 0 blocking, bit 1 pinned (run through a thunk on the
    /// blocking pool / the pinned thread while the task parks).
    Foreign {
        name: String,
        params: Vec<FfiArg>,
        ret: Option<FfiArg>,
        contract: u8,
    },
}

#[derive(Debug, Clone)]
pub enum Rvalue {
    Use(Operand),
    /// Arithmetic; integer operations are checked (defects carry `loc`).
    Bin {
        op: BinOp,
        a: Operand,
        b: Operand,
        ty: NumTy,
        loc: u64,
    },
    Neg {
        a: Operand,
        ty: NumTy,
        loc: u64,
    },
    Not(Operand),
    Cmp {
        op: CmpOp,
        a: Operand,
        b: Operand,
        ty: NumTy,
    },
    /// Numeric conversion (checked when lossy and `checked`).
    Convert {
        a: Operand,
        from: NumTy,
        to: NumTy,
        checked: bool,
        loc: u64,
    },
    /// Whether an integer converts to `to` without loss (Bool).
    Fits {
        a: Operand,
        from: NumTy,
        to: NumTy,
    },
    /// Whether an integer operation overflows or divides by zero (Bool).
    Overflows {
        op: BinOp,
        a: Operand,
        b: Operand,
        ty: NumTy,
    },
    Load {
        base: Operand,
        offset: i32,
        repr: Repr,
    },
    /// Reinterpret a 64-bit runtime word as a value of `to` (truncate/bitcast).
    FromWord {
        a: Operand,
        to: Repr,
    },
    /// Widen a value to a 64-bit runtime word (zero-extend/bitcast).
    ToWord(Operand),
    /// Enum tag (u32 at payload offset 0).
    Tag(Operand),
    Alloc {
        desc: u64,
        size: u32,
    },
    Call {
        callee: Callee,
        args: Vec<Operand>,
        ret: Repr,
    },
    /// Concatenate strings (interpolation).
    Concat(Vec<Operand>),
    GlobalLoad(u32),
    /// Address of a global slot.
    GlobalAddr(u32),
    /// Address of a function's code (for closure construction).
    FuncAddr(FuncId),
    /// Address of a function's word adapter.
    AdapterAddr(FuncId),
}

/// How one value crosses the C boundary.
#[derive(Debug, Clone, PartialEq)]
pub enum FfiArg {
    Scalar(Repr),
    /// A `repr c` struct of at most 16 bytes, passed in registers: one
    /// class per eightbyte. `desc` is the managed struct's descriptor.
    Regs {
        desc: u64,
        size: u32,
        sse: Vec<bool>,
    },
    /// A larger `repr c` struct, passed (or returned) in memory.
    Memory {
        desc: u64,
        size: u32,
    },
}

#[derive(Debug, Clone)]
pub enum Term {
    Goto(BlockId),
    Branch(Operand, BlockId, BlockId),
    Switch(Operand, Vec<(i64, BlockId)>, BlockId),
    Return(Operand),
    Unreachable,
}

impl Func {
    pub fn new(name: String, kind: FuncKind, ret: Repr) -> Func {
        let mut locals = IndexVec::new();
        let env = locals.push(LocalDecl { repr: Repr::Ptr, name: Some("env".into()) });
        let mut blocks = IndexVec::new();
        blocks.push(Block::default());
        Func {
            name,
            kind,
            env,
            params: vec![],
            ret,
            locals,
            blocks,
            needs_adapter: false,
            live_key: None,
            live_sig: String::new(),
            is_global: false,
        }
    }
}

pub fn repr_ty(r: Repr) -> &'static str {
    match r {
        Repr::I8 => "i8",
        Repr::I16 => "i16",
        Repr::I32 => "i32",
        Repr::I64 => "i64",
        Repr::F32 => "f32",
        Repr::F64 => "f64",
        Repr::Ptr => "ptr",
    }
}

/// Human-readable MIR dump (debugging, `--emit=mir`).
pub fn dump(p: &Program) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    for (id, f) in p.funcs.iter_enumerated() {
        let _ = writeln!(s, "fn {:?} {} ({:?}) -> {}", id, f.name, f.params, repr_ty(f.ret));
        for (l, d) in f.locals.iter_enumerated() {
            let _ = writeln!(s, "  {l:?}: {} {}", repr_ty(d.repr), d.name.clone().unwrap_or_default());
        }
        for (b, blk) in f.blocks.iter_enumerated() {
            let _ = writeln!(s, " {b:?}:");
            for st in &blk.stmts {
                let _ = writeln!(s, "    {st:?}");
            }
            let _ = writeln!(s, "    {:?}", blk.term);
        }
    }
    s
}
