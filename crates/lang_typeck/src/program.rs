//! Whole-program checking: declared schemes, binding groups, generalization,
//! and per-instance re-checking for code generation.

use std::cell::RefCell;

use lang_base::{Diagnostic, Interner, Span, Symbol};
use lang_hir::{
    DefId, DefKind, ExprId, ExprKind, FnAbi, FnDef, GenericId, HTyKind, Hir, LocalId, Owner, PatId, Stmt,
};
use petgraph::graph::{DiGraph, NodeIndex};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::env::*;
use crate::infer::*;
use crate::solve::Constraint;
use crate::ty::*;

/// A (possibly) polymorphic type: `Gen(0..n)` are quantified. The first
/// `generics.len()` correspond to declared generic parameters in order.
#[derive(Debug, Clone)]
pub struct Scheme {
    pub n: u32,
    pub ty: Ty,
    pub constraints: Vec<Constraint>,
    pub generics: Vec<GenericId>,
}

impl Scheme {
    pub fn mono(ty: Ty) -> Scheme {
        Scheme { n: 0, ty, constraints: vec![], generics: vec![] }
    }
}

/// State shared by all checks of a program.
pub struct Global<'a> {
    pub hir: &'a Hir,
    pub interner: &'a Interner,
    pub env: TypeEnv,
    pub schemes: FxHashMap<DefId, Scheme>,
    /// Globals whose type was generalized (re-evaluated per instance).
    pub poly_globals: FxHashSet<DefId>,
    /// Mutable during checking: schemes produced by binding groups.
    pub pending_schemes: RefCell<FxHashMap<DefId, Scheme>>,
    /// Scheme parameters a generic function may duplicate (use more than
    /// once): instantiating them with an affine type is rejected.
    pub dup_params: RefCell<FxHashMap<DefId, Vec<u32>>>,
    /// Generic functions whose duplication analysis ran, and calls to ones
    /// not yet analysed (checked once every group is done).
    pub dup_done: RefCell<FxHashSet<DefId>>,
    pub dup_deferred: RefCell<Vec<(DefId, Vec<Ty>, Span)>>,
}

pub struct Names<'g, 'a> {
    pub g: &'g Global<'a>,
    pub skolems: &'g [SkolemInfo],
}

impl TyNames for Names<'_, '_> {
    fn sym(&self, s: Symbol) -> &str {
        self.g.interner.resolve(s)
    }
    fn def_name(&self, d: DefId) -> String {
        let def = &self.g.hir.defs[d];
        let name = self.g.interner.resolve(def.name).to_string();
        let m = &self.g.hir.modules[def.module];
        if m.is_std {
            match name.as_str() {
                "I64" => return "Int".into(),
                "F64" => return "Float".into(),
                _ => {}
            }
        }
        if m.is_std && m.path.len() > 1 && self.g.interner.resolve(m.path[1]) != "prelude" {
            format!("{}.{name}", self.g.interner.resolve(*m.path.last().unwrap()))
        } else {
            name
        }
    }
    fn skolem_name(&self, s: u32) -> String {
        self.skolems
            .get(s as usize)
            .map(|k| self.g.interner.resolve(k.name).to_string())
            .unwrap_or_else(|| format!("S{s}"))
    }
}

/// Print a type with source names.
pub fn show_ty(g: &Global<'_>, t: &Ty) -> String {
    crate::ty::Printer::new(&Names { g, skolems: &[] }).print(t)
}

/// Result of checking a whole program.
pub struct Checked<'a> {
    pub g: Global<'a>,
}

/// Concrete typing of one definition instance, for code generation.
#[derive(Debug, Default)]
pub struct Instance {
    pub fn_ty: Option<Ty>,
    pub exprs: FxHashMap<ExprId, Ty>,
    pub locals: FxHashMap<LocalId, Ty>,
    pub pats: FxHashMap<PatId, Ty>,
    pub res: FxHashMap<ExprId, Res>,
    pub displays: FxHashMap<ExprId, Target>,
    pub coercions: FxHashMap<ExprId, Coercion>,
    pub errors: Vec<Diagnostic>,
    /// Generalized local `let`s (binding -> initializer); their subtrees'
    /// types use `Gen(i)`, instantiated per use from `local_insts`.
    pub poly_locals: FxHashMap<LocalId, ExprId>,
    pub local_insts: FxHashMap<ExprId, Vec<Ty>>,
}

/// An instance with quantified variables replaced (one use of a generalized
/// local).
pub fn subst_instance(inst: &Instance, args: &[Ty]) -> Instance {
    let s = |t: &Ty| ground_resolved(&subst_gen(t, args));
    let st = |t: &Target| match t {
        Target::Fn { def, args: a } => Target::Fn { def: *def, args: a.iter().map(s).collect() },
        Target::Derived { imp, self_ty } => Target::Derived { imp: imp.clone(), self_ty: s(self_ty) },
        other => other.clone(),
    };
    Instance {
        fn_ty: inst.fn_ty.clone(),
        exprs: inst.exprs.iter().map(|(k, v)| (*k, s(v))).collect(),
        locals: inst.locals.iter().map(|(k, v)| (*k, s(v))).collect(),
        pats: inst.pats.iter().map(|(k, v)| (*k, s(v))).collect(),
        res: inst
            .res
            .iter()
            .map(|(k, r)| {
                let r = match r {
                    Res::Callee { def, args: a } => {
                        Res::Callee { def: *def, args: a.iter().map(s).collect() }
                    }
                    Res::Ctor { enum_def, variant, args: a } => {
                        Res::Ctor { enum_def: *enum_def, variant: *variant, args: a.iter().map(s).collect() }
                    }
                    Res::Method(t) => Res::Method(st(t)),
                    Res::Try(t) => Res::Try(t.as_ref().map(st)),
                    Res::Display(t) => Res::Display(st(t)),
                    Res::OpFn(t) => Res::OpFn(st(t)),
                    Res::Compare(t) => Res::Compare(s(t)),
                    other => other.clone(),
                };
                (*k, r)
            })
            .collect(),
        displays: inst.displays.iter().map(|(k, t)| (*k, st(t))).collect(),
        coercions: inst
            .coercions
            .iter()
            .map(|(k, c)| {
                let c = match c {
                    Coercion::Dyn { protocol, args: a } => {
                        Coercion::Dyn { protocol: *protocol, args: a.iter().map(s).collect() }
                    }
                    other => other.clone(),
                };
                (*k, c)
            })
            .collect(),
        errors: vec![],
        poly_locals: inst.poly_locals.clone(),
        local_insts: inst.local_insts.clone(),
    }
}

/// Every expression, pattern, and local inside a subtree.
fn subtree(
    hir: &Hir,
    e: ExprId,
    exprs: &mut FxHashSet<ExprId>,
    pats: &mut FxHashSet<PatId>,
    locals: &mut FxHashSet<LocalId>,
) {
    exprs.insert(e);
    let add_pat = |p: PatId, pats: &mut FxHashSet<PatId>, locals: &mut FxHashSet<LocalId>| {
        pats.insert(p);
        let mut ls = vec![];
        crate::regions::collect(hir, p, &mut ls);
        locals.extend(ls);
    };
    match &hir.exprs[e].kind {
        ExprKind::Lambda(l) => l.params.iter().for_each(|p| add_pat(p.pat, pats, locals)),
        ExprKind::For { pat, .. } => add_pat(*pat, pats, locals),
        ExprKind::Match { arms, .. } => arms.iter().for_each(|a| add_pat(a.pat, pats, locals)),
        ExprKind::Block { stmts, .. } => {
            for st in stmts {
                if let Stmt::Let { pat, .. } = st {
                    add_pat(*pat, pats, locals);
                }
            }
        }
        _ => {}
    }
    for c in crate::regions::sub_exprs(&hir.exprs[e].kind) {
        subtree(hir, c, exprs, pats, locals);
    }
}

/// `ground` that keeps quantified variables (generalized local subtrees).
fn ground_keep_gen(icx: &mut InferCtxt, t: &Ty) -> Ty {
    let t = icx.resolve(t);
    let gens: Vec<u32> = {
        let mut v = vec![];
        ty_any(&t, &mut |k| {
            if let TyKind::Gen(i) = k {
                v.push(*i);
            }
            false
        });
        v
    };
    if gens.is_empty() {
        return ground_resolved(&t);
    }
    // ground everything else; keep the variables by round-tripping them
    // through unique placeholder skolem numbers
    let marked = ty_map(&t, &mut |x| match &**x {
        TyKind::Gen(i) => Some(mk(TyKind::Bound(1_000_000 + *i))),
        _ => None,
    });
    let g = ground_keep_bound(&marked);
    ty_map(&g, &mut |x| match &**x {
        TyKind::Bound(b) if *b >= 1_000_000 => Some(mk(TyKind::Gen(*b - 1_000_000))),
        _ => None,
    })
}

