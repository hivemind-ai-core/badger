//! Canonical spacing within a line: one space around binary operators and
//! after commas, semicolons, and colons; none inside parentheses, brackets,
//! and generic arguments, before `,` `;` `?` `.` or a call's `(`; one space
//! inside non-empty single-line braces. A trailing comma before `]` or `}` on
//! the same line is dropped (one in parentheses may make a tuple). Strings, characters, symbols, numbers, and
//! comments are copied verbatim, and a line that opens a multi-line string or
//! comment is left unchanged, so spacing never changes meaning.

// decision ladders list each case separately for readability
#![allow(clippy::if_same_then_else)]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum K {
    Ident,
    Keyword,
    Number,
    Str,
    Symbol,
    Comment,
    Op,
    Open,
    Close,
    Comma,
    Semi,
    Colon,
    Dot,
    DotDot,
    Question,
    Other,
}

#[derive(Clone, Debug)]
struct Tok {
    k: K,
    text: String,
    /// Whitespace preceded the token in the source.
    spaced: bool,
    /// `<`/`>` delimiting generic arguments.
    generic: bool,
}

const KEYWORDS: &[&str] = &[
    "pub", "once", "mut", "import", "as", "type", "struct", "enum", "protocol", "impl", "for", "where", "fn",
    "if", "else", "match", "while", "loop", "in", "use", "unsafe", "extern", "dyn", "derive", "repr",
    "system", "pipeline", "map", "set", "ref", "return", "break", "continue", "let", "var", "const",
];

/// Keywords after which `(` and `[` start a parenthesized expression.
const CONTROL: &[&str] = &[
    "if", "while", "match", "return", "in", "for", "else", "use", "loop", "break", "where", "let", "var",
    "const",
];

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The index just past the string literal starting at byte `i` of `line`
/// (at `"`, or at `$` of an interpolated string), if it ends on the line.
pub fn string_end(line: &str, i: usize) -> Option<usize> {
    let prefix = line.get(..i)?;
    let start = prefix.chars().count();
    let b: Vec<char> = line.chars().collect();
    let end = if b.get(start) == Some(&'$') {
        scan_string(&b, start + 1, true)?
    } else {
        scan_string(&b, start, false)?
    };
    Some(b[..end].iter().collect::<String>().len())
}

/// Scan a (possibly interpolated) string starting at `i` (at the quote);
/// returns the index after it, or `None` if it does not end on this line.
fn scan_string(b: &[char], mut i: usize, interpolated: bool) -> Option<usize> {
    i += 1;
    while i < b.len() {
        match b[i] {
            '\\' => i += 2,
            '"' => return Some(i + 1),
            '{' if interpolated && b.get(i + 1) == Some(&'{') => i += 2,
            '}' if interpolated && b.get(i + 1) == Some(&'}') => i += 2,
            '{' if interpolated => {
                // an interpolated expression, possibly with nested strings
                let mut depth = 1;
                i += 1;
                while i < b.len() && depth > 0 {
                    match b[i] {
                        '"' => i = scan_string(b, i, false)?,
                        '$' if b.get(i + 1) == Some(&'"') => i = scan_string(b, i + 1, true)?,
                        '{' => {
                            depth += 1;
                            i += 1
                        }
                        '}' => {
                            depth -= 1;
                            i += 1
                        }
                        _ => i += 1,
                    }
                }
            }
            _ => i += 1,
        }
    }
    None
}

