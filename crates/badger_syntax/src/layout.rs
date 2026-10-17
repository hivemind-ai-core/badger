//! Layout pass: turns statement-terminating newlines into explicit separators
//! before the pest grammar sees the text.
//!
//! The rules follow `spec/01-syntax.md`: a newline ends a statement when the
//! previous token can end one, unless it occurs inside parentheses/brackets,
//! or the next line continues with `|>` or `else`. A nested `{` restores
//! statement boundaries even inside parentheses.
//!
//! The pass preserves byte offsets exactly: a terminating newline is replaced
//! by [`NL`] (one byte) and comment bytes are replaced by spaces, so spans
//! computed on the rewritten text are valid for the original source.

// decision ladders list each case separately for readability
#![allow(clippy::if_same_then_else)]

/// Inserted statement separator (ASCII "record separator").
pub const NL: u8 = 0x1E;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tok {
    /// Token that may end a statement (identifier, literal, closer, `?`).
    Ends,
    /// `|>` or `else`: continues the previous line.
    Continues,
    Other,
}

pub fn layout(src: &str) -> String {
    layout_checked(src).0
}

/// [`layout`], also returning where an unterminated block comment opens.
pub fn layout_checked(src: &str) -> (String, Option<usize>) {
    let b = src.as_bytes();
    let mut unterminated = None;
    let mut out = b.to_vec();
    let mut stack: Vec<u8> = Vec::new();
    let mut last_ends = false;
    let mut pending: Option<usize> = None;
    let mut i = 0;

    while i < b.len() {
        let c = b[i];
        // whitespace & newlines
        if c == b'\n' {
            if last_ends && matches!(stack.last(), None | Some(b'{')) && pending.is_none() {
                pending = Some(i);
            }
            last_ends = false;
            i += 1;
            continue;
        }
        if c == b' ' || c == b'\t' || c == b'\r' {
            i += 1;
            continue;
        }
        // comments → spaces (newlines inside block comments are kept)
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                out[i] = b' ';
                i += 1;
            }
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'*') {
            let open = i;
            let mut depth = 0usize;
            while i < b.len() {
                if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
                    depth += 1;
                    out[i] = b' ';
                    out[i + 1] = b' ';
                    i += 2;
                } else if b[i] == b'*' && b.get(i + 1) == Some(&b'/') {
                    depth -= 1;
                    out[i] = b' ';
                    out[i + 1] = b' ';
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    if b[i] != b'\n' {
                        out[i] = b' ';
                    }
                    i += 1;
                }
            }
            if depth > 0 {
                unterminated.get_or_insert(open);
            }
            continue;
        }

        // a significant token starts here
        let (kind, end) = scan_token(b, i);
        if let Some(p) = pending.take()
            && kind != Tok::Continues
        {
            out[p] = NL;
        }
        match c {
            b'(' | b'[' | b'{' => stack.push(c),
            b')' | b']' | b'}' => {
                stack.pop();
            }
            _ => {}
        }
        last_ends = kind == Tok::Ends;
        i = end;
    }
    if let Some(p) = pending {
        out[p] = NL;
    }
    // Comment bytes were replaced by ASCII spaces only; the result is UTF-8.
    (String::from_utf8(out).expect("layout preserves UTF-8"), unterminated)
}

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// Scan one token starting at `i`, returning its classification and end.
fn scan_token(b: &[u8], i: usize) -> (Tok, usize) {
    let c = b[i];
    if c.is_ascii_alphabetic() || c == b'_' {
        let mut j = i;
        while j < b.len() && is_ident(b[j]) {
            j += 1;
        }
        let word = &b[i..j];
        let kind = match word {
            b"else" => Tok::Continues,
            b"true" | b"false" | b"break" | b"continue" | b"return" => Tok::Ends,
            // other keywords never end a statement
            b"import" | b"as" | b"pub" | b"type" | b"struct" | b"enum" | b"protocol" | b"impl" | b"for"
            | b"where" | b"fn" | b"once" | b"let" | b"var" | b"const" | b"if" | b"match" | b"while"
            | b"loop" | b"in" | b"use" | b"unsafe" | b"extern" | b"dyn" | b"mut" => Tok::Other,
            _ => Tok::Ends,
        };
        return (kind, j);
    }
    if c.is_ascii_digit() {
        let mut j = i;
        while j < b.len() {
            let d = b[j];
            if is_ident(d) {
                j += 1;
            } else if d == b'.' && b.get(j + 1).is_some_and(|n| n.is_ascii_digit()) {
                j += 1;
            } else if (d == b'+' || d == b'-') && matches!(b[j - 1], b'e' | b'E') && !hexish(b, i) {
                j += 1;
            } else {
                break;
            }
        }
        return (Tok::Ends, j);
    }
    match c {
        b'"' => (Tok::Ends, skip_string(b, i)),
        b'$' if b.get(i + 1) == Some(&b'"') => (Tok::Ends, skip_string(b, i + 1)),
        b'\'' => {
            let mut j = i + 1;
            while j < b.len() && b[j] != b'\'' {
                if b[j] == b'\\' {
                    j += 1;
                }
                j += 1;
            }
            (Tok::Ends, (j + 1).min(b.len()))
        }
        b':' if b.get(i + 1).is_some_and(|n| n.is_ascii_alphabetic()) => {
            // symbol literal :name or :ns/name
            let mut j = i + 1;
            while j < b.len() && (is_ident(b[j]) || b[j] == b'/') {
                j += 1;
            }
            (Tok::Ends, j)
        }
        b')' | b']' | b'}' | b'?' => (Tok::Ends, i + 1),
        b'|' if b.get(i + 1) == Some(&b'>') => (Tok::Continues, i + 2),
        _ => (Tok::Other, i + 1),
    }
}