fn ground_keep_bound(t: &Ty) -> Ty {
    match &**t {
        TyKind::Bound(b) if *b >= 1_000_000 => t.clone(),
        TyKind::Con(d, args) => mk(TyKind::Con(*d, args.iter().map(ground_keep_bound).collect())),
        TyKind::Dyn(d, args) => mk(TyKind::Dyn(*d, args.iter().map(ground_keep_bound).collect())),
        TyKind::Tuple(ts) => mk(TyKind::Tuple(ts.iter().map(ground_keep_bound).collect())),
        TyKind::Ref(_, x) => ground_keep_bound(x),
        TyKind::Fn(f) => mk(TyKind::Fn(FnTy {
            params: f.params.iter().map(ground_keep_bound).collect(),
            ret: ground_keep_bound(&f.ret),
            eff: row_empty(),
            once: false,
        })),
        TyKind::Record(r) => {
            let (mut fields, _) = row_fields(r);
            fields.sort_by_key(|a| a.0);
            mk(TyKind::Record(row(fields.into_iter().map(|(l, t)| (l, ground_keep_bound(&t))), row_empty())))
        }
        _ => ground_resolved(t),
    }
}

// ------------------------------------------------------------------ checking

static REPL_SESSION: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Mark this process as a local REPL session: bindings typed at the
/// prompt are interactive state, exempt from the module-value rules.
/// Only `lang repl` sets it (it cannot be switched on from outside).
pub fn set_repl_session() {
    REPL_SESSION.store(true, std::sync::atomic::Ordering::SeqCst);
}

fn repl_session() -> bool {
    REPL_SESSION.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn check_program<'a>(hir: &'a Hir, interner: &'a Interner) -> (Checked<'a>, Vec<Diagnostic>) {
    let mut diags = vec![];
    for s in ["%confined", "display", "S", "Foreign", "Io", "Wait", "State"] {
        interner.intern(s);
    }
    let mut icx0 = InferCtxt::new(None);
    let env = TypeEnv::build(hir, interner, &mut icx0, &mut diags);
    let mut g = Global {
        hir,
        interner,
        env,
        schemes: FxHashMap::default(),
        poly_globals: FxHashSet::default(),
        pending_schemes: RefCell::new(FxHashMap::default()),
        dup_params: RefCell::new(FxHashMap::default()),
        dup_done: RefCell::new(FxHashSet::default()),
        dup_deferred: RefCell::new(vec![]),
    };
    // Declared schemes: methods, intrinsics, and fully annotated functions.
    let mut declared: FxHashSet<DefId> = FxHashSet::default();
    for (id, def) in hir.defs.iter() {
        if let DefKind::Fn(f) = &def.kind {
            let annotated = fully_annotated(f);
            if f.owner.is_some() || f.abi != FnAbi::Lang || annotated {
                if !annotated {
                    let n = interner.resolve(def.name).to_string();
                    diags.push(
                        Diagnostic::error(format!("`{n}` must declare all parameter and result types"))
                            .with_code("E0135")
                            .with_label(def.span, "methods and external functions need full signatures"),
                    );
                }
                let s = declared_scheme(&g, id, f, &mut diags);
                if f.abi == FnAbi::C {
                    check_ffi_signature(&g, id, &s.ty, &mut diags);
                    if f.ffi.blocking || f.ffi.pinned {
                        check_contract_signature(&g, id, &s.ty, &mut diags);
                    }
                }
                g.schemes.insert(id, s);
                declared.insert(id);
            }
        }
    }
    check_impl_signatures(&g, &mut diags);
    check_impl_supers(&g, &mut diags);
    // Binding groups over value definitions.
    let values: Vec<DefId> = hir
        .defs
        .iter()
        .filter(|(_, d)| match &d.kind {
            DefKind::Fn(f) => f.body.is_some(),
            DefKind::Global(_) => true,
            _ => false,
        })
        .map(|(id, _)| id)
        .collect();
    let mut graph: DiGraph<DefId, ()> = DiGraph::new();
    let mut nodes: FxHashMap<DefId, NodeIndex> = FxHashMap::default();
    for &v in &values {
        nodes.insert(v, graph.add_node(v));
    }
    for &v in &values {
        let mut refs = vec![];
        let body = match &hir.defs[v].kind {
            DefKind::Fn(f) => f.body.unwrap(),
            DefKind::Global(gd) => gd.value,
            _ => unreachable!(),
        };
        collect_def_refs(hir, body, &mut refs);
        for r in refs {
            // edges to declared functions do not constrain inference order
            if declared.contains(&r) && !matches!(hir.defs[r].kind, DefKind::Global(_)) {
                continue;
            }
            if let Some(&n) = nodes.get(&r) {
                graph.add_edge(nodes[&v], n, ());
            }
        }
    }
    let sccs = petgraph::algo::tarjan_scc(&graph);
    let mut inf_diags = vec![];
    // resolved call targets, for the effect fixpoint below
    let mut call_targets: FxHashMap<ExprId, DefId> = FxHashMap::default();
    {
        let gref: &Global = &g;
        let mut inf = Infer::new(gref, Mode::Generic);
        let mut new_schemes: Vec<(DefId, Scheme)> = vec![];
        for scc in sccs {
            let defs: Vec<DefId> = scc.iter().map(|n| graph[*n]).collect();
            // globals referring to each other cyclically
            if defs.len() > 1 && defs.iter().any(|d| matches!(hir.defs[*d].kind, DefKind::Global(_))) {
                let d = defs.iter().find(|d| matches!(hir.defs[**d].kind, DefKind::Global(_))).unwrap();
                inf.diags.push(
                    Diagnostic::error("cyclic module-level value initialization")
                        .with_code("E0134")
                        .with_label(hir.defs[*d].span, "this value depends on itself"),
                );
            }
            let schemes = check_group(&mut inf, &defs, &declared);
            for (d, s) in schemes {
                inf.g.pending_schemes.borrow_mut().insert(d, s.clone());
                new_schemes.push((d, s));
            }
        }
        inf.solve(true);
        // calls to generic functions analysed after their callers
        crate::ownership::check_deferred_dups(&mut inf);
        crate::ownership::check_task_params(&mut inf);
        if !repl_session() {
            crate::ownership::check_module_closures(&mut inf);
        }
        // monomorphic schemes (e.g. module-level values) may mention variables
        // resolved by later uses: resolve them against the final substitution
        {
            let mut ps = inf.g.pending_schemes.borrow_mut();
            // leftover variables (unconstrained effects, unused positions)
            // must not leak into per-instance inference contexts
            let poison = |t: &Ty| {
                ty_map(t, &mut |x| match &**x {
                    TyKind::Var(_) => Some(error_ty()),
                    _ => None,
                })
            };
            for s in ps.values_mut() {
                s.ty = poison(&inf.icx.resolve(&s.ty));
                let cs: Vec<Constraint> =
                    s.constraints.iter().map(|c| c.map_types(&mut |t| poison(&inf.icx.resolve(t)))).collect();
                s.constraints = cs;
            }
        }
        inf_diags.append(&mut inf.diags);
        for (e, r) in &inf.res {
            let t = match r {
                Res::Callee { def, .. } => Some(*def),
                Res::Method(Target::Fn { def, .. }) | Res::Display(Target::Fn { def, .. }) => Some(*def),
                Res::Try(Some(Target::Fn { def, .. })) => Some(*def),
                // dictionary dispatch: the protocol's declaration of the method
                Res::Method(Target::Dyn { protocol, method }) => match &hir.defs[*protocol].kind {
                    DefKind::Protocol(pd) => {
                        pd.methods.iter().copied().find(|m| hir.defs[*m].name == *method)
                    }
                    _ => None,
                },
                _ => None,
            };
            if let Some(d) = t {
                call_targets.insert(*e, d);
            }
        }
        for (e, t) in &inf.displays {
            if let Target::Fn { def, .. } = t {
                call_targets.insert(*e, *def);
            }
        }
        for (d, s) in new_schemes {
            g_insert(&g, d, s);
        }
    }
    let pend = std::mem::take(&mut *g.pending_schemes.borrow_mut());
    for (d, s) in pend {
        if g.schemes.contains_key(&d) && declared.contains(&d) {
            // refined effect of a declared function
            g.schemes.insert(d, s);
        } else {
            g.schemes.insert(d, s);
        }
    }
    diags.extend(inf_diags);
    effect_fixpoint(&mut g, &call_targets, &mut diags);
    check_impl_effects(&g, &mut diags);
    // module values are pure immutable data: no task-confined mutable cells,
    // affine builders/buffers, or open transactions (a local REPL session's
    // bindings are interactive state instead)
    let repl_session = repl_session();
    for (id, def) in hir.defs.iter() {
        if repl_session && hir.entry == Some(def.module) {
            continue;
        }
        if !matches!(def.kind, DefKind::Global(_)) || hir.modules[def.module].is_std {
            continue;
        }
        let Some(s) = g.schemes.get(&id) else { continue };
        let bad = stored_state(&g, &s.ty, &mut vec![]);
        if let Some(n) = bad {
            let name = g.interner.resolve(def.name).to_string();
            diags.push(
                Diagnostic::error(format!("module-level value `{name}` holds a `{n}`"))
                    .with_code("E0138")
                    .with_label(def.span, "module values are pure immutable data shared by every task")
                    .with_note("create mutable state and acquire resources in functions or system initialization, and pass them where they are needed"),
            );
        }
    }
    (Checked { g }, diags)
}