fn lex(line: &str) -> Option<Vec<Tok>> {
    let b: Vec<char> = line.chars().collect();
    let mut out: Vec<Tok> = vec![];
    let mut i = 0;
    let mut spaced = false;
    let push = |out: &mut Vec<Tok>, k: K, text: String, spaced: &mut bool| {
        out.push(Tok { k, text, spaced: *spaced, generic: false });
        *spaced = false;
    };
    while i < b.len() {
        let c = b[i];
        let next = b.get(i + 1).copied();
        if c.is_whitespace() {
            spaced = true;
            i += 1;
            continue;
        }
        // prefix tests on the characters (building the rest of the line as a
        // string for each token made long lines quadratic)
        let at = |s: &str| s.chars().enumerate().all(|(k, ch)| b.get(i + k) == Some(&ch));
        if at("//") {
            push(&mut out, K::Comment, b[i..].iter().collect(), &mut spaced);
            break;
        }
        if at("/*") {
            // an inline block comment (nested); multi-line ones are left alone
            let mut depth = 0;
            let mut j = i;
            loop {
                if j + 1 >= b.len() {
                    return None;
                }
                if b[j] == '/' && b[j + 1] == '*' {
                    depth += 1;
                    j += 2;
                } else if b[j] == '*' && b[j + 1] == '/' {
                    depth -= 1;
                    j += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    j += 1;
                }
            }
            push(&mut out, K::Comment, b[i..j].iter().collect(), &mut spaced);
            i = j;
            continue;
        }
        if at("\"\"\"") || at("$\"\"\"") {
            // a raw string closed on this line is copied verbatim; one that
            // continues leaves the line unchanged
            let open = if c == '$' { 4 } else { 3 };
            let tail: String = b[i + open..].iter().collect();
            let close = tail.find("\"\"\"")?;
            let end = i + open + tail[..close].chars().count() + 3;
            push(&mut out, K::Str, b[i..end].iter().collect(), &mut spaced);
            i = end;
            continue;
        }
        if c == '"' || (c == '$' && next == Some('"')) {
            let start = i;
            let end = if c == '$' { scan_string(&b, i + 1, true)? } else { scan_string(&b, i, false)? };
            push(&mut out, K::Str, b[start..end].iter().collect(), &mut spaced);
            i = end;
            continue;
        }
        if c == '\'' {
            let mut j = i + 1;
            while j < b.len() && b[j] != '\'' {
                if b[j] == '\\' {
                    j += 1;
                }
                j += 1;
            }
            if j >= b.len() {
                return None;
            }
            push(&mut out, K::Str, b[i..=j].iter().collect(), &mut spaced);
            i = j + 1;
            continue;
        }
        if c.is_ascii_digit() {
            let mut j = i;
            while j < b.len()
                && (is_ident_char(b[j]) || (b[j] == '.' && b.get(j + 1).is_some_and(|d| d.is_ascii_digit())))
            {
                j += 1;
            }
            push(&mut out, K::Number, b[i..j].iter().collect(), &mut spaced);
            i = j;
            continue;
        }
        if is_ident_start(c) {
            let mut j = i;
            while j < b.len() && is_ident_char(b[j]) {
                j += 1;
            }
            let w: String = b[i..j].iter().collect();
            let k = if KEYWORDS.contains(&w.as_str()) { K::Keyword } else { K::Ident };
            push(&mut out, k, w, &mut spaced);
            i = j;
            continue;
        }
        if c == '#' && next.is_some_and(|n| n.is_ascii_alphabetic()) {
            // a symbol literal: `#name` or `#ns/name`
            let mut j = i + 1;
            while j < b.len()
                && (is_ident_char(b[j])
                    || (b[j] == '/' && b.get(j + 1).is_some_and(|d| d.is_ascii_alphabetic())))
            {
                j += 1;
            }
            push(&mut out, K::Symbol, b[i..j].iter().collect(), &mut spaced);
            i = j;
            continue;
        }
        let two: String = b[i..(i + 2).min(b.len())].iter().collect();
        let three: String = b[i..(i + 3).min(b.len())].iter().collect();
        if three == "..=" {
            push(&mut out, K::DotDot, three, &mut spaced);
            i += 3;
            continue;
        }
        if two == ".." {
            push(&mut out, K::DotDot, two, &mut spaced);
            i += 2;
            continue;
        }
        if ["->", "=>", "|>", "<-", "==", "!=", "<=", ">=", "&&", "||", "+=", "-=", "*=", "/=", "%="]
            .contains(&two.as_str())
        {
            push(&mut out, K::Op, two, &mut spaced);
            i += 2;
            continue;
        }
        let k = match c {
            '(' | '[' | '{' => K::Open,
            ')' | ']' | '}' => K::Close,
            ',' => K::Comma,
            ';' => K::Semi,
            ':' => K::Colon,
            '.' => K::Dot,
            '?' => K::Question,
            '+' | '-' | '*' | '/' | '%' | '=' | '!' | '&' | '|' | '^' | '<' | '>' => K::Op,
            _ => K::Other,
        };
        push(&mut out, k, c.to_string(), &mut spaced);
        i += 1;
    }
    Some(out)
}