fn hexish(b: &[u8], i: usize) -> bool {
    b.get(i) == Some(&b'0') && matches!(b.get(i + 1), Some(b'x' | b'X'))
}

/// Skip a string literal starting at the opening quote `b[i] == '"'`.
/// Handles triple-quoted raw strings and interpolation holes (which may
/// themselves contain strings).
fn skip_string(b: &[u8], i: usize) -> usize {
    let interp = i > 0 && b[i - 1] == b'$';
    if b[i..].starts_with(b"\"\"\"") {
        let mut j = i + 3;
        while j < b.len() {
            if b[j..].starts_with(b"\"\"\"") {
                return j + 3;
            }
            if interp && b[j] == b'{' {
                if b.get(j + 1) == Some(&b'{') {
                    j += 2;
                    continue;
                }
                j = skip_hole(b, j);
                continue;
            }
            j += 1;
        }
        return b.len();
    }
    let mut j = i + 1;
    while j < b.len() {
        match b[j] {
            b'\\' => j += 2,
            b'"' => return j + 1,
            b'\n' => return j,
            b'{' if interp => {
                if b.get(j + 1) == Some(&b'{') {
                    j += 2;
                } else {
                    j = skip_hole(b, j);
                }
            }
            _ => j += 1,
        }
    }
    b.len()
}

/// Skip an interpolation hole `{ ... }` starting at `b[i] == '{'`.
fn skip_hole(b: &[u8], i: usize) -> usize {
    let mut depth = 0usize;
    let mut j = i;
    while j < b.len() {
        match b[j] {
            b'{' => {
                depth += 1;
                j += 1;
            }
            b'}' => {
                depth -= 1;
                j += 1;
                if depth == 0 {
                    return j;
                }
            }
            b'"' => j = skip_string(b, j),
            b'\n' => return j,
            _ => j += 1,
        }
    }
    b.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn show(s: &str) -> String {
        layout(s).replace(NL as char, ";")
    }

    #[test]
    fn inserts_after_statement_end() {
        assert_eq!(show("let a = 1\nlet b = 2\n"), "let a = 1;let b = 2;");
    }

    #[test]
    fn no_insert_inside_parens_or_after_operator() {
        assert_eq!(show("f(\n a,\n b\n)\n"), "f(\n a,\n b\n);");
        assert_eq!(show("a +\n b\n"), "a +\n b;");
    }

    #[test]
    fn pipe_and_else_continue() {
        assert_eq!(show("x\n  |> f()\n"), "x\n  |> f();");
        assert_eq!(show("if a { b }\nelse { c }\n"), "if a { b }\nelse { c };");
    }

    #[test]
    fn braces_restore_inside_parens() {
        assert_eq!(show("f(fn() {\n a()\n b()\n})\n"), "f(fn() {\n a(); b();});");
    }

    #[test]
    fn comments_become_spaces_and_strings_are_opaque() {
        assert_eq!(show("a // c\nb"), "a     ;b");
        assert_eq!(show("$\"x {f(\"}\")} y\"\nz"), "$\"x {f(\"}\")} y\";z");
    }
}
