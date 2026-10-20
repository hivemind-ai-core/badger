//! Monomorphization and typed-HIR → MIR lowering.
//!
//! Every function instance is re-checked at concrete types
//! ([`badger_typeck::check_instance`]); the resulting concrete expression types
//! and resolutions drive lowering. Lambdas become separate functions whose
//! first parameter is their closure object.

use std::collections::VecDeque;
use std::rc::Rc;

use badger_base::{SourceMap, Span, Symbol};
use badger_hir::{
    BinOp as HBin, DefId, DefKind, ExprId, ExprKind, FnAbi, InterpPart, Lit, LocalId, Owner, PatId, PatKind,
    Stmt as HStmt, UnOp,
};
use badger_runtime::{Repr, TypeDesc};
use badger_typeck::env::DerivedImpl as DerivedImplPub;
use badger_typeck::{Global, IndexKind, Instance, IterKind, Res, Target, Ty, TyKind};
use rustc_hash::FxHashMap;

use crate::layout::Layouts;
use crate::mir::*;

mod fusion;

pub struct Mono<'g, 'a> {
    pub g: &'g Global<'a>,
    pub lay: Layouts<'g, 'a>,
    pub sources: &'g SourceMap,
    pub prog: Program,
    instances: FxHashMap<(DefId, Vec<Ty>), FuncId>,
    /// Generated derived `Error.cause` functions.
    error_causes: FxHashMap<(DefId, Vec<Ty>), FuncId>,
    queue: VecDeque<(FuncId, DefId, Vec<Ty>)>,
    globals: FxHashMap<(DefId, Vec<Ty>), FuncId>,
    strings: FxHashMap<String, u64>,
    locs: FxHashMap<Span, u64>,
    /// Debug file index of each source file named by `Stmt::Loc`.
    debug_files: FxHashMap<badger_base::FileId, u32>,
    static_closures: FxHashMap<FuncId, u64>,
    ctor_thunks: FxHashMap<(Ty, u32), FuncId>,
    nullary: FxHashMap<(Ty, u32), u64>,
    targs: FxHashMap<Vec<Ty>, u64>,
    pub errors: Vec<String>,
    fuse_sequences: bool,
    /// The instance whose body is being lowered (polymorphic recursion is
    /// a request for the same definition at a deeper type).
    current: Option<(DefId, Vec<Ty>)>,
    /// Fusion templates declared by `@fusion(role = ..., template = ...)`
    /// on any annotated item, keyed by definition path (a method shares
    /// its free function's path).
    fusion: FxHashMap<String, FusionTemplate>,
}

/// A `@fusion(...)` annotation: the kind of flow, and the values that
/// parameterize it (`mode` for `cursor`, `combine` for `accumulate`). Any
/// module may annotate a function (opt-in fusion); the recognizers still
/// require a matching shape and receiver type, so an unsuited marking
/// simply does not fuse.
#[derive(Clone, Debug)]
pub(crate) struct FusionTemplate {
    pub kind: String,
    pub params: Vec<(String, String)>,
}

impl FusionTemplate {
    /// The value the annotation gave a parameter (`mode = take`).
    pub(crate) fn param(&self, key: &str) -> Option<&str> {
        self.params.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
}

fn fusion_templates(g: &Global) -> FxHashMap<String, FusionTemplate> {
    let mut out = FxHashMap::default();
    for (def, attrs) in &g.hir.attributes {
        let m = &g.hir.modules[g.hir.defs[*def].module];
        for a in attrs {
            if g.interner.resolve(a.name) != "fusion" {
                continue;
            }
            let mut kind = None;
            let mut params = Vec::new();
            for arg in &a.args {
                let badger_syntax::ast::AttrArg::KeyValue(k, v) = arg else { continue };
                let key = g.interner.resolve(*k).to_string();
                let value = match &**v {
                    badger_syntax::ast::AttrArg::Name(s) => g.interner.resolve(*s).to_string(),
                    badger_syntax::ast::AttrArg::Str(s) => s.clone(),
                    _ => continue,
                };
                match key.as_str() {
                    "template" => kind = Some(value),
                    // the role is the kind's own, checked where it is read
                    "role" => {}
                    _ => params.push((key, value)),
                }
            }
            if let Some(kind) = kind {
                let mp: Vec<&str> = m.path.iter().map(|s| g.interner.resolve(*s)).collect();
                let path = format!("{}.{}", mp.join("."), g.interner.resolve(g.hir.defs[*def].name));
                out.insert(path, FusionTemplate { kind, params });
            }
        }
    }
    out
}

impl<'g, 'a> Mono<'g, 'a> {
    pub fn new(g: &'g Global<'a>, sources: &'g SourceMap) -> Self {
        Mono {
            g,
            lay: Layouts::new(g),
            sources,
            prog: Program::default(),
            instances: FxHashMap::default(),
            error_causes: FxHashMap::default(),
            queue: VecDeque::new(),
            globals: FxHashMap::default(),
            strings: FxHashMap::default(),
            locs: FxHashMap::default(),
            debug_files: FxHashMap::default(),
            static_closures: FxHashMap::default(),
            ctor_thunks: FxHashMap::default(),
            nullary: FxHashMap::default(),
            targs: FxHashMap::default(),
            errors: vec![],
            fuse_sequences: false,
            current: None,
            fusion: fusion_templates(g),
        }
    }

    fn sym(&self, s: Symbol) -> &str {
        self.g.interner.resolve(s)
    }

    fn def_path(&self, d: DefId) -> String {
        let def = &self.g.hir.defs[d];
        let m = &self.g.hir.modules[def.module];
        let mp: Vec<&str> = m.path.iter().map(|s| self.sym(*s)).collect();
        format!("{}.{}", mp.join("."), self.sym(def.name))
    }

    /// The live-reload identity of a definition: its path, and for a method
    /// the impl it belongs to (`app.<Point as Display>.display`), so methods
    /// of different impls never share it.
    fn live_path(&self, d: DefId) -> String {
        let hir = self.g.hir;
        if let DefKind::Fn(f) = &hir.defs[d].kind
            && let Some(o) = f.owner
            && let Some(imp) = self.g.env.impls.iter().find(|i| i.def == Some(o))
        {
            let m = &hir.modules[hir.defs[d].module];
            let mp: Vec<&str> = m.path.iter().map(|s| self.sym(*s)).collect();
            let ty = badger_typeck::show_ty(self.g, &imp.self_ty);
            let owner = match &imp.protocol {
                Some((p, _)) => format!("<{ty} as {}>", self.sym(hir.defs[*p].name)),
                None => format!("<{ty}>"),
            };
            return format!("{}.{owner}.{}", mp.join("."), self.sym(hir.defs[d].name));
        }
        self.def_path(d)
    }

    /// A constraint the runtime can satisfy for a `Dynamic` value through
    /// its descriptor.
    fn runtime_dispatchable(&self, c: &badger_typeck::solve::Constraint) -> bool {
        match c {
            badger_typeck::solve::Constraint::Impl { proto, .. } => {
                let n = self.g.interner.resolve(self.g.hir.defs[*proto].name);
                self.g.hir.modules[self.g.hir.defs[*proto].module].is_std
                    && matches!(
                        n,
                        "Display" | "Eq" | "PartialEq" | "PartialOrd" | "Hash" | "Transfer" | "Share"
                    )
            }
            badger_typeck::solve::Constraint::Display { .. } => true,
            _ => false,
        }
    }

    /// A stand-in for an instance beyond the specialization depth: calling
    /// it is a task defect.
    fn depth_stub(&mut self, def: DefId, args: Vec<Ty>) -> FuncId {
        let ft = self.g.scheme(def).map(|s| badger_typeck::env::subst_gen(&s.ty, &args));
        let (params, ret) = match ft.as_deref() {
            Some(TyKind::Fn(f)) => (
                f.params
                    .iter()
                    .map(|p| self.lay.repr(&badger_typeck::ground_resolved(p)))
                    .collect::<Vec<_>>(),
                self.lay.repr(&badger_typeck::ground_resolved(&f.ret)),
            ),
            _ => (vec![], Repr::I8),
        };
        let name = format!("{}#depth_limit", self.def_path(def));
        let mut f = Func::new(name, FuncKind::Normal, ret);
        for r in params {
            let l = f.locals.push(crate::mir::LocalDecl { repr: r, name: None });
            f.params.push(l);
        }
        let text = format!(
            "polymorphic recursion of `{}` exceeds the specialization depth ({SPECIALIZATION_DEPTH} nested type levels)",
            self.def_path(def)
        );
        if !self.prog.warnings.contains(&text) {
            self.prog.warnings.push(text.clone());
        }
        let msg = self.str_const(&text);
        let loc = self.str_const("<specialization>");
        let tmp = f.locals.push(crate::mir::LocalDecl { repr: Repr::I64, name: None });
        f.blocks[BlockId::new(0)].stmts.push(Stmt::Assign(
            tmp,
            Rvalue::Call {
                callee: Callee::Runtime("badger_rt_panic"),
                args: vec![Operand::Const(Const::Addr(msg)), Operand::Const(Const::Addr(loc))],
                ret: Repr::I64,
            },
        ));
        f.blocks[BlockId::new(0)].term = Some(Term::Unreachable);
        let fid = self.prog.funcs.push(f);
        self.instances.insert((def, args), fid);
        fid
    }

    /// Request a function instance.
    pub fn instance(&mut self, def: DefId, args: Vec<Ty>) -> FuncId {
        if let Some(f) = self.instances.get(&(def, args.clone())) {
            return *f;
        }
        // polymorphic recursion: the body of `def` asks for `def` at deeper
        // types. A function without protocol constraints switches to one
        // instance whose growing arguments are `Dynamic` (values carry their
        // own runtime descriptors: the typed dictionary fallback).
        if let Some((cur_def, cur_args)) = self.current.clone()
            && cur_def == def
            && cur_args.len() == args.len()
            && args != cur_args
            && args.iter().zip(&cur_args).any(|(a, c)| a != c && type_depth(a) > type_depth(c))
            && self.g.scheme(def).is_some_and(|s| s.constraints.iter().all(|c| self.runtime_dispatchable(c)))
            && let Some(dynamic) = self.g.hir.try_lang("Dynamic")
        {
            let dy = badger_typeck::ty::mk(TyKind::Con(dynamic, vec![]));
            let erased: Vec<Ty> = args
                .iter()
                .zip(&cur_args)
                .map(|(a, c)| if a != c { dy.clone() } else { a.clone() })
                .collect();
            if erased != args {
                return self.instance(def, erased);
            }
        }
        // past the specialization limit an instance is a stub that fails at
        // run time
        if args.iter().any(|a| type_depth(a) > SPECIALIZATION_DEPTH) {
            return self.depth_stub(def, args);
        }
        let scheme = self.g.scheme(def);
        let ret = scheme
            .as_ref()
            .and_then(|s| match &*badger_typeck::env::subst_gen(&s.ty, &args) {
                TyKind::Fn(f) => Some(badger_typeck::ground_resolved(&f.ret)),
                _ => None,
            })
            .map(|t| self.lay.repr(&t))
            .unwrap_or(Repr::I8);
        let name = format!(
            "{}{}",
            self.def_path(def),
            if args.is_empty() { String::new() } else { format!("#{}", self.instances.len()) }
        );
        let fid = self.prog.funcs.push(Func::new(name, FuncKind::Normal, ret));
        self.set_live_identity(fid, def, &args, false);
        self.instances.insert((def, args.clone()), fid);
        self.queue.push_back((fid, def, args));
        fid
    }

    /// Stable identity of a user instance across recompilations (live reload).
    fn set_live_identity(&mut self, fid: FuncId, def: DefId, args: &[Ty], global: bool) {
        if self.g.hir.modules[self.g.hir.defs[def].module].is_std {
            return;
        }
        // effect-row arguments do not name a distinct instance: an effect
        // change is a signature change of the same function (incompatible)
        let scheme = self.g.scheme(def);
        let value_gens: Vec<bool> = (0..args.len())
            .map(|i| scheme.as_ref().is_none_or(|s| gen_in_value_position(&s.ty, i as u32)))
            .collect();
        let targs: Vec<String> = args
            .iter()
            .zip(&value_gens)
            .filter(|(_, v)| **v)
            .map(|(a, _)| badger_typeck::show_ty(self.g, a))
            .collect();
        let key =
            format!("{}{}<{}>", self.live_path(def), if global { "#global" } else { "" }, targs.join(", "));
        let sig = scheme
            .map(|s| {
                // the declared (generic) signature with its concrete effects:
                // the same for every instance sharing this key
                badger_typeck::show_ty(self.g, &s.ty)
            })
            .unwrap_or_default();
        let f = &mut self.prog.funcs[fid];
        f.live_key = Some(key);
        f.live_sig = sig;
        f.is_global = global;
    }

    /// Request a global getter instance.
    pub fn global(&mut self, def: DefId, args: Vec<Ty>) -> FuncId {
        if let Some(f) = self.globals.get(&(def, args.clone())) {
            return *f;
        }
        let ty = self
            .g
            .scheme(def)
            .map(|s| badger_typeck::ground_resolved(&badger_typeck::env::subst_gen(&s.ty, &args)))
            .unwrap_or_else(badger_typeck::ty::unit);
        let ret = self.lay.repr(&ty);
        let fid =
            self.prog.funcs.push(Func::new(format!("{}#global", self.def_path(def)), FuncKind::Normal, ret));
        self.set_live_identity(fid, def, &args, true);
        self.globals.insert((def, args.clone()), fid);
        self.queue.push_back((fid, def, args));
        fid
    }

    /// Lower everything reachable from the queued instances, then register
    /// user `Display` implementations on the descriptors of their types so
    /// runtime-generic display (derived `Error`, nested values) uses them.
    pub fn run(&mut self) {
        let mut hooked: rustc_hash::FxHashSet<usize> = rustc_hash::FxHashSet::default();
        loop {
            while let Some((fid, def, args)) = self.queue.pop_front() {
                self.current = Some((def, args.clone()));
                self.lower_instance(fid, def, args);
                self.current = None;
            }
            let mut added = false;
            for (ty, desc) in self.lay.all() {
                if !hooked.insert(desc.id) {
                    continue;
                }
                if let Some(fid) = self.display_impl(&ty) {
                    self.prog.funcs[fid].needs_adapter = true;
                    self.prog.display_hooks.push((desc, fid));
                    added = true;
                }
            }
            if !added && self.queue.is_empty() {
                break;
            }
        }
    }

    /// Instance of the user-written `Display.display` for a nominal type.
    /// A protocol method not written in the implementation: the protocol's
    /// default body, or the derived `Error` cause, as a function of `self`.
    fn protocol_default(
        &mut self,
        t: &Ty,
        protocol: DefId,
        pargs: &[Ty],
        method: Symbol,
        ret_ty: &Ty,
    ) -> Option<FuncId> {
        if protocol == self.g.env.b.error && self.sym(method) == "cause" {
            return Some(self.error_cause_fn(t, ret_ty));
        }
        let pm = *self.g.env.protocols.get(&protocol)?.methods.get(&method)?;
        if !matches!(&self.g.hir.defs[pm].kind, DefKind::Fn(f) if f.body.is_some()) {
            return None;
        }
        let scheme = self.g.scheme(pm)?;
        let mut args = vec![t.clone()];
        args.extend_from_slice(pargs);
        while (args.len() as u32) < scheme.n {
            args.push(badger_typeck::ty::unit());
        }
        Some(self.instance(pm, args))
    }

    /// `fn(self) -> Option<dyn Error>` for a derived `Error` type.
    fn error_cause_fn(&mut self, t: &Ty, ret_ty: &Ty) -> FuncId {
        let key = (self.g.env.b.error, vec![t.clone(), ret_ty.clone()]);
        if let Some(f) = self.error_causes.get(&key) {
            return *f;
        }
        let name = format!("{}.cause", self.lay.type_name(t));
        let fid = self.prog.funcs.push(Func::new(name, FuncKind::Normal, Repr::Ptr));
        self.error_causes.insert(key, fid);
        let owner = Owner::Fn(self.g.env.b.error);
        let mut fl =
            FnLower::new(self, Rc::new(badger_typeck::Instance::default()), owner, ret_ty.clone(), fid);
        let p = fl.func.locals.push(LocalDecl { repr: Repr::Ptr, name: Some("self".into()) });
        fl.func.params = vec![p];
        let r = fl.error_cause(t, Operand::Local(p), ret_ty);
        fl.term(Term::Return(r));
        fl.finish();
        fid
    }

    pub fn has_impl_pub(&self, proto: DefId, ty: &Ty) -> bool {
        self.has_impl(proto, ty, 0)
    }

    /// Does a ground type implement `proto`? (Nominal types are checked
    /// against their implementations; structural types are assumed to,
    /// except for `Display`, which only unit has among them.)
    fn has_impl(&self, proto: DefId, ty: &Ty, depth: u32) -> bool {
        let TyKind::Con(head, _) = &**ty else {
            return proto != self.g.env.b.display || matches!(&**ty, TyKind::Tuple(ts) if ts.is_empty());
        };
        if depth > 16 {
            return true;
        }
        let Some(idxs) = self.g.env.impls_by_head.get(head) else { return false };
        idxs.iter().any(|&i| {
            let imp = &self.g.env.impls[i];
            if imp.protocol.as_ref().map(|p| p.0) != Some(proto) {
                return false;
            }
            let mut binds: Vec<Option<Ty>> = vec![None; imp.n_generics as usize];
            if !match_ty(&imp.self_ty, ty, &mut binds) {
                return false;
            }
            let full: Vec<Ty> =
                binds.into_iter().map(|b| b.unwrap_or_else(badger_typeck::ty::unit)).collect();
            imp.preds
                .iter()
                .all(|(p, _, subj)| self.has_impl(*p, &badger_typeck::env::subst_gen(subj, &full), depth + 1))
        })
    }

    fn display_impl(&mut self, ty: &Ty) -> Option<FuncId> {
        let TyKind::Con(head, _) = &**ty else { return None };
        if !self.g.env.structs.contains_key(head) && !self.g.env.enums.contains_key(head) {
            return None;
        }
        let display = self.g.env.b.display;
        let name = self.g.interner.intern("display");
        let idxs = self.g.env.impls_by_head.get(head)?.clone();
        for i in idxs {
            let imp = &self.g.env.impls[i];
            if imp.protocol.as_ref().map(|p| p.0) != Some(display) {
                continue;
            }
            let badger_typeck::env::ImplMethods::Defs(ms) = &imp.methods else { return None };
            let m = *ms.get(&name)?;
            let mut binds: Vec<Option<Ty>> = vec![None; imp.n_generics as usize];
            if !match_ty(&imp.self_ty, ty, &mut binds) {
                continue;
            }
            // conditional implementations apply only when their bounds hold
            let preds = imp.preds.clone();
            let holds = preds.iter().all(|(p, _, subj)| {
                let full: Vec<Ty> =
                    binds.iter().map(|b| b.clone().unwrap_or_else(badger_typeck::ty::unit)).collect();
                self.has_impl(*p, &badger_typeck::env::subst_gen(subj, &full), 0)
            });
            if !holds {
                return None;
            }
            let scheme = self.g.scheme(m)?;
            let mut args: Vec<Ty> =
                binds.into_iter().map(|b| b.unwrap_or_else(badger_typeck::ty::unit)).collect();
            while (args.len() as u32) < scheme.n {
                args.push(badger_typeck::ty::unit());
            }
            return Some(self.instance(m, args));
        }
        None
    }

