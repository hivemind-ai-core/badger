//! Concrete layouts and runtime descriptors for ground types.
//!
//! Records order fields by canonical label text, align each field to its
//! size, and round the payload to 8 bytes. Transparent structs use the same
//! rule (so a record coerces to a struct without reconstruction). Tuples are
//! positional. Enums store a `u32` tag at offset 0 and variant fields from
//! offset 8. Closures store the code pointer, adapter pointer, then captures.

use std::cell::RefCell;

use badger_base::Symbol;
use badger_hir::DefId;
use badger_runtime::Repr;
use badger_runtime::desc::{Field, Kind, TypeDesc, Variant, builtin};
use badger_typeck::{Global, Ty, TyKind};
use rustc_hash::FxHashMap;

#[derive(Debug, Clone)]
pub struct FieldLayout {
    pub label: Symbol,
    pub name: String,
    pub offset: i32,
    pub repr: Repr,
    pub ty: Ty,
}

#[derive(Debug, Clone)]
pub struct RecLayout {
    pub size: u32,
    pub fields: Vec<FieldLayout>,
}

impl RecLayout {
    pub fn field(&self, l: Symbol) -> Option<&FieldLayout> {
        self.fields.iter().find(|f| f.label == l)
    }
}

pub struct Layouts<'g, 'a> {
    pub g: &'g Global<'a>,
    descs: RefCell<FxHashMap<Ty, &'static TypeDesc>>,
    /// Every descriptor created (including closure environments).
    pub created: RefCell<Vec<&'static TypeDesc>>,
}

fn align(off: u32, size: u32) -> u32 {
    off.div_ceil(size) * size
}

impl<'g, 'a> Layouts<'g, 'a> {
    pub fn new(g: &'g Global<'a>) -> Self {
        Layouts { g, descs: RefCell::new(FxHashMap::default()), created: RefCell::new(vec![]) }
    }

    pub fn sym(&self, s: Symbol) -> &str {
        self.g.interner.resolve(s)
    }

    /// Standard-library name of a nominal definition (`None` for user types).
    pub fn is_repr_c(&self, d: DefId) -> bool {
        matches!(&self.g.hir.defs[d].kind, badger_hir::DefKind::Struct(s) if s.repr_c)
    }

    /// C layout of an FFI type: size, alignment, and its scalar leaves
    /// (offset, size, is-float). `None` for types that cannot cross.
    pub fn c_layout(&self, t: &Ty) -> Option<(u32, u32, Vec<(u32, u32, bool)>)> {
        let TyKind::Con(d, args) = &**t else { return None };
        let scalar = |size: u32, float: bool| Some((size, size, vec![(0, size, float)]));
        match self.std_name(*d) {
            Some("Bool" | "I8" | "U8") => return scalar(1, false),
            Some("I16" | "U16") => return scalar(2, false),
            Some("I32" | "U32" | "Char") => return scalar(4, false),
            Some("I64" | "U64" | "Ptr") => return scalar(8, false),
            Some("F32") => return scalar(4, true),
            Some("F64") => return scalar(8, true),
            _ => {}
        }
        if !self.is_repr_c(*d) {
            return None;
        }
        let (mut off, mut align_max, mut leaves) = (0u32, 1u32, vec![]);
        for (_, ft, _) in self.g.env.struct_fields(*d, args) {
            let ft = badger_typeck::ground_resolved(&ft);
            let (size, al, ls) = self.c_layout(&ft)?;
            off = align(off, al);
            leaves.extend(ls.into_iter().map(|(o, s, f)| (o + off, s, f)));
            off += size;
            align_max = align_max.max(al);
        }
        Some((align(off.max(1), align_max), align_max, leaves))
    }

    /// How a value of type `t` is passed to or returned from C.
    pub fn ffi_arg(&self, t: &Ty) -> Option<crate::mir::FfiArg> {
        use crate::mir::FfiArg;
        let TyKind::Con(d, _) = &**t else { return None };
        if !self.is_repr_c(*d) {
            return Some(FfiArg::Scalar(self.repr(t)));
        }
        let (size, _, leaves) = self.c_layout(t)?;
        let desc = self.desc(t) as *const TypeDesc as u64;
        if size > 16 {
            return Some(FfiArg::Memory { desc, size });
        }
        // SysV: an eightbyte is SSE when every scalar in it is a float
        let n = size.div_ceil(8);
        let sse = (0..n)
            .map(|i| {
                let (lo, hi) = (i * 8, i * 8 + 8);
                let inside: Vec<&(u32, u32, bool)> =
                    leaves.iter().filter(|(o, s, _)| *o < hi && o + s > lo).collect();
                !inside.is_empty() && inside.iter().all(|(_, _, f)| *f)
            })
            .collect();
        Some(FfiArg::Regs { desc, size, sse })
    }

