use std::sync::LazyLock;

use lang_base::{Diagnostic, FileId, Interner, Span};
use pest::Parser as _;
use pest::iterators::{Pair, Pairs};
use pest::pratt_parser::{Assoc, Op, PrattParser};
use pest_derive::Parser;

use crate::ast::*;
use crate::layout;

#[derive(Parser)]
#[grammar = "grammar.pest"]
struct LangParser;

static PRATT: LazyLock<PrattParser<Rule>> = LazyLock::new(|| {
    use Assoc::Left;
    PrattParser::new()
        .op(Op::infix(Rule::pipe_op, Left))
        .op(Op::infix(Rule::or_op, Left))
        .op(Op::infix(Rule::and_op, Left))
        .op(Op::infix(Rule::eq_op, Left)
            | Op::infix(Rule::ne_op, Left)
            | Op::infix(Rule::lt_op, Left)
            | Op::infix(Rule::le_op, Left)
            | Op::infix(Rule::gt_op, Left)
            | Op::infix(Rule::ge_op, Left))
        .op(Op::infix(Rule::range_op, Left) | Op::infix(Rule::range_incl_op, Left))
        .op(Op::infix(Rule::bitor_op, Left))
        .op(Op::infix(Rule::bitxor_op, Left))
        .op(Op::infix(Rule::bitand_op, Left))
        .op(Op::infix(Rule::shl_op, Left) | Op::infix(Rule::shr_op, Left))
        .op(Op::infix(Rule::add_op, Left) | Op::infix(Rule::sub_op, Left))
        .op(Op::infix(Rule::mul_op, Left) | Op::infix(Rule::div_op, Left) | Op::infix(Rule::rem_op, Left))
        .op(Op::prefix(Rule::neg_op)
            | Op::prefix(Rule::not_op)
            | Op::prefix(Rule::ref_op)
            | Op::prefix(Rule::ref_mut_op))
        .op(Op::postfix(Rule::call_op)
            | Op::postfix(Rule::field_op)
            | Op::postfix(Rule::index_op)
            | Op::postfix(Rule::try_op))
});

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl From<ParseError> for Diagnostic {
    fn from(e: ParseError) -> Self {
        Diagnostic::error("syntax error").with_code("E0001").with_label(e.span, e.message)
    }
}

/// Parse a whole file. Syntax errors abort the parse; case-rule and other
/// lexical-structure violations are reported in `diags` and parsing continues.
pub fn parse(
    file: FileId,
    src: &str,
    interner: &mut Interner,
    diags: &mut Vec<Diagnostic>,
) -> Result<Ast, ParseError> {
    let text = layout::layout(src);
    let mut pairs = LangParser::parse(Rule::program, &text).map_err(|e| {
        let (start, end) = match e.location {
            pest::error::InputLocation::Pos(p) => (p, p),
            pest::error::InputLocation::Span((s, e)) => (s, e),
        };
        let message = match &e.variant {
            pest::error::ErrorVariant::ParsingError { positives, .. } => {
                let mut names: Vec<String> = positives.iter().filter_map(|r| describe(*r)).collect();
                names.sort();
                names.dedup();
                if names.is_empty() {
                    "unexpected input".to_string()
                } else {
                    format!("expected {}", names.join(", "))
                }
            }
            pest::error::ErrorVariant::CustomError { message } => message.clone(),
        };
        ParseError { message, span: Span::new(file, start, end.max(start)) }
    })?;
    let program = pairs.next().unwrap();
    let mut lower = Lower { file, interner, ast: Ast::default(), diags, parenthesized: Default::default() };
    for p in program.into_inner() {
        if p.as_rule() == Rule::item {
            let item = lower.item(p);
            lower.push_item(item);
        }
    }
    Ok(lower.ast)
}

/// The names a pattern binds, in order.
fn bound_names(ast: &Ast, p: PatId, out: &mut Vec<Name>) {
    match &ast.pats[p].kind {
        PatKind::Bind(n) => out.push(*n),
        PatKind::Tuple(ps) => ps.iter().for_each(|q| bound_names(ast, *q, out)),
        PatKind::Record { fields, .. } => {
            for (label, sub) in fields {
                match sub {
                    Some(q) => bound_names(ast, *q, out),
                    None => out.push(Name { sym: label.sym, span: label.span }),
                }
            }
        }
        PatKind::Variant { args: Some(ps), .. } => ps.iter().for_each(|q| bound_names(ast, *q, out)),
        _ => {}
    }
}

impl Lower<'_> {
    /// Add an item; a module-level binding of a destructuring pattern
    /// becomes a hidden value plus one binding per name, each taking its
    /// part (`let (a, b) = e` binds `a` and `b`).
    fn push_item(&mut self, item: Item) {
        let ItemKind::Global { kind, pat, ty, value } = &item.kind else {
            self.ast.items.push(item);
            return;
        };
        if matches!(self.ast.pats[*pat].kind, PatKind::Bind(_)) {
            self.ast.items.push(item);
            return;
        }
        let (kind, pat, ty, value, span) = (*kind, *pat, *ty, *value, item.span);
        let mut names = vec![];
        bound_names(&self.ast, pat, &mut names);
        let hidden =
            Name { sym: self.interner.intern(&format!("destructured_{}", self.ast.items.len())), span };
        let hidden_pat = self.ast.pats.alloc(Pat { kind: PatKind::Bind(hidden), span });
        self.ast.items.push(Item {
            kind: ItemKind::Global { kind, pat: hidden_pat, ty, value },
            span,
            is_pub: false,
        });
        for n in names {
            let whole = self.alloc_expr(ExprKind::Name(hidden), n.span);
            let part = self.alloc_expr(ExprKind::Name(n), n.span);
            let block = self.alloc_expr(
                ExprKind::Block {
                    stmts: vec![
                        Stmt::Let { kind: LetKind::Let, pat, ty: None, value: whole, span },
                        Stmt::Expr { expr: part, semi: false },
                    ],
                },
                n.span,
            );
            let bind = self.ast.pats.alloc(Pat { kind: PatKind::Bind(n), span: n.span });
            self.ast.items.push(Item {
                kind: ItemKind::Global { kind, pat: bind, ty: None, value: block },
                span,
                is_pub: item.is_pub,
            });
        }
    }
}

/// Parse a single expression (used by the REPL and tests).
pub fn parse_expr(
    file: FileId,
    src: &str,
    interner: &mut Interner,
    diags: &mut Vec<Diagnostic>,
) -> Result<(Ast, ExprId), ParseError> {
    let wrapped = format!("let __repl = {src}");
    let ast = parse(file, &wrapped, interner, diags)?;
    let ItemKind::Global { value, .. } = ast.items[0].kind else { unreachable!() };
    Ok((ast, value))
}

