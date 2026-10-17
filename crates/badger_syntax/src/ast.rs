//! Arena-based surface AST. Expressions, patterns and type expressions live
//! in arenas addressed by typed indices; items are a plain vector.

use badger_base::{Span, Symbol};
use la_arena::{Arena, Idx};

pub type ExprId = Idx<Expr>;
pub type PatId = Idx<Pat>;
pub type TypeId = Idx<TypeExpr>;

#[derive(Debug, Default)]
pub struct Ast {
    pub exprs: Arena<Expr>,
    pub pats: Arena<Pat>,
    pub types: Arena<TypeExpr>,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Name {
    pub sym: Symbol,
    pub span: Span,
}

/// A record label: a lowercase identifier or a qualified symbol label
/// (`[#auth/user]`). Both are stored as the symbol's text without the `#`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Label {
    pub sym: Symbol,
    pub span: Span,
}

// ------------------------------------------------------------------ items

#[derive(Debug, Clone)]
pub struct Item {
    pub kind: ItemKind,
    pub span: Span,
    pub is_pub: bool,
}

#[derive(Debug, Clone)]
pub enum ItemKind {
    Import(Import),
    TypeAlias {
        name: Name,
        generics: Vec<GenericParam>,
        ty: TypeId,
    },
    Struct(StructDecl),
    Enum(EnumDecl),
    Protocol(ProtocolDecl),
    Impl(ImplDecl),
    Fn(FnDecl),
    Extern {
        abi: String,
        decl: FnDecl,
    },
    /// `extern type Name<T>`: an opaque runtime-provided type (standard library only).
    ExternType {
        name: Name,
        generics: Vec<GenericParam>,
    },
    Global {
        kind: LetKind,
        pat: PatId,
        ty: Option<TypeId>,
        value: ExprId,
    },
}

#[derive(Debug, Clone)]
pub struct Import {
    pub path: Vec<Name>,
    pub alias: Option<Name>,
    /// `import a.b.{x, y as z}`
    pub items: Option<Vec<(Name, Option<Name>)>>,
}

#[derive(Debug, Clone)]
pub struct GenericParam {
    pub name: Name,
    pub bounds: Vec<TypeId>,
}

#[derive(Debug, Clone)]
pub struct WherePred {
    pub ty: TypeId,
    pub bounds: Vec<TypeId>,
}

#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: Name,
    pub generics: Vec<GenericParam>,
    pub fields: Vec<StructField>,
    pub derives: Vec<TypeId>,
    pub repr_c: bool,
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub is_pub: bool,
    pub name: Name,
    pub ty: TypeId,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: Name,
    pub generics: Vec<GenericParam>,
    pub variants: Vec<Variant>,
    pub derives: Vec<TypeId>,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub name: Name,
    pub fields: Vec<(Option<Name>, TypeId)>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ProtocolDecl {
    pub name: Name,
    pub generics: Vec<GenericParam>,
    pub supers: Vec<TypeId>,
    pub methods: Vec<FnDecl>,
    pub assoc_types: Vec<(Name, Option<TypeId>)>,
}

#[derive(Debug, Clone)]
pub struct ImplDecl {
    pub generics: Vec<GenericParam>,
    /// `impl Protocol for Type`: protocol is `Some`. `impl Type { .. }`: inherent.
    pub protocol: Option<TypeId>,
    pub self_ty: TypeId,
    pub where_preds: Vec<WherePred>,
    pub methods: Vec<FnDecl>,
    pub assoc_types: Vec<(Name, Option<TypeId>)>,
}

#[derive(Debug, Clone)]
pub struct FnDecl {
    pub name: Name,
    pub is_pub: bool,
    pub generics: Vec<GenericParam>,
    pub params: Vec<Param>,
    pub ret: Option<TypeId>,
    pub effect: Option<Effect>,
    pub where_preds: Vec<WherePred>,
    pub body: Option<ExprId>,
    pub span: Span,
    /// `requires` / `ensures` clauses written before the function.
    pub contracts: Vec<Contract>,
}

/// A function contract clause: `requires COND` is checked on entry,
/// `ensures COND` (with `result` bound to the return value) on every return.
#[derive(Debug, Clone)]
pub struct Contract {
    pub ensures: bool,
    pub cond: ExprId,
    pub message: Option<String>,
    pub span: Span,
    /// Source text of the clause and of its condition (whitespace
    /// collapsed), for messages and tools.
    pub text: String,
    pub cond_text: String,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub pat: PatId,
    pub ty: Option<TypeId>,
}

