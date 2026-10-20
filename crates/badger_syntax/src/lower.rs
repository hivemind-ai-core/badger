//! Syntax tree ([`crate::cst`]) to the arena AST ([`crate::ast`]).
//!
//! Lowering checks what the grammar does not: case rules (E0101, E0102),
//! duplicate and overlapping fields (E0103), record spreads (E0104),
//! chained comparisons (E0003), escapes (E0004), and literal values
//! (E0005). It desugars `if let` and `while let` into `match`, applies
//! `x |> f(a)?` as `(x |> f(a))?`, splits module-level destructuring
//! bindings, and refuses expressions nested deeper than
//! [`MAX_EXPR_DEPTH`].

use std::collections::{HashMap, HashSet};

use badger_base::{Diagnostic, FileId, Interner, Span, Symbol};

use crate::ast::*;
use crate::cst;

/// Expressions nested deeper than this are refused: every pass after
/// parsing recurses once per level.
pub const MAX_EXPR_DEPTH: u32 = 2000;

pub struct Lower<'a, 'i> {
    file: FileId,
    src: &'a str,
    interner: &'i mut Interner,
    pub ast: Ast,
    diags: &'i mut Vec<Diagnostic>,
    /// Expressions written in parentheses.
    parenthesized: HashSet<ExprId>,
    /// Nesting depth of each expression, by index (children are built first).
    depth: Vec<u32>,
    /// The first expression nested deeper than [`MAX_EXPR_DEPTH`].
    pub too_deep: Option<Span>,
    /// Items a declaration generates besides itself (a provider's
    /// initializer when it has contracts).
    extra_items: Vec<Item>,
    /// Local functions in scope, innermost block last: the name written,
    /// and the module-level function it was lifted to.
    local_fns: Vec<HashMap<Symbol, Symbol>>,
    /// The functions being lowered (names for lifted local functions), and
    /// the lifted names already taken.
    fn_path: Vec<String>,
    lifted: HashSet<String>,
}

