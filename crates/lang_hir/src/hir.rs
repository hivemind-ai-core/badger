//! HIR data structures: the AST after module/name resolution and desugaring.
//!
//! - Names resolve to [`LocalId`]s, [`DefId`]s, enum constructors, or type
//!   namespaces.
//! - Pipes, `use` continuation binding, dotted record updates, field
//!   assignment, `system { ... }` plans, `pipeline { ... }` chains, and
//!   `pipeline.stage` descriptors are desugared into ordinary calls.
//! - Lambdas record their captured locals.

use la_arena::{Arena, Idx};
use lang_base::{Span, Symbol};
use rustc_hash::FxHashMap;

pub use lang_syntax::ast::{BinOp, DurationUnit, UnOp};

pub type ExprId = Idx<Expr>;
pub type PatId = Idx<Pat>;
pub type LocalId = Idx<Local>;
pub type DefId = Idx<Def>;
pub type ModuleId = Idx<Module>;
pub type GenericId = Idx<GenericParam>;

#[derive(Debug, Default)]
pub struct Hir {
    pub exprs: Arena<Expr>,
    pub pats: Arena<Pat>,
    pub locals: Arena<Local>,
    pub defs: Arena<Def>,
    pub modules: Arena<Module>,
    pub generics: Arena<GenericParam>,
    /// Well-known definitions the compiler relies on (`Option`, `List`, ...).
    pub lang: FxHashMap<&'static str, DefId>,
    /// Module-level value initializers in source order (globals).
    pub globals: Vec<DefId>,
    /// Entry module (the user's main module).
    pub entry: Option<ModuleId>,
    /// `const` initializers (module and local), evaluated at check time,
    /// and the initializer of each local `const` binding.
    pub const_inits: Vec<ExprId>,
    pub const_local_inits: Vec<(LocalId, ExprId)>,
}

impl Hir {
    pub fn lang(&self, name: &str) -> DefId {
        *self.lang.get(name).unwrap_or_else(|| panic!("missing lang item `{name}`"))
    }

    pub fn try_lang(&self, name: &str) -> Option<DefId> {
        self.lang.get(name).copied()
    }
}

#[derive(Debug)]
pub struct Module {
    pub path: Vec<Symbol>,
    pub file: lang_base::FileId,
    pub is_std: bool,
    /// Items declared in this module (both namespaces; case distinguishes).
    pub items: FxHashMap<Symbol, DefId>,
    pub tests: Vec<DefId>,
}

#[derive(Debug, Clone)]
pub struct GenericParam {
    pub name: Symbol,
    pub span: Span,
    pub bounds: Vec<HTy>,
    /// Owning definition.
    pub owner: Option<DefId>,
}

// ------------------------------------------------------------------ definitions

#[derive(Debug)]
pub struct Def {
    pub name: Symbol,
    pub module: ModuleId,
    pub span: Span,
    pub is_pub: bool,
    pub kind: DefKind,
}