    fn lower_instance(&mut self, fid: FuncId, def: DefId, args: Vec<Ty>) {
        let inst = badger_typeck::check_instance(self.g, def, &args);
        if std::env::var_os("BADGER_DEBUG_INSTANCE").is_some() {
            let names: Vec<String> = args.iter().map(|a| self.lay.type_name(a)).collect();
            eprintln!("instance {} [{}]", self.def_path(def), names.join(", "));
        }
        for e in &inst.errors {
            self.errors.push(format!("{}: {}", self.def_path(def), e.message));
        }
        let inst = Rc::new(inst);
        let hir = self.g.hir;
        match &hir.defs[def].kind {
            DefKind::Fn(f) => {
                let Some(body) = f.body else { return };
                let ret_ty = match inst.fn_ty.as_deref() {
                    Some(TyKind::Fn(ft)) => ft.ret.clone(),
                    _ => badger_typeck::ty::unit(),
                };
                let mut fl = FnLower::new(self, inst.clone(), Owner::Fn(def), ret_ty, fid);
                let mut params = vec![];
                for p in &f.params {
                    let t = fl.pat_ty(p.pat);
                    let l = fl.fresh(fl.repr(&t), None);
                    params.push((l, p.pat, t));
                }
                fl.func.params = params.iter().map(|p| p.0).collect();
                fl.emit(Stmt::Poll);
                // `badger contracts --tests`: integer arguments as observed
                let observed = observing(def);
                if observed {
                    for (k, (l, _, t)) in params.iter().enumerate() {
                        if fl.m.lay.num_ty(t) == Some(NumTy::I(64, true)) {
                            fl.rt(
                                "badger_rt_observe_int",
                                vec![int(slot(def, k)), Operand::Local(*l)],
                                Repr::I64,
                            );
                        }
                    }
                }
                for (l, pat, t) in params {
                    fl.bind_irrefutable(pat, Operand::Local(l), &t);
                }
                let observe_result = observed && fl.m.lay.num_ty(&fl.ret_ty) == Some(NumTy::I(64, true));
                if !f.ensures.is_empty() || observe_result {
                    let rl = fl.fresh(fl.func.ret, Some("result".into()));
                    let epi = fl.block();
                    fl.epilogue = Some((epi, rl));
                }
                let v = fl.lower(body);
                if let Some(v) = v {
                    fl.ret(v);
                }
                // postconditions, on every return path
                if let Some((epi, rl)) = fl.epilogue.take() {
                    fl.switch(epi);
                    if let Some(r) = f.result {
                        fl.locals.insert(r, rl);
                    }
                    if observe_result {
                        fl.rt(
                            "badger_rt_observe_int",
                            vec![int(slot(def, RESULT_SLOT)), Operand::Local(rl)],
                            Repr::I64,
                        );
                    }
                    for (k, pc) in f.ensures.iter().enumerate() {
                        if elided_post(def, k) {
                            continue;
                        }
                        let Some(c) = fl.lower(pc.cond) else { continue };
                        if let Some(id) = observed_post(def, k) {
                            fl.rt("badger_rt_observe_clause", vec![int(id as i64), c], Repr::I64);
                        }
                        let (ok, bad) = (fl.block(), fl.block());
                        fl.term(Term::Branch(c, ok, bad));
                        fl.switch(bad);
                        let msg = fl.m.str_const(&pc.message);
                        let l = fl.loc(pc.span);
                        fl.rt(
                            "badger_rt_panic",
                            vec![Operand::Const(Const::Addr(msg)), Operand::Const(Const::Addr(l))],
                            Repr::I64,
                        );
                        fl.dead();
                        fl.switch(ok);
                    }
                    fl.term(Term::Return(Operand::Local(rl)));
                }
                fl.finish();
            }
            DefKind::Global(gd) => {
                // getter with lazy initialization
                let slot = self.prog.n_globals;
                self.prog.n_globals += 2;
                let ret_ty = badger_typeck::ty::unit();
                let mut fl = FnLower::new(self, inst.clone(), Owner::Global(def), ret_ty, fid);
                if fl.func.ret == Repr::Ptr {
                    fl.m.prog.ptr_globals.push(slot);
                }
                // flag: 0 uninitialized, 1 initializing, 2 ready; the slow
                // path lets exactly one task initialize while others wait
                let flag = fl.assign(Repr::I64, Rvalue::GlobalLoad(slot + 1));
                let ready = fl.assign(
                    Repr::I8,
                    Rvalue::Cmp {
                        op: CmpOp::Eq,
                        a: flag,
                        b: Operand::Const(Const::Int(2, Repr::I64)),
                        ty: NumTy::I(64, true),
                    },
                );
                let (done, slow, init) = (fl.block(), fl.block(), fl.block());
                fl.term(Term::Branch(ready, done, slow));
                fl.switch(slow);
                let addr = fl.assign(Repr::I64, Rvalue::GlobalAddr(slot + 1));
                let go = fl.assign(
                    Repr::I64,
                    Rvalue::Call {
                        callee: Callee::Runtime("badger_rt_global_init"),
                        args: vec![addr],
                        ret: Repr::I64,
                    },
                );
                fl.term(Term::Branch(go, init, done));
                fl.switch(init);
                if let Some(v) = fl.lower(gd.value) {
                    fl.emit(Stmt::GlobalStore { slot, value: v });
                    let addr = fl.assign(Repr::I64, Rvalue::GlobalAddr(slot + 1));
                    fl.assign(
                        Repr::I64,
                        Rvalue::Call {
                            callee: Callee::Runtime("badger_rt_global_done"),
                            args: vec![addr],
                            ret: Repr::I64,
                        },
                    );
                }
                fl.term(Term::Goto(done));
                fl.switch(done);
                let r = fl.assign(fl.func.ret, Rvalue::GlobalLoad(slot));
                fl.term(Term::Return(r));
                fl.finish();
            }
            _ => {}
        }
    }

    pub fn str_const(&mut self, s: &str) -> u64 {
        if let Some(&w) = self.strings.get(s) {
            return w;
        }
        let w = badger_runtime::string::immortal_string(s);
        self.strings.insert(s.to_string(), w);
        self.prog.origins.strings.insert(w, s.to_string());
        w
    }

    /// An interned symbol's address.
    pub fn symbol(&mut self, name: &str) -> u64 {
        let w = badger_runtime::string::symbol(name);
        self.prog.origins.symbols.insert(w, name.to_string());
        w
    }

    /// `(file index, line, column)` of a span in a user source file.
    pub fn debug_pos(&mut self, span: Span) -> Option<(u32, u32, u32)> {
        let f = self.sources.get(span.file);
        if f.name.starts_with('<') {
            return None;
        }
        let (l, c) = f.line_col(span.start as usize);
        let name = f.name.clone();
        let idx = match self.debug_files.get(&span.file) {
            Some(i) => *i,
            None => {
                let i = self.prog.debug_files.len() as u32;
                self.prog.debug_files.push(name);
                self.debug_files.insert(span.file, i);
                i
            }
        };
        Some((idx, l as u32, c as u32))
    }

    pub fn loc(&mut self, span: Span) -> u64 {
        if let Some(&w) = self.locs.get(&span) {
            return w;
        }
        let text = if span.is_dummy() {
            "<unknown>".to_string()
        } else {
            let f = self.sources.get(span.file);
            let (l, c) = f.line_col(span.start as usize);
            format!("{}:{l}:{c}", f.name)
        };
        let w = badger_runtime::string::immortal_string(&text);
        self.locs.insert(span, w);
        self.prog.origins.strings.insert(w, text);
        w
    }

    pub fn static_closure(&mut self, fid: FuncId) -> u64 {
        if let Some(&w) = self.static_closures.get(&fid) {
            return w;
        }
        self.prog.funcs[fid].needs_adapter = true;
        let layout = self.lay.closure(&[]);
        let desc = self.lay.closure_desc(&layout);
        let w = badger_runtime::gc::alloc_immortal(desc, layout.size as usize);
        self.prog.origins.closures.insert(w, (desc as *const TypeDesc as u64, layout.size, fid));
        self.prog.static_closures.push((w, fid));
        self.static_closures.insert(fid, w);
        w
    }

    pub fn targs_ptr(&mut self, args: &[Ty]) -> u64 {
        if let Some(&p) = self.targs.get(args) {
            return p;
        }
        let v: Vec<&'static TypeDesc> = args.iter().map(|a| self.lay.desc(a)).collect();
        let v = Box::leak(v.into_boxed_slice());
        let p = v.as_ptr() as u64;
        self.prog.origins.targs.insert(p, v.iter().map(|d| *d as *const TypeDesc as u64).collect());
        self.targs.insert(args.to_vec(), p);
        p
    }

    fn nullary_variant(&mut self, ty: &Ty, idx: u32) -> u64 {
        if let Some(&w) = self.nullary.get(&(ty.clone(), idx)) {
            return w;
        }
        let desc = self.lay.desc(ty);
        let w = badger_runtime::gc::alloc_immortal(desc, desc.payload_size() as usize);
        unsafe { *(badger_runtime::gc::obj(w).payload() as *mut u32) = idx };
        self.prog.origins.nullary.insert(w, (desc as *const TypeDesc as u64, idx));
        self.nullary.insert((ty.clone(), idx), w);
        w
    }

    /// Function wrapping an enum constructor so it can be used as a value.
    fn ctor_thunk(&mut self, ty: &Ty, idx: u32) -> FuncId {
        if let Some(&f) = self.ctor_thunks.get(&(ty.clone(), idx)) {
            return f;
        }
        let ret = self.lay.repr(ty);
        let fid = self.prog.funcs.push(Func::new(
            format!("ctor.{}#{idx}", self.lay.type_name(ty)),
            FuncKind::Lambda,
            ret,
        ));
        self.ctor_thunks.insert((ty.clone(), idx), fid);
        let (_, vs) = self.lay.enum_variants(ty);
        let fields = vs[idx as usize].1.clone();
        let mut f = Func::new(self.prog.funcs[fid].name.clone(), FuncKind::Lambda, ret);
        let params: Vec<Local> = fields
            .iter()
            .map(|fl| f.locals.push(LocalDecl { repr: fl.repr, name: Some(fl.name.clone()) }))
            .collect();
        f.params = params.clone();
        let desc = self.lay.desc(ty);
        let obj = f.locals.push(LocalDecl { repr: Repr::Ptr, name: None });
        let b = &mut f.blocks[BlockId::new(0)];
        b.stmts.push(Stmt::Assign(
            obj,
            Rvalue::Alloc { desc: desc as *const _ as u64, size: desc.payload_size() },
        ));
        b.stmts.push(Stmt::Store {
            base: Operand::Local(obj),
            offset: 0,
            repr: Repr::I32,
            value: Operand::Const(Const::Int(idx as i64, Repr::I32)),
        });
        for (fl, p) in fields.iter().zip(params) {
            b.stmts.push(Stmt::Store {
                base: Operand::Local(obj),
                offset: fl.offset,
                repr: fl.repr,
                value: Operand::Local(p),
            });
        }
        b.term = Some(Term::Return(Operand::Local(obj)));
        f.needs_adapter = true;
        self.prog.funcs[fid] = f;
        fid
    }

    /// Thunk `fn(boxed self, args...) -> R` dispatching protocol method
    /// `method` to `T`'s implementation (used in `dyn` dictionaries).
    pub fn dyn_thunk(&mut self, t: &Ty, protocol: DefId, pargs: &[Ty], method: Symbol) -> FuncId {
        // the method may come from a supertrait (then without its arguments)
        let (protocol, pargs): (DefId, Vec<Ty>) =
            if self.g.env.protocols[&protocol].methods.contains_key(&method) {
                (protocol, pargs.to_vec())
            } else {
                let mut stack = self.g.env.protocols[&protocol].supers.clone();
                let mut found = protocol;
                while let Some(q) = stack.pop() {
                    if self.g.env.protocols.get(&q).is_some_and(|i| i.methods.contains_key(&method)) {
                        found = q;
                        break;
                    }
                    stack.extend(self.g.env.protocols.get(&q).map(|i| i.supers.clone()).unwrap_or_default());
                }
                (found, vec![])
            };
        let pargs = &pargs[..];
        let pm = self.g.env.protocols[&protocol].methods[&method];
        let scheme = self.g.scheme(pm).unwrap();
        let mut sargs = vec![t.clone()];
        sargs.extend_from_slice(pargs);
        while (sargs.len() as u32) < scheme.n {
            sargs.push(badger_typeck::ty::unit());
        }
        let fty = badger_typeck::ground_resolved(&badger_typeck::env::subst_gen(&scheme.ty, &sargs));
        let TyKind::Fn(ft) = &*fty else { unreachable!() };
        let ret = self.lay.repr(&ft.ret);
        let name = format!("dyn.{}.{}.{}", self.lay.type_name(t), self.def_path(protocol), self.sym(method));
        let fid = self.prog.funcs.push(Func::new(name.clone(), FuncKind::Lambda, ret));
        let mut f = Func::new(name, FuncKind::Lambda, ret);
        let data = f.locals.push(LocalDecl { repr: Repr::Ptr, name: Some("data".into()) });
        let mut params = vec![data];
        for p in &ft.params[1..] {
            params.push(f.locals.push(LocalDecl { repr: self.lay.repr(p), name: None }));
        }
        f.params = params.clone();
        let self_repr = self.lay.repr(t);
        let sv = f.locals.push(LocalDecl { repr: self_repr, name: Some("self".into()) });
        let r = f.locals.push(LocalDecl { repr: ret, name: None });
        let mut stmts =
            vec![Stmt::Assign(sv, Rvalue::Load { base: Operand::Local(data), offset: 0, repr: self_repr })];
        let mut cargs = vec![Operand::Local(sv)];
        cargs.extend(params[1..].iter().map(|l| Operand::Local(*l)));
        let callee = match self.protocol_impl(t, protocol, method) {
            Some(f) => Some(f),
            None => self.protocol_default(t, protocol, pargs, method, &ft.ret),
        };
        let rv = match callee {
            Some(target) => Rvalue::Call { callee: Callee::Direct(target), args: cargs, ret },
            None => {
                // derived Display (schema-driven)
                let d = self.lay.desc(t) as *const TypeDesc as u64;
                Rvalue::Call {
                    callee: Callee::Runtime("badger_rt_display"),
                    args: vec![Operand::Const(Const::Addr(d)), Operand::Local(sv)],
                    ret,
                }
            }
        };
        stmts.push(Stmt::Assign(r, rv));
        let b = &mut f.blocks[BlockId::new(0)];
        b.stmts = stmts;
        b.term = Some(Term::Return(Operand::Local(r)));
        f.needs_adapter = true;
        self.prog.funcs[fid] = f;
        fid
    }

    /// Instance of `T`'s implementation of a protocol method, if written.
    fn protocol_impl(&mut self, ty: &Ty, protocol: DefId, method: Symbol) -> Option<FuncId> {
        let TyKind::Con(head, _) = &**ty else { return None };
        let idxs = self.g.env.impls_by_head.get(head)?.clone();
        for i in idxs {
            let imp = &self.g.env.impls[i];
            if imp.protocol.as_ref().map(|p| p.0) != Some(protocol) {
                continue;
            }
            let badger_typeck::env::ImplMethods::Defs(ms) = &imp.methods else { return None };
            let m = *ms.get(&method)?;
            let mut binds: Vec<Option<Ty>> = vec![None; imp.n_generics as usize];
            if !match_ty(&imp.self_ty, ty, &mut binds) {
                continue;
            }
            // conditional implementations apply only when their bounds hold
            let preds = imp.preds.clone();
            let holds = preds.iter().all(|(p, _, subj)| {
                let full: Vec<Ty> =
                    binds.iter().map(|b| b.clone().unwrap_or_else(badger_typeck::ty::unit)).collect();
                self.has_impl(*p, &badger_typeck::env::subst_gen(subj, &full), 0)
            });
            if !holds {
                return None;
            }
            let scheme = self.g.scheme(m)?;
            let mut args: Vec<Ty> =
                binds.into_iter().map(|b| b.unwrap_or_else(badger_typeck::ty::unit)).collect();
            while (args.len() as u32) < scheme.n {
                args.push(badger_typeck::ty::unit());
            }
            return Some(self.instance(m, args));
        }
        None
    }

    /// Instance of a standard-library function by lang-item name.
    pub fn lang_fn(&mut self, name: &str, args: Vec<Ty>) -> Option<FuncId> {
        let d = self.g.hir.try_lang(name)?;
        Some(self.instance(d, args))
    }
}

struct LoopCtx {
    expr: ExprId,
    cont: BlockId,
    brk: BlockId,
    result: Option<Local>,
    /// Closer of a sequence loop's iterator, run when control leaves early.
    close: Option<(FuncId, Local)>,
}

pub struct FnLower<'m, 'g, 'a> {
    m: &'m mut Mono<'g, 'a>,
    inst: Rc<Instance>,
    pub func: Func,
    fid: FuncId,
    cur: BlockId,
    locals: FxHashMap<LocalId, Local>,
    stored_sequences: FxHashMap<LocalId, fusion::Prepared>,
    sequence_locals: Option<rustc_hash::FxHashSet<LocalId>>,
    loops: Vec<LoopCtx>,
    ret_ty: Ty,
    owner: Owner,
    /// With postconditions: every return stores its value in the local and
    /// jumps to the block that checks them.
    epilogue: Option<(BlockId, Local)>,
}

fn unit_op() -> Operand {
    Operand::Const(Const::Int(0, Repr::I8))
}

fn int(v: i64) -> Operand {
    Operand::Const(Const::Int(v, Repr::I64))
}

impl<'m, 'g, 'a> FnLower<'m, 'g, 'a> {
    fn new(m: &'m mut Mono<'g, 'a>, inst: Rc<Instance>, owner: Owner, ret_ty: Ty, fid: FuncId) -> Self {
        let f = &m.prog.funcs[fid];
        let mut func = Func::new(f.name.clone(), f.kind, f.ret);
        func.live_key = f.live_key.clone();
        func.live_sig = f.live_sig.clone();
        func.is_global = f.is_global;
        FnLower {
            m,
            inst,
            func,
            fid,
            cur: BlockId::new(0),
            locals: FxHashMap::default(),
            stored_sequences: FxHashMap::default(),
            sequence_locals: None,
            loops: vec![],
            ret_ty,
            owner,
            epilogue: None,
        }
    }

    /// Return `v` from the function (through its postconditions, if any).
    fn ret(&mut self, v: Operand) {
        match self.epilogue {
            Some((b, l)) => {
                self.emit(Stmt::Assign(l, Rvalue::Use(v)));
                self.term(Term::Goto(b));
            }
            None => self.term(Term::Return(v)),
        }
    }

    fn finish(mut self) {
        // terminate any open block
        for b in self.func.blocks.iter_mut() {
            if b.term.is_none() {
                b.term = Some(Term::Unreachable);
            }
        }
        let needs = self.m.prog.funcs[self.fid].needs_adapter;
        self.func.needs_adapter |= needs;
        let fid = self.fid;
        if self.func.kind == FuncKind::Normal {
            tail_calls(&mut self.func, fid);
        }
        crate::sra::scalar_replace(&mut self.func);
        self.m.prog.funcs[fid] = self.func;
    }