    pub fn std_name(&self, d: DefId) -> Option<&str> {
        let def = &self.g.hir.defs[d];
        if self.g.hir.modules[def.module].is_std { Some(self.sym(def.name)) } else { None }
    }

    pub fn def_name(&self, d: DefId) -> String {
        self.sym(self.g.hir.defs[d].name).to_string()
    }

    pub fn repr(&self, t: &Ty) -> Repr {
        match &**t {
            TyKind::Tuple(ts) if ts.is_empty() => Repr::I8,
            TyKind::Con(d, _) => match self.std_name(*d) {
                Some("Bool" | "I8" | "U8" | "Never") => Repr::I8,
                Some("I16" | "U16") => Repr::I16,
                Some("I32" | "U32" | "Char") => Repr::I32,
                Some("I64" | "U64" | "Duration" | "Instant" | "Timestamp" | "Ptr") => Repr::I64,
                Some("F32") => Repr::F32,
                Some("F64") => Repr::F64,
                _ => Repr::Ptr,
            },
            _ => Repr::Ptr,
        }
    }

    pub fn num_ty(&self, t: &Ty) -> Option<crate::mir::NumTy> {
        use crate::mir::NumTy;
        match &**t {
            TyKind::Con(d, _) => Some(match self.std_name(*d)? {
                "I8" => NumTy::I(8, true),
                "I16" => NumTy::I(16, true),
                "I32" => NumTy::I(32, true),
                "I64" | "Duration" | "Instant" | "Timestamp" => NumTy::I(64, true),
                "U8" | "Bool" => NumTy::I(8, false),
                "U16" => NumTy::I(16, false),
                "U32" | "Char" => NumTy::I(32, false),
                "U64" => NumTy::I(64, false),
                "F32" => NumTy::F(32),
                "F64" => NumTy::F(64),
                _ => return None,
            }),
            TyKind::Tuple(ts) if ts.is_empty() => Some(NumTy::I(8, false)),
            _ => None,
        }
    }

    /// Layout of a record, tuple, or struct type.
    pub fn record(&self, t: &Ty) -> RecLayout {
        let mut items: Vec<(Symbol, String, Ty)> = match &**t {
            TyKind::Record(r) => {
                let (fs, _) = badger_typeck::ty::row_fields(r);
                let mut v: Vec<(Symbol, String, Ty)> =
                    fs.into_iter().map(|(l, ty)| (l, self.sym(l).to_string(), ty)).collect();
                v.sort_by(|a, b| a.1.cmp(&b.1));
                v
            }
            TyKind::Tuple(ts) => ts
                .iter()
                .enumerate()
                .map(|(i, ty)| (self.g.interner.intern(&i.to_string()), i.to_string(), ty.clone()))
                .collect(),
            TyKind::Dyn(p, _) => {
                // boxed data, then one closure per protocol method (by name)
                let any_fn = badger_typeck::ty::mk(TyKind::Fn(badger_typeck::FnTy {
                    params: vec![],
                    ret: badger_typeck::ty::unit(),
                    eff: badger_typeck::ty::row_empty(),
                    once: false,
                }));
                // methods of the protocol and of its supertraits
                let mut names: Vec<Symbol> = vec![];
                let mut stack = vec![*p];
                let mut seen = vec![];
                while let Some(q) = stack.pop() {
                    if seen.contains(&q) {
                        continue;
                    }
                    seen.push(q);
                    if let Some(info) = self.g.env.protocols.get(&q) {
                        names.extend(
                            info.methods.keys().copied().filter(|m| !names.contains(m)).collect::<Vec<_>>(),
                        );
                        stack.extend(info.supers.iter().copied());
                    }
                }
                let mut ms: Vec<(Symbol, String, Ty)> =
                    names.into_iter().map(|m| (m, format!("m_{}", self.sym(m)), any_fn.clone())).collect();
                ms.sort_by(|a, b| a.1.cmp(&b.1));
                let mut v = vec![(self.g.interner.intern("data"), "data".to_string(), any_fn)];
                v.extend(ms);
                v
            }
            TyKind::Con(d, args) if self.g.env.structs.contains_key(d) => {
                let mut v: Vec<(Symbol, String, Ty)> = self
                    .g
                    .env
                    .struct_fields(*d, args)
                    .into_iter()
                    .map(|(l, ty, _)| (l, self.sym(l).to_string(), badger_typeck::ground_resolved(&ty)))
                    .collect();
                // `repr c` structs keep their declared field order
                if !self.is_repr_c(*d) {
                    v.sort_by(|a, b| a.1.cmp(&b.1));
                }
                v
            }
            _ => vec![],
        };
        let mut off = 0u32;
        let mut fields = vec![];
        let is_dyn = matches!(&**t, TyKind::Dyn(..));
        if !is_dyn {
            // records/structs sort by label (tuples are positional)
        }
        for (label, name, ty) in items.drain(..) {
            let repr = self.repr(&ty);
            let s = repr.size();
            off = align(off, s);
            fields.push(FieldLayout { label, name, offset: off as i32, repr, ty });
            off += s;
        }
        RecLayout { size: align(off.max(1), 8), fields }
    }

