//! Expression and pattern inference (bidirectional HM with rows).

use lang_base::{Diagnostic, Span, Symbol};
use lang_hir::{
    Arm, BinOp, DefId, DefKind, ExprId, ExprKind, GenericId, HTy, InterpPart, Lit, LocalId, Owner, PatId,
    PatKind, Stmt, UnOp,
};
use rustc_hash::FxHashMap;

use crate::env::*;
use crate::program::{Global, Scheme};
use crate::solve::*;
use crate::ty::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Checking a definition once, generically (skolems for declared generics).
    Generic,
    /// Re-checking a definition at concrete type arguments for code generation.
    Instance,
}

/// Per-function (or lambda) checking context.
pub struct FnCtx {
    pub owner: Owner,
    pub ret: Ty,
    /// Ambient effect row (open).
    pub eff: Ty,
    /// Where each effect label was first introduced.
    pub eff_sites: Vec<(Symbol, Span)>,
    /// Declared effect bound, if annotated.
    pub bound: Option<(Vec<Symbol>, Span)>,
}

/// Resolutions recorded for code generation.
#[derive(Debug, Clone)]
pub enum Res {
    /// Function or global instance.
    Callee {
        def: DefId,
        args: Vec<Ty>,
    },
    Ctor {
        enum_def: DefId,
        variant: u32,
        args: Vec<Ty>,
    },
    Method(Target),
    /// Field read from a record (`label`), struct, or tuple.
    Field(Symbol),
    /// `?` with an optional `From` conversion of the error.
    Try(Option<Target>),
    /// Display implementation for an interpolated expression.
    Display(Target),
    /// Equality / ordering on a type (structural, runtime-generic).
    Compare(Ty),
    /// Operator on a nominal type implemented by a library function.
    OpFn(Target),
    Iter(IterKind),
    Index(IndexKind),
    /// Record value coerced to a transparent nominal struct (no-op).
    Coerce,
}

/// Implicit conversions applied to an expression's value.
#[derive(Debug, Clone)]
pub enum Coercion {
    /// Exact-shape record to a transparent struct (same layout).
    Struct,
    /// Concrete value to `dyn Protocol<args>`.
    Dyn { protocol: DefId, args: Vec<Ty> },
    /// A list-like collection viewed as a lazy `Seq` of its elements.
    ToSeq,
    /// An integer range viewed as a lazy `Seq<Int>`.
    RangeToSeq,
    /// A frozen byte array (or buffer) as immutable `Bytes`.
    ToBytes,
    /// Explicit dynamic encoding of a literal (or map field) as `Data`.
    ToData,
}

#[derive(Debug, Clone)]
pub enum Target {
    /// Dictionary call through a `dyn Protocol` value.
    Dyn {
        protocol: DefId,
        method: Symbol,
    },
    Fn {
        def: DefId,
        args: Vec<Ty>,
    },
    Derived {
        imp: DerivedImpl,
        self_ty: Ty,
    },
    /// Call a function-valued record field.
    Field(Symbol),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IterKind {
    Range,
    List,
    Array,
    Seq,
    Stream,
    Map,
    Set,
    Buffer,
}

#[derive(Debug, Clone)]
pub enum IndexKind {
    List,
    Array,
    Buffer,
    Map,
    Bytes,
    /// Static record label (`record[:label]`).
    Label(Symbol),
}

pub struct Infer<'g, 'a> {
    pub g: &'g Global<'a>,
    pub icx: InferCtxt,
    pub mode: Mode,
    pub diags: Vec<Diagnostic>,
    pub pending: Vec<Constraint>,
    pub locals: FxHashMap<LocalId, Ty>,
    pub exprs: FxHashMap<ExprId, Ty>,
    pub pats: FxHashMap<PatId, Ty>,
    pub res: FxHashMap<ExprId, Res>,
    /// `Display` resolutions of interpolated expressions (kept apart from
    /// the expressions' own resolutions).
    pub displays: FxHashMap<ExprId, Target>,
    pub coercions: FxHashMap<ExprId, Coercion>,
    /// Callbacks that introduced region brands (`for<S>` arguments) and
    /// their region skolems, for the escape analysis in `regions`.
    pub region_lambdas: Vec<(ExprId, Vec<u32>)>,
    /// Provider initializers (`fn(InitScope, C)`): their scope and
    /// configuration (with dependency outputs) belong to the running system.
    pub init_lambdas: Vec<ExprId>,
    /// Locals bound by patterns from a parameter's value: cells reached
    /// through them are the caller's.
    pub param_derived: rustc_hash::FxHashSet<LocalId>,
    /// Capabilities a generic body requires of its type parameters
    /// (skolem, `Transfer`/`Share`), checked where it is instantiated.
    pub implied_caps: Vec<(u32, &'static str)>,
    /// Generalized local `let`s: binding -> (initializer, scheme size, type).
    pub poly_locals: FxHashMap<LocalId, (ExprId, u32, Ty)>,
    /// Instantiation of each use of a generalized local.
    pub local_insts: FxHashMap<ExprId, Vec<Ty>>,
    pub fns: Vec<FnCtx>,
    /// The user function whose arguments are being checked.
    pub callee_ctx: Option<DefId>,
    pub loops: FxHashMap<ExprId, (Ty, bool)>,
    /// Declared generic parameters in scope.
    pub generics: FxHashMap<GenericId, Ty>,
    pub self_ty: Option<Ty>,
    /// Where-clause assumptions in scope: (protocol, args, subject).
    pub givens: Vec<(DefId, Vec<Ty>, Ty)>,
    /// In-progress monotypes for definitions of the current binding group.
    pub mono: FxHashMap<DefId, Ty>,
    /// Module of the definition being checked (for privacy).
    pub module: Option<lang_hir::ModuleId>,
    /// Nominal types currently being explored by structural checks
    /// (recursive types are assumed to satisfy them coinductively).
    pub visiting: Vec<DefId>,
    /// Depth of enclosing `unsafe` blocks.
    pub unsafe_depth: u32,
}

impl<'g, 'a> Infer<'g, 'a> {
    pub fn new(g: &'g Global<'a>, mode: Mode) -> Self {
        Infer {
            g,
            icx: InferCtxt::new(Some(g.env.b.never)),
            mode,
            diags: vec![],
            pending: vec![],
            locals: FxHashMap::default(),
            exprs: FxHashMap::default(),
            pats: FxHashMap::default(),
            res: FxHashMap::default(),
            displays: FxHashMap::default(),
            coercions: FxHashMap::default(),
            region_lambdas: vec![],
            init_lambdas: vec![],
            param_derived: Default::default(),
            implied_caps: vec![],
            poly_locals: FxHashMap::default(),
            callee_ctx: None,
            local_insts: FxHashMap::default(),
            fns: vec![],
            loops: FxHashMap::default(),
            generics: FxHashMap::default(),
            self_ty: None,
            givens: vec![],
            mono: FxHashMap::default(),
            module: None,
            visiting: vec![],
            unsafe_depth: 0,
        }
    }

    pub fn b(&self) -> &Builtins {
        &self.g.env.b
    }

    pub fn sym(&self, s: Symbol) -> &str {
        self.g.interner.resolve(s)
    }

    pub fn intern_existing(&self, s: &str) -> Option<Symbol> {
        self.g.interner.get(s)
    }

    // ------------------------------------------------------------ errors

    pub fn show(&mut self, t: &Ty) -> String {
        let t = self.icx.resolve(t);
        let names = crate::program::Names { g: self.g, skolems: &self.icx.skolems };
        Printer::new(&names).print(&t)
    }

    pub fn error(
        &mut self,
        code: &'static str,
        span: Span,
        msg: impl Into<String>,
        label: impl Into<String>,
    ) {
        if self.mode == Mode::Instance && std::env::var_os("LANG_DEBUG_INSTANCE").is_none() {
            // Instances re-check already-validated code; suppress duplicates.
            return;
        }
        let mut d = Diagnostic::error(msg).with_code(code).with_label(span, label);
        if let Some(def) = self.callee_ctx {
            let dd = &self.g.hir.defs[def];
            d = d.with_related(self.g.interner.resolve(dd.name).to_string(), dd.span);
        }
        self.diags.push(d);
    }

    /// `error` with a related user declaration (callee or type).
    pub fn error_rel(
        &mut self,
        code: &'static str,
        span: Span,
        msg: impl Into<String>,
        label: impl Into<String>,
        rel: Option<DefId>,
    ) {
        let saved = self.callee_ctx;
        if let Some(d) = rel.filter(|d| !self.g.hir.modules[self.g.hir.defs[*d].module].is_std) {
            self.callee_ctx = Some(d);
        }
        self.error(code, span, msg, label);
        self.callee_ctx = saved;
    }

    /// The user-defined nominal type at the head of `t`, if any.
    pub fn head_def(&mut self, t: &Ty) -> Option<DefId> {
        let t = self.icx.resolve(t);
        let t = self.deref(&t);
        match &*t {
            TyKind::Con(def, _) => Some(*def),
            _ => None,
        }
    }

    pub fn push_diag(&mut self, d: Diagnostic) {
        if self.mode == Mode::Generic || std::env::var_os("LANG_DEBUG_INSTANCE").is_some() {
            let mut d = d;
            if let Some(def) = self.callee_ctx {
                let dd = &self.g.hir.defs[def];
                if !d.related.iter().any(|r| r.1 == dd.span) {
                    d = d.with_related(self.g.interner.resolve(dd.name).to_string(), dd.span);
                }
            }
            self.diags.push(d);
        }
    }

    /// Report a unification failure with expected/actual context.
    pub fn report_unify(&mut self, e: UnifyError, span: Span, expected: &Ty, actual: &Ty) {
        let d = match e {
            UnifyError::MissingField { label, .. }
                if matches!(self.label_str(label).chars().next(), Some('A'..='Z' | '%')) =>
            {
                // effect rows share the row machinery with records
                let raw = self.label_str(label);
                let (exp, act) = (self.show(expected), self.show(actual));
                if raw.starts_with("%region") {
                    // a closure capturing a scoped value, where its type cannot say so
                    Diagnostic::error("a closure capturing a scoped value would escape its scope")
                        .with_code("E0401")
                        .with_label(span, "this closure captures a value bound to an inner scope")
                        .with_note("values carrying a scope brand (handles, transactions, running systems, or closures capturing them) cannot outlive the callback that introduced the scope")
                } else {
                    let l = if raw == "%confined" {
                        "task-confined capture".to_string()
                    } else {
                        format!("effect `{raw}`")
                    };
                    Diagnostic::error(format!("{l} is not permitted here"))
                        .with_code("E0501")
                        .with_label(span, format!("this has {l}"))
                        .with_shapes(exp, act)
                }
            }
            UnifyError::MissingField { label, record } => {
                let l = self.label_str(label);
                let r = self.show(&record);
                // the value has the field but the expected type does not: an
                // extra field; otherwise the value lacks a required one
                let act_r = self.icx.resolve(actual);
                let extra = match &*act_r {
                    TyKind::Record(row) => row_fields(row).0.iter().any(|(x, _)| *x == label),
                    _ => false,
                };
                let msg =
                    if extra { format!("unexpected field `{l}`") } else { format!("missing field `{l}`") };
                let mut d = Diagnostic::error(msg).with_code("E0301").with_label(
                    span,
                    if extra {
                        format!("the expected type `{r}` has no field `{l}`")
                    } else {
                        format!("the record type `{r}` has no field `{l}`")
                    },
                );
                let (exp, act) = (self.show(expected), self.show(actual));
                d = d.with_shapes(exp, act);
                if record_is_closed(&self.icx.resolve(&record)) {
                    d = d.with_note(
                        "closed record types never drop fields implicitly; use an open row `{..Rest}` or `records.project`",
                    );
                }
                d
            }
            UnifyError::RigidRow { label, record } => {
                let l = self.label_str(label);
                let r = self.show(&record);
                Diagnostic::error(format!("field `{l}` is not visible here"))
                    .with_code("E0302")
                    .with_label(span, format!("`{r}` only exposes its declared prefix"))
                    .with_shapes(self.show(expected), self.show(actual))
            }
            UnifyError::Escape { skolem, ty } => {
                // fresh callback regions are numbered internally (`S12`)
                let name = self
                    .g
                    .interner
                    .resolve(self.icx.skolems[skolem as usize].name)
                    .trim_end_matches(|c: char| c.is_ascii_digit())
                    .to_string();
                let t = self.show(&ty);
                let _ = t;
                Diagnostic::error("a scoped value escapes its scope")
                    .with_code("E0401")
                    .with_label(span, format!("this value is bound to scope region `{name}`"))
                    .with_note("values carrying a scope brand (handles, transactions, running systems, or closures capturing them) cannot outlive the callback that introduced the scope")
            }
            UnifyError::Occurs(..) => {
                let (exp, act) = (self.show(expected), self.show(actual));
                Diagnostic::error("infinite type")
                    .with_code("E0304")
                    .with_label(span, format!("cannot unify `{exp}` with `{act}`"))
                    .with_shapes(exp, act)
            }
            UnifyError::Arity(a, b) => Diagnostic::error("function arity mismatch")
                .with_code("E0305")
                .with_label(span, format!("expected {a} parameters, found {b}")),
            UnifyError::Mismatch(..) => {
                let (exp, act) = (self.show(expected), self.show(actual));
                let mut d = Diagnostic::error("type mismatch")
                    .with_code("E0300")
                    .with_label(span, format!("expected `{exp}`, found `{act}`"))
                    .with_shapes(exp.clone(), act.clone());
                // the declarations of user-defined types involved
                for t in [expected, actual] {
                    if let TyKind::Con(def, _) = &*self.icx.resolve(t) {
                        let dd = &self.g.hir.defs[*def];
                        if !self.g.hir.modules[dd.module].is_std && !d.related.iter().any(|r| r.1 == dd.span)
                        {
                            d = d.with_related(self.g.interner.resolve(dd.name).to_string(), dd.span);
                        }
                    }
                }
                d
            }
        };
        self.push_diag(d);
    }