    fn hir(&self) -> &'g badger_hir::Hir {
        self.m.g.hir
    }

    /// The representation of `e` as lowered: a coerced value (`dyn`, `Seq`,
    /// `Data`, `Bytes`) is a managed pointer whatever the source type, and
    /// must be kept in a pointer (rooted) local.
    fn lowered_repr(&self, e: ExprId) -> Repr {
        if self.inst.coercions.contains_key(&e) { Repr::Ptr } else { self.repr(&self.ty(e)) }
    }

    pub fn ty(&self, e: ExprId) -> Ty {
        self.inst.exprs.get(&e).cloned().unwrap_or_else(badger_typeck::ty::unit)
    }

    pub fn pat_ty(&self, p: PatId) -> Ty {
        self.inst.pats.get(&p).cloned().unwrap_or_else(badger_typeck::ty::unit)
    }

    pub fn repr(&self, t: &Ty) -> Repr {
        self.m.lay.repr(t)
    }

    pub fn fresh(&mut self, repr: Repr, name: Option<String>) -> Local {
        self.func.locals.push(LocalDecl { repr, name })
    }

    pub fn emit(&mut self, s: Stmt) {
        self.func.blocks[self.cur].stmts.push(s);
    }

    pub fn assign(&mut self, repr: Repr, rv: Rvalue) -> Operand {
        let l = self.fresh(repr, None);
        self.emit(Stmt::Assign(l, rv));
        Operand::Local(l)
    }

    pub fn block(&mut self) -> BlockId {
        self.func.blocks.push(Block::default())
    }

    pub fn switch(&mut self, b: BlockId) {
        self.cur = b;
    }

    pub fn term(&mut self, t: Term) {
        let b = &mut self.func.blocks[self.cur];
        if b.term.is_none() {
            b.term = Some(t);
        }
    }

    /// After a diverging expression: continue in a fresh unreachable block.
    fn dead(&mut self) {
        self.term(Term::Unreachable);
        let b = self.block();
        self.switch(b);
    }

    fn loc(&mut self, span: Span) -> u64 {
        self.m.loc(span)
    }

    fn span(&self, e: ExprId) -> Span {
        self.hir().exprs[e].span
    }

    fn call(&mut self, callee: Callee, args: Vec<Operand>, ret: Repr) -> Operand {
        // an intrinsic returning a managed value always lands in a rooted
        // local, whatever word representation the call site asked for
        let ret = match &callee {
            Callee::Intrinsic { ret: rd, .. }
                if ret == Repr::I64 && *rd != 0 && unsafe { &*(*rd as *const TypeDesc) }.is_ptr() =>
            {
                Repr::Ptr
            }
            _ => ret,
        };
        self.assign(ret, Rvalue::Call { callee, args, ret })
    }

    /// Whether the function being lowered is standard-library code.
    fn in_std(&self) -> bool {
        let hir = self.hir();
        match self.owner {
            Owner::Fn(d) | Owner::Global(d) => hir.modules[hir.defs[d].module].is_std,
            Owner::Lambda(e) => {
                let f = hir.exprs[e].span.file;
                hir.modules.iter().any(|(_, m)| m.is_std && m.file == f)
            }
        }
    }

    /// The prelude's `err` helper called from user code (an error made
    /// inside the standard library keeps its origin at the user's call).
    fn is_prelude_err(&self, def: DefId) -> bool {
        let hir = self.hir();
        let m = &hir.modules[hir.defs[def].module];
        m.is_std
            && !self.in_std()
            && m.path.len() == 2
            && self.m.g.interner.resolve(m.path[1]) == "prelude"
            && self.m.g.interner.resolve(hir.defs[def].name) == "err"
    }

    /// A function with `requires` clauses (its failures blame the caller).
    fn has_requires(&self, def: DefId) -> bool {
        matches!(&self.hir().defs[def].kind, DefKind::Fn(f) if f.contracts.iter().any(|c| c.starts_with("requires")))
    }

    /// A `std.test` function called from outside `std.test`.
    fn is_test_helper(&self, def: DefId, call: Span) -> bool {
        let hir = self.hir();
        let m = &hir.modules[hir.defs[def].module];
        m.is_std && m.path.len() == 2 && self.m.g.interner.resolve(m.path[1]) == "test" && call.file != m.file
    }

    /// `Buffer`/`FixedBuffer` `get`/`set` called from user code.
    fn is_checked_element_access(&self, def: DefId, call: Span) -> bool {
        let hir = self.hir();
        let d = &hir.defs[def];
        let m = &hir.modules[d.module];
        if !m.is_std || call.file == m.file {
            return false;
        }
        let name = self.m.g.interner.resolve(d.name);
        let owner = match &d.kind {
            DefKind::Fn(f) => f.owner.and_then(|o| match &hir.defs[o].kind {
                DefKind::Impl(i) => match &i.self_ty.kind {
                    badger_hir::HTyKind::Named { def, .. } => {
                        Some(self.m.g.interner.resolve(hir.defs[*def].name))
                    }
                    _ => None,
                },
                _ => None,
            }),
            _ => None,
        };
        matches!(name, "get" | "set") && matches!(owner, Some("Buffer" | "FixedBuffer"))
    }

    /// Mark the source position of the code that follows (user code only).
    fn mark(&mut self, span: Span) {
        if span.is_dummy() {
            return;
        }
        if let Some((file, line, col)) = self.m.debug_pos(span) {
            self.emit(Stmt::Loc { file, line, col });
        }
    }

    fn rt(&mut self, name: &'static str, args: Vec<Operand>, ret: Repr) -> Operand {
        self.call(Callee::Runtime(name), args, ret)
    }

    fn defect(&mut self, kind: i64, span: Span) {
        let l = self.loc(span);
        self.rt("badger_rt_defect", vec![int(kind), Operand::Const(Const::Addr(l))], Repr::I64);
        self.dead();
    }

    fn desc_op(&mut self, t: &Ty) -> Operand {
        Operand::Const(Const::Addr(self.m.lay.desc(t) as *const TypeDesc as u64))
    }

    fn copy_to_local(&mut self, v: Operand, repr: Repr, name: Option<String>) -> Local {
        let l = self.fresh(repr, name);
        self.emit(Stmt::Assign(l, Rvalue::Use(v)));
        l
    }

    // ------------------------------------------------------------ values

    fn alloc_enum(&mut self, ty: &Ty, idx: u32, fields: Vec<Operand>) -> Operand {
        if fields.is_empty() {
            let w = self.m.nullary_variant(ty, idx);
            return Operand::Const(Const::Addr(w));
        }
        let desc = self.m.lay.desc(ty);
        let (_, vs) = self.m.lay.enum_variants(ty);
        // every value of a variant whose payload is all unit (`ok(())`) is
        // the same: share one static object (not `err`: error origins are
        // tracked by the error value's identity)
        let (name, fls) = &vs[idx as usize];
        if name != "err" && fls.iter().all(|f| matches!(&*f.ty, TyKind::Tuple(ts) if ts.is_empty())) {
            let w = self.m.nullary_variant(ty, idx);
            return Operand::Const(Const::Addr(w));
        }
        let obj = self
            .assign(Repr::Ptr, Rvalue::Alloc { desc: desc as *const _ as u64, size: desc.payload_size() });
        self.emit(Stmt::Store {
            base: obj,
            offset: 0,
            repr: Repr::I32,
            value: Operand::Const(Const::Int(idx as i64, Repr::I32)),
        });
        for (fl, v) in vs[idx as usize].1.iter().zip(fields) {
            self.emit(Stmt::Store { base: obj, offset: fl.offset, repr: fl.repr, value: v });
        }
        obj
    }

    fn variant_index(&self, ty: &Ty, name: &str) -> u32 {
        let (_, vs) = self.m.lay.enum_variants(ty);
        vs.iter().position(|v| v.0 == name).unwrap_or(0) as u32
    }

    /// Load field `i` of variant `idx` from an enum value.
    fn variant_field(&mut self, ty: &Ty, idx: u32, i: usize, v: Operand) -> (Operand, Ty) {
        let (_, vs) = self.m.lay.enum_variants(ty);
        let fl = vs[idx as usize].1[i].clone();
        let r = self.assign(fl.repr, Rvalue::Load { base: v, offset: fl.offset, repr: fl.repr });
        (r, fl.ty)
    }

    fn load_field(&mut self, ty: &Ty, label: Symbol, v: Operand) -> Operand {
        let l = self.m.lay.record(ty);
        match l.field(label) {
            Some(f) => {
                let (o, r) = (f.offset, f.repr);
                self.assign(r, Rvalue::Load { base: v, offset: o, repr: r })
            }
            None => {
                self.m.errors.push(format!("internal: no field `{}` in layout", self.m.sym(label)));
                unit_op()
            }
        }
    }

    // ------------------------------------------------------------ expressions

    pub fn lower(&mut self, e: ExprId) -> Option<Operand> {
        let v = self.lower_inner(e)?;
        match self.inst.coercions.get(&e).cloned() {
            Some(badger_typeck::Coercion::Dyn { protocol, args }) => {
                let t = self.ty(e);
                Some(self.dyn_coerce(v, &t, protocol, &args))
            }
            Some(badger_typeck::Coercion::RangeToSeq) => match self.m.lang_fn("seq.from_range", vec![]) {
                Some(f) => Some(self.call(Callee::Direct(f), vec![v], Repr::Ptr)),
                None => Some(v),
            },
            Some(badger_typeck::Coercion::ToSeq) => {
                let t = self.ty(e);
                let elem = self.m.lay.args(&t).first().cloned().unwrap_or_else(badger_typeck::ty::unit);
                let list = if self.m.lay.is_std(&t, "List") {
                    v
                } else {
                    let lt = badger_typeck::ty::mk(TyKind::Con(self.m.g.env.b.list, vec![elem.clone()]));
                    let tp = self.m.targs_ptr(&[elem.clone(), t.clone()]);
                    let rd = self.m.lay.desc(&lt) as *const TypeDesc as u64;
                    self.call(
                        Callee::Intrinsic { name: "badger_rt_i_buffer_to_list".into(), targs: tp, ret: rd },
                        vec![v],
                        Repr::Ptr,
                    )
                };
                match self.m.lang_fn("list.of", vec![elem]) {
                    Some(f) => Some(self.call(Callee::Direct(f), vec![list], Repr::Ptr)),
                    None => Some(list),
                }
            }
            Some(badger_typeck::Coercion::ToData) => {
                let t = self.ty(e);
                let v = Operand::Local(self.copy_to_local(v, self.repr(&t), None));
                // an unrepresentable number reports this location
                let l = self.loc(self.span(e));
                self.rt("badger_rt_caller_loc", vec![Operand::Const(Const::Addr(l))], Repr::I8);
                match self.m.lang_fn("codec.to_data", vec![t]) {
                    Some(f) => Some(self.call(Callee::Direct(f), vec![v], Repr::Ptr)),
                    None => Some(v),
                }
            }
            Some(badger_typeck::Coercion::ToBytes) => {
                let bt = badger_typeck::ty::mk(TyKind::Con(self.m.g.env.b.bytes, vec![]));
                let tp = self.m.targs_ptr(&[]);
                let rd = self.m.lay.desc(&bt) as *const TypeDesc as u64;
                Some(self.call(
                    Callee::Intrinsic { name: "badger_rt_i_buffer_to_bytes".into(), targs: tp, ret: rd },
                    vec![v],
                    Repr::Ptr,
                ))
            }
            _ => Some(v),
        }
    }

    /// `derive(Error)` cause: the payload of a single-field variant whose
    /// type is itself an `Error`, as `dyn Error`; otherwise `none`.
    fn error_cause(&mut self, self_ty: &Ty, v: Operand, ret_ty: &Ty) -> Operand {
        let error = self.m.g.env.b.error;
        let none = self.alloc_enum(ret_ty, 0, vec![]);
        let TyKind::Con(d, _) = &**self_ty else { return none };
        if !self.m.g.env.enums.contains_key(d) {
            return none;
        }
        let (_, vs) = self.m.lay.enum_variants(self_ty);
        let causes: Vec<(usize, Ty)> = vs
            .iter()
            .enumerate()
            .filter(|(_, v)| v.1.len() == 1 && self.m.has_impl_pub(error, &v.1[0].ty))
            .map(|(i, v)| (i, v.1[0].ty.clone()))
            .collect();
        if causes.is_empty() {
            return none;
        }
        let res = self.func.locals.push(LocalDecl { repr: Repr::Ptr, name: None });
        let v = Operand::Local(self.copy_to_local(v, Repr::Ptr, None));
        let tag = self.assign(Repr::I32, Rvalue::Tag(v));
        let (join, default) = (self.block(), self.block());
        let mut cases = vec![];
        let blocks: Vec<BlockId> = causes.iter().map(|_| self.block()).collect();
        for ((i, _), b) in causes.iter().zip(&blocks) {
            cases.push((*i as i64, *b));
        }
        self.term(Term::Switch(tag, cases, default));
        for ((i, ft), b) in causes.into_iter().zip(blocks) {
            self.switch(b);
            let (x, _) = self.variant_field(self_ty, i as u32, 0, v);
            let dynv = self.dyn_coerce(x, &ft, error, &[]);
            let dl = self.copy_to_local(dynv, Repr::Ptr, None);
            let some = self.alloc_enum(ret_ty, 1, vec![Operand::Local(dl)]);
            self.emit(Stmt::Assign(res, Rvalue::Use(some)));
            self.term(Term::Goto(join));
        }
        self.switch(default);
        let none = self.alloc_enum(ret_ty, 0, vec![]);
        self.emit(Stmt::Assign(res, Rvalue::Use(none)));
        self.term(Term::Goto(join));
        self.switch(join);
        Operand::Local(res)
    }

    /// Package a concrete value with its protocol dictionary.
    fn dyn_coerce(&mut self, v: Operand, t: &Ty, protocol: DefId, pargs: &[Ty]) -> Operand {
        let dyn_ty = badger_typeck::ty::mk(TyKind::Dyn(protocol, pargs.to_vec()));
        let v = Operand::Local(self.copy_to_local(v, self.repr(t), None));
        // box the data
        let rec = badger_typeck::ground_resolved(&badger_typeck::ty::record(
            [(self.m.g.interner.intern("value"), t.clone())],
            badger_typeck::ty::row_empty(),
        ));
        let bl = self.m.lay.record(&rec);
        let bd = self.m.lay.desc(&rec);
        let boxed = self.assign(Repr::Ptr, Rvalue::Alloc { desc: bd as *const _ as u64, size: bl.size });
        self.emit(Stmt::Store { base: boxed, offset: 0, repr: bl.fields[0].repr, value: v });
        let boxed = Operand::Local(self.copy_to_local(boxed, Repr::Ptr, None));
        let layout = self.m.lay.record(&dyn_ty);
        let desc = self.m.lay.desc(&dyn_ty);
        let obj = self.assign(Repr::Ptr, Rvalue::Alloc { desc: desc as *const _ as u64, size: layout.size });
        for fl in &layout.fields {
            if fl.name == "data" {
                self.emit(Stmt::Store { base: obj, offset: fl.offset, repr: Repr::Ptr, value: boxed });
                continue;
            }
            let fid = self.m.dyn_thunk(t, protocol, pargs, fl.label);
            let c = self.m.static_closure(fid);
            self.emit(Stmt::Store {
                base: obj,
                offset: fl.offset,
                repr: Repr::Ptr,
                value: Operand::Const(Const::Addr(c)),
            });
        }
        obj
    }

    fn lower_inner(&mut self, e: ExprId) -> Option<Operand> {
        let hir = self.hir();
        let span = hir.exprs[e].span;
        let kind = hir.exprs[e].kind.clone();
        let ty = self.ty(e);
        if self.m.fuse_sequences
            && let Some(plan) = self.fused_terminal(e)
        {
            return self.lower_fused(e, plan);
        }
        match kind {
            ExprKind::Lit(l) => Some(self.lit(&l, &ty)),
            ExprKind::Interp(parts) => {
                let mut ops = vec![];
                for p in parts {
                    match p {
                        InterpPart::Text(t) => {
                            let w = self.m.str_const(&t);
                            ops.push(Operand::Const(Const::Addr(w)));
                        }
                        InterpPart::Expr(x) => {
                            let v = self.lower(x)?;
                            let xt = self.ty(x);
                            let s = self.display(v, &xt, x);
                            ops.push(s);
                        }
                    }
                }
                Some(self.assign(Repr::Ptr, Rvalue::Concat(ops)))
            }
            // one use of a generalized local: its initializer at these types
            ExprKind::Local(l) if self.inst.poly_locals.contains_key(&l) => {
                let init = self.inst.poly_locals[&l];
                let args = self.inst.local_insts.get(&e).cloned().unwrap_or_default();
                let saved = self.inst.clone();
                self.inst = Rc::new(badger_typeck::subst_instance(&saved, &args));
                let v = self.lower(init);
                self.inst = saved;
                v
            }
            // a value of type `Never` does not exist: code reading one
            // (`diverge`'s parameter) is unreachable
            ExprKind::Local(_) if self.is_never(&ty) => {
                self.dead();
                None
            }
            ExprKind::Local(l) => match self.locals.get(&l).copied() {
                // a `var` is read as its value now: an operand evaluated
                // later may assign it before this one is used
                Some(x) if hir.locals[l].mutable => {
                    let r = self.repr(&ty);
                    Some(self.assign(r, Rvalue::Use(Operand::Local(x))))
                }
                Some(x) => Some(Operand::Local(x)),
                None => {
                    // a global's own binding referenced (should not happen)
                    Some(unit_op())
                }
            },
            ExprKind::Def { def, .. } => {
                let args = match self.inst.res.get(&e) {
                    Some(Res::Callee { args, .. }) => args.clone(),
                    _ => vec![],
                };
                match &hir.defs[def].kind {
                    DefKind::Global(_) => {
                        let f = self.m.global(def, args);
                        let r = self.repr(&ty);
                        Some(self.call(Callee::Direct(f), vec![], r))
                    }
                    DefKind::Fn(f) if f.abi == FnAbi::Lang => {
                        let fid = self.m.instance(def, args);
                        let w = self.m.static_closure(fid);
                        Some(Operand::Const(Const::Addr(w)))
                    }
                    _ => {
                        self.m.errors.push(format!("intrinsic `{}` used as a value", self.m.def_path(def)));
                        Some(unit_op())
                    }
                }
            }
            ExprKind::Ctor { variant, .. } => {
                // constructor as a value: nullary variant or function
                match &*ty {
                    TyKind::Fn(f) => {
                        let enum_ty = f.ret.clone();
                        let fid = self.m.ctor_thunk(&enum_ty, variant);
                        Some(Operand::Const(Const::Addr(self.m.static_closure(fid))))
                    }
                    _ => Some(self.alloc_enum(&ty, variant, vec![])),
                }
            }
            ExprKind::AssocFn { .. } => {
                // associated function as a value
                match self.inst.res.get(&e).cloned() {
                    Some(Res::Method(Target::Fn { def, args })) => {
                        let fid = self.m.instance(def, args);
                        Some(Operand::Const(Const::Addr(self.m.static_closure(fid))))
                    }
                    // `Error.from` of `derive(From)`: the variant's constructor
                    Some(Res::Method(Target::Derived {
                        imp: DerivedImplPub::FromVariant { variant, .. },
                        ..
                    })) if matches!(&*ty, TyKind::Fn(_)) => {
                        let TyKind::Fn(f) = &*ty else { unreachable!() };
                        let fid = self.m.ctor_thunk(&f.ret.clone(), variant);
                        Some(Operand::Const(Const::Addr(self.m.static_closure(fid))))
                    }
                    _ => {
                        self.m.errors.push("unsupported associated function value".into());
                        Some(unit_op())
                    }
                }
            }
            ExprKind::Field { base, label, .. } => {
                let b = self.lower(base)?;
                let bt = self.ty(base);
                Some(self.load_field(&bt, label, b))
            }
            ExprKind::Index { base, index } => {
                let b = self.lower(base)?;
                let bt = self.ty(base);
                match self.inst.res.get(&e).cloned() {
                    Some(Res::Index(IndexKind::Label(l))) => Some(self.load_field(&bt, l, b)),
                    Some(Res::Index(IndexKind::Bytes)) => {
                        let i = self.lower(index)?;
                        let l = self.loc(span);
                        let tp = self.m.targs_ptr(&[]);
                        let rd = self.m.lay.desc(&ty) as *const TypeDesc as u64;
                        // a failed bounds check reports this location
                        self.rt("badger_rt_caller_loc", vec![Operand::Const(Const::Addr(l))], Repr::I8);
                        let w = self.call(
                            Callee::Intrinsic { name: "badger_rt_i_bytes_get".into(), targs: tp, ret: rd },
                            vec![b, i],
                            Repr::I64,
                        );
                        Some(self.from_word(w, &ty))
                    }
                    Some(Res::Index(IndexKind::Map)) => {
                        let k = self.lower(index)?;
                        let l = self.loc(span);
                        let w = self.rt(
                            "badger_rt_map_index",
                            vec![b, k, Operand::Const(Const::Addr(l))],
                            Repr::I64,
                        );
                        Some(self.from_word(w, &ty))
                    }
                    _ => {
                        let i = self.lower(index)?;
                        let l = self.loc(span);
                        if let Some(v) = self.inline_list_index(&bt, b, i, l, &ty) {
                            return Some(v);
                        }
                        let w = self.rt(
                            "badger_rt_list_get",
                            vec![b, i, Operand::Const(Const::Addr(l))],
                            Repr::I64,
                        );
                        Some(self.from_word(w, &ty))
                    }
                }
            }
            ExprKind::Call { callee, args } => self.lower_call(e, callee, &args, &ty),
            ExprKind::MethodCall { recv, args, .. } => {
                let target = match self.inst.res.get(&e).cloned() {
                    Some(Res::Method(t)) => t,
                    _ => {
                        let rt = self.ty(recv);
                        let loc = self.m.loc(span);
                        self.m.errors.push(format!(
                            "internal: unresolved method on `{}` at {} in {}",
                            self.m.lay.type_name(&rt),
                            badger_runtime::string::str_of(loc),
                            self.func.name
                        ));
                        return Some(unit_op());
                    }
                };
                // `m.entries().map(..)` / `.filter(..)` / `.fold(..)` as
                // one trie walk, with no intermediate entry list.
                if let Target::Fn { def, args: targs } = &target {
                    let (def, targs) = (*def, targs.clone());
                    if let Some(v) = self.try_fused_map_entries(e, recv, &args, def, &targs, &ty, span) {
                        return Some(v);
                    }
                }
                // `map(filter(xs,p), f)` / `filter(map(xs,f), p)` as one
                // traversal, when the earlier callback cannot be observed
                // or fail (see `try_fused_list_map`).
                if args.len() == 1
                    && let Target::Fn { def, args: targs } = &target
                {
                    let (def, targs) = (*def, targs.clone());
                    if let Some(v) = self.try_fused_list_map(e, recv, args[0], def, &targs, &ty, span) {
                        return Some(v);
                    }
                }
                let r = self.lower(recv)?;
                let rt = self.ty(recv);
                let mut ops = vec![r];
                for a in &args {
                    ops.push(self.lower(*a)?);
                }
                // checked buffer element access reports the caller's location
                if let Target::Fn { def, .. } = &target {
                    if self.is_checked_element_access(*def, span) {
                        let l = self.loc(span);
                        self.rt("badger_rt_caller_loc", vec![Operand::Const(Const::Addr(l))], Repr::I8);
                    }
                    if self.has_requires(*def) {
                        let l = self.loc(span);
                        self.rt("badger_rt_contract_site", vec![Operand::Const(Const::Addr(l))], Repr::I8);
                    }
                }
                self.call_target(&target, ops, &rt, &ty, span)
            }
            ExprKind::Try(inner) => self.lower_try(e, inner, span),
            ExprKind::Unary { op, operand } => {
                let v = self.lower(operand)?;
                match op {
                    UnOp::Not => Some(self.assign(Repr::I8, Rvalue::Not(v))),
                    UnOp::Neg => {
                        let nt = self.m.lay.num_ty(&ty).unwrap_or(NumTy::I(64, true));
                        let l = self.loc(span);
                        Some(self.assign(self.repr(&ty), Rvalue::Neg { a: v, ty: nt, loc: l }))
                    }
                    UnOp::Ref | UnOp::RefMut => Some(v),
                }
            }
            ExprKind::Binary { op, lhs, rhs } => self.lower_binary(e, op, lhs, rhs, &ty, span),
            ExprKind::Tuple(items) => {
                let mut vals = vec![];
                for x in &items {
                    vals.push(self.lower(*x)?);
                }
                if items.is_empty() {
                    return Some(unit_op());
                }
                let layout = self.m.lay.record(&ty);
                let desc = self.m.lay.desc(&ty);
                let obj = self
                    .assign(Repr::Ptr, Rvalue::Alloc { desc: desc as *const _ as u64, size: layout.size });
                for (fl, v) in layout.fields.iter().zip(vals) {
                    self.emit(Stmt::Store { base: obj, offset: fl.offset, repr: fl.repr, value: v });
                }
                Some(obj)
            }
            ExprKind::List(items) => {
                let mut vals = vec![];
                for x in &items {
                    let v = self.lower(*x)?;
                    // keep values rooted across allocation
                    let r = self.lowered_repr(*x);
                    let l = self.copy_to_local(v, r, None);
                    vals.push(Operand::Local(l));
                }
                let d = self.desc_op(&ty);
                let list = self.rt("badger_rt_list_new", vec![d], Repr::Ptr);
                let list = Operand::Local(self.copy_to_local(list, Repr::Ptr, None));
                for v in vals {
                    self.rt("badger_rt_list_push", vec![list, v], Repr::I8);
                }
                Some(list)
            }
            ExprKind::Record { spread, fields } => self.lower_record(&ty, spread, &fields),
            ExprKind::Map(entries) => {
                let mut kvs = vec![];
                for (k, v) in &entries {
                    let kv = self.lower(*k)?;
                    let kr = self.lowered_repr(*k);
                    let kl = self.copy_to_local(kv, kr, None);
                    let vv = self.lower(*v)?;
                    let vr = self.lowered_repr(*v);
                    let vl = self.copy_to_local(vv, vr, None);
                    kvs.push((Operand::Local(kl), Operand::Local(vl)));
                }
                let d = self.desc_op(&ty);
                let m = self.rt("badger_rt_map_new", vec![d], Repr::Ptr);
                let m = Operand::Local(self.copy_to_local(m, Repr::Ptr, None));
                for (k, v) in kvs {
                    self.rt("badger_rt_map_insert_mut", vec![m, k, v], Repr::I8);
                }
                Some(m)
            }
            ExprKind::Set(items) => {
                let mut ks = vec![];
                for k in &items {
                    let kv = self.lower(*k)?;
                    let kr = self.lowered_repr(*k);
                    ks.push(Operand::Local(self.copy_to_local(kv, kr, None)));
                }
                let d = self.desc_op(&ty);
                let m = self.rt("badger_rt_map_new", vec![d], Repr::Ptr);
                let m = Operand::Local(self.copy_to_local(m, Repr::Ptr, None));
                for k in ks {
                    self.rt("badger_rt_map_insert_mut", vec![m, k, int(0)], Repr::I8);
                }
                Some(m)
            }
            ExprKind::Lambda(l) => Some(self.lower_lambda(e, &l, &ty)),
            // a precondition check the contract analysis proved never fails
            ExprKind::If { .. } if elided_check(e) => Some(int(0)),
            // a precondition check under observation: count its outcome,
            // then fail as usual
            ExprKind::If { then, .. } if observed_pre(e).is_some() => {
                let (id, held_cond) = observed_pre(e).unwrap();
                let c = self.lower(held_cond)?;
                self.rt("badger_rt_observe_clause", vec![int(id as i64), c], Repr::I64);
                let (fail, join) = (self.block(), self.block());
                self.term(Term::Branch(c, join, fail));
                self.switch(fail);
                if self.lower(then).is_some() {
                    self.term(Term::Goto(join));
                }
                self.switch(join);
                Some(int(0))
            }
            ExprKind::If { cond, then, els } => {
                let c = self.lower(cond)?;
                let (tb, eb, join) = (self.block(), self.block(), self.block());
                self.term(Term::Branch(c, tb, eb));
                let r = self.repr(&ty);
                let res = self.fresh(r, None);
                self.switch(tb);
                if let Some(v) = self.lower(then) {
                    if els.is_some() {
                        self.emit(Stmt::Assign(res, Rvalue::Use(v)));
                    }
                    self.term(Term::Goto(join));
                }
                self.switch(eb);
                match els {
                    Some(x) => {
                        if let Some(v) = self.lower(x) {
                            self.emit(Stmt::Assign(res, Rvalue::Use(v)));
                            self.term(Term::Goto(join));
                        }
                    }
                    None => self.term(Term::Goto(join)),
                }
                self.switch(join);
                if els.is_none() {
                    return Some(unit_op());
                }
                Some(Operand::Local(res))
            }
            ExprKind::Match { scrutinee, arms } => {
                let s = self.lower(scrutinee)?;
                let st = self.ty(scrutinee);
                let sl = self.copy_to_local(s, self.repr(&st), None);
                let r = self.repr(&ty);
                let res = self.fresh(r, None);
                let join = self.block();
                for arm in &arms {
                    let next = self.block();
                    self.test_pat(arm.pat, Operand::Local(sl), &st, next);
                    if let Some(g) = arm.guard {
                        match self.lower(g) {
                            Some(gv) => {
                                let body = self.block();
                                self.term(Term::Branch(gv, body, next));
                                self.switch(body);
                            }
                            None => {
                                self.switch(next);
                                continue;
                            }
                        }
                    }
                    if let Some(v) = self.lower(arm.body) {
                        self.emit(Stmt::Assign(res, Rvalue::Use(v)));
                        self.term(Term::Goto(join));
                    }
                    self.switch(next);
                }
                self.defect(4, span);
                self.switch(join);
                Some(Operand::Local(res))
            }
            ExprKind::While { cond, body } => {
                let (head, bodyb, exit) = (self.block(), self.block(), self.block());
                self.term(Term::Goto(head));
                self.switch(head);
                self.emit(Stmt::Poll);
                if let Some(c) = self.lower(cond) {
                    self.term(Term::Branch(c, bodyb, exit));
                }
                self.switch(bodyb);
                self.loops.push(LoopCtx { expr: e, cont: head, brk: exit, result: None, close: None });
                if self.lower(body).is_some() {
                    self.term(Term::Goto(head));
                }
                self.loops.pop();
                self.switch(exit);
                Some(unit_op())
            }
            ExprKind::Loop { body } => {
                let (head, exit) = (self.block(), self.block());
                let r = self.repr(&ty);
                let res = self.fresh(r, None);
                self.term(Term::Goto(head));
                self.switch(head);
                self.emit(Stmt::Poll);
                self.loops.push(LoopCtx { expr: e, cont: head, brk: exit, result: Some(res), close: None });
                if self.lower(body).is_some() {
                    self.term(Term::Goto(head));
                }
                self.loops.pop();
                self.switch(exit);
                Some(Operand::Local(res))
            }
            ExprKind::For { pat, iter, body } => {
                self.lower_for(e, pat, iter, body)?;
                Some(unit_op())
            }
            ExprKind::Block { stmts, tail } => {
                for s in &stmts {
                    let at = match s {
                        HStmt::Let { span, .. } | HStmt::Assign { span, .. } => *span,
                        HStmt::Expr { expr, .. } => self.hir().exprs[*expr].span,
                    };
                    self.mark(at);
                    match s {
                        // a generalized local is built at each use instead
                        HStmt::Let { pat, .. } if matches!(&self.hir().pats[*pat].kind, badger_hir::PatKind::Bind(l) if self.inst.poly_locals.contains_key(l)) =>
                            {}
                        HStmt::Let { pat, init, .. } => {
                            if let Some((local, plan)) = self.stored_sequence_plan(*pat, *init) {
                                let prepared = self.prepare_sequence(plan)?;
                                self.stored_sequences.insert(local, prepared);
                            } else {
                                let v = self.lower(*init)?;
                                let t = self.pat_ty(*pat);
                                self.bind_irrefutable(*pat, v, &t);
                            }
                        }
                        HStmt::Assign { local, value, .. } => {
                            let v = self.lower(*value)?;
                            if let Some(l) = self.locals.get(local).copied() {
                                self.emit(Stmt::Assign(l, Rvalue::Use(v)))
                            }
                        }
                        HStmt::Expr { expr, .. } => {
                            self.lower(*expr)?;
                        }
                    }
                }
                match tail {
                    Some(t) => {
                        let at = self.hir().exprs[t].span;
                        self.mark(at);
                        self.lower(t)
                    }
                    None => Some(unit_op()),
                }
            }
            ExprKind::Return { value, .. } => {
                let v = match value {
                    Some(v) => self.lower(v)?,
                    None => unit_op(),
                };
                self.close_iters(0);
                self.ret(v);
                self.dead();
                None
            }
            ExprKind::Break { value, target } => {
                let v = match value {
                    Some(v) => Some(self.lower(v)?),
                    None => None,
                };
                if let Some(k) = self.loops.iter().rposition(|l| l.expr == target) {
                    self.close_iters(k + 1);
                    let lc = &self.loops[k];
                    let (brk, res) = (lc.brk, lc.result);
                    if let (Some(r), Some(v)) = (res, v) {
                        self.emit(Stmt::Assign(r, Rvalue::Use(v)));
                    } else if let Some(r) = res {
                        self.emit(Stmt::Assign(r, Rvalue::Use(unit_op())));
                    }
                    self.term(Term::Goto(brk));
                }
                self.dead();
                None
            }
            ExprKind::Continue { target } => {
                if let Some(k) = self.loops.iter().rposition(|l| l.expr == target) {
                    self.close_iters(k + 1);
                    let c = self.loops[k].cont;
                    self.term(Term::Goto(c));
                }
                self.dead();
                None
            }
            ExprKind::Unsafe(b) => self.lower(b),
            ExprKind::Error => Some(unit_op()),
        }
    }

    fn lit(&mut self, l: &Lit, ty: &Ty) -> Operand {
        let r = self.repr(ty);
        match l {
            Lit::Int { value, .. } => match r {
                // negative literals are two's complement in u128
                Repr::F32 => Operand::Const(Const::F32(*value as i128 as f32)),
                Repr::F64 => Operand::Const(Const::F64(*value as i128 as f64)),
                _ => Operand::Const(Const::Int(*value as i64, r)),
            },
            Lit::Float { value, .. } => match r {
                Repr::F32 => Operand::Const(Const::F32(*value as f32)),
                _ => Operand::Const(Const::F64(*value)),
            },
            Lit::Duration(n) => Operand::Const(Const::Int(*n as i64, Repr::I64)),
            Lit::Str(s) => Operand::Const(Const::Addr(self.m.str_const(s))),
            Lit::Char(c) => Operand::Const(Const::Int(*c as i64, Repr::I32)),
            Lit::Symbol(s) => {
                let name = self.m.sym(*s).to_string();
                Operand::Const(Const::Addr(self.m.symbol(&name)))
            }
            Lit::Bool(b) => Operand::Const(Const::Int(*b as i64, Repr::I8)),
            Lit::Unit => unit_op(),
        }
    }

    /// Convert a runtime word to a value of type `t`.
    fn from_word(&mut self, w: Operand, t: &Ty) -> Operand {
        let r = self.repr(t);
        // a managed pointer returned as a plain word moves into a pointer
        // local at once, so the collector sees it (it may run while this
        // task waits at its next runtime call)
        if r == Repr::Ptr
            && let Operand::Local(l) = w
            && self.func.locals[l].repr != Repr::Ptr
        {
            return Operand::Local(self.copy_to_local(w, Repr::Ptr, None));
        }
        if r == Repr::I64 || r == Repr::Ptr {
            return w;
        }
        self.assign(r, Rvalue::FromWord { a: w, to: r })
    }

    /// Display `v : t` as a String using the resolution recorded for `e`.
    fn display(&mut self, v: Operand, t: &Ty, e: ExprId) -> Operand {
        if self.m.lay.is_std(t, "String") {
            return v;
        }
        match self.inst.displays.get(&e).cloned() {
            Some(Target::Fn { def, args }) => {
                let span = self.span(e);
                self.call_target(&Target::Fn { def, args }, vec![v], t, &self.string_ty(), span).unwrap_or(v)
            }
            // a `dyn` value displays through its dictionary
            Some(tg @ Target::Dyn { .. }) => {
                let span = self.span(e);
                let v = Operand::Local(self.copy_to_local(v, Repr::Ptr, None));
                self.call_target(&tg, vec![v], t, &self.string_ty(), span).unwrap_or(v)
            }
            _ => {
                let d = self.desc_op(t);
                self.rt("badger_rt_display", vec![d, v], Repr::Ptr)
            }
        }
    }

    fn string_ty(&self) -> Ty {
        badger_typeck::ty::mk(TyKind::Con(self.m.g.env.b.string, vec![]))
    }

    /// Call a resolved method/associated target with lowered arguments.
    fn call_target(
        &mut self,
        target: &Target,
        args: Vec<Operand>,
        recv_ty: &Ty,
        ret_ty: &Ty,
        span: Span,
    ) -> Option<Operand> {
        let ret = self.repr(ret_ty);
        match target {
            Target::Fn { def, args: targs } => self.call_def(*def, targs.clone(), args, ret_ty, span),
            Target::Derived { imp, self_ty } => match imp {
                DerivedImplPub::Display => {
                    let d = self.desc_op(self_ty);
                    Some(self.rt("badger_rt_display", vec![d, args[0]], Repr::Ptr))
                }
                DerivedImplPub::FromVariant { variant, .. } => {
                    Some(self.alloc_enum(ret_ty, *variant, vec![args[0]]))
                }
                DerivedImplPub::Marker => Some(unit_op()),
                DerivedImplPub::Identity => Some(args[0]),
                DerivedImplPub::ErrorCause => Some(self.error_cause(self_ty, args[0], ret_ty)),
            },
            Target::Field(label) => {
                let f = self.load_field(recv_ty, *label, args[0]);
                Some(self.call(Callee::Closure(f), args[1..].to_vec(), ret))
            }
            Target::Dyn { method, .. } => {
                let layout = self.m.lay.record(recv_ty);
                let data = layout.fields.iter().find(|f| f.name == "data").unwrap().clone();
                let mname = format!("m_{}", self.m.sym(*method));
                let mf = layout.fields.iter().find(|f| f.name == mname).unwrap().clone();
                let d = self
                    .assign(Repr::Ptr, Rvalue::Load { base: args[0], offset: data.offset, repr: Repr::Ptr });
                let c = self
                    .assign(Repr::Ptr, Rvalue::Load { base: args[0], offset: mf.offset, repr: Repr::Ptr });
                let mut cargs = vec![d];
                cargs.extend_from_slice(&args[1..]);
                Some(self.call(Callee::Closure(c), cargs, ret))
            }
        }
    }

    /// Call a function definition (language function or intrinsic).
    fn call_def(
        &mut self,
        def: DefId,
        targs: Vec<Ty>,
        args: Vec<Operand>,
        ret_ty: &Ty,
        span: Span,
    ) -> Option<Operand> {
        let ret = self.repr(ret_ty);
        let hir = self.hir();
        let DefKind::Fn(f) = &hir.defs[def].kind else {
            // calling a global holding a closure
            let fid = self.m.global(def, targs);
            let c = self.call(Callee::Direct(fid), vec![], Repr::Ptr);
            return Some(self.call(Callee::Closure(c), args, ret));
        };
        match f.abi {
            FnAbi::Lang => {
                let fid = self.m.instance(def, targs);
                let r = self.call(Callee::Direct(fid), args, ret);
                if self.is_never(ret_ty) {
                    self.dead();
                    return None;
                }
                Some(r)
            }
            FnAbi::Intrinsic => {
                let name = self.m.sym(hir.defs[def].name).to_string();
                self.intrinsic(&name, targs, args, ret_ty, span)
            }
            FnAbi::C => {
                let name = self.m.sym(hir.defs[def].name).to_string();
                // parameter types from the declaration (ground)
                let fty = self.m.g.scheme(def).map(|s| badger_typeck::ground_resolved(&s.ty));
                let (params, fret) = match fty.as_deref() {
                    Some(TyKind::Fn(f)) => (
                        f.params
                            .iter()
                            .map(|p| {
                                self.m
                                    .lay
                                    .ffi_arg(p)
                                    .unwrap_or(crate::mir::FfiArg::Scalar(self.m.lay.repr(p)))
                            })
                            .collect(),
                        Some(f.ret.clone()),
                    ),
                    _ => (vec![], None),
                };
                let ret_ffi = fret.as_ref().and_then(|t| match self.m.lay.ffi_arg(t) {
                    Some(crate::mir::FfiArg::Scalar(_)) | None => None,
                    other => other,
                });
                let contract = match &hir.defs[def].kind {
                    DefKind::Fn(f) => f.ffi.blocking as u8 | (f.ffi.pinned as u8) << 1,
                    _ => 0,
                };
                Some(self.call(Callee::Foreign { name, params, ret: ret_ffi, contract }, args, ret))
            }
            FnAbi::Native => {
                // the host converts by the declared types; the site names
                // the module and function
                let DefKind::Fn(fd) = &hir.defs[def].kind else { unreachable!() };
                let spelled = fd.native.clone().unwrap_or_default();
                let decl_name = self.m.sym(hir.defs[def].name).to_string();
                let (lib, name) = match spelled.split_once('.') {
                    Some((m, f)) => (m.to_string(), f.to_string()),
                    None => (spelled.clone(), decl_name),
                };
                let fty = self.m.g.scheme(def).map(|s| badger_typeck::ground_resolved(&s.ty));
                let params: Vec<Ty> = match fty.as_deref() {
                    Some(TyKind::Fn(f)) => f.params.clone(),
                    _ => vec![],
                };
                if args.len() > 8 {
                    self.m.errors.push(format!("native function `{name}` takes more than 8 arguments"));
                    return Some(unit_op());
                }
                let site = self.m.str_const(&format!("{lib}\0{name}"));
                self.m.prog.natives.insert(lib.clone());
                let tp = self.m.targs_ptr(&params);
                let rd = self.m.lay.desc(ret_ty) as *const TypeDesc as u64;
                let wr = if self.repr(ret_ty) == Repr::Ptr { Repr::Ptr } else { Repr::I64 };
                let mut all = vec![Operand::Const(Const::Addr(site))];
                all.extend(args);
                let w = self.call(
                    Callee::Intrinsic {
                        name: format!("badger_rt_i_native_call{}", all.len() - 1),
                        targs: tp,
                        ret: rd,
                    },
                    all,
                    wr,
                );
                if self.is_never(ret_ty) {
                    self.dead();
                    return None;
                }
                Some(self.from_word(w, ret_ty))
            }
        }
    }

    fn is_never(&self, t: &Ty) -> bool {
        self.m.lay.is_std(t, "Never")
    }

    fn intrinsic(
        &mut self,
        name: &str,
        targs: Vec<Ty>,
        args: Vec<Operand>,
        ret_ty: &Ty,
        span: Span,
    ) -> Option<Operand> {
        let ret = self.repr(ret_ty);
        match name {
            "panic" => {
                let l = self.loc(span);
                self.rt("badger_rt_panic", vec![args[0], Operand::Const(Const::Addr(l))], Repr::I64);
                self.dead();
                None
            }
            "precondition_failed" => {
                let l = self.loc(span);
                self.rt(
                    "badger_rt_precondition",
                    vec![args[0], args[1], Operand::Const(Const::Addr(l))],
                    Repr::I64,
                );
                self.dead();
                None
            }
            "assert" => {
                let (ok, fail) = (self.block(), self.block());
                self.term(Term::Branch(args[0], ok, fail));
                self.switch(fail);
                self.defect(3, span);
                self.term(Term::Unreachable);
                self.switch(ok);
                Some(unit_op())
            }
            // reinterpret a value as a word and back (system outputs, erasure)
            "erase" => Some(self.to_word(args[0], &targs[0])),
            "unerase" => Some(self.from_word(args[0], ret_ty)),
            "convert_int" => {
                let from = self.m.lay.num_ty(&targs[0]).unwrap_or(NumTy::I(64, true));
                let to = self.m.lay.num_ty(&targs[1]).unwrap_or(NumTy::I(64, true));
                let l = self.loc(span);
                Some(self.assign(ret, Rvalue::Convert { a: args[0], from, to, checked: true, loc: l }))
            }
            "convert_lossy" => {
                let from = self.m.lay.num_ty(&targs[0]).unwrap_or(NumTy::I(64, true));
                let to = self.m.lay.num_ty(&targs[1]).unwrap_or(NumTy::I(64, true));
                Some(self.assign(ret, Rvalue::Convert { a: args[0], from, to, checked: false, loc: 0 }))
            }
            "int_fits" => {
                let from = self.m.lay.num_ty(&targs[0]).unwrap_or(NumTy::I(64, true));
                let to = self.m.lay.num_ty(&targs[1]).unwrap_or(NumTy::I(64, true));
                Some(self.assign(Repr::I8, Rvalue::Fits { a: args[0], from, to }))
            }
            "add_overflows" | "sub_overflows" | "mul_overflows" | "div_overflows" | "rem_overflows" => {
                let ty = self.m.lay.num_ty(&targs[0]).unwrap_or(NumTy::I(64, true));
                let op = match name {
                    "add_overflows" => BinOp::Add,
                    "sub_overflows" => BinOp::Sub,
                    "mul_overflows" => BinOp::Mul,
                    "div_overflows" => BinOp::Div,
                    _ => BinOp::Rem,
                };
                Some(self.assign(Repr::I8, Rvalue::Overflows { op, a: args[0], b: args[1], ty }))
            }
            "wrapping_add" | "wrapping_sub" | "wrapping_mul" => {
                let ty = self.m.lay.num_ty(&targs[0]).unwrap_or(NumTy::I(64, true));
                let op = match name {
                    "wrapping_add" => BinOp::Add,
                    "wrapping_sub" => BinOp::Sub,
                    _ => BinOp::Mul,
                };
                Some(self.assign(ret, Rvalue::Bin { op, a: args[0], b: args[1], ty, loc: 0 }))
            }
            // a value paired with its call site
            "located" => {
                let (file, line, col) = if span.is_dummy() {
                    ("<unknown>".to_string(), 0, 0)
                } else {
                    let f = self.m.sources.get(span.file);
                    let (l, c) = f.line_col(span.start as usize);
                    (display_path(&f.name), l as i64, c as i64)
                };
                let v = Operand::Local(self.copy_to_local(args[0], self.repr(&targs[0]), None));
                let layout = self.m.lay.record(ret_ty);
                let desc = self.m.lay.desc(ret_ty);
                let obj = self
                    .assign(Repr::Ptr, Rvalue::Alloc { desc: desc as *const _ as u64, size: layout.size });
                let file = Operand::Const(Const::Addr(self.m.str_const(&file)));
                for fl in layout.fields.clone() {
                    let value = match self.m.sym(fl.label) {
                        "value" => v,
                        "file" => file,
                        "line" => Operand::Const(Const::Int(line, Repr::I64)),
                        _ => Operand::Const(Const::Int(col, Repr::I64)),
                    };
                    self.emit(Stmt::Store { base: obj, offset: fl.offset, repr: fl.repr, value });
                }
                Some(obj)
            }
            "identity" => Some(args[0]),
            "buffer_len"
                if self.m.lay.is_std(&targs[0], "Array") || self.m.lay.is_std(&targs[0], "Buffer") =>
            {
                let data = args[0];
                Some(self.buffer_len(data))
            }
            "buffer_get"
                if targs
                    .get(1)
                    .is_some_and(|t| self.m.lay.is_std(t, "Array") || self.m.lay.is_std(t, "Buffer")) =>
            {
                let data = args[0];
                let len = self.buffer_len(data);
                let inside = self.assign(
                    Repr::I8,
                    Rvalue::Cmp { op: CmpOp::Lt, a: args[1], b: len, ty: NumTy::I(64, false) },
                );
                let (yes, no, join) = (self.block(), self.block(), self.block());
                let result = self.fresh(Repr::Ptr, None);
                self.term(Term::Branch(inside, yes, no));
                self.switch(yes);
                let value = self.buffer_load(data, args[1], self.repr(&targs[0]));
                let some = self.alloc_enum(ret_ty, 1, vec![value]);
                self.emit(Stmt::Assign(result, Rvalue::Use(some)));
                self.term(Term::Goto(join));
                self.switch(no);
                let none = self.alloc_enum(ret_ty, 0, vec![]);
                self.emit(Stmt::Assign(result, Rvalue::Use(none)));
                self.term(Term::Goto(join));
                self.switch(join);
                Some(Operand::Local(result))
            }
            // box a value of any type behind an opaque pointer (traced precisely
            // through the box's own descriptor)
            "box_erase" => {
                let t = targs[0].clone();
                let rec = badger_typeck::ty::record(
                    [(self.m.g.interner.intern("value"), t.clone())],
                    badger_typeck::ty::row_empty(),
                );
                let rec = badger_typeck::ground_resolved(&rec);
                let layout = self.m.lay.record(&rec);
                let desc = self.m.lay.desc(&rec);
                let v = Operand::Local(self.copy_to_local(args[0], self.repr(&t), None));
                let obj = self
                    .assign(Repr::Ptr, Rvalue::Alloc { desc: desc as *const _ as u64, size: layout.size });
                self.emit(Stmt::Store { base: obj, offset: 0, repr: layout.fields[0].repr, value: v });
                Some(obj)
            }
            "box_unerase" => {
                let r = self.repr(ret_ty);
                Some(self.assign(r, Rvalue::Load { base: args[0], offset: 0, repr: r }))
            }
            // typed projection onto a closed subset (field copies)
            "project" | "remove" | "omit" => {
                let (t, src) = (targs[0].clone(), targs[1].clone());
                if t == src {
                    return Some(args[0]);
                }
                let base = Operand::Local(self.copy_to_local(args[0], Repr::Ptr, None));
                let layout = self.m.lay.record(&t);
                let desc = self.m.lay.desc(&t);
                let obj = self
                    .assign(Repr::Ptr, Rvalue::Alloc { desc: desc as *const _ as u64, size: layout.size });
                for fl in &layout.fields {
                    let v = self.load_field(&src, fl.label, base);
                    self.emit(Stmt::Store { base: obj, offset: fl.offset, repr: fl.repr, value: v });
                }
                Some(obj)
            }
            "merge" => {
                let (a_t, b_t, c_t) = (targs[0].clone(), targs[1].clone(), targs[2].clone());
                let a = Operand::Local(self.copy_to_local(args[0], Repr::Ptr, None));
                let b = Operand::Local(self.copy_to_local(args[1], Repr::Ptr, None));
                let layout = self.m.lay.record(&c_t);
                let bl = self.m.lay.record(&b_t);
                let desc = self.m.lay.desc(&c_t);
                let obj = self
                    .assign(Repr::Ptr, Rvalue::Alloc { desc: desc as *const _ as u64, size: layout.size });
                for fl in &layout.fields {
                    let v = if bl.field(fl.label).is_some() {
                        self.load_field(&b_t, fl.label, b)
                    } else {
                        self.load_field(&a_t, fl.label, a)
                    };
                    self.emit(Stmt::Store { base: obj, offset: fl.offset, repr: fl.repr, value: v });
                }
                Some(obj)
            }
            // left-biased union (the mirror of `merge`: the left operand wins)
            "merge_left" => {
                let (a_t, b_t, c_t) = (targs[0].clone(), targs[1].clone(), targs[2].clone());
                let a = Operand::Local(self.copy_to_local(args[0], Repr::Ptr, None));
                let b = Operand::Local(self.copy_to_local(args[1], Repr::Ptr, None));
                let layout = self.m.lay.record(&c_t);
                let al = self.m.lay.record(&a_t);
                let desc = self.m.lay.desc(&c_t);
                let obj = self
                    .assign(Repr::Ptr, Rvalue::Alloc { desc: desc as *const _ as u64, size: layout.size });
                for fl in &layout.fields {
                    let v = if al.field(fl.label).is_some() {
                        self.load_field(&a_t, fl.label, a)
                    } else {
                        self.load_field(&b_t, fl.label, b)
                    };
                    self.emit(Stmt::Store { base: obj, offset: fl.offset, repr: fl.repr, value: v });
                }
                Some(obj)
            }
            // one label copied to a different label (the only source label
            // the target lacks is where the renamed field comes from)
            "rename" => {
                let (t, src) = (targs[0].clone(), targs[1].clone());
                if t == src {
                    return Some(args[0]);
                }
                let base = Operand::Local(self.copy_to_local(args[0], Repr::Ptr, None));
                let layout = self.m.lay.record(&t);
                let desc = self.m.lay.desc(&t);
                let sl = self.m.lay.record(&src);
                let renamed_from = sl.fields.iter().map(|f| f.label).find(|l| layout.field(*l).is_none());
                let obj = self
                    .assign(Repr::Ptr, Rvalue::Alloc { desc: desc as *const _ as u64, size: layout.size });
                for fl in &layout.fields {
                    let from_label = if sl.field(fl.label).is_some() {
                        fl.label
                    } else {
                        renamed_from.unwrap_or(fl.label)
                    };
                    let v = self.load_field(&src, from_label, base);
                    self.emit(Stmt::Store { base: obj, offset: fl.offset, repr: fl.repr, value: v });
                }
                Some(obj)
            }
            // the number of fields of a record/struct type: a constant
            "field_count" => {
                let layout = self.m.lay.record(&targs[0]);
                Some(Operand::Const(Const::Int(layout.fields.len() as i64, Repr::I64)))
            }
            // a buffer's push into reserved capacity, without a runtime call
            // (a buffer is affine: its storage is its own until it is
            // frozen); a full or frozen buffer takes the runtime path, which
            // grows (and may collect) or panics
            "buffer_push" if self.builder_elem(&targs[0]).is_some() => {
                let repr = self.builder_elem(&targs[0]).unwrap();
                let size = repr.size() as i64;
                let u = NumTy::I(64, false);
                let b = Operand::Local(self.copy_to_local(args[0], Repr::Ptr, None));
                let v = Operand::Local(self.copy_to_local(args[1], repr, None));
                let load = |s: &mut Self, base: Operand, offset: i32, repr: Repr| {
                    s.assign(repr, Rvalue::Load { base, offset, repr })
                };
                let bin = |s: &mut Self, op: BinOp, a: Operand, b: Operand| {
                    s.assign(Repr::I64, Rvalue::Bin { op, a, b, ty: u, loc: 0 })
                };
                let len = load(self, b, badger_runtime::buffer::ITEMS_LEN, Repr::I64);
                let cap = load(self, b, badger_runtime::buffer::ITEMS_CAP, Repr::I64);
                let full = self.assign(Repr::I8, Rvalue::Cmp { op: CmpOp::Ge, a: len, b: cap, ty: u });
                let frozen = load(self, b, badger_runtime::buffer::FROZEN, Repr::I8);
                let slow_path = self.assign(
                    Repr::I8,
                    Rvalue::Bin { op: BinOp::Or, a: full, b: frozen, ty: NumTy::I(8, false), loc: 0 },
                );
                let (fast, slow, join) = (self.block(), self.block(), self.block());
                self.term(Term::Branch(slow_path, slow, fast));
                self.switch(fast);
                let ptr = load(self, b, badger_runtime::buffer::ITEMS_PTR, Repr::I64);
                let offset = bin(self, BinOp::Mul, len, int(size));
                let at = bin(self, BinOp::Add, ptr, offset);
                self.emit(Stmt::Store {
                    base: at,
                    offset: -(badger_runtime::gc::HEADER as i32),
                    repr,
                    value: v,
                });
                let len1 = bin(self, BinOp::Add, len, int(1));
                self.emit(Stmt::Store {
                    base: b,
                    offset: badger_runtime::buffer::ITEMS_LEN,
                    repr: Repr::I64,
                    value: len1,
                });
                if repr == Repr::Ptr {
                    // the write barrier for the buffer (it may predate a
                    // collection): see `gc::barrier`
                    let meta = load(self, b, -8, Repr::I64);
                    let mask = (badger_runtime::gc::MARK
                        | badger_runtime::gc::IMMORTAL
                        | badger_runtime::gc::REMEMBERED) as i64;
                    let bits = bin(self, BinOp::And, meta, int(mask));
                    let old = self.assign(
                        Repr::I8,
                        Rvalue::Cmp {
                            op: CmpOp::Eq,
                            a: bits,
                            b: int(badger_runtime::gc::MARK as i64),
                            ty: NumTy::I(64, true),
                        },
                    );
                    let remember = self.block();
                    self.term(Term::Branch(old, remember, join));
                    self.switch(remember);
                    self.rt("badger_rt_barrier", vec![b], Repr::I8);
                }
                self.term(Term::Goto(join));
                self.switch(slow);
                let tp = self.m.targs_ptr(&targs);
                let rd = self.m.lay.desc(ret_ty) as *const TypeDesc as u64;
                self.call(
                    Callee::Intrinsic { name: "badger_rt_i_buffer_push".into(), targs: tp, ret: rd },
                    vec![b, v],
                    Repr::I64,
                );
                self.term(Term::Goto(join));
                self.switch(join);
                Some(unit_op())
            }
            // a builder's append within its tail page, without a runtime
            // call (a builder's pages are its own until it is finished);
            // a new page, or a finished builder, takes the runtime path
            "list_builder_append" if self.builder_elem(&targs[0]).is_some() => {
                let repr = self.builder_elem(&targs[0]).unwrap();
                let size = repr.size() as i64;
                let cap = badger_runtime::list::page_cap_for(size as usize) as i64;
                let u = NumTy::I(64, false);
                let s64 = NumTy::I(64, true);
                let b = Operand::Local(self.copy_to_local(args[0], Repr::Ptr, None));
                let v = Operand::Local(self.copy_to_local(args[1], repr, None));
                let load = |s: &mut Self, base: Operand, offset: i32| {
                    s.assign(Repr::I64, Rvalue::Load { base, offset, repr: Repr::I64 })
                };
                let bin = |s: &mut Self, op: BinOp, a: Operand, b: Operand| {
                    s.assign(Repr::I64, Rvalue::Bin { op, a, b, ty: u, loc: 0 })
                };
                let len = load(self, b, badger_runtime::list::LEN_OFFSET);
                let capw = load(self, b, badger_runtime::list::CAP_OFFSET);
                let consumed =
                    self.assign(Repr::I8, Rvalue::Cmp { op: CmpOp::Lt, a: capw, b: int(0), ty: s64 });
                let off = bin(self, BinOp::And, len, int(cap - 1));
                let new_page =
                    self.assign(Repr::I8, Rvalue::Cmp { op: CmpOp::Eq, a: off, b: int(0), ty: s64 });
                let slow_path = self.assign(
                    Repr::I8,
                    Rvalue::Bin { op: BinOp::Or, a: consumed, b: new_page, ty: NumTy::I(8, false), loc: 0 },
                );
                let (fast, slow, join) = (self.block(), self.block(), self.block());
                self.term(Term::Branch(slow_path, slow, fast));
                self.switch(fast);
                let dir = load(self, b, badger_runtime::list::DIR_OFFSET);
                let page_index = bin(self, BinOp::Shr, len, int(cap.trailing_zeros() as i64));
                let slot_offset = bin(self, BinOp::Shl, page_index, int(3));
                let slot = bin(self, BinOp::Add, dir, slot_offset);
                let page = load(self, slot, badger_runtime::list::DIR_PAGES);
                let elem_offset = bin(self, BinOp::Mul, off, int(size));
                // a small first page that is full grows on the runtime path:
                // room is the page's payload size (header meta, top 32 bits)
                // past its header
                let meta = load(self, page, -8);
                let payload = bin(self, BinOp::Shr, meta, int(32));
                let end =
                    bin(self, BinOp::Add, elem_offset, int(size + badger_runtime::list::PAGE_ELEMS as i64));
                let full = self.assign(Repr::I8, Rvalue::Cmp { op: CmpOp::Gt, a: end, b: payload, ty: u });
                let room = self.block();
                self.term(Term::Branch(full, slow, room));
                self.switch(room);
                let at = bin(self, BinOp::Add, page, elem_offset);
                self.emit(Stmt::Store { base: at, offset: badger_runtime::list::PAGE_ELEMS, repr, value: v });
                if repr == Repr::Ptr {
                    // the write barrier for the page (it may predate a
                    // collection): see `gc::barrier`
                    let meta = load(self, page, -8);
                    let mask = (badger_runtime::gc::MARK
                        | badger_runtime::gc::IMMORTAL
                        | badger_runtime::gc::REMEMBERED) as i64;
                    let bits = bin(self, BinOp::And, meta, int(mask));
                    let old = self.assign(
                        Repr::I8,
                        Rvalue::Cmp {
                            op: CmpOp::Eq,
                            a: bits,
                            b: int(badger_runtime::gc::MARK as i64),
                            ty: s64,
                        },
                    );
                    let (remember, after) = (self.block(), self.block());
                    self.term(Term::Branch(old, remember, after));
                    self.switch(remember);
                    self.rt("badger_rt_barrier", vec![page], Repr::I8);
                    self.term(Term::Goto(after));
                    self.switch(after);
                }
                let written = bin(self, BinOp::Add, off, int(1));
                self.emit(Stmt::Store {
                    base: page,
                    offset: badger_runtime::list::PAGE_COUNT,
                    repr: Repr::I64,
                    value: written,
                });
                let len1 = bin(self, BinOp::Add, len, int(1));
                self.emit(Stmt::Store {
                    base: b,
                    offset: badger_runtime::list::LEN_OFFSET,
                    repr: Repr::I64,
                    value: len1,
                });
                self.term(Term::Goto(join));
                self.switch(slow);
                let tp = self.m.targs_ptr(&targs);
                let rd = self.m.lay.desc(ret_ty) as *const TypeDesc as u64;
                self.call(
                    Callee::Intrinsic { name: "badger_rt_i_list_builder_append".into(), targs: tp, ret: rd },
                    vec![b, v],
                    Repr::I64,
                );
                self.term(Term::Goto(join));
                self.switch(join);
                Some(unit_op())
            }
            "cell_get" => {
                let r = self.repr(ret_ty);
                Some(self.assign(r, Rvalue::Load { base: args[0], offset: 0, repr: r }))
            }
            "cell_set" => {
                let r = self.repr(&targs[0]);
                self.emit(Stmt::Store { base: args[0], offset: 0, repr: r, value: args[1] });
                Some(unit_op())
            }
            "ptr_eq" => Some(self.assign(
                Repr::I8,
                Rvalue::Cmp { op: CmpOp::Eq, a: args[0], b: args[1], ty: NumTy::I(64, false) },
            )),
            _ => {
                let tp = self.m.targs_ptr(&targs);
                let rd = self.m.lay.desc(ret_ty) as *const TypeDesc as u64;
                // a managed result goes straight into a rooted (pointer) local
                let wr = if self.repr(ret_ty) == Repr::Ptr { Repr::Ptr } else { Repr::I64 };
                let w = self.call(
                    Callee::Intrinsic { name: format!("badger_rt_i_{name}"), targs: tp, ret: rd },
                    args,
                    wr,
                );
                if self.is_never(ret_ty) {
                    self.dead();
                    return None;
                }
                Some(self.from_word(w, ret_ty))
            }
        }
    }

    fn to_word(&mut self, v: Operand, t: &Ty) -> Operand {
        let r = self.repr(t);
        if r == Repr::I64 || r == Repr::Ptr {
            return v;
        }
        self.assign(Repr::I64, Rvalue::ToWord(v))
    }

    /// The map expression of `m.entries()` / `m.to_list()`.
    fn map_entries_source(&self, e: ExprId) -> Option<ExprId> {
        let hir = self.hir();
        let is_entries = |def: DefId| {
            let p = self.m.def_path(def);
            self.m.fusion.get(&p).is_some_and(|t| t.kind == "entries")
        };
        match &hir.exprs[e].kind {
            ExprKind::MethodCall { recv, args, .. } if args.is_empty() => {
                let Res::Method(Target::Fn { def, .. }) = self.inst.res.get(&e)? else { return None };
                is_entries(*def).then_some(*recv)
            }
            ExprKind::Call { callee, args } if args.len() == 1 => {
                let ExprKind::Def { def, .. } = &hir.exprs[*callee].kind else { return None };
                is_entries(*def).then_some(args[0])
            }
            _ => None,
        }
    }

    /// Lower `m.entries().map/filter/fold(..)` as one trie walk in the
    /// corresponding std helper. The result is not materialized as a list
    /// of tuples on the way; the callback order (trie order) is unchanged.
    #[allow(clippy::too_many_arguments)]
    fn try_fused_map_entries(
        &mut self,
        e: ExprId,
        recv: ExprId,
        args: &[ExprId],
        outer: DefId,
        _outer_targs: &[Ty],
        ret_ty: &Ty,
        span: Span,
    ) -> Option<Operand> {
        if std::env::var_os("BADGER_NO_MAP_FUSION").is_some() {
            return None;
        }
        let source = self.map_entries_source(recv)?;
        let path = self.m.def_path(outer);
        let is_fold = self.m.fusion.get(&path).is_some_and(|t| t.kind == "fold");
        let kind = if self.is_list_call(outer) && args.len() == 1 {
            // map (0) or filter (1), by the callback's shape
            if self.callback_returns_bool(args[0]) { 1 } else { 0 }
        } else if is_fold && args.len() == 2 {
            2
        } else {
            return None;
        };
        let map_ty = self.ty(source);
        let kv = self.m.lay.args(&map_ty);
        if kv.len() != 2 {
            return None;
        }
        let (new_name, targs) = match kind {
            0 => {
                let out = self.ty(e);
                let u = self.m.lay.args(&out);
                if u.len() != 1 {
                    return None;
                }
                ("map.entries_map", vec![kv[0].clone(), kv[1].clone(), u[0].clone()])
            }
            1 => ("map.entries_filter", vec![kv[0].clone(), kv[1].clone()]),
            _ => {
                let a = self.ty(args[0]);
                ("map.entries_fold", vec![kv[0].clone(), kv[1].clone(), a])
            }
        };
        let new_def = self.hir().try_lang(new_name)?;
        let m = self.lower(source)?;
        let mut ops = vec![m];
        for a in args {
            ops.push(self.lower(*a)?);
        }
        self.call_def(new_def, targs, ops, ret_ty, span)
    }

    /// A callback stage on a `List` (`map`/`filter`): the `call` template, or
    /// the legacy paths. Which of the two it is comes from the callback's
    /// return type, not the name.
    fn is_list_call(&self, def: DefId) -> bool {
        let p = self.m.def_path(def);
        self.m.fusion.get(&p).is_some_and(|t| t.kind == "call")
    }

    /// One `List.map`/`List.filter` stage: (is map, receiver, callback,
    /// type arguments), for method and free-call forms.
    fn list_stage(&self, e: ExprId) -> Option<(bool, ExprId, ExprId, Vec<Ty>)> {
        let hir = self.hir();
        match &hir.exprs[e].kind {
            ExprKind::MethodCall { recv, args, .. } if args.len() == 1 => {
                let Res::Method(Target::Fn { def, args: targs }) = self.inst.res.get(&e)? else {
                    return None;
                };
                if !self.is_list_call(*def) {
                    return None;
                }
                let is_map = !self.callback_returns_bool(args[0]);
                Some((is_map, *recv, args[0], targs.clone()))
            }
            ExprKind::Call { callee, args } if args.len() == 2 => {
                let ExprKind::Def { def, .. } = &hir.exprs[*callee].kind else { return None };
                if !self.is_list_call(*def) {
                    return None;
                }
                let is_map = !self.callback_returns_bool(args[1]);
                let targs = match self.inst.res.get(callee) {
                    Some(Res::Callee { args, .. }) => args.clone(),
                    _ => vec![],
                };
                Some((is_map, args[0], args[1], targs))
            }
            _ => None,
        }
    }

    /// Lower `map(filter(xs,p), f)` as `filter_then_map(xs,p,f)` (and the
    /// mirrored `filter(map(xs,f), p)` as `map_then_filter`). The fused
    /// loop interleaves the stages, so it is used only when the earlier
    /// callback is side-effect free and cannot fail: then its calls are
    /// unobservable and the later callback's calls and failures still occur
    /// on the same elements in the same order. `p` may be a lambda or a
    /// named function; anything else keeps the two-pass form.
    #[allow(clippy::too_many_arguments)]
    fn try_fused_list_map(
        &mut self,
        _e: ExprId,
        recv: ExprId,
        callback: ExprId,
        outer: DefId,
        outer_targs: &[Ty],
        ret_ty: &Ty,
        span: Span,
    ) -> Option<Operand> {
        if std::env::var_os("BADGER_NO_LIST_FUSION").is_some() {
            return None;
        }
        // the eager engine: only a `List` receiver (a `Seq` chain is the
        // sequence pipeline's, below)
        let rt = self.ty(recv);
        if !self.m.lay.is_std(&rt, "List") {
            return None;
        }
        let (inner_is_map, inner_recv, earlier, inner_targs) = self.list_stage(recv)?;
        if !self.is_list_call(outer) {
            return None;
        }
        let outer_is_map = !self.callback_returns_bool(callback);
        let (new_name, targs) = if outer_is_map && !inner_is_map {
            ("list.filter_then_map", outer_targs.to_vec())
        } else if !outer_is_map && inner_is_map {
            ("list.map_then_filter", inner_targs.clone())
        } else {
            return None;
        };
        if !self.callback_pure_infallible(earlier) {
            return None;
        }
        let new_def = self.hir().try_lang(new_name)?;
        let xs = self.lower(inner_recv)?;
        let early = self.lower(earlier)?;
        let late = self.lower(callback)?;
        self.call_def(new_def, targs, vec![xs, early, late], ret_ty, span)
    }

    /// Whether a callback returns `Bool` (the `filter` shape).
    fn callback_returns_bool(&self, cb: ExprId) -> bool {
        let t = self.ty(cb);
        matches!(&*t, TyKind::Fn(ft) if self.m.lay.is_std(&ft.ret, "Bool"))
    }

    /// Whether a callback returns `Option<..>` (the `filter_map` shape).
    fn callback_returns_option(&self, cb: ExprId) -> bool {
        let t = self.ty(cb);
        matches!(&*t, TyKind::Fn(ft) if self.m.lay.is_std(&ft.ret, "Option"))
    }

    /// A lambda whose body has no calls, allocation, mutation, or operation
    /// that can defect. Reads of concrete record/struct fields and total
    /// comparisons/bitwise operations qualify.
    fn callback_pure_infallible(&self, cb: ExprId) -> bool {
        let ExprKind::Lambda(l) = &self.hir().exprs[cb].kind else { return false };
        self.pure_expr(l.body, 0)
    }

    fn pure_expr(&self, e: ExprId, depth: usize) -> bool {
        if depth > 32 {
            return false;
        }
        let hir = self.hir();
        match &hir.exprs[e].kind {
            ExprKind::Lit(_) | ExprKind::Local(_) => true,
            ExprKind::Def { def, .. } => matches!(hir.defs[*def].kind, DefKind::Fn(_)),
            ExprKind::Field { base, .. } => {
                let base_ty = self.ty(*base);
                let record = match &*base_ty {
                    TyKind::Con(d, _) => self.m.g.env.structs.contains_key(d),
                    TyKind::Record(_) => true,
                    TyKind::Ref(_, inner) | TyKind::Forall(_, inner) => match &**inner {
                        TyKind::Con(d, _) => self.m.g.env.structs.contains_key(d),
                        TyKind::Record(_) => true,
                        _ => false,
                    },
                    _ => false,
                };
                record && self.pure_expr(*base, depth + 1)
            }
            ExprKind::Unary { op, operand } => {
                matches!(op, UnOp::Not | UnOp::Ref | UnOp::RefMut) && self.pure_expr(*operand, depth + 1)
            }
            ExprKind::Binary { op, lhs, rhs } => {
                matches!(
                    op,
                    HBin::Eq
                        | HBin::Ne
                        | HBin::Lt
                        | HBin::Le
                        | HBin::Gt
                        | HBin::Ge
                        | HBin::And
                        | HBin::Or
                        | HBin::BitOr
                        | HBin::BitXor
                        | HBin::BitAnd
                ) && self.pure_expr(*lhs, depth + 1)
                    && self.pure_expr(*rhs, depth + 1)
            }
            ExprKind::If { cond, then, els } => {
                self.pure_expr(*cond, depth + 1)
                    && self.pure_expr(*then, depth + 1)
                    && els.is_none_or(|e| self.pure_expr(e, depth + 1))
            }
            ExprKind::Match { scrutinee, arms } => {
                self.pure_expr(*scrutinee, depth + 1)
                    && arms.iter().all(|a| {
                        a.guard.is_none_or(|g| self.pure_expr(g, depth + 1))
                            && self.pure_expr(a.body, depth + 1)
                    })
            }
            ExprKind::Block { stmts, tail } => {
                stmts.iter().all(|s| match s {
                    badger_hir::Stmt::Let { init, .. } => self.pure_expr(*init, depth + 1),
                    badger_hir::Stmt::Assign { value, .. } => self.pure_expr(*value, depth + 1),
                    badger_hir::Stmt::Expr { expr, .. } => self.pure_expr(*expr, depth + 1),
                }) && tail.is_none_or(|t| self.pure_expr(t, depth + 1))
            }
            _ => false,
        }
    }

    fn lower_call(&mut self, e: ExprId, callee: ExprId, args: &[ExprId], ty: &Ty) -> Option<Operand> {
        let hir = self.hir();
        let span = self.span(e);
        match &hir.exprs[callee].kind {
            ExprKind::Def { def, .. } if matches!(hir.defs[*def].kind, DefKind::Fn(_)) => {
                let targs = match self.inst.res.get(&callee) {
                    Some(Res::Callee { args, .. }) => args.clone(),
                    _ => vec![],
                };
                if let Some(&first) = args.first()
                    && let Some(v) = self.try_fused_map_entries(e, first, args, *def, &targs, ty, span)
                {
                    return Some(v);
                }
                if args.len() == 2
                    && let Some(v) = self.try_fused_list_map(e, args[0], args[1], *def, &targs, ty, span)
                {
                    return Some(v);
                }
                let mut ops = vec![];
                for a in args {
                    ops.push(self.lower(*a)?);
                }
                // test assertions report their caller's location, and
                // functions with preconditions blame their call site
                if self.is_test_helper(*def, span) {
                    let l = self.loc(span);
                    self.rt("badger_rt_caller_loc", vec![Operand::Const(Const::Addr(l))], Repr::I8);
                }
                if self.has_requires(*def) {
                    let l = self.loc(span);
                    self.rt("badger_rt_contract_site", vec![Operand::Const(Const::Addr(l))], Repr::I8);
                }
                let r = self.call_def(*def, targs, ops, ty, span)?;
                // a user `err(...)` is where an error starts
                if self.is_prelude_err(*def) {
                    let rl = self.copy_to_local(r, Repr::Ptr, None);
                    let l = self.loc(span);
                    self.rt(
                        "badger_rt_err_origin",
                        vec![int(0), Operand::Local(rl), Operand::Const(Const::Addr(l))],
                        Repr::I8,
                    );
                    return Some(Operand::Local(rl));
                }
                Some(r)
            }
            ExprKind::Ctor { variant, enum_def, .. } => {
                let mut ops = vec![];
                for a in args {
                    let v = self.lower(*a)?;
                    let r = self.lowered_repr(*a);
                    ops.push(Operand::Local(self.copy_to_local(v, r, None)));
                }
                let v = self.alloc_enum(ty, *variant, ops);
                // a user `Result.err(...)` is where an error starts
                if *variant == 1 && Some(*enum_def) == hir.try_lang("Result") && !self.in_std() {
                    let rl = self.copy_to_local(v, Repr::Ptr, None);
                    let l = self.loc(span);
                    self.rt(
                        "badger_rt_err_origin",
                        vec![int(0), Operand::Local(rl), Operand::Const(Const::Addr(l))],
                        Repr::I8,
                    );
                    return Some(Operand::Local(rl));
                }
                Some(v)
            }
            ExprKind::AssocFn { .. } => {
                let target = match self.inst.res.get(&callee).cloned() {
                    Some(Res::Method(t)) => t,
                    _ => {
                        self.m.errors.push("internal: unresolved associated function".into());
                        return Some(unit_op());
                    }
                };
                let mut ops = vec![];
                for a in args {
                    ops.push(self.lower(*a)?);
                }
                let at = args.first().map(|a| self.ty(*a)).unwrap_or_else(badger_typeck::ty::unit);
                self.call_target(&target, ops, &at, ty, span)
            }
            _ => {
                let f = self.lower(callee)?;
                let fl = self.copy_to_local(f, Repr::Ptr, None);
                let mut ops = vec![];
                for a in args {
                    ops.push(self.lower(*a)?);
                }
                let r = self.repr(ty);
                let v = self.call(Callee::Closure(Operand::Local(fl)), ops, r);
                if self.is_never(ty) {
                    self.dead();
                    return None;
                }
                Some(v)
            }
        }
    }

    fn lower_try(&mut self, e: ExprId, inner: ExprId, span: Span) -> Option<Operand> {
        let r = self.lower(inner)?;
        let rt = self.ty(inner);
        if self.m.lay.is_std(&rt, "Option") {
            return self.lower_try_option(e, r, &rt, span);
        }
        let rl = self.copy_to_local(r, Repr::Ptr, None);
        let tag = self.assign(Repr::I32, Rvalue::Tag(Operand::Local(rl)));
        let (okb, errb) = (self.block(), self.block());
        self.term(Term::Switch(tag, vec![(0, okb)], errb));
        self.switch(errb);
        let (ev, _) = self.variant_field(&rt, 1, 0, Operand::Local(rl));
        let ret_ty = self.ret_ty.clone();
        let err_ty = self.m.lay.args(&ret_ty).get(1).cloned().unwrap_or_else(badger_typeck::ty::unit);
        let conv = match self.inst.res.get(&e).cloned() {
            Some(Res::Try(Some(t))) => {
                let src = self.m.lay.args(&rt).get(1).cloned().unwrap_or_else(badger_typeck::ty::unit);
                self.call_target(&t, vec![ev], &src, &err_ty, span)?
            }
            _ => ev,
        };
        let cl = self.copy_to_local(conv, self.repr(&err_ty), None);
        let out = self.alloc_enum(&ret_ty, 1, vec![Operand::Local(cl)]);
        let ol = self.copy_to_local(out, Repr::Ptr, None);
        if self.m.lay.is_std(&rt, "Result") {
            let l = self.loc(span);
            self.rt(
                "badger_rt_err_origin",
                vec![Operand::Local(rl), Operand::Local(ol), Operand::Const(Const::Addr(l))],
                Repr::I8,
            );
        }
        self.close_iters(0);
        self.ret(Operand::Local(ol));
        self.switch(okb);
        let (v, _) = self.variant_field(&rt, 0, 0, Operand::Local(rl));
        Some(v)
    }

    /// `opt?`: `none` returns `err(E.from(OptionMissing.missing))`.
    fn lower_try_option(&mut self, e: ExprId, r: Operand, rt: &Ty, span: Span) -> Option<Operand> {
        let rl = self.copy_to_local(r, Repr::Ptr, None);
        let tag = self.assign(Repr::I32, Rvalue::Tag(Operand::Local(rl)));
        let (someb, noneb) = (self.block(), self.block());
        self.term(Term::Switch(tag, vec![(1, someb)], noneb));
        self.switch(noneb);
        let missing = self.hir().lang("OptionMissing");
        let src = badger_typeck::ty::mk(badger_typeck::ty::TyKind::Con(missing, vec![]));
        let mv = self.alloc_enum(&src, 0, vec![]);
        let ret_ty = self.ret_ty.clone();
        let err_ty = self.m.lay.args(&ret_ty).get(1).cloned().unwrap_or_else(badger_typeck::ty::unit);
        let conv = match self.inst.res.get(&e).cloned() {
            Some(Res::Try(Some(t))) => self.call_target(&t, vec![mv], &src, &err_ty, span)?,
            _ => mv,
        };
        let cl = self.copy_to_local(conv, self.repr(&err_ty), None);
        let out = self.alloc_enum(&ret_ty, 1, vec![Operand::Local(cl)]);
        let ol = self.copy_to_local(out, Repr::Ptr, None);
        self.close_iters(0);
        self.ret(Operand::Local(ol));
        self.switch(someb);
        let (v, _) = self.variant_field(rt, 1, 0, Operand::Local(rl));
        Some(v)
    }

    fn lower_binary(
        &mut self,
        e: ExprId,
        op: HBin,
        lhs: ExprId,
        rhs: ExprId,
        ty: &Ty,
        span: Span,
    ) -> Option<Operand> {
        match op {
            HBin::And | HBin::Or => {
                let a = self.lower(lhs)?;
                let res = self.fresh(Repr::I8, None);
                self.emit(Stmt::Assign(res, Rvalue::Use(a)));
                let (rb, join) = (self.block(), self.block());
                if op == HBin::And {
                    self.term(Term::Branch(a, rb, join));
                } else {
                    self.term(Term::Branch(a, join, rb));
                }
                self.switch(rb);
                if let Some(b) = self.lower(rhs) {
                    self.emit(Stmt::Assign(res, Rvalue::Use(b)));
                    self.term(Term::Goto(join));
                }
                self.switch(join);
                Some(Operand::Local(res))
            }
            HBin::Eq | HBin::Ne | HBin::Lt | HBin::Le | HBin::Gt | HBin::Ge => {
                let a = self.lower(lhs)?;
                let at = self.ty(lhs);
                let al = self.copy_to_local(a, self.repr(&at), None);
                let b = self.lower(rhs)?;
                let cop = match op {
                    HBin::Eq => CmpOp::Eq,
                    HBin::Ne => CmpOp::Ne,
                    HBin::Lt => CmpOp::Lt,
                    HBin::Le => CmpOp::Le,
                    HBin::Gt => CmpOp::Gt,
                    _ => CmpOp::Ge,
                };
                Some(self.compare(cop, Operand::Local(al), b, &at))
            }
            HBin::Range | HBin::RangeIncl => {
                let a = self.lower(lhs)?;
                let at = self.ty(lhs);
                let al = self.copy_to_local(a, self.repr(&at), None);
                let b = self.lower(rhs)?;
                let nt = self.m.lay.num_ty(&at).unwrap_or(NumTy::I(64, true));
                let a64 = self.assign(
                    Repr::I64,
                    Rvalue::Convert {
                        a: Operand::Local(al),
                        from: nt,
                        to: NumTy::I(64, true),
                        checked: false,
                        loc: 0,
                    },
                );
                let b64 = self.assign(
                    Repr::I64,
                    Rvalue::Convert { a: b, from: nt, to: NumTy::I(64, true), checked: false, loc: 0 },
                );
                let desc = self.m.lay.desc(ty);
                let obj = self.assign(Repr::Ptr, Rvalue::Alloc { desc: desc as *const _ as u64, size: 24 });
                self.emit(Stmt::Store { base: obj, offset: 0, repr: Repr::I64, value: a64 });
                self.emit(Stmt::Store { base: obj, offset: 8, repr: Repr::I64, value: b64 });
                self.emit(Stmt::Store {
                    base: obj,
                    offset: 16,
                    repr: Repr::I64,
                    value: int((op == HBin::RangeIncl) as i64),
                });
                Some(obj)
            }
            _ => {
                let a = self.lower(lhs)?;
                let at = self.ty(lhs);
                let al = self.copy_to_local(a, self.repr(&at), None);
                let b = self.lower(rhs)?;
                let _ = e;
                let bop = match op {
                    HBin::Add => BinOp::Add,
                    HBin::Sub => BinOp::Sub,
                    HBin::Mul => BinOp::Mul,
                    HBin::Div => BinOp::Div,
                    HBin::Rem => BinOp::Rem,
                    HBin::BitAnd => BinOp::And,
                    HBin::BitOr => BinOp::Or,
                    HBin::BitXor => BinOp::Xor,
                    HBin::Shl => BinOp::Shl,
                    _ => BinOp::Shr,
                };
                let nt = self.m.lay.num_ty(&at).unwrap_or(NumTy::I(64, true));
                // the contract analysis proved this cannot overflow: no check
                let l = if matches!(bop, BinOp::Add | BinOp::Sub | BinOp::Mul) && elided_arith(e) {
                    0
                } else {
                    self.loc(span)
                };
                let v = self
                    .assign(self.repr(ty), Rvalue::Bin { op: bop, a: Operand::Local(al), b, ty: nt, loc: l });
                // durations are never negative
                if self.m.lay.is_std(ty, "Duration") {
                    let vl = self.copy_to_local(v, Repr::I64, None);
                    let neg = self.assign(
                        Repr::I8,
                        Rvalue::Cmp {
                            op: CmpOp::Lt,
                            a: Operand::Local(vl),
                            b: int(0),
                            ty: NumTy::I(64, true),
                        },
                    );
                    let (bad, ok) = (self.block(), self.block());
                    self.term(Term::Branch(neg, bad, ok));
                    self.switch(bad);
                    self.defect(8, span);
                    self.term(Term::Unreachable);
                    self.switch(ok);
                    return Some(Operand::Local(vl));
                }
                Some(v)
            }
        }
    }

    fn compare(&mut self, op: CmpOp, a: Operand, b: Operand, t: &Ty) -> Operand {
        if let Some(nt) = self.m.lay.num_ty(t) {
            return self.assign(Repr::I8, Rvalue::Cmp { op, a, b, ty: nt });
        }
        if self.m.lay.is_std(t, "Symbol") && matches!(op, CmpOp::Eq | CmpOp::Ne) {
            return self.assign(Repr::I8, Rvalue::Cmp { op, a, b, ty: NumTy::I(64, false) });
        }
        // small records and tuples of scalars: field by field, without the
        // runtime's descriptor walk (the same semantics: structural, with
        // each field compared as its own type)
        if matches!(op, CmpOp::Eq | CmpOp::Ne) && self.scalar_fields(t, 0).is_some_and(|n| n <= 8) {
            let eq = self.fields_equal(a, b, t);
            return if op == CmpOp::Eq { eq } else { self.assign(Repr::I8, Rvalue::Not(eq)) };
        }
        let d = self.desc_op(t);
        match op {
            CmpOp::Eq | CmpOp::Ne => {
                let r = self.rt("badger_rt_eq", vec![d, a, b], Repr::I8);
                if op == CmpOp::Eq { r } else { self.assign(Repr::I8, Rvalue::Not(r)) }
            }
            _ => {
                let c = self.rt("badger_rt_cmp", vec![d, a, b], Repr::I64);
                self.assign(Repr::I8, Rvalue::Cmp { op, a: c, b: int(0), ty: NumTy::I(64, true) })
            }
        }
    }

    /// The number of scalar leaves of a record or tuple whose every field
    /// (recursively) is a number, `Bool`, `Char`, or `Symbol`; `None`
    /// otherwise (or when nested too deeply).
    fn scalar_fields(&mut self, t: &Ty, depth: u32) -> Option<usize> {
        let shaped = match &**t {
            TyKind::Record(_) | TyKind::Tuple(_) => true,
            TyKind::Con(d, _) => self.m.g.env.structs.contains_key(d),
            _ => false,
        };
        if depth > 3 || !shaped {
            return None;
        }
        let layout = self.m.lay.record(t);
        if layout.fields.is_empty() {
            return None;
        }
        let mut n = 0;
        for f in &layout.fields {
            if self.m.lay.num_ty(&f.ty).is_some() || self.m.lay.is_std(&f.ty, "Symbol") {
                n += 1;
            } else {
                n += self.scalar_fields(&f.ty, depth + 1)?;
            }
        }
        Some(n)
    }

    /// Structural equality of two values of record/tuple type `t` (see
    /// [`Self::scalar_fields`]): field by field, stopping at the first
    /// difference.
    fn fields_equal(&mut self, a: Operand, b: Operand, t: &Ty) -> Operand {
        let a = Operand::Local(self.copy_to_local(a, Repr::Ptr, None));
        let b = Operand::Local(self.copy_to_local(b, Repr::Ptr, None));
        let res = self.fresh(Repr::I8, None);
        self.emit(Stmt::Assign(res, Rvalue::Use(Operand::Const(Const::Int(0, Repr::I8)))));
        let done = self.block();
        let layout = self.m.lay.record(t);
        for f in &layout.fields {
            let x = self.assign(f.repr, Rvalue::Load { base: a, offset: f.offset, repr: f.repr });
            let y = self.assign(f.repr, Rvalue::Load { base: b, offset: f.offset, repr: f.repr });
            let same = self.compare(CmpOp::Eq, x, y, &f.ty);
            let next = self.block();
            self.term(Term::Branch(same, next, done));
            self.switch(next);
        }
        self.emit(Stmt::Assign(res, Rvalue::Use(Operand::Const(Const::Int(1, Repr::I8)))));
        self.term(Term::Goto(done));
        self.switch(done);
        Operand::Local(res)
    }

    fn lower_record(
        &mut self,
        ty: &Ty,
        spread: Option<ExprId>,
        fields: &[(Symbol, Span, ExprId)],
    ) -> Option<Operand> {
        let base = match spread {
            Some(s) => {
                let v = self.lower(s)?;
                Some((self.copy_to_local(v, Repr::Ptr, None), self.ty(s)))
            }
            None => None,
        };
        if base.is_none() && self.m.lay.is_std(ty, "Map") {
            // contextual construction: labels become symbol keys
            let vt = self.m.lay.args(ty).get(1).cloned().unwrap_or_else(badger_typeck::ty::unit);
            let vr = self.repr(&vt);
            let mut kvs = vec![];
            for (l, _, x) in fields {
                let v = self.lower(*x)?;
                let vl = self.copy_to_local(v, vr, None);
                let name = self.m.sym(*l).to_string();
                kvs.push((Operand::Const(Const::Addr(self.m.symbol(&name))), Operand::Local(vl)));
            }
            let d = self.desc_op(ty);
            let m = self.rt("badger_rt_map_new", vec![d], Repr::Ptr);
            let m = Operand::Local(self.copy_to_local(m, Repr::Ptr, None));
            for (k, v) in kvs {
                self.rt("badger_rt_map_insert_mut", vec![m, k, v], Repr::I8);
            }
            return Some(m);
        }
        let mut vals: Vec<(Symbol, Operand)> = vec![];
        for (l, _, x) in fields {
            let v = self.lower(*x)?;
            let r = self.lowered_repr(*x);
            vals.push((*l, Operand::Local(self.copy_to_local(v, r, None))));
        }
        let layout = self.m.lay.record(ty);
        let desc = self.m.lay.desc(ty);
        let obj = self.assign(Repr::Ptr, Rvalue::Alloc { desc: desc as *const _ as u64, size: layout.size });
        for fl in &layout.fields {
            let v = match vals.iter().find(|v| v.0 == fl.label) {
                Some((_, v)) => *v,
                None => match &base {
                    Some((b, bt)) => {
                        let (b, bt) = (*b, bt.clone());
                        self.load_field(&bt, fl.label, Operand::Local(b))
                    }
                    None => unit_op(),
                },
            };
            self.emit(Stmt::Store { base: obj, offset: fl.offset, repr: fl.repr, value: v });
        }
        Some(obj)
    }

    fn lower_lambda(&mut self, e: ExprId, l: &badger_hir::LambdaExpr, ty: &Ty) -> Operand {
        let TyKind::Fn(ft) = &**ty else { return unit_op() };
        let ret = self.repr(&ft.ret);
        let name = format!("{}.lambda{}", self.func.name, e.into_raw().into_u32());
        let fid = self.m.prog.funcs.push(Func::new(name, FuncKind::Lambda, ret));
        self.m.prog.funcs[fid].needs_adapter = true;
        let cap_tys: Vec<Ty> = l
            .captures
            .iter()
            .map(|c| self.inst.locals.get(c).cloned().unwrap_or_else(badger_typeck::ty::unit))
            .collect();
        let layout = self.m.lay.closure(&cap_tys);
        {
            let mut fl = FnLower::new(self.m, self.inst.clone(), Owner::Lambda(e), ft.ret.clone(), fid);
            let mut params = vec![];
            for p in &l.params {
                let t = fl.pat_ty(p.pat);
                let x = fl.fresh(fl.repr(&t), None);
                params.push((x, p.pat, t));
            }
            fl.func.params = params.iter().map(|p| p.0).collect();
            fl.emit(Stmt::Poll);
            let env = Operand::Local(fl.func.env);
            for (c, f) in l.captures.iter().zip(&layout.fields) {
                let v = fl.assign(f.repr, Rvalue::Load { base: env, offset: f.offset, repr: f.repr });
                let Operand::Local(x) = v else { unreachable!() };
                fl.locals.insert(*c, x);
            }
            for (x, pat, t) in params {
                fl.bind_irrefutable(pat, Operand::Local(x), &t);
            }
            if let Some(v) = fl.lower(l.body) {
                fl.term(Term::Return(v));
            }
            fl.finish();
        }
        let desc = self.m.lay.closure_desc(&layout);
        let obj = self.assign(Repr::Ptr, Rvalue::Alloc { desc: desc as *const _ as u64, size: layout.size });
        let code = self.assign(Repr::I64, Rvalue::FuncAddr(fid));
        let adapter = self.assign(Repr::I64, Rvalue::AdapterAddr(fid));
        self.emit(Stmt::Store { base: obj, offset: 0, repr: Repr::I64, value: code });
        self.emit(Stmt::Store { base: obj, offset: 8, repr: Repr::I64, value: adapter });
        for (c, f) in l.captures.iter().zip(&layout.fields) {
            let v = match self.locals.get(c) {
                Some(x) => Operand::Local(*x),
                None => unit_op(),
            };
            self.emit(Stmt::Store { base: obj, offset: f.offset, repr: f.repr, value: v });
        }
        obj
    }

    /// Close the iterators of sequence loops `from..` that control is leaving.
    fn close_iters(&mut self, from: usize) {
        let closers: Vec<(FuncId, Local)> =
            self.loops[from.min(self.loops.len())..].iter().rev().filter_map(|l| l.close).collect();
        for (f, l) in closers {
            let r = self.m.prog.funcs[f].ret;
            self.call(Callee::Direct(f), vec![Operand::Local(l)], r);
        }
    }

    fn lower_for(&mut self, e: ExprId, pat: PatId, iter: ExprId, body: ExprId) -> Option<()> {
        let kind = match self.inst.res.get(&e) {
            Some(Res::Iter(k)) => *k,
            _ => IterKind::List,
        };
        if self.m.fuse_sequences
            && kind == IterKind::Seq
            && let Some(plan) = self.fused_for(iter, pat, body)
        {
            self.lower_fused(e, plan)?;
            return Some(());
        }
        // `for p in map.to_list(m)` (or `set.to_list(s)`) walks `m` itself:
        // the list would be a temporary nobody else sees, built only to be
        // consumed in the trie's order, which the walk follows
        let (kind, iter) = match self.to_list_of(iter) {
            Some((k, m)) => (k, m),
            None => (kind, iter),
        };
        let it = self.lower(iter)?;
        let it_ty = self.ty(iter);
        let itl = self.copy_to_local(it, Repr::Ptr, None);
        let item_ty = self.pat_ty(pat);
        let (head, bodyb, step, exit) = (self.block(), self.block(), self.block(), self.block());
        match kind {
            IterKind::Range => {
                let start = self.assign(
                    Repr::I64,
                    Rvalue::Load { base: Operand::Local(itl), offset: 0, repr: Repr::I64 },
                );
                let end = self.assign(
                    Repr::I64,
                    Rvalue::Load { base: Operand::Local(itl), offset: 8, repr: Repr::I64 },
                );
                let incl = self.assign(
                    Repr::I64,
                    Rvalue::Load { base: Operand::Local(itl), offset: 16, repr: Repr::I64 },
                );
                let i = self.copy_to_local(start, Repr::I64, None);
                // the bounds are the element type's values widened to 64 bits
                // (unsigned ones zero-extended): compare in its own order
                let nt = self.m.lay.num_ty(&item_ty).unwrap_or(NumTy::I(64, true));
                let wide = match nt {
                    NumTy::I(_, signed) => NumTy::I(64, signed),
                    NumTy::F(_) => NumTy::I(64, true),
                };
                self.term(Term::Goto(head));
                self.switch(head);
                // a short constant inner loop needs no poll of its own: its
                // work between the enclosing loop's (or function's) polls is
                // bounded
                if !self.short_inner_loop(iter, body) {
                    self.emit(Stmt::Poll);
                }
                let lt = self
                    .assign(Repr::I8, Rvalue::Cmp { op: CmpOp::Lt, a: Operand::Local(i), b: end, ty: wide });
                let eq = self.assign(
                    Repr::I8,
                    Rvalue::Cmp { op: CmpOp::Eq, a: Operand::Local(i), b: end, ty: NumTy::I(64, true) },
                );
                let inc8 = self.assign(
                    Repr::I8,
                    Rvalue::Convert {
                        a: incl,
                        from: NumTy::I(64, false),
                        to: NumTy::I(8, false),
                        checked: false,
                        loc: 0,
                    },
                );
                let both = self.assign(
                    Repr::I8,
                    Rvalue::Bin { op: BinOp::And, a: eq, b: inc8, ty: NumTy::I(8, false), loc: 0 },
                );
                let c = self.assign(
                    Repr::I8,
                    Rvalue::Bin { op: BinOp::Or, a: lt, b: both, ty: NumTy::I(8, false), loc: 0 },
                );
                self.term(Term::Branch(c, bodyb, exit));
                self.switch(bodyb);
                let x = self.assign(
                    self.repr(&item_ty),
                    Rvalue::Convert { a: Operand::Local(i), from: wide, to: nt, checked: false, loc: 0 },
                );
                self.bind_irrefutable(pat, x, &item_ty);
                self.loops.push(LoopCtx { expr: e, cont: step, brk: exit, result: None, close: None });
                if self.lower(body).is_some() {
                    self.term(Term::Goto(step));
                }
                self.loops.pop();
                self.switch(step);
                // stop before overflowing at the inclusive upper bound
                let last = self.assign(
                    Repr::I8,
                    Rvalue::Cmp { op: CmpOp::Eq, a: Operand::Local(i), b: end, ty: NumTy::I(64, true) },
                );
                let next = self.block();
                self.term(Term::Branch(last, exit, next));
                self.switch(next);
                let n = self.assign(
                    Repr::I64,
                    Rvalue::Bin { op: BinOp::Add, a: Operand::Local(i), b: int(1), ty: wide, loc: 0 },
                );
                self.emit(Stmt::Assign(i, Rvalue::Use(n)));
                self.term(Term::Goto(head));
            }
            IterKind::Seq | IterKind::Stream => {
                // a sequence's `start`, or a stream's `next` (fields are laid
                // out by label, so look it up)
                let seq_l = self.m.lay.record(&it_ty);
                let want = if kind == IterKind::Seq { "start" } else { "next" };
                let start_f = seq_l.fields.iter().find(|f| f.name == want).expect("iteration field").clone();
                let start = self.assign(
                    Repr::Ptr,
                    Rvalue::Load { base: Operand::Local(itl), offset: start_f.offset, repr: Repr::Ptr },
                );
                // a sequence starts a fresh iterator; a stream is its own iterator
                let next_fn = if kind == IterKind::Seq {
                    self.call(Callee::Closure(start), vec![], Repr::Ptr)
                } else {
                    start
                };
                let nf = self.copy_to_local(next_fn, Repr::Ptr, None);
                let close = if kind == IterKind::Seq {
                    self.m.lang_fn("seq.close_iter", vec![item_ty.clone()]).map(|f| (f, nf))
                } else {
                    None
                };
                self.term(Term::Goto(head));
                self.switch(head);
                self.emit(Stmt::Poll);
                let r = self.call(Callee::Closure(Operand::Local(nf)), vec![], Repr::Ptr);
                let rl = self.copy_to_local(r, Repr::Ptr, None);
                let tag = self.assign(Repr::I32, Rvalue::Tag(Operand::Local(rl)));
                self.term(Term::Switch(tag, vec![(1, bodyb)], exit));
                self.switch(bodyb);
                let opt_ty = badger_typeck::ty::mk(TyKind::Con(self.m.g.env.b.option, vec![item_ty.clone()]));
                let (x, _) = self.variant_field(&opt_ty, 1, 0, Operand::Local(rl));
                self.bind_irrefutable(pat, x, &item_ty);
                self.loops.push(LoopCtx { expr: e, cont: head, brk: exit, result: None, close });
                if self.lower(body).is_some() {
                    self.term(Term::Goto(head));
                }
                self.loops.pop();
                self.term(Term::Goto(head));
                self.switch(step);
                self.term(Term::Goto(head));
                if let Some((f, l)) = close {
                    // exhausted or broken out of: close scoped resources
                    self.switch(exit);
                    let r = self.m.prog.funcs[f].ret;
                    self.call(Callee::Direct(f), vec![Operand::Local(l)], r);
                }
            }
            IterKind::List if self.paged_elem(&it_ty).is_some() => {
                // page by page: one runtime call per page, then direct
                // loads of the page's elements (pages never move and a
                // version's elements never change; the page stays rooted
                // through its word conversion)
                let repr = self.paged_elem(&it_ty).unwrap();
                let size = repr.size() as i64;
                let list = itl;
                let n = self.rt("badger_rt_list_len", vec![Operand::Local(list)], Repr::I64);
                let n = self.copy_to_local(n, Repr::I64, None);
                let cap = self.rt("badger_rt_list_page_cap", vec![Operand::Local(list)], Repr::I64);
                let cap = self.copy_to_local(cap, Repr::I64, None);
                let i = self.copy_to_local(int(0), Repr::I64, None);
                // slot within the current page; starts full to fetch page 0
                let slot = self.copy_to_local(Operand::Local(cap), Repr::I64, None);
                let cur = self.copy_to_local(int(0), Repr::I64, None);
                let (check, fetch) = (self.block(), self.block());
                self.term(Term::Goto(head));
                self.switch(head);
                self.emit(Stmt::Poll);
                let more = self.assign(
                    Repr::I8,
                    Rvalue::Cmp {
                        op: CmpOp::Lt,
                        a: Operand::Local(i),
                        b: Operand::Local(n),
                        ty: NumTy::I(64, true),
                    },
                );
                self.term(Term::Branch(more, check, exit));
                self.switch(check);
                let in_page = self.assign(
                    Repr::I8,
                    Rvalue::Cmp {
                        op: CmpOp::Lt,
                        a: Operand::Local(slot),
                        b: Operand::Local(cap),
                        ty: NumTy::I(64, true),
                    },
                );
                self.term(Term::Branch(in_page, bodyb, fetch));
                self.switch(fetch);
                let page =
                    self.rt("badger_rt_list_page", vec![Operand::Local(list), Operand::Local(i)], Repr::Ptr);
                let page = self.copy_to_local(page, Repr::Ptr, None);
                let w = self.assign(Repr::I64, Rvalue::ToWord(Operand::Local(page)));
                self.emit(Stmt::Assign(cur, Rvalue::Use(w)));
                self.emit(Stmt::Assign(slot, Rvalue::Use(int(0))));
                self.term(Term::Goto(bodyb));
                self.switch(bodyb);
                let x = self.assign(
                    repr,
                    Rvalue::Load {
                        base: Operand::Local(cur),
                        offset: badger_runtime::list::PAGE_HDR as i32,
                        repr,
                    },
                );
                let x = if repr == Repr::Ptr {
                    Operand::Local(self.copy_to_local(x, Repr::Ptr, None))
                } else {
                    x
                };
                self.bind_irrefutable(pat, x, &item_ty);
                self.loops.push(LoopCtx { expr: e, cont: step, brk: exit, result: None, close: None });
                if self.lower(body).is_some() {
                    self.term(Term::Goto(step));
                }
                self.loops.pop();
                self.switch(step);
                for (l, d) in [(i, 1), (slot, 1), (cur, size)] {
                    let nx = self.assign(
                        Repr::I64,
                        Rvalue::Bin {
                            op: BinOp::Add,
                            a: Operand::Local(l),
                            b: int(d),
                            ty: NumTy::I(64, true),
                            loc: 0,
                        },
                    );
                    self.emit(Stmt::Assign(l, Rvalue::Use(nx)));
                }
                self.term(Term::Goto(head));
            }
            IterKind::Map | IterKind::Set => {
                // a walk over the trie: no list of entries is built, and a
                // `(key, value)` pattern binds both without a tuple
                let args = self.m.lay.args(&it_ty).to_vec();
                let cur = self.rt("badger_rt_map_cursor", vec![Operand::Local(itl)], Repr::Ptr);
                let cur = self.copy_to_local(cur, Repr::Ptr, None);
                self.term(Term::Goto(head));
                self.switch(head);
                self.emit(Stmt::Poll);
                let more = self.rt("badger_rt_map_next", vec![Operand::Local(cur)], Repr::I64);
                let c = self.assign(
                    Repr::I8,
                    Rvalue::Cmp { op: CmpOp::Ne, a: more, b: int(0), ty: NumTy::I(64, true) },
                );
                self.term(Term::Branch(c, bodyb, exit));
                self.switch(bodyb);
                let word = |s: &mut Self, offset: i32| {
                    s.assign(Repr::I64, Rvalue::Load { base: Operand::Local(cur), offset, repr: Repr::I64 })
                };
                let entry = word(self, badger_runtime::hamt::CURSOR_ENTRY);
                let entry = Operand::Local(self.copy_to_local(entry, Repr::I64, None));
                // entry points at the node's (hash, key, value) triple;
                // `Load` adds the object header, so key is at -8 and value
                // at 0
                let kw = self.assign(Repr::I64, Rvalue::Load { base: entry, offset: -8, repr: Repr::I64 });
                let k = self.from_word(kw, &args[0]);
                if kind == IterKind::Set {
                    self.bind_irrefutable(pat, k, &item_ty);
                } else {
                    let vw = self.assign(Repr::I64, Rvalue::Load { base: entry, offset: 0, repr: Repr::I64 });
                    let v = self.from_word(vw, &args[1]);
                    match self.hir().pats[pat].kind.clone() {
                        PatKind::Tuple(ps) if ps.len() == 2 => {
                            self.bind_irrefutable(ps[0], k, &args[0]);
                            self.bind_irrefutable(ps[1], v, &args[1]);
                        }
                        _ => {
                            // the entry as a tuple value
                            let layout = self.m.lay.record(&item_ty);
                            let desc = self.m.lay.desc(&item_ty);
                            let obj = self.assign(
                                Repr::Ptr,
                                Rvalue::Alloc { desc: desc as *const _ as u64, size: layout.size },
                            );
                            let obj = Operand::Local(self.copy_to_local(obj, Repr::Ptr, None));
                            for (fl, x) in layout.fields.iter().zip([k, v]) {
                                self.emit(Stmt::Store {
                                    base: obj,
                                    offset: fl.offset,
                                    repr: fl.repr,
                                    value: x,
                                });
                            }
                            self.bind_irrefutable(pat, obj, &item_ty);
                        }
                    }
                }
                self.loops.push(LoopCtx { expr: e, cont: head, brk: exit, result: None, close: None });
                if self.lower(body).is_some() {
                    self.term(Term::Goto(head));
                }
                self.loops.pop();
                self.switch(step);
                self.term(Term::Goto(head));
            }
            IterKind::Array => {
                let data = Operand::Local(itl);
                let n = self.buffer_len(data);
                let i = self.copy_to_local(int(0), Repr::I64, None);
                self.term(Term::Goto(head));
                self.switch(head);
                self.emit(Stmt::Poll);
                let c = self.assign(
                    Repr::I8,
                    Rvalue::Cmp { op: CmpOp::Lt, a: Operand::Local(i), b: n, ty: NumTy::I(64, false) },
                );
                self.term(Term::Branch(c, bodyb, exit));
                self.switch(bodyb);
                // Reload through the owner in the loop so its managed root
                // stays live across body calls, polls and suspension.
                let data = Operand::Local(itl);
                let x = self.buffer_load(data, Operand::Local(i), self.repr(&item_ty));
                self.bind_irrefutable(pat, x, &item_ty);
                self.loops.push(LoopCtx { expr: e, cont: step, brk: exit, result: None, close: None });
                if self.lower(body).is_some() {
                    self.term(Term::Goto(step));
                }
                self.loops.pop();
                self.switch(step);
                let next = self.assign(
                    Repr::I64,
                    Rvalue::Bin {
                        op: BinOp::Add,
                        a: Operand::Local(i),
                        b: int(1),
                        ty: NumTy::I(64, false),
                        loc: 0,
                    },
                );
                self.emit(Stmt::Assign(i, Rvalue::Use(next)));
                self.term(Term::Goto(head));
            }
            _ => {
                // lists and list-backed collections
                let list = itl;
                let n = self.rt("badger_rt_list_len", vec![Operand::Local(list)], Repr::I64);
                let i = self.copy_to_local(int(0), Repr::I64, None);
                self.term(Term::Goto(head));
                self.switch(head);
                self.emit(Stmt::Poll);
                let c = self.assign(
                    Repr::I8,
                    Rvalue::Cmp { op: CmpOp::Lt, a: Operand::Local(i), b: n, ty: NumTy::I(64, true) },
                );
                self.term(Term::Branch(c, bodyb, exit));
                self.switch(bodyb);
                let l0 = self.m.loc(Span::dummy());
                let w = self.rt(
                    "badger_rt_list_get",
                    vec![Operand::Local(list), Operand::Local(i), Operand::Const(Const::Addr(l0))],
                    Repr::I64,
                );
                let x = self.from_word(w, &item_ty);
                self.bind_irrefutable(pat, x, &item_ty);
                self.loops.push(LoopCtx { expr: e, cont: step, brk: exit, result: None, close: None });
                if self.lower(body).is_some() {
                    self.term(Term::Goto(step));
                }
                self.loops.pop();
                self.switch(step);
                let nx = self.assign(
                    Repr::I64,
                    Rvalue::Bin {
                        op: BinOp::Add,
                        a: Operand::Local(i),
                        b: int(1),
                        ty: NumTy::I(64, true),
                        loc: 0,
                    },
                );
                self.emit(Stmt::Assign(i, Rvalue::Use(nx)));
                self.term(Term::Goto(head));
            }
        }
        self.switch(exit);
        Some(())
    }

    fn buffer_len(&mut self, data: Operand) -> Operand {
        self.assign(
            Repr::I64,
            Rvalue::Load { base: data, offset: badger_runtime::buffer::ITEMS_LEN, repr: Repr::I64 },
        )
    }

    fn buffer_load(&mut self, data: Operand, index: Operand, repr: Repr) -> Operand {
        let ptr = self.assign(
            Repr::I64,
            Rvalue::Load { base: data, offset: badger_runtime::buffer::ITEMS_PTR, repr: Repr::I64 },
        );
        let offset = self.assign(
            Repr::I64,
            Rvalue::Bin {
                op: BinOp::Mul,
                a: index,
                b: int(repr.size() as i64),
                ty: NumTy::I(64, false),
                loc: 0,
            },
        );
        let ptr = self.assign(
            Repr::I64,
            Rvalue::Bin { op: BinOp::Add, a: ptr, b: offset, ty: NumTy::I(64, false), loc: 0 },
        );
        self.assign(repr, Rvalue::Load { base: ptr, offset: -(badger_runtime::gc::HEADER as i32), repr })
    }

    /// A range of integer literals with at most 64 values, iterated by a
    /// body that has no loop of its own (calls poll on entry).
    fn short_inner_loop(&self, iter: ExprId, body: ExprId) -> bool {
        let hir = self.hir();
        let ExprKind::Binary { op, lhs, rhs } = &hir.exprs[iter].kind else { return false };
        let lit = |x: ExprId| match &hir.exprs[x].kind {
            ExprKind::Lit(badger_hir::Lit::Int { value, .. }) => Some(*value),
            _ => None,
        };
        let (Some(a), Some(b)) = (lit(*lhs), lit(*rhs)) else { return false };
        let n = b.saturating_sub(a) + (*op == HBin::RangeIncl) as u128;
        if !matches!(op, HBin::Range | HBin::RangeIncl) || n > 64 {
            return false;
        }
        let mut looping = false;
        badger_typeck::program::walk_exprs(hir, body, &mut |e| {
            looping |= matches!(
                hir.exprs[e].kind,
                ExprKind::For { .. } | ExprKind::While { .. } | ExprKind::Loop { .. }
            );
        });
        !looping
    }

    /// A temporary standard collection view consumed directly by a loop.
    /// Stored lists retain their representation. Resolve method definitions,
    /// not spellings, so user-defined `entries` methods remain ordinary calls.
    fn to_list_of(&self, x: ExprId) -> Option<(IterKind, ExprId)> {
        let hir = self.hir();
        let (def, source) = match &hir.exprs[x].kind {
            ExprKind::Call { callee, args } if args.len() == 1 => {
                let ExprKind::Def { def, .. } = &hir.exprs[*callee].kind else { return None };
                (*def, args[0])
            }
            ExprKind::MethodCall { recv, args, .. } if args.is_empty() => {
                let Res::Method(Target::Fn { def, .. }) = self.inst.res.get(&x)? else { return None };
                (*def, *recv)
            }
            _ => return None,
        };
        if hir.try_lang("map.to_list") == Some(def)
            || (self.m.fuse_sequences
                && hir.modules[hir.defs[def].module].is_std
                && self.m.def_path(def) == "std.map.entries")
        {
            Some((IterKind::Map, source))
        } else if hir.try_lang("set.to_list") == Some(def) {
            Some((IterKind::Set, source))
        } else {
            None
        }
    }

    /// The page element representation of a `List<T>` iterated page by
    /// page (the runtime's representation of `T`, which pages store).
    /// `list[i]` without a runtime call: a bounds check against the length,
    /// then the page (directory slot `i / cap`) and the slot `i % cap`, with
    /// shifts and masks (page capacities are powers of two). Out of range,
    /// the runtime reports the defect as before.
    fn inline_list_index(
        &mut self,
        list_ty: &Ty,
        list: Operand,
        i: Operand,
        loc: u64,
        ty: &Ty,
    ) -> Option<Operand> {
        let repr = self.paged_elem(list_ty)?;
        let size = repr.size() as usize;
        let i_repr = match i {
            Operand::Local(x) => self.func.locals[x].repr,
            Operand::Const(Const::Int(_, r)) => r,
            _ => return None,
        };
        if size == 0 || i_repr != Repr::I64 {
            return None;
        }
        let cap = badger_runtime::list::page_cap_for(size);
        let u = NumTy::I(64, false);
        let int = |n: i64| Operand::Const(Const::Int(n, Repr::I64));
        let bin = |s: &mut Self, op: BinOp, a: Operand, b: Operand| {
            s.assign(Repr::I64, Rvalue::Bin { op, a, b, ty: u, loc: 0 })
        };
        let res = self.fresh(repr, None);
        let (fast, slow, join) = (self.block(), self.block(), self.block());
        let len = self.assign(
            Repr::I64,
            Rvalue::Load { base: list, offset: badger_runtime::list::LEN_OFFSET, repr: Repr::I64 },
        );
        let inside = self.assign(Repr::I8, Rvalue::Cmp { op: CmpOp::Lt, a: i, b: len, ty: u });
        self.term(Term::Branch(inside, fast, slow));
        self.switch(slow);
        let w = self.rt("badger_rt_list_get", vec![list, i, Operand::Const(Const::Addr(loc))], Repr::I64);
        let v = self.from_word(w, ty);
        self.emit(Stmt::Assign(res, Rvalue::Use(v)));
        self.term(Term::Goto(join));
        self.switch(fast);
        // the list (rooted as `list`) keeps its directory and pages alive
        let dir = self.assign(
            Repr::I64,
            Rvalue::Load { base: list, offset: badger_runtime::list::DIR_OFFSET, repr: Repr::I64 },
        );
        let page_index = bin(self, BinOp::Shr, i, int(cap.trailing_zeros() as i64));
        let slot_offset = bin(self, BinOp::Shl, page_index, int(3));
        let slot = bin(self, BinOp::Add, dir, slot_offset);
        let page = self.assign(
            Repr::I64,
            Rvalue::Load { base: slot, offset: badger_runtime::list::DIR_PAGES, repr: Repr::I64 },
        );
        let within = bin(self, BinOp::And, i, int(cap as i64 - 1));
        let elem_offset = bin(self, BinOp::Shl, within, int(size.trailing_zeros() as i64));
        let at = bin(self, BinOp::Add, page, elem_offset);
        let v =
            self.assign(repr, Rvalue::Load { base: at, offset: badger_runtime::list::PAGE_HDR as i32, repr });
        self.emit(Stmt::Assign(res, Rvalue::Use(v)));
        self.term(Term::Goto(join));
        self.switch(join);
        Some(Operand::Local(res))
    }

    /// The page element representation of `ListBuilder<T>`'s `T` (as
    /// [`Self::paged_elem`]), for elements with a size.
    fn builder_elem(&mut self, elem: &Ty) -> Option<Repr> {
        let r = self.m.lay.desc(elem).repr();
        (r == self.repr(elem) && r.size() > 0).then_some(r)
    }

    fn paged_elem(&mut self, list_ty: &Ty) -> Option<Repr> {
        if !self.m.lay.is_std(list_ty, "List") {
            return None;
        }
        let elem = self.m.lay.args(list_ty).first()?.clone();
        let r = self.m.lay.desc(&elem).repr();
        (r == self.repr(&elem)).then_some(r)
    }

    // ------------------------------------------------------------ patterns

    fn bind_irrefutable(&mut self, p: PatId, v: Operand, t: &Ty) {
        let fail = self.block();
        self.test_pat(p, v, t, fail);
        let cont = self.cur;
        self.switch(fail);
        self.term(Term::Unreachable);
        self.switch(cont);
    }

    /// Test `v : t` against pattern `p`; on mismatch jump to `fail`. On
    /// success, continues in the current block with bindings made.
    fn test_pat(&mut self, p: PatId, v: Operand, t: &Ty, fail: BlockId) {
        let pat = self.hir().pats[p].clone();
        match pat.kind {
            PatKind::Discard | PatKind::Error => {}
            PatKind::Bind(l) => {
                let name = self.m.sym(self.hir().locals[l].name).to_string();
                let x = self.copy_to_local(v, self.repr(t), Some(name));
                self.locals.insert(l, x);
            }
            PatKind::Lit(lit) => {
                let c = self.lit(&lit, t);
                let eq = self.compare(CmpOp::Eq, v, c, t);
                let ok = self.block();
                self.term(Term::Branch(eq, ok, fail));
                self.switch(ok);
            }
            PatKind::Tuple(ps) => {
                let layout = self.m.lay.record(t);
                for (sp, fl) in ps.iter().zip(layout.fields.clone()) {
                    let x = self.assign(fl.repr, Rvalue::Load { base: v, offset: fl.offset, repr: fl.repr });
                    self.test_pat(*sp, x, &fl.ty, fail);
                }
            }
            PatKind::Record { fields, .. } => {
                let layout = self.m.lay.record(t);
                for (l, sp) in fields {
                    if let Some(fl) = layout.field(l).cloned() {
                        let x =
                            self.assign(fl.repr, Rvalue::Load { base: v, offset: fl.offset, repr: fl.repr });
                        self.test_pat(sp, x, &fl.ty, fail);
                    }
                }
            }
            PatKind::Variant { name, args, .. } => {
                let vname = self.m.sym(name).to_string();
                let idx = self.variant_index(t, &vname);
                let (_, vs) = self.m.lay.enum_variants(t);
                if vs.len() > 1 {
                    let tag = self.assign(Repr::I32, Rvalue::Tag(v));
                    let ok = self.block();
                    self.term(Term::Switch(tag, vec![(idx as i64, ok)], fail));
                    self.switch(ok);
                }
                let fields = vs.get(idx as usize).map(|x| x.1.clone()).unwrap_or_default();
                for (sp, fl) in args.iter().zip(fields) {
                    let x = self.assign(fl.repr, Rvalue::Load { base: v, offset: fl.offset, repr: fl.repr });
                    self.test_pat(*sp, x, &fl.ty, fail);
                }
            }
        }
    }
}

