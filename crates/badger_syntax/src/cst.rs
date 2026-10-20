//! The parser's output: an owned syntax tree that borrows the source text.
//! [`crate::lower`] turns it into the arena [`crate::ast::Ast`], checking
//! the rules the grammar does not (case rules, duplicate fields, literal
//! values) and desugaring. Error nodes stand where a syntax error was
//! recovered from.

use crate::token::Span;

/// A word of the source with its span.
#[derive(Debug, Clone, Copy)]
pub struct Ident<'a> {
    pub text: &'a str,
    pub span: Span,
}

/// An attribute: `@name`, `@name(a, b)`, `@name(key = value)`.
#[derive(Debug, Clone)]
pub struct Attribute<'a> {
    pub name: Ident<'a>,
    pub args: Vec<AttrArg<'a>>,
    pub span: Span,
}

/// An attribute argument. Names and symbols are the common forms; a
/// `key = value` pair carries a key and one value.
#[derive(Debug, Clone)]
pub enum AttrArg<'a> {
    /// A bare word, e.g. `stage`.
    Name(Ident<'a>),
    /// A symbol, `#name` or `#ns/name` (the text without the `#`).
    Symbol(&'a str, Span),
    /// An integer literal (its text).
    Int(&'a str, Span),
    /// A string literal (its text).
    Str(&'a str, Span),
    /// `key = value`.
    KeyValue(Ident<'a>, Box<AttrArg<'a>>),
}

/// A record label: a field name, or a qualified symbol (`[#auth/user]`).
#[derive(Debug, Clone, Copy)]
pub enum Label<'a> {
    Name(Ident<'a>),
    /// The symbol's text without the colon; `span` covers the brackets.
    Symbol(&'a str, Span),
}

impl Label<'_> {
    pub fn span(&self) -> Span {
        match self {
            Label::Name(i) => i.span,
            Label::Symbol(_, s) => *s,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Lit<'a> {
    Int(&'a str),
    Float(&'a str),
    Duration(&'a str),
    /// A string's text between the quotes, escapes not processed.
    Str(&'a str),
    RawStr(&'a str),
    Char(&'a str),
    /// A symbol's text without the colon.
    Symbol(&'a str),
    Bool(bool),
}

pub type E<'a> = (Expr<'a>, Span);
pub type P<'a> = (Pat<'a>, Span);
pub type T<'a> = (Ty<'a>, Span);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LetKind {
    Let,
    Var,
    Const,
}

// ------------------------------------------------------------------ items

#[derive(Debug, Clone)]
pub struct Item<'a> {
    pub kind: ItemKind<'a>,
    pub span: Span,
    pub is_pub: bool,
    pub attributes: Vec<Attribute<'a>>,
}

#[derive(Debug, Clone)]
pub enum ItemKind<'a> {
    Import {
        path: Vec<Ident<'a>>,
        alias: Option<Ident<'a>>,
        items: Option<Vec<(Ident<'a>, Option<Ident<'a>>)>>,
    },
    TypeAlias {
        name: Ident<'a>,
        generics: Vec<Generic<'a>>,
        ty: T<'a>,
    },
    Struct {
        repr_c: bool,
        name: Ident<'a>,
        generics: Vec<Generic<'a>>,
        fields: Vec<(bool, Ident<'a>, T<'a>)>,
        derives: Vec<T<'a>>,
    },
    Enum {
        name: Ident<'a>,
        generics: Vec<Generic<'a>>,
        variants: Vec<Variant<'a>>,
        derives: Vec<T<'a>>,
    },
    Protocol {
        name: Ident<'a>,
        generics: Vec<Generic<'a>>,
        supers: Vec<T<'a>>,
        members: Vec<Member<'a>>,
    },
    Impl {
        generics: Vec<Generic<'a>>,
        first: T<'a>,
        /// `impl P for T`: `T` (and `first` is the protocol).
        for_ty: Option<T<'a>>,
        where_preds: Vec<WherePred<'a>>,
        members: Vec<Member<'a>>,
    },
    Fn(FnDecl<'a>),
    ExternType {
        name: Ident<'a>,
        generics: Vec<Generic<'a>>,
    },
    Extern {
        abi: (&'a str, Span),
        decl: FnDecl<'a>,
    },
    Global {
        kind: LetKind,
        pat: P<'a>,
        ty: Option<T<'a>>,
        value: E<'a>,
    },
    /// `provider NAME [as #sym](scope, config) [-> Result<O, E>] { ... }`
    Provider {
        contracts: Vec<Contract<'a>>,
        name: Ident<'a>,
        alias: Option<(&'a str, Span)>,
        params: Vec<Param<'a>>,
        params_span: Span,
        ret: Option<T<'a>>,
        effect: Option<Effect<'a>>,
        body: Block<'a>,
    },
    /// `stage NAME [as #sym][<G>](params) [-> Result<_, E>] { enter(..) .. }`
    Stage {
        contracts: Vec<Contract<'a>>,
        name: Ident<'a>,
        alias: Option<(&'a str, Span)>,
        generics: Vec<Generic<'a>>,
        params: Vec<Param<'a>>,
        ret: Option<T<'a>>,
        members: Vec<StageMember<'a>>,
        body_span: Span,
    },
    /// A declaration that did not parse (the error is reported).
    Error,
}

#[derive(Debug, Clone)]
pub struct Variant<'a> {
    pub attributes: Vec<Attribute<'a>>,
    pub name: Ident<'a>,
    pub fields: Vec<(Option<Ident<'a>>, T<'a>)>,
    pub span: Span,
}

/// A stage callback: `enter(ctx) { ... }`, `leave(ctx, reply) => ...`.
#[derive(Debug, Clone)]
pub struct StageMember<'a> {
    pub kind: Ident<'a>,
    pub params: Vec<Param<'a>>,
    /// The callback's own result annotation (overrides the stage's).
    pub ret: Option<T<'a>>,
    pub body: E<'a>,
}

#[derive(Debug, Clone)]
pub enum Member<'a> {
    Fn(FnDecl<'a>),
    AssocType(Ident<'a>, Option<T<'a>>),
}

#[derive(Debug, Clone)]
pub struct Generic<'a> {
    pub name: Ident<'a>,
    pub bounds: Vec<T<'a>>,
}

#[derive(Debug, Clone)]
pub struct WherePred<'a> {
    pub ty: T<'a>,
    pub bounds: Vec<T<'a>>,
}

#[derive(Debug, Clone)]
pub struct Contract<'a> {
    pub ensures: bool,
    pub cond: E<'a>,
    pub message: Option<Lit<'a>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FnDecl<'a> {
    pub contracts: Vec<Contract<'a>>,
    pub is_pub: bool,
    pub name: Ident<'a>,
    pub generics: Vec<Generic<'a>>,
    pub params: Vec<Param<'a>>,
    pub ret: Option<T<'a>>,
    pub effect: Option<Effect<'a>>,
    pub where_preds: Vec<WherePred<'a>>,
    pub body: Option<Block<'a>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Param<'a> {
    pub pat: P<'a>,
    pub ty: Option<T<'a>>,
}

#[derive(Debug, Clone)]
pub enum Effect<'a> {
    Set(Vec<T<'a>>, Span),
    Var(Ident<'a>),
}

// ------------------------------------------------------------------ types

#[derive(Debug, Clone)]
pub enum Ty<'a> {
    Hole,
    /// `(value_ident .)* TypeIdent (. TypeIdent)? <args>?`
    Path {
        module: Vec<Ident<'a>>,
        names: Vec<Ident<'a>>,
        args: Vec<T<'a>>,
    },
    Unit,
    /// `(T)` is the inner type itself; `(A, B)` is a tuple.
    Tuple(Vec<T<'a>>),
    Fn {
        once: bool,
        params: Vec<T<'a>>,
        ret: Box<T<'a>>,
        effect: Option<Effect<'a>>,
    },
    Forall {
        generics: Vec<Generic<'a>>,
        body: Box<T<'a>>,
    },
    Ref {
        mutable: bool,
        inner: Box<T<'a>>,
    },
    Dyn(Box<T<'a>>),
    Record {
        fields: Vec<(Label<'a>, T<'a>)>,
        rest: Option<Ident<'a>>,
    },
    Effect(Effect<'a>),
    Error,
}

// ------------------------------------------------------------------ expressions

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
    Ref,
    RefMut,
}

#[derive(Debug, Clone)]
pub enum InterpPart<'a> {
    /// Literal text (escapes and `{{` not processed).
    Text(&'a str, Span),
    Hole(E<'a>),
}

#[derive(Debug, Clone)]
pub enum RecordEntry<'a> {
    Spread(E<'a>, Span),
    Field(Vec<Label<'a>>, E<'a>),
    Shorthand(Ident<'a>),
}

#[derive(Debug, Clone)]
pub enum Cond<'a> {
    Expr(Box<E<'a>>),
    /// `if let PATTERN = EXPR`
    Let(P<'a>, Box<E<'a>>),
}

#[derive(Debug, Clone)]
pub struct Block<'a> {
    pub items: Vec<BlockItem<'a>>,
    pub span: Span,
}

// short-lived: one per statement while a block is parsed
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum BlockItem<'a> {
    Stmt(Stmt<'a>),
    /// `;`: marks the expression statement before it.
    Semi,
}

#[derive(Debug, Clone)]
pub enum PlaceElem<'a> {
    Field(Ident<'a>),
    Index(E<'a>),
}

#[derive(Debug, Clone)]
pub enum Stmt<'a> {
    /// A local function declaration (lifted to its module, scoped to the
    /// block).
    Fn(Box<FnDecl<'a>>),
    /// `els`: `let PATTERN = VALUE else { ... }` (the block must diverge).
    Let {
        kind: LetKind,
        pat: P<'a>,
        ty: Option<T<'a>>,
        value: E<'a>,
        els: Option<Box<Block<'a>>>,
        span: Span,
    },
    Use {
        pats: Vec<P<'a>>,
        call: E<'a>,
        span: Span,
    },
    /// `op`: a compound assignment (`x += v` is `x = x + v`).
    Assign {
        base: Ident<'a>,
        path: Vec<PlaceElem<'a>>,
        op: Option<BinOp>,
        value: E<'a>,
        span: Span,
    },
    Expr(E<'a>),
}

#[derive(Debug, Clone)]
pub struct Arm<'a> {
    pub pat: P<'a>,
    /// Further alternatives (`.a | .b =>`): the arm is one arm per pattern.
    pub alts: Vec<P<'a>>,
    pub guard: Option<E<'a>>,
    pub body: E<'a>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Lambda<'a> {
    pub once: bool,
    pub params: Vec<Param<'a>>,
    pub ret: Option<T<'a>>,
    pub effect: Option<Effect<'a>>,
    pub body: Box<E<'a>>,
}

#[derive(Debug, Clone)]
pub enum Expr<'a> {
    Lit(Lit<'a>),
    /// An interpolated string; `true` for a raw one (`$"""..."""`).
    Interp(bool, Vec<InterpPart<'a>>),
    Name(Ident<'a>),
    TypeName(Ident<'a>),
    Unit,
    /// `(e)`: kept so that later checks can tell a parenthesized operand.
    Paren(Box<E<'a>>),
    Tuple(Vec<E<'a>>),
    List(Vec<E<'a>>),
    Record(Vec<RecordEntry<'a>>),
    Map(Vec<(E<'a>, E<'a>)>),
    Set(Vec<E<'a>>),
    Block(Block<'a>),
    Lambda(Lambda<'a>),
    If {
        cond: Cond<'a>,
        then: Block<'a>,
        els: Option<Box<E<'a>>>,
    },
    Match {
        scrutinee: Box<E<'a>>,
        arms: Vec<Arm<'a>>,
    },
    While {
        cond: Cond<'a>,
        body: Block<'a>,
    },
    For {
        pat: P<'a>,
        iter: Box<E<'a>>,
        body: Block<'a>,
    },
    Loop(Block<'a>),
    System(Vec<(Label<'a>, E<'a>)>),
    Pipeline(Vec<E<'a>>),
    /// `ref(#key)`: the symbol's text and span.
    Ref(&'a str, Span),
    Unsafe(Block<'a>),
    Return(Option<Box<E<'a>>>),
    Break(Option<Box<E<'a>>>),
    Continue,
    Unary {
        op: UnOp,
        operand: Box<E<'a>>,
    },
    Binary {
        op: BinOp,
        lhs: Box<E<'a>>,
        rhs: Box<E<'a>>,
    },
    /// Postfix operators carry the span of the operator itself.
    Call {
        callee: Box<E<'a>>,
        type_args: Vec<T<'a>>,
        args: Vec<E<'a>>,
    },
    Field {
        base: Box<E<'a>>,
        name: Ident<'a>,
    },
    Index {
        base: Box<E<'a>>,
        index: Box<E<'a>>,
    },
    Try(Box<E<'a>>),
    /// An expression that did not parse (the error is reported).
    /// `_`: a hole, reported with the type expected there.
    Hole,
    Error,
}

// ------------------------------------------------------------------ patterns

#[derive(Debug, Clone)]
pub enum Pat<'a> {
    Discard,
    Bind(Ident<'a>),
    Lit { negative: bool, lit: Lit<'a> },
    Unit,
    Tuple(Vec<P<'a>>),
    Record { fields: Vec<(Label<'a>, Option<P<'a>>)>, open: bool },
    Variant { ty: Option<(Vec<Ident<'a>>, Ident<'a>)>, name: Ident<'a>, args: Option<Vec<P<'a>>> },
    Error,
}