fn g_insert(_g: &Global, _d: DefId, _s: Scheme) {}

impl Global<'_> {
    pub fn scheme(&self, d: DefId) -> Option<Scheme> {
        if let Some(s) = self.pending_schemes.borrow().get(&d) {
            return Some(s.clone());
        }
        self.schemes.get(&d).cloned()
    }
}

fn fully_annotated(f: &FnDef) -> bool {
    f.params.iter().all(|p| p.ty.as_ref().is_some_and(|t| !has_hole(t)))
        && f.ret.as_ref().is_some_and(|t| !has_hole(t))
}

fn has_hole(t: &lang_hir::HTy) -> bool {
    match &t.kind {
        HTyKind::Hole => true,
        HTyKind::Named { args, .. } | HTyKind::Dyn { args, .. } => args.iter().any(has_hole),
        HTyKind::Tuple(ts) => ts.iter().any(has_hole),
        HTyKind::Record { fields, .. } => fields.iter().any(|f| has_hole(&f.1)),
        HTyKind::Fn { params, ret, .. } => params.iter().any(has_hole) || has_hole(ret),
        HTyKind::Forall { body, .. } => has_hole(body),
        HTyKind::Ref { inner, .. } => has_hole(inner),
        _ => false,
    }
}

/// The generic parameters in scope for a function, in scheme order:
/// owner (impl/protocol) generics first, then the function's own.
pub fn fn_generics(hir: &Hir, f: &FnDef) -> (Vec<GenericId>, bool) {
    let mut gs = vec![];
    let mut protocol_self = false;
    if let Some(owner) = f.owner {
        match &hir.defs[owner].kind {
            DefKind::Impl(i) => gs.extend(i.generics.iter().copied()),
            DefKind::Protocol(p) => {
                protocol_self = true;
                gs.extend(p.generics.iter().copied());
            }
            _ => {}
        }
    }
    gs.extend(f.generics.iter().copied());
    (gs, protocol_self)
}