    pub fn label_str(&self, l: Symbol) -> String {
        let s = self.sym(l);
        if s.contains('/') { format!("[:{s}]") } else { s.to_string() }
    }

    /// Unify `actual` with `expected`, reporting at `span`.
    pub fn expect(&mut self, span: Span, expected: &Ty, actual: &Ty) {
        self.icx.span_hint = span;
        if let Err(e) = self.icx.unify(expected, actual) {
            self.report_unify(e, span, expected, actual);
        }
    }

    // ------------------------------------------------------------ type helpers

    pub fn hty(&mut self, t: &HTy) -> Ty {
        let gens = self.generics.clone();
        let sub = Subst { generics: &gens, self_ty: self.self_ty.clone() };
        let mut diags = vec![];
        let r = self.g.env.lower(self.g.hir, &mut self.icx, t, &sub, true, &mut diags);
        for d in diags {
            self.push_diag(d);
        }
        r
    }

    pub fn con(&self, d: DefId, args: Vec<Ty>) -> Ty {
        mk(TyKind::Con(d, args))
    }

    pub fn prim(&self, d: DefId) -> Ty {
        mk(TyKind::Con(d, vec![]))
    }

    pub fn bool_ty(&self) -> Ty {
        self.prim(self.b().bool_)
    }

    pub fn int_ty(&self) -> Ty {
        self.prim(self.b().i64_)
    }

    pub fn string_ty(&self) -> Ty {
        self.prim(self.b().string)
    }

    pub fn option_ty(&self, t: Ty) -> Ty {
        self.con(self.b().option, vec![t])
    }

    pub fn result_ty(&self, t: Ty, e: Ty) -> Ty {
        self.con(self.b().result, vec![t, e])
    }

    /// Strip references (borrows are transparent for typing).
    pub fn deref(&mut self, t: &Ty) -> Ty {
        let t = self.icx.shallow(t);
        match &*t {
            TyKind::Ref(_, inner) => self.deref(inner),
            _ => t,
        }
    }

    pub fn fn_ty(&mut self, params: Vec<Ty>, ret: Ty) -> Ty {
        let eff = self.icx.fresh_row();
        mk(TyKind::Fn(FnTy { params, ret, eff, once: false }))
    }

    pub fn set_expr(&mut self, e: ExprId, t: Ty) -> Ty {
        self.exprs.insert(e, t.clone());
        t
    }

    pub fn record_res(&mut self, e: ExprId, r: Res) {
        self.res.insert(e, r);
    }

    pub fn cur_fn(&mut self) -> &mut FnCtx {
        self.fns.last_mut().expect("inside a function")
    }

    /// Add the effects of a callee (its effect row) to the ambient effects.
    pub fn add_effects(&mut self, eff: &Ty, span: Span) {
        if self.fns.is_empty() {
            return;
        }
        let (labels, tail) = self.icx.flatten_row(eff);
        for (l, t) in &labels {
            // capture marks (`%region`, `%confined`) describe a closure value,
            // not an effect of calling it
            if self.sym(*l).starts_with('%') {
                continue;
            }
            self.add_effect_label(*l, t.clone(), span);
        }
        if let TyKind::Var(_) = &*tail {
            let amb = self.fns.last().unwrap().eff.clone();
            let (amb_fields, amb_tail) = self.icx.flatten_row(&amb);
            if let TyKind::Var(_) = &*amb_tail
                && !matches!((&*tail, &*amb_tail), (TyKind::Var(a), TyKind::Var(b)) if self.icx.root(*a) == self.icx.root(*b))
            {
                let rest: Vec<(Symbol, Ty)> =
                    amb_fields.into_iter().filter(|(l, _)| !labels.iter().any(|(m, _)| m == l)).collect();
                let r = row(rest, amb_tail);
                let _ = self.icx.unify(&tail, &r);
            }
        }
    }

    pub fn add_effect_label(&mut self, l: Symbol, payload: Ty, span: Span) {
        let amb = self.fns.last().unwrap().eff.clone();
        let (fields, tail) = self.icx.flatten_row(&amb);
        if fields.iter().any(|(m, _)| *m == l) {
            return;
        }
        if let TyKind::Var(_) = &*tail {
            let rest = self.icx.fresh_row();
            let _ = self.icx.unify(&tail, &row([(l, payload)], rest));
            self.cur_fn().eff_sites.push((l, span));
            return;
        }
        // the closure's function type is already fixed (a declared `fn`
        // type): a capture mark it cannot carry would be lost
        let name = self.sym(l).to_string();
        if matches!(tail.as_ref(), TyKind::RowEmpty) && name.starts_with('%') {
            let shown = self.show(&amb);
            let (code, what) = if name == "%confined" {
                ("E0402", "task-confined state (a `Cell` or a transaction)")
            } else {
                ("E0401", "a scoped value")
            };
            self.push_diag(
                Diagnostic::error(format!("this closure captures {what} its declared function type cannot track"))
                    .with_code(code)
                    .with_label(span, "captured here")
                    .with_note(format!("the closure is used as a function with effects `!{{{}}}`, which may be sent to other tasks or outlive the scope; keep it local (let its type be inferred) or capture shareable state such as an `Atom`", shown.trim_start_matches('{').trim_end_matches('}'))),
            );
        }
    }

    // ------------------------------------------------------------ schemes

    /// Instantiate a definition's type at a use site.
    pub fn instantiate_def(&mut self, def: DefId, type_args: &[HTy], span: Span, e: ExprId) -> Ty {
        if let Some(t) = self.mono.get(&def).cloned() {
            if !type_args.is_empty() {
                self.error("E0306", span, "explicit type arguments on a recursive use", "remove `<...>`");
            }
            self.record_res(e, Res::Callee { def, args: vec![] });
            return t;
        }
        let Some(scheme) = self.g.scheme(def) else {
            // Not yet checked (cycle through globals) or erroneous.
            return self.icx.fresh();
        };
        let explicit: Vec<Ty> = type_args.iter().map(|t| self.hty(t)).collect();
        // a record written for a `Row` type parameter stands for its fields
        let row_marker = self.g.hir.try_lang("Row");
        let explicit: Vec<Ty> = explicit
            .into_iter()
            .enumerate()
            .map(|(i, t)| {
                let is_row = scheme.generics.get(i).is_some_and(|g| {
                    self.g.hir.generics[*g].bounds.iter().any(
                        |b| matches!(&b.kind, lang_hir::HTyKind::Named { def, .. } if Some(*def) == row_marker),
                    )
                });
                match &*t {
                    TyKind::Record(r) if is_row => r.clone(),
                    _ => t,
                }
            })
            .collect();
        if explicit.len() > scheme.generics.len() {
            let n = scheme.generics.len();
            self.error("E0210", span, format!("too many type arguments: expected at most {n}"), "here");
        }
        let (ty, args) = self.instantiate(&scheme, &explicit, span);
        self.record_res(e, Res::Callee { def, args });
        open_effects(&mut self.icx, &ty)
    }

    /// Instantiate a scheme: fresh variables for all quantified variables
    /// (explicit arguments bind the leading declared generics). Re-adds the
    /// scheme's constraints.
    pub fn instantiate(&mut self, scheme: &Scheme, explicit: &[Ty], span: Span) -> (Ty, Vec<Ty>) {
        let args: Vec<Ty> = (0..scheme.n)
            .map(|i| explicit.get(i as usize).cloned().unwrap_or_else(|| self.icx.fresh()))
            .collect();
        let ty = subst_gen(&scheme.ty, &args);
        for c in &scheme.constraints {
            let c = c.subst(&args, span);
            self.pending.push(c);
        }
        (ty, args)
    }

    // ------------------------------------------------------------ expressions

    pub fn infer(&mut self, e: ExprId) -> Ty {
        self.check_opt(e, None)
    }

    /// Check `e` against `expected`, reporting a mismatch.
    /// Coerce a concrete value to an expected `dyn Protocol`.
    fn try_dyn(&mut self, e: ExprId, actual: &Ty, expected: &Ty) -> bool {
        let exp = self.deref(expected);
        let TyKind::Dyn(p, pargs) = &*exp else { return false };
        let act = self.deref(actual);
        if matches!(&*act, TyKind::Dyn(..) | TyKind::Var(_) | TyKind::Error) {
            return false;
        }
        let span = self.g.hir.exprs[e].span;
        // a dictionary-dispatched value has no brand: scoped values cannot
        // hide behind one
        if self.carries_region(&act) {
            self.error(
                "E0401",
                span,
                "a scoped value cannot become `dyn`",
                "this value carries a scope brand that `dyn` would erase",
            );
        }
        self.pending.push(Constraint::Impl {
            proto: *p,
            args: pargs.clone(),
            ty: act.clone(),
            span,
            expr: None,
        });
        // a `dyn` value is copied freely and may cross tasks: affine or
        // task-confined values cannot hide in one (a type parameter is held
        // to `Transfer`; its instances are checked where they are concrete)
        let generic = ty_any(&act, &mut |k| matches!(k, TyKind::Skolem(_)));
        if generic {
            let skolems = self.icx.skolems.clone();
            ty_any(&act, &mut |k| {
                if let TyKind::Skolem(s) = k
                    && !skolems.get(*s as usize).is_some_and(|i| i.region)
                {
                    self.implied_caps.push((*s, "Share"));
                }
                false
            });
        }
        if let Some(share) = self.g.hir.try_lang(if generic { "Transfer" } else { "Share" }) {
            self.pending.push(Constraint::Impl {
                proto: share,
                args: vec![],
                ty: act.clone(),
                span,
                expr: None,
            });
        }
        self.coercions.insert(e, Coercion::Dyn { protocol: *p, args: pargs.clone() });
        true
    }