fn describe(r: Rule) -> Option<String> {
    let s = match r {
        Rule::expr => "expression",
        Rule::pattern => "pattern",
        Rule::type_expr => "type",
        Rule::item => "declaration",
        Rule::stmt => "statement",
        Rule::block => "block",
        Rule::name | Rule::value_ident => "identifier",
        Rule::type_ident => "type name",
        Rule::member => "field name",
        Rule::EOI => "end of input",
        Rule::semi => "`;`",
        Rule::args => "arguments",
        Rule::params | Rule::param => "parameter",
        Rule::label | Rule::label_path => "label",
        Rule::match_arm => "match arm",
        Rule::string => "string",
        Rule::symbol => "symbol",
        Rule::ret_type => "`->`",
        Rule::effect => "effect",
        Rule::where_clause => "`where`",
        Rule::generic_params | Rule::type_args => "`<`",
        Rule::call_op => "`(`",
        Rule::field_op => "`.`",
        Rule::index_op => "`[`",
        Rule::try_op => "`?`",
        Rule::pipe_op
        | Rule::or_op
        | Rule::and_op
        | Rule::eq_op
        | Rule::ne_op
        | Rule::lt_op
        | Rule::le_op
        | Rule::gt_op
        | Rule::ge_op
        | Rule::add_op
        | Rule::sub_op
        | Rule::mul_op
        | Rule::div_op
        | Rule::rem_op => "operator",
        _ => return None,
    };
    Some(s.to_string())
}

struct Lower<'a> {
    file: FileId,
    interner: &'a mut Interner,
    ast: Ast,
    diags: &'a mut Vec<Diagnostic>,
    /// Expressions written in parentheses.
    parenthesized: std::collections::HashSet<ExprId>,
}

fn is_upper(s: &str) -> bool {
    s.starts_with(|c: char| c.is_ascii_uppercase())
}

