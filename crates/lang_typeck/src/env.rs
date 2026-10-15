//! Type environment: declared types, protocols, impls, and conversion from
//! resolved HIR type expressions to [`Ty`].

use lang_base::{Diagnostic, Interner, Span, Symbol};
use lang_hir::{DefId, DefKind, GenericId, HEffect, HTy, HTyKind, Hir};
use rustc_hash::FxHashMap;

use crate::ty::*;

/// Definition ids of built-in and prelude types the checker relies on.
#[derive(Debug, Clone)]
pub struct Builtins {
    pub never: DefId,
    pub bool_: DefId,
    pub ints: Vec<DefId>,
    pub i64_: DefId,
    pub u8_: DefId,
    pub u16_: DefId,
    pub floats: Vec<DefId>,
    pub f64_: DefId,
    pub char_: DefId,
    pub symbol: DefId,
    pub string: DefId,
    pub bytes: DefId,
    pub duration: DefId,
    pub instant: DefId,
    pub list: DefId,
    pub map: DefId,
    pub set: DefId,
    pub range: DefId,
    pub option: DefId,
    pub result: DefId,
    pub data: DefId,
    pub display: DefId,
    pub error: DefId,
    pub from: DefId,
    pub eq: DefId,
    pub hash: DefId,
    pub codec: DefId,
    pub cell: DefId,
}

impl Builtins {
    pub fn new(hir: &Hir) -> Self {
        let l = |n: &str| hir.lang(n);
        Builtins {
            never: l("Never"),
            bool_: l("Bool"),
            ints: ["I8", "I16", "I32", "I64", "U8", "U16", "U32", "U64"].iter().map(|n| l(n)).collect(),
            i64_: l("I64"),
            u8_: l("U8"),
            u16_: l("U16"),
            floats: vec![l("F32"), l("F64")],
            f64_: l("F64"),
            char_: l("Char"),
            symbol: l("Symbol"),
            string: l("String"),
            bytes: l("Bytes"),
            duration: l("Duration"),
            instant: l("Instant"),
            list: l("List"),
            map: l("Map"),
            set: l("Set"),
            range: l("Range"),
            option: l("Option"),
            result: l("Result"),
            data: l("Data"),
            display: l("Display"),
            error: l("Error"),
            from: l("From"),
            eq: l("Eq"),
            hash: l("Hash"),
            codec: l("Codec"),
            cell: l("Cell"),
        }
    }

    pub fn is_int(&self, d: DefId) -> bool {
        self.ints.contains(&d)
    }

    pub fn is_float(&self, d: DefId) -> bool {
        self.floats.contains(&d)
    }

    pub fn is_numeric(&self, d: DefId) -> bool {
        self.is_int(d) || self.is_float(d)
    }

    pub fn con(&self, d: DefId, args: Vec<Ty>) -> Ty {
        mk(TyKind::Con(d, args))
    }

    pub fn prim(&self, d: DefId) -> Ty {
        mk(TyKind::Con(d, vec![]))
    }
}

#[derive(Debug, Clone)]
pub struct StructInfo {
    pub n_generics: u32,
    /// Field types use `Gen(i)` for the struct's generic parameters.
    pub fields: Vec<(Symbol, Ty, bool)>,
    pub module: lang_hir::ModuleId,
}

impl StructInfo {
    pub fn module_differs(&self, m: Option<lang_hir::ModuleId>) -> bool {
        Some(self.module) != m
    }
}

#[derive(Debug, Clone)]
pub struct EnumInfo {
    pub n_generics: u32,
    pub variants: Vec<VariantInfo>,
}

#[derive(Debug, Clone)]
pub struct VariantInfo {
    pub name: Symbol,
    pub fields: Vec<(Option<Symbol>, Ty)>,
}

#[derive(Debug, Clone)]
pub enum ImplMethods {
    /// User- or library-written methods.
    Defs(FxHashMap<Symbol, DefId>),
    /// Compiler-derived implementation (display via schema, From variant, ...).
    Derived(DerivedImpl),
}

#[derive(Debug, Clone)]
pub enum DerivedImpl {
    /// `derive(Display)` or `derive(Error)`: schema-driven display.
    Display,
    /// `derive(From)`: wrap the source value in the given variant.
    FromVariant { enum_def: DefId, variant: u32 },
    /// Marker protocols (Eq, Hash, Codec, Error).
    Marker,
    /// Reflexive `From<T> for T`.
    Identity,
    /// `derive(Error)`: the wrapped error of a single-payload variant.
    ErrorCause,
}

#[derive(Debug, Clone)]
pub struct ImplInfo {
    pub def: Option<DefId>,
    pub n_generics: u32,
    pub generic_ids: Vec<GenericId>,
    /// `None` for inherent impls.
    pub protocol: Option<(DefId, Vec<Ty>)>,
    pub self_ty: Ty,
    /// Where-clause obligations: protocol, protocol args, subject type.
    pub preds: Vec<(DefId, Vec<Ty>, Ty)>,
    pub methods: ImplMethods,
    pub span: Span,
    /// `type Name = T` bindings (generic parameters as `Gen(i)`).
    pub assoc: Vec<(Symbol, Ty)>,
}

#[derive(Debug, Clone)]
pub struct ProtocolInfo {
    pub n_generics: u32,
    pub methods: FxHashMap<Symbol, DefId>,
    pub supers: Vec<DefId>,
}

pub struct TypeEnv {
    pub b: Builtins,
    pub structs: FxHashMap<DefId, StructInfo>,
    pub enums: FxHashMap<DefId, EnumInfo>,
    pub protocols: FxHashMap<DefId, ProtocolInfo>,
    pub impls: Vec<ImplInfo>,
    /// Impl index by self type head constructor.
    pub impls_by_head: FxHashMap<DefId, Vec<usize>>,
    /// Impls whose self type is not a nominal constructor (records, tuples...).
    pub structural_impls: Vec<usize>,
    pub aliases: FxHashMap<DefId, (Vec<GenericId>, HTy)>,
    /// Every modeled effect: the bound of a function type stored without an
    /// effect annotation in user declarations (nothing can be inferred there).
    pub all_effects: Vec<Symbol>,
}