    /// Encode a value into an expected `Data` (record literals and the fields
    /// of a record literal constructing `Map<Symbol, Data>`).
    fn try_data(&mut self, e: ExprId, actual: &Ty, expected: &Ty) -> bool {
        let exp = self.deref(expected);
        if !matches!(&*exp, TyKind::Con(d, _) if *d == self.b().data) {
            return false;
        }
        let act = self.deref(actual);
        if matches!(&*act, TyKind::Error) || matches!(&*act, TyKind::Con(d, _) if *d == self.b().data) {
            return false;
        }
        let span = self.g.hir.exprs[e].span;
        self.pending.push(Constraint::Impl {
            proto: self.b().codec,
            args: vec![],
            ty: act.clone(),
            span,
            expr: None,
        });
        self.coercions.insert(e, Coercion::ToData);
        true
    }

    /// Collection-view coercions: `List<T>`/`Array<T>` where `Seq<T>` is
    /// expected, and `Array<Byte>`/`Buffer<Byte>` where `Bytes` is expected.
    fn try_view(&mut self, e: ExprId, actual: &Ty, expected: &Ty) -> bool {
        let exp = self.deref(expected);
        let act = self.deref(actual);
        let (TyKind::Con(ed, eargs), TyKind::Con(ad, aargs)) = (&*exp, &*act) else { return false };
        let (en, an) = (self.lang_name(*ed), self.lang_name(*ad));
        match (en.as_deref(), an.as_deref()) {
            (Some("Seq"), Some("List" | "Array")) if eargs.len() == 1 && aargs.len() == 1 => {
                let (x, y) = (eargs[0].clone(), aargs[0].clone());
                let span = self.g.hir.exprs[e].span;
                self.expect(span, &x, &y);
                self.coercions.insert(e, Coercion::ToSeq);
                true
            }
            (Some("Seq"), Some("Range")) if eargs.len() == 1 && aargs.len() == 1 => {
                let int = self.int_ty();
                let span = self.g.hir.exprs[e].span;
                self.expect(span, &int, &aargs[0].clone());
                self.expect(span, &eargs[0].clone(), &int);
                self.coercions.insert(e, Coercion::RangeToSeq);
                true
            }
            (Some("Bytes"), Some("Array" | "Buffer")) if aargs.len() == 1 => {
                let u8t = self.prim(self.b().u8_);
                let span = self.g.hir.exprs[e].span;
                self.expect(span, &u8t, &aargs[0].clone());
                self.coercions.insert(e, Coercion::ToBytes);
                true
            }
            _ => false,
        }
    }

    pub fn check(&mut self, e: ExprId, expected: &Ty) -> Ty {
        let t = self.check_opt(e, Some(expected));
        self.icx.span_hint = self.g.hir.exprs[e].span;
        if self.try_dyn(e, &t, expected) || self.try_view(e, &t, expected) {
            return expected.clone();
        }
        if matches!(self.g.hir.exprs[e].kind, ExprKind::Record { .. }) && self.try_data(e, &t, expected) {
            return expected.clone();
        }
        if !self.icx.try_unify(expected, &t) {
            // an existing exact-shape record may coerce to a transparent struct
            if self.try_coerce(e, &t, expected) {
                return expected.clone();
            }
            if let Err(err) = self.icx.unify(expected, &t) {
                let span = self.g.hir.exprs[e].span;
                self.report_unify(err, span, expected, &t);
            }
        }
        t
    }

    pub fn check_opt(&mut self, e: ExprId, expected: Option<&Ty>) -> Ty {
        let t = self.check_inner(e, expected);
        self.set_expr(e, t.clone());
        t
    }

    fn check_inner(&mut self, e: ExprId, expected: Option<&Ty>) -> Ty {
        let hir = self.g.hir;
        let span = hir.exprs[e].span;
        let kind = hir.exprs[e].kind.clone();
        match kind {
            ExprKind::Lit(l) => self.lit(&l, span, expected),
            ExprKind::Interp(parts) => {
                for p in parts {
                    if let InterpPart::Expr(x) = p {
                        let t = self.infer(x);
                        let xs = hir.exprs[x].span;
                        self.pending.push(Constraint::Display { ty: t, span: xs, expr: Some(x) });
                    }
                }
                self.string_ty()
            }
            ExprKind::Local(l) => {
                // a generalized local is instantiated at each use
                if let Some((_, n, scheme)) = self.poly_locals.get(&l).cloned() {
                    let args: Vec<Ty> = (0..n).map(|_| self.icx.fresh()).collect();
                    self.local_insts.insert(e, args.clone());
                    return crate::env::subst_gen(&scheme, &args);
                }
                match self.locals.get(&l) {
                    Some(t) => t.clone(),
                    None => self.icx.fresh(),
                }
            }
            ExprKind::Def { def, type_args } => match &hir.defs[def].kind {
                DefKind::Fn(f) if f.abi == lang_hir::FnAbi::C => {
                    if self.unsafe_depth == 0 {
                        let n = self.sym(hir.defs[def].name).to_string();
                        self.error(
                            "E0406",
                            span,
                            format!("call to foreign function `{n}` requires `unsafe`"),
                            "wrap the call in `unsafe { ... }`",
                        );
                    }
                    if let Some(fl) = self.intern_existing("Foreign")
                        && !self.fns.is_empty()
                    {
                        self.add_effect_label(fl, unit(), span);
                    }
                    self.instantiate_def(def, &type_args, span, e)
                }
                // raw-pointer operations of `std.ffi` are foreign calls too
                DefKind::Fn(f)
                    if hir.modules[hir.defs[def].module].is_std
                        && matches!(&f.effect, Some(lang_hir::HEffect::Set { labels, .. }) if labels.iter().any(|l| self.sym(*l) == "Foreign")) =>
                {
                    if self.unsafe_depth == 0 {
                        let n = self.sym(hir.defs[def].name).to_string();
                        self.error(
                            "E0406",
                            span,
                            format!("`{n}` uses raw pointers and requires `unsafe`"),
                            "wrap the call in `unsafe { ... }`",
                        );
                    }
                    self.instantiate_def(def, &type_args, span, e)
                }
                DefKind::Fn(_) | DefKind::Global(_) => self.instantiate_def(def, &type_args, span, e),
                _ => error_ty(),
            },
            ExprKind::Ctor { enum_def, variant, type_args } => {
                self.ctor(enum_def, variant, &type_args, span, e, expected)
            }
            ExprKind::AssocFn { ty, name, name_span, type_args } => {
                // a bare type namespace (`Cell.new`) leaves its arguments to inference
                let self_ty = match &ty.kind {
                    lang_hir::HTyKind::Named { def, args } if args.is_empty() => {
                        let n = match &self.g.hir.defs[*def].kind {
                            DefKind::Struct(s) => s.generics.len(),
                            DefKind::Enum(en) => en.generics.len(),
                            DefKind::Opaque(o) => o.generics.len(),
                            _ => 0,
                        };
                        if n > 0 && !matches!(self.g.hir.defs[*def].kind, DefKind::Alias(_)) {
                            let args = (0..n).map(|_| self.icx.fresh()).collect();
                            self.con(*def, args)
                        } else {
                            self.hty(&ty)
                        }
                    }
                    _ => self.hty(&ty),
                };
                let explicit: Vec<Ty> = type_args.iter().map(|t| self.hty(t)).collect();
                let fn_ty = self.icx.fresh();
                self.pending.push(Constraint::Assoc {
                    self_ty,
                    name,
                    fn_ty: fn_ty.clone(),
                    explicit,
                    span: name_span,
                    expr: Some(e),
                });
                self.solve_some();
                fn_ty
            }
            ExprKind::Field { base, label, label_span } => {
                let bt = self.infer(base);
                self.field(bt, label, label_span, e)
            }
            ExprKind::Index { base, index } => {
                let bt = self.infer(base);
                // a literal symbol index is a static record label
                if let ExprKind::Lit(Lit::Symbol(s)) = &hir.exprs[index].kind {
                    let s = *s;
                    let bt = self.deref(&bt);
                    if matches!(&*bt, TyKind::Record(_) | TyKind::Var(_)) {
                        self.set_expr(index, self.prim(self.b().symbol));
                        let t = self.field(bt, s, hir.exprs[index].span, e);
                        self.record_res(e, Res::Index(IndexKind::Label(s)));
                        return t;
                    }
                }
                let it = self.infer(index);
                let out = self.icx.fresh();
                self.pending.push(Constraint::Index {
                    coll: bt,
                    index: it,
                    out: out.clone(),
                    span,
                    expr: Some(e),
                });
                self.solve_some();
                out
            }
            ExprKind::Call { callee, args } => self.call(e, callee, &args, span, expected),
            ExprKind::MethodCall { recv, method, method_span, type_args, args } => {
                self.method_call(e, recv, method, method_span, &type_args, &args, span, expected)
            }
            ExprKind::Try(inner) => self.try_expr(e, inner, span),
            ExprKind::Unary { op, operand } => match op {
                UnOp::Not => {
                    let b = self.bool_ty();
                    self.check(operand, &b);
                    b
                }
                UnOp::Neg => {
                    let t = self.check_opt(operand, expected);
                    self.pending.push(Constraint::Numeric { ty: t.clone(), span, signed: true });
                    t
                }
                UnOp::Ref | UnOp::RefMut => {
                    let inner_expected = expected.map(|t| self.deref(t));
                    let t = self.check_opt(operand, inner_expected.as_ref());
                    mk(TyKind::Ref(op == UnOp::RefMut, t))
                }
            },
            ExprKind::Binary { op, lhs, rhs } => self.binary(e, op, lhs, rhs, span, expected),
            ExprKind::Tuple(items) => {
                let exp_items = expected.map(|t| self.icx.shallow(t)).and_then(|t| match &*t {
                    TyKind::Tuple(ts) if ts.len() == items.len() => Some(ts.clone()),
                    _ => None,
                });
                let ts = items
                    .iter()
                    .enumerate()
                    .map(|(i, x)| self.check_opt(*x, exp_items.as_ref().map(|v| &v[i])))
                    .collect();
                mk(TyKind::Tuple(ts))
            }
            ExprKind::List(items) => self.list_lit(e, &items, span, expected),
            ExprKind::Record { spread, fields } => self.record_lit(e, spread, &fields, span, expected),
            ExprKind::Map(entries) => {
                let (k, v) = (self.icx.fresh(), self.icx.fresh());
                for (ke, ve) in entries {
                    self.check(ke, &k);
                    self.check(ve, &v);
                }
                self.pending.push(Constraint::Impl {
                    proto: self.b().hash,
                    args: vec![],
                    ty: k.clone(),
                    span,
                    expr: None,
                });
                self.con(self.b().map, vec![k, v])
            }
            ExprKind::Set(items) => {
                let t = self.icx.fresh();
                for x in items {
                    self.check(x, &t);
                }
                self.pending.push(Constraint::Impl {
                    proto: self.b().hash,
                    args: vec![],
                    ty: t.clone(),
                    span,
                    expr: None,
                });
                self.con(self.b().set, vec![t])
            }
            ExprKind::Lambda(l) => self.lambda(e, &l, span, expected),
            ExprKind::If { cond, then, els } => {
                let b = self.bool_ty();
                self.check(cond, &b);
                match els {
                    Some(els) => {
                        let t = match expected {
                            Some(x) => x.clone(),
                            None => self.icx.fresh(),
                        };
                        self.check_branch(then, &t);
                        self.check_branch(els, &t);
                        t
                    }
                    None => {
                        // without `else`, the branch's value is discarded
                        let tt = self.infer(then);
                        self.discarded(tt, then);
                        unit()
                    }
                }
            }
            ExprKind::Match { scrutinee, arms } => self.match_expr(e, scrutinee, &arms, span, expected),
            ExprKind::While { cond, body } => {
                let b = self.bool_ty();
                self.check(cond, &b);
                self.loops.insert(e, (unit(), false));
                let bt = self.infer(body);
                self.discarded(bt, body);
                unit()
            }
            ExprKind::For { pat, iter, body } => {
                let it = self.infer(iter);
                let item = self.icx.fresh();
                let ispan = hir.exprs[iter].span;
                self.pending.push(Constraint::Iter {
                    coll: it,
                    item: item.clone(),
                    span: ispan,
                    expr: Some(e),
                });
                self.solve_some();
                self.check_pat(pat, &item, true);
                self.loops.insert(e, (unit(), false));
                let bt = self.infer(body);
                self.discarded(bt, body);
                unit()
            }
            ExprKind::Loop { body } => {
                let t = match expected {
                    Some(x) => x.clone(),
                    None => self.icx.fresh(),
                };
                self.loops.insert(e, (t.clone(), false));
                let bt = self.infer(body);
                self.discarded(bt, body);
                let (t, broke) = self.loops[&e].clone();
                if !broke {
                    // never terminates normally
                    return self.icx.fresh();
                }
                t
            }
            ExprKind::Block { stmts, tail } => {
                for s in &stmts {
                    self.stmt(s);
                }
                match tail {
                    // a record literal in final position may encode to an
                    // expected `Data`
                    Some(t)
                        if matches!(hir.exprs[t].kind, ExprKind::Record { .. })
                            && expected.is_some_and(
                                |x| matches!(&*self.deref(x), TyKind::Con(d, _) if *d == self.b().data),
                            ) =>
                    {
                        self.check(t, expected.unwrap())
                    }
                    Some(t) => self.check_opt(t, expected),
                    None => {
                        // a block ending in a diverging statement has any type
                        if let Some(Stmt::Expr { expr, .. }) = stmts.last()
                            && diverges(hir, *expr)
                        {
                            return self.icx.fresh();
                        }
                        unit()
                    }
                }
            }
            ExprKind::Return { value, target } => {
                let ret = self.fns.iter().rev().find(|f| f.owner == target).map(|f| f.ret.clone());
                let ret = ret.unwrap_or_else(|| self.icx.fresh());
                match value {
                    Some(v) => {
                        self.check(v, &ret);
                    }
                    None => self.expect(span, &ret, &unit()),
                }
                self.icx.fresh()
            }
            ExprKind::Break { value, target } => {
                let (t, _) = self.loops.get(&target).cloned().unwrap_or((unit(), false));
                match value {
                    Some(v) => {
                        self.check(v, &t);
                    }
                    None => self.expect(span, &t, &unit()),
                }
                if let Some(l) = self.loops.get_mut(&target) {
                    l.1 = true;
                }
                self.icx.fresh()
            }
            ExprKind::Continue { .. } => self.icx.fresh(),
            ExprKind::Unsafe(b) => {
                let foreign = self.intern_existing("Foreign");
                if let Some(f) = foreign
                    && !self.fns.is_empty()
                {
                    self.add_effect_label(f, unit(), span);
                }
                self.unsafe_depth += 1;
                let t = self.check_opt(b, expected);
                self.unsafe_depth -= 1;
                t
            }
            ExprKind::Error => error_ty(),
        }
    }

