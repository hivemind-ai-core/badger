//! Collection, text, environment, and filesystem intrinsics (uniform ABI).

use crate::desc::{Kind, TypeDesc};
use crate::gc::Roots;
use crate::intrinsics::{Targs, rdesc, targ};
use crate::intrinsics_io::{io_err, io_ok};
use crate::string::{new_string, str_of};
use crate::task::TaskCtx;
use crate::{list, make_variant, map};

fn some(ret: &'static TypeDesc, w: u64) -> u64 {
    make_variant(ret, "some", &[w])
}

fn none(ret: &'static TypeDesc) -> u64 {
    make_variant(ret, "none", &[])
}

fn bool_w(b: bool) -> u64 {
    b as u64
}

/// Descriptor of `List<String>` style results: the `ret` itself.
fn strings_to_list(ret: &'static TypeDesc, items: impl IntoIterator<Item = String>) -> u64 {
    let r = Roots::new();
    let l = r.add(list::new(ret));
    for s in items {
        let w = r.add(new_string(&s));
        list::push(l, w);
    }
    l
}

/// Atomic access to an `Atom<T>`'s value field (payload offset 0).
fn atom_field(a: u64) -> *mut u8 {
    unsafe { crate::gc::obj(a).payload() }
}

fn atomic_load(r: crate::desc::Repr, p: *mut u8) -> u64 {
    use crate::desc::Repr;
    use std::sync::atomic::*;
    unsafe {
        match r {
            Repr::I8 => AtomicU8::from_ptr(p).load(Ordering::SeqCst) as u64,
            Repr::I16 => AtomicU16::from_ptr(p as *mut u16).load(Ordering::SeqCst) as u64,
            Repr::I32 | Repr::F32 => AtomicU32::from_ptr(p as *mut u32).load(Ordering::SeqCst) as u64,
            Repr::I64 | Repr::F64 | Repr::Ptr => AtomicU64::from_ptr(p as *mut u64).load(Ordering::SeqCst),
        }
    }
}

fn atomic_cas(r: crate::desc::Repr, p: *mut u8, old: u64, new: u64) -> bool {
    use crate::desc::Repr;
    use std::sync::atomic::*;
    let o = Ordering::SeqCst;
    unsafe {
        match r {
            Repr::I8 => AtomicU8::from_ptr(p).compare_exchange(old as u8, new as u8, o, o).is_ok(),
            Repr::I16 => {
                AtomicU16::from_ptr(p as *mut u16).compare_exchange(old as u16, new as u16, o, o).is_ok()
            }
            Repr::I32 | Repr::F32 => {
                AtomicU32::from_ptr(p as *mut u32).compare_exchange(old as u32, new as u32, o, o).is_ok()
            }
            Repr::I64 | Repr::F64 | Repr::Ptr => {
                AtomicU64::from_ptr(p as *mut u64).compare_exchange(old, new, o, o).is_ok()
            }
        }
    }
}

macro_rules! std_intrinsics {
    ($( fn $name:ident($ctx:ident, $targs:ident, $ret:ident $(, $arg:ident)*) $body:block )*) => {
        $(
            #[allow(unused_variables)]
            pub extern "C" fn $name($ctx: *mut TaskCtx, $targs: Targs, $ret: *const TypeDesc $(, $arg: u64)*) -> u64 $body
        )*
        pub fn symbols() -> Vec<(&'static str, *const u8)> {
            vec![$((concat!("lang_rt_i_", stringify!($name)), $name as *const u8)),*]
        }
    };
}