/// How generic parameters map to types while converting annotations.
pub struct Subst<'s> {
    pub generics: &'s FxHashMap<GenericId, Ty>,
    pub self_ty: Option<Ty>,
}

impl TypeEnv {
    pub fn build(
        hir: &Hir,
        interner: &Interner,
        icx: &mut InferCtxt,
        diags: &mut Vec<Diagnostic>,
    ) -> TypeEnv {
        let b = Builtins::new(hir);
        let mut env = TypeEnv {
            b,
            structs: FxHashMap::default(),
            enums: FxHashMap::default(),
            protocols: FxHashMap::default(),
            impls: vec![],
            impls_by_head: FxHashMap::default(),
            structural_impls: vec![],
            aliases: FxHashMap::default(),
            all_effects: ["Io", "Wait", "State", "Foreign"].iter().map(|l| interner.intern(l)).collect(),
        };
        for (id, def) in hir.defs.iter() {
            if let DefKind::Alias(a) = &def.kind {
                env.aliases.insert(id, (a.generics.clone(), a.ty.clone()));
            }
        }
        env.check_aliases(hir, interner, diags);
        for (id, def) in hir.defs.iter() {
            match &def.kind {
                DefKind::Struct(s) => {
                    let gens = gen_map(&s.generics);
                    let fields = s
                        .fields
                        .iter()
                        .map(|f| (f.name, env.lower_static(hir, icx, &f.ty, &gens, diags), f.is_pub))
                        .collect();
                    env.structs.insert(
                        id,
                        StructInfo { n_generics: s.generics.len() as u32, fields, module: def.module },
                    );
                }
                DefKind::Enum(e) => {
                    let gens = gen_map(&e.generics);
                    let variants = e
                        .variants
                        .iter()
                        .map(|v| VariantInfo {
                            name: v.name,
                            fields: v
                                .fields
                                .iter()
                                .map(|(l, t)| (*l, env.lower_static(hir, icx, t, &gens, diags)))
                                .collect(),
                        })
                        .collect();
                    env.enums.insert(id, EnumInfo { n_generics: e.generics.len() as u32, variants });
                }
                DefKind::Protocol(p) => {
                    let methods = p.methods.iter().map(|m| (hir.defs[*m].name, *m)).collect();
                    let supers = p
                        .supers
                        .iter()
                        .filter_map(|s| match &s.kind {
                            HTyKind::Named { def, .. } => Some(*def),
                            _ => None,
                        })
                        .collect();
                    env.protocols
                        .insert(id, ProtocolInfo { n_generics: p.generics.len() as u32, methods, supers });
                }
                _ => {}
            }
        }
        for (id, def) in hir.defs.iter() {
            if let DefKind::Impl(i) = &def.kind {
                let gens = gen_map(&i.generics);
                let self_ty = env.lower_static(hir, icx, &i.self_ty, &gens, diags);
                let protocol = i.protocol.as_ref().and_then(|p| match &p.kind {
                    HTyKind::Named { def, args } if env.protocols.contains_key(def) => {
                        let sub = Subst { generics: &gens, self_ty: Some(self_ty.clone()) };
                        let args = args.iter().map(|a| env.lower(hir, icx, a, &sub, false, diags)).collect();
                        Some((*def, args))
                    }
                    HTyKind::Error => None,
                    _ => {
                        diags.push(
                            Diagnostic::error("expected a protocol")
                                .with_code("E0201")
                                .with_label(p.span, "not a protocol"),
                        );
                        None
                    }
                });
                let mut preds = vec![];
                let sub = Subst { generics: &gens, self_ty: Some(self_ty.clone()) };
                for w in &i.where_preds {
                    let subject = env.lower(hir, icx, &w.ty, &sub, false, diags);
                    for bnd in &w.bounds {
                        if let Some((p, args)) = env.lower_bound(hir, icx, bnd, &sub, diags) {
                            preds.push((p, args, subject.clone()));
                        }
                    }
                }
                for (g, gid) in i.generics.iter().enumerate() {
                    for bnd in &hir.generics[*gid].bounds {
                        if let Some((p, args)) = env.lower_bound(hir, icx, bnd, &sub, diags) {
                            preds.push((p, args, mk(TyKind::Gen(g as u32))));
                        }
                    }
                }
                let methods = i.methods.iter().map(|m| (hir.defs[*m].name, *m)).collect();
                let assoc: Vec<(Symbol, Ty)> = i
                    .assoc_types
                    .iter()
                    .map(|(n, t)| (*n, env.lower_static_self(hir, icx, t, &gens, &self_ty, diags)))
                    .collect();
                // the implementation binds exactly the protocol's associated types
                if let Some((p, _)) = &protocol
                    && let DefKind::Protocol(pd) = &hir.defs[*p].kind
                {
                    let pname = interner.resolve(hir.defs[*p].name).to_string();
                    for want in &pd.assoc_types {
                        if !assoc.iter().any(|(n, _)| n == want) {
                            let w = interner.resolve(*want).to_string();
                            diags.push(
                                Diagnostic::error(format!("missing associated type `{w}`"))
                                    .with_code("E0214")
                                    .with_label(def.span, format!("`{pname}` requires `type {w} = ...`")),
                            );
                        }
                    }
                    for (have, _) in &assoc {
                        if !pd.assoc_types.contains(have) {
                            let h = interner.resolve(*have).to_string();
                            diags.push(
                                Diagnostic::error(format!("`{pname}` has no associated type `{h}`"))
                                    .with_code("E0214")
                                    .with_label(def.span, "not declared by the protocol"),
                            );
                        }
                    }
                }
                // an implementation provides every method without a default,
                // and only the protocol's methods
                if let Some((p, _)) = &protocol
                    && let DefKind::Protocol(pd) = &hir.defs[*p].kind
                {
                    let pname = interner.resolve(hir.defs[*p].name).to_string();
                    for &pm in &pd.methods {
                        let n = hir.defs[pm].name;
                        let has_default = matches!(&hir.defs[pm].kind, DefKind::Fn(f) if f.body.is_some());
                        if !has_default && !i.methods.iter().any(|m| hir.defs[*m].name == n) {
                            let ms = interner.resolve(n).to_string();
                            diags.push(
                                Diagnostic::error(format!("missing method `{ms}`"))
                                    .with_code("E0219")
                                    .with_label(def.span, format!("`{pname}` requires `{ms}`")),
                            );
                        }
                    }
                    for &m in &i.methods {
                        let n = hir.defs[m].name;
                        if !pd.methods.iter().any(|pm| hir.defs[*pm].name == n) {
                            let ms = interner.resolve(n).to_string();
                            diags.push(
                                Diagnostic::error(format!("`{pname}` has no method `{ms}`"))
                                    .with_code("E0219")
                                    .with_label(hir.defs[m].span, "not declared by the protocol"),
                            );
                        }
                    }
                }
                env.add_impl(ImplInfo {
                    def: Some(id),
                    n_generics: i.generics.len() as u32,
                    generic_ids: i.generics.clone(),
                    protocol,
                    self_ty,
                    preds,
                    methods: ImplMethods::Defs(methods),
                    span: def.span,
                    assoc,
                });
            }
        }
        // associated type bindings drive projection normalization
        let table: Vec<AssocImpl> = env
            .impls
            .iter()
            .flat_map(|imp| {
                imp.assoc.iter().map(|(name, ty)| AssocImpl {
                    n_generics: imp.n_generics,
                    self_ty: imp.self_ty.clone(),
                    name: *name,
                    ty: ty.clone(),
                })
            })
            .collect();
        set_assoc_table(table);
        env.derive_impls(hir, interner, diags);
        env.check_coherence(hir, interner, icx, diags);
        env.check_sizes(hir, interner, diags);
        env
    }

