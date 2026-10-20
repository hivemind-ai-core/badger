//! Collection, text, environment, and filesystem intrinsics (uniform ABI).

use crate::desc::{Kind, TypeDesc};
use crate::gc::{self, Roots};
use crate::intrinsics::{rdesc, split_option, targ};
use crate::intrinsics_io::io_ok;
use crate::string::{new_string, str_of};
use crate::{list, make_variant, map};

use crate::intrinsics::{none, some};

fn bool_w(b: bool) -> u64 {
    b as u64
}

/// Descriptor of `List<String>` style results: the `ret` itself.
fn strings_to_list<S: AsRef<str>>(ret: &'static TypeDesc, items: impl IntoIterator<Item = S>) -> u64 {
    crate::intrinsics::strings(ret, items)
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

define_intrinsics! {
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
                gc::barrier(a);
                return 0;
            }
        }
    }
    fn atom_cas(ctx, targs, ret, a, old, new) {
        let ok = atomic_cas(targ(targs, 0).repr(), atom_field(a), old, new);
        if ok {
            gc::barrier(a);
        }
        bool_w(ok)
    }
    // ---------------------------------------------------------------- lists
    fn list_len(ctx, targs, ret, l) { list::len(l) as u64 }
    fn list_get(ctx, targs, ret, l, i) {
        let n = list::len(l);
        if (i as i64) >= 0 && (i as usize) < n { some(rdesc(ret), list::get(l, i as usize)) } else { none(rdesc(ret)) }
    }
    fn list_get_split(ctx, targs, ret, l, i) {
        let n = list::len(l);
        let value = if (i as i64) >= 0 && (i as usize) < n { Some(list::get(l, i as usize)) } else { None };
        split_option(rdesc(ret), value)
    }
    fn list_builder(ctx, targs, ret) { list::mark_builder(list::new(rdesc(ret))) }
    fn list_builder_capacity(ctx, targs, ret, n) {
        static PLAIN: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let plain = *PLAIN.get_or_init(|| {
            std::env::var("BADGER_LIST_PREALLOC").map(|v| v == "0").unwrap_or(false)
        });
        if plain {
            list::mark_builder(list::new(rdesc(ret)))
        } else {
            list::mark_builder(list::with_capacity(rdesc(ret), (n as i64).max(0) as usize))
        }
    }
    fn list_builder_append(ctx, targs, ret, b, v) {
        gc::budget_check(8);
        list::push(b, v);
        0
    }
    // Bulk append: copy every element of `l` into `b` in whole pages.
    fn list_builder_extend(ctx, targs, ret, b, l) {
        let r = Roots::new();
        r.add(b);
        r.add(l);
        list::push_run(b, l, 0, list::len(l));
        0
    }
    // Bulk construction of the integers `start..end` (pages filled directly).
    fn list_range(ctx, targs, ret, start, end) {
        let n = (end as i64 - start as i64).max(0) as usize;
        list::from_range(rdesc(ret), start as i64, n)
    }
    fn list_builder_len(ctx, targs, ret, b) { list::len(b) as u64 }
    fn list_finish(ctx, targs, ret, b) { list::finish(b, rdesc(ret)) }
    fn list_append(ctx, targs, ret, l, v) {
        gc::budget_check(64);
        list::append(l, v, rdesc(ret))
    }
    fn list_concat(ctx, targs, ret, a, b) {
        let r = Roots::new();
        r.add(a);
        r.add(b);
        let out = r.add(list::new(rdesc(ret)));
        list::push_run(out, a, 0, list::len(a));
        list::push_run(out, b, 0, list::len(b));
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
        if b > a {
            list::push_run(out, l, a as usize, b as usize);
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
    fn page_count(ctx, targs, ret, l) { list::page_count(l) as u64 }
    // a range's start, end, or inclusive flag (words 0, 1, 2 of its payload)
    fn range_field(ctx, targs, ret, r, i) { unsafe { *(crate::gc::obj(r).payload().add(i as usize * 8) as *const u64) } }
    fn element_address(ctx, targs, ret, l, i) { list::addr(l, i as usize) as u64 }
    fn list_page(ctx, targs, ret, l, i) {
        let len = list::len(l);
        if i as usize >= len {
            crate::task::panic_current(format!("index out of bounds: the length is {} but the index is {}", len, i as i64), String::new());
        }
        // a stable reference: the element's page must not move
        let r = Roots::new();
        r.add(l);
        list::stabilize(l, i as usize);
        list::page(l, i as usize / list::page_cap(l))
    }
    fn list_slot(ctx, targs, ret, l, i) { (i as usize % list::page_cap(l)) as u64 }
    fn page_read(ctx, targs, ret, page, slot) { list::page_read(page, slot as usize, targ(targs, 0).repr()) }
    fn list_to_array(ctx, targs, ret, l) { list::to_array(l, targ(targs, 0)) }

    // ---------------------------------------------------------------- buffers
    fn buffer_new(ctx, targs, ret, cap) { crate::buffer::new(targ(targs, 0), cap as usize) }
    fn buffer_push(ctx, targs, ret, b, v) {
        crate::buffer::push(b, v);
        0
    }
    fn buffer_pop(ctx, targs, ret, b) {
        match crate::buffer::data(b).items.pop() { Some(v) => some(rdesc(ret), v), None => none(rdesc(ret)) }
    }
    fn buffer_get(ctx, targs, ret, b, i) {
        match crate::buffer::data(b).items.get(i as usize) { Some(v) => some(rdesc(ret), v), None => none(rdesc(ret)) }
    }
    fn buffer_set(ctx, targs, ret, b, i, v) {
        let d = crate::buffer::data(b);
        if d.frozen {
            crate::task::panic_current("write to a frozen buffer".into(), crate::task::caller_location());
        }
        if d.items.set(i as usize, v) { gc::barrier(b); 0 }
        else { crate::task::panic_current(format!("index out of bounds: the length is {} but the index is {}", d.items.len(), i as i64), crate::task::caller_location()) }
    }
    fn buffer_len(ctx, targs, ret, b) { crate::buffer::data(b).items.len() as u64 }
    fn buffer_capacity(ctx, targs, ret, b) { crate::buffer::data(b).items.capacity() as u64 }
    fn buffer_address(ctx, targs, ret, b) { crate::buffer::data(b).items.as_ptr() as u64 }
    fn buffer_freeze(ctx, targs, ret, b) { crate::buffer::freeze(b) }
    fn bytes_from_buffer(ctx, targs, ret, b) {
        let d = crate::buffer::data(b);
        let n = d.items.len();
        // packed byte storage: copy the whole run, not one word per element
        let v = if n == 0 {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(d.items.as_ptr(), n) }.to_vec()
        };
        // the buffer was consumed: its storage is released now
        d.items = crate::buffer::Items::new(d.elem.repr(), 0);
        d.frozen = true;
        crate::string::new_bytes(&v)
    }
    fn bytes_from_list(ctx, targs, ret, l) {
        let n = list::len(l);
        let mut v = vec![0u8; n];
        list::copy_into(l, v.as_mut_ptr());
        crate::string::new_bytes(&v)
    }
    fn buffer_to_list(ctx, targs, ret, b) {
        let r = Roots::new();
        r.add(b);
        let out = r.add(list::new(rdesc(ret)));
        let n = crate::buffer::data(b).items.len();
        if n > 0 {
            list::push_raw(out, crate::buffer::data(b).items.as_ptr(), n);
        }
        out
    }

    fn buffer_to_bytes(ctx, targs, ret, b) {
        let items = &crate::buffer::data(b).items;
        let n = items.len();
        let bytes = if n == 0 {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(items.as_ptr(), n) }.to_vec()
        };
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
        // one contiguous (frozen) array per page, each bulk-copied
        let r = Roots::new();
        r.add(l);
        let out = r.add(list::new(rdesc(ret)));
        let elem = targ(targs, 0);
        let (cap, len) = (list::page_cap(l), list::len(l));
        let mut i = 0;
        while i < len {
            let end = (i + cap).min(len);
            let a = r.add(crate::buffer::frozen_from_raw(elem, list::page_elems(l, i / cap), end - i));
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
    fn map_insert(ctx, targs, ret, m, k, v) {
        gc::budget_check(256);
        map::insert(m, k, v)
    }
    // Compiler-only: the map and every trie node are private until its last
    // update. Preserve the ordinary insert's budget check and existing barriers.
    fn map_insert_private(ctx, targs, ret, m, k, v) {
        gc::budget_check(256);
        map::insert_mut(m, k, v);
        m
    }
    fn map_insert_owned(ctx, targs, ret, m, k, v, old_alias_live) {
        gc::budget_check(256);
        map::insert_owned(m, k, v, old_alias_live != 0)
    }
    fn map_freeze(ctx, targs, ret, m) {
        map::freeze(m);
        m
    }
    fn map_remove(ctx, targs, ret, m, k) { map::remove(m, k) }
    fn map_get(ctx, targs, ret, m, k) {
        match map::get(m, k) { Some(v) => some(rdesc(ret), v), None => none(rdesc(ret)) }
    }
    fn map_get_split(ctx, targs, ret, m, k) { split_option(rdesc(ret), map::get(m, k)) }
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
    fn contract_caller(ctx, targs, ret) { new_string(&crate::task::take_contract_site()) }
    fn text_lines(ctx, targs, ret, s) {
        strings_to_list(rdesc(ret), str_of(s).lines())
    }
    fn text_split(ctx, targs, ret, s, sep) {
        let st = str_of(s);
        let sep = str_of(sep);
        // A one-character separator splits by `char`, whose search uses a
        // byte scan instead of the two-way string search; results are the same.
        let mut chars = sep.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => strings_to_list(rdesc(ret), st.split(c)),
            _ => strings_to_list(rdesc(ret), st.split(sep)),
        }
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
    // a count of zero or less repeats nothing
    fn text_repeat(ctx, targs, ret, s, n) { new_string(&str_of(s).repeat((n as i64).max(0) as usize)) }
    fn text_join(ctx, targs, ret, l, sep) {
        let parts: Vec<&str> = list::to_vec(l).into_iter().map(str_of).collect();
        new_string(&parts.join(str_of(sep)))
    }
    fn text_chars(ctx, targs, ret, s) {
        // The count is a cheap second pass over the text; knowing it fills
        // the pages in one go instead of one `push` per character.
        let st = str_of(s);
        let mut chars = st.chars();
        list::fill(rdesc(ret), st.chars().count(), |_| chars.next().unwrap_or('\0') as u64)
    }
    // The character index of the first occurrence of `p`.
    fn text_find(ctx, targs, ret, s, p) {
        let st = str_of(s);
        match st.find(str_of(p)) {
            Some(i) => some(rdesc(ret), st[..i].chars().count() as u64),
            None => none(rdesc(ret)),
        }
    }
    // The character index of the last occurrence of `p`.
    fn text_rfind(ctx, targs, ret, s, p) {
        let st = str_of(s);
        match st.rfind(str_of(p)) {
            Some(i) => some(rdesc(ret), st[..i].chars().count() as u64),
            None => none(rdesc(ret)),
        }
    }
    // Non-overlapping occurrences of `p` (0 for an empty `p`).
    fn text_count(ctx, targs, ret, s, p) {
        let p = str_of(p);
        if p.is_empty() { 0 } else { str_of(s).matches(p).count() as u64 }
    }
    fn text_replace_first(ctx, targs, ret, s, a, b) { new_string(&str_of(s).replacen(str_of(a), str_of(b), 1)) }
    fn text_trim_start(ctx, targs, ret, s) { new_string(str_of(s).trim_start()) }
    fn text_trim_end(ctx, targs, ret, s) { new_string(str_of(s).trim_end()) }
    fn text_split_whitespace(ctx, targs, ret, s) {
        strings_to_list(rdesc(ret), str_of(s).split_whitespace())
    }
    // Remove the indentation every non-blank line shares (spaces and
    // tabs, counted as characters); blank lines become empty.
    fn text_dedent(ctx, targs, ret, s) {
        let st = str_of(s);
        let indent = st
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.len() - l.trim_start_matches([' ', '\t']).len())
            .min()
            .unwrap_or(0);
        let mut out = String::with_capacity(st.len());
        for (i, line) in st.split('\n').enumerate() {
            if i > 0 {
                out.push('\n');
            }
            if !line.trim().is_empty() {
                out.push_str(&line[indent.min(line.len())..]);
            }
        }
        new_string(&out)
    }
    fn text_slice(ctx, targs, ret, s, a, b) {
        // character-indexed slice; returns Option<String> (one pass, no
        // index table)
        let st = str_of(s);
        let Some(range) = crate::string::char_range(st, a as i64, b as i64) else {
            return none(rdesc(ret));
        };
        let w = new_string(&st[range]);
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
        let n = str_of(name).to_string();
        let v = crate::repro::external("env", &n, None, crate::repro::secret_name(&n), || std::env::var(&n).ok());
        match v { Some(v) => { let w = new_string(&v); some(rdesc(ret), w) } None => none(rdesc(ret)) }
    }
    fn fs_read_text(ctx, targs, ret, path) {
        let p = str_of(path).to_string();
        let r = crate::repro::external("read", &p.clone(), None, false, move || {
            crate::task::run_blocking_abandonable(move || std::fs::read_to_string(p)).map_err(|e| crate::intrinsics_io::io_pair(&e))
        });
        match r { Ok(s) => { let w = new_string(&s); io_ok(rdesc(ret), w) } Err((k, m)) => crate::intrinsics_io::io_err_msg(rdesc(ret), &k, &m) }
    }
    fn fs_write_text(ctx, targs, ret, path, text) {
        let (p, t) = (str_of(path).to_string(), str_of(text).to_string());
        let d = Some(crate::repro::digest(t.as_bytes()));
        let r = crate::repro::external("write", &p.clone(), d, false, move || {
            crate::task::run_blocking_abandonable(move || std::fs::write(p, t)).map_err(|e| crate::intrinsics_io::io_pair(&e))
        });
        crate::intrinsics_io::io_result(rdesc(ret), r.map(|()| 0))
    }
    fn fs_exists(ctx, targs, ret, path) {
        let p = str_of(path).to_string();
        bool_w(crate::repro::external("exists", &p.clone(), None, false, move || std::path::Path::new(&p).exists()))
    }
    fn fs_root(ctx, targs, ret, path) {
        let p = str_of(path).to_string();
        let r = crate::repro::external("root", &p.clone(), None, false, move || match std::fs::canonicalize(&p) {
            Ok(c) if c.is_dir() => Ok(c.to_string_lossy().into_owned()),
            Ok(_) => Err(("io/invalid_input".to_string(), format!("{p} is not a directory"))),
            Err(e) => Err(crate::intrinsics_io::io_pair(&e)),
        });
        match r {
            Ok(c) => { let w = new_string(&c); io_ok(rdesc(ret), w) }
            Err((k, m)) => crate::intrinsics_io::io_err_msg(rdesc(ret), &k, &m),
        }
    }
    fn fs_within(ctx, targs, ret, root, path) {
        let (r, p) = (str_of(root).to_string(), str_of(path).to_string());
        let key = format!("{r} {p}");
        let res = crate::repro::external("within", &key, None, false, move || {
            resolve_within(&r, &p).map(|x| x.to_string_lossy().into_owned())
        });
        match res {
            Ok(p) => { let w = new_string(&p); io_ok(rdesc(ret), w) }
            Err(m) => crate::intrinsics_io::io_err_msg(rdesc(ret), "io/permission_denied", &m),
        }
    }
    fn fs_list(ctx, targs, ret, path) {
        let p = str_of(path).to_string();
        let r = crate::repro::external("list", &p.clone(), None, false, move || {
            crate::task::run_blocking_abandonable(move || -> std::io::Result<Vec<String>> {
                let mut names: Vec<String> = std::fs::read_dir(p)?.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
                names.sort();
                Ok(names)
            })
            .map_err(|e| crate::intrinsics_io::io_pair(&e))
        });
        match r {
            Ok(names) => {
                let list_desc = match &rdesc(ret).kind { Kind::Enum { .. } => rdesc(ret).variant(0).unwrap().fields[0].desc, _ => rdesc(ret) };
                let w = strings_to_list(list_desc, names);
                let g = Roots::new();
                g.add(w);
                io_ok(rdesc(ret), w)
            }
            Err((k, m)) => crate::intrinsics_io::io_err_msg(rdesc(ret), &k, &m),
        }
    }
}