/// Mark `<`/`>` pairs that delimit generic arguments: `<` written directly
/// after a name (or `impl`/`for`), with a matching `>` on the line and only
/// type syntax between.
fn mark_generics(ts: &mut [Tok]) {
    let mut i = 0;
    while i < ts.len() {
        let t = &ts[i];
        let after_name = i > 0
            && !t.spaced
            && t.text == "<"
            && (ts[i - 1].k == K::Ident
                || (ts[i - 1].k == K::Keyword && !CONTROL.contains(&ts[i - 1].text.as_str())
                    || ts[i - 1].text == "for"));
        if !after_name {
            i += 1;
            continue;
        }
        let mut depth = 0;
        let mut close = None;
        for (j, u) in ts.iter().enumerate().skip(i) {
            let typeish = match u.k {
                K::Ident | K::Comma | K::Colon | K::Dot | K::DotDot | K::Open | K::Close | K::Symbol => true,
                K::Keyword => !CONTROL.contains(&u.text.as_str()) || u.text == "for",
                K::Op => matches!(u.text.as_str(), "<" | ">" | "->" | "&" | "!" | "+"),
                _ => false,
            };
            if !typeish {
                break;
            }
            if u.text == "<" {
                depth += 1;
            } else if u.text == ">" {
                depth -= 1;
                if depth == 0 {
                    close = Some(j);
                    break;
                }
            }
        }
        if let Some(j) = close {
            // mark every angle pair inside as generic
            for u in ts[i..=j].iter_mut() {
                if u.text == "<" || u.text == ">" {
                    u.generic = true;
                }
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
}

/// A token that ends an operand (so `-`, `&`, `!` after it are binary, and
/// `(`/`[` after it are a call or index).
fn ends_operand(t: &Tok) -> bool {
    match t.k {
        K::Ident | K::Number | K::Str | K::Symbol | K::Question => true,
        K::Close => true,
        K::Keyword => {
            !CONTROL.contains(&t.text.as_str())
                && !matches!(
                    t.text.as_str(),
                    "pub"
                        | "mut"
                        | "import"
                        | "type"
                        | "struct"
                        | "enum"
                        | "protocol"
                        | "impl"
                        | "extern"
                        | "dyn"
                        | "repr"
                        | "once"
                        | "as"
                )
        }
        K::Op => t.generic && t.text == ">",
        _ => false,
    }
}

/// Rewrite the spacing of one line (without its indentation). The result
/// is re-lexed: if its tokens differ from the input's (apart from a dropped
/// trailing comma), the line is kept as written.
pub fn space_line(line: &str) -> String {
    let Some((ts, out)) = respace(line) else { return line.to_string() };
    let same = lex(&out).is_some_and(|us| {
        let a: Vec<&str> = ts.iter().map(|t| t.text.as_str()).collect();
        let b: Vec<&str> = us.iter().map(|t| t.text.as_str()).collect();
        // compare after re-merging shift operators the lexer splits
        a.concat() == b.concat()
            && b.len() >= a.len()
            && b.len() <= a.len() + a.iter().filter(|t| **t == "<<" || **t == ">>").count()
    });
    if same { out } else { line.to_string() }
}

fn respace(line: &str) -> Option<(Vec<Tok>, String)> {
    let mut ts = lex(line)?;
    mark_generics(&mut ts);
    // drop a trailing comma before a closer on the same line
    let mut k = 0;
    while k + 1 < ts.len() {
        // (not in parentheses: `(item,)` is a one-element tuple)
        if ts[k].k == K::Comma && ts[k + 1].k == K::Close && ts[k + 1].text != ")" {
            ts.remove(k);
        } else {
            k += 1;
        }
    }
    // shift operators split by the lexer
    let mut k = 0;
    while k + 1 < ts.len() {
        let pair = (ts[k].text.as_str(), ts[k + 1].text.as_str());
        if !ts[k].generic
            && !ts[k + 1].generic
            && !ts[k + 1].spaced
            && (pair == ("<", "<") || pair == (">", ">"))
        {
            let t = format!("{}{}", pair.0, pair.1);
            ts[k].text = t;
            ts.remove(k + 1);
        } else {
            k += 1;
        }
    }
    let n = ts.len();
    // operator roles: unary prefix (`-x`, `!x`, `&x`) and effect markers (`!{`)
    let mut unary = vec![false; n];
    let mut effect = vec![false; n];
    let mut generic_depth = 0usize;
    for i in 0..n {
        if ts[i].generic {
            if ts[i].text == "<" {
                generic_depth += 1;
            } else {
                generic_depth = generic_depth.saturating_sub(1);
            }
        }
        if ts[i].k == K::Op && matches!(ts[i].text.as_str(), "-" | "!" | "&") {
            let prev_operand = i > 0 && ends_operand(&ts[i - 1]);
            // an effect argument among generic arguments (`Seq<T, !{Io}>`)
            let effect_arg = ts[i].text == "!"
                && generic_depth > 0
                && i > 0
                && (ts[i - 1].k == K::Comma || (ts[i - 1].generic && ts[i - 1].text == "<"));
            // `!` is never binary: after an operand it marks an effect
            // (`-> T !{Io}`, `-> T !Eff`)
            if ts[i].text == "!" && (prev_operand || effect_arg) {
                effect[i] = true;
            } else if !prev_operand {
                unary[i] = true;
            }
        }
    }
    // braces that hold an import list (`std.system.{component}`) stay tight
    let mut tight = vec![false; n];
    let mut stack: Vec<(usize, bool)> = vec![];
    for i in 0..n {
        if ts[i].k == K::Open {
            // import lists and effect rows (`!{Io}`) are tight
            let t = ts[i].text == "{" && i > 0 && (ts[i - 1].k == K::Dot || effect[i - 1]);
            stack.push((i, t));
            tight[i] = t;
        } else if ts[i].k == K::Close
            && let Some((_, t)) = stack.pop()
        {
            tight[i] = t;
        }
    }
    let mut out = String::new();
    for i in 0..n {
        let t = &ts[i];
        if i == 0 {
            out.push_str(&t.text);
            continue;
        }
        let p = &ts[i - 1];
        let space = if t.k == K::Comment {
            true
        } else if p.k == K::Open {
            // `{ a }` (unless empty or tight); `(a`, `[a`
            p.text == "{" && t.text != "}" && !tight[i - 1]
        } else if t.k == K::Close {
            t.text == "}" && p.text != "{" && !tight[i]
        } else if matches!(t.k, K::Comma | K::Semi | K::Colon | K::Question) {
            false
        } else if matches!(p.k, K::Comma | K::Semi | K::Colon) {
            true
        } else if p.text.ends_with('.')
            && (t.text.starts_with('.') || (p.text == ".." && t.text.starts_with('=')))
        {
            // tokens that would lex differently joined (`. ..`, `.. =`)
            true
        } else if p.k == K::Dot {
            false
        } else if t.k == K::Dot {
            // member access is tight; a leading-dot variant is spaced like an operand
            !ends_operand(p) && !(p.k == K::Op && unary[i - 1])
        } else if t.k == K::DotDot {
            // a range is tight; a spread/rest starts an operand
            !ends_operand(p) && p.k != K::Open
        } else if p.k == K::DotDot {
            false
        } else if t.k == K::Op && t.generic {
            false
        } else if p.k == K::Op && p.generic {
            // after a generic `<` tight; after `>` like after a name
            if p.text == "<" { false } else { !matches!(t.k, K::Open) || t.text == "{" }
        } else if p.k == K::Op && (unary[i - 1] || effect[i - 1]) {
            false
        } else if t.k == K::Op && effect[i] {
            true
        } else if t.k == K::Op && unary[i] {
            true
        } else if t.k == K::Op || p.k == K::Op {
            true
        } else if t.k == K::Open && t.text != "{" {
            // calls and indexing are tight; grouping after a keyword is spaced
            !ends_operand(p)
        } else if t.k == K::Open {
            true
        } else if t.k == K::Other || p.k == K::Other {
            t.spaced
        } else {
            true
        };
        // never join tokens that would lex as others (`! =>` is not `!=>`)
        let space = space || (p.k == K::Op && t.k == K::Op && !lexes_apart(&p.text, &t.text));
        if space {
            out.push(' ');
        }
        out.push_str(&t.text);
    }
    Some((ts, out))
}

/// Whether `a` and `b` written together still lex as `a` then `b`.
fn lexes_apart(a: &str, b: &str) -> bool {
    match lex(&format!("{a}{b}")) {
        Some(ts) => ts.len() == 2 && ts[0].text == a && ts[1].text == b,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::space_line;

    #[test]
    fn canonical_spacing() {
        let cases = [
            ("fn   f( a:Int,b :Int )->Int{a+b}", "fn f(a: Int, b: Int) -> Int { a + b }"),
            ("let x=f(1,2);let y = [1,2,3,]", "let x = f(1, 2); let y = [1, 2, 3]"),
            ("let r={a:1,b:2}", "let r = { a: 1, b: 2 }"),
            (
                "if x>2{io.println(x)?}else{io.println(\"small\")?}",
                "if x > 2 { io.println(x)? } else { io.println(\"small\")? }",
            ),
            ("io.println(r.a) // trailing comment", "io.println(r.a) // trailing comment"),
            (
                "pub fn main()->Result<Unit,io.Error>!{Io,Wait}{",
                "pub fn main() -> Result<Unit, io.Error> !{Io, Wait} {",
            ),
            ("let m: Map<String, List<Int>> = map {}", "let m: Map<String, List<Int>> = map {}"),
            ("for i in 0..n { s = s + -i }", "for i in 0..n { s = s + -i }"),
            ("let p2 = {..p,x:1}", "let p2 = { ..p, x: 1 }"),
            (
                "match r { .ok(v)=>v, .err(e)=>{ return err(e) } }",
                "match r { .ok(v) => v, .err(e) => { return err(e) } }",
            ),
            ("let b = list.builder<Event>()", "let b = list.builder<Event>()"),
            ("let s = $\"{a}-{ f(\"x\") }\"", "let s = $\"{a}-{ f(\"x\") }\""),
            ("component(p,{v:ref(#base)+1})", "component(p, { v: ref(#base) + 1 })"),
            (
                "impl<T> Display for Box<T> where T: Display {",
                "impl<T> Display for Box<T> where T: Display {",
            ),
            (
                "fn f(self: &mut Self, x: &Int) -> Bool { !self.done && x<y }",
                "fn f(self: &mut Self, x: &Int) -> Bool { !self.done && x < y }",
            ),
            ("import std.system.{component, ref}", "import std.system.{component, ref}"),
            ("let x = a<<2 | b>>1", "let x = a << 2 | b >> 1"),
            // an effect marker before `=>` stays apart from it
            (".real(f)! => x", ".real(f) ! => x"),
            ("use tx<-sqlite.transaction(db)", "use tx <- sqlite.transaction(db)"),
            ("body: for<S: Scope> fn(Tx<S>) -> R", "body: for<S: Scope> fn(Tx<S>) -> R"),
            ("xs[i]=xs[i-1]*2", "xs[i] = xs[i - 1] * 2"),
            ("pub fn map<T, U>(items: Seq<T>) -> Seq<U> {", "pub fn map<T, U>(items: Seq<T>) -> Seq<U> {"),
            ("} derive(Display, Eq)", "} derive(Display, Eq)"),
            (
                "pub fn main() -> Result<Unit, system.Error> {",
                "pub fn main() -> Result<Unit, system.Error> {",
            ),
            ("let one = (item,)", "let one = (item,)"),
            ("f(a, b,)", "f(a, b,)"),
            (
                "fn spawn<T, Eff>(f: fn() -> T !Eff) -> Task<T> where fn() -> T !Eff: Transfer {",
                "fn spawn<T, Eff>(f: fn() -> T !Eff) -> Task<T> where fn() -> T !Eff: Transfer {",
            ),
            ("let s = \"\"\"", "let s = \"\"\""),
            // joined, these would lex as other tokens
            ("import std. ..R", "import std. ..R"),
            ("a.. .b", "a.. .b"),
        ];
        for (src, want) in cases {
            assert_eq!(space_line(src), want, "{src}");
            assert_eq!(space_line(want), want, "idempotent: {want}");
        }
    }
}