    /// Structs and enums store their fields inline (conceptually); a type
    /// that contains itself without indirection (`Box`, `Cell`, a collection,
    /// a function) has no finite size.
    fn check_sizes(&self, hir: &Hir, interner: &Interner, diags: &mut Vec<Diagnostic>) {
        let boxed = [hir.try_lang("Box"), Some(self.b.cell)];
        let fields_of = |d: DefId| -> Vec<Ty> {
            if let Some(s) = self.structs.get(&d) {
                s.fields.iter().map(|f| f.1.clone()).collect()
            } else if let Some(e) = self.enums.get(&d) {
                e.variants.iter().flat_map(|v| v.fields.iter().map(|f| f.1.clone())).collect()
            } else {
                vec![]
            }
        };
        let nominal = |d: DefId| {
            (self.structs.contains_key(&d) || self.enums.contains_key(&d)) && !boxed.contains(&Some(d))
        };
        // which generic parameters of each nominal type are stored inline
        let mut inline_params: FxHashMap<DefId, Vec<bool>> = FxHashMap::default();
        let ids: Vec<DefId> =
            self.structs.keys().chain(self.enums.keys()).copied().filter(|d| nominal(*d)).collect();
        fn walk(
            t: &Ty,
            nominal: &dyn Fn(DefId) -> bool,
            ip: &FxHashMap<DefId, Vec<bool>>,
            out: &mut Vec<DefId>,
            gens: &mut Vec<u32>,
        ) {
            match &**t {
                TyKind::Gen(i) => gens.push(*i),
                TyKind::Con(d, args) if nominal(*d) => {
                    out.push(*d);
                    let flags = ip.get(d);
                    for (i, a) in args.iter().enumerate() {
                        if flags.is_some_and(|f| f.get(i).copied().unwrap_or(false)) {
                            walk(a, nominal, ip, out, gens);
                        }
                    }
                }
                TyKind::Tuple(ts) => ts.iter().for_each(|x| walk(x, nominal, ip, out, gens)),
                TyKind::Ref(_, x) => walk(x, nominal, ip, out, gens),
                TyKind::Record(r) => walk(r, nominal, ip, out, gens),
                TyKind::RowExt(_, x, rest) => {
                    walk(x, nominal, ip, out, gens);
                    walk(rest, nominal, ip, out, gens);
                }
                _ => {}
            }
        }
        loop {
            let mut changed = false;
            for &d in &ids {
                let mut gens = vec![];
                for f in fields_of(d) {
                    walk(&f, &nominal, &inline_params, &mut vec![], &mut gens);
                }
                let n = self
                    .structs
                    .get(&d)
                    .map(|s| s.n_generics)
                    .or(self.enums.get(&d).map(|e| e.n_generics))
                    .unwrap_or(0);
                let flags: Vec<bool> = (0..n).map(|i| gens.contains(&i)).collect();
                if inline_params.get(&d) != Some(&flags) {
                    inline_params.insert(d, flags);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        let edges: FxHashMap<DefId, Vec<DefId>> = ids
            .iter()
            .map(|&d| {
                let mut out = vec![];
                for f in fields_of(d) {
                    walk(&f, &nominal, &inline_params, &mut out, &mut vec![]);
                }
                (d, out)
            })
            .collect();
        for &d in &ids {
            if hir.modules[hir.defs[d].module].is_std {
                continue;
            }
            // reachable from itself through inline edges?
            let mut stack = edges.get(&d).cloned().unwrap_or_default();
            let mut seen: Vec<DefId> = vec![];
            let mut cyclic = false;
            while let Some(x) = stack.pop() {
                if x == d {
                    cyclic = true;
                    break;
                }
                if !seen.contains(&x) {
                    seen.push(x);
                    stack.extend(edges.get(&x).cloned().unwrap_or_default());
                }
            }
            if cyclic {
                let name = interner.resolve(hir.defs[d].name).to_string();
                diags.push(
                    Diagnostic::error(format!("`{name}` has infinite size"))
                        .with_code("E0213")
                        .with_label(hir.defs[d].span, "this type contains itself without indirection")
                        .with_note(
                            "store the recursive part in a `Box<T>`, `Option<Box<T>>`, or a collection",
                        ),
                );
            }
        }
    }

    /// Aliases name existing types: a hole is not a type, and an alias that
    /// expands to itself would be infinitely sized (structural types have no
    /// nominal indirection).
    fn check_aliases(&mut self, hir: &Hir, interner: &Interner, diags: &mut Vec<Diagnostic>) {
        fn refs(t: &HTy, out: &mut Vec<(DefId, Span)>, holes: &mut Vec<Span>) {
            match &t.kind {
                HTyKind::Hole => holes.push(t.span),
                HTyKind::Named { def, args } => {
                    out.push((*def, t.span));
                    args.iter().for_each(|a| refs(a, out, holes));
                }
                HTyKind::Dyn { args, .. } => args.iter().for_each(|a| refs(a, out, holes)),
                HTyKind::Tuple(ts) => ts.iter().for_each(|a| refs(a, out, holes)),
                HTyKind::Record { fields, .. } => fields.iter().for_each(|(_, a)| refs(a, out, holes)),
                HTyKind::Fn { params, ret, .. } => {
                    params.iter().for_each(|a| refs(a, out, holes));
                    refs(ret, out, holes);
                }
                HTyKind::Forall { body, .. } => refs(body, out, holes),
                HTyKind::Ref { inner, .. } => refs(inner, out, holes),
                HTyKind::Assoc { base, .. } => refs(base, out, holes),
                _ => {}
            }
        }
        let ids: Vec<DefId> = self.aliases.keys().copied().collect();
        let mut bad = vec![];
        for &a in &ids {
            let (_, body) = &self.aliases[&a];
            let (mut out, mut holes) = (vec![], vec![]);
            refs(body, &mut out, &mut holes);
            for h in holes {
                diags.push(Diagnostic::error("`_` is not a type name").with_code("E0205").with_label(
                    h,
                    "a type alias must name a type; `_` is only an inference hole in private annotations",
                ));
                bad.push(a);
            }
            // does the alias reach itself through other aliases?
            let mut stack: Vec<DefId> =
                out.iter().map(|x| x.0).filter(|d| self.aliases.contains_key(d)).collect();
            let mut seen = vec![];
            while let Some(d) = stack.pop() {
                if d == a {
                    let name = interner.resolve(hir.defs[a].name).to_string();
                    diags.push(
                        Diagnostic::error(format!("type alias `{name}` refers to itself"))
                            .with_code("E0213")
                            .with_label(
                                hir.defs[a].span,
                                "an alias is expanded in place, so this type would be infinitely sized",
                            )
                            .with_note(
                                "use a `struct` or `enum` with `Box<T>` or a collection for recursive data",
                            ),
                    );
                    bad.push(a);
                    break;
                }
                if seen.contains(&d) {
                    continue;
                }
                seen.push(d);
                let (mut o, mut h) = (vec![], vec![]);
                refs(&self.aliases[&d].1, &mut o, &mut h);
                stack.extend(o.into_iter().map(|x| x.0).filter(|d| self.aliases.contains_key(d)));
            }
        }
        for a in bad {
            if let Some(e) = self.aliases.get_mut(&a) {
                e.1 = HTy { kind: HTyKind::Error, span: e.1.span };
            }
        }
    }

    /// One implementation per protocol/type combination, owned by the package
    /// defining the protocol or the outer nominal type.
    fn check_coherence(
        &self,
        hir: &Hir,
        interner: &Interner,
        icx: &mut InferCtxt,
        diags: &mut Vec<Diagnostic>,
    ) {
        let is_std = |d: DefId| hir.modules[hir.defs[d].module].is_std;
        let pname = |p: DefId| interner.resolve(hir.defs[p].name).to_string();
        for imp in &self.impls {
            let (Some(def), Some((p, _))) = (imp.def, &imp.protocol) else { continue };
            // memory capabilities are verified by the compiler, never asserted
            if !is_std(def)
                && is_std(*p)
                && matches!(
                    pname(*p).as_str(),
                    "Transfer" | "Share" | "Integer" | "Row" | "Scope" | "Effect" | "Unscoped"
                )
            {
                diags.push(
                    Diagnostic::error(format!("`{}` cannot be implemented", pname(*p)))
                        .with_code("E0217")
                        .with_label(
                            imp.span,
                            "this capability is derived by the compiler from the type's representation",
                        ),
                );
                continue;
            }
            if is_std(def) || !is_std(*p) {
                continue;
            }
            if matches!(&*imp.self_ty, TyKind::Record(_) | TyKind::Tuple(_) | TyKind::Fn(_)) {
                diags.push(
                    Diagnostic::error(format!("`{}` cannot be implemented for a structural type", pname(*p)))
                        .with_code("E0218")
                        .with_label(imp.span, "records, tuples, and function types (and aliases of them) are not implementation identities")
                        .with_note("declare a `struct` for the shape and implement the protocol for it"),
                );
                continue;
            }
            let local_head = match &*imp.self_ty {
                TyKind::Con(d, _) => !is_std(*d),
                TyKind::Dyn(dp, _) => !is_std(*dp),
                _ => false,
            };
            if !local_head {
                diags.push(
                    Diagnostic::error(format!("orphan implementation of `{}`", pname(*p)))
                        .with_code("E0211")
                        .with_label(imp.span, "neither the protocol nor the implementing type belongs to this package")
                        .with_note("wrap the type in a local nominal struct and implement the protocol for the wrapper"),
                );
            }
        }
        for imp in &self.impls {
            let (Some(def), Some((p, _))) = (imp.def, &imp.protocol) else { continue };
            if is_std(def) || is_std(*p) {
                continue;
            }
            if matches!(&*imp.self_ty, TyKind::Record(_) | TyKind::Tuple(_) | TyKind::Fn(_)) {
                diags.push(
                    Diagnostic::error(format!("`{}` cannot be implemented for a structural type", pname(*p)))
                        .with_code("E0218")
                        .with_label(imp.span, "records, tuples, and function types (and aliases of them) are not implementation identities")
                        .with_note("declare a `struct` for the shape and implement the protocol for it"),
                );
            }
        }
        let mut by_proto: FxHashMap<DefId, Vec<usize>> = FxHashMap::default();
        for (i, imp) in self.impls.iter().enumerate() {
            if let Some((p, _)) = &imp.protocol {
                by_proto.entry(*p).or_default().push(i);
            }
        }
        for (p, idxs) in by_proto {
            for (k, &i) in idxs.iter().enumerate() {
                for &j in &idxs[k + 1..] {
                    let (a, b) = (&self.impls[i], &self.impls[j]);
                    // reported for user code only; std impls are trusted
                    let user = |x: &ImplInfo| x.def.is_some_and(|d| !is_std(d));
                    if !user(a) && !user(b) {
                        continue;
                    }
                    let snap = icx.snapshot();
                    let va: Vec<Ty> = (0..a.n_generics).map(|_| icx.fresh()).collect();
                    let vb: Vec<Ty> = (0..b.n_generics).map(|_| icx.fresh()).collect();
                    let (pa, pb) = (&a.protocol.as_ref().unwrap().1, &b.protocol.as_ref().unwrap().1);
                    let mut overlap = pa.len() == pb.len()
                        && icx.try_unify(&subst_gen(&a.self_ty, &va), &subst_gen(&b.self_ty, &vb));
                    for (x, y) in pa.iter().zip(pb) {
                        overlap = overlap && icx.try_unify(&subst_gen(x, &va), &subst_gen(y, &vb));
                    }
                    icx.rollback(snap);
                    if overlap {
                        let (first, second) = if user(b) { (a, b) } else { (b, a) };
                        diags.push(
                            Diagnostic::error(format!("overlapping implementations of `{}`", pname(p)))
                                .with_code("E0212")
                                .with_label(second.span, "this implementation overlaps")
                                .with_label(first.span, "with this one"),
                        );
                    }
                }
            }
        }
    }

    pub fn add_impl(&mut self, info: ImplInfo) {
        let idx = self.impls.len();
        match &*info.self_ty {
            TyKind::Con(d, _) => self.impls_by_head.entry(*d).or_default().push(idx),
            _ => self.structural_impls.push(idx),
        }
        self.impls.push(info);
    }

    /// The first field (and why) that prevents deriving `proto` for `id`.
    /// Generic parameters are checked where the type is used.
    fn underivable(
        &self,
        hir: &Hir,
        interner: &Interner,
        id: DefId,
        proto: &str,
    ) -> Option<(String, String)> {
        let fields: Vec<(String, Ty)> = if let Some(si) = self.structs.get(&id) {
            si.fields.iter().map(|f| (interner.resolve(f.0).to_string(), f.1.clone())).collect()
        } else {
            let ei = self.enums.get(&id)?;
            ei.variants
                .iter()
                .flat_map(|v| {
                    let vn = interner.resolve(v.name).to_string();
                    v.fields.iter().enumerate().map(move |(i, f)| (format!("{vn}.{i}"), f.1.clone()))
                })
                .collect()
        };
        fn bad(
            env: &TypeEnv,
            hir: &Hir,
            interner: &Interner,
            t: &Ty,
            proto: &str,
            seen: &mut Vec<DefId>,
        ) -> Option<String> {
            match &**t {
                TyKind::Fn(_) if proto != "From" && proto != "Error" && proto != "Display" => {
                    Some("is a function".into())
                }
                TyKind::Fn(_) if proto == "Display" || proto == "Error" => {
                    Some("is a function, which has no `Display`".into())
                }
                TyKind::Tuple(ts) => ts.iter().find_map(|x| bad(env, hir, interner, x, proto, seen)),
                TyKind::Record(r) => {
                    row_fields(r).0.iter().find_map(|(_, x)| bad(env, hir, interner, x, proto, seen))
                }
                TyKind::Con(d, args) => {
                    let name = interner.resolve(hir.defs[*d].name);
                    let std = hir.modules[hir.defs[*d].module].is_std;
                    if proto == "Hash" && matches!(name, "F32" | "F64") && std {
                        return Some("is a float, which has no total `Hash`".into());
                    }
                    if proto == "Eq" && matches!(name, "F32" | "F64") && std {
                        return Some("is a float, which has no total `Eq`".into());
                    }
                    if std && name == "Cell" && matches!(proto, "Eq" | "Hash" | "Codec") {
                        return Some("is a `Cell`".into());
                    }
                    if std && proto == "Codec" && !env.structs.contains_key(d) && !env.enums.contains_key(d) {
                        let data = [
                            "Bool",
                            "I8",
                            "I16",
                            "I32",
                            "I64",
                            "U8",
                            "U16",
                            "U32",
                            "U64",
                            "F32",
                            "F64",
                            "Char",
                            "Symbol",
                            "String",
                            "Bytes",
                            "Duration",
                            "Instant",
                            "Timestamp",
                            "List",
                            "Map",
                            "Set",
                            "Array",
                            "Data",
                        ];
                        if !data.contains(&name) {
                            return Some(format!("is a `{name}`, which has no data encoding"));
                        }
                    }
                    if let Some(x) = args.iter().find_map(|x| bad(env, hir, interner, x, proto, seen)) {
                        return Some(x);
                    }
                    if !std && !seen.contains(d) {
                        seen.push(*d);
                        let inner: Vec<Ty> = if let Some(si) = env.structs.get(d) {
                            si.fields.iter().map(|f| f.1.clone()).collect()
                        } else if let Some(ei) = env.enums.get(d) {
                            ei.variants.iter().flat_map(|v| v.fields.iter().map(|f| f.1.clone())).collect()
                        } else {
                            vec![]
                        };
                        return inner.iter().find_map(|x| bad(env, hir, interner, x, proto, seen));
                    }
                    None
                }
                _ => None,
            }
        }
        if !matches!(proto, "Eq" | "Hash" | "Codec" | "Display" | "Error") {
            return None;
        }
        for (f, t) in fields {
            let mut seen = vec![id];
            if let Some(why) = bad(self, hir, interner, &t, proto, &mut seen) {
                return Some((f, why));
            }
        }
        None
    }

    fn derive_impls(&mut self, hir: &Hir, interner: &Interner, diags: &mut Vec<Diagnostic>) {
        for (id, def) in hir.defs.iter() {
            let (derives, n) = match &def.kind {
                DefKind::Struct(s) => (&s.derives, s.generics.len()),
                DefKind::Enum(e) => (&e.derives, e.generics.len()),
                _ => continue,
            };
            let self_ty = mk(TyKind::Con(id, (0..n as u32).map(|i| mk(TyKind::Gen(i))).collect()));
            for d in derives {
                let pname = interner.resolve(hir.defs[d.protocol].name);
                // a derivation needs every field to support it
                if let Some((field, why)) = self.underivable(hir, interner, id, pname) {
                    let tn = interner.resolve(def.name).to_string();
                    diags.push(
                        Diagnostic::error(format!("cannot derive `{pname}` for `{tn}`"))
                            .with_code("E0204")
                            .with_label(d.span, format!("field `{field}` {why}")),
                    );
                    continue;
                }
                let base = |methods: ImplMethods, protocol: (DefId, Vec<Ty>)| ImplInfo {
                    def: None,
                    n_generics: n as u32,
                    generic_ids: vec![],
                    protocol: Some(protocol),
                    self_ty: self_ty.clone(),
                    preds: vec![],
                    methods,
                    span: d.span,
                    assoc: vec![],
                };
                match pname {
                    "Error" => {
                        self.add_impl(base(
                            ImplMethods::Derived(DerivedImpl::Display),
                            (self.b.display, vec![]),
                        ));
                        self.add_impl(base(
                            ImplMethods::Derived(DerivedImpl::Marker),
                            (self.b.error, vec![]),
                        ));
                    }
                    "Display" => {
                        self.add_impl(base(
                            ImplMethods::Derived(DerivedImpl::Display),
                            (self.b.display, vec![]),
                        ));
                    }
                    "Eq" | "Hash" | "Codec" => {
                        self.add_impl(base(ImplMethods::Derived(DerivedImpl::Marker), (d.protocol, vec![])));
                    }
                    "From" => {
                        let Some(info) = self.enums.get(&id).cloned() else {
                            diags.push(
                                Diagnostic::error("`derive(From)` requires an enum")
                                    .with_code("E0202")
                                    .with_label(d.span, "not an enum"),
                            );
                            continue;
                        };
                        // eligible variants: exactly one payload field
                        let eligible: Vec<(u32, Ty)> = info
                            .variants
                            .iter()
                            .enumerate()
                            .filter(|(_, v)| v.fields.len() == 1)
                            .map(|(i, v)| (i as u32, v.fields[0].1.clone()))
                            .collect();
                        for (i, (vi, src)) in eligible.iter().enumerate() {
                            if let Some((vj, _)) = eligible[..i].iter().find(|(_, s)| s == src) {
                                let names = &info.variants;
                                let ename = interner.resolve(def.name);
                                let a = interner.resolve(names[*vj as usize].name);
                                let b = interner.resolve(names[*vi as usize].name);
                                diags.push(
                                    Diagnostic::error(format!(
                                        "ambiguous `derive(From)` for `{ename}`: variants `{a}` and `{b}` accept the same source type"
                                    ))
                                    .with_code("E0203")
                                    .with_label(d.span, "derived here")
                                    .with_note("duplicate source types require an explicit conversion"),
                                );
                                continue;
                            }
                            if eligible[i + 1..].iter().any(|(_, s)| s == src) {
                                continue;
                            }
                            self.add_impl(base(
                                ImplMethods::Derived(DerivedImpl::FromVariant { enum_def: id, variant: *vi }),
                                (self.b.from, vec![src.clone()]),
                            ));
                        }
                    }
                    other => diags.push(
                        Diagnostic::error(format!("`{other}` cannot be derived"))
                            .with_code("E0204")
                            .with_label(d.span, "supported: Eq, Hash, Display, Codec, From, Error"),
                    ),
                }
            }
        }
    }

    /// Lower a type in a declaration context: generics map to `Gen(i)`.
    fn lower_static(
        &self,
        hir: &Hir,
        icx: &mut InferCtxt,
        t: &HTy,
        gens: &FxHashMap<GenericId, Ty>,
        diags: &mut Vec<Diagnostic>,
    ) -> Ty {
        self.lower(hir, icx, t, &Subst { generics: gens, self_ty: None }, false, diags)
    }

    fn lower_static_self(
        &self,
        hir: &Hir,
        icx: &mut InferCtxt,
        t: &HTy,
        gens: &FxHashMap<GenericId, Ty>,
        self_ty: &Ty,
        diags: &mut Vec<Diagnostic>,
    ) -> Ty {
        self.lower(hir, icx, t, &Subst { generics: gens, self_ty: Some(self_ty.clone()) }, false, diags)
    }

    pub fn lower_bound(
        &self,
        hir: &Hir,
        icx: &mut InferCtxt,
        bnd: &HTy,
        sub: &Subst<'_>,
        diags: &mut Vec<Diagnostic>,
    ) -> Option<(DefId, Vec<Ty>)> {
        match &bnd.kind {
            // kind markers constrain nothing at runtime (`Row` is checked as a
            // kind: the argument must be a row)
            HTyKind::Named { def, .. }
                if hir.lang.get("Scope") == Some(def) || hir.lang.get("Effect") == Some(def) =>
            {
                None
            }
            HTyKind::Named { def, args } if matches!(hir.defs[*def].kind, DefKind::Protocol(_)) => {
                Some((*def, args.iter().map(|a| self.lower(hir, icx, a, sub, true, diags)).collect()))
            }
            HTyKind::Error => None,
            _ => {
                diags.push(
                    Diagnostic::error("expected a protocol bound")
                        .with_code("E0201")
                        .with_label(bnd.span, "not a protocol"),
                );
                None
            }
        }
    }

    /// Convert a resolved type expression. `holes` permits `_` (fresh variable).
    pub fn lower(
        &self,
        hir: &Hir,
        icx: &mut InferCtxt,
        t: &HTy,
        sub: &Subst<'_>,
        holes: bool,
        diags: &mut Vec<Diagnostic>,
    ) -> Ty {
        match &t.kind {
            HTyKind::Hole => {
                if !holes {
                    diags.push(
                        Diagnostic::error("`_` is not allowed here")
                            .with_code("E0205")
                            .with_label(t.span, "inference hole"),
                    );
                }
                icx.fresh()
            }
            HTyKind::Unit => unit(),
            HTyKind::Error => error_ty(),
            HTyKind::Generic(g) => match sub.generics.get(g) {
                Some(ty) => ty.clone(),
                None => error_ty(),
            },
            HTyKind::SelfTy => sub.self_ty.clone().unwrap_or_else(error_ty),
            HTyKind::Assoc { base, name } => {
                mk(TyKind::Proj(self.lower(hir, icx, base, sub, holes, diags), *name))
            }
            HTyKind::Tuple(ts) => {
                mk(TyKind::Tuple(ts.iter().map(|x| self.lower(hir, icx, x, sub, holes, diags)).collect()))
            }
            HTyKind::Record { fields, rest } => {
                let fs: Vec<(Symbol, Ty)> =
                    fields.iter().map(|(l, x)| (*l, self.lower(hir, icx, x, sub, holes, diags))).collect();
                let tail = match rest {
                    // a record-typed argument supplies its row
                    Some(g) => match sub.generics.get(g).cloned() {
                        Some(t) => match &*t {
                            TyKind::Record(r) => r.clone(),
                            _ => t,
                        },
                        None => error_ty(),
                    },
                    None => row_empty(),
                };
                record(fs, tail)
            }
            HTyKind::Fn { once, params, ret, effect } => {
                let params = params.iter().map(|x| self.lower(hir, icx, x, sub, holes, diags)).collect();
                let ret = self.lower(hir, icx, ret, sub, holes, diags);
                let eff = match effect {
                    Some(e) => self.lower_effect(hir, icx, e, sub, diags),
                    // in signatures an omitted effect is inferred (generalized
                    // in parameter positions)
                    None if holes => icx.fresh_row(),
                    // in user declarations (struct fields, payloads, aliases)
                    // a stored function may perform any effect; the standard
                    // library's own plumbing accepts any effect untracked
                    None if !hir.modules.iter().any(|(_, m)| m.is_std && m.file == t.span.file) => {
                        self.any_effect()
                    }
                    None => error_ty(),
                };
                mk(TyKind::Fn(FnTy { params, ret, eff, once: *once }))
            }
            HTyKind::Forall { params, body } => {
                let mut gens = sub.generics.clone();
                for (i, p) in params.iter().enumerate() {
                    gens.insert(*p, mk(TyKind::Bound(i as u32)));
                }
                let inner = Subst { generics: &gens, self_ty: sub.self_ty.clone() };
                let body = self.lower(hir, icx, body, &inner, holes, diags);
                mk(TyKind::Forall(params.len() as u32, body))
            }
            HTyKind::Ref { mutable, inner } => {
                mk(TyKind::Ref(*mutable, self.lower(hir, icx, inner, sub, holes, diags)))
            }
            HTyKind::Dyn { protocol, args } => {
                if let DefKind::Protocol(p) = &hir.defs[*protocol].kind {
                    if !p.assoc_types.is_empty() {
                        diags.push(
                            Diagnostic::error("a protocol with associated types cannot be used as `dyn`")
                                .with_code("E0215")
                                .with_label(t.span, "its method types depend on the implementing type")
                                .with_note("use a generic parameter bounded by the protocol instead"),
                        );
                        return error_ty();
                    }
                    if let Some(why) = object_unsafe(hir, p) {
                        diags.push(
                            Diagnostic::error("this protocol cannot be used as `dyn`")
                                .with_code("E0216")
                                .with_label(t.span, why)
                                .with_note("a `dyn` value calls methods through a dictionary: every method takes `self`, has no type parameters, and mentions `Self` only as its receiver"),
                        );
                        return error_ty();
                    }
                }
                let args = args.iter().map(|x| self.lower(hir, icx, x, sub, holes, diags)).collect();
                mk(TyKind::Dyn(*protocol, args))
            }
            HTyKind::EffectLabel(_) => {
                diags.push(
                    Diagnostic::error("effect names are not types")
                        .with_code("E0207")
                        .with_label(t.span, "use in `!{...}`"),
                );
                error_ty()
            }
            HTyKind::Named { def, args } => {
                let args: Vec<Ty> = args.iter().map(|x| self.lower(hir, icx, x, sub, holes, diags)).collect();
                let d = &hir.defs[*def];
                let expected = match &d.kind {
                    DefKind::Alias(a) => {
                        if a.generics.len() != args.len() {
                            arity_err(diags, t.span, a.generics.len(), args.len());
                            return error_ty();
                        }
                        let (gs, body) = self.aliases[def].clone();
                        let mut m = FxHashMap::default();
                        for (g, a) in gs.iter().zip(args) {
                            m.insert(*g, a);
                        }
                        return self.lower(
                            hir,
                            icx,
                            &body,
                            &Subst { generics: &m, self_ty: sub.self_ty.clone() },
                            holes,
                            diags,
                        );
                    }
                    DefKind::Struct(s) => s.generics.len(),
                    DefKind::Enum(e) => e.generics.len(),
                    DefKind::Opaque(o) => o.generics.len(),
                    DefKind::Protocol(_) => {
                        diags.push(
                            Diagnostic::error("a protocol is not a type")
                                .with_code("E0208")
                                .with_label(t.span, "use a generic parameter with a bound, or `dyn`"),
                        );
                        return error_ty();
                    }
                    _ => {
                        diags.push(
                            Diagnostic::error("not a type")
                                .with_code("E0209")
                                .with_label(t.span, "expected a type"),
                        );
                        return error_ty();
                    }
                };
                let mut args = args;
                // an omitted trailing region parameter (`S: Scope`) is elided:
                // it is inferred where inference holes are allowed
                if holes && args.len() + 1 == expected && self.last_param_is_region(hir, *def) {
                    args.push(icx.fresh());
                }
                if expected != args.len() {
                    arity_err(diags, t.span, expected, args.len());
                    return error_ty();
                }
                mk(TyKind::Con(*def, args))
            }
        }
    }

    /// The closed row of every modeled effect.
    pub fn any_effect(&self) -> Ty {
        row(self.all_effects.iter().map(|l| (*l, unit())), row_empty())
    }

    /// Whether a type constructor's last generic parameter is a region
    /// (bounded by `Scope`).
    pub fn last_param_is_region(&self, hir: &Hir, def: DefId) -> bool {
        let n = match &hir.defs[def].kind {
            DefKind::Struct(s) => s.generics.len(),
            DefKind::Enum(e) => e.generics.len(),
            DefKind::Opaque(o) => o.generics.len(),
            _ => return false,
        };
        n > 0 && self.param_is_region(hir, def, n - 1)
    }

    /// Whether generic parameter `i` of a type constructor is a region
    /// (bounded by `Scope`).
    pub fn param_is_region(&self, hir: &Hir, def: DefId, i: usize) -> bool {
        let gens = match &hir.defs[def].kind {
            DefKind::Struct(s) => &s.generics,
            DefKind::Enum(e) => &e.generics,
            DefKind::Opaque(o) => &o.generics,
            _ => return false,
        };
        let Some(g) = gens.get(i) else { return false };
        hir.generics[*g].bounds.iter().any(|b| match &b.kind {
            HTyKind::Named { def, .. } => {
                let d = &hir.defs[*def];
                hir.modules[d.module].is_std && hir.lang.get("Scope") == Some(def)
            }
            _ => false,
        })
    }

    pub fn lower_effect(
        &self,
        hir: &Hir,
        icx: &mut InferCtxt,
        e: &HEffect,
        sub: &Subst<'_>,
        diags: &mut Vec<Diagnostic>,
    ) -> Ty {
        let _ = (hir, icx, diags);
        match e {
            HEffect::Set { labels, .. } => row(labels.iter().map(|l| (*l, unit())), row_empty()),
            HEffect::Var(g) => sub.generics.get(g).cloned().unwrap_or_else(row_empty),
        }
    }

    /// Instantiate an enum's variant field types for concrete arguments.
    pub fn variant_fields(&self, enum_def: DefId, variant: u32, args: &[Ty]) -> Vec<(Option<Symbol>, Ty)> {
        let info = &self.enums[&enum_def];
        info.variants[variant as usize].fields.iter().map(|(l, t)| (*l, subst_gen(t, args))).collect()
    }

    pub fn struct_fields(&self, def: DefId, args: &[Ty]) -> Vec<(Symbol, Ty, bool)> {
        self.structs[&def].fields.iter().map(|(l, t, p)| (*l, subst_gen(t, args), *p)).collect()
    }
}

fn arity_err(diags: &mut Vec<Diagnostic>, span: Span, expected: usize, got: usize) {
    diags.push(
        Diagnostic::error(format!("wrong number of type arguments: expected {expected}, found {got}"))
            .with_code("E0210")
            .with_label(span, "here"),
    );
}

pub fn gen_map(gs: &[GenericId]) -> FxHashMap<GenericId, Ty> {
    gs.iter().enumerate().map(|(i, g)| (*g, mk(TyKind::Gen(i as u32)))).collect()
}

/// Replace `Gen(i)` with `args[i]`.
pub fn subst_gen(t: &Ty, args: &[Ty]) -> Ty {
    ty_map(t, &mut |x| match &**x {
        TyKind::Gen(i) => Some(args.get(*i as usize).cloned().unwrap_or_else(error_ty)),
        _ => None,
    })
}

/// Replace `Bound(i)` with `args[i]` (opening a `Forall`).
pub fn subst_bound(t: &Ty, args: &[Ty]) -> Ty {
    ty_map(t, &mut |x| match &**x {
        TyKind::Bound(i) => Some(args[*i as usize].clone()),
        _ => None,
    })
}

/// Why a protocol has no dictionary calling convention (object safety).
fn object_unsafe(hir: &Hir, p: &lang_hir::ProtocolDef) -> Option<String> {
    fn mentions_self(t: &HTy) -> bool {
        match &t.kind {
            HTyKind::SelfTy => true,
            HTyKind::Named { args, .. } | HTyKind::Dyn { args, .. } => args.iter().any(mentions_self),
            HTyKind::Tuple(ts) => ts.iter().any(mentions_self),
            HTyKind::Record { fields, .. } => fields.iter().any(|(_, t)| mentions_self(t)),
            HTyKind::Fn { params, ret, .. } => params.iter().any(mentions_self) || mentions_self(ret),
            HTyKind::Forall { body, .. } => mentions_self(body),
            HTyKind::Ref { inner, .. } => mentions_self(inner),
            HTyKind::Assoc { base, .. } => mentions_self(base),
            _ => false,
        }
    }
    for &m in &p.methods {
        let DefKind::Fn(f) = &hir.defs[m].kind else { continue };
        let name = &hir.defs[m].name;
        let _ = name;
        if !f.has_self {
            return Some("a method without `self` cannot be called through `dyn`".into());
        }
        if !f.generics.is_empty() {
            return Some("a generic method cannot be called through `dyn`".into());
        }
        if f.params.iter().skip(1).filter_map(|p| p.ty.as_ref()).any(mentions_self)
            || f.ret.as_ref().is_some_and(mentions_self)
        {
            return Some("a method mentions `Self` beyond its receiver".into());
        }
    }
    None
}