std_intrinsics! {
    // ---------------------------------------------------------------- errors
    // default `Error.code`: the variant name of an enum, else the type name
    fn error_code(ctx, targs, ret, v) {
        let d = targ(targs, 0);
        if let Kind::Enum { variants, .. } = &d.kind
            && v != 0 {
                let tag = unsafe { *(crate::gc::obj(v).payload() as *const u32) };
                if let Some(var) = variants.get(tag as usize) {
                    return crate::string::symbol(&var.name);
                }
            }
        let name = d.name.rsplit('.').next().unwrap_or(&d.name).to_string();
        let mut snake = String::new();
        for (i, c) in name.chars().enumerate() {
            if c.is_uppercase() {
                if i > 0 {
                    snake.push('_');
                }
                snake.extend(c.to_lowercase());
            } else if c.is_alphanumeric() || c == '_' {
                snake.push(c);
            }
        }
        crate::string::symbol(&snake)
    }
    // default `Error.details`: none
    // default structured details: the error's encodable payload
    fn error_details(ctx, targs, ret, v) {
        let d = targ(targs, 0);
        let j: serde_json::Value = serde_json::from_str(&crate::codec::encode(d, v)).unwrap_or(serde_json::Value::Null);
        // a unit variant or payload-free error has no details
        let j = match j {
            serde_json::Value::String(_) => serde_json::Value::Null,
            other => other,
        };
        crate::codec::data_of(rdesc(ret), &j)
    }
    // ---------------------------------------------------------------- atoms
    fn atom_get(ctx, targs, ret, a) { atomic_load(targ(targs, 0).repr(), atom_field(a)) }
    fn atom_set(ctx, targs, ret, a, v) {
        let r = targ(targs, 0).repr();
        let p = atom_field(a);
        loop {
            let cur = atomic_load(r, p);
            if atomic_cas(r, p, cur, v) {
                return 0;
            }
        }
    }
    fn atom_cas(ctx, targs, ret, a, old, new) { bool_w(atomic_cas(targ(targs, 0).repr(), atom_field(a), old, new)) }
    // ---------------------------------------------------------------- lists
    fn list_len(ctx, targs, ret, l) { list::len(l) as u64 }
    fn list_get(ctx, targs, ret, l, i) {
        let n = list::len(l);
        if (i as i64) >= 0 && (i as usize) < n { some(rdesc(ret), list::get(l, i as usize)) } else { none(rdesc(ret)) }
    }
    fn list_builder(ctx, targs, ret) { list::new(rdesc(ret)) }
    fn list_builder_append(ctx, targs, ret, b, v) { list::push(b, v); 0 }
    fn list_builder_len(ctx, targs, ret, b) { list::len(b) as u64 }
    fn list_finish(ctx, targs, ret, b) { list::finish(b, rdesc(ret)) }
    fn list_append(ctx, targs, ret, l, v) { list::append(l, v, rdesc(ret)) }
    fn list_concat(ctx, targs, ret, a, b) {
        let r = Roots::new();
        r.add(a);
        r.add(b);
        let out = r.add(list::new(rdesc(ret)));
        for x in [a, b] {
            for i in 0..list::len(x) {
                list::push(out, list::get(x, i));
            }
        }
        out
    }
    fn list_reverse(ctx, targs, ret, l) {
        let r = Roots::new();
        r.add(l);
        let out = r.add(list::new(rdesc(ret)));
        for i in (0..list::len(l)).rev() {
            list::push(out, list::get(l, i));
        }
        out
    }
    fn list_slice(ctx, targs, ret, l, a, b) {
        let r = Roots::new();
        r.add(l);
        let n = list::len(l) as i64;
        let (a, b) = ((a as i64).clamp(0, n), (b as i64).clamp(0, n));
        let out = r.add(list::new(rdesc(ret)));
        for i in a..b.max(a) {
            list::push(out, list::get(l, i as usize));
        }
        out
    }
    fn list_sort(ctx, targs, ret, l) {
        let r = Roots::new();
        r.add(l);
        let elem = targ(targs, 0);
        let mut v = list::to_vec(l);
        v.sort_by(|a, b| crate::value::cmp(elem, *a, *b));
        let out = r.add(list::new(rdesc(ret)));
        for w in v {
            list::push(out, w);
        }
        out
    }
    fn list_contains(ctx, targs, ret, l, v) {
        let elem = targ(targs, 0);
        bool_w((0..list::len(l)).any(|i| crate::value::eq(elem, list::get(l, i), v)))
    }
    fn page_count(ctx, targs, ret, l) { list::data(l).pages.len() as u64 }
    // a range's start, end, or inclusive flag (words 0, 1, 2 of its payload)
    fn range_field(ctx, targs, ret, r, i) { unsafe { *(crate::gc::obj(r).payload().add(i as usize * 8) as *const u64) } }
    fn element_address(ctx, targs, ret, l, i) { list::addr(l, i as usize) as u64 }
    fn list_page(ctx, targs, ret, l, i) {
        let d = list::data(l);
        if i as usize >= d.len {
            crate::task::panic_current(format!("index out of bounds: the length is {} but the index is {}", d.len, i as i64), String::new());
        }
        d.pages[i as usize / d.page_cap]
    }
    fn list_slot(ctx, targs, ret, l, i) { (i as usize % list::data(l).page_cap) as u64 }
    fn page_read(ctx, targs, ret, page, slot) { list::page_read(page, slot as usize, targ(targs, 0).repr()) }
    fn list_to_array(ctx, targs, ret, l) {
        let r = Roots::new();
        r.add(l);
        let items = list::to_vec(l);
        let b = crate::buffer::new(targ(targs, 0), items.len());
        crate::buffer::data(b).items.extend(items);
        crate::buffer::data(b).frozen = true;
        b
    }

    // ---------------------------------------------------------------- buffers
    fn buffer_new(ctx, targs, ret, cap) { crate::buffer::new(targ(targs, 0), cap as usize) }
    fn buffer_push(ctx, targs, ret, b, v) { crate::buffer::data(b).items.push(v); 0 }
    fn buffer_pop(ctx, targs, ret, b) {
        match crate::buffer::data(b).items.pop() { Some(v) => some(rdesc(ret), v), None => none(rdesc(ret)) }
    }
    fn buffer_get(ctx, targs, ret, b, i) {
        match crate::buffer::data(b).items.get(i as usize) { Some(v) => some(rdesc(ret), *v), None => none(rdesc(ret)) }
    }
    fn buffer_set(ctx, targs, ret, b, i, v) {
        let d = crate::buffer::data(b);
        if d.frozen {
            crate::task::panic_current("write to a frozen buffer".into(), crate::task::caller_location());
        }
        match d.items.get_mut(i as usize) {
            Some(slot) => { *slot = v; 0 }
            None => crate::task::panic_current(format!("index out of bounds: the length is {} but the index is {}", d.items.len(), i as i64), crate::task::caller_location()),
        }
    }
    fn buffer_len(ctx, targs, ret, b) { crate::buffer::data(b).items.len() as u64 }
    fn buffer_capacity(ctx, targs, ret, b) { crate::buffer::data(b).items.capacity() as u64 }
    fn buffer_address(ctx, targs, ret, b) { crate::buffer::data(b).items.as_ptr() as u64 }
    fn buffer_freeze(ctx, targs, ret, b) { crate::buffer::data(b).frozen = true; b }
    fn buffer_to_list(ctx, targs, ret, b) {
        let r = Roots::new();
        r.add(b);
        let out = r.add(list::new(rdesc(ret)));
        for v in crate::buffer::data(b).items.clone() {
            list::push(out, v);
        }
        out
    }

    fn buffer_to_bytes(ctx, targs, ret, b) {
        let items = &crate::buffer::data(b).items;
        let bytes: Vec<u8> = items.iter().map(|w| *w as u8).collect();
        crate::string::new_bytes(&bytes)
    }
    fn bytes_get(ctx, targs, ret, b, i) {
        let bs = crate::string::bytes_of(b);
        match bs.get(i as usize) {
            Some(x) => *x as u64,
            None => crate::task::panic_current(format!("index out of bounds: the length is {} but the index is {}", bs.len(), i as i64), crate::task::caller_location()),
        }
    }
    fn list_chunks(ctx, targs, ret, l) {
        // one contiguous (frozen) array per page
        let r = Roots::new();
        r.add(l);
        let out = r.add(list::new(rdesc(ret)));
        let elem = targ(targs, 0);
        let d = list::data(l);
        let (cap, len) = (d.page_cap, d.len);
        let mut i = 0;
        while i < len {
            let end = (i + cap).min(len);
            let a = r.add(crate::buffer::new(elem, end - i));
            for k in i..end {
                crate::buffer::data(a).items.push(list::get(l, k));
            }
            crate::buffer::data(a).frozen = true;
            list::push(out, a);
            i = end;
        }
        out
    }
    fn schema(ctx, targs, ret) {
        // a schema value carries its type's descriptor
        let w = crate::gc::alloc_immortal(rdesc(ret), 8);
        unsafe { *(crate::gc::obj(w).payload() as *mut *const TypeDesc) = targ(targs, 0) };
        w
    }
    fn dynamic_display(ctx, targs, ret, v) {
        let d = unsafe { crate::gc::obj(v).desc() };
        new_string(&crate::value::display(ctx, d, v, true))
    }
    fn url_encode(ctx, targs, ret, s) {
        let mut out = String::new();
        for b in str_of(s).bytes() {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                out.push(b as char);
            } else {
                out.push_str(&format!("%{b:02X}"));
            }
        }
        new_string(&out)
    }
    fn url_decode(ctx, targs, ret, s) {
        let bytes = str_of(s).as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
            if bytes[i] == b'%'
                && let (Some(h), Some(l)) = (bytes.get(i + 1).and_then(|c| hex(*c)), bytes.get(i + 2).and_then(|c| hex(*c))) {
                    out.push(h * 16 + l);
                    i += 3;
                    continue;
                }
            out.push(bytes[i]);
            i += 1;
        }
        new_string(&String::from_utf8_lossy(&out))
    }
    fn json_schema_of(ctx, targs, ret) { new_string(&json_schema(targ(targs, 0), 0)) }
    fn schema_describe(ctx, targs, ret, s) {
        let d = unsafe { &**(crate::gc::obj(s).payload() as *const *const TypeDesc) };
        new_string(&describe(d, 0))
    }

    // ---------------------------------------------------------------- maps
    fn map_empty(ctx, targs, ret) { map::new(rdesc(ret)) }
    fn map_insert(ctx, targs, ret, m, k, v) { map::insert(m, k, v) }
    fn map_remove(ctx, targs, ret, m, k) { map::remove(m, k) }
    fn map_get(ctx, targs, ret, m, k) {
        match map::get(m, k) { Some(v) => some(rdesc(ret), v), None => none(rdesc(ret)) }
    }
    fn map_contains(ctx, targs, ret, m, k) { bool_w(map::get(m, k).is_some()) }
    fn map_len(ctx, targs, ret, m) { map::len(m) as u64 }
    fn map_entries(ctx, targs, ret, m) {
        // List<(K, V)>
        let r = Roots::new();
        r.add(m);
        let out = r.add(list::new(rdesc(ret)));
        let tuple = match &rdesc(ret).kind { Kind::List { elem } => *elem, _ => return out };
        for (k, v) in map::entries(m) {
            let t = r.add(crate::make_record(tuple, &[("0", k), ("1", v)]));
            list::push(out, t);
        }
        out
    }
    fn set_insert(ctx, targs, ret, m, k) { map::insert(m, k, 0) }
    fn map_insert_mut(ctx, targs, ret, m, k, v) { map::insert_mut(m, k, v); 0 }
    fn set_insert_mut(ctx, targs, ret, m, k) { map::insert_mut(m, k, 0); 0 }
    fn set_remove(ctx, targs, ret, m, k) { map::remove(m, k) }
    fn set_items(ctx, targs, ret, m) {
        let r = Roots::new();
        r.add(m);
        let out = r.add(list::new(rdesc(ret)));
        for (k, _) in map::entries(m) {
            list::push(out, k);
        }
        out
    }

    // ---------------------------------------------------------------- text
    fn text_lines(ctx, targs, ret, s) {
        let v: Vec<String> = str_of(s).lines().map(str::to_string).collect();
        strings_to_list(rdesc(ret), v)
    }
    fn text_split(ctx, targs, ret, s, sep) {
        let v: Vec<String> = str_of(s).split(str_of(sep)).map(str::to_string).collect();
        strings_to_list(rdesc(ret), v)
    }
    fn text_trim(ctx, targs, ret, s) { new_string(str_of(s).trim()) }
    fn text_len(ctx, targs, ret, s) { str_of(s).chars().count() as u64 }
    fn text_byte_len(ctx, targs, ret, s) { str_of(s).len() as u64 }
    fn text_is_empty(ctx, targs, ret, s) { bool_w(str_of(s).is_empty()) }
    fn text_contains(ctx, targs, ret, s, p) { bool_w(str_of(s).contains(str_of(p))) }
    fn text_starts_with(ctx, targs, ret, s, p) { bool_w(str_of(s).starts_with(str_of(p))) }
    fn text_ends_with(ctx, targs, ret, s, p) { bool_w(str_of(s).ends_with(str_of(p))) }
    fn text_to_upper(ctx, targs, ret, s) { new_string(&str_of(s).to_uppercase()) }
    fn text_to_lower(ctx, targs, ret, s) { new_string(&str_of(s).to_lowercase()) }
    fn text_replace(ctx, targs, ret, s, a, b) { new_string(&str_of(s).replace(str_of(a), str_of(b))) }
    fn text_concat(ctx, targs, ret, a, b) { new_string(&format!("{}{}", str_of(a), str_of(b))) }
    fn text_repeat(ctx, targs, ret, s, n) { new_string(&str_of(s).repeat(n as usize)) }
    fn text_join(ctx, targs, ret, l, sep) {
        let parts: Vec<&str> = list::to_vec(l).into_iter().map(str_of).collect();
        new_string(&parts.join(str_of(sep)))
    }
    fn text_chars(ctx, targs, ret, s) {
        let r = Roots::new();
        let out = r.add(list::new(rdesc(ret)));
        for c in str_of(s).chars() {
            list::push(out, c as u64);
        }
        out
    }
    fn text_slice(ctx, targs, ret, s, a, b) {
        // character-indexed slice; returns Option<String>
        let st = str_of(s);
        let chars: Vec<(usize, char)> = st.char_indices().collect();
        let n = chars.len() as i64;
        let (a, b) = (a as i64, b as i64);
        if a < 0 || b < a || b > n {
            return none(rdesc(ret));
        }
        let start = chars.get(a as usize).map(|c| c.0).unwrap_or(st.len());
        let end = chars.get(b as usize).map(|c| c.0).unwrap_or(st.len());
        let w = new_string(&st[start..end]);
        some(rdesc(ret), w)
    }
    fn text_parse_int(ctx, targs, ret, s) {
        match str_of(s).trim().parse::<i64>() { Ok(v) => some(rdesc(ret), v as u64), Err(_) => none(rdesc(ret)) }
    }
    fn text_parse_float(ctx, targs, ret, s) {
        match str_of(s).trim().parse::<f64>() { Ok(v) => some(rdesc(ret), v.to_bits()), Err(_) => none(rdesc(ret)) }
    }
    fn text_from_bytes(ctx, targs, ret, b) {
        match std::str::from_utf8(crate::string::bytes_of(b)) {
            Ok(s) => { let w = new_string(s); make_variant(rdesc(ret), "ok", &[w]) }
            Err(e) => { let m = new_string(&format!("invalid UTF-8: {e}")); make_variant(rdesc(ret), "err", &[m]) }
        }
    }
    fn text_to_bytes(ctx, targs, ret, s) { crate::string::new_bytes(crate::string::bytes_of(s)) }

    // ---------------------------------------------------------------- env & fs
    fn env_args(ctx, targs, ret) { strings_to_list(rdesc(ret), crate::intrinsics_io::args()) }
    fn env_get(ctx, targs, ret, name) {
        match std::env::var(str_of(name)) { Ok(v) => { let w = new_string(&v); some(rdesc(ret), w) } Err(_) => none(rdesc(ret)) }
    }
    fn fs_read_text(ctx, targs, ret, path) {
        let p = str_of(path).to_string();
        let r = crate::task::run_blocking(move || std::fs::read_to_string(p));
        match r { Ok(s) => { let w = new_string(&s); io_ok(rdesc(ret), w) } Err(e) => io_err(rdesc(ret), &e) }
    }
    fn fs_write_text(ctx, targs, ret, path, text) {
        let (p, t) = (str_of(path).to_string(), str_of(text).to_string());
        match crate::task::run_blocking(move || std::fs::write(p, t)) { Ok(()) => io_ok(rdesc(ret), 0), Err(e) => io_err(rdesc(ret), &e) }
    }
    fn fs_exists(ctx, targs, ret, path) { bool_w(std::path::Path::new(str_of(path)).exists()) }
}