    fn check_branch(&mut self, e: ExprId, expected: &Ty) {
        self.check(e, expected);
    }

    fn lit(&mut self, l: &Lit, span: Span, expected: Option<&Ty>) -> Ty {
        match l {
            Lit::Int { value, suffix } => {
                if let Some(s) = suffix {
                    match self.suffix_ty(*s, span) {
                        Some(t) => {
                            self.pending.push(Constraint::IntLit { ty: t.clone(), value: *value, span });
                            return t;
                        }
                        None => return error_ty(),
                    }
                }
                let t = match expected.map(|t| self.deref(t)) {
                    Some(t) if matches!(&*t, TyKind::Con(d, _) if self.b().is_numeric(*d)) => t,
                    // a literal coerced to `dyn P` defaults to its natural type
                    Some(t) if matches!(&*t, TyKind::Dyn(..)) => self.int_ty(),
                    _ => self.icx.fresh(),
                };
                self.pending.push(Constraint::IntLit { ty: t.clone(), value: *value, span });
                t
            }
            Lit::Float { suffix, .. } => {
                if let Some(s) = suffix {
                    return self.suffix_ty(*s, span).unwrap_or_else(error_ty);
                }
                let t = match expected.map(|t| self.deref(t)) {
                    Some(t) if matches!(&*t, TyKind::Con(d, _) if self.b().is_float(*d)) => t,
                    Some(t) if matches!(&*t, TyKind::Dyn(..)) => self.prim(self.b().f64_),
                    _ => self.icx.fresh(),
                };
                self.pending.push(Constraint::FloatLit { ty: t.clone(), span });
                t
            }
            Lit::Duration(_) => self.prim(self.b().duration),
            Lit::Str(_) => self.string_ty(),
            Lit::Char(_) => self.prim(self.b().char_),
            Lit::Symbol(_) => self.prim(self.b().symbol),
            Lit::Bool(_) => self.bool_ty(),
            Lit::Unit => unit(),
        }
    }

    fn suffix_ty(&mut self, s: Symbol, span: Span) -> Option<Ty> {
        let name = self.sym(s).to_string();
        match self.g.hir.try_lang(&name) {
            Some(d) if self.b().is_numeric(d) => Some(self.prim(d)),
            _ => {
                self.error(
                    "E0307",
                    span,
                    format!("unknown literal suffix `{name}`"),
                    "expected I8..I64, U8..U64, F32 or F64",
                );
                None
            }
        }
    }

    fn ctor(
        &mut self,
        enum_def: DefId,
        variant: u32,
        type_args: &[HTy],
        span: Span,
        e: ExprId,
        expected: Option<&Ty>,
    ) -> Ty {
        let info = &self.g.env.enums[&enum_def];
        let n = info.n_generics as usize;
        let mut args: Vec<Ty> = type_args.iter().map(|t| self.hty(t)).collect();
        if args.len() > n {
            self.error("E0210", span, "too many type arguments", "here");
            args.truncate(n);
        }
        while args.len() < n {
            args.push(self.icx.fresh());
        }
        let fields = self.g.env.variant_fields(enum_def, variant, &args);
        let result = self.con(enum_def, args.clone());
        self.record_res(e, Res::Ctor { enum_def, variant, args });
        if fields.is_empty() {
            if let Some(x) = expected {
                let _ = self.icx.unify(x, &result);
            }
            result
        } else {
            let eff = row_empty();

            open_effects(
                &mut self.icx,
                &mk(TyKind::Fn(FnTy {
                    params: fields.into_iter().map(|f| f.1).collect(),
                    ret: result,
                    eff,
                    once: false,
                })),
            )
        }
    }

    pub fn field(&mut self, bt: Ty, label: Symbol, span: Span, e: ExprId) -> Ty {
        let bt = self.deref(&bt);
        match &*bt {
            TyKind::Con(d, args) if self.g.env.structs.contains_key(d) => {
                let fields = self.g.env.struct_fields(*d, args);
                match fields.iter().find(|f| f.0 == label) {
                    Some((_, t, is_pub)) => {
                        let module = self.g.env.structs[d].module;
                        if !is_pub && Some(module) != self.module {
                            let l = self.label_str(label);
                            self.error(
                                "E0308",
                                span,
                                format!("field `{l}` is private"),
                                "declared without `pub`",
                            );
                        }
                        self.record_res(e, Res::Field(label));
                        t.clone()
                    }
                    None => {
                        let (l, ty) = (self.label_str(label), self.show(&bt));
                        self.error("E0301", span, format!("no field `{l}` on `{ty}`"), "unknown field");
                        error_ty()
                    }
                }
            }
            TyKind::Tuple(ts) => {
                let idx: Option<usize> = self.sym(label).parse().ok();
                match idx.and_then(|i| ts.get(i)) {
                    Some(t) => {
                        self.record_res(e, Res::Field(label));
                        t.clone()
                    }
                    None => {
                        let ty = self.show(&bt);
                        self.error(
                            "E0301",
                            span,
                            format!("no field `{}` on `{ty}`", self.label_str(label)),
                            "unknown field",
                        );
                        error_ty()
                    }
                }
            }
            TyKind::Record(_) | TyKind::Var(_) => {
                let t = self.icx.fresh();
                let rest = self.icx.fresh_row();
                let want = record([(label, t.clone())], rest);
                if let Err(err) = self.icx.unify(&want, &bt) {
                    self.report_unify(err, span, &want, &bt);
                    return error_ty();
                }
                self.record_res(e, Res::Field(label));
                t
            }
            TyKind::Error => error_ty(),
            _ => {
                let (l, ty) = (self.label_str(label), self.show(&bt));
                self.error("E0301", span, format!("no field `{l}` on `{ty}`"), "not a record");
                error_ty()
            }
        }
    }

    fn call(&mut self, e: ExprId, callee: ExprId, args: &[ExprId], span: Span, expected: Option<&Ty>) -> Ty {
        // typed record operations with label/shape-dependent results
        if let ExprKind::Def { def, .. } = &self.g.hir.exprs[callee].kind {
            let def = *def;
            if Some(def) == self.g.hir.try_lang("records.remove") && args.len() == 2 {
                let ft = self.infer(callee);
                let rt = self.infer(args[0]);
                let label = match &self.g.hir.exprs[args[1]].kind {
                    ExprKind::Lit(Lit::Symbol(l)) => Some(*l),
                    _ => None,
                };
                self.set_expr(args[1], self.prim(self.b().symbol));
                let Some(label) = label else {
                    self.error(
                        "E0344",
                        self.g.hir.exprs[args[1]].span,
                        "`records.remove` needs a literal label",
                        "e.g. `:name` or `:ns/name`",
                    );
                    return error_ty();
                };
                // the label must exist; the result keeps every other field
                let present = self.icx.fresh();
                let rest = self.icx.fresh_row();
                let want = record([(label, present)], rest.clone());
                let aspan = self.g.hir.exprs[args[0]].span;
                self.expect(aspan, &want, &rt);
                let out = record(std::iter::empty(), rest);
                if let TyKind::Fn(f) = &*self.icx.shallow(&ft) {
                    let (p0, ret) = (f.params[0].clone(), f.ret.clone());
                    let _ = self.icx.unify(&p0, &rt);
                    let _ = self.icx.unify(&ret, &out);
                }
                return out;
            }
            if Some(def) == self.g.hir.try_lang("records.merge") && args.len() == 2 {
                let ft = self.infer(callee);
                let lt = self.infer(args[0]);
                let rt = self.infer(args[1]);
                let out = self.icx.fresh();
                self.pending.push(Constraint::Merge {
                    left: lt.clone(),
                    right: rt.clone(),
                    out: out.clone(),
                    span,
                });
                if let TyKind::Fn(f) = &*self.icx.shallow(&ft) {
                    let (p0, p1, ret) = (f.params[0].clone(), f.params[1].clone(), f.ret.clone());
                    let _ = self.icx.unify(&p0, &lt);
                    let _ = self.icx.unify(&p1, &rt);
                    let _ = self.icx.unify(&ret, &out);
                }
                self.solve_some();
                return out;
            }
        }
        let ft = self.infer(callee);
        let ft = self.icx.shallow(&ft);
        let cspan = self.g.hir.exprs[callee].span;
        self.apply(e, ft, args, span, cspan, expected)
    }