/// Only primitive, raw-pointer (`ffi.Ptr<T>`), and `repr c` types cross the
/// C boundary; managed values go through adapters such as `ffi.with_cstring`.
fn check_ffi_signature(g: &Global, id: DefId, t: &Ty, diags: &mut Vec<Diagnostic>) {
    fn ok(g: &Global, t: &Ty, ret: bool, depth: usize) -> bool {
        if depth > 8 {
            return false;
        }
        match &**t {
            TyKind::Tuple(ts) if ts.is_empty() => ret,
            TyKind::Con(d, args) => {
                let def = &g.hir.defs[*d];
                let name = g.interner.resolve(def.name);
                if g.hir.modules[def.module].is_std {
                    return matches!(
                        name,
                        "Bool"
                            | "I8"
                            | "I16"
                            | "I32"
                            | "I64"
                            | "U8"
                            | "U16"
                            | "U32"
                            | "U64"
                            | "F32"
                            | "F64"
                            | "Char"
                            | "Ptr"
                    );
                }
                match &def.kind {
                    DefKind::Struct(s) if s.repr_c => {
                        g.env.struct_fields(*d, args).iter().all(|f| ok(g, &f.1, false, depth + 1))
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }
    let TyKind::Fn(f) = &**t else { return };
    let span = g.hir.defs[id].span;
    for (i, p) in f.params.iter().enumerate() {
        if !ok(g, p, false, 0) {
            let shown = crate::program::show_ty(g, p);
            diags.push(
                Diagnostic::error(format!("`{shown}` cannot cross the C boundary"))
                    .with_code("E0409")
                    .with_label(span, format!("parameter {} of this foreign function", i + 1))
                    .with_note("foreign functions take primitives, `ffi.Ptr<T>`, and `repr c` structs; pass text with `ffi.with_cstring`"),
            );
        }
    }
    if !ok(g, &f.ret, true, 0) {
        let shown = crate::program::show_ty(g, &f.ret);
        diags.push(
            Diagnostic::error(format!("`{shown}` cannot cross the C boundary"))
                .with_code("E0409")
                .with_label(span, "the result of this foreign function")
                .with_note("foreign functions return primitives, `ffi.Ptr<T>`, `repr c` structs, or `Unit`"),
        );
    }
}

/// Calls under a `blocking`/`pinned` contract move to another OS thread
/// as machine words: integers, `Bool`, `Char`, and `ffi.Ptr<T>` (at most 8
/// parameters), returning one of those or `Unit`.
fn check_contract_signature(g: &Global, id: DefId, t: &Ty, diags: &mut Vec<Diagnostic>) {
    fn word(g: &Global, t: &Ty, ret: bool) -> bool {
        match &**t {
            TyKind::Tuple(ts) if ts.is_empty() => ret,
            TyKind::Con(d, _) => {
                let def = &g.hir.defs[*d];
                g.hir.modules[def.module].is_std
                    && matches!(
                        g.interner.resolve(def.name),
                        "Bool" | "I8" | "I16" | "I32" | "I64" | "U8" | "U16" | "U32" | "U64" | "Char" | "Ptr"
                    )
            }
            _ => false,
        }
    }
    let TyKind::Fn(f) = &**t else { return };
    let span = g.hir.defs[id].span;
    let bad = f.params.len() > 8 || f.params.iter().any(|p| !word(g, p, false)) || !word(g, &f.ret, true);
    if bad {
        diags.push(
            Diagnostic::error("a `blocking` or `pinned` foreign function takes and returns machine words")
                .with_code("E0409")
                .with_label(span, "this signature cannot move to the blocking pool or the pinned thread")
                .with_note("use integers, `Bool`, `Char`, or `ffi.Ptr<T>` (at most 8 parameters); call float or struct functions without a thread contract"),
        );
    }
}

/// Each implementation method has its protocol method's signature, with
/// `Self` and the protocol's arguments replaced by the implementation's
/// (method generics correspond by position; effects are checked with bodies).
fn check_impl_signatures(g: &Global, diags: &mut Vec<Diagnostic>) {
    let hir = g.hir;
    for imp in &g.env.impls {
        let (Some(idef), Some((proto, pargs)), ImplMethods::Defs(methods)) =
            (imp.def, &imp.protocol, &imp.methods)
        else {
            continue;
        };
        if hir.modules[hir.defs[idef].module].is_std {
            continue;
        }
        let DefKind::Protocol(pd) = &hir.defs[*proto].kind else { continue };
        let pname = g.interner.resolve(hir.defs[*proto].name).to_string();
        for (name, &m) in methods {
            let Some(&pm) = pd.methods.iter().find(|pm| hir.defs[**pm].name == *name) else { continue };
            let (DefKind::Fn(mf), DefKind::Fn(pf)) = (&hir.defs[m].kind, &hir.defs[pm].kind) else {
                continue;
            };
            let (Some(ms), Some(ps)) = (g.schemes.get(&m), g.schemes.get(&pm)) else { continue };
            let mname = g.interner.resolve(*name).to_string();
            let span = hir.defs[m].span;
            if mf.params.len() != pf.params.len() || mf.generics.len() != pf.generics.len() {
                diags.push(
                    Diagnostic::error(format!("`{mname}` does not match its declaration in `{pname}`"))
                        .with_code("E0220")
                        .with_label(
                            span,
                            format!(
                                "takes {} parameter(s) and {} type parameter(s); the protocol declares {} and {}",
                                mf.params.len(),
                                mf.generics.len(),
                                pf.params.len(),
                                pf.generics.len()
                            ),
                        )
                        .with_related(pname.clone(), hir.defs[pm].span),
                );
                continue;
            }
            let mut icx = InferCtxt::new(None);
            // the implementation's generics are rigid; method generics are shared
            let impl_gens: Vec<Ty> =
                imp.generic_ids.iter().map(|gid| icx.fresh_skolem(hir.generics[*gid].name, false)).collect();
            let own: Vec<Ty> =
                mf.generics.iter().map(|gid| icx.fresh_skolem(hir.generics[*gid].name, false)).collect();
            let fill = |icx: &mut InferCtxt, mut args: Vec<Ty>, n: u32| {
                while (args.len() as u32) < n {
                    args.push(icx.fresh());
                }
                args
            };
            let margs: Vec<Ty> = impl_gens.iter().chain(&own).cloned().collect();
            let margs = fill(&mut icx, margs, ms.n);
            let self_ty = crate::env::subst_gen(&imp.self_ty, &impl_gens);
            let mut pa = vec![self_ty];
            pa.extend(pargs.iter().map(|a| crate::env::subst_gen(a, &impl_gens)));
            pa.extend(own.iter().cloned());
            let pa = fill(&mut icx, pa, ps.n);
            let mt = crate::env::subst_gen(&ms.ty, &margs);
            let pt = crate::env::subst_gen(&ps.ty, &pa);
            let (TyKind::Fn(a), TyKind::Fn(b)) = (&*mt, &*pt) else { continue };
            let shape =
                |f: &FnTy| mk(TyKind::Tuple(f.params.iter().cloned().chain([f.ret.clone()]).collect()));
            // parameter modes must agree exactly (owned, `&`, `&mut`): unification
            // alone allows an automatic borrow
            let mode = |t: &Ty| match &**t {
                TyKind::Ref(m, _) => Some(*m),
                _ => None,
            };
            let modes_differ = a.params.iter().zip(&b.params).any(|(x, y)| mode(x) != mode(y))
                || mode(&a.ret) != mode(&b.ret);
            if modes_differ || !icx.try_unify(&shape(a), &shape(b)) {
                let names = Names { g, skolems: &icx.skolems };
                let strip = |f: &FnTy| {
                    mk(TyKind::Fn(FnTy {
                        params: f.params.clone(),
                        ret: f.ret.clone(),
                        eff: row_empty(),
                        once: false,
                    }))
                };
                let (have, want) = (
                    crate::ty::Printer::new(&names).print(&strip(a)),
                    crate::ty::Printer::new(&names).print(&strip(b)),
                );
                diags.push(
                    Diagnostic::error(format!("`{mname}` does not match its declaration in `{pname}`"))
                        .with_code("E0220")
                        .with_label(span, format!("expected `{want}`, found `{have}`"))
                        .with_shapes(want, have)
                        .with_related(pname.clone(), hir.defs[pm].span),
                );
            }
        }
    }
}

/// The mutable state or acquired resource a value of type `t` stores (through
/// fields, elements, and payloads; a function's signature stores nothing).
pub(crate) fn stored_state(g: &Global, t: &Ty, seen: &mut Vec<DefId>) -> Option<String> {
    let hir = g.hir;
    match &**t {
        TyKind::Con(d, args) => {
            let def = &hir.defs[*d];
            let n = g.interner.resolve(def.name);
            if hir.modules[def.module].is_std
                && matches!(
                    n,
                    "Cell"
                        | "Atom"
                        | "Buffer"
                        | "ListBuilder"
                        | "MapBuilder"
                        | "SetBuilder"
                        | "FixedBuffer"
                        | "Batch"
                        | "Tx"
                        | "Pool"
                        | "Sender"
                        | "Receiver"
                        | "BatchSender"
                        | "BatchReceiver"
                        | "Task"
                        | "TaskScope"
                )
            {
                return Some(n.to_string());
            }
            if seen.contains(d) {
                return None;
            }
            seen.push(*d);
            let parts: Vec<Ty> = if let Some(e) = g.env.enums.get(d) {
                (0..e.variants.len())
                    .flat_map(|v| g.env.variant_fields(*d, v as u32, args).into_iter().map(|f| f.1))
                    .collect()
            } else if g.env.structs.contains_key(d) {
                g.env.struct_fields(*d, args).into_iter().map(|f| f.1).collect()
            } else {
                args.clone()
            };
            let r = parts.iter().find_map(|p| stored_state(g, p, seen));
            seen.pop();
            r
        }
        TyKind::Tuple(ts) => ts.iter().find_map(|x| stored_state(g, x, seen)),
        TyKind::Record(r) => row_fields(r).0.iter().find_map(|(_, x)| stored_state(g, x, seen)),
        _ => None,
    }
}

/// Open effect-row variables of the function types inside `t`.
fn effect_tails(t: &Ty, out: &mut Vec<TyVid>) {
    match &**t {
        TyKind::Fn(f) => {
            if let TyKind::Var(v) = &*f.eff {
                out.push(*v);
            }
            f.params.iter().for_each(|p| effect_tails(p, out));
            effect_tails(&f.ret, out);
        }
        TyKind::Con(_, args) | TyKind::Tuple(args) | TyKind::Dyn(_, args) => {
            args.iter().for_each(|a| effect_tails(a, out))
        }
        TyKind::Record(r) | TyKind::Ref(_, r) => effect_tails(r, out),
        TyKind::RowExt(_, a, rest) => {
            effect_tails(a, out);
            effect_tails(rest, out);
        }
        _ => {}
    }
}

/// Functions and methods declared without an effect annotation are
/// effect-polymorphic until their bodies are checked, so callers checked
/// earlier did not see their effects. Propagate the modeled effects of
/// every resolved callee to its callers (to a fixpoint), refine the
/// schemes of unannotated functions, and report an explicit bound the
/// propagated effects exceed (E0501).
fn effect_fixpoint(g: &mut Global, targets: &FxHashMap<ExprId, DefId>, diags: &mut Vec<Diagnostic>) {
    let hir = g.hir;
    let modeled: Vec<Symbol> = g.env.all_effects.clone();
    let labels_of = |g: &Global, d: DefId| -> Vec<Symbol> {
        match g.schemes.get(&d).map(|s| s.ty.clone()) {
            Some(t) => match &*t {
                TyKind::Fn(f) => {
                    row_fields(&f.eff).0.into_iter().map(|(l, _)| l).filter(|l| modeled.contains(l)).collect()
                }
                _ => vec![],
            },
            None => vec![],
        }
    };
    // callees of each user function body (closures excluded: their
    // effects belong to the closure value)
    fn callees(hir: &Hir, e: ExprId, targets: &FxHashMap<ExprId, DefId>, out: &mut Vec<DefId>) {
        if let Some(d) = targets.get(&e) {
            out.push(*d);
        }
        if matches!(hir.exprs[e].kind, lang_hir::ExprKind::Lambda(_)) {
            return;
        }
        for c in crate::regions::sub_exprs(&hir.exprs[e].kind) {
            callees(hir, c, targets, out);
        }
    }
    let mut calls: Vec<(DefId, Vec<DefId>)> = vec![];
    for (id, def) in hir.defs.iter() {
        if hir.modules[def.module].is_std {
            continue;
        }
        if let DefKind::Fn(f) = &def.kind
            && let Some(b) = f.body
        {
            let mut cs = vec![];
            callees(hir, b, targets, &mut cs);
            cs.sort();
            cs.dedup();
            calls.push((id, cs));
        }
    }
    // a call to a protocol method (through a bound or `dyn`) may reach any
    // implementation of it in the program
    for imp in &g.env.impls {
        let (Some((proto, _)), ImplMethods::Defs(methods)) = (&imp.protocol, &imp.methods) else { continue };
        let DefKind::Protocol(pd) = &hir.defs[*proto].kind else { continue };
        for &pm in &pd.methods {
            if let Some(&m) = methods.get(&hir.defs[pm].name) {
                match calls.iter_mut().find(|c| c.0 == pm) {
                    Some(c) => c.1.push(m),
                    None => calls.push((pm, vec![m])),
                }
            }
        }
    }
    let mut effects: FxHashMap<DefId, Vec<Symbol>> = FxHashMap::default();
    for (d, _) in &calls {
        effects.insert(*d, labels_of(g, *d));
    }
    loop {
        let mut changed = false;
        for (d, cs) in &calls {
            let mut add = vec![];
            for c in cs {
                let ls = effects.get(c).cloned().unwrap_or_else(|| labels_of(g, *c));
                for l in ls {
                    if !effects[d].contains(&l) && !add.contains(&l) {
                        add.push(l);
                    }
                }
            }
            if !add.is_empty() {
                effects.get_mut(d).unwrap().extend(add);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    for (d, _) in &calls {
        let have = labels_of(g, *d);
        let all = &effects[d];
        let extra: Vec<Symbol> = all.iter().copied().filter(|l| !have.contains(l)).collect();
        if extra.is_empty() {
            continue;
        }
        let DefKind::Fn(f) = &hir.defs[*d].kind else { continue };
        // a protocol's declaration: its implementations are checked against
        // an explicit annotation by `check_impl_effects`
        if f.body.is_none() {
            continue;
        }
        match &f.effect {
            // an explicit bound: these effects were not permitted
            Some(_) => {
                let names: Vec<String> = extra.iter().map(|l| g.interner.resolve(*l).to_string()).collect();
                let name = g.interner.resolve(hir.defs[*d].name).to_string();
                diags.push(
                    Diagnostic::error(format!("`{name}` performs effects its annotation does not permit"))
                        .with_code("E0501")
                        .with_label(hir.defs[*d].span, format!("calls functions that perform {}", names.join(", ")))
                        .with_note("an explicit effect annotation is an upper bound on everything the function calls"),
                );
            }
            // unannotated: its scheme carries the callees' effects
            None => {
                if let Some(s) = g.schemes.get_mut(d)
                    && let TyKind::Fn(ft) = &*s.ty
                {
                    let mut ft = ft.clone();
                    ft.eff = row(extra.iter().map(|l| (*l, unit())), ft.eff.clone());
                    s.ty = mk(TyKind::Fn(ft));
                }
            }
        }
    }
}

/// An implementation method may not perform effects its protocol method's
/// explicit effect annotation does not allow (checked after inference,
/// when the method's effects are known).
fn check_impl_effects(g: &Global, diags: &mut Vec<Diagnostic>) {
    let hir = g.hir;
    for imp in &g.env.impls {
        let (Some(idef), Some((proto, _)), ImplMethods::Defs(methods)) =
            (imp.def, &imp.protocol, &imp.methods)
        else {
            continue;
        };
        if hir.modules[hir.defs[idef].module].is_std {
            continue;
        }
        let DefKind::Protocol(pd) = &hir.defs[*proto].kind else { continue };
        let pname = g.interner.resolve(hir.defs[*proto].name).to_string();
        for (name, &m) in methods {
            let Some(&pm) = pd.methods.iter().find(|pm| hir.defs[**pm].name == *name) else { continue };
            let DefKind::Fn(pf) = &hir.defs[pm].kind else { continue };
            if pf.effect.is_none() {
                continue;
            }
            let (Some(ms), Some(ps)) = (g.schemes.get(&m), g.schemes.get(&pm)) else { continue };
            let (TyKind::Fn(a), TyKind::Fn(b)) = (&*ms.ty, &*ps.ty) else { continue };
            let (allowed, _) = row_fields(&b.eff);
            let (have, _) = row_fields(&a.eff);
            let extra: Vec<String> = have
                .iter()
                .map(|(l, _)| g.interner.resolve(*l).to_string())
                .filter(|l| matches!(l.as_str(), "Io" | "Wait" | "State" | "Foreign"))
                .filter(|l| !allowed.iter().any(|(x, _)| g.interner.resolve(*x) == l.as_str()))
                .collect();
            if !extra.is_empty() {
                let mname = g.interner.resolve(*name).to_string();
                diags.push(
                    Diagnostic::error(format!("`{mname}` performs effects `{pname}` does not allow"))
                        .with_code("E0501")
                        .with_label(hir.defs[m].span, format!("performs {}", extra.join(", ")))
                        .with_related(pname.clone(), hir.defs[pm].span)
                        .with_note("an implementation is bounded by the protocol method's effect annotation"),
                );
            }
        }
    }
}

/// `impl P for T` where `protocol P: Q` requires that `T` implements `Q`
/// (under the implementation's own where-clauses).
fn check_impl_supers(g: &Global, diags: &mut Vec<Diagnostic>) {
    let hir = g.hir;
    let mut inf = Infer::new(g, Mode::Generic);
    for imp in &g.env.impls {
        let (Some(idef), Some((proto, _))) = (imp.def, &imp.protocol) else { continue };
        if hir.modules[hir.defs[idef].module].is_std {
            continue;
        }
        let Some(info) = g.env.protocols.get(proto) else { continue };
        if info.supers.is_empty() {
            continue;
        }
        let skolems: Vec<Ty> =
            imp.generic_ids.iter().map(|gid| inf.icx.fresh_skolem(hir.generics[*gid].name, false)).collect();
        let self_ty = subst_gen(&imp.self_ty, &skolems);
        inf.givens = imp
            .preds
            .iter()
            .map(|(p, a, s)| (*p, a.iter().map(|x| subst_gen(x, &skolems)).collect(), subst_gen(s, &skolems)))
            .collect();
        for q in info.supers.clone() {
            let n = g.env.protocols.get(&q).map(|i| i.n_generics).unwrap_or(0);
            let args: Vec<Ty> = (0..n).map(|_| inf.icx.fresh()).collect();
            inf.pending.push(Constraint::Impl {
                proto: q,
                args,
                ty: self_ty.clone(),
                span: imp.span,
                expr: None,
            });
        }
        inf.solve(true);
        inf.givens.clear();
    }
    diags.append(&mut inf.diags);
}

fn declared_scheme(g: &Global, id: DefId, f: &FnDef, diags: &mut Vec<Diagnostic>) -> Scheme {
    let hir = g.hir;
    let (gens, protocol_self) = fn_generics(hir, f);
    let offset = protocol_self as u32;
    let mut map: FxHashMap<GenericId, Ty> = FxHashMap::default();
    for (i, gid) in gens.iter().enumerate() {
        map.insert(*gid, mk(TyKind::Gen(i as u32 + offset)));
    }
    let self_ty = match f.owner.map(|o| &hir.defs[o].kind) {
        Some(DefKind::Impl(_)) => g.env.impls.iter().find(|i| i.def == f.owner).map(|i| i.self_ty.clone()),
        Some(DefKind::Protocol(_)) => Some(mk(TyKind::Gen(0))),
        _ => None,
    };
    let mut icx = InferCtxt::new(None);
    let sub = Subst { generics: &map, self_ty: self_ty.clone() };
    let params: Vec<Ty> = f
        .params
        .iter()
        .map(|p| match &p.ty {
            Some(t) => g.env.lower(hir, &mut icx, t, &sub, true, diags),
            None => error_ty(),
        })
        .collect();
    let ret = match &f.ret {
        Some(t) => g.env.lower(hir, &mut icx, t, &sub, true, diags),
        None => unit(),
    };
    let n_named = gens.len() as u32 + offset;
    let mut n = n_named;
    let eff = match &f.effect {
        Some(e) => g.env.lower_effect(hir, &mut icx, e, &sub, diags),
        None => {
            n += 1;
            mk(TyKind::Gen(n - 1))
        }
    };
    // an omitted effect of a function in the result (not tied to a
    // parameter) cannot be chosen by the caller: it may be any effect
    if !hir.modules[hir.defs[id].module].is_std {
        let mut pvars = vec![];
        for p in &params {
            icx.free_vars(p, &mut pvars);
        }
        let mut rvars = vec![];
        icx.free_vars(&ret, &mut rvars);
        let any = g.env.any_effect();
        for v in rvars {
            if !pvars.contains(&v) {
                let _ = icx.unify(&mk(TyKind::Var(v)), &any);
            }
        }
    }
    let ret = icx.resolve(&ret);
    let mut ty = mk(TyKind::Fn(FnTy { params, ret, eff, once: false }));
    // effect variables inside parameter types (`fn(A) -> B` without `!`)
    let mut vars = vec![];
    icx.free_vars(&ty, &mut vars);
    if !vars.is_empty() {
        let mut m: FxHashMap<TyVid, u32> = FxHashMap::default();
        for v in vars {
            m.insert(v, n);
            n += 1;
        }
        ty = ty_map(&ty, &mut |x| match &**x {
            TyKind::Var(v) => m.get(v).map(|i| mk(TyKind::Gen(*i))),
            _ => None,
        });
    }
    // where-clauses and bounds become scheme constraints
    let mut constraints = vec![];
    let mut preds: Vec<(lang_hir::HTy, Vec<lang_hir::HTy>)> =
        f.where_preds.iter().map(|w| (w.ty.clone(), w.bounds.clone())).collect();
    for gid in &f.generics {
        let gp = &hir.generics[*gid];
        if !gp.bounds.is_empty() {
            let t = lang_hir::HTy { kind: HTyKind::Generic(*gid), span: gp.span };
            preds.push((t, gp.bounds.clone()));
        }
    }
    for (t, bounds) in preds {
        let subj = g.env.lower(hir, &mut icx, &t, &sub, false, diags);
        for b in bounds {
            if let Some((p, args)) = g.env.lower_bound(hir, &mut icx, &b, &sub, diags) {
                constraints.push(Constraint::Impl {
                    proto: p,
                    args,
                    ty: subj.clone(),
                    span: b.span,
                    expr: None,
                });
            }
        }
    }
    let _ = id;
    let mut generics = gens;
    if protocol_self {
        // Gen(0) is Self; declared generics start at 1
        generics.insert(0, GenericId::from_raw(u32::MAX.into()));
    }
    Scheme { n, ty, constraints, generics }
}

/// Givens (assumptions) for checking a generic body: where-clauses and
/// bounds lowered with the in-scope generic types.
fn givens_for(inf: &mut Infer<'_, '_>, f: &FnDef) -> Vec<(DefId, Vec<Ty>, Ty)> {
    let hir = inf.g.hir;
    let mut out = vec![];
    let mut preds: Vec<(lang_hir::HTy, Vec<lang_hir::HTy>)> =
        f.where_preds.iter().map(|w| (w.ty.clone(), w.bounds.clone())).collect();
    let (gens, _) = fn_generics(hir, f);
    for gid in &gens {
        let gp = &hir.generics[*gid];
        if !gp.bounds.is_empty() {
            preds.push((lang_hir::HTy { kind: HTyKind::Generic(*gid), span: gp.span }, gp.bounds.clone()));
        }
    }
    if let Some(owner) = f.owner
        && let DefKind::Impl(i) = &hir.defs[owner].kind
    {
        preds.extend(i.where_preds.iter().map(|w| (w.ty.clone(), w.bounds.clone())));
    }
    for (t, bounds) in preds {
        let subj = inf.hty(&t);
        for b in bounds {
            let gens = inf.generics.clone();
            let sub = Subst { generics: &gens, self_ty: inf.self_ty.clone() };
            let mut d = vec![];
            if let Some((p, args)) = inf.g.env.lower_bound(hir, &mut inf.icx, &b, &sub, &mut d) {
                out.push((p, args, subj.clone()));
            }
        }
    }
    out
}

/// Check one binding group and return its schemes.
fn check_group(inf: &mut Infer<'_, '_>, defs: &[DefId], declared: &FxHashSet<DefId>) -> Vec<(DefId, Scheme)> {
    let hir = inf.g.hir;
    inf.icx.level += 1;
    let outer_pending = std::mem::take(&mut inf.pending);
    // monotypes for inferred members
    for &d in defs {
        if !declared.contains(&d) {
            let t = inf.icx.fresh();
            inf.mono.insert(d, t);
        }
    }
    let mut def_tys: Vec<(DefId, Ty, Vec<Ty>)> = vec![];
    for &d in defs {
        let (ty, skolems) = check_def_body(inf, d, declared.contains(&d));
        if let Some(m) = inf.mono.get(&d).cloned() {
            let span = hir.defs[d].span;
            inf.expect(span, &m, &ty);
        }
        def_tys.push((d, ty, skolems));
    }
    inf.solve_some();
    inf.default_err_convs();
    inf.solve_some();
    let visible: Vec<Ty> =
        def_tys.iter().map(|(_, t, _)| t.clone()).chain(inf.mono.values().cloned()).collect();
    inf.solve_internal(&visible);
    inf.icx.level -= 1;
    for &d in defs {
        inf.mono.remove(&d);
    }
    // post-checks need resolved types
    for &d in defs {
        crate::exhaust::check_def(inf, d);
        crate::exhaust::check_system_nodes(inf, d);
        crate::exhaust::check_row_labels(inf, d);
        crate::ownership::check_def(inf, d);
    }
    // user code only: the standard library's own region plumbing is trusted
    if defs.iter().any(|d| !hir.modules[hir.defs[*d].module].is_std) {
        crate::regions::check(inf);
    } else {
        inf.region_lambdas.clear();
    }
    // generalize
    let level = inf.icx.level;
    let mut out = vec![];
    let group_pending = std::mem::take(&mut inf.pending);
    let mut used = vec![false; group_pending.len()];
    for (d, ty, skolems) in def_tys {
        let ty = inf.icx.resolve(&ty);
        let generalizable = match &hir.defs[d].kind {
            DefKind::Global(gd) => is_value(hir, gd.value),
            _ => true,
        };
        let mut vars = vec![];
        if generalizable {
            inf.icx.free_vars(&ty, &mut vars);
            vars.retain(|v| inf.icx.var_level(*v).is_some_and(|l| l > level));
        }
        // declared generics (skolems) first
        let n_sk = skolems.len() as u32;
        let mut var_idx: FxHashMap<TyVid, u32> = FxHashMap::default();
        for (i, v) in vars.iter().enumerate() {
            var_idx.insert(inf.icx.root(*v), n_sk + i as u32);
        }
        let sk_idx: FxHashMap<u32, u32> = skolems
            .iter()
            .enumerate()
            .filter_map(|(i, s)| match &**s {
                TyKind::Skolem(k) => Some((*k, i as u32)),
                _ => None,
            })
            .collect();
        let mut n = n_sk + vars.len() as u32;
        let icx = &mut inf.icx;
        let generalize =
            |t: &Ty, icx: &mut InferCtxt, var_idx: &mut FxHashMap<TyVid, u32>, n: &mut u32| -> Ty {
                let t = icx.resolve(t);
                ty_map(&t, &mut |x| match &**x {
                    TyKind::Var(v) => {
                        let r = icx.root(*v);
                        if let Some(i) = var_idx.get(&r) {
                            return Some(mk(TyKind::Gen(*i)));
                        }
                        // a variable that only occurs in a captured constraint
                        if icx.var_level(r).is_some_and(|l| l > level) && generalizable {
                            let i = *n;
                            *n += 1;
                            var_idx.insert(r, i);
                            return Some(mk(TyKind::Gen(i)));
                        }
                        None
                    }
                    TyKind::Skolem(k) => sk_idx.get(k).map(|i| mk(TyKind::Gen(*i))),
                    _ => None,
                })
            };
        let gty = generalize(&ty, icx, &mut var_idx, &mut n);
        // constraints mentioning generalized variables belong to the scheme
        let mut cs = vec![];
        if generalizable {
            for (ci, c) in group_pending.iter().enumerate() {
                let mut mentions = false;
                for t in c.types() {
                    let mut fv = vec![];
                    icx.free_vars(&t, &mut fv);
                    if fv.iter().any(|v| var_idx.contains_key(&icx.root(*v))) {
                        mentions = true;
                    }
                    let rt = icx.resolve(&t);
                    if ty_any(&rt, &mut |k| matches!(k, TyKind::Skolem(s) if sk_idx.contains_key(s))) {
                        mentions = true;
                    }
                }
                if mentions {
                    used[ci] = true;
                    let c2 = c.map_types(&mut |t| generalize(t, icx, &mut var_idx, &mut n));
                    cs.push(c2);
                }
            }
        }
        let generics = match &hir.defs[d].kind {
            DefKind::Fn(f) => {
                let (gs, ps) = fn_generics(hir, f);
                let mut gs = gs;
                if ps {
                    gs.insert(0, GenericId::from_raw(u32::MAX.into()));
                }
                gs
            }
            _ => vec![],
        };
        let scheme = if declared.contains(&d) {
            // keep the declared shape; refine only the inferred effect
            let mut s = inf.g.scheme(d).unwrap_or_else(|| Scheme::mono(gty.clone()));
            if let (TyKind::Fn(decl), TyKind::Fn(inferred)) = (&*s.ty, &*gty)
                && let TyKind::Gen(_) = &*decl.eff
            {
                // replace the provisional "any effect" with the inferred row
                let eff = close_effect_row(&inferred.eff);
                let eff = if ty_any(&eff, &mut |k| matches!(k, TyKind::Gen(i) if *i >= s.n)) {
                    decl.eff.clone()
                } else {
                    eff
                };
                s.ty = mk(TyKind::Fn(FnTy {
                    params: decl.params.clone(),
                    ret: decl.ret.clone(),
                    eff,
                    once: false,
                }));
            }
            s
        } else {
            if matches!(hir.defs[d].kind, DefKind::Global(_)) && n > 0 {
                inf.g.poly_globals_insert(d);
            }
            Scheme { n, ty: gty, constraints: cs, generics }
        };
        out.push((d, scheme));
    }
    // generic bodies: which type parameters they may duplicate
    for (d, s) in &out {
        crate::ownership::generic_pass(inf, *d, s);
    }
    // constraints not captured by any scheme stay pending
    inf.pending = outer_pending;
    for (ci, c) in group_pending.into_iter().enumerate() {
        if !used[ci] {
            inf.pending.push(c);
        }
    }
    out
}

/// Close an inferred effect row: unresolved tails become empty unless they
/// are quantified effect variables.
fn close_effect_row(t: &Ty) -> Ty {
    let (fields, tail) = row_fields(t);
    match &*tail {
        TyKind::Gen(_) => t.clone(),
        _ => row(fields, row_empty()),
    }
}

impl Global<'_> {
    fn poly_globals_insert(&self, _d: DefId) {
        // recorded via scheme arity (n > 0) instead
    }
}

/// Syntactic values that may be generalized (value restriction).
fn is_value(hir: &Hir, e: ExprId) -> bool {
    match &hir.exprs[e].kind {
        ExprKind::Lambda(_) | ExprKind::Lit(_) | ExprKind::Ctor { .. } | ExprKind::Def { .. } => true,
        ExprKind::Call { callee, args } => {
            matches!(hir.exprs[*callee].kind, ExprKind::Ctor { .. }) && args.iter().all(|a| is_value(hir, *a))
        }
        ExprKind::Tuple(xs) => xs.iter().all(|x| is_value(hir, *x)),
        ExprKind::Record { spread: None, fields } => fields.iter().all(|f| is_value(hir, f.2)),
        _ => false,
    }
}

/// Check a definition body in generic mode. Returns its type and the
/// skolems standing for its declared generics (in scheme order).
fn check_def_body(inf: &mut Infer<'_, '_>, d: DefId, declared: bool) -> (Ty, Vec<Ty>) {
    let hir = inf.g.hir;
    let def = &hir.defs[d];
    inf.module = Some(def.module);
    match &def.kind {
        DefKind::Fn(f) => {
            let (gens, protocol_self) = fn_generics(hir, f);
            inf.generics.clear();
            let mut skolems = vec![];
            inf.self_ty = None;
            if protocol_self {
                let s = inf.icx.fresh_skolem(inf.g.interner.intern("Self"), false);
                inf.self_ty = Some(s.clone());
                skolems.push(s);
            }
            for gid in &gens {
                let s = inf.icx.fresh_skolem(hir.generics[*gid].name, false);
                inf.generics.insert(*gid, s.clone());
                skolems.push(s);
            }
            if let Some(owner) = f.owner
                && let DefKind::Impl(_) = &hir.defs[owner].kind
                && let Some(imp) = inf.g.env.impls.iter().find(|i| i.def == Some(owner))
            {
                let args: Vec<Ty> = skolems.iter().take(imp.n_generics as usize).cloned().collect();
                inf.self_ty = Some(subst_gen(&imp.self_ty, &args));
            }
            inf.givens = givens_for(inf, f);
            // a default method body may use its own protocol on `Self`
            if let Some(owner) = f.owner
                && let DefKind::Protocol(pd) = &hir.defs[owner].kind
                && let Some(st) = inf.self_ty.clone()
            {
                let pargs: Vec<Ty> =
                    pd.generics.iter().filter_map(|g| inf.generics.get(g).cloned()).collect();
                inf.givens.push((owner, pargs, st));
            }
            let params: Vec<Ty> = f
                .params
                .iter()
                .map(|p| match &p.ty {
                    Some(t) => inf.hty(t),
                    None => inf.icx.fresh(),
                })
                .collect();
            let ret = match &f.ret {
                Some(t) => inf.hty(t),
                None => inf.icx.fresh(),
            };
            // as in the declared scheme: a function in the result whose
            // effect is omitted (and not tied to a parameter) may perform any
            // effect, so the body cannot return a closure the type cannot track
            if f.ret.is_some() && !hir.modules[hir.defs[d].module].is_std {
                let mut pvars = vec![];
                for p in &params {
                    let p = inf.icx.resolve(p);
                    inf.icx.free_vars(&p, &mut pvars);
                }
                let mut tails = vec![];
                effect_tails(&inf.icx.resolve(&ret), &mut tails);
                let any = inf.g.env.any_effect();
                for v in tails {
                    if !pvars.contains(&v) {
                        let _ = inf.icx.unify(&mk(TyKind::Var(v)), &any);
                    }
                }
            }
            let bound = f.effect.as_ref().map(|e| inf.effect_bound(e));
            let eff = match f.body {
                Some(b) => inf.check_fn_body(Owner::Fn(d), &f.params, &params, &ret, bound, b),
                None => row_empty(),
            };
            let _ = declared;
            (mk(TyKind::Fn(FnTy { params, ret, eff, once: false })), skolems)
        }
        DefKind::Global(gd) => {
            inf.generics.clear();
            inf.self_ty = None;
            inf.givens.clear();
            // module initialization is checked like a function body; it may
            // not perform I/O, wait, or call foreign code (no hidden
            // environment reads during imports)
            let ret = inf.icx.fresh();
            let eff = inf.icx.fresh_row();
            inf.fns.push(FnCtx {
                owner: Owner::Global(d),
                ret: ret.clone(),
                eff: eff.clone(),
                eff_sites: vec![],
                bound: None,
            });
            let t = match &gd.ty {
                Some(h) => {
                    let at = inf.hty(h);
                    inf.check_arg(gd.value, &at);
                    at
                }
                None => inf.infer(gd.value),
            };
            inf.fns.pop();
            let (labels, _) = inf.icx.flatten_row(&eff);
            for (l, _) in labels {
                let name = inf.sym(l).to_string();
                if matches!(name.as_str(), "Io" | "Wait" | "Foreign") {
                    let span = hir.exprs[gd.value].span;
                    inf.push_diag(
                        Diagnostic::error(format!("a module-level value cannot have effect `{name}`"))
                            .with_code("E0137")
                            .with_label(span, "module values are computed on first use, outside any explicit call")
                            .with_note("read the environment, files, or clocks in `main` (or a function it calls) and pass the values along"),
                    );
                }
            }
            inf.locals.insert(gd.local, t.clone());
            (t, vec![])
        }
        _ => (error_ty(), vec![]),
    }
}

/// Definitions referenced from an expression tree.
pub fn collect_def_refs(hir: &Hir, e: ExprId, out: &mut Vec<DefId>) {
    walk_exprs(hir, e, &mut |x| {
        if let ExprKind::Def { def, .. } = &hir.exprs[x].kind {
            out.push(*def)
        }
    });
}

/// Visit every expression in a tree (pre-order).
pub fn walk_exprs(hir: &Hir, e: ExprId, f: &mut impl FnMut(ExprId)) {
    f(e);
    let go = |x: ExprId, f: &mut dyn FnMut(ExprId)| walk_exprs_dyn(hir, x, f);
    match &hir.exprs[e].kind {
        ExprKind::Interp(parts) => {
            for p in parts {
                if let lang_hir::InterpPart::Expr(x) = p {
                    go(*x, f);
                }
            }
        }
        ExprKind::Field { base, .. } => go(*base, f),
        ExprKind::Index { base, index } => {
            go(*base, f);
            go(*index, f);
        }
        ExprKind::Call { callee, args } => {
            go(*callee, f);
            for a in args {
                go(*a, f);
            }
        }
        ExprKind::MethodCall { recv, args, .. } => {
            go(*recv, f);
            for a in args {
                go(*a, f);
            }
        }
        ExprKind::Try(x)
        | ExprKind::Unary { operand: x, .. }
        | ExprKind::Unsafe(x)
        | ExprKind::Loop { body: x } => go(*x, f),
        ExprKind::Binary { lhs, rhs, .. } => {
            go(*lhs, f);
            go(*rhs, f);
        }
        ExprKind::Tuple(xs) | ExprKind::List(xs) | ExprKind::Set(xs) => {
            for x in xs {
                go(*x, f);
            }
        }
        ExprKind::Record { spread, fields } => {
            if let Some(s) = spread {
                go(*s, f);
            }
            for (_, _, x) in fields {
                go(*x, f);
            }
        }
        ExprKind::Map(kv) => {
            for (k, v) in kv {
                go(*k, f);
                go(*v, f);
            }
        }
        ExprKind::Lambda(l) => go(l.body, f),
        ExprKind::If { cond, then, els } => {
            go(*cond, f);
            go(*then, f);
            if let Some(x) = els {
                go(*x, f);
            }
        }
        ExprKind::Match { scrutinee, arms } => {
            go(*scrutinee, f);
            for a in arms {
                if let Some(g) = a.guard {
                    go(g, f);
                }
                go(a.body, f);
            }
        }
        ExprKind::While { cond, body } => {
            go(*cond, f);
            go(*body, f);
        }
        ExprKind::For { iter, body, .. } => {
            go(*iter, f);
            go(*body, f);
        }
        ExprKind::Block { stmts, tail } => {
            for s in stmts {
                match s {
                    Stmt::Let { init, .. } => go(*init, f),
                    Stmt::Assign { value, .. } => go(*value, f),
                    Stmt::Expr { expr, .. } => go(*expr, f),
                }
            }
            if let Some(t) = tail {
                go(*t, f);
            }
        }
        ExprKind::Return { value, .. } | ExprKind::Break { value, .. } => {
            if let Some(v) = value {
                go(*v, f);
            }
        }
        ExprKind::Lit(_)
        | ExprKind::Local(_)
        | ExprKind::Def { .. }
        | ExprKind::Ctor { .. }
        | ExprKind::AssocFn { .. }
        | ExprKind::Continue { .. }
        | ExprKind::Error => {}
    }
}

fn walk_exprs_dyn(hir: &Hir, e: ExprId, f: &mut dyn FnMut(ExprId)) {
    walk_exprs(hir, e, &mut |x| f(x));
}

// ------------------------------------------------------------------ instances

/// Re-check definition `def` at concrete scheme arguments `args`, producing
/// concrete types and resolutions for code generation.
pub fn check_instance(g: &Global<'_>, def: DefId, args: &[Ty]) -> Instance {
    let hir = g.hir;
    let mut inf = Infer::new(g, Mode::Instance);
    inf.icx.lenient_effects = true;
    let Some(scheme) = g.scheme(def) else { return Instance::default() };
    let d = &hir.defs[def];
    inf.module = Some(d.module);
    let concrete = subst_gen(&scheme.ty, args);
    match &d.kind {
        DefKind::Fn(f) => {
            let (gens, protocol_self) = fn_generics(hir, f);
            let off = protocol_self as usize;
            for (i, gid) in gens.iter().enumerate() {
                if let Some(a) = args.get(i + off) {
                    inf.generics.insert(*gid, a.clone());
                }
            }
            if protocol_self {
                inf.self_ty = args.first().cloned();
            }
            if let Some(owner) = f.owner
                && let Some(imp) = g.env.impls.iter().find(|i| i.def == Some(owner))
            {
                let iargs: Vec<Ty> = args.iter().take(imp.n_generics as usize).cloned().collect();
                inf.self_ty = Some(subst_gen(&imp.self_ty, &iargs));
            }
            let TyKind::Fn(cf) = &*concrete else { return Instance::default() };
            let params: Vec<Ty> = f
                .params
                .iter()
                .zip(&cf.params)
                .map(|(p, c)| {
                    let t = match &p.ty {
                        Some(h) => inf.hty(h),
                        None => inf.icx.fresh(),
                    };
                    let _ = inf.icx.unify(&t, c);
                    t
                })
                .collect();
            let ret = match &f.ret {
                Some(h) => inf.hty(h),
                None => inf.icx.fresh(),
            };
            let _ = inf.icx.unify(&ret, &cf.ret);
            if let Some(b) = f.body {
                inf.check_fn_body(Owner::Fn(def), &f.params, &params, &ret, None, b);
            }
        }
        DefKind::Global(gd) => {
            let t = match &gd.ty {
                Some(h) => inf.hty(h),
                None => inf.icx.fresh(),
            };
            let _ = inf.icx.unify(&t, &concrete);
            let eff = inf.icx.fresh_row();
            inf.fns.push(FnCtx {
                owner: Owner::Global(def),
                ret: t.clone(),
                eff,
                eff_sites: vec![],
                bound: None,
            });
            inf.check_arg(gd.value, &t);
            inf.fns.pop();
            inf.locals.insert(gd.local, t);
        }
        _ => {}
    }
    inf.solve_some();
    inf.default_err_convs();
    inf.solve(true);
    zonk_instance(inf, concrete)
}

/// Fully resolve all recorded types, defaulting unconstrained variables.
fn zonk_instance(mut inf: Infer<'_, '_>, fn_ty: Ty) -> Instance {
    let z = |icx: &mut InferCtxt, t: &Ty| -> Ty { ground(icx, t) };
    // generalized local subtrees keep their quantified variables
    let (mut pe, mut pp, mut pl) = (FxHashSet::default(), FxHashSet::default(), FxHashSet::default());
    for (_, (init, _, _)) in inf.poly_locals.clone() {
        subtree(inf.g.hir, init, &mut pe, &mut pp, &mut pl);
    }
    let exprs = inf
        .exprs
        .clone()
        .iter()
        .map(|(k, v)| {
            (*k, if pe.contains(k) { ground_keep_gen(&mut inf.icx, v) } else { z(&mut inf.icx, v) })
        })
        .collect();
    let locals = inf
        .locals
        .clone()
        .iter()
        .map(|(k, v)| {
            (*k, if pl.contains(k) { ground_keep_gen(&mut inf.icx, v) } else { z(&mut inf.icx, v) })
        })
        .collect();
    let pats = inf
        .pats
        .clone()
        .iter()
        .map(|(k, v)| {
            (*k, if pp.contains(k) { ground_keep_gen(&mut inf.icx, v) } else { z(&mut inf.icx, v) })
        })
        .collect();
    let poly_locals = inf.poly_locals.iter().map(|(l, (init, _, _))| (*l, *init)).collect();
    let local_insts = inf
        .local_insts
        .clone()
        .iter()
        .map(|(k, v)| (*k, v.iter().map(|t| z(&mut inf.icx, t)).collect()))
        .collect();
    let zk = |icx: &mut InferCtxt, keep: bool, t: &Ty| -> Ty {
        if keep { ground_keep_gen(icx, t) } else { ground(icx, t) }
    };
    let res = inf
        .res
        .clone()
        .iter()
        .map(|(k, r)| {
            let keep = pe.contains(k);
            let z = |icx: &mut InferCtxt, t: &Ty| zk(icx, keep, t);
            let zonk_target = |icx: &mut InferCtxt, t: &Target| zonk_target_with(icx, t, keep);
            let r = match r {
                Res::Callee { def, args } => {
                    Res::Callee { def: *def, args: args.iter().map(|a| z(&mut inf.icx, a)).collect() }
                }
                Res::Ctor { enum_def, variant, args } => Res::Ctor {
                    enum_def: *enum_def,
                    variant: *variant,
                    args: args.iter().map(|a| z(&mut inf.icx, a)).collect(),
                },
                Res::Method(t) => Res::Method(zonk_target(&mut inf.icx, t)),
                Res::Try(t) => Res::Try(t.as_ref().map(|t| zonk_target(&mut inf.icx, t))),
                Res::Display(t) => Res::Display(zonk_target(&mut inf.icx, t)),
                Res::OpFn(t) => Res::OpFn(zonk_target(&mut inf.icx, t)),
                Res::Compare(t) => Res::Compare(z(&mut inf.icx, t)),
                other => other.clone(),
            };
            (*k, r)
        })
        .collect();
    let displays = inf
        .displays
        .clone()
        .iter()
        .map(|(k, t)| (*k, zonk_target_with(&mut inf.icx, t, pe.contains(k))))
        .collect();
    let coercions = inf
        .coercions
        .clone()
        .iter()
        .map(|(k, c)| {
            let keep = pe.contains(k);
            let c = match c {
                Coercion::Struct => Coercion::Struct,
                Coercion::ToSeq => Coercion::ToSeq,
                Coercion::RangeToSeq => Coercion::RangeToSeq,
                Coercion::ToBytes => Coercion::ToBytes,
                Coercion::ToData => Coercion::ToData,
                Coercion::Dyn { protocol, args } => Coercion::Dyn {
                    protocol: *protocol,
                    args: args.iter().map(|a| zk(&mut inf.icx, keep, a)).collect(),
                },
            };
            (*k, c)
        })
        .collect();
    let fn_ty = Some(z(&mut inf.icx, &fn_ty));
    Instance {
        fn_ty,
        exprs,
        locals,
        pats,
        res,
        displays,
        coercions,
        errors: inf.diags,
        poly_locals,
        local_insts,
    }
}

fn zonk_target_with(icx: &mut InferCtxt, t: &Target, keep: bool) -> Target {
    let g = |icx: &mut InferCtxt, t: &Ty| if keep { ground_keep_gen(icx, t) } else { ground(icx, t) };
    match t {
        Target::Fn { def, args } => Target::Fn { def: *def, args: args.iter().map(|a| g(icx, a)).collect() },
        Target::Derived { imp, self_ty } => Target::Derived { imp: imp.clone(), self_ty: g(icx, self_ty) },
        Target::Field(l) => Target::Field(*l),
        Target::Dyn { protocol, method } => Target::Dyn { protocol: *protocol, method: *method },
    }
}

/// Resolve a type completely for code generation: unresolved type variables
/// become `Unit`, open rows close, effect rows are erased, and references
/// are stripped (borrows share their referent's representation).
pub fn ground(icx: &mut InferCtxt, t: &Ty) -> Ty {
    let t = icx.resolve(t);
    ground_resolved(&t)
}

pub fn ground_resolved(t: &Ty) -> Ty {
    match &**t {
        TyKind::Proj(b, n) => {
            let b = ground_resolved(b);
            match crate::ty::normalize_proj(&b, *n) {
                Some(x) => ground_resolved(&x),
                None => unit(),
            }
        }
        TyKind::Var(_) | TyKind::Error | TyKind::Skolem(_) | TyKind::Gen(_) | TyKind::Bound(_) => unit(),
        TyKind::Ref(_, x) => ground_resolved(x),
        TyKind::Forall(_, x) => ground_resolved(x),
        TyKind::Con(d, args) => mk(TyKind::Con(*d, args.iter().map(ground_resolved).collect())),
        TyKind::Dyn(d, args) => mk(TyKind::Dyn(*d, args.iter().map(ground_resolved).collect())),
        TyKind::Tuple(ts) => mk(TyKind::Tuple(ts.iter().map(ground_resolved).collect())),
        TyKind::Fn(f) => mk(TyKind::Fn(FnTy {
            params: f.params.iter().map(ground_resolved).collect(),
            ret: ground_resolved(&f.ret),
            eff: row_empty(),
            once: false,
        })),
        TyKind::Record(r) => {
            let (mut fields, _) = row_fields(r);
            fields.sort_by_key(|a| a.0);
            mk(TyKind::Record(row(fields.into_iter().map(|(l, t)| (l, ground_resolved(&t))), row_empty())))
        }
        TyKind::RowEmpty | TyKind::RowExt(..) => {
            let (mut fields, _) = row_fields(t);
            fields.sort_by_key(|a| a.0);
            row(fields.into_iter().map(|(l, t)| (l, ground_resolved(&t))), row_empty())
        }
    }
}

/// Span of a definition, for diagnostics produced late.
pub fn def_span(hir: &Hir, d: DefId) -> Span {
    hir.defs[d].span
}