    /// Variant layouts of an enum type (fields start at offset 8).
    pub fn enum_variants(&self, t: &Ty) -> (u32, Vec<(String, Vec<FieldLayout>)>) {
        let TyKind::Con(d, args) = &**t else { return (8, vec![]) };
        let Some(info) = self.g.env.enums.get(d) else { return (8, vec![]) };
        let mut size = 8u32;
        let mut out = vec![];
        for (vi, v) in info.variants.iter().enumerate() {
            let mut off = 8u32;
            let mut fields = vec![];
            for (i, (l, ty)) in self.g.env.variant_fields(*d, vi as u32, args).into_iter().enumerate() {
                let ty = badger_typeck::ground_resolved(&ty);
                let repr = self.repr(&ty);
                off = align(off, repr.size());
                let name = l.map(|l| self.sym(l).to_string()).unwrap_or_else(|| i.to_string());
                let label = l.unwrap_or_else(|| self.g.interner.intern(&i.to_string()));
                fields.push(FieldLayout { label, name, offset: off as i32, repr, ty });
                off += repr.size();
            }
            size = size.max(align(off, 8));
            out.push((self.sym(v.name).to_string(), fields));
        }
        (size, out)
    }

    /// Layout of a closure's captures (after code and adapter pointers).
    pub fn closure(&self, captures: &[Ty]) -> RecLayout {
        let mut off = 16u32;
        let mut fields = vec![];
        for (i, ty) in captures.iter().enumerate() {
            let repr = self.repr(ty);
            off = align(off, repr.size());
            fields.push(FieldLayout {
                label: Symbol::dummy(),
                name: format!("capture{i}"),
                offset: off as i32,
                repr,
                ty: ty.clone(),
            });
            off += repr.size();
        }
        RecLayout { size: align(off, 8), fields }
    }