/// Self tail calls become parameter reassignment and a jump back to the
/// function's first block (which re-binds parameter patterns and polls), so
/// self tail recursion runs in constant stack space.
fn tail_calls(f: &mut Func, me: FuncId) {
    // blocks that only return a local: `return res`
    let returns: Vec<Option<Local>> = f
        .blocks
        .iter()
        .map(|b| match (&b.term, b.stmts.is_empty()) {
            (Some(Term::Return(Operand::Local(l))), true) => Some(*l),
            _ => None,
        })
        .collect();
    let params = f.params.clone();
    let entry = BlockId::new(0);
    for bi in 0..f.blocks.len() {
        let b = &f.blocks[BlockId::new(bi)];
        let n = b.stmts.len();
        // the local finally returned from this block
        let (ret_local, call_index) = match &b.term {
            Some(Term::Return(Operand::Local(l))) if n >= 1 => (*l, n - 1),
            Some(Term::Goto(j)) if n >= 2 => match (returns[j.index()], &b.stmts[n - 1]) {
                (Some(r), Stmt::Assign(dst, Rvalue::Use(Operand::Local(src)))) if *dst == r => {
                    // `res = call_result; goto join` where join returns res
                    match &b.stmts[n - 2] {
                        Stmt::Assign(t, _) if t == src => (*src, n - 2),
                        _ => continue,
                    }
                }
                _ => continue,
            },
            _ => continue,
        };
        let Stmt::Assign(dst, Rvalue::Call { callee: Callee::Direct(target), args, .. }) =
            &b.stmts[call_index]
        else {
            continue;
        };
        if *target != me || *dst != ret_local || args.len() != params.len() {
            continue;
        }
        let args = args.clone();
        // evaluate all arguments into temporaries before reassigning
        let temps: Vec<Local> = params.iter().map(|p| f.locals.push(f.locals[*p].clone())).collect();
        let b = &mut f.blocks[BlockId::new(bi)];
        b.stmts.truncate(call_index);
        for (t, a) in temps.iter().zip(&args) {
            b.stmts.push(Stmt::Assign(*t, Rvalue::Use(*a)));
        }
        for (p, t) in params.iter().zip(&temps) {
            b.stmts.push(Stmt::Assign(*p, Rvalue::Use(Operand::Local(*t))));
        }
        b.term = Some(Term::Goto(entry));
    }
}