    /// Apply a function type to argument expressions (bidirectionally).
    pub fn apply(
        &mut self,
        _e: ExprId,
        ft: Ty,
        args: &[ExprId],
        span: Span,
        cspan: Span,
        expected: Option<&Ty>,
    ) -> Ty {
        let ft = self.deref(&ft);
        // calling a `for<S>`-typed value: any brand may be chosen by the caller
        let ft = match &*ft {
            TyKind::Forall(n, body) => {
                let vars: Vec<Ty> = (0..*n).map(|_| self.icx.fresh()).collect();
                subst_bound(body, &vars)
            }
            _ => ft,
        };
        // argument diagnostics point at the called declaration
        let callee = match &self.g.hir.exprs[_e].kind {
            ExprKind::Call { callee, .. } => match &self.g.hir.exprs[*callee].kind {
                ExprKind::Def { def, .. } if !self.g.hir.modules[self.g.hir.defs[*def].module].is_std => {
                    Some(*def)
                }
                _ => None,
            },
            _ => None,
        };
        match &*ft {
            TyKind::Fn(f) => {
                if f.params.len() != args.len() {
                    self.error_rel(
                        "E0309",
                        span,
                        format!(
                            "this function takes {} argument(s) but {} were supplied",
                            f.params.len(),
                            args.len()
                        ),
                        "wrong number of arguments",
                        callee,
                    );
                    for a in args {
                        self.infer(*a);
                    }
                    return f.ret.clone();
                }
                if let Some(x) = expected {
                    // propagate the expected result into generic arguments
                    let snapshot_ok = self.try_unify(&f.ret, x);
                    let _ = snapshot_ok;
                }
                let saved = self.callee_ctx.take();
                self.callee_ctx = callee;
                for (a, p) in args.iter().zip(&f.params) {
                    self.check_arg(*a, p);
                }
                self.callee_ctx = saved;
                self.add_effects(&f.eff, span);
                // callback effects propagate to the caller: functions passed
                // in (directly or inside records/tuples) may be called
                for p in &f.params {
                    self.add_callback_effects(p, span);
                }
                f.ret.clone()
            }
            TyKind::Var(_) => {
                let params: Vec<Ty> = args.iter().map(|a| self.infer(*a)).collect();
                let ret = self.icx.fresh();
                let f = self.fn_ty(params, ret.clone());
                self.expect(cspan, &ft, &f);
                if let TyKind::Fn(fx) = &*f {
                    self.add_effects(&fx.eff.clone(), span);
                }
                ret
            }
            TyKind::Error => {
                for a in args {
                    self.infer(*a);
                }
                error_ty()
            }
            _ => {
                let t = self.show(&ft);
                self.error("E0310", cspan, format!("`{t}` is not a function"), "called here");
                for a in args {
                    self.infer(*a);
                }
                error_ty()
            }
        }
    }

    /// Unify in a snapshot; keep the result only if it succeeds.
    pub fn try_unify(&mut self, a: &Ty, b: &Ty) -> bool {
        self.icx.try_unify(a, b)
    }

    /// Check an argument against a parameter type, opening `for<S>` scope
    /// binders with fresh region skolems.
    pub fn check_arg(&mut self, a: ExprId, p: &Ty) {
        let p = self.icx.shallow(p);
        let span = self.g.hir.exprs[a].span;
        if let TyKind::Forall(n, body) = &*p {
            self.icx.level += 1;
            let names: Vec<Ty> = (0..*n)
                .map(|_| {
                    // scope brands are all shown as `S`
                    let sym = self.g.interner.get("S").unwrap_or(lang_base::Symbol::dummy());
                    self.icx.fresh_skolem(sym, true)
                })
                .collect();
            let opened = subst_bound(body, &names);
            let ids: Vec<u32> = names
                .iter()
                .filter_map(|t| if let TyKind::Skolem(k) = &**t { Some(*k) } else { None })
                .collect();
            self.region_lambdas.push((a, ids));
            self.check(a, &opened);
            self.icx.level -= 1;
            return;
        }
        let t = self.check_opt(a, Some(&p));
        if self.try_dyn(a, &t, &p) || self.try_view(a, &t, &p) {
            return;
        }
        if matches!(self.g.hir.exprs[a].kind, ExprKind::Record { .. }) && self.try_data(a, &t, &p) {
            return;
        }
        if let Err(err) = self.icx.unify(&p, &t) {
            // allow a closed record to coerce into a transparent struct
            if !self.try_coerce(a, &t, &p) {
                self.report_unify(err, span, &p, &t);
            }
        }
    }

    /// Record → struct coercion for an existing exact-shape record value.
    fn try_coerce(&mut self, e: ExprId, actual: &Ty, expected: &Ty) -> bool {
        let exp = self.deref(expected);
        let act = self.deref(actual);
        let TyKind::Con(d, args) = &*exp else { return false };
        if !self.g.env.structs.contains_key(d) {
            return false;
        }
        // a `repr c` struct is a foreign layout, not equivalent to a record
        if matches!(&self.g.hir.defs[*d].kind, DefKind::Struct(sd) if sd.repr_c) {
            return false;
        }
        let TyKind::Record(r) = &*act else { return false };
        let (fields, tail) = self.icx.flatten_row(r);
        if !matches!(&*tail, TyKind::RowEmpty) {
            return false;
        }
        let sfields = self.g.env.struct_fields(*d, args);
        if sfields.len() != fields.len() {
            return false;
        }
        for (l, t, is_pub) in &sfields {
            let Some((_, ft)) = fields.iter().find(|f| f.0 == *l) else { return false };
            if !self.try_unify(t, ft) {
                return false;
            }
            if !is_pub && Some(self.g.env.structs[d].module) != self.module {
                return false;
            }
        }
        self.coercions.insert(e, Coercion::Struct);
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn method_call(
        &mut self,
        e: ExprId,
        recv: ExprId,
        method: Symbol,
        method_span: Span,
        type_args: &[HTy],
        args: &[ExprId],
        span: Span,
        expected: Option<&Ty>,
    ) -> Ty {
        let mut rt = self.infer(recv);
        let explicit: Vec<Ty> = type_args.iter().map(|t| self.hty(t)).collect();
        let mut r = self.deref(&rt);
        // an integer range offers the `Seq<Int>` methods it lacks
        if let TyKind::Con(d, a) = &*r.clone()
            && self.lang_name(*d).as_deref() == Some("Range")
            && !self.lookup_method_peek(&r, method)
            && let Some(seq) = self.g.hir.try_lang("Seq")
        {
            let int = self.int_ty();
            if let Some(elem) = a.first() {
                let _ = self.icx.unify(elem, &int);
            }
            self.coercions.insert(recv, Coercion::RangeToSeq);
            rt = mk(TyKind::Con(seq, vec![int]));
            r = rt.clone();
        }
        // Record field holding a function: call it.
        if let TyKind::Record(_) = &*r {
            let ft = self.field(r.clone(), method, method_span, e);
            self.record_res(e, Res::Method(Target::Field(method)));
            return self.apply(e, ft, args, span, method_span, expected);
        }
        // Struct field holding a function (when no method of that name exists).
        if let TyKind::Con(d, sargs) = &*r
            && self.g.env.structs.contains_key(d)
        {
            // the field's type at this instance (`v: T` with `T = fn(...)`)
            let field_ty =
                self.g.env.struct_fields(*d, sargs).into_iter().find(|f| f.0 == method).map(|f| f.1);
            let has_fn_field = match field_ty {
                Some(t) => {
                    let t = self.icx.resolve(&t);
                    matches!(&*self.deref(&t), TyKind::Fn(_))
                }
                None => false,
            };
            if has_fn_field && !self.lookup_method_peek(&r, method) {
                let ft = self.field(r.clone(), method, method_span, e);
                self.record_res(e, Res::Method(Target::Field(method)));
                return self.apply(e, ft, args, span, method_span, expected);
            }
        }
        // a cell reached through a parameter's fields or elements (or bound
        // from one by a pattern) is the caller's too
        let recv_ty = self.icx.resolve(&rt);
        let recv_ty = self.deref(&recv_ty);
        let user_code = self.module.is_some_and(|m| !self.g.hir.modules[m].is_std);
        if user_code
            && !matches!(&self.g.hir.exprs[recv].kind, ExprKind::Local(_))
            && matches!(self.sym(method), "set" | "get")
            && self.param_rooted(recv)
            && matches!(&*recv_ty, TyKind::Con(d, _) if *d == self.b().cell)
            && let Some(st) = self.intern_existing("State")
        {
            let eff = row([(st, unit())], row_empty());
            self.add_effects(&eff, span);
        }
        // mutating a cell reached through a borrow is state observable
        // outside this computation
        if let ExprKind::Local(l) = &self.g.hir.exprs[recv].kind
            && let Some(lt) = self.locals.get(l).cloned()
        {
            let lt = self.icx.resolve(&lt);
            let cell = self.b().cell;
            let is_cell = |t: &Ty| matches!(&**t, TyKind::Con(d, _) if *d == cell);
            // a cell reached through a borrow or passed in as a parameter is
            // shared with the caller: reading or writing it is observable
            let shared = match &*lt {
                TyKind::Ref(_, inner) => is_cell(inner),
                _ => is_cell(&lt) && (self.is_param(*l) || self.param_derived.contains(l)),
            };
            if shared
                && matches!(self.sym(method), "set" | "get")
                && let Some(st) = self.intern_existing("State")
            {
                let eff = row([(st, unit())], row_empty());
                self.add_effects(&eff, span);
            }
        }
        let ret = self.icx.fresh();
        // the expected result selects among several protocol implementations
        // (`Conv<Int>` / `Conv<String>`); still ambiguous -> decided later
        let mut probe_params = vec![rt.clone()];
        probe_params.extend(args.iter().map(|_| self.icx.fresh()));
        let probe_ret = expected.cloned().unwrap_or_else(|| self.icx.fresh());
        let probe = self.fn_ty(probe_params, probe_ret);
        match self.lookup_method_typed(&r, method, &explicit, method_span, Some(&probe), false) {
            Lookup::Found(target, fty) => {
                self.record_res(e, Res::Method(target));
                // first parameter is the receiver (auto-borrowed)
                let fty = self.icx.shallow(&fty);
                let TyKind::Fn(f) = &*fty else { return error_ty() };
                if f.params.is_empty() {
                    let m = self.sym(method).to_string();
                    self.error(
                        "E0311",
                        method_span,
                        format!("`{m}` is not a method"),
                        "associated function without `self`",
                    );
                    return error_ty();
                }
                self.expect(self.g.hir.exprs[recv].span, &f.params[0], &rt);
                let rest = mk(TyKind::Fn(FnTy {
                    params: f.params[1..].to_vec(),
                    ret: f.ret.clone(),
                    eff: f.eff.clone(),
                    once: f.once,
                }));
                self.apply(e, rest, args, span, method_span, expected)
            }
            Lookup::NotFound => {
                let (m, t) = (self.sym(method).to_string(), self.show(&r));
                let rel = self.head_def(&r);
                self.error_rel(
                    "E0312",
                    method_span,
                    format!("no method `{m}` on `{t}`"),
                    "method not found",
                    rel,
                );
                for a in args {
                    self.infer(*a);
                }
                error_ty()
            }
            Lookup::Unknown => {
                // receiver type not known yet: defer
                let params: Vec<Ty> = args.iter().map(|a| self.infer(*a)).collect();
                let mut all = vec![rt.clone()];
                all.extend(params);
                let fty = self.fn_ty(all, ret.clone());
                if let TyKind::Fn(f) = &*fty {
                    let eff = f.eff.clone();
                    self.add_effects(&eff, span);
                }
                self.pending.push(Constraint::Method {
                    recv: rt,
                    name: method,
                    fn_ty: fty,
                    explicit,
                    span: method_span,
                    expr: Some(e),
                });
                ret
            }
        }
    }

    /// Does a method named `name` exist for this nominal receiver?
    fn lookup_method_peek(&mut self, r: &Ty, name: Symbol) -> bool {
        let TyKind::Con(head, _) = &**r else { return false };
        self.g.env.impls_by_head.get(head).is_some_and(|idxs| {
            idxs.iter().any(|&i| match &self.g.env.impls[i].methods {
                ImplMethods::Defs(ms) => ms.contains_key(&name),
                ImplMethods::Derived(_) => self.g.env.impls[i].protocol.as_ref().is_some_and(|(p, _)| {
                    self.g.env.protocols.get(p).is_some_and(|pi| pi.methods.contains_key(&name))
                }),
            })
        })
    }

    fn try_expr(&mut self, e: ExprId, inner: ExprId, span: Span) -> Ty {
        let t = self.infer(inner);
        let (ok_t, err_t) = (self.icx.fresh(), self.icx.fresh());
        let want = self.result_ty(ok_t.clone(), err_t.clone());
        let it = self.deref(&t);
        if self.icx.unify(&want, &it).is_err() {
            let ty = self.show(&t);
            let mut d = Diagnostic::error("`?` requires a `Result`")
                .with_code("E0313")
                .with_label(span, format!("this has type `{ty}`"));
            if matches!(&*self.icx.resolve(&it), TyKind::Con(d, _) if *d == self.b().option) {
                d = d
                    .with_note("`Option` must be matched or converted explicitly with a missing-value error");
            }
            self.push_diag(d);
            return error_ty();
        }
        // enclosing function must return Result<_, E2>
        let Some(f) = self.fns.last() else {
            self.error("E0314", span, "`?` outside of a function", "here");
            return ok_t;
        };
        let ret = f.ret.clone();
        let (r_ok, r_err) = (self.icx.fresh(), self.icx.fresh());
        let want_ret = self.result_ty(r_ok, r_err.clone());
        if self.icx.unify(&want_ret, &ret).is_err() {
            let rt = self.show(&ret);
            self.error(
                "E0314",
                span,
                "`?` requires the enclosing function to return `Result`",
                format!("the function returns `{rt}`"),
            );
            return ok_t;
        }
        self.pending.push(Constraint::ErrConv { from: err_t, to: r_err, span, expr: Some(e) });
        ok_t
    }

    fn binary(
        &mut self,
        e: ExprId,
        op: BinOp,
        lhs: ExprId,
        rhs: ExprId,
        span: Span,
        expected: Option<&Ty>,
    ) -> Ty {
        match op {
            BinOp::And | BinOp::Or => {
                let b = self.bool_ty();
                self.check(lhs, &b);
                self.check(rhs, &b);
                b
            }
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let lt = self.infer(lhs);
                self.check(rhs, &lt.clone());
                let ordered = !matches!(op, BinOp::Eq | BinOp::Ne);
                self.pending.push(Constraint::Compare { ty: lt.clone(), ordered, span, expr: Some(e) });
                self.bool_ty()
            }
            BinOp::Range | BinOp::RangeIncl => {
                let lt = self.infer(lhs);
                self.check(rhs, &lt.clone());
                self.pending.push(Constraint::Numeric { ty: lt.clone(), span, signed: false });
                self.pending.push(Constraint::IntOnly { ty: lt.clone(), span });
                self.con(self.b().range, vec![lt])
            }
            BinOp::Pipe => unreachable!("pipes are desugared"),
            _ => {
                let lt = self.check_opt(lhs, expected);
                let rt = self.infer(rhs);
                let out = self.icx.fresh();
                self.pending.push(Constraint::Arith {
                    op,
                    lhs: lt,
                    rhs: rt,
                    out: out.clone(),
                    span,
                    expr: Some(e),
                });
                self.solve_some();
                out
            }
        }
    }