/// `path` inside the directory tree `root` (a canonical directory): no
/// absolute paths or `..`, and the deepest part that exists must resolve
/// (following symlinks) inside the tree. The result is that canonical
/// part with the rest appended.
fn resolve_within(root: &str, path: &str) -> Result<std::path::PathBuf, String> {
    use std::path::{Component, Path};
    let rel = Path::new(path);
    if rel.is_absolute() {
        return Err(format!("`{path}` is absolute; a directory capability takes paths inside it"));
    }
    if rel.components().any(|c| !matches!(c, Component::Normal(_) | Component::CurDir)) {
        return Err(format!("`{path}` leaves the directory"));
    }
    let root = Path::new(root);
    let full = root.join(rel);
    let mut existing = full.clone();
    let mut rest: Vec<std::ffi::OsString> = vec![];
    while std::fs::symlink_metadata(&existing).is_err() {
        match (existing.file_name().map(|n| n.to_os_string()), existing.parent()) {
            (Some(n), Some(p)) => {
                rest.push(n);
                existing = p.to_path_buf();
            }
            _ => return Err(format!("`{path}` cannot be resolved")),
        }
    }
    let canon = std::fs::canonicalize(&existing).map_err(|e| format!("`{path}`: {e}"))?;
    if !canon.starts_with(root) {
        return Err(format!("`{path}` resolves outside the directory"));
    }
    let mut out = canon;
    for n in rest.into_iter().rev() {
        out.push(n);
    }
    Ok(out)
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
        Kind::List { elem } => {
            format!("{{\"type\":\"array\",\"items\":{}}}", json_schema(elem, depth + 1))
        }
        Kind::Set { elem } => {
            format!("{{\"type\":\"array\",\"items\":{},\"uniqueItems\":true}}", json_schema(elem, depth + 1))
        }
        Kind::Opaque { .. } if crate::buffer::array_elem(d).is_some() => {
            let elem = crate::buffer::array_elem(d).expect("array");
            format!("{{\"type\":\"array\",\"items\":{}}}", json_schema(elem, depth + 1))
        }
        Kind::Map { val, .. } => {
            format!("{{\"type\":\"object\",\"additionalProperties\":{}}}", json_schema(val, depth + 1))
        }
        Kind::Record { .. } if d.name == crate::codec::SECRET => {
            "{\"type\":\"string\",\"writeOnly\":true}".into()
        }
        Kind::Record { fields, flat: true, .. } => json_schema(fields[0].desc, depth),
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
                .filter(|f| !crate::codec::omittable(f.desc))
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