/// One-way match of a pattern type (with `Gen(i)`) against a ground type.
fn match_ty(p: &Ty, t: &Ty, binds: &mut Vec<Option<Ty>>) -> bool {
    match (&**p, &**t) {
        (TyKind::Gen(i), _) => {
            let i = *i as usize;
            if i >= binds.len() {
                binds.resize(i + 1, None);
            }
            match &binds[i] {
                Some(b) => b == t,
                None => {
                    binds[i] = Some(t.clone());
                    true
                }
            }
        }
        (TyKind::Con(a, xs), TyKind::Con(b, ys)) => {
            a == b && xs.len() == ys.len() && xs.iter().zip(ys).all(|(x, y)| match_ty(x, y, binds))
        }
        (TyKind::Tuple(xs), TyKind::Tuple(ys)) => {
            xs.len() == ys.len() && xs.iter().zip(ys).all(|(x, y)| match_ty(x, y, binds))
        }
        _ => p == t,
    }
}

/// Type nesting beyond which instances are not specialized (only
/// polymorphic recursion builds such types).
pub(crate) const SPECIALIZATION_DEPTH: usize = 24;

pub(crate) fn type_depth(t: &Ty) -> usize {
    match &**t {
        TyKind::Con(_, args) | TyKind::Tuple(args) | TyKind::Dyn(_, args) => {
            1 + args.iter().map(type_depth).max().unwrap_or(0)
        }
        TyKind::Fn(f) => {
            1 + f.params.iter().chain(std::iter::once(&f.ret)).map(type_depth).max().unwrap_or(0)
        }
        TyKind::Record(r) | TyKind::Ref(_, r) => 1 + type_depth(r),
        TyKind::RowExt(_, a, rest) => type_depth(a).max(type_depth(rest)),
        _ => 1,
    }
}