fn is_upper(s: &str) -> bool {
    s.starts_with(|c: char| c.is_ascii_uppercase())
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl<'a, 'i> Lower<'a, 'i> {
    pub fn new(
        file: FileId,
        src: &'a str,
        interner: &'i mut Interner,
        diags: &'i mut Vec<Diagnostic>,
    ) -> Self {
        Lower {
            file,
            src,
            interner,
            ast: Ast::default(),
            diags,
            parenthesized: HashSet::new(),
            depth: Vec::new(),
            too_deep: None,
            extra_items: vec![],
            local_fns: vec![],
            fn_path: vec![],
            lifted: HashSet::new(),
        }
    }

    fn span(&self, s: crate::token::Span) -> Span {
        Span::new(self.file, s.start, s.end)
    }

    fn name(&mut self, i: cst::Ident<'_>) -> Name {
        Name { sym: self.interner.intern(i.text), span: self.span(i.span) }
    }

    fn error(&mut self, code: &'static str, span: Span, msg: impl Into<String>, label: impl Into<String>) {
        self.diags.push(Diagnostic::error(msg).with_code(code).with_label(span, label));
    }

    /// A declaration name that must start with an uppercase letter.
    fn type_name(&mut self, i: cst::Ident<'_>) -> Name {
        let n = self.name(i);
        if !is_upper(i.text) {
            let msg = format!("type name `{}` must start with an uppercase letter", i.text);
            self.error("E0101", n.span, msg, "types, protocols and type parameters are uppercase");
        }
        n
    }

    /// A declaration name that must start lowercase (functions, values, variants, fields).
    fn value_name(&mut self, i: cst::Ident<'_>) -> Name {
        let n = self.name(i);
        if is_upper(i.text) {
            let msg = format!("value name `{}` must start with a lowercase letter", i.text);
            self.error("E0102", n.span, msg, "uppercase identifiers name types");
        }
        n
    }

    fn alloc_expr(&mut self, kind: ExprKind, span: Span) -> ExprId {
        let mut d = 0;
        let depth = &self.depth;
        expr_children(&kind, &mut |c| d = d.max(depth.get(index(c)).copied().unwrap_or(0)));
        let id = self.ast.exprs.alloc(Expr { kind, span });
        if self.depth.len() <= index(id) {
            self.depth.resize(index(id) + 1, 0);
        }
        self.depth[index(id)] = d + 1;
        if d + 1 > MAX_EXPR_DEPTH && self.too_deep.is_none() {
            self.too_deep = Some(span);
        }
        id
    }

    fn alloc_pat(&mut self, kind: PatKind, span: Span) -> PatId {
        self.ast.pats.alloc(Pat { kind, span })
    }

    fn alloc_ty(&mut self, kind: TypeKind, span: Span) -> TypeId {
        self.ast.types.alloc(TypeExpr { kind, span })
    }

    // ------------------------------------------------------------ items

    pub fn items(&mut self, items: Vec<cst::Item<'_>>) {
        for it in items {
            if let Some(item) = self.item(it) {
                self.push_item(item);
            }
            for extra in std::mem::take(&mut self.extra_items) {
                self.push_item(extra);
            }
        }
    }

    /// Is derive type `t` the prelude's `Default`?
    fn names_default(&self, t: TypeId) -> bool {
        self.names_derive(t, "Default")
    }

    /// Is derive type `t` the bare name `which`?
    fn names_derive(&self, t: TypeId, which: &str) -> bool {
        matches!(&self.ast.types[t].kind,
            TypeKind::Path { module, name, args } if module.is_empty() && args.is_empty() && self.interner.resolve(name.sym) == which)
    }

    /// Variant attributes: `@status(code)`, read by `derive(HttpError)`.
    fn check_variant_attributes(&mut self, variants: &[Variant], http: bool) {
        for v in variants {
            let mut status: Option<Span> = None;
            for a in &v.attributes {
                match self.interner.resolve(a.name) {
                    "status" => {
                        if let Some(first) = status {
                            self.diags.push(
                                Diagnostic::error("a variant has one `@status`")
                                    .with_code("E0193")
                                    .with_label(a.span, "a second one")
                                    .with_label(first, "the first"),
                            );
                            continue;
                        }
                        status = Some(a.span);
                        // an error's status: a client (4xx) or server (5xx) error
                        let ok = matches!(a.args.as_slice(), [AttrArg::Int(n)] if (400..=599).contains(n));
                        if !ok {
                            self.error(
                                "E0193",
                                a.span,
                                "`@status` takes one HTTP error status code (400 to 599)",
                                "e.g. `@status(404)`",
                            );
                        } else if !http {
                            self.diags.push(
                                Diagnostic::warning("`@status` has no effect without `derive(HttpError)`")
                                    .with_code("E0193")
                                    .with_label(a.span, "read by `derive(HttpError)` on the enum"),
                            );
                        }
                    }
                    other => {
                        let other = other.to_string();
                        self.diags.push(
                            Diagnostic::warning(format!("unknown variant attribute `@{other}`"))
                                .with_code("E0191")
                                .with_label(a.span, "a variant takes `@status(code)`"),
                        );
                    }
                }
            }
        }
    }

    /// `enum E { @status(404) missing(id: Int), other } derive(HttpError)` is
    /// `impl HttpStatus for E { fn http_status(self: &Self) -> Int { match self { .missing(_) => 404, _ => 500 } } }`.
    fn derive_http_error(&mut self, name: Name, generics: &[GenericParam], variants: &[Variant], at: Span) {
        let path = |this: &mut Self, text: &str, args: Vec<TypeId>| {
            let sym = this.interner.intern(text);
            this.alloc_ty(TypeKind::Path { module: vec![], name: Name { sym, span: at }, args }, at)
        };
        let gens: Vec<GenericParam> =
            generics.iter().map(|g| GenericParam { name: g.name, bounds: vec![] }).collect();
        let args: Vec<TypeId> = generics
            .iter()
            .map(|g| self.alloc_ty(TypeKind::Path { module: vec![], name: g.name, args: vec![] }, at))
            .collect();
        let ename = self.interner.resolve(name.sym).to_string();
        let self_ty = path(self, &ename, args);
        let protocol = path(self, "HttpStatus", vec![]);
        let int = path(self, "Int", vec![]);
        let selfref_inner = path(self, "Self", vec![]);
        let selfref = self.alloc_ty(TypeKind::Ref { mutable: false, inner: selfref_inner }, at);
        let self_sym = self.interner.intern("self");
        let self_pat = self.alloc_pat(PatKind::Bind(Name { sym: self_sym, span: at }), at);
        let mut arms = vec![];
        let mut all = true;
        for v in variants {
            let code =
                v.attributes.iter().find_map(|a| match (self.interner.resolve(a.name), a.args.as_slice()) {
                    ("status", [AttrArg::Int(n)]) => Some(*n),
                    _ => None,
                });
            let Some(code) = code else {
                all = false;
                continue;
            };
            let pargs = (!v.fields.is_empty())
                .then(|| v.fields.iter().map(|_| self.alloc_pat(PatKind::Discard, v.span)).collect());
            let pat = self.alloc_pat(PatKind::Variant { ty: None, name: v.name, args: pargs }, v.span);
            let body = self.alloc_expr(ExprKind::Lit(Lit::Int { value: code as u128, suffix: None }), v.span);
            arms.push(MatchArm { pat, guard: None, body, span: v.span });
        }
        if !all || arms.is_empty() {
            let pat = self.alloc_pat(PatKind::Discard, at);
            let body = self.alloc_expr(ExprKind::Lit(Lit::Int { value: 500, suffix: None }), at);
            arms.push(MatchArm { pat, guard: None, body, span: at });
        }
        let scrutinee = self.alloc_expr(ExprKind::Name(Name { sym: self_sym, span: at }), at);
        let body = self.alloc_expr(ExprKind::Match { scrutinee, arms }, at);
        let method = FnDecl {
            name: Name { sym: self.interner.intern("http_status"), span: at },
            is_pub: false,
            generics: vec![],
            params: vec![Param { pat: self_pat, ty: Some(selfref) }],
            ret: Some(int),
            effect: None,
            where_preds: vec![],
            body: Some(body),
            span: at,
            contracts: vec![],
            result_inferred: false,
        };
        self.extra_items.push(Item {
            kind: ItemKind::Impl(ImplDecl {
                generics: gens,
                protocol: Some(protocol),
                self_ty,
                where_preds: vec![],
                methods: vec![method],
                assoc_types: vec![],
            }),
            span: at,
            is_pub: false,
            attributes: vec![],
        });
    }

    /// `struct S<T> { a: A, .. } derive(Default)` is
    /// `impl<T> Default for S<T> where T: Default { fn default() -> S<T> { { a: default_of(), .. } } }`:
    /// each field's type must have a default.
    fn derive_default(&mut self, name: Name, generics: &[GenericParam], fields: &[StructField], at: Span) {
        let path = |this: &mut Self, text: &str, args: Vec<TypeId>| {
            let sym = this.interner.intern(text);
            this.alloc_ty(TypeKind::Path { module: vec![], name: Name { sym, span: at }, args }, at)
        };
        let gens: Vec<GenericParam> =
            generics.iter().map(|g| GenericParam { name: g.name, bounds: vec![] }).collect();
        let args: Vec<TypeId> = generics
            .iter()
            .map(|g| self.alloc_ty(TypeKind::Path { module: vec![], name: g.name, args: vec![] }, at))
            .collect();
        let sname = self.interner.resolve(name.sym).to_string();
        let self_ty = path(self, &sname, args.clone());
        let ret = path(self, &sname, args.clone());
        let protocol = path(self, "Default", vec![]);
        let where_preds = args
            .iter()
            .map(|a| {
                let bound = path(self, "Default", vec![]);
                WherePred { ty: *a, bounds: vec![bound] }
            })
            .collect();
        let entries = fields
            .iter()
            .map(|f| {
                // (a field without a default is reported at the field)
                let fs = f.name.span;
                let callee = self.alloc_expr(ExprKind::Std("default_of"), fs);
                let value = self.alloc_expr(ExprKind::Call { callee, type_args: vec![], args: vec![] }, fs);
                RecordEntry::Field { path: vec![Label { sym: f.name.sym, span: f.name.span }], value }
            })
            .collect();
        let body = self.alloc_expr(ExprKind::Record { spread: None, entries }, at);
        let method = FnDecl {
            name: Name { sym: self.interner.intern("default"), span: at },
            is_pub: false,
            generics: vec![],
            params: vec![],
            ret: Some(ret),
            effect: None,
            where_preds: vec![],
            body: Some(body),
            span: at,
            contracts: vec![],
            result_inferred: false,
        };
        self.extra_items.push(Item {
            kind: ItemKind::Impl(ImplDecl {
                generics: gens,
                protocol: Some(protocol),
                self_ty,
                where_preds,
                methods: vec![method],
                assoc_types: vec![],
            }),
            span: at,
            is_pub: false,
            attributes: vec![],
        });
    }

    fn item(&mut self, it: cst::Item<'_>) -> Option<Item> {
        let span = self.span(it.span);
        let is_pub = it.is_pub;
        // `@derive(A, B)` names protocols; they feed the same derive path as
        // `derive(A, B)`, so resolution and validation are shared.
        let attribute_derives: Vec<cst::T<'_>> = it
            .attributes
            .iter()
            .filter(|a| a.name.text == "derive")
            .flat_map(|a| a.args.iter())
            .filter_map(|arg| match arg {
                cst::AttrArg::Name(id) => {
                    Some((cst::Ty::Path { module: vec![], names: vec![*id], args: vec![] }, id.span))
                }
                _ => None,
            })
            .collect();
        let attributes = self.attributes(it.attributes);
        let kind = match it.kind {
            cst::ItemKind::Import { path, alias, items } => {
                let path = path.into_iter().map(|n| self.name(n)).collect();
                let alias = alias.map(|a| self.name(a));
                let items = items
                    .map(|xs| xs.into_iter().map(|(n, a)| (self.name(n), a.map(|a| self.name(a)))).collect());
                ItemKind::Import(Import { path, alias, items })
            }
            cst::ItemKind::TypeAlias { name, generics, ty } => {
                let name = self.type_name(name);
                let generics = self.generics(generics);
                let ty = self.ty(ty);
                ItemKind::TypeAlias { name, generics, ty }
            }
            cst::ItemKind::Struct { repr_c, name, generics, fields } => {
                let name = self.type_name(name);
                let generics = self.generics(generics);
                let fields: Vec<StructField> = fields
                    .into_iter()
                    .map(|(is_pub, n, t)| {
                        let name = self.value_name(n);
                        let ty = self.ty(t);
                        StructField { is_pub, name, ty }
                    })
                    .collect();
                let mut derives: Vec<TypeId> = attribute_derives.iter().map(|t| self.ty(t.clone())).collect();
                // `@derive(Default)` is an impl built from the fields' defaults
                if let Some(i) = derives.iter().position(|t| self.names_default(*t)) {
                    let at = self.ast.types[derives[i]].span;
                    derives.remove(i);
                    self.derive_default(name, &generics, &fields, at);
                }
                if let Some(i) = derives.iter().position(|t| self.names_derive(*t, "HttpError")) {
                    let at = self.ast.types[derives[i]].span;
                    derives.remove(i);
                    self.diags.push(
                        Diagnostic::error("`derive(HttpError)` is for an enum")
                            .with_code("E0193")
                            .with_label(at, "on a struct")
                            .with_note("each variant of an error enum gives its status: `enum AppError { @status(404) missing(id: Int), ... } derive(HttpError)`; for one status, `impl HttpStatus for T { fn http_status(self: &Self) -> Int { 404 } }`"),
                    );
                }
                ItemKind::Struct(StructDecl { name, generics, fields, derives, repr_c })
            }
            cst::ItemKind::Enum { name, generics, variants } => {
                let name = self.type_name(name);
                let generics = self.generics(generics);
                let variants: Vec<Variant> = variants
                    .into_iter()
                    .map(|v| {
                        let vspan = self.span(v.span);
                        let attributes = self.attributes(v.attributes);
                        let vname = self.value_name(v.name);
                        let fields = v
                            .fields
                            .into_iter()
                            .map(|(l, t)| {
                                let l = l.map(|l| self.value_name(l));
                                (l, self.ty(t))
                            })
                            .collect();
                        Variant { attributes, name: vname, fields, span: vspan }
                    })
                    .collect();
                let mut derives: Vec<TypeId> = attribute_derives.iter().map(|t| self.ty(t.clone())).collect();
                // `@derive(HttpError)`: an HttpStatus implementation from the
                // variants' `@status(code)`
                let http = derives.iter().position(|t| self.names_derive(*t, "HttpError"));
                if let Some(i) = http {
                    let at = self.ast.types[derives[i]].span;
                    derives.remove(i);
                    self.derive_http_error(name, &generics, &variants, at);
                }
                self.check_variant_attributes(&variants, http.is_some());
                ItemKind::Enum(EnumDecl { name, generics, variants, derives })
            }
            cst::ItemKind::Protocol { name, generics, supers, members } => {
                let name = self.type_name(name);
                let generics = self.generics(generics);
                let supers = supers.into_iter().map(|t| self.ty(t)).collect();
                let (methods, assoc_types) = self.members(members);
                ItemKind::Protocol(ProtocolDecl { name, generics, supers, methods, assoc_types })
            }
            cst::ItemKind::Impl { generics, first, for_ty, where_preds, members } => {
                let generics = self.generics(generics);
                let first = self.ty(first);
                let second = for_ty.map(|t| self.ty(t));
                let where_preds = self.where_preds(where_preds);
                let (methods, assoc_types) = self.members(members);
                let (protocol, self_ty) = match second {
                    Some(s) => (Some(first), s),
                    None => (None, first),
                };
                ItemKind::Impl(ImplDecl { generics, protocol, self_ty, where_preds, methods, assoc_types })
            }
            cst::ItemKind::Fn(d) => {
                let d = self.fn_decl(d, span);
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
            cst::ItemKind::ExternType { name, generics } => {
                let name = self.type_name(name);
                let generics = self.generics(generics);
                ItemKind::ExternType { name, generics }
            }
            cst::ItemKind::Extern { abi, decl } => {
                let abi = self.unescape(abi.0, self.span(abi.1));
                let decl = self.fn_decl(decl, span);
                ItemKind::Extern { abi, decl }
            }
            cst::ItemKind::Global { kind, pat, ty, value } => {
                let kind = let_kind(kind);
                let pat = self.pattern(pat);
                let ty = ty.map(|t| self.ty(t));
                let value = self.expr(value);
                ItemKind::Global { kind, pat, ty, value }
            }
            cst::ItemKind::Provider { contracts, name, alias, params, params_span, ret, effect, body } => {
                let contracts = contracts.into_iter().map(|c| self.contract(c)).collect();
                self.provider(span, is_pub, contracts, name, alias, params, params_span, ret, effect, body)
            }
            cst::ItemKind::Stage { contracts, name, alias, generics, params, ret, members, body_span } => {
                let contracts = contracts.into_iter().map(|c| self.contract(c)).collect();
                let mut d = self.stage(span, is_pub, name, alias, generics, params, ret, members, body_span);
                // a stage's contracts are its factory's
                d.contracts = contracts;
                ItemKind::Fn(d)
            }
            cst::ItemKind::Error => return None,
        };
        Some(Item { kind, span, is_pub, attributes })
    }

    fn attributes(&mut self, attrs: Vec<cst::Attribute<'_>>) -> Vec<Attribute> {
        attrs
            .into_iter()
            .map(|a| {
                let span = self.span(a.span);
                let name = self.interner.intern(a.name.text);
                let args = a.args.into_iter().map(|x| self.attr_arg(x)).collect();
                Attribute { name, args, span }
            })
            .collect()
    }

    fn attr_arg(&mut self, a: cst::AttrArg<'_>) -> AttrArg {
        match a {
            cst::AttrArg::Name(n) => AttrArg::Name(self.interner.intern(n.text)),
            cst::AttrArg::Symbol(s, _) => AttrArg::Symbol(self.interner.intern(s)),
            cst::AttrArg::Int(n, _) => AttrArg::Int(n.parse().unwrap_or(0)),
            cst::AttrArg::Str(s, sp) => AttrArg::Str(self.unescape(s, self.span(sp))),
            cst::AttrArg::KeyValue(k, v) => {
                AttrArg::KeyValue(self.interner.intern(k.text), Box::new(self.attr_arg(*v)))
            }
        }
    }

    fn members(&mut self, members: Vec<cst::Member<'_>>) -> (Vec<FnDecl>, Vec<(Name, Option<TypeId>)>) {
        let mut methods = vec![];
        let mut assoc = vec![];
        for m in members {
            match m {
                cst::Member::Fn(d) => {
                    let span = self.span(d.span);
                    methods.push(self.fn_decl(d, span))
                }
                cst::Member::AssocType(n, t) => {
                    let n = self.type_name(n);
                    let t = t.map(|t| self.ty(t));
                    assoc.push((n, t));
                }
            }
        }
        (methods, assoc)
    }

    fn fn_decl(&mut self, d: cst::FnDecl<'_>, span: Span) -> FnDecl {
        self.fn_path.push(d.name.text.to_string());
        let decl = self.fn_decl_inner(d, span);
        self.fn_path.pop();
        decl
    }

    fn fn_decl_inner(&mut self, d: cst::FnDecl<'_>, span: Span) -> FnDecl {
        let contracts = d.contracts.into_iter().map(|c| self.contract(c)).collect();
        let name = self.value_name(d.name);
        let generics = self.generics(d.generics);
        let params = self.params(d.params);
        let ret = d.ret.map(|t| self.ty(t));
        let effect = d.effect.map(|e| self.effect(e));
        let where_preds = self.where_preds(d.where_preds);
        let body = d.body.map(|b| self.block(b));
        FnDecl {
            name,
            is_pub: d.is_pub,
            generics,
            params,
            ret,
            effect,
            where_preds,
            body,
            span,
            contracts,
            result_inferred: false,
        }
    }

    fn contract(&mut self, c: cst::Contract<'_>) -> Contract {
        let span = self.span(c.span);
        let text = squash(&self.src[c.span.start..c.span.end]);
        let cond_text = squash(&self.src[c.cond.1.start..c.cond.1.end]);
        let cond = self.expr(c.cond);
        let message = c.message.map(|m| self.string(&m, span));
        Contract { ensures: c.ensures, cond, message, span, text, cond_text }
    }

    /// The descriptor's `name`: an explicit `as #sym`, else `#module/NAME`.
    fn decl_name(&mut self, name: Name, alias: Option<(&str, crate::token::Span)>) -> ExprId {
        match alias {
            Some((text, s)) => {
                let sym = self.interner.intern(text);
                let span = self.span(s);
                self.alloc_expr(ExprKind::Lit(Lit::Symbol(sym)), span)
            }
            None => self.alloc_expr(ExprKind::DeclName(name), name.span),
        }
    }

    /// `provider NAME(scope, config) -> R { body }` is
    /// `let NAME = system.provider({ name: #module/NAME, init: fn(scope, config) -> R { body } })`;
    /// a public one has the type `system.Provider<C, O, E>` of its signature.
    #[allow(clippy::too_many_arguments)]
    fn provider(
        &mut self,
        span: Span,
        is_pub: bool,
        contracts: Vec<Contract>,
        name: cst::Ident<'_>,
        alias: Option<(&str, crate::token::Span)>,
        params: Vec<cst::Param<'_>>,
        params_span: crate::token::Span,
        ret: Option<cst::T<'_>>,
        effect: Option<cst::Effect<'_>>,
        body: cst::Block<'_>,
    ) -> ItemKind {
        let n = self.value_name(name);
        let mut params = self.params(params);
        if params.len() == 1 {
            // `(config)`: the scope is unused
            let pspan = self.span(params_span);
            let scope = self.alloc_pat(PatKind::Discard, pspan);
            params.insert(0, Param { pat: scope, ty: None });
        } else if params.len() != 2 {
            self.error(
                "E0705",
                self.span(params_span),
                "a provider takes `(config)` or `(scope, config)`",
                "the initializer's parameters",
            );
        }
        let ret = ret.map(|t| self.ty(t));
        // the public type: `system.Provider<Config, Output, Error>`
        let ty =
            match (is_pub, params.get(1).and_then(|p| p.ty), ret.map(|r| self.result_args(r))) {
                (true, Some(config), Some(Some((out, error)))) => Some(self.alloc_ty(
                    TypeKind::Std { path: "system.Provider", args: vec![config, out, error] },
                    n.span,
                )),
                _ => None,
            };
        let effect = effect.map(|e| self.effect(e));
        let body = self.block(body);
        let init = if contracts.is_empty() {
            self.alloc_expr(ExprKind::Lambda(Lambda { once: false, params, ret, effect, body }), span)
        } else {
            // contracts belong to functions: the initializer becomes a
            // private module function carrying them
            let text = format!("{}.init", self.interner.resolve(n.sym));
            let fname = Name { sym: self.interner.intern(&text), span: n.span };
            self.extra_items.push(Item {
                kind: ItemKind::Fn(FnDecl {
                    name: fname,
                    is_pub: false,
                    generics: vec![],
                    params,
                    ret,
                    effect,
                    where_preds: vec![],
                    body: Some(body),
                    span,
                    contracts,
                    result_inferred: false,
                }),
                span,
                is_pub: false,
                attributes: vec![],
            });
            self.alloc_expr(ExprKind::Name(fname), n.span)
        };
        let name_e = self.decl_name(n, alias);
        let record = self.descriptor(&[("name", name_e), ("init", init)], span);
        // the generated reference is zero-width at the declaration's start,
        // so tools find the declaration's own names, not `system.provider`
        let at = Span { end: span.start, ..span };
        let callee = self.alloc_expr(ExprKind::Std("system.provider"), at);
        let value = self.alloc_expr(ExprKind::Call { callee, type_args: vec![], args: vec![record] }, n.span);
        let pat = self.alloc_pat(PatKind::Bind(n), n.span);
        ItemKind::Global { kind: LetKind::Let, pat, ty, value }
    }

    /// `stage NAME(params) -> Result<_, E> { enter(ctx) { .. } leave(..) .. error(..) .. }`
    /// is `fn NAME(params) { pipeline.stage({ name: #module/NAME, enter: fn(ctx) -> Result<_, E> { .. }, .. }) }`:
    /// the shared annotation types `enter` and `leave` results and the
    /// `error` callback's error.
    #[allow(clippy::too_many_arguments)]
    fn stage(
        &mut self,
        span: Span,
        is_pub: bool,
        name: cst::Ident<'_>,
        alias: Option<(&str, crate::token::Span)>,
        generics: Vec<cst::Generic<'_>>,
        params: Vec<cst::Param<'_>>,
        ret: Option<cst::T<'_>>,
        members: Vec<cst::StageMember<'_>>,
        body_span: crate::token::Span,
    ) -> FnDecl {
        let n = self.value_name(name);
        let generics = self.generics(generics);
        let params = self.params(params);
        let ret = ret.map(|t| self.ty(t));
        let error_ty = match ret.map(|r| (r, self.result_args(r))) {
            Some((_, Some((_, e)))) => Some(e),
            Some((r, None)) => {
                let rs = self.ast.types[r].span;
                self.error(
                    "E0702",
                    rs,
                    "a stage's result annotation is `-> Result<_, Error>`",
                    "it names the error type of the stage's callbacks",
                );
                None
            }
            None => None,
        };
        let mut fields: Vec<(&'static str, ExprId)> = vec![];
        let name_e = self.decl_name(n, alias);
        fields.push(("name", name_e));
        let mut seen: Vec<(&str, Span)> = vec![];
        for m in members {
            let kspan = self.span(m.kind.span);
            if let Some((_, first)) = seen.iter().find(|(k, _)| *k == m.kind.text) {
                self.diags.push(
                    Diagnostic::error(format!("a stage has one `{}` callback", m.kind.text))
                        .with_code("E0702")
                        .with_label(kspan, "a second one")
                        .with_label(*first, "the first"),
                );
                continue;
            }
            seen.push((m.kind.text, kspan));
            let mut params = self.params(m.params);
            let kind: &'static str = match m.kind.text {
                "enter" => "enter",
                "leave" => "leave",
                "around" => "around",
                _ => "error",
            };
            let own = m.ret.map(|t| self.ty(t));
            let ret = match kind {
                _ if own.is_some() => own,
                "error" => {
                    // `error(ctx, error)`: the error parameter has the stage's error type
                    if let (Some(e), Some(p)) = (error_ty, params.get_mut(1))
                        && p.ty.is_none()
                    {
                        p.ty = Some(e);
                    }
                    None
                }
                _ => ret,
            };
            let mspan = Span::new(self.file, m.kind.span.start, m.body.1.end);
            let body = self.expr(m.body);
            let f = self
                .alloc_expr(ExprKind::Lambda(Lambda { once: false, params, ret, effect: None, body }), mspan);
            fields.push((kind, f));
        }
        if fields.len() == 1 {
            self.error(
                "E0702",
                self.span(body_span),
                "a stage needs a callback",
                "add `enter(ctx) { ... }`, `leave(ctx, reply) { ... }`, or `error(ctx, error) { ... }`",
            );
        }
        let record = self.descriptor(&fields, span);
        // generated references are zero-width (see `provider`)
        let at = Span { end: span.start, ..span };
        let callee = self.alloc_expr(ExprKind::Std("pipeline.stage"), at);
        let call = self.alloc_expr(ExprKind::Call { callee, type_args: vec![], args: vec![record] }, at);
        let body =
            self.alloc_expr(ExprKind::Block { stmts: vec![Stmt::Expr { expr: call, semi: false }] }, span);
        FnDecl {
            name: n,
            is_pub,
            generics,
            params,
            ret: None,
            effect: None,
            where_preds: vec![],
            body: Some(body),
            span,
            contracts: vec![],
            result_inferred: true,
        }
    }

    /// A descriptor record literal from labelled values.
    fn descriptor(&mut self, fields: &[(&str, ExprId)], span: Span) -> ExprId {
        let entries = fields
            .iter()
            .map(|(label, value)| RecordEntry::Field {
                path: vec![Label { sym: self.interner.intern(label), span: self.ast.exprs[*value].span }],
                value: *value,
            })
            .collect();
        self.alloc_expr(ExprKind::Record { spread: None, entries }, span)
    }

    /// `Result<O, E>`'s type arguments.
    fn result_args(&self, t: TypeId) -> Option<(TypeId, TypeId)> {
        match &self.ast.types[t].kind {
            TypeKind::Path { module, name, args }
                if module.is_empty() && self.interner.resolve(name.sym) == "Result" && args.len() == 2 =>
            {
                Some((args[0], args[1]))
            }
            _ => None,
        }
    }

    fn params(&mut self, ps: Vec<cst::Param<'_>>) -> Vec<Param> {
        ps.into_iter()
            .map(|p| {
                let pat = self.pattern(p.pat);
                let ty = p.ty.map(|t| self.ty(t));
                Param { pat, ty }
            })
            .collect()
    }

    fn generics(&mut self, gs: Vec<cst::Generic<'_>>) -> Vec<GenericParam> {
        gs.into_iter()
            .map(|g| {
                let name = self.type_name(g.name);
                let bounds = g.bounds.into_iter().map(|b| self.ty(b)).collect();
                GenericParam { name, bounds }
            })
            .collect()
    }

    fn where_preds(&mut self, ws: Vec<cst::WherePred<'_>>) -> Vec<WherePred> {
        ws.into_iter()
            .map(|w| {
                let ty = self.ty(w.ty);
                let bounds = w.bounds.into_iter().map(|b| self.ty(b)).collect();
                WherePred { ty, bounds }
            })
            .collect()
    }

    fn effect(&mut self, e: cst::Effect<'_>) -> Effect {
        match e {
            cst::Effect::Var(n) => Effect::Var(self.name(n)),
            cst::Effect::Set(ts, s) => {
                let span = self.span(s);
                Effect::Set(ts.into_iter().map(|t| self.ty(t)).collect(), span)
            }
        }
    }

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
            attributes: vec![],
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
                attributes: vec![],
            });
        }
    }

    // ------------------------------------------------------------ types

    fn ty(&mut self, (t, s): cst::T<'_>) -> TypeId {
        let span = self.span(s);
        match t {
            cst::Ty::Hole => self.alloc_ty(TypeKind::Hole, span),
            cst::Ty::Path { module, names, args } => {
                let (module, name, args) = self.path_parts(module, names, args);
                self.alloc_ty(TypeKind::Path { module, name, args }, span)
            }
            cst::Ty::Unit => self.alloc_ty(TypeKind::Unit, span),
            cst::Ty::Tuple(ts) => {
                let items = ts.into_iter().map(|t| self.ty(t)).collect();
                self.alloc_ty(TypeKind::Tuple(items), span)
            }
            cst::Ty::Fn { once, params, ret, effect } => {
                let params = params.into_iter().map(|t| self.ty(t)).collect();
                let ret = self.ty(*ret);
                let effect = effect.map(|e| self.effect(e));
                self.alloc_ty(TypeKind::Fn { once, params, ret, effect }, span)
            }
            cst::Ty::Forall { generics, body } => {
                let generics = self.generics(generics);
                let body = self.ty(*body);
                self.alloc_ty(TypeKind::Forall { generics, body }, span)
            }
            cst::Ty::Ref { mutable, inner } => {
                let inner = self.ty(*inner);
                self.alloc_ty(TypeKind::Ref { mutable, inner }, span)
            }
            cst::Ty::Dyn(p) => {
                let t = self.ty(*p);
                let TypeKind::Path { module, name, args } = self.ast.types[t].kind.clone() else {
                    unreachable!()
                };
                self.alloc_ty(TypeKind::Dyn { module, name, args }, span)
            }
            cst::Ty::Record { fields, rest } => {
                let fields: Vec<(Label, TypeId)> = fields
                    .into_iter()
                    .map(|(l, t)| {
                        let l = self.label(l);
                        (l, self.ty(t))
                    })
                    .collect();
                let rest = rest.map(|r| self.name(r));
                self.check_dup_labels(fields.iter().map(|f| f.0));
                self.alloc_ty(TypeKind::Record { fields, rest }, span)
            }
            cst::Ty::Effect(e) => {
                let e = self.effect(e);
                self.alloc_ty(TypeKind::Effect(e), span)
            }
            cst::Ty::Error => self.alloc_ty(TypeKind::Error, span),
        }
    }

    /// `a.b.Name<args>`, `T.Item`: the first type name qualifies an
    /// associated type.
    fn path_parts(
        &mut self,
        module: Vec<cst::Ident<'_>>,
        names: Vec<cst::Ident<'_>>,
        args: Vec<cst::T<'_>>,
    ) -> (Vec<Name>, Name, Vec<TypeId>) {
        let mut module: Vec<Name> = module.into_iter().map(|n| self.name(n)).collect();
        let mut names: Vec<Name> = names.into_iter().map(|n| self.name(n)).collect();
        let name = names.pop().unwrap();
        module.extend(names);
        let args = args.into_iter().map(|t| self.ty(t)).collect();
        (module, name, args)
    }

    fn label(&mut self, l: cst::Label<'_>) -> Label {
        match l {
            cst::Label::Symbol(text, s) => Label { sym: self.interner.intern(text), span: self.span(s) },
            cst::Label::Name(i) => {
                let span = self.span(i.span);
                if is_upper(i.text) {
                    let msg = format!("record label `{}` must start with a lowercase letter", i.text);
                    self.error("E0102", span, msg, "ordinary labels are lowercase identifiers");
                }
                Label { sym: self.interner.intern(i.text), span }
            }
        }
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

    // ------------------------------------------------------------ statements

    fn block(&mut self, b: cst::Block<'_>) -> ExprId {
        let span = self.span(b.span);
        // local functions are visible in the whole block (they may call one
        // another): each is lifted to a private module function
        let mut frame = HashMap::new();
        for it in &b.items {
            if let cst::BlockItem::Stmt(cst::Stmt::Fn(d)) = it {
                let base = format!("{}.{}", self.fn_path.join("."), d.name.text);
                let mut lifted = base.clone();
                let mut k = 2;
                while !self.lifted.insert(lifted.clone()) {
                    lifted = format!("{base}#{k}");
                    k += 1;
                }
                let written = self.interner.intern(d.name.text);
                let sym = self.interner.intern(&lifted);
                frame.insert(written, sym);
            }
        }
        let scoped = !frame.is_empty();
        let lifted: Vec<Symbol> = frame.values().copied().collect();
        if scoped {
            self.local_fns.push(frame);
        }
        let block = self.block_items(b, span);
        if scoped {
            self.local_fns.pop();
            self.ast.local_fn_blocks.insert(block, lifted);
        }
        block
    }

    /// The function a name written in a body refers to, when it names a
    /// local function in scope.
    fn local_fn(&self, sym: Symbol) -> Option<Symbol> {
        self.local_fns.iter().rev().find_map(|f| f.get(&sym).copied())
    }

    /// A local function: lifted to a private module function (it does not
    /// capture the enclosing function's locals; a closure does).
    fn local_fn_decl(&mut self, d: cst::FnDecl<'_>) -> Stmt {
        let span = self.span(d.span);
        let written = self.interner.intern(d.name.text);
        let lifted = self.local_fn(written).unwrap_or(written);
        if d.is_pub {
            self.error(
                "E0002",
                span,
                "a local function cannot be `pub`",
                "declare it at module level to export it",
            );
        }
        let mut decl = self.fn_decl(d, span);
        if decl.body.is_none() {
            self.error(
                "E0002",
                decl.name.span,
                "function declaration without a body",
                "add a body `{ ... }`",
            );
        }
        decl.name = Name { sym: lifted, span: decl.name.span };
        decl.is_pub = false;
        self.ast.local_fns.push(lifted);
        self.extra_items.push(Item { kind: ItemKind::Fn(decl), span, is_pub: false, attributes: vec![] });
        let unit = self.alloc_expr(ExprKind::Lit(Lit::Unit), span);
        Stmt::Expr { expr: unit, semi: true }
    }

    fn block_items(&mut self, b: cst::Block<'_>, span: Span) -> ExprId {
        let mut stmts: Vec<Stmt> = vec![];
        for it in b.items {
            match it {
                cst::BlockItem::Semi => {
                    if let Some(Stmt::Expr { semi, .. }) = stmts.last_mut() {
                        *semi = true;
                    }
                }
                cst::BlockItem::Stmt(s) => {
                    let s = self.stmt(s);
                    stmts.push(s);
                }
            }
        }
        self.alloc_expr(ExprKind::Block { stmts }, span)
    }

    /// A pattern that always matches (a name, `_`, a tuple or record of
    /// them) where an `else` is written for the case it does not.
    fn warn_irrefutable(&mut self, pat: PatId, form: &str, why: &str) {
        fn always(ast: &Ast, p: PatId) -> bool {
            match &ast.pats[p].kind {
                PatKind::Bind(_) | PatKind::Discard => true,
                PatKind::Tuple(ps) => ps.iter().all(|q| always(ast, *q)),
                PatKind::Record { fields, .. } => {
                    fields.iter().all(|(_, q)| q.is_none_or(|q| always(ast, q)))
                }
                _ => false,
            }
        }
        if always(&self.ast, pat) {
            let span = self.ast.pats[pat].span;
            self.diags.push(
                Diagnostic::warning(format!("this {form} pattern always matches"))
                    .with_code("E0346")
                    .with_label(span, why.to_string())
                    .with_note("bind it with a plain `let`, or match a variant (`.some(x)`)"),
            );
        }
    }

    /// A `let ... else` block ending in `err(...)` returns it: `let .some(x)
    /// = e else { err(AppError.missing) }` is `... else { return err(...) }`.
    fn return_tail_err(&mut self, block: ExprId) {
        let ExprKind::Block { stmts } = &self.ast.exprs[block].kind else { return };
        let Some(Stmt::Expr { expr, semi: false }) = stmts.last() else { return };
        let expr = *expr;
        let is_err = match &self.ast.exprs[expr].kind {
            ExprKind::Call { callee, .. } => {
                matches!(&self.ast.exprs[*callee].kind, ExprKind::Name(n) if self.interner.resolve(n.sym) == "err")
            }
            _ => false,
        };
        if !is_err {
            return;
        }
        let span = self.ast.exprs[expr].span;
        let ret = self.alloc_expr(ExprKind::Return(Some(expr)), span);
        if let ExprKind::Block { stmts } = &mut self.ast.exprs[block].kind
            && let Some(Stmt::Expr { expr, .. }) = stmts.last_mut()
        {
            *expr = ret;
        }
    }

    fn stmt(&mut self, s: cst::Stmt<'_>) -> Stmt {
        match s {
            cst::Stmt::Fn(d) => self.local_fn_decl(*d),
            cst::Stmt::Let { kind, pat, ty, value, els: None, span } => {
                let span = self.span(span);
                let pat = self.pattern(pat);
                let ty = ty.map(|t| self.ty(t));
                let value = self.expr(value);
                Stmt::Let { kind: let_kind(kind), pat, ty, value, span }
            }
            // `let p = e else { b }` is `let (x, ..) = match e { p => (x, ..), _ => diverge(b) }`
            // over the names `p` binds; `b` must not finish (its type is `Never`)
            cst::Stmt::Let { kind, pat, ty, value, els: Some(els), span } => {
                let span = self.span(span);
                let pat = self.pattern(pat);
                self.warn_irrefutable(pat, "`let ... else`", "the `else` block never runs");
                let ty = ty.map(|t| self.ty(t));
                let value = self.expr(value);
                let mut names = vec![];
                bound_names(&self.ast, pat, &mut names);
                let values: Vec<ExprId> =
                    names.iter().map(|n| self.alloc_expr(ExprKind::Name(*n), n.span)).collect();
                let binds: Vec<PatId> =
                    names.iter().map(|n| self.alloc_pat(PatKind::Bind(*n), n.span)).collect();
                let (matched, target) = match values.len() {
                    1 => (values[0], binds[0]),
                    _ => (
                        self.alloc_expr(ExprKind::Tuple(values), span),
                        self.alloc_pat(PatKind::Tuple(binds), span),
                    ),
                };
                let otherwise = self.block(*els);
                self.return_tail_err(otherwise);
                // generated references are zero-width (see `provider`): hover
                // and references over the `else` block are its own
                let diverge = self.alloc_expr(ExprKind::Std("diverge"), Span { end: span.start, ..span });
                let otherwise = self.alloc_expr(
                    ExprKind::Call { callee: diverge, args: vec![otherwise], type_args: vec![] },
                    span,
                );
                if let Some(t) = ty {
                    self.error(
                        "E0002",
                        self.ast.types[t].span,
                        "`let ... else` takes no type annotation",
                        "annotate the value instead (`let v: T = e`, then `let p = v else { ... }`)",
                    );
                }
                let arms = self.let_arms(pat, matched, otherwise, span);
                let whole = self.alloc_expr(ExprKind::Match { scrutinee: value, arms }, span);
                Stmt::Let { kind: let_kind(kind), pat: target, ty: None, value: whole, span }
            }
            cst::Stmt::Use { pats, call, span } => {
                let span = self.span(span);
                let pats = pats.into_iter().map(|p| self.pattern(p)).collect();
                let call = self.expr(call);
                Stmt::Use { pats, call, span }
            }
            cst::Stmt::Assign { base, path, op, value, span: cspan } => {
                // `place op= v` is `place = place op v`; an index is read
                // again, so it must be a name or a literal
                let value = match op {
                    None => value,
                    Some(op) => {
                        let mut read: cst::E<'_> = (cst::Expr::Name(base), base.span);
                        for p in &path {
                            read = match p {
                                cst::PlaceElem::Field(m) => {
                                    (cst::Expr::Field { base: Box::new(read), name: *m }, cspan)
                                }
                                cst::PlaceElem::Index(e) => {
                                    if !matches!(e.0, cst::Expr::Name(_) | cst::Expr::Lit(_)) {
                                        let at = self.span(e.1);
                                        self.error(
                                            "E0002",
                                            at,
                                            "a compound assignment's index is evaluated twice",
                                            "use a name or a literal (bind the index first)",
                                        );
                                    }
                                    (
                                        cst::Expr::Index { base: Box::new(read), index: Box::new(e.clone()) },
                                        cspan,
                                    )
                                }
                            };
                        }
                        (cst::Expr::Binary { op, lhs: Box::new(read), rhs: Box::new(value) }, cspan)
                    }
                };
                let span = self.span(cspan);
                let base = self.name(base);
                let path = path
                    .into_iter()
                    .map(|p| match p {
                        cst::PlaceElem::Field(m) => PlaceElem::Field(Label {
                            sym: self.interner.intern(m.text),
                            span: self.span(m.span),
                        }),
                        cst::PlaceElem::Index(e) => PlaceElem::Index(self.expr(e)),
                    })
                    .collect();
                let value = self.expr(value);
                Stmt::Assign { base, path, value, span }
            }
            cst::Stmt::Expr(e) => Stmt::Expr { expr: self.expr(e), semi: false },
        }
    }

    // ------------------------------------------------------------ expressions

    fn expr(&mut self, (e, s): cst::E<'_>) -> ExprId {
        let span = self.span(s);
        match e {
            cst::Expr::Lit(l) => {
                let l = self.lit(&l, span);
                self.alloc_expr(ExprKind::Lit(l), span)
            }
            cst::Expr::Interp(raw, parts) => {
                let parts = parts
                    .into_iter()
                    .map(|p| match p {
                        cst::InterpPart::Text(t, ts) if raw => {
                            let _ = ts;
                            InterpPart::Text(t.replace("{{", "{").replace("}}", "}"))
                        }
                        cst::InterpPart::Text(t, ts) => {
                            let ts = self.span(ts);
                            InterpPart::Text(self.interp_text(t, ts))
                        }
                        cst::InterpPart::Hole(e) => InterpPart::Expr(self.expr(e)),
                    })
                    .collect();
                self.alloc_expr(ExprKind::Interp(parts), span)
            }
            cst::Expr::Name(n) => {
                let mut n = self.name(n);
                let written = n.sym;
                if let Some(lifted) = self.local_fn(n.sym) {
                    n.sym = lifted;
                }
                let id = self.alloc_expr(ExprKind::Name(n), span);
                if n.sym != written {
                    self.ast.local_fn_refs.insert(id, written);
                }
                id
            }
            cst::Expr::TypeName(n) => {
                let n = self.name(n);
                self.alloc_expr(ExprKind::TypeName(n), span)
            }
            cst::Expr::Unit => self.alloc_expr(ExprKind::Lit(Lit::Unit), span),
            cst::Expr::Paren(inner) => {
                let id = self.expr(*inner);
                self.parenthesized.insert(id);
                id
            }
            cst::Expr::Tuple(xs) => {
                let items = xs.into_iter().map(|x| self.expr(x)).collect();
                self.alloc_expr(ExprKind::Tuple(items), span)
            }
            cst::Expr::List(xs) => {
                let items = xs.into_iter().map(|x| self.expr(x)).collect();
                self.alloc_expr(ExprKind::List(items), span)
            }
            cst::Expr::Record(entries) => {
                let mut spread = None;
                let mut out = vec![];
                let mut labels = vec![];
                for en in entries {
                    match en {
                        cst::RecordEntry::Spread(e, es) => {
                            let es = self.span(es);
                            let e = self.expr(e);
                            if spread.is_some() || !out.is_empty() {
                                self.error(
                                    "E0104",
                                    es,
                                    "a record spread must come first",
                                    "only one leading spread is allowed",
                                );
                            }
                            spread = Some(e);
                        }
                        cst::RecordEntry::Field(path, value) => {
                            let path: Vec<Label> = path.into_iter().map(|l| self.label(l)).collect();
                            let value = self.expr(value);
                            labels.push(path.clone());
                            out.push(RecordEntry::Field { path, value });
                        }
                        cst::RecordEntry::Shorthand(n) => {
                            let n = self.name(n);
                            labels.push(vec![Label { sym: n.sym, span: n.span }]);
                            out.push(RecordEntry::Shorthand(n));
                        }
                    }
                }
                self.check_record_paths(&labels);
                self.alloc_expr(ExprKind::Record { spread, entries: out }, span)
            }
            cst::Expr::Map(kvs) => {
                let entries = kvs
                    .into_iter()
                    .map(|(k, v)| {
                        let k = self.expr(k);
                        (k, self.expr(v))
                    })
                    .collect();
                self.alloc_expr(ExprKind::Map(entries), span)
            }
            cst::Expr::Set(xs) => {
                let items = xs.into_iter().map(|x| self.expr(x)).collect();
                self.alloc_expr(ExprKind::Set(items), span)
            }
            cst::Expr::Block(b) => self.block(b),
            cst::Expr::Lambda(l) => {
                let params = self.params(l.params);
                let ret = l.ret.map(|t| self.ty(t));
                let effect = l.effect.map(|e| self.effect(e));
                let body = self.expr(*l.body);
                self.alloc_expr(ExprKind::Lambda(Lambda { once: l.once, params, ret, effect, body }), span)
            }
            cst::Expr::If { cond, then, els } => match cond {
                // `if let p = e { a } else { b }` is `match e { p => a, _ => b }`
                cst::Cond::Let(pat, scrutinee) => {
                    let pat = self.pattern(pat);
                    if els.is_some() {
                        self.warn_irrefutable(pat, "`if let`", "the `else` branch never runs");
                    }
                    let scrutinee = self.expr(*scrutinee);
                    let then = self.block(then);
                    let otherwise = match els {
                        Some(e) => self.expr(*e),
                        None => self.alloc_expr(ExprKind::Lit(Lit::Unit), span),
                    };
                    let arms = self.let_arms(pat, then, otherwise, span);
                    self.alloc_expr(ExprKind::Match { scrutinee, arms }, span)
                }
                cst::Cond::Expr(c) => {
                    let cond = self.expr(*c);
                    let then = self.block(then);
                    let els = els.map(|e| self.expr(*e));
                    self.alloc_expr(ExprKind::If { cond, then, els }, span)
                }
            },
            cst::Expr::Match { scrutinee, arms } => {
                let scrutinee = self.expr(*scrutinee);
                let mut out = vec![];
                for a in arms {
                    let aspan = self.span(a.span);
                    if a.alts.is_empty() {
                        let pat = self.pattern(a.pat);
                        let guard = a.guard.map(|g| self.expr(g));
                        let body = self.expr(a.body);
                        out.push(MatchArm { pat, guard, body, span: aspan });
                        continue;
                    }
                    // `p | q => body` is one arm per alternative, each with
                    // its own copy of the guard and body; every alternative
                    // binds the same names
                    let mut first: Option<Vec<badger_base::Symbol>> = None;
                    for alt in std::iter::once(a.pat).chain(a.alts) {
                        let pat = self.pattern(alt);
                        let mut names = vec![];
                        bound_names(&self.ast, pat, &mut names);
                        let mut syms: Vec<badger_base::Symbol> = names.iter().map(|n| n.sym).collect();
                        syms.sort();
                        match &first {
                            None => first = Some(syms),
                            Some(f) if *f != syms => {
                                let at = self.ast.pats[pat].span;
                                self.error(
                                    "E0002",
                                    at,
                                    "the alternatives of a pattern bind different names",
                                    "every alternative must bind the same names",
                                );
                            }
                            _ => {}
                        }
                        let guard = a.guard.clone().map(|g| self.expr(g));
                        let body = self.expr(a.body.clone());
                        out.push(MatchArm { pat, guard, body, span: aspan });
                    }
                }
                self.alloc_expr(ExprKind::Match { scrutinee, arms: out }, span)
            }
            cst::Expr::While { cond, body } => match cond {
                // `while let p = e { body }` is
                // `loop { match e { p => body, _ => break } }`
                cst::Cond::Let(pat, scrutinee) => {
                    let pat = self.pattern(pat);
                    let scrutinee = self.expr(*scrutinee);
                    let body = self.block(body);
                    let stop = self.alloc_expr(ExprKind::Break(None), span);
                    let arms = self.let_arms(pat, body, stop, span);
                    let m = self.alloc_expr(ExprKind::Match { scrutinee, arms }, span);
                    let block = self.alloc_expr(
                        ExprKind::Block { stmts: vec![Stmt::Expr { expr: m, semi: false }] },
                        span,
                    );
                    self.alloc_expr(ExprKind::Loop { body: block }, span)
                }
                cst::Cond::Expr(c) => {
                    let cond = self.expr(*c);
                    let body = self.block(body);
                    self.alloc_expr(ExprKind::While { cond, body }, span)
                }
            },
            cst::Expr::For { pat, iter, body } => {
                let pat = self.pattern(pat);
                let iter = self.expr(*iter);
                let body = self.block(body);
                self.alloc_expr(ExprKind::For { pat, iter, body }, span)
            }
            cst::Expr::Loop(b) => {
                let body = self.block(b);
                self.alloc_expr(ExprKind::Loop { body }, span)
            }
            cst::Expr::System(entries) => {
                let entries: Vec<(Label, ExprId)> = entries
                    .into_iter()
                    .map(|(l, e)| {
                        let l = self.label(l);
                        (l, self.expr(e))
                    })
                    .collect();
                self.check_dup_labels(entries.iter().map(|e| e.0));
                self.alloc_expr(ExprKind::System(entries), span)
            }
            cst::Expr::Pipeline(xs) => {
                let stages = xs.into_iter().map(|x| self.expr(x)).collect();
                self.alloc_expr(ExprKind::Pipeline(stages), span)
            }
            cst::Expr::Ref(text, s) => {
                let sym = self.interner.intern(text);
                let sspan = self.span(s);
                self.alloc_expr(ExprKind::Ref(Label { sym, span: sspan }), span)
            }
            cst::Expr::Unsafe(b) => {
                let b = self.block(b);
                self.alloc_expr(ExprKind::Unsafe(b), span)
            }
            cst::Expr::Return(e) => {
                let e = e.map(|e| self.expr(*e));
                self.alloc_expr(ExprKind::Return(e), span)
            }
            cst::Expr::Break(e) => {
                let e = e.map(|e| self.expr(*e));
                self.alloc_expr(ExprKind::Break(e), span)
            }
            cst::Expr::Continue => self.alloc_expr(ExprKind::Continue, span),
            cst::Expr::Unary { op, operand } => {
                let operand = self.expr(*operand);
                let op = match op {
                    cst::UnOp::Neg => UnOp::Neg,
                    cst::UnOp::Not => UnOp::Not,
                    cst::UnOp::Ref => UnOp::Ref,
                    cst::UnOp::RefMut => UnOp::RefMut,
                };
                self.alloc_expr(ExprKind::Unary { op, operand }, span)
            }
            cst::Expr::Binary { op, lhs, rhs } => {
                let lhs = self.expr(*lhs);
                let rhs = self.expr(*rhs);
                self.binary(bin_op(op), lhs, rhs, span)
            }
            cst::Expr::Call { callee, type_args, args } => {
                let callee = self.expr(*callee);
                let type_args = type_args.into_iter().map(|t| self.ty(t)).collect();
                let args = args.into_iter().map(|a| self.expr(a)).collect();
                self.alloc_expr(ExprKind::Call { callee, type_args, args }, span)
            }
            cst::Expr::Field { base, name } => {
                let base = self.expr(*base);
                let name = self.name(name);
                self.alloc_expr(ExprKind::Field { base, name }, span)
            }
            cst::Expr::Index { base, index } => {
                let base = self.expr(*base);
                let index = self.expr(*index);
                self.alloc_expr(ExprKind::Index { base, index }, span)
            }
            cst::Expr::Try(base) => {
                let base = self.expr(*base);
                self.alloc_expr(ExprKind::Try(base), span)
            }
            cst::Expr::Error => self.alloc_expr(ExprKind::Error, span),
            // a hole is a call of `prelude.hole`, which the checker reports
            // with the type it was given
            cst::Expr::Hole => {
                let callee = self.alloc_expr(ExprKind::Std("hole"), span);
                self.alloc_expr(ExprKind::Call { callee, type_args: vec![], args: vec![] }, span)
            }
        }
    }

    fn binary(&mut self, op: BinOp, lhs: ExprId, rhs: ExprId, span: Span) -> ExprId {
        // comparisons do not chain; a parenthesized comparison is one operand
        if op.is_comparison() {
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
        if op == BinOp::Pipe {
            // `x |> f(a)?` is `(x |> f(a))?`: the value goes into the call,
            // then each `?` applies to its result. A parenthesized
            // `(make())?` stays the stage's function.
            let mut tries = 0;
            let mut stage = rhs;
            while let ExprKind::Try(inner) = self.ast.exprs[stage].kind {
                if self.parenthesized.contains(&stage) {
                    break;
                }
                tries += 1;
                stage = inner;
            }
            if tries > 0
                && matches!(self.ast.exprs[stage].kind, ExprKind::Call { .. })
                && !self.parenthesized.contains(&stage)
            {
                let mut e = self.alloc_expr(ExprKind::Binary { op, lhs, rhs: stage }, span);
                for _ in 0..tries {
                    e = self.alloc_expr(ExprKind::Try(e), span);
                }
                return e;
            }
        }
        self.alloc_expr(ExprKind::Binary { op, lhs, rhs }, span)
    }

    /// The arms of a pattern condition: the pattern's, then any other value.
    fn let_arms(&mut self, pat: PatId, matched: ExprId, otherwise: ExprId, span: Span) -> Vec<MatchArm> {
        let any = self.alloc_pat(PatKind::Discard, span);
        vec![
            MatchArm { pat, guard: None, body: matched, span },
            MatchArm { pat: any, guard: None, body: otherwise, span },
        ]
    }

    // ------------------------------------------------------------ literals

    fn string(&mut self, l: &cst::Lit<'_>, span: Span) -> String {
        match l {
            cst::Lit::Str(s) => self.unescape(s, span),
            cst::Lit::RawStr(s) => s.to_string(),
            _ => unreachable!("a string literal"),
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

    fn lit(&mut self, l: &cst::Lit<'_>, span: Span) -> Lit {
        match l {
            cst::Lit::Int(text) => {
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
                Lit::Int { value, suffix }
            }
            cst::Lit::Float(text) => {
                let (digits, suffix) = split_suffix(text);
                let clean: String = digits.chars().filter(|&c| c != '_').collect();
                let suffix_text = suffix;
                let suffix = suffix.map(|s| self.interner.intern(s));
                // `3F32`: an integer written with a float suffix
                let value = clean.parse::<f64>().unwrap_or(0.0);
                if suffix_text.is_some_and(|s| !s.starts_with('F')) {
                    self.error("E0005", span, "integer suffix on a decimal literal", "use F32 or F64");
                }
                Lit::Float { value, suffix }
            }
            cst::Lit::Duration(text) => {
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
            cst::Lit::Str(_) | cst::Lit::RawStr(_) => Lit::Str(self.string(l, span)),
            cst::Lit::Char(raw) => {
                let s = self.unescape(raw, span);
                Lit::Char(s.chars().next().unwrap_or('\0'))
            }
            cst::Lit::Symbol(text) => Lit::Symbol(self.interner.intern(text)),
            cst::Lit::Bool(b) => Lit::Bool(*b),
        }
    }

    // ------------------------------------------------------------ patterns

    fn pattern(&mut self, (p, s): cst::P<'_>) -> PatId {
        let span = self.span(s);
        match p {
            cst::Pat::Discard => self.alloc_pat(PatKind::Discard, span),
            cst::Pat::Bind(n) => {
                let n = self.value_name(n);
                self.alloc_pat(PatKind::Bind(n), span)
            }
            cst::Pat::Lit { negative, lit } => {
                let lit = self.lit(&lit, span);
                self.alloc_pat(PatKind::Lit { lit, negative }, span)
            }
            cst::Pat::Unit => self.alloc_pat(PatKind::Lit { lit: Lit::Unit, negative: false }, span),
            cst::Pat::Tuple(ps) => {
                let items = ps.into_iter().map(|q| self.pattern(q)).collect();
                self.alloc_pat(PatKind::Tuple(items), span)
            }
            cst::Pat::Record { fields, open } => {
                let fields: Vec<(Label, Option<PatId>)> = fields
                    .into_iter()
                    .map(|(l, sub)| {
                        let l = self.label(l);
                        (l, sub.map(|q| self.pattern(q)))
                    })
                    .collect();
                self.check_dup_labels(fields.iter().map(|f| f.0));
                self.alloc_pat(PatKind::Record { fields, open }, span)
            }
            cst::Pat::Variant { ty, name, args } => {
                let ty = ty.map(|(module, t)| {
                    let module = module.into_iter().map(|n| self.name(n)).collect();
                    (module, self.name(t))
                });
                let name = self.name(name);
                let args = args.map(|xs| xs.into_iter().map(|q| self.pattern(q)).collect());
                self.alloc_pat(PatKind::Variant { ty, name, args }, span)
            }
            cst::Pat::Error => self.alloc_pat(PatKind::Error, span),
        }
    }
}

fn index(e: ExprId) -> usize {
    e.into_raw().into_u32() as usize
}

fn let_kind(k: cst::LetKind) -> LetKind {
    match k {
        cst::LetKind::Let => LetKind::Let,
        cst::LetKind::Var => LetKind::Var,
        cst::LetKind::Const => LetKind::Const,
    }
}

fn bin_op(op: cst::BinOp) -> BinOp {
    use cst::BinOp as C;
    match op {
        C::Pipe => BinOp::Pipe,
        C::Or => BinOp::Or,
        C::And => BinOp::And,
        C::Eq => BinOp::Eq,
        C::Ne => BinOp::Ne,
        C::Lt => BinOp::Lt,
        C::Le => BinOp::Le,
        C::Gt => BinOp::Gt,
        C::Ge => BinOp::Ge,
        C::Range => BinOp::Range,
        C::RangeIncl => BinOp::RangeIncl,
        C::BitOr => BinOp::BitOr,
        C::BitXor => BinOp::BitXor,
        C::BitAnd => BinOp::BitAnd,
        C::Shl => BinOp::Shl,
        C::Shr => BinOp::Shr,
        C::Add => BinOp::Add,
        C::Sub => BinOp::Sub,
        C::Mul => BinOp::Mul,
        C::Div => BinOp::Div,
        C::Rem => BinOp::Rem,
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

/// The subexpressions an expression holds directly.
fn expr_children(kind: &ExprKind, f: &mut impl FnMut(ExprId)) {
    let stmt = |s: &Stmt, f: &mut dyn FnMut(ExprId)| match s {
        Stmt::Let { value, .. } => f(*value),
        Stmt::Assign { path, value, .. } => {
            for p in path {
                if let PlaceElem::Index(i) = p {
                    f(*i)
                }
            }
            f(*value)
        }
        Stmt::Use { call, .. } => f(*call),
        Stmt::Expr { expr, .. } => f(*expr),
    };
    match kind {
        ExprKind::Lit(_)
        | ExprKind::Name(_)
        | ExprKind::TypeName(_)
        | ExprKind::Continue
        | ExprKind::Ref(_)
        | ExprKind::Std(_)
        | ExprKind::DeclName(_)
        | ExprKind::Error => {}
        ExprKind::Interp(parts) => {
            for p in parts {
                if let InterpPart::Expr(e) = p {
                    f(*e)
                }
            }
        }
        ExprKind::Field { base, .. } | ExprKind::Try(base) | ExprKind::Unary { operand: base, .. } => {
            f(*base)
        }
        ExprKind::Index { base, index } => {
            f(*base);
            f(*index)
        }
        ExprKind::Call { callee, args, .. } => {
            f(*callee);
            args.iter().for_each(|a| f(*a))
        }
        ExprKind::Binary { lhs, rhs, .. } => {
            f(*lhs);
            f(*rhs)
        }
        ExprKind::Tuple(xs) | ExprKind::List(xs) | ExprKind::Set(xs) | ExprKind::Pipeline(xs) => {
            xs.iter().for_each(|x| f(*x))
        }
        ExprKind::Record { spread, entries } => {
            spread.iter().for_each(|x| f(*x));
            for e in entries {
                if let RecordEntry::Field { value, .. } = e {
                    f(*value)
                }
            }
        }
        ExprKind::Map(kvs) => {
            for (k, v) in kvs {
                f(*k);
                f(*v)
            }
        }
        ExprKind::Lambda(l) => f(l.body),
        ExprKind::If { cond, then, els } => {
            f(*cond);
            f(*then);
            els.iter().for_each(|x| f(*x))
        }
        ExprKind::Match { scrutinee, arms } => {
            f(*scrutinee);
            for a in arms {
                a.guard.iter().for_each(|g| f(*g));
                f(a.body)
            }
        }
        ExprKind::While { cond, body } => {
            f(*cond);
            f(*body)
        }
        ExprKind::For { iter, body, .. } => {
            f(*iter);
            f(*body)
        }
        ExprKind::Loop { body } | ExprKind::Unsafe(body) => f(*body),
        ExprKind::Block { stmts } => {
            for s in stmts {
                stmt(s, f)
            }
        }
        ExprKind::Return(x) | ExprKind::Break(x) => x.iter().for_each(|x| f(*x)),
        ExprKind::System(entries) => entries.iter().for_each(|(_, x)| f(*x)),
    }
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