    fn list_lit(&mut self, _e: ExprId, items: &[ExprId], _span: Span, expected: Option<&Ty>) -> Ty {
        let exp = expected.map(|t| self.deref(t));
        let (head, elem) = match exp.as_deref() {
            Some(TyKind::Con(d, args)) if args.len() == 1 && self.is_list_like(*d) => (*d, args[0].clone()),
            _ => (self.b().list, self.icx.fresh()),
        };
        for x in items {
            self.check(*x, &elem);
        }
        self.con(head, vec![elem])
    }

    fn is_list_like(&self, d: DefId) -> bool {
        d == self.b().list
            || Some(d) == self.g.hir.try_lang("Array")
            || Some(d) == self.g.hir.try_lang("Buffer")
    }

    fn record_lit(
        &mut self,
        e: ExprId,
        spread: Option<ExprId>,
        fields: &[(Symbol, Span, ExprId)],
        span: Span,
        expected: Option<&Ty>,
    ) -> Ty {
        let exp = expected.map(|t| self.deref(t));
        // Contextual struct construction.
        if let (None, Some(t)) = (spread, &exp)
            && let TyKind::Con(d, args) = &**t
        {
            if *d == self.b().map && args.len() == 2 {
                // labels become symbol keys; every field must fit `V`
                let sym = self.prim(self.b().symbol);
                self.expect(span, &args[0], &sym);
                let v = args[1].clone();
                let is_data = matches!(&*self.deref(&v), TyKind::Con(d, _) if *d == self.b().data);
                for (_, _, x) in fields {
                    if is_data {
                        let t = self.infer(*x);
                        if !self.try_data(*x, &t, &v) {
                            let xs = self.g.hir.exprs[*x].span;
                            self.expect(xs, &v, &t);
                        }
                    } else {
                        self.check(*x, &v);
                    }
                }
                return t.clone();
            }
            if self.g.env.structs.contains_key(d) {
                return self.struct_lit(e, *d, args.clone(), fields, span);
            }
        }
        let expected_fields: FxHashMap<Symbol, Ty> = match exp.as_deref() {
            Some(TyKind::Record(r)) => {
                let r = r.clone();
                self.icx.flatten_row(&r).0.into_iter().collect()
            }
            _ => FxHashMap::default(),
        };
        let mut fs = vec![];
        for (l, _, x) in fields {
            let t = self.check_opt(*x, expected_fields.get(l));
            fs.push((*l, t));
        }
        match spread {
            None => record(fs, row_empty()),
            Some(s) => {
                let st = self.infer(s);
                let st = self.deref(&st);
                if let TyKind::Con(d, args) = &*st
                    && self.g.env.structs.contains_key(d)
                {
                    // struct update keeps the nominal type; fields keep their types
                    let sf = self.g.env.struct_fields(*d, args);
                    let outside = Some(self.g.env.structs[d].module) != self.module;
                    for ((l, t), (_, lspan, _)) in fs.iter().zip(fields) {
                        match sf.iter().find(|f| f.0 == *l) {
                            Some((_, ft, is_pub)) => {
                                if outside && !is_pub {
                                    let ls = self.label_str(*l);
                                    self.error(
                                        "E0308",
                                        *lspan,
                                        format!("field `{ls}` is private"),
                                        "an update outside the defining module may change only `pub` fields",
                                    );
                                }
                                self.expect(*lspan, ft, t)
                            }
                            None => {
                                let ls = self.label_str(*l);
                                self.error(
                                    "E0301",
                                    *lspan,
                                    format!("struct has no field `{ls}`"),
                                    "unknown field",
                                );
                            }
                        }
                    }
                    return st;
                }
                let tail = self.icx.fresh_row();
                let labels: Vec<Symbol> = fs.iter().map(|f| f.0).collect();
                let base_row = self.icx.fresh_row();
                let want = mk(TyKind::Record(base_row.clone()));
                let sspan = self.g.hir.exprs[s].span;
                self.expect(sspan, &want, &st);
                self.pending.push(Constraint::RowRemove { out: tail.clone(), base: base_row, labels, span });
                self.solve_some();
                record(fs, tail)
            }
        }
    }

    fn struct_lit(
        &mut self,
        _e: ExprId,
        d: DefId,
        args: Vec<Ty>,
        fields: &[(Symbol, Span, ExprId)],
        span: Span,
    ) -> Ty {
        let sf = self.g.env.struct_fields(d, &args);
        let module = self.g.env.structs[&d].module;
        let outside = Some(module) != self.module;
        if outside && sf.iter().any(|f| !f.2) {
            let n = self.sym(self.g.hir.defs[d].name).to_string();
            self.push_diag(
                Diagnostic::error(format!("cannot construct `{n}` outside its defining module"))
                    .with_code("E0315")
                    .with_label(span, "struct with private fields")
                    .with_note("use the module's constructor function"),
            );
        }
        for (l, lspan, x) in fields {
            match sf.iter().find(|f| f.0 == *l) {
                Some((_, ft, _)) => {
                    self.check(*x, ft);
                    let _ = lspan;
                }
                None => {
                    let ls = self.label_str(*l);
                    self.error("E0301", *lspan, format!("struct has no field `{ls}`"), "unknown field");
                    self.infer(*x);
                }
            }
        }
        for (l, _, _) in &sf {
            if !fields.iter().any(|f| f.0 == *l) {
                let ls = self.label_str(*l);
                self.error("E0316", span, format!("missing field `{ls}`"), "all struct fields must be given");
            }
        }
        self.con(d, args)
    }

    fn lambda(&mut self, e: ExprId, l: &lang_hir::LambdaExpr, span: Span, expected: Option<&Ty>) -> Ty {
        let exp = expected.map(|t| self.icx.shallow(t));
        let exp_fn = match exp.as_deref() {
            Some(TyKind::Fn(f)) if f.params.len() == l.params.len() => Some(f.clone()),
            _ => None,
        };
        let mut params = vec![];
        for (i, p) in l.params.iter().enumerate() {
            let t = match (&p.ty, &exp_fn) {
                (Some(h), Some(f)) => {
                    let t = self.hty(h);
                    let _ = self.icx.unify(&t, &f.params[i]);
                    t
                }
                (Some(h), None) => self.hty(h),
                (None, Some(f)) => f.params[i].clone(),
                (None, None) => self.icx.fresh(),
            };
            self.check_pat(p.pat, &t, true);
            params.push(t);
        }
        if let Some(first) = params.first() {
            let first = self.icx.shallow(first);
            if matches!(&*first, TyKind::Con(d, _) if Some(*d) == self.g.hir.try_lang("system.InitScope")) {
                self.init_lambdas.push(e);
            }
        }
        let ret = match (&l.ret, &exp_fn) {
            (Some(h), Some(f)) => {
                let t = self.hty(h);
                let _ = self.icx.unify(&t, &f.ret);
                t
            }
            (Some(h), None) => self.hty(h),
            (None, Some(f)) => f.ret.clone(),
            (None, None) => self.icx.fresh(),
        };
        let eff = self.icx.fresh_row();
        let bound = l.effect.as_ref().map(|ef| self.effect_bound(ef));
        self.fns.push(FnCtx {
            owner: Owner::Lambda(e),
            ret: ret.clone(),
            eff: eff.clone(),
            eff_sites: vec![],
            bound,
        });
        self.check(l.body, &ret);
        // capturing scoped or task-confined values marks the closure type
        for c in &l.captures {
            if let Some(ct) = self.locals.get(c).cloned() {
                self.mark_capture(&ct, span);
            }
        }
        let ctx = self.fns.pop().unwrap();
        self.finish_effects(&ctx);
        let eff = match &ctx.bound {
            Some((labels, _)) => {
                let mut fields: Vec<(Symbol, Ty)> = labels.iter().map(|l| (*l, unit())).collect();
                // keep capture marks (regions, confinement) on annotated closures
                let (amb, _) = self.icx.flatten_row(&eff);
                for (l, t) in amb {
                    if self.sym(l).starts_with('%') {
                        fields.push((l, t));
                    }
                }
                row(fields, row_empty())
            }
            None => eff,
        };
        let once = l.once || exp_fn.as_ref().is_some_and(|f| f.once);
        let fty = mk(TyKind::Fn(FnTy { params, ret, eff, once }));
        if let Some(f) = &exp_fn {
            let ef = mk(TyKind::Fn(FnTy {
                params: f.params.clone(),
                ret: f.ret.clone(),
                eff: f.eff.clone(),
                once,
            }));
            let _ = ef;
        }
        fty
    }