/// Whether generic parameter `i` occurs outside effect rows in `t`.
fn gen_in_value_position(t: &Ty, i: u32) -> bool {
    match &**t {
        TyKind::Gen(j) => *j == i,
        TyKind::Fn(f) => {
            f.params.iter().any(|p| gen_in_value_position(p, i)) || gen_in_value_position(&f.ret, i)
        }
        TyKind::Con(_, args) | TyKind::Tuple(args) | TyKind::Dyn(_, args) => {
            args.iter().any(|a| gen_in_value_position(a, i))
        }
        TyKind::Record(r) | TyKind::Ref(_, r) | TyKind::Forall(_, r) | TyKind::Proj(r, _) => {
            gen_in_value_position(r, i)
        }
        TyKind::RowExt(_, a, rest) => gen_in_value_position(a, i) || gen_in_value_position(rest, i),
        _ => false,
    }
}

/// Monomorphize the program reachable from `main` (and tests).
pub fn lower_program<'g, 'a>(
    g: &'g Global<'a>,
    sources: &'g SourceMap,
    tests: bool,
    entry_fn: &str,
) -> (Program, Vec<String>) {
    lower_program_optimized(g, sources, tests, entry_fn, false)
}

/// Lower with sequence fusion when optimized, non-live code is requested.
pub fn lower_program_optimized<'g, 'a>(
    g: &'g Global<'a>,
    sources: &'g SourceMap,
    tests: bool,
    entry_fn: &str,
    fuse_sequences: bool,
) -> (Program, Vec<String>) {
    let mut m = Mono::new(g, sources);
    m.fuse_sequences = fuse_sequences;
    let hir = g.hir;
    if let Some(entry) = hir.entry {
        let main = hir.modules[entry].items.get(&g.interner.intern(entry_fn)).copied();
        if let Some(main) = main.filter(|d| matches!(hir.defs[*d].kind, DefKind::Fn(_))) {
            let f = m.instance(main, vec![]);
            m.prog.entry = Some(f);
            if let Some(s) = g.scheme(main)
                && let TyKind::Fn(ft) = &*s.ty
            {
                m.prog.entry_ret = Some(m.lay.desc(&badger_typeck::ground_resolved(&ft.ret)));
                // an entry failing with an `Error` is reported with its code,
                // causes, and details
                let ret = badger_typeck::ground_resolved(&ft.ret);
                if m.lay.is_std(&ret, "Result")
                    && let Some(e) = m.lay.args(&ret).get(1).cloned()
                    && let Some(err_proto) = g.hir.try_lang("Error")
                    && m.has_impl_pub(err_proto, &e)
                    && let Some(f) = m.lang_fn("describe_error", vec![e])
                {
                    m.static_closure(f);
                    m.prog.error_report = Some(f);
                }
                // the tool passes the command-line arguments, or nothing
                match ft.params.as_slice() {
                    [] => {}
                    [p] if m.lay.is_std(p, "List")
                        && m.lay.args(p).first().is_some_and(|a| m.lay.is_std(a, "String")) =>
                    {
                        m.prog.entry_args = Some(m.lay.desc(&badger_typeck::ground_resolved(p)));
                    }
                    _ => {
                        m.prog.entry_error = Some(format!(
                            "`{entry_fn}` must take no parameters or one `List<String>` of arguments (found {})",
                            badger_typeck::show_ty(g, &s.ty)
                        ))
                    }
                }
            }
            m.static_closure(f);
        }
        if tests {
            // the tests of every module of the package (not of its
            // dependencies); outside the entry module they are named with
            // their module path
            for (mid, module) in hir.modules.iter() {
                if module.is_std {
                    continue;
                }
                for t in module.tests.clone() {
                    let f = m.instance(t, vec![]);
                    m.static_closure(f);
                    let short = g.interner.resolve(hir.defs[t].name).to_string();
                    let name = if mid == entry {
                        short
                    } else {
                        let path: Vec<&str> = module.path.iter().map(|s| g.interner.resolve(*s)).collect();
                        format!("{}.{short}", path.join("."))
                    };
                    if let Some(s) = g.scheme(t)
                        && let TyKind::Fn(ft) = &*s.ty
                    {
                        let ret = badger_typeck::ground_resolved(&ft.ret);
                        if m.lay.is_std(&ret, "Result")
                            && let Some(e) = m.lay.args(&ret).get(1).cloned()
                        {
                            m.prog.test_errors.push((name.clone(), m.lay.desc(&e)));
                        }
                    }
                    m.prog.tests.push((name, f));
                }
            }
        }
    }
    m.run();
    let mut errors = std::mem::take(&mut m.errors);
    m.prog.origins.descs = m.lay.created.borrow().clone();
    // the lowered program keeps the MIR invariants (`verify`)
    errors.extend(crate::verify::verify(&m.prog));
    (m.prog, errors)
}