    pub fn closure_desc(&self, layout: &RecLayout) -> &'static TypeDesc {
        let captures = layout
            .fields
            .iter()
            .map(|f| Field {
                name: f.name.clone(),
                offset: f.offset as u32,
                repr: f.repr,
                desc: self.desc(&f.ty),
            })
            .collect();
        let d = TypeDesc::new("closure", Kind::Closure { size: layout.size, captures }).leak();
        self.created.borrow_mut().push(d);
        d
    }

    fn fields_of(&self, l: &RecLayout) -> Vec<Field> {
        l.fields
            .iter()
            .map(|f| Field {
                name: f.name.clone(),
                offset: f.offset as u32,
                repr: f.repr,
                desc: self.desc(&f.ty),
            })
            .collect()
    }

    /// All descriptors created so far.
    pub fn all(&self) -> Vec<(Ty, &'static TypeDesc)> {
        self.descs.borrow().iter().map(|(t, d)| (t.clone(), *d)).collect()
    }

    /// Runtime descriptor of a ground type (cached; recursive types allowed).
    pub fn desc(&self, t: &Ty) -> &'static TypeDesc {
        if let Some(d) = self.descs.borrow().get(t) {
            return d;
        }
        // a polymorphically recursive type nests without bound; past the
        // specialization depth only stub code mentions it, and its objects
        // are traced through their own headers
        if crate::lower::type_depth(t) > crate::lower::SPECIALIZATION_DEPTH {
            let d: &'static TypeDesc = TypeDesc::new(
                "<beyond specialization depth>",
                Kind::Opaque { trace: None, finalize: None, args: vec![] },
            )
            .leak();
            self.descs.borrow_mut().insert(t.clone(), d);
            self.created.borrow_mut().push(d);
            return d;
        }
        let name = self.type_name(t);
        let p: *mut TypeDesc = Box::into_raw(Box::new(TypeDesc::new(name, Kind::Unit)));
        let d: &'static TypeDesc = unsafe { &*p };
        self.descs.borrow_mut().insert(t.clone(), d);
        self.created.borrow_mut().push(d);
        let kind = self.kind_of(t);
        unsafe {
            (*p).kind = kind;
            (*p).leaf.store(0, std::sync::atomic::Ordering::Relaxed);
            (*p).floats.store(0, std::sync::atomic::Ordering::Relaxed);
        }
        d
    }

    fn kind_of(&self, t: &Ty) -> Kind {
        match &**t {
            TyKind::Tuple(ts) if ts.is_empty() => Kind::Unit,
            TyKind::Tuple(_) | TyKind::Record(_) => {
                let l = self.record(t);
                Kind::Record {
                    size: l.size,
                    fields: self.fields_of(&l),
                    nominal: false,
                    tuple: matches!(&**t, TyKind::Tuple(_)),
                    flat: false,
                }
            }
            TyKind::Fn(_) => Kind::Fn,
            TyKind::Dyn(..) => {
                let l = self.record(t);
                Kind::Record {
                    size: l.size,
                    fields: self.fields_of(&l),
                    nominal: true,
                    tuple: false,
                    flat: false,
                }
            }
            TyKind::Con(d, args) => {
                if self.g.env.structs.contains_key(d) {
                    let l = self.record(t);
                    let fields = self.fields_of(&l);
                    // `@flatten` on a one-field struct
                    let flat = fields.len() == 1
                        && self.g.hir.attributes.get(d).is_some_and(|attrs| {
                            attrs.iter().any(|a| self.g.interner.resolve(a.name) == "flatten")
                        });
                    return Kind::Record { size: l.size, fields, nominal: true, tuple: false, flat };
                }
                if self.g.env.enums.contains_key(d) {
                    let (size, vs) = self.enum_variants(t);
                    let variants = vs
                        .into_iter()
                        .map(|(name, fs)| Variant {
                            name,
                            fields: fs
                                .iter()
                                .map(|f| Field {
                                    name: f.name.clone(),
                                    offset: f.offset as u32,
                                    repr: f.repr,
                                    desc: self.desc(&f.ty),
                                })
                                .collect(),
                        })
                        .collect();
                    return Kind::Enum { size, variants };
                }
                let arg = |i: usize| args.get(i).map(|a| self.desc(a)).unwrap_or(builtin::unit());
                match self.std_name(*d) {
                    Some("Bool") => Kind::Bool,
                    Some("I8") => Kind::Int { bits: 8, signed: true },
                    Some("I16") => Kind::Int { bits: 16, signed: true },
                    Some("I32") => Kind::Int { bits: 32, signed: true },
                    Some("I64") => Kind::Int { bits: 64, signed: true },
                    Some("U8") => Kind::Int { bits: 8, signed: false },
                    Some("U16") => Kind::Int { bits: 16, signed: false },
                    Some("U32") => Kind::Int { bits: 32, signed: false },
                    Some("U64") => Kind::Int { bits: 64, signed: false },
                    Some("F32") => Kind::Float { bits: 32 },
                    Some("F64") => Kind::Float { bits: 64 },
                    Some("Char") => Kind::Char,
                    Some("Symbol") => Kind::Symbol,
                    Some("String") => Kind::String,
                    Some("Bytes") => Kind::Bytes,
                    Some("Duration") => Kind::Duration,
                    Some("Instant") => Kind::Instant,
                    Some("Timestamp") => Kind::Timestamp,
                    // a raw C address: a plain word, never traced
                    Some("Ptr") => Kind::Int { bits: 64, signed: false },
                    Some("Never") => Kind::Unit,
                    Some("Dynamic") => Kind::Dynamic,
                    Some("List" | "ListBuilder") => Kind::List { elem: arg(0) },
                    Some("Map") => Kind::Map { key: arg(0), val: arg(1) },
                    Some("Set") => Kind::Set { elem: arg(0) },
                    Some("Range") => Kind::Range { elem: arg(0) },
                    _ => Kind::Opaque {
                        trace: None,
                        finalize: None,
                        args: args.iter().map(|a| self.desc(a)).collect(),
                    },
                }
            }
            _ => Kind::Unit,
        }
    }

    pub fn type_name(&self, t: &Ty) -> String {
        match &**t {
            // a secret's codec redacts it, wherever it is nested
            TyKind::Con(d, _) if self.std_name(*d) == Some("Secret") => "std.Secret".into(),
            TyKind::Con(d, _) => self.def_name(*d),
            TyKind::Tuple(ts) if ts.is_empty() => "Unit".into(),
            TyKind::Tuple(_) => "Tuple".into(),
            TyKind::Record(_) => "Record".into(),
            TyKind::Fn(_) => "Fn".into(),
            TyKind::Dyn(p, _) => format!("dyn {}", self.def_name(*p)),
            _ => "?".into(),
        }
    }

    /// Is `t` the named standard-library type?
    pub fn is_std(&self, t: &Ty, name: &str) -> bool {
        matches!(&**t, TyKind::Con(d, _) if self.std_name(*d) == Some(name))
    }

    pub fn args<'t>(&self, t: &'t Ty) -> &'t [Ty] {
        match &**t {
            TyKind::Con(_, a) => a,
            _ => &[],
        }
    }
}