    /// Mark the current (innermost) function's effect row with the regions
    /// and confinement of a captured value's type.
    fn mark_capture(&mut self, t: &Ty, span: Span) {
        let t = self.icx.resolve(t);
        let mut regions = vec![];
        let mut confined = false;
        let cell = self.b().cell;
        let tx = self.g.hir.try_lang("sqlite.Tx");
        let mut visit = |k: &TyKind| {
            match k {
                TyKind::Skolem(s) => regions.push(*s),
                TyKind::Con(d, _) if *d == cell || Some(*d) == tx => confined = true,
                TyKind::RowExt(l, _, _) => {
                    let _ = l;
                }
                _ => {}
            }
            false
        };
        ty_any(&t, &mut visit);
        // cells inside struct and enum fields confine their holders too
        confined |= self.holds_confined(&t, &mut vec![]);
        // confinement marks on captured closures propagate
        let (has_confined_label, region_labels) = self.closure_marks(&t);
        confined |= has_confined_label;
        for s in regions {
            if self.icx.skolems[s as usize].region
                && let Some(l) = self.region_label(s)
            {
                self.add_effect_label(l, mk(TyKind::Skolem(s)), span);
            }
        }
        for (l, p) in region_labels {
            self.add_effect_label(l, p, span);
        }
        if confined && let Some(l) = self.intern_existing("%confined") {
            self.add_effect_label(l, unit(), span);
        }
    }

    /// Whether a nominal type's fields (transitively) hold a `Cell` or a
    /// transaction.
    fn holds_confined(&mut self, t: &Ty, seen: &mut Vec<DefId>) -> bool {
        let t = self.icx.resolve(t);
        match &*t {
            TyKind::Con(d, args) => {
                if *d == self.b().cell || Some(*d) == self.g.hir.try_lang("sqlite.Tx") {
                    return true;
                }
                if seen.contains(d) {
                    return false;
                }
                seen.push(*d);
                let fields: Vec<Ty> = if let Some(e) = self.g.env.enums.get(d) {
                    (0..e.variants.len())
                        .flat_map(|v| self.g.env.variant_fields(*d, v as u32, args).into_iter().map(|f| f.1))
                        .collect()
                } else if self.g.env.structs.contains_key(d) {
                    self.g.env.struct_fields(*d, args).into_iter().map(|f| f.1).collect()
                } else {
                    args.clone()
                };
                let r = fields.iter().any(|f| self.holds_confined(f, seen));
                seen.pop();
                r
            }
            TyKind::Tuple(ts) => ts.clone().iter().any(|x| self.holds_confined(x, seen)),
            TyKind::Record(r) => row_fields(r).0.iter().any(|(_, x)| self.holds_confined(x, seen)),
            TyKind::Ref(_, x) => self.holds_confined(&x.clone(), seen),
            _ => false,
        }
    }

    fn closure_marks(&mut self, t: &Ty) -> (bool, Vec<(Symbol, Ty)>) {
        let mut confined = false;
        let mut regions = vec![];
        let mut stack = vec![t.clone()];
        while let Some(t) = stack.pop() {
            match &*t {
                TyKind::Fn(f) => {
                    let (labels, _) = row_fields(&f.eff);
                    for (l, p) in labels {
                        let s = self.sym(l);
                        if s == "%confined" {
                            confined = true;
                        } else if s.starts_with("%region") {
                            regions.push((l, p));
                        }
                    }
                }
                TyKind::Con(_, args) | TyKind::Tuple(args) => stack.extend(args.iter().cloned()),
                TyKind::Record(r) => {
                    let (fs, _) = row_fields(r);
                    stack.extend(fs.into_iter().map(|f| f.1));
                }
                _ => {}
            }
        }
        (confined, regions)
    }

    /// A `let` whose value may generalize: a plain binding of a closure,
    /// function reference, or constructor (no effects, no allocation of
    /// mutable state).
    fn generalizable(&mut self, pat: PatId, init: ExprId) -> bool {
        let hir = self.g.hir;
        if !matches!(hir.pats[pat].kind, PatKind::Bind(l) if !hir.locals[l].mutable) {
            return false;
        }
        match &hir.exprs[init].kind {
            // a closure over a scoped value (e.g. a task scope) belongs to
            // that region: it stays monomorphic
            ExprKind::Lambda(l) => {
                let caps: Vec<Ty> = l.captures.iter().filter_map(|c| self.locals.get(c).cloned()).collect();
                !l.once
                    && !caps.iter().any(|t| {
                        let t = self.icx.resolve(t);
                        crate::ty::ty_any(&t, &mut |k| matches!(k, TyKind::Skolem(_)))
                    })
            }
            ExprKind::Def { def, .. } => match hir.defs[*def].kind {
                DefKind::Fn(_) => true,
                // a generalized module value (the prelude's `none`)
                DefKind::Global(_) => self.g.scheme(*def).is_some_and(|s| s.n > 0),
                _ => false,
            },
            ExprKind::Ctor { .. } => true,
            // an empty list is an immutable value of any element type
            ExprKind::List(xs) => xs.is_empty(),
            // a constructor applied to generalizable values
            ExprKind::Call { callee, args } => {
                matches!(hir.exprs[*callee].kind, ExprKind::Ctor { .. })
                    && args
                        .iter()
                        .all(|a| matches!(hir.exprs[*a].kind, ExprKind::Lit(_) | ExprKind::Ctor { .. }))
            }
            _ => false,
        }
    }

    /// A loop body's value is discarded each iteration: must-use values
    /// (results, fallible task handles) in its final position are errors.
    fn discarded(&mut self, t: Ty, body: ExprId) {
        let span = match &self.g.hir.exprs[body].kind {
            ExprKind::Block { tail: Some(x), .. } => self.g.hir.exprs[*x].span,
            _ => self.g.hir.exprs[body].span,
        };
        self.pending.push(Constraint::MustUse { ty: t, span });
    }

    /// Modeled effects (`Io`, `Wait`, `State`, `Foreign`) of the functions
    /// carried by an argument type, added to the current function.
    /// Whether local `l` is a parameter of the function or lambda binding it.
    /// Whether a place (`p.c`, `l[0]`, `p`) is rooted in a parameter or a
    /// local bound from one.
    fn param_rooted(&self, mut e: ExprId) -> bool {
        let hir = self.g.hir;
        loop {
            match &hir.exprs[e].kind {
                ExprKind::Local(l) => return self.is_param(*l) || self.param_derived.contains(l),
                ExprKind::Field { base, .. } | ExprKind::Index { base, .. } => e = *base,
                ExprKind::Unary { operand, .. } => e = *operand,
                // what a parameter's method or callback returns may be the
                // caller's too (`m.get(k)`, `make()`)
                ExprKind::MethodCall { recv, .. } => e = *recv,
                ExprKind::Call { callee, .. } => e = *callee,
                ExprKind::Try(x) => e = *x,
                _ => return false,
            }
        }
    }

    /// Record the locals a pattern binds from a parameter-rooted value.
    fn note_param_derived(&mut self, source: ExprId, pat: lang_hir::PatId) {
        if self.param_rooted(source) {
            let mut ls = vec![];
            crate::regions::collect(self.g.hir, pat, &mut ls);
            self.param_derived.extend(ls);
        }
    }

    fn is_param(&self, l: LocalId) -> bool {
        let hir = self.g.hir;
        let params: &[lang_hir::Param] = match hir.locals[l].owner {
            Some(lang_hir::Owner::Fn(d)) => match &hir.defs[d].kind {
                lang_hir::DefKind::Fn(f) => &f.params,
                _ => return false,
            },
            Some(lang_hir::Owner::Lambda(e)) => match &hir.exprs[e].kind {
                ExprKind::Lambda(lx) => &lx.params,
                _ => return false,
            },
            _ => return false,
        };
        params.iter().any(|p| matches!(hir.pats[p.pat].kind, lang_hir::PatKind::Bind(x) if x == l))
    }

    fn add_callback_effects(&mut self, p: &Ty, span: Span) {
        // a module value stores functions (e.g. providers) without calling them
        if self.fns.last().is_some_and(|f| matches!(f.owner, lang_hir::Owner::Global(_))) {
            return;
        }
        let p = self.icx.resolve(p);
        let mut labels: Vec<Symbol> = vec![];
        let mut stack = vec![p];
        let mut seen = 0;
        while let Some(t) = stack.pop() {
            seen += 1;
            if seen > 64 {
                break;
            }
            match &*t {
                TyKind::Fn(f) => {
                    for (l, _) in row_fields(&f.eff).0 {
                        if matches!(self.sym(l), "Io" | "Wait" | "State" | "Foreign") && !labels.contains(&l)
                        {
                            labels.push(l);
                        }
                    }
                }
                TyKind::Ref(_, x) | TyKind::Record(x) | TyKind::Forall(_, x) => stack.push(x.clone()),
                TyKind::RowExt(_, a, rest) => {
                    stack.push(a.clone());
                    stack.push(rest.clone());
                }
                TyKind::Tuple(ts) => stack.extend(ts.iter().cloned()),
                TyKind::Con(d, args) if self.g.env.structs.contains_key(d) => {
                    let fs = self.g.env.struct_fields(*d, args);
                    stack.extend(fs.into_iter().map(|f| self.icx.resolve(&f.1)));
                }
                TyKind::Con(d, args) if self.g.env.enums.contains_key(d) => {
                    let n = self.g.env.enums[d].variants.len();
                    for v in 0..n {
                        let fs = self.g.env.variant_fields(*d, v as u32, args);
                        stack.extend(fs.into_iter().map(|f| self.icx.resolve(&f.1)));
                    }
                }
                // collections (lists, maps, ...) hold the functions they carry
                TyKind::Con(_, args) => stack.extend(args.iter().map(|a| self.icx.resolve(a))),
                _ => {}
            }
        }
        if !labels.is_empty() {
            let eff = row(labels.into_iter().map(|l| (l, unit())), row_empty());
            self.add_effects(&eff, span);
        }
    }

    /// Does a type mention a region brand (directly, or as a closure's
    /// captured-region mark)?
    pub fn carries_region(&mut self, t: &Ty) -> bool {
        let t = self.icx.resolve(t);
        let skolems = self.icx.skolems.clone();
        if ty_any(
            &t,
            &mut |k| matches!(k, TyKind::Skolem(s) if skolems.get(*s as usize).is_some_and(|i| i.region)),
        ) {
            return true;
        }
        !self.closure_marks(&t).1.is_empty()
    }

    pub fn region_label(&self, s: u32) -> Option<Symbol> {
        self.g.interner.get(&format!("%region{s}"))
    }

    pub fn effect_bound(&mut self, e: &lang_hir::HEffect) -> (Vec<Symbol>, Span) {
        match e {
            lang_hir::HEffect::Set { labels, span } => {
                for l in labels {
                    let s = self.sym(*l);
                    if !matches!(s, "Io" | "Wait" | "State" | "Foreign") {
                        let s = s.to_string();
                        self.error(
                            "E0317",
                            *span,
                            format!("unknown effect `{s}`"),
                            "expected Io, Wait, State, or Foreign",
                        );
                    }
                }
                (labels.clone(), *span)
            }
            lang_hir::HEffect::Var(_) => (vec![], Span::dummy()),
        }
    }

    /// Check a function's ambient effects against its declared bound.
    pub fn finish_effects(&mut self, ctx: &FnCtx) {
        let Some((bound, bspan)) = &ctx.bound else { return };
        if *bspan == Span::dummy() {
            return;
        }
        let amb = self.icx.resolve(&ctx.eff);
        let (labels, _) = row_fields(&amb);
        for (l, _) in labels {
            let name = self.sym(l).to_string();
            if name.starts_with('%') || bound.contains(&l) {
                continue;
            }
            let site = ctx.eff_sites.iter().find(|s| s.0 == l).map(|s| s.1).unwrap_or(*bspan);
            let allowed: Vec<&str> = bound.iter().map(|b| self.sym(*b)).collect();
            let d = Diagnostic::error(format!(
                "effect `{name}` is not permitted by this function's effect bound"
            ))
            .with_code("E0501")
            .with_label(site, format!("this operation has effect `{name}`"))
            .with_label(*bspan, format!("declared bound `!{{{}}}`", allowed.join(", ")));
            self.push_diag(d);
        }
    }