/// A source path as shown at run time: relative to the working directory
/// when the file lies under it (`a/../b` segments resolved lexically).
fn display_path(name: &str) -> String {
    use std::path::{Component, Path, PathBuf};
    let p = Path::new(name);
    if !p.is_absolute() {
        return name.to_string();
    }
    let mut norm = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                norm.pop();
            }
            Component::CurDir => {}
            c => norm.push(c),
        }
    }
    match std::env::current_dir().ok().and_then(|d| norm.strip_prefix(d).ok().map(Path::to_path_buf)) {
        Some(rel) => rel.to_string_lossy().into_owned(),
        None => norm.to_string_lossy().into_owned(),
    }
}

/// Contract checks the static analysis proved (`badger_driver::contracts`):
/// precondition checks (by expression) and postconditions (by function and
/// clause index) that are compiled out.
#[derive(Default, Clone)]
pub struct Elided {
    pub checks: std::collections::HashSet<ExprId>,
    pub posts: std::collections::HashSet<(DefId, usize)>,
    /// `Int` additions, subtractions, and multiplications proven not to
    /// overflow (compiled without the overflow check).
    pub arith: std::collections::HashSet<ExprId>,
}

thread_local! {
    static ELIDED: std::cell::RefCell<Elided> = Default::default();
}