impl Lower<'_> {
    fn span(&self, p: &Pair<'_, Rule>) -> Span {
        let s = p.as_span();
        Span::new(self.file, s.start(), s.end())
    }

    fn name(&mut self, p: Pair<'_, Rule>) -> Name {
        let span = self.span(&p);
        Name { sym: self.interner.intern(p.as_str()), span }
    }

    fn error(&mut self, code: &'static str, span: Span, msg: impl Into<String>, label: impl Into<String>) {
        self.diags.push(Diagnostic::error(msg).with_code(code).with_label(span, label));
    }

    /// A declaration name that must start with an uppercase letter.
    fn type_name(&mut self, p: Pair<'_, Rule>) -> Name {
        let n = self.name(p);
        let s = self.interner.resolve(n.sym);
        if !is_upper(s) {
            let msg = format!("type name `{s}` must start with an uppercase letter");
            self.error("E0101", n.span, msg, "types, protocols and type parameters are uppercase");
        }
        n
    }

    /// A declaration name that must start lowercase (functions, values, variants, fields).
    fn value_name(&mut self, p: Pair<'_, Rule>) -> Name {
        let n = self.name(p);
        let s = self.interner.resolve(n.sym);
        if is_upper(s) {
            let msg = format!("value name `{s}` must start with a lowercase letter");
            self.error("E0102", n.span, msg, "uppercase identifiers name types");
        }
        n
    }

    fn alloc_expr(&mut self, kind: ExprKind, span: Span) -> ExprId {
        self.ast.exprs.alloc(Expr { kind, span })
    }

    fn alloc_pat(&mut self, kind: PatKind, span: Span) -> PatId {
        self.ast.pats.alloc(Pat { kind, span })
    }

    fn alloc_ty(&mut self, kind: TypeKind, span: Span) -> TypeId {
        self.ast.types.alloc(TypeExpr { kind, span })
    }

    // ------------------------------------------------------------ items

    fn item(&mut self, p: Pair<'_, Rule>) -> Item {
        let span = self.span(&p);
        let inner = p.into_inner().next().unwrap();
        let rule = inner.as_rule();
        let mut parts = inner.into_inner().peekable();
        let is_pub = parts.peek().is_some_and(|q| q.as_rule() == Rule::pub_kw);
        if is_pub {
            parts.next();
        }
        let kind = match rule {
            Rule::import_decl => {
                let path_pair = parts.next().unwrap();
                let path: Vec<Name> = path_pair.into_inner().map(|n| self.name(n)).collect();
                let mut alias = None;
                let mut items = None;
                for q in parts {
                    match q.as_rule() {
                        Rule::name => alias = Some(self.name(q)),
                        Rule::import_item => {
                            let mut it = q.into_inner();
                            let n = self.name(it.next().unwrap());
                            let a = it.next().map(|a| self.name(a));
                            items.get_or_insert_with(Vec::new).push((n, a));
                        }
                        _ => {}
                    }
                }
                ItemKind::Import(Import { path, alias, items })
            }
            Rule::type_decl => {
                let name = self.type_name(parts.next().unwrap());
                let mut generics = vec![];
                let mut ty = None;
                for q in parts {
                    match q.as_rule() {
                        Rule::generic_params => generics = self.generic_params(q),
                        Rule::type_expr => ty = Some(self.type_expr(q)),
                        _ => {}
                    }
                }
                ItemKind::TypeAlias { name, generics, ty: ty.unwrap() }
            }
            Rule::struct_decl => {
                let mut repr_c = false;
                let mut name = None;
                let mut generics = vec![];
                let mut fields = vec![];
                let mut derives = vec![];
                for q in parts {
                    match q.as_rule() {
                        Rule::repr_c => repr_c = true,
                        Rule::name => name = Some(self.type_name(q)),
                        Rule::generic_params => generics = self.generic_params(q),
                        Rule::struct_field => {
                            let mut it = q.into_inner().peekable();
                            let fpub = it.peek().is_some_and(|x| x.as_rule() == Rule::pub_kw);
                            if fpub {
                                it.next();
                            }
                            let fname = self.value_name(it.next().unwrap());
                            let fty = self.type_expr(it.next().unwrap());
                            fields.push(StructField { is_pub: fpub, name: fname, ty: fty });
                        }
                        Rule::derives => derives = self.derives(q),
                        _ => {}
                    }
                }
                ItemKind::Struct(StructDecl { name: name.unwrap(), generics, fields, derives, repr_c })
            }
            Rule::enum_decl => {
                let name = self.type_name(parts.next().unwrap());
                let mut generics = vec![];
                let mut variants = vec![];
                let mut derives = vec![];
                for q in parts {
                    match q.as_rule() {
                        Rule::generic_params => generics = self.generic_params(q),
                        Rule::variant => {
                            let vspan = self.span(&q);
                            let mut it = q.into_inner();
                            let vname = self.value_name(it.next().unwrap());
                            let mut fields = vec![];
                            for f in it {
                                let mut fi = f.into_inner().peekable();
                                let label = if fi
                                    .peek()
                                    .is_some_and(|x| matches!(x.as_rule(), Rule::name | Rule::member))
                                {
                                    Some(self.value_name(fi.next().unwrap()))
                                } else {
                                    None
                                };
                                let fty = self.type_expr(fi.next().unwrap());
                                fields.push((label, fty));
                            }
                            variants.push(Variant { name: vname, fields, span: vspan });
                        }
                        Rule::derives => derives = self.derives(q),
                        _ => {}
                    }
                }
                ItemKind::Enum(EnumDecl { name, generics, variants, derives })
            }
            Rule::protocol_decl => {
                let name = self.type_name(parts.next().unwrap());
                let mut generics = vec![];
                let mut supers = vec![];
                let mut methods = vec![];
                let mut assoc_types = vec![];
                for q in parts {
                    match q.as_rule() {
                        Rule::generic_params => generics = self.generic_params(q),
                        Rule::type_expr => supers.push(self.type_expr(q)),
                        Rule::fn_decl | Rule::method_decl => methods.push(self.fn_decl(q)),
                        Rule::assoc_type => assoc_types.push(self.assoc_type(q)),
                        _ => {}
                    }
                }
                ItemKind::Protocol(ProtocolDecl { name, generics, supers, methods, assoc_types })
            }
            Rule::impl_decl => {
                let mut generics = vec![];
                let mut first = None;
                let mut second = None;
                let mut where_preds = vec![];
                let mut methods = vec![];
                let mut assoc_types = vec![];
                for q in parts {
                    match q.as_rule() {
                        Rule::generic_params => generics = self.generic_params(q),
                        Rule::type_expr => first = Some(self.type_expr(q)),
                        Rule::impl_for => second = Some(self.type_expr(q.into_inner().next().unwrap())),
                        Rule::where_clause => where_preds = self.where_clause(q),
                        Rule::fn_decl | Rule::method_decl => methods.push(self.fn_decl(q)),
                        Rule::assoc_type => assoc_types.push(self.assoc_type(q)),
                        _ => {}
                    }
                }
                let (protocol, self_ty) = match second {
                    Some(s) => (first, s),
                    None => (None, first.unwrap()),
                };
                ItemKind::Impl(ImplDecl { generics, protocol, self_ty, where_preds, methods, assoc_types })
            }
            Rule::fn_decl => {
                let mut d = self.fn_decl_parts(parts, span);
                d.is_pub = is_pub;
                if d.body.is_none() {
                    self.error(
                        "E0002",
                        d.name.span,
                        "function declaration without a body",
                        "add a body `{ ... }`",
                    );
                }
                ItemKind::Fn(d)
            }
            Rule::extern_type => {
                let name = self.type_name(parts.next().unwrap());
                let generics = parts.next().map(|g| self.generic_params(g)).unwrap_or_default();
                ItemKind::ExternType { name, generics }
            }
            Rule::extern_decl => {
                let abi_pair = parts.next().unwrap();
                let abi = self.string_content(abi_pair);
                let mut d = self.fn_decl_parts(parts, span);
                d.is_pub = is_pub;
                ItemKind::Extern { abi, decl: d }
            }
            Rule::global_decl => {
                let kind = self.let_kind(parts.next().unwrap());
                let pat = self.pattern(parts.next().unwrap());
                let mut ty = None;
                let mut value = None;
                for q in parts {
                    match q.as_rule() {
                        Rule::type_expr => ty = Some(self.type_expr(q)),
                        Rule::expr => value = Some(self.expr(q)),
                        _ => {}
                    }
                }
                ItemKind::Global { kind, pat, ty, value: value.unwrap() }
            }
            r => unreachable!("item {r:?}"),
        };
        Item { kind, span, is_pub }
    }

    fn assoc_type(&mut self, p: Pair<'_, Rule>) -> (Name, Option<TypeId>) {
        let mut it = p.into_inner();
        let n = self.type_name(it.next().unwrap());
        let t = it.next().map(|t| self.type_expr(t));
        (n, t)
    }

    fn let_kind(&mut self, p: Pair<'_, Rule>) -> LetKind {
        match p.as_str().trim() {
            "let" => LetKind::Let,
            "var" => LetKind::Var,
            _ => LetKind::Const,
        }
    }

    fn derives(&mut self, p: Pair<'_, Rule>) -> Vec<TypeId> {
        p.into_inner().map(|t| self.path_type(t)).collect()
    }

    fn fn_decl(&mut self, p: Pair<'_, Rule>) -> FnDecl {
        let span = self.span(&p);
        let mut parts = p.into_inner().peekable();
        let is_pub = parts.peek().is_some_and(|q| q.as_rule() == Rule::pub_kw);
        if is_pub {
            parts.next();
        }
        let mut d = self.fn_decl_parts(parts, span);
        d.is_pub = is_pub;
        d
    }

    fn fn_decl_parts<'i>(&mut self, parts: impl Iterator<Item = Pair<'i, Rule>>, span: Span) -> FnDecl {
        let mut parts = parts.peekable();
        let name = self.value_name(parts.next().unwrap());
        let mut d = FnDecl {
            name,
            is_pub: false,
            generics: vec![],
            params: vec![],
            ret: None,
            effect: None,
            where_preds: vec![],
            body: None,
            span,
        };
        for q in parts {
            match q.as_rule() {
                Rule::generic_params => d.generics = self.generic_params(q),
                Rule::params => d.params = self.params(q),
                Rule::ret_type => d.ret = Some(self.type_expr(q.into_inner().next().unwrap())),
                Rule::effect => d.effect = Some(self.effect(q)),
                Rule::where_clause => d.where_preds = self.where_clause(q),
                Rule::block => d.body = Some(self.block(q)),
                _ => {}
            }
        }
        d
    }

    fn params(&mut self, p: Pair<'_, Rule>) -> Vec<Param> {
        p.into_inner()
            .map(|param| {
                let mut it = param.into_inner();
                let pat = self.pattern(it.next().unwrap());
                let ty = it.next().map(|t| self.type_expr(t));
                Param { pat, ty }
            })
            .collect()
    }

    fn generic_params(&mut self, p: Pair<'_, Rule>) -> Vec<GenericParam> {
        p.into_inner()
            .map(|g| {
                let mut it = g.into_inner();
                let name = self.type_name(it.next().unwrap());
                let bounds = it.map(|b| self.type_expr(b)).collect();
                GenericParam { name, bounds }
            })
            .collect()
    }

    fn where_clause(&mut self, p: Pair<'_, Rule>) -> Vec<WherePred> {
        p.into_inner()
            .map(|w| {
                let mut it = w.into_inner();
                let ty = self.type_expr(it.next().unwrap());
                let bounds = it.map(|b| self.type_expr(b)).collect();
                WherePred { ty, bounds }
            })
            .collect()
    }

    fn effect(&mut self, p: Pair<'_, Rule>) -> Effect {
        let span = self.span(&p);
        let mut it = p.into_inner().peekable();
        match it.peek().map(|q| q.as_rule()) {
            Some(Rule::type_ident) => Effect::Var(self.name(it.next().unwrap())),
            _ => Effect::Set(it.map(|t| self.type_expr(t)).collect(), span),
        }
    }

    // ------------------------------------------------------------ types

    fn type_expr(&mut self, p: Pair<'_, Rule>) -> TypeId {
        let inner = if p.as_rule() == Rule::type_expr { p.into_inner().next().unwrap() } else { p };
        let span = self.span(&inner);
        match inner.as_rule() {
            Rule::hole_type => self.alloc_ty(TypeKind::Hole, span),
            Rule::path_type => self.path_type(inner),
            Rule::unit_type => self.alloc_ty(TypeKind::Unit, span),
            Rule::paren_type => self.type_expr(inner.into_inner().next().unwrap()),
            Rule::tuple_type => {
                let items = inner.into_inner().map(|t| self.type_expr(t)).collect();
                self.alloc_ty(TypeKind::Tuple(items), span)
            }
            Rule::fn_type => {
                let mut once = false;
                let mut tys = vec![];
                let mut effect = None;
                for q in inner.into_inner() {
                    match q.as_rule() {
                        Rule::once_kw => once = true,
                        Rule::type_expr => tys.push(self.type_expr(q)),
                        Rule::effect => effect = Some(self.effect(q)),
                        _ => {}
                    }
                }
                let ret = tys.pop().unwrap();
                self.alloc_ty(TypeKind::Fn { once, params: tys, ret, effect }, span)
            }
            Rule::forall_type => {
                let mut it = inner.into_inner();
                let generics = self.generic_params(it.next().unwrap());
                let body = self.type_expr(it.next().unwrap());
                self.alloc_ty(TypeKind::Forall { generics, body }, span)
            }
            Rule::ref_type => {
                let mut mutable = false;
                let mut t = None;
                for q in inner.into_inner() {
                    match q.as_rule() {
                        Rule::mut_kw => mutable = true,
                        _ => t = Some(self.type_expr(q)),
                    }
                }
                self.alloc_ty(TypeKind::Ref { mutable, inner: t.unwrap() }, span)
            }
            Rule::dyn_type => {
                let t = self.path_type(inner.into_inner().next().unwrap());
                let TypeKind::Path { module, name, args } = self.ast.types[t].kind.clone() else {
                    unreachable!()
                };
                self.alloc_ty(TypeKind::Dyn { module, name, args }, span)
            }
            Rule::record_type => {
                let mut fields = vec![];
                let mut rest = None;
                for q in inner.into_inner() {
                    match q.as_rule() {
                        Rule::type_field => {
                            let mut it = q.into_inner();
                            let l = self.label(it.next().unwrap());
                            let t = self.type_expr(it.next().unwrap());
                            fields.push((l, t));
                        }
                        Rule::row_rest => rest = Some(self.name(q.into_inner().next().unwrap())),
                        _ => {}
                    }
                }
                self.check_dup_labels(fields.iter().map(|f| f.0));
                self.alloc_ty(TypeKind::Record { fields, rest }, span)
            }
            r => unreachable!("type {r:?}"),
        }
    }

    fn path_type(&mut self, p: Pair<'_, Rule>) -> TypeId {
        let span = self.span(&p);
        let mut module = vec![];
        let mut name = None;
        let mut args = vec![];
        for q in p.into_inner() {
            match q.as_rule() {
                Rule::value_ident => module.push(self.name(q)),
                Rule::type_ident => {
                    // `T.Item`: the first type name qualifies an associated type
                    let n = self.name(q);
                    if let Some(prev) = name.replace(n) {
                        module.push(prev);
                    }
                }
                Rule::type_args => args = q.into_inner().map(|t| self.type_expr(t)).collect(),
                _ => {}
            }
        }
        self.alloc_ty(TypeKind::Path { module, name: name.unwrap(), args }, span)
    }

    fn label(&mut self, p: Pair<'_, Rule>) -> Label {
        let span = self.span(&p);
        let inner = p.into_inner().next().unwrap();
        let text = match inner.as_rule() {
            Rule::symbol => inner.as_str()[1..].to_string(),
            _ => {
                let t = inner.as_str().to_string();
                if is_upper(&t) {
                    let msg = format!("record label `{t}` must start with a lowercase letter");
                    self.error("E0102", span, msg, "ordinary labels are lowercase identifiers");
                }
                t
            }
        };
        Label { sym: self.interner.intern(&text), span }
    }

    fn check_dup_labels(&mut self, labels: impl Iterator<Item = Label>) {
        let mut seen: Vec<Label> = vec![];
        for l in labels {
            if let Some(prev) = seen.iter().find(|p| p.sym == l.sym) {
                let s = self.interner.resolve(l.sym).to_string();
                self.diags.push(
                    Diagnostic::error(format!("duplicate field `{s}`"))
                        .with_code("E0103")
                        .with_label(l.span, "duplicate occurrence")
                        .with_label(prev.span, "first occurrence"),
                );
            } else {
                seen.push(l);
            }
        }
    }

    // ------------------------------------------------------------ statements

    fn block(&mut self, p: Pair<'_, Rule>) -> ExprId {
        let span = self.span(&p);
        let mut stmts: Vec<Stmt> = vec![];
        for q in p.into_inner() {
            match q.as_rule() {
                Rule::semi => {
                    if let Some(Stmt::Expr { semi, .. }) = stmts.last_mut() {
                        *semi = true;
                    }
                }
                Rule::stmt => {
                    let s = self.stmt(q);
                    stmts.push(s);
                }
                _ => {}
            }
        }
        self.alloc_expr(ExprKind::Block { stmts }, span)
    }

    fn stmt(&mut self, p: Pair<'_, Rule>) -> Stmt {
        let inner = p.into_inner().next().unwrap();
        let span = self.span(&inner);
        match inner.as_rule() {
            Rule::let_stmt => {
                let mut it = inner.into_inner();
                let kind = self.let_kind(it.next().unwrap());
                let pat = self.pattern(it.next().unwrap());
                let mut ty = None;
                let mut value = None;
                for q in it {
                    match q.as_rule() {
                        Rule::type_expr => ty = Some(self.type_expr(q)),
                        Rule::expr => value = Some(self.expr(q)),
                        _ => {}
                    }
                }
                Stmt::Let { kind, pat, ty, value: value.unwrap(), span }
            }
            Rule::use_stmt => {
                let mut pats = vec![];
                let mut call = None;
                for q in inner.into_inner() {
                    match q.as_rule() {
                        Rule::pattern => pats.push(self.pattern(q)),
                        Rule::expr => call = Some(self.expr(q)),
                        _ => {}
                    }
                }
                Stmt::Use { pats, call: call.unwrap(), span }
            }
            Rule::assign_stmt => {
                let mut it = inner.into_inner();
                let place = it.next().unwrap();
                let value = self.expr(it.next().unwrap());
                let mut pi = place.into_inner();
                let base = self.name(pi.next().unwrap());
                let mut path = vec![];
                for q in pi {
                    match q.as_rule() {
                        Rule::place_field => {
                            let m = q.into_inner().next().unwrap();
                            let span = self.span(&m);
                            path.push(PlaceElem::Field(Label {
                                sym: self.interner.intern(m.as_str()),
                                span,
                            }));
                        }
                        Rule::place_index => {
                            let e = self.expr(q.into_inner().next().unwrap());
                            path.push(PlaceElem::Index(e));
                        }
                        _ => {}
                    }
                }
                Stmt::Assign { base, path, value, span }
            }
            Rule::expr => Stmt::Expr { expr: self.expr(inner), semi: false },
            r => unreachable!("stmt {r:?}"),
        }
    }

    // ------------------------------------------------------------ expressions

    fn expr(&mut self, p: Pair<'_, Rule>) -> ExprId {
        debug_assert_eq!(p.as_rule(), Rule::expr);
        let span = self.span(&p);
        let id = self.pratt(p.into_inner());
        // Comparisons do not chain.
        if let ExprKind::Binary { op, lhs, rhs } = self.ast.exprs[id].kind
            && op.is_comparison()
        {
            // a parenthesized comparison is one operand
            let chained = |e: ExprId, this: &Self| {
                !this.parenthesized.contains(&e)
                    && matches!(this.ast.exprs[e].kind, ExprKind::Binary { op, .. } if op.is_comparison())
            };
            if chained(lhs, self) || chained(rhs, self) {
                self.error(
                    "E0003",
                    span,
                    "comparison operators do not chain",
                    "use `&&` to combine comparisons",
                );
            }
        }
        id
    }

    fn pratt(&mut self, pairs: Pairs<'_, Rule>) -> ExprId {
        let this = std::cell::RefCell::new(self);
        PRATT
            .map_primary(|p| this.borrow_mut().primary(p))
            .map_prefix(|op, operand| {
                let mut this = this.borrow_mut();
                let span = this.span(&op).to(this.ast.exprs[operand].span);
                let op = match op.as_rule() {
                    Rule::neg_op => UnOp::Neg,
                    Rule::not_op => UnOp::Not,
                    Rule::ref_op => UnOp::Ref,
                    Rule::ref_mut_op => UnOp::RefMut,
                    r => unreachable!("prefix {r:?}"),
                };
                this.alloc_expr(ExprKind::Unary { op, operand }, span)
            })
            .map_postfix(|operand, op| {
                let mut this = this.borrow_mut();
                let span = this.ast.exprs[operand].span.to(this.span(&op));
                match op.as_rule() {
                    Rule::call_op => {
                        let mut type_args = vec![];
                        let mut args = vec![];
                        for q in op.into_inner() {
                            match q.as_rule() {
                                Rule::type_args => {
                                    type_args = q.into_inner().map(|t| this.type_expr(t)).collect()
                                }
                                Rule::args => args = q.into_inner().map(|a| this.expr(a)).collect(),
                                _ => {}
                            }
                        }
                        this.alloc_expr(ExprKind::Call { callee: operand, type_args, args }, span)
                    }
                    Rule::field_op => {
                        let m = op.into_inner().next().unwrap();
                        let name = this.name(m);
                        this.alloc_expr(ExprKind::Field { base: operand, name }, span)
                    }
                    Rule::index_op => {
                        let index = this.expr(op.into_inner().next().unwrap());
                        this.alloc_expr(ExprKind::Index { base: operand, index }, span)
                    }
                    Rule::try_op => this.alloc_expr(ExprKind::Try(operand), span),
                    r => unreachable!("postfix {r:?}"),
                }
            })
            .map_infix(|lhs, op, rhs| {
                let mut this = this.borrow_mut();
                let op = match op.as_rule() {
                    Rule::pipe_op => BinOp::Pipe,
                    Rule::or_op => BinOp::Or,
                    Rule::and_op => BinOp::And,
                    Rule::eq_op => BinOp::Eq,
                    Rule::ne_op => BinOp::Ne,
                    Rule::lt_op => BinOp::Lt,
                    Rule::le_op => BinOp::Le,
                    Rule::gt_op => BinOp::Gt,
                    Rule::ge_op => BinOp::Ge,
                    Rule::range_op => BinOp::Range,
                    Rule::range_incl_op => BinOp::RangeIncl,
                    Rule::bitor_op => BinOp::BitOr,
                    Rule::bitxor_op => BinOp::BitXor,
                    Rule::bitand_op => BinOp::BitAnd,
                    Rule::shl_op => BinOp::Shl,
                    Rule::shr_op => BinOp::Shr,
                    Rule::add_op => BinOp::Add,
                    Rule::sub_op => BinOp::Sub,
                    Rule::mul_op => BinOp::Mul,
                    Rule::div_op => BinOp::Div,
                    Rule::rem_op => BinOp::Rem,
                    r => unreachable!("infix {r:?}"),
                };
                let span = this.ast.exprs[lhs].span.to(this.ast.exprs[rhs].span);
                this.alloc_expr(ExprKind::Binary { op, lhs, rhs }, span)
            })
            .parse(pairs)
    }

    fn string_content(&mut self, p: Pair<'_, Rule>) -> String {
        let span = self.span(&p);
        match p.as_rule() {
            Rule::string => {
                let raw = p.into_inner().next().map(|c| c.as_str()).unwrap_or("");
                self.unescape(raw, span)
            }
            Rule::raw_string => p.into_inner().next().map(|c| c.as_str().to_string()).unwrap_or_default(),
            r => unreachable!("string {r:?}"),
        }
    }

    fn unescape(&mut self, raw: &str, span: Span) -> String {
        let mut out = String::with_capacity(raw.len());
        let mut chars = raw.chars();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('0') => out.push('\0'),
                Some('\\') => out.push('\\'),
                Some('"') => out.push('"'),
                Some('\'') => out.push('\''),
                Some('u') => {
                    let rest: String = chars.by_ref().take_while(|&c| c != '}').collect();
                    let hex = rest.trim_start_matches('{');
                    match u32::from_str_radix(hex, 16).ok().and_then(char::from_u32) {
                        Some(ch) => out.push(ch),
                        None => {
                            self.error("E0004", span, "invalid unicode escape", "not a Unicode scalar value")
                        }
                    }
                }
                _ => self.error("E0004", span, "invalid escape sequence", "unknown escape"),
            }
        }
        out
    }

    fn interp_text(&mut self, raw: &str, span: Span) -> String {
        let s = raw.replace("{{", "\u{0}LB").replace("}}", "\u{0}RB");
        let s = self.unescape(&s, span);
        s.replace("\u{0}LB", "{").replace("\u{0}RB", "}")
    }

    fn lit(&mut self, p: Pair<'_, Rule>) -> Lit {
        let span = self.span(&p);
        let text = p.as_str();
        match p.as_rule() {
            Rule::int => {
                let (digits, suffix) = split_suffix(text);
                let clean: String = digits.chars().filter(|&c| c != '_').collect();
                let parsed = if let Some(h) = clean.strip_prefix("0x") {
                    u128::from_str_radix(h, 16)
                } else if let Some(b) = clean.strip_prefix("0b") {
                    u128::from_str_radix(b, 2)
                } else {
                    clean.parse::<u128>()
                };
                let value = parsed.unwrap_or_else(|_| {
                    self.error(
                        "E0005",
                        span,
                        "integer literal is too large",
                        "does not fit any integer type",
                    );
                    0
                });
                let suffix = suffix.map(|s| self.interner.intern(s));
                if let Some(s) = suffix
                    && self.interner.resolve(s).starts_with('F')
                {
                    return Lit::Float { value: value as f64, suffix: Some(s) };
                }
                Lit::Int { value, suffix }
            }
            Rule::float => {
                let (digits, suffix) = split_suffix(text);
                let clean: String = digits.chars().filter(|&c| c != '_').collect();
                let value = clean.parse::<f64>().unwrap_or(0.0);
                let suffix = suffix.map(|s| self.interner.intern(s));
                if let Some(s) = suffix
                    && !self.interner.resolve(s).starts_with('F')
                {
                    self.error("E0005", span, "integer suffix on a decimal literal", "use F32 or F64");
                }
                Lit::Float { value, suffix }
            }
            Rule::duration => {
                let unit_start = text.find(|c: char| c.is_ascii_alphabetic()).unwrap();
                let (n, u) = text.split_at(unit_start);
                let value = n.replace('_', "").parse::<u128>().unwrap_or(0);
                let unit = match u {
                    "ns" => DurationUnit::Ns,
                    "us" => DurationUnit::Us,
                    "ms" => DurationUnit::Ms,
                    "s" => DurationUnit::S,
                    "min" => DurationUnit::Min,
                    _ => DurationUnit::H,
                };
                Lit::Duration { value, unit }
            }
            Rule::string | Rule::raw_string => Lit::Str(self.string_content(p)),
            Rule::char_lit => {
                let raw = p.into_inner().next().unwrap().as_str();
                let s = self.unescape(raw, span);
                Lit::Char(s.chars().next().unwrap_or('\0'))
            }
            Rule::symbol => Lit::Symbol(self.interner.intern(&text[1..])),
            Rule::bool_lit => Lit::Bool(text == "true"),
            r => unreachable!("lit {r:?}"),
        }
    }

    fn primary(&mut self, p: Pair<'_, Rule>) -> ExprId {
        let span = self.span(&p);
        match p.as_rule() {
            Rule::int
            | Rule::float
            | Rule::duration
            | Rule::string
            | Rule::raw_string
            | Rule::char_lit
            | Rule::symbol
            | Rule::bool_lit => {
                let l = self.lit(p);
                self.alloc_expr(ExprKind::Lit(l), span)
            }
            Rule::interp | Rule::interp_raw => {
                let mut parts = vec![];
                for q in p.into_inner() {
                    match q.as_rule() {
                        Rule::interp_text => {
                            let qs = self.span(&q);
                            let t = self.interp_text(q.as_str(), qs);
                            parts.push(InterpPart::Text(t));
                        }
                        Rule::interp_raw_text => {
                            let t = q.as_str().replace("{{", "{").replace("}}", "}");
                            parts.push(InterpPart::Text(t));
                        }
                        Rule::interp_hole => {
                            let e = self.expr(q.into_inner().next().unwrap());
                            parts.push(InterpPart::Expr(e));
                        }
                        _ => {}
                    }
                }
                self.alloc_expr(ExprKind::Interp(parts), span)
            }
            Rule::name_expr => {
                let n = self.name(p.into_inner().next().unwrap());
                self.alloc_expr(ExprKind::Name(n), span)
            }
            Rule::type_name_expr => {
                let n = self.name(p.into_inner().next().unwrap());
                self.alloc_expr(ExprKind::TypeName(n), span)
            }
            Rule::unit_lit => self.alloc_expr(ExprKind::Lit(Lit::Unit), span),
            Rule::paren => {
                let e = self.expr(p.into_inner().next().unwrap());
                self.parenthesized.insert(e);
                e
            }
            Rule::tuple_lit => {
                let items = p.into_inner().map(|e| self.expr(e)).collect();
                self.alloc_expr(ExprKind::Tuple(items), span)
            }
            Rule::list_lit => {
                let items = p.into_inner().map(|e| self.expr(e)).collect();
                self.alloc_expr(ExprKind::List(items), span)
            }
            Rule::record_lit => {
                let mut spread = None;
                let mut entries = vec![];
                let mut labels = vec![];
                for q in p.into_inner() {
                    match q.as_rule() {
                        Rule::spread => {
                            let qs = self.span(&q);
                            let e = self.expr(q.into_inner().next().unwrap());
                            if spread.is_some() || !entries.is_empty() {
                                self.error(
                                    "E0104",
                                    qs,
                                    "a record spread must come first",
                                    "only one leading spread is allowed",
                                );
                            }
                            spread = Some(e);
                        }
                        Rule::field_entry => {
                            let mut it = q.into_inner();
                            let lp = it.next().unwrap();
                            let path: Vec<Label> = lp.into_inner().map(|l| self.label(l)).collect();
                            let value = self.expr(it.next().unwrap());
                            labels.push(path.clone());
                            entries.push(RecordEntry::Field { path, value });
                        }
                        Rule::shorthand => {
                            let n = self.name(q.into_inner().next().unwrap());
                            labels.push(vec![Label { sym: n.sym, span: n.span }]);
                            entries.push(RecordEntry::Shorthand(n));
                        }
                        _ => {}
                    }
                }
                self.check_record_paths(&labels);
                self.alloc_expr(ExprKind::Record { spread, entries }, span)
            }
            Rule::block => self.block(p),
            Rule::lambda => {
                let l = self.lambda(p);
                self.alloc_expr(ExprKind::Lambda(l), span)
            }
            Rule::if_expr => self.if_expr(p),
            Rule::match_expr => {
                let mut it = p.into_inner();
                let scrutinee = self.expr(it.next().unwrap());
                let arms = it
                    .map(|a| {
                        let aspan = self.span(&a);
                        let mut ai = a.into_inner();
                        let pat = self.pattern(ai.next().unwrap());
                        let mut guard = None;
                        let mut body = None;
                        for q in ai {
                            match q.as_rule() {
                                Rule::guard => guard = Some(self.expr(q.into_inner().next().unwrap())),
                                Rule::expr => body = Some(self.expr(q)),
                                _ => {}
                            }
                        }
                        MatchArm { pat, guard, body: body.unwrap(), span: aspan }
                    })
                    .collect();
                self.alloc_expr(ExprKind::Match { scrutinee, arms }, span)
            }
            Rule::while_expr => {
                let mut it = p.into_inner();
                let cond = self.expr(it.next().unwrap());
                let body = self.block(it.next().unwrap());
                self.alloc_expr(ExprKind::While { cond, body }, span)
            }
            Rule::for_expr => {
                let mut it = p.into_inner();
                let pat = self.pattern(it.next().unwrap());
                let iter = self.expr(it.next().unwrap());
                let body = self.block(it.next().unwrap());
                self.alloc_expr(ExprKind::For { pat, iter, body }, span)
            }
            Rule::loop_expr => {
                let body = self.block(p.into_inner().next().unwrap());
                self.alloc_expr(ExprKind::Loop { body }, span)
            }
            Rule::system_expr => {
                let mut entries = vec![];
                for q in p.into_inner() {
                    let mut it = q.into_inner();
                    let l = self.label(it.next().unwrap());
                    let e = self.expr(it.next().unwrap());
                    entries.push((l, e));
                }
                self.check_dup_labels(entries.iter().map(|e| e.0));
                self.alloc_expr(ExprKind::System(entries), span)
            }
            Rule::pipeline_expr => {
                let stages = p.into_inner().map(|e| self.expr(e)).collect();
                self.alloc_expr(ExprKind::Pipeline(stages), span)
            }
            Rule::map_lit => {
                let entries = p
                    .into_inner()
                    .map(|q| {
                        let mut it = q.into_inner();
                        let k = self.expr(it.next().unwrap());
                        let v = self.expr(it.next().unwrap());
                        (k, v)
                    })
                    .collect();
                self.alloc_expr(ExprKind::Map(entries), span)
            }
            Rule::set_lit => {
                let items = p.into_inner().map(|e| self.expr(e)).collect();
                self.alloc_expr(ExprKind::Set(items), span)
            }
            Rule::ref_expr => {
                let s = p.into_inner().next().unwrap();
                let sspan = self.span(&s);
                let sym = self.interner.intern(&s.as_str()[1..]);
                self.alloc_expr(ExprKind::Ref(Label { sym, span: sspan }), span)
            }
            Rule::unsafe_expr => {
                let b = self.block(p.into_inner().next().unwrap());
                self.alloc_expr(ExprKind::Unsafe(b), span)
            }
            Rule::return_expr => {
                let e = p.into_inner().next().map(|e| self.expr(e));
                self.alloc_expr(ExprKind::Return(e), span)
            }
            Rule::break_expr => {
                let e = p.into_inner().next().map(|e| self.expr(e));
                self.alloc_expr(ExprKind::Break(e), span)
            }
            Rule::continue_expr => self.alloc_expr(ExprKind::Continue, span),
            Rule::expr => self.expr(p),
            r => unreachable!("primary {r:?}"),
        }
    }

    /// Reject duplicate and overlapping record field paths (`a` and `a.b`).
    fn check_record_paths(&mut self, paths: &[Vec<Label>]) {
        for (i, a) in paths.iter().enumerate() {
            for b in &paths[..i] {
                let n = a.len().min(b.len());
                if a[..n].iter().zip(&b[..n]).all(|(x, y)| x.sym == y.sym) {
                    let text: Vec<&str> = a.iter().map(|l| self.interner.resolve(l.sym)).collect();
                    let msg = if a.len() == b.len() {
                        format!("duplicate field `{}`", text.join("."))
                    } else {
                        format!("overlapping field paths `{}`", text.join("."))
                    };
                    let span = a.last().unwrap().span;
                    let prev = b.last().unwrap().span;
                    self.diags.push(
                        Diagnostic::error(msg)
                            .with_code("E0103")
                            .with_label(span, "this field")
                            .with_label(prev, "conflicts with this field"),
                    );
                }
            }
        }
    }

    fn lambda(&mut self, p: Pair<'_, Rule>) -> Lambda {
        let mut l =
            Lambda { once: false, params: vec![], ret: None, effect: None, body: ExprId::from_raw(0.into()) };
        for q in p.into_inner() {
            match q.as_rule() {
                Rule::once_kw => l.once = true,
                Rule::params => l.params = self.params(q),
                Rule::ret_type => l.ret = Some(self.type_expr(q.into_inner().next().unwrap())),
                Rule::effect => l.effect = Some(self.effect(q)),
                Rule::block => l.body = self.block(q),
                _ => {}
            }
        }
        l
    }

    fn if_expr(&mut self, p: Pair<'_, Rule>) -> ExprId {
        let span = self.span(&p);
        let mut it = p.into_inner();
        let cond = self.expr(it.next().unwrap());
        let then = self.block(it.next().unwrap());
        let els = it.next().map(|q| match q.as_rule() {
            Rule::if_expr => self.if_expr(q),
            _ => self.block(q),
        });
        self.alloc_expr(ExprKind::If { cond, then, els }, span)
    }

    // ------------------------------------------------------------ patterns

    fn pattern(&mut self, p: Pair<'_, Rule>) -> PatId {
        let inner = if p.as_rule() == Rule::pattern { p.into_inner().next().unwrap() } else { p };
        let span = self.span(&inner);
        match inner.as_rule() {
            Rule::discard_pat => self.alloc_pat(PatKind::Discard, span),
            Rule::bind_pat => {
                let n = self.value_name(inner.into_inner().next().unwrap());
                self.alloc_pat(PatKind::Bind(n), span)
            }
            Rule::literal_pat => {
                let mut negative = false;
                let mut lit = Lit::Unit;
                for q in inner.into_inner() {
                    match q.as_rule() {
                        Rule::neg_op => negative = true,
                        _ => lit = self.lit(q),
                    }
                }
                self.alloc_pat(PatKind::Lit { lit, negative }, span)
            }
            Rule::unit_pat => self.alloc_pat(PatKind::Lit { lit: Lit::Unit, negative: false }, span),
            Rule::paren_pat => self.pattern(inner.into_inner().next().unwrap()),
            Rule::tuple_pat => {
                let items = inner.into_inner().map(|q| self.pattern(q)).collect();
                self.alloc_pat(PatKind::Tuple(items), span)
            }
            Rule::record_pat => {
                let mut fields = vec![];
                let mut open = false;
                for q in inner.into_inner() {
                    match q.as_rule() {
                        Rule::pat_field => {
                            let mut it = q.into_inner();
                            let l = self.label(it.next().unwrap());
                            let sub = it.next().map(|s| self.pattern(s));
                            fields.push((l, sub));
                        }
                        Rule::rest_pat => open = true,
                        _ => {}
                    }
                }
                self.check_dup_labels(fields.iter().map(|f| f.0));
                self.alloc_pat(PatKind::Record { fields, open }, span)
            }
            Rule::variant_pat => {
                let mut ty = None;
                let mut name = None;
                let mut args = None;
                for q in inner.into_inner() {
                    match q.as_rule() {
                        Rule::variant_type => {
                            let mut module = vec![];
                            let mut tname = None;
                            for r in q.into_inner() {
                                match r.as_rule() {
                                    Rule::value_ident => module.push(self.name(r)),
                                    _ => tname = Some(self.name(r)),
                                }
                            }
                            ty = Some((module, tname.unwrap()));
                        }
                        Rule::member => name = Some(self.name(q)),
                        Rule::variant_args => args = Some(q.into_inner().map(|a| self.pattern(a)).collect()),
                        _ => {}
                    }
                }
                self.alloc_pat(PatKind::Variant { ty, name: name.unwrap(), args }, span)
            }
            r => unreachable!("pattern {r:?}"),
        }
    }
}