/// `!{Io, Wait}` or `!Eff`
#[derive(Debug, Clone)]
pub enum Effect {
    Set(Vec<TypeId>, Span),
    Var(Name),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LetKind {
    Let,
    Var,
    Const,
}

// ------------------------------------------------------------------ types

#[derive(Debug, Clone)]
pub struct TypeExpr {
    pub kind: TypeKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TypeKind {
    Hole,
    Path {
        module: Vec<Name>,
        name: Name,
        args: Vec<TypeId>,
    },
    Unit,
    Tuple(Vec<TypeId>),
    Record {
        fields: Vec<(Label, TypeId)>,
        rest: Option<Name>,
    },
    Fn {
        once: bool,
        params: Vec<TypeId>,
        ret: TypeId,
        effect: Option<Effect>,
    },
    Forall {
        generics: Vec<GenericParam>,
        body: TypeId,
    },
    Ref {
        mutable: bool,
        inner: TypeId,
    },
    Dyn {
        module: Vec<Name>,
        name: Name,
        args: Vec<TypeId>,
    },
    /// An effect row as a type argument: `Seq<Int, !{Io}>`.
    Effect(Effect),
    /// A type that did not parse (only in a tree recovered from syntax errors).
    Error,
}

// ------------------------------------------------------------------ expressions

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Pipe,
    Or,
    And,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Range,
    RangeIncl,
    BitOr,
    BitXor,
    BitAnd,
    Shl,
    Shr,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

impl BinOp {
    pub fn is_comparison(self) -> bool {
        matches!(self, BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            BinOp::Pipe => "|>",
            BinOp::Or => "||",
            BinOp::And => "&&",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::Range => "..",
            BinOp::RangeIncl => "..=",
            BinOp::BitOr => "|",
            BinOp::BitXor => "^",
            BinOp::BitAnd => "&",
            BinOp::Shl => "<<",
            BinOp::Shr => ">>",
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
    Ref,
    RefMut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurationUnit {
    Ns,
    Us,
    Ms,
    S,
    Min,
    H,
}

impl DurationUnit {
    pub fn nanos(self) -> u128 {
        match self {
            DurationUnit::Ns => 1,
            DurationUnit::Us => 1_000,
            DurationUnit::Ms => 1_000_000,
            DurationUnit::S => 1_000_000_000,
            DurationUnit::Min => 60_000_000_000,
            DurationUnit::H => 3_600_000_000_000,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Lit {
    Int { value: u128, suffix: Option<Symbol> },
    Float { value: f64, suffix: Option<Symbol> },
    Duration { value: u128, unit: DurationUnit },
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
pub enum RecordEntry {
    Field { path: Vec<Label>, value: ExprId },
    Shorthand(Name),
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pat: PatId,
    pub guard: Option<ExprId>,
    pub body: ExprId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Lambda {
    pub once: bool,
    pub params: Vec<Param>,
    pub ret: Option<TypeId>,
    pub effect: Option<Effect>,
    pub body: ExprId,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let { kind: LetKind, pat: PatId, ty: Option<TypeId>, value: ExprId, span: Span },
    Use { pats: Vec<PatId>, call: ExprId, span: Span },
    Assign { base: Name, path: Vec<PlaceElem>, value: ExprId, span: Span },
    Expr { expr: ExprId, semi: bool },
}

#[derive(Debug, Clone)]
pub enum PlaceElem {
    Field(Label),
    Index(ExprId),
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Lit(Lit),
    Interp(Vec<InterpPart>),
    Name(Name),
    TypeName(Name),
    Field {
        base: ExprId,
        name: Name,
    },
    Index {
        base: ExprId,
        index: ExprId,
    },
    Call {
        callee: ExprId,
        type_args: Vec<TypeId>,
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
        entries: Vec<RecordEntry>,
    },
    Map(Vec<(ExprId, ExprId)>),
    Set(Vec<ExprId>),
    Lambda(Lambda),
    If {
        cond: ExprId,
        then: ExprId,
        els: Option<ExprId>,
    },
    Match {
        scrutinee: ExprId,
        arms: Vec<MatchArm>,
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
    },
    Return(Option<ExprId>),
    Break(Option<ExprId>),
    Continue,
    System(Vec<(Label, ExprId)>),
    Pipeline(Vec<ExprId>),
    Ref(Label),
    Unsafe(ExprId),
    /// An expression that did not parse (only in a tree recovered from
    /// syntax errors).
    Error,
}

// ------------------------------------------------------------------ patterns

#[derive(Debug, Clone)]
pub struct Pat {
    pub kind: PatKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum PatKind {
    Discard,
    Bind(Name),
    Lit {
        lit: Lit,
        negative: bool,
    },
    Tuple(Vec<PatId>),
    Record {
        fields: Vec<(Label, Option<PatId>)>,
        open: bool,
    },
    Variant {
        ty: Option<(Vec<Name>, Name)>,
        name: Name,
        args: Option<Vec<PatId>>,
    },
    /// A pattern that did not parse (only in a tree recovered from syntax errors).
    Error,
}