    fn match_expr(
        &mut self,
        _e: ExprId,
        scrutinee: ExprId,
        arms: &[Arm],
        _span: Span,
        expected: Option<&Ty>,
    ) -> Ty {
        let st = self.infer(scrutinee);
        for a in arms {
            self.note_param_derived(scrutinee, a.pat);
        }
        let t = match expected {
            Some(x) => x.clone(),
            None => self.icx.fresh(),
        };
        for arm in arms {
            self.check_pat(arm.pat, &st, false);
            if let Some(g) = arm.guard {
                let b = self.bool_ty();
                self.check(g, &b);
            }
            self.check(arm.body, &t);
        }
        t
    }

    fn stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Let { pat, ty: None, init, .. } if self.generalizable(*pat, *init) => {
                // let-polymorphism for nonexpansive values (closures,
                // function references, constructors)
                self.icx.level += 1;
                let t = self.infer(*init);
                self.solve_some();
                self.icx.level -= 1;
                let rt = self.icx.resolve(&t);
                // only value-type variables generalize: an effect-polymorphic
                // closure stays one closure (its identity is observable)
                let mut vars = vec![];
                self.icx.free_value_vars(&rt, &mut vars);
                let level = self.icx.level;
                vars.retain(|v| self.icx.var_level(*v).is_some_and(|l| l > level));
                // constrained variables stay monomorphic
                let constrained = self.pending.iter().any(|c| {
                    c.types().iter().any(|ct| {
                        let mut fv = vec![];
                        self.icx.free_vars(ct, &mut fv);
                        fv.iter().any(|v| vars.iter().any(|w| self.icx.root(*w) == self.icx.root(*v)))
                    })
                });
                let PatKind::Bind(l) = &self.g.hir.pats[*pat].kind else { unreachable!() };
                if vars.is_empty() || constrained {
                    self.check_pat(*pat, &t, true);
                } else {
                    for (i, v) in vars.iter().enumerate() {
                        let _ = self.icx.unify(&mk(TyKind::Var(*v)), &mk(TyKind::Gen(i as u32)));
                    }
                    let scheme = self.icx.resolve(&t);
                    self.poly_locals.insert(*l, (*init, vars.len() as u32, scheme.clone()));
                    self.check_pat(*pat, &scheme, true);
                }
            }
            Stmt::Let { pat, ty, init, .. } => {
                self.note_param_derived(*init, *pat);
                let t = match ty {
                    Some(h) => {
                        let at = self.hty(h);
                        self.check_arg(*init, &at);
                        at
                    }
                    None => self.infer(*init),
                };
                self.check_pat(*pat, &t, true);
            }
            Stmt::Assign { local, value, span } => {
                let lt = self.locals.get(local).cloned().unwrap_or_else(error_ty);
                self.check(*value, &lt);
                let _ = span;
            }
            Stmt::Expr { expr, .. } => {
                let t = self.infer(*expr);
                let span = self.g.hir.exprs[*expr].span;
                self.pending.push(Constraint::MustUse { ty: t, span });
                // a discarded aggregate discards each of its parts
                let parts: Vec<ExprId> = match &self.g.hir.exprs[*expr].kind {
                    ExprKind::Tuple(xs) | ExprKind::List(xs) => xs.clone(),
                    ExprKind::Record { fields, .. } => fields.iter().map(|f| f.2).collect(),
                    // a discarded constructor value (`Wrap.w(fails())`)
                    ExprKind::Call { callee, args }
                        if matches!(self.g.hir.exprs[*callee].kind, ExprKind::Ctor { .. }) =>
                    {
                        args.clone()
                    }
                    _ => vec![],
                };
                for x in parts {
                    if let Some(pt) = self.exprs.get(&x).cloned() {
                        let span = self.g.hir.exprs[x].span;
                        self.pending.push(Constraint::MustUse { ty: pt, span });
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------ patterns

    pub fn check_pat(&mut self, p: PatId, expected: &Ty, irrefutable: bool) {
        let hir = self.g.hir;
        let pat = &hir.pats[p];
        let span = pat.span;
        self.pats.insert(p, expected.clone());
        match &pat.kind {
            PatKind::Discard | PatKind::Error => {}
            PatKind::Bind(l) => {
                self.locals.insert(*l, expected.clone());
            }
            PatKind::Lit(l) => {
                if irrefutable && !matches!(l, Lit::Unit) {
                    self.error(
                        "E0318",
                        span,
                        "refutable pattern in a binding",
                        "use `match` for literal patterns",
                    );
                }
                let t = self.lit(l, span, Some(expected));
                self.expect(span, expected, &t);
            }
            PatKind::Tuple(ps) => {
                let ts: Vec<Ty> = ps.iter().map(|_| self.icx.fresh()).collect();
                let want = mk(TyKind::Tuple(ts.clone()));
                let exp = self.deref(expected);
                self.expect(span, &want, &exp);
                for (p, t) in ps.iter().zip(&ts) {
                    self.check_pat(*p, t, irrefutable);
                }
            }
            PatKind::Record { fields, open } => {
                let exp = self.deref(expected);
                if let TyKind::Con(d, args) = &*exp
                    && self.g.env.structs.contains_key(d)
                {
                    let sf = self.g.env.struct_fields(*d, args);
                    let outside = Some(self.g.env.structs[d].module) != self.module;
                    for (l, sp) in fields {
                        match sf.iter().find(|f| f.0 == *l) {
                            Some((_, t, is_pub)) => {
                                if outside && !is_pub {
                                    let ls = self.label_str(*l);
                                    self.error(
                                        "E0308",
                                        span,
                                        format!("field `{ls}` is private"),
                                        "declared without `pub`",
                                    );
                                }
                                let t = t.clone();
                                self.check_pat(*sp, &t, irrefutable)
                            }
                            None => {
                                let ls = self.label_str(*l);
                                self.error(
                                    "E0301",
                                    span,
                                    format!("struct has no field `{ls}`"),
                                    "unknown field",
                                );
                            }
                        }
                    }
                    if !open && fields.len() != sf.len() {
                        self.error(
                            "E0319",
                            span,
                            "pattern does not mention all fields",
                            "add `..` to ignore the rest",
                        );
                    }
                    return;
                }
                let ts: Vec<(Symbol, Ty)> = fields.iter().map(|(l, _)| (*l, self.icx.fresh())).collect();
                let tail = if *open { self.icx.fresh_row() } else { row_empty() };
                let want = record(ts.clone(), tail);
                self.expect(span, &want, &exp);
                for ((_, sp), (_, t)) in fields.iter().zip(&ts) {
                    self.check_pat(*sp, t, irrefutable);
                }
            }
            PatKind::Variant { enum_def, name, name_span, args, has_args } => {
                let exp = self.deref(expected);
                let (ed, eargs) = match (enum_def, &*exp) {
                    (Some(d), _) => {
                        let n = self.g.env.enums[d].n_generics;
                        let eargs: Vec<Ty> = (0..n).map(|_| self.icx.fresh()).collect();
                        let want = self.con(*d, eargs.clone());
                        self.expect(span, &want, &exp);
                        (*d, eargs)
                    }
                    (None, TyKind::Con(d, a)) if self.g.env.enums.contains_key(d) => (*d, a.clone()),
                    (None, TyKind::Var(_)) => {
                        // find the unique enum declaring this variant
                        let cands: Vec<DefId> = self
                            .g
                            .env
                            .enums
                            .iter()
                            .filter(|(_, info)| info.variants.iter().any(|v| v.name == *name))
                            .map(|(d, _)| *d)
                            .collect();
                        if cands.len() != 1 {
                            let n = self.sym(*name).to_string();
                            self.error(
                                "E0320",
                                *name_span,
                                format!("cannot infer the enum of variant pattern `.{n}`"),
                                "annotate the scrutinee or write `Enum.variant`",
                            );
                            for a in args {
                                let f = self.icx.fresh();
                                self.check_pat(*a, &f, irrefutable);
                            }
                            return;
                        }
                        let d = cands[0];
                        let n = self.g.env.enums[&d].n_generics;
                        let eargs: Vec<Ty> = (0..n).map(|_| self.icx.fresh()).collect();
                        let want = self.con(d, eargs.clone());
                        self.expect(span, &want, &exp);
                        (d, eargs)
                    }
                    (None, TyKind::Error) => return,
                    (None, _) => {
                        let t = self.show(&exp);
                        self.error(
                            "E0321",
                            span,
                            format!("variant pattern on non-enum type `{t}`"),
                            "not an enum",
                        );
                        return;
                    }
                };
                let info = &self.g.env.enums[&ed];
                let Some(vi) = info.variants.iter().position(|v| v.name == *name) else {
                    let (n, en) =
                        (self.sym(*name).to_string(), self.sym(self.g.hir.defs[ed].name).to_string());
                    self.error(
                        "E0322",
                        *name_span,
                        format!("`{en}` has no variant `{n}`"),
                        "unknown variant",
                    );
                    return;
                };
                if irrefutable && info.variants.len() > 1 {
                    self.error(
                        "E0318",
                        span,
                        "refutable pattern in a binding",
                        "use `match` for variant patterns",
                    );
                }
                let fields = self.g.env.variant_fields(ed, vi as u32, &eargs);
                if (*has_args && args.len() != fields.len()) || (!*has_args && !fields.is_empty() && false) {
                    let n = self.sym(*name).to_string();
                    self.error(
                        "E0323",
                        span,
                        format!("variant `{n}` has {} field(s), pattern has {}", fields.len(), args.len()),
                        "wrong number of fields",
                    );
                }
                for (a, (_, t)) in args.iter().zip(&fields) {
                    self.check_pat(*a, t, irrefutable);
                }
            }
        }
    }

    // ------------------------------------------------------------ bodies

    /// Check a function body given parameter types and a return type.
    pub fn check_fn_body(
        &mut self,
        owner: Owner,
        params: &[lang_hir::Param],
        param_tys: &[Ty],
        ret: &Ty,
        bound: Option<(Vec<Symbol>, Span)>,
        body: ExprId,
    ) -> Ty {
        for (p, t) in params.iter().zip(param_tys) {
            self.check_pat(p.pat, t, true);
        }
        let eff = self.icx.fresh_row();
        self.fns.push(FnCtx { owner, ret: ret.clone(), eff: eff.clone(), eff_sites: vec![], bound });
        self.check(body, ret);
        let ctx = self.fns.pop().unwrap();
        self.solve_some();
        self.finish_effects(&ctx);
        match &ctx.bound {
            Some((labels, s)) if *s != Span::dummy() => row(labels.iter().map(|l| (*l, unit())), row_empty()),
            _ => eff,
        }
    }
}

fn record_is_closed(t: &Ty) -> bool {
    match &**t {
        TyKind::Record(r) => matches!(&*row_fields(r).1, TyKind::RowEmpty),
        _ => false,
    }
}

/// Is this expression statically diverging (return/break/continue/panic)?
pub fn diverges(hir: &lang_hir::Hir, e: ExprId) -> bool {
    match &hir.exprs[e].kind {
        ExprKind::Return { .. } | ExprKind::Break { .. } | ExprKind::Continue { .. } => true,
        ExprKind::Block { stmts, tail } => match tail {
            Some(t) => diverges(hir, *t),
            None => stmts.iter().any(|s| matches!(s, Stmt::Expr { expr, .. } if diverges(hir, *expr))),
        },
        _ => false,
    }
}

/// Open closed effect rows of a function type so a less effectful function
/// can be used where more effects are allowed.
pub fn open_effects(icx: &mut InferCtxt, t: &Ty) -> Ty {
    let t = icx.shallow(t);
    match &*t {
        TyKind::Fn(f) => {
            let (labels, tail) = icx.flatten_row(&f.eff);
            let eff =
                if matches!(&*tail, TyKind::RowEmpty) { row(labels, icx.fresh_row()) } else { f.eff.clone() };
            mk(TyKind::Fn(FnTy { params: f.params.clone(), ret: f.ret.clone(), eff, once: f.once }))
        }
        _ => t,
    }
}