/// Set the proven contract checks for the next lowering on this thread.
pub fn set_elided(e: Elided) {
    ELIDED.with(|x| *x.borrow_mut() = e);
}

fn elided_check(e: ExprId) -> bool {
    ELIDED.with(|x| x.borrow().checks.contains(&e))
}

fn elided_arith(e: ExprId) -> bool {
    ELIDED.with(|x| x.borrow().arith.contains(&e))
}

fn elided_post(def: DefId, k: usize) -> bool {
    ELIDED.with(|x| x.borrow().posts.contains(&(def, k)))
}

/// What `badger contracts --tests` observes: the functions whose integer
/// arguments and results are recorded, and the contract clauses whose
/// outcomes are counted (precondition checks by expression, with the
/// condition that holds; postconditions by function and index).
#[derive(Default, Clone)]
pub struct Observe {
    pub fns: std::collections::HashSet<DefId>,
    pub pre: std::collections::HashMap<ExprId, (u32, ExprId)>,
    pub post: std::collections::HashMap<(DefId, usize), u32>,
}

/// The slot index of a function's result in observations.
pub const RESULT_SLOT: usize = 255;

/// The observation slot of parameter `k` (or the result) of `def`.
pub fn slot(def: DefId, k: usize) -> i64 {
    ((def.into_raw().into_u32() as i64) << 8) | (k.min(255) as i64)
}

thread_local! {
    static OBSERVE: std::cell::RefCell<Observe> = Default::default();
}

/// Set what the next lowering on this thread instruments.
pub fn set_observe(o: Observe) {
    OBSERVE.with(|x| *x.borrow_mut() = o);
}

fn observing(def: DefId) -> bool {
    OBSERVE.with(|x| x.borrow().fns.contains(&def))
}

fn observed_pre(e: ExprId) -> Option<(u32, ExprId)> {
    OBSERVE.with(|x| x.borrow().pre.get(&e).copied())
}

fn observed_post(def: DefId, k: usize) -> Option<u32> {
    OBSERVE.with(|x| x.borrow().post.get(&(def, k)).copied())
}