/// JSON description of a schema descriptor.
/// JSON Schema of a type's wire contract (honoring active wire names), as
/// used by OpenAPI documents.
pub fn json_schema(d: &TypeDesc, depth: usize) -> String {
    use crate::intrinsics_io::json_quote;
    if depth > 8 {
        return "{}".into();
    }
    match &d.kind {
        Kind::Unit => "{\"type\":\"null\"}".into(),
        Kind::Bool => "{\"type\":\"boolean\"}".into(),
        Kind::Int { .. } | Kind::Duration | Kind::Instant | Kind::Timestamp => {
            "{\"type\":\"integer\"}".into()
        }
        Kind::Float { .. } => "{\"type\":\"number\"}".into(),
        Kind::String | Kind::Symbol | Kind::Char | Kind::Bytes => "{\"type\":\"string\"}".into(),
        Kind::List { elem } | Kind::Set { elem } => {
            format!("{{\"type\":\"array\",\"items\":{}}}", json_schema(elem, depth + 1))
        }
        Kind::Opaque { .. } if crate::buffer::array_elem(d).is_some() => {
            let elem = crate::buffer::array_elem(d).expect("array");
            format!("{{\"type\":\"array\",\"items\":{}}}", json_schema(elem, depth + 1))
        }
        Kind::Map { val, .. } => {
            format!("{{\"type\":\"object\",\"additionalProperties\":{}}}", json_schema(val, depth + 1))
        }
        Kind::Record { fields, tuple: true, .. } => {
            let items: Vec<String> = fields.iter().map(|f| json_schema(f.desc, depth + 1)).collect();
            format!(
                "{{\"type\":\"array\",\"prefixItems\":[{}],\"minItems\":{},\"maxItems\":{}}}",
                items.join(","),
                items.len(),
                items.len()
            )
        }
        Kind::Record { fields, .. } => {
            let props: Vec<String> = fields
                .iter()
                .map(|f| {
                    format!(
                        "{}:{}",
                        json_quote(&crate::codec::wire_name(&d.name, &f.name)),
                        json_schema(f.desc, depth + 1)
                    )
                })
                .collect();
            let required: Vec<String> = fields
                .iter()
                .filter(|f| f.desc.name != "Option")
                .map(|f| json_quote(&crate::codec::wire_name(&d.name, &f.name)))
                .collect();
            format!(
                "{{\"type\":\"object\",\"properties\":{{{}}},\"required\":[{}]}}",
                props.join(","),
                required.join(",")
            )
        }
        Kind::Enum { variants, .. } if d.name == "Option" => match variants
            .get(1)
            .and_then(|v| v.fields.first())
        {
            Some(f) => format!("{{\"anyOf\":[{},{{\"type\":\"null\"}}]}}", json_schema(f.desc, depth + 1)),
            None => "{}".into(),
        },
        Kind::Enum { .. } if d.name == "Data" => "{}".into(),
        Kind::Enum { variants, .. } => {
            let units: Vec<String> = variants
                .iter()
                .filter(|v| v.fields.is_empty())
                .map(|v| json_quote(&crate::codec::wire_name(&d.name, &v.name)))
                .collect();
            let mut alts: Vec<String> = vec![];
            if !units.is_empty() {
                alts.push(format!("{{\"type\":\"string\",\"enum\":[{}]}}", units.join(",")));
            }
            for v in variants.iter().filter(|v| !v.fields.is_empty()) {
                let vty = format!("{}.{}", d.name, v.name);
                let props: Vec<String> = v
                    .fields
                    .iter()
                    .map(|f| {
                        format!(
                            "{}:{}",
                            json_quote(&crate::codec::wire_name(&vty, &f.name)),
                            json_schema(f.desc, depth + 1)
                        )
                    })
                    .collect();
                let req: Vec<String> =
                    v.fields.iter().map(|f| json_quote(&crate::codec::wire_name(&vty, &f.name))).collect();
                let vn = json_quote(&crate::codec::wire_name(&d.name, &v.name));
                alts.push(format!(
                    "{{\"type\":\"object\",\"properties\":{{{vn}:{{\"type\":\"object\",\"properties\":{{{}}},\"required\":[{}]}}}},\"required\":[{vn}]}}",
                    props.join(","),
                    req.join(",")
                ));
            }
            if alts.len() == 1 { alts.pop().unwrap() } else { format!("{{\"oneOf\":[{}]}}", alts.join(",")) }
        }
        _ => "{}".into(),
    }
}