fn split_suffix(text: &str) -> (&str, Option<&str>) {
    // Suffixes are an uppercase I/U/F followed by digits at the end. Hex
    // literals may contain uppercase hex digits, so only accept I/U there.
    let is_hex = text.starts_with("0x");
    let bytes = text.as_bytes();
    let mut i = bytes.len();
    while i > 0 && bytes[i - 1].is_ascii_digit() {
        i -= 1;
    }
    if i > 0 && i < bytes.len() && matches!(bytes[i - 1], b'I' | b'U' | b'F') {
        if is_hex && bytes[i - 1] == b'F' {
            return (text, None);
        }
        return (&text[..i - 1], Some(&text[i - 1..]));
    }
    (text, None)
}

/// The parse tree's leaves as `(rule, text)` pairs: a whitespace-insensitive
/// fingerprint of a source's structure (used to check that formatting
/// preserves meaning).
pub fn leaf_tokens(src: &str) -> Result<Vec<(String, String)>, String> {
    let text = layout::layout(src);
    let pairs = LangParser::parse(Rule::program, &text).map_err(|e| e.to_string())?;
    let mut out = vec![];
    fn walk(p: Pair<Rule>, out: &mut Vec<(String, String)>) {
        let inner: Vec<Pair<Rule>> = p.clone().into_inner().collect();
        if inner.is_empty() {
            out.push((
                format!("{:?}", p.as_rule()),
                p.as_str().split_whitespace().collect::<Vec<_>>().join(" "),
            ));
        } else {
            for c in inner {
                walk(c, out);
            }
        }
    }
    for p in pairs {
        walk(p, &mut out);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lang_base::SourceMap;

    fn parse_ok(src: &str) -> (Ast, Interner) {
        let mut sm = SourceMap::default();
        let f = sm.add("t", src);
        let mut int = Interner::new();
        let mut diags = vec![];
        let ast = parse(f, &sm.get(f).text, &mut int, &mut diags).unwrap_or_else(|e| {
            panic!("parse error at {}: {}", e.span.start, e.message);
        });
        assert!(diags.is_empty(), "{diags:?}");
        (ast, int)
    }

    fn body_expr(ast: &Ast) -> ExprId {
        let ItemKind::Global { value, .. } = ast.items[0].kind else { panic!() };
        value
    }

    #[test]
    fn precedence() {
        let (ast, _) = parse_ok("let x = 1 + 2 * 3");
        let root = body_expr(&ast);
        let ExprKind::Binary { op: BinOp::Add, rhs, .. } = ast.exprs[root].kind else {
            panic!("expected + at root");
        };
        assert!(matches!(ast.exprs[rhs].kind, ExprKind::Binary { op: BinOp::Mul, .. }));
    }

    #[test]
    fn pipe_is_weakest() {
        let (ast, _) = parse_ok("let x = a + 1 |> f(b) |> g()");
        let root = body_expr(&ast);
        let ExprKind::Binary { op: BinOp::Pipe, lhs, .. } = ast.exprs[root].kind else { panic!() };
        assert!(matches!(ast.exprs[lhs].kind, ExprKind::Binary { op: BinOp::Pipe, .. }));
    }

    #[test]
    fn record_vs_block() {
        let (ast, _) = parse_ok("let a = { value }\nlet b = { x + 1 }\nlet c = {}\nlet d = { ..p, age: 1 }");
        let kinds: Vec<_> = ast
            .items
            .iter()
            .map(|i| match i.kind {
                ItemKind::Global { value, .. } => match ast.exprs[value].kind {
                    ExprKind::Record { .. } => "record",
                    ExprKind::Block { .. } => "block",
                    _ => "other",
                },
                _ => "item",
            })
            .collect();
        assert_eq!(kinds, ["record", "block", "record", "record"]);
    }

    #[test]
    fn generic_call_and_comparison() {
        let (ast, _) = parse_ok("let a = f<Int>(1)\nlet b = x < y");
        let ItemKind::Global { value, .. } = ast.items[0].kind else { panic!() };
        assert!(matches!(&ast.exprs[value].kind, ExprKind::Call { type_args, .. } if type_args.len() == 1));
        let ItemKind::Global { value, .. } = ast.items[1].kind else { panic!() };
        assert!(matches!(ast.exprs[value].kind, ExprKind::Binary { op: BinOp::Lt, .. }));
    }

    #[test]
    fn literals() {
        let (ast, int) = parse_ok("let a = [0xFFU32, 12U8, 0.5F32, 1_000, 100ms, 'A', :app/db, $\"n={n}\"]");
        let ExprKind::List(items) = &ast.exprs[body_expr(&ast)].kind else { panic!() };
        let ExprKind::Lit(Lit::Int { value: 255, suffix: Some(s) }) = ast.exprs[items[0]].kind else {
            panic!()
        };
        assert_eq!(int.resolve(s), "U32");
        assert!(matches!(ast.exprs[items[2]].kind, ExprKind::Lit(Lit::Float { .. })));
        assert!(matches!(ast.exprs[items[3]].kind, ExprKind::Lit(Lit::Int { value: 1000, suffix: None })));
        assert!(matches!(
            ast.exprs[items[4]].kind,
            ExprKind::Lit(Lit::Duration { value: 100, unit: DurationUnit::Ms })
        ));
        assert!(matches!(ast.exprs[items[5]].kind, ExprKind::Lit(Lit::Char('A'))));
        let ExprKind::Lit(Lit::Symbol(s)) = ast.exprs[items[6]].kind else { panic!() };
        assert_eq!(int.resolve(s), "app/db");
        assert!(matches!(&ast.exprs[items[7]].kind, ExprKind::Interp(p) if p.len() == 2));
    }

    #[test]
    fn statements_and_semicolons() {
        let (ast, _) = parse_ok("fn f() {\n  let a = 1\n  a;\n}\nfn g() { 1 }");
        let ItemKind::Fn(f) = &ast.items[0].kind else { panic!() };
        let ExprKind::Block { stmts } = &ast.exprs[f.body.unwrap()].kind else { panic!() };
        assert!(matches!(stmts[1], Stmt::Expr { semi: true, .. }));
        let ItemKind::Fn(g) = &ast.items[1].kind else { panic!() };
        let ExprKind::Block { stmts } = &ast.exprs[g.body.unwrap()].kind else { panic!() };
        assert!(matches!(stmts[0], Stmt::Expr { semi: false, .. }));
    }

    #[test]
    fn case_rules() {
        let mut sm = SourceMap::default();
        let f = sm.add("t", "type person = { name: String }\nlet Count = 1\nenum Reply { ok(value: Int) }");
        let mut diags = vec![];
        parse(f, &sm.get(f).text, &mut Interner::new(), &mut diags).unwrap();
        let codes: Vec<_> = diags.iter().map(|d| d.code.unwrap()).collect();
        assert_eq!(codes, ["E0101", "E0102"]);
    }

    #[test]
    fn reports_error_span() {
        let mut sm = SourceMap::default();
        let f = sm.add("t", "let x = 1 + ");
        let err = parse(f, &sm.get(f).text, &mut Interner::new(), &mut vec![]).unwrap_err();
        assert_eq!(err.span.start, 12);
    }

    #[test]
    fn design_examples_parse() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/design");
        for file in ["normalize.lang", "batch_jobs.lang", "prefix_views.lang", "service/src/main.lang"] {
            let src = std::fs::read_to_string(format!("{root}/{file}")).unwrap();
            let mut sm = SourceMap::default();
            let f = sm.add(file, src);
            let mut diags = vec![];
            if let Err(e) = parse(f, &sm.get(f).text, &mut Interner::new(), &mut diags) {
                let text = &sm.get(f).text;
                let line = text[..e.span.start as usize].lines().count();
                panic!("{file}:{line}: {}", e.message);
            }
            assert!(diags.is_empty(), "{file}: {diags:?}");
        }
    }
}