#[derive(Debug)]
pub enum DefKind {
    Fn(FnDef),
    Global(GlobalDef),
    Struct(StructDef),
    Enum(EnumDef),
    Alias(AliasDef),
    Opaque(OpaqueDef),
    Protocol(ProtocolDef),
    Impl(ImplDef),
    /// Placeholder while collecting; replaced during lowering.
    Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FnAbi {
    /// Ordinary language function with a body.
    Lang,
    /// Runtime-provided intrinsic (`extern "intrinsic"`).
    Intrinsic,
    /// Foreign C function (`extern "c"`).
    C,
    /// Function of a native module (`extern "native NAME"`), named in
    /// [`FnDef::native`].
    Native,
}

#[derive(Debug)]
pub struct FnDef {
    pub generics: Vec<GenericId>,
    pub params: Vec<Param>,
    pub ret: Option<HTy>,
    pub effect: Option<HEffect>,
    pub where_preds: Vec<WherePred>,
    pub body: Option<ExprId>,
    pub abi: FnAbi,
    /// Impl or protocol that owns this method.
    pub owner: Option<DefId>,
    /// Whether the first parameter is `self`.
    pub has_self: bool,
    /// Declared adapter contract of an `extern "c"` function.
    pub ffi: FfiContract,
    /// The native module of an `extern "native NAME"` function, with the
    /// module's function name after a `.` when it differs from the
    /// declaration's (`"native NAME.FUNCTION"`).
    pub native: Option<String>,
    /// `requires` clauses, checked at the start of `body` (each by the
    /// `check` expression placed there).
    pub requires: Vec<Precondition>,
    /// `ensures` clauses: checked on every return, with `result` (the
    /// local below) bound to the value returned.
    pub ensures: Vec<Postcondition>,
    pub result: Option<LocalId>,
    /// Source text of every contract clause, for tools.
    pub contracts: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Precondition {
    pub cond: ExprId,
    /// The `if !cond { ... }` check at the start of the body.
    pub check: ExprId,
    /// The clause's source text.
    pub text: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Postcondition {
    pub cond: ExprId,
    /// The defect message when it does not hold.
    pub message: String,
    pub span: Span,
}

/// Contract words after `c` in an extern ABI string, e.g.
/// `extern "c blocking pinned"`: `blocking` calls run on the bounded
/// blocking pool while the calling task parks (cancellation waits for the
/// call to return); `pinned` calls always run on one dedicated OS thread
/// (thread affinity). Without them a call runs inline on the worker and
/// must return promptly. C code never calls back into the language.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FfiContract {
    pub blocking: bool,
    pub pinned: bool,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub pat: PatId,
    pub ty: Option<HTy>,
}

#[derive(Debug, Clone)]
pub struct WherePred {
    pub ty: HTy,
    pub bounds: Vec<HTy>,
}

#[derive(Debug)]
pub struct GlobalDef {
    pub pat: PatId,
    pub ty: Option<HTy>,
    pub value: ExprId,
    pub is_const: bool,
    /// Names bound by `pat` — globals bind exactly one name in draft 0.
    pub local: LocalId,
}

#[derive(Debug)]
pub struct StructDef {
    pub generics: Vec<GenericId>,
    pub fields: Vec<FieldDef>,
    pub derives: Vec<Derive>,
    pub repr_c: bool,
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: Symbol,
    pub ty: HTy,
    pub is_pub: bool,
    pub span: Span,
}

#[derive(Debug)]
pub struct EnumDef {
    pub generics: Vec<GenericId>,
    pub variants: Vec<VariantDef>,
    pub derives: Vec<Derive>,
}

#[derive(Debug, Clone)]
pub struct VariantDef {
    pub name: Symbol,
    pub fields: Vec<(Option<Symbol>, HTy)>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Derive {
    pub protocol: DefId,
    pub span: Span,
}

#[derive(Debug)]
pub struct AliasDef {
    pub generics: Vec<GenericId>,
    pub ty: HTy,
}

#[derive(Debug)]
pub struct OpaqueDef {
    pub generics: Vec<GenericId>,
}

#[derive(Debug)]
pub struct ProtocolDef {
    pub generics: Vec<GenericId>,
    pub supers: Vec<HTy>,
    pub methods: Vec<DefId>,
    pub assoc_types: Vec<Symbol>,
}

#[derive(Debug)]
pub struct ImplDef {
    pub generics: Vec<GenericId>,
    pub protocol: Option<HTy>,
    pub self_ty: HTy,
    pub where_preds: Vec<WherePred>,
    pub methods: Vec<DefId>,
    pub assoc_types: Vec<(Symbol, HTy)>,
}

// ------------------------------------------------------------------ types

/// A resolved type expression from source.
#[derive(Debug, Clone)]
pub struct HTy {
    pub kind: HTyKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HTyKind {
    Hole,
    /// Struct, enum, alias, opaque, or protocol (in bound/dyn position).
    Named {
        def: DefId,
        args: Vec<HTy>,
    },
    Generic(GenericId),
    /// `Self` inside an impl or protocol.
    SelfTy,
    Unit,
    /// Associated type projection: `Self.Item`, `T.Item`.
    Assoc {
        base: Box<HTy>,
        name: Symbol,
    },
    Tuple(Vec<HTy>),
    Record {
        fields: Vec<(Symbol, HTy)>,
        rest: Option<GenericId>,
    },
    Fn {
        once: bool,
        params: Vec<HTy>,
        ret: Box<HTy>,
        effect: Option<HEffect>,
    },
    Forall {
        params: Vec<GenericId>,
        body: Box<HTy>,
    },
    Ref {
        mutable: bool,
        inner: Box<HTy>,
    },
    Dyn {
        protocol: DefId,
        args: Vec<HTy>,
    },
    /// Effect labels in effect position (`Io`, `Wait`, ...).
    EffectLabel(Symbol),
    /// An effect row argument (`Seq<Int, !{Io}>`).
    Effect(HEffect),
    Error,
}

#[derive(Debug, Clone)]
pub enum HEffect {
    /// `!{Io, Wait}`; with `rest`, `!{Io, Eff}` (the labels and an effect
    /// variable's).
    Set {
        labels: Vec<Symbol>,
        rest: Option<GenericId>,
        span: Span,
    },
    Var(GenericId),
}

// ------------------------------------------------------------------ bodies

#[derive(Debug)]
pub struct Local {
    pub name: Symbol,
    pub span: Span,
    pub mutable: bool,
    /// Function or lambda expression that binds this local (`None` = global).
    pub owner: Option<Owner>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Owner {
    Fn(DefId),
    Lambda(ExprId),
    Global(DefId),
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Lit {
    Int { value: u128, suffix: Option<Symbol> },
    Float { value: f64, suffix: Option<Symbol> },
    Duration(u128),
    Str(String),
    Char(char),
    Symbol(Symbol),
    Bool(bool),
    Unit,
}

#[derive(Debug, Clone)]
pub enum InterpPart {
    Text(String),
    Expr(ExprId),
}

#[derive(Debug, Clone)]
pub struct Arm {
    pub pat: PatId,
    pub guard: Option<ExprId>,
    pub body: ExprId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let { pat: PatId, ty: Option<HTy>, init: ExprId, span: Span },
    Assign { local: LocalId, value: ExprId, span: Span },
    Expr { expr: ExprId, semi: bool },
}

#[derive(Debug, Clone)]
pub struct LambdaExpr {
    pub once: bool,
    pub params: Vec<Param>,
    pub ret: Option<HTy>,
    pub effect: Option<HEffect>,
    pub body: ExprId,
    pub captures: Vec<LocalId>,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Lit(Lit),
    Interp(Vec<InterpPart>),
    Local(LocalId),
    /// Function, global, or intrinsic, with explicit type arguments.
    Def {
        def: DefId,
        type_args: Vec<HTy>,
    },
    /// Enum variant used as a value (`Job.shutdown`) or constructor function.
    Ctor {
        enum_def: DefId,
        variant: u32,
        type_args: Vec<HTy>,
    },
    /// `Type.member` that is not a variant: an associated function (static
    /// method of an inherent or protocol impl), resolved by type checking.
    AssocFn {
        ty: HTy,
        name: Symbol,
        name_span: Span,
        type_args: Vec<HTy>,
    },
    Field {
        base: ExprId,
        label: Symbol,
        label_span: Span,
    },
    Index {
        base: ExprId,
        index: ExprId,
    },
    Call {
        callee: ExprId,
        args: Vec<ExprId>,
    },
    MethodCall {
        recv: ExprId,
        method: Symbol,
        method_span: Span,
        type_args: Vec<HTy>,
        args: Vec<ExprId>,
    },
    Try(ExprId),
    Unary {
        op: UnOp,
        operand: ExprId,
    },
    Binary {
        op: BinOp,
        lhs: ExprId,
        rhs: ExprId,
    },
    Tuple(Vec<ExprId>),
    List(Vec<ExprId>),
    Record {
        spread: Option<ExprId>,
        fields: Vec<(Symbol, Span, ExprId)>,
    },
    Map(Vec<(ExprId, ExprId)>),
    Set(Vec<ExprId>),
    Lambda(LambdaExpr),
    If {
        cond: ExprId,
        then: ExprId,
        els: Option<ExprId>,
    },
    Match {
        scrutinee: ExprId,
        arms: Vec<Arm>,
    },
    While {
        cond: ExprId,
        body: ExprId,
    },
    For {
        pat: PatId,
        iter: ExprId,
        body: ExprId,
    },
    Loop {
        body: ExprId,
    },
    Block {
        stmts: Vec<Stmt>,
        tail: Option<ExprId>,
    },
    Return {
        value: Option<ExprId>,
        target: Owner,
    },
    Break {
        value: Option<ExprId>,
        target: ExprId,
    },
    Continue {
        target: ExprId,
    },
    Unsafe(ExprId),
    Error,
}

#[derive(Debug, Clone)]
pub struct Pat {
    pub kind: PatKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum PatKind {
    Discard,
    Bind(LocalId),
    Lit(Lit),
    Tuple(Vec<PatId>),
    Record {
        fields: Vec<(Symbol, PatId)>,
        open: bool,
    },
    /// `.name(args)` or `Enum.name(args)`. The enum is resolved from the
    /// scrutinee type when not written.
    Variant {
        enum_def: Option<DefId>,
        name: Symbol,
        name_span: Span,
        args: Vec<PatId>,
        has_args: bool,
    },
    Error,
}