pub fn describe(d: &TypeDesc, depth: usize) -> String {
    use crate::intrinsics_io::json_quote;
    if depth > 6 {
        return json_quote(&d.name);
    }
    match &d.kind {
        Kind::Record { fields, tuple, .. } => {
            let fs: Vec<String> = fields
                .iter()
                .map(|f| {
                    let n = if *tuple { f.name.clone() } else { crate::codec::wire_name(&d.name, &f.name) };
                    format!("{}:{}", json_quote(&n), describe(f.desc, depth + 1))
                })
                .collect();
            if *tuple {
                format!("{{\"tuple\":{{{}}}}}", fs.join(","))
            } else {
                format!("{{\"record\":{},\"fields\":{{{}}}}}", json_quote(&d.name), fs.join(","))
            }
        }
        Kind::Enum { variants, .. } => {
            let vs: Vec<String> = variants
                .iter()
                .map(|v| {
                    let vty = format!("{}.{}", d.name, v.name);
                    let fs: Vec<String> = v
                        .fields
                        .iter()
                        .map(|f| {
                            format!(
                                "{}:{}",
                                json_quote(&crate::codec::wire_name(&vty, &f.name)),
                                describe(f.desc, depth + 1)
                            )
                        })
                        .collect();
                    format!("{}:{{{}}}", json_quote(&crate::codec::wire_name(&d.name, &v.name)), fs.join(","))
                })
                .collect();
            format!("{{\"enum\":{},\"variants\":{{{}}}}}", json_quote(&d.name), vs.join(","))
        }
        Kind::List { elem } => format!("{{\"list\":{}}}", describe(elem, depth + 1)),
        Kind::Opaque { .. } if crate::buffer::array_elem(d).is_some() => {
            format!("{{\"array\":{}}}", describe(crate::buffer::array_elem(d).expect("array"), depth + 1))
        }
        Kind::Map { key, val } => {
            format!("{{\"map\":[{},{}]}}", describe(key, depth + 1), describe(val, depth + 1))
        }
        _ => json_quote(&d.name),
    }
}
