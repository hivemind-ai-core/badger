//! Lexer: source text to a flat buffer of `(Tok, Span)`.
//!
//! [logos] scans the ordinary tokens. The driver ([`lex`]) adds what a
//! regular scanner cannot: interpolated strings, whose `{ ... }` holes are
//! lexed as ordinary tokens between [`Tok::HoleStart`] and
//! [`Tok::HoleEnd`]; and numeric tuple fields (`t.0.1`), which would
//! otherwise read as a float. [`layout`] then turns the newlines that end
//! statements into [`Tok::Sep`] and drops the others (the rules of
//! `spec/01-syntax.md`).
//!
//! Words are one token kind: reserved words, contextual keywords (`system`,
//! `map`, ...), and names are told apart by the parser, so a keyword can
//! still name a field (`s.match()`, `{ type: t }`). Symbols (`:name`) and the
//! two-character operators `>>` and `>=` are likewise recognized by the
//! parser from adjacent tokens, so `Map<K, List<V>>` closes two generic
//! argument lists and `x: Int` never reads as a symbol.

// decision ladders list each case separately for readability
#![allow(clippy::if_same_then_else)]

use chumsky::span::SimpleSpan;
use logos::{FilterResult, Logos};

pub type Span = SimpleSpan<usize>;

/// The span `start..end`.
pub fn sp(start: usize, end: usize) -> Span {
    (start..end).into()
}

#[derive(Logos, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[logos(skip r"[ \t\r\x0C]+")]
pub enum Tok<'a> {
    /// An identifier-shaped word: a name, a reserved word, or `_`.
    #[regex(r"[A-Za-z_][A-Za-z0-9_]*", |lex| lex.slice())]
    Word(&'a str),
    /// A numeric literal, validated by the driver (see [`Tok::Int`]).
    #[regex(r"[0-9]", number)]
    Number(&'a str),
    /// Validated numeric literals (the driver classifies [`Tok::Number`]).
    Int(&'a str),
    Float(&'a str),
    Duration(&'a str),
    /// `"..."`, with the text between the quotes (escapes not processed).
    #[token("\"", string)]
    Str(&'a str),
    /// `"""..."""`, with the text between the quotes.
    #[token("\"\"\"", raw_string)]
    RawStr(&'a str),
    /// `'c'`, with the text between the quotes (escapes not processed).
    #[token("'", char_lit)]
    Char(&'a str),
    /// `$"` / `$"""`: the driver scans the rest of the string.
    #[token("$\"")]
    InterpOpen,
    #[token("$\"\"\"")]
    InterpRawOpen,
    /// An interpolated string: `InterpStart`, text and holes, `InterpEnd`.
    InterpStart(bool),
    /// Literal text of an interpolated string (escapes and `{{` not processed).
    InterpText(&'a str),
    HoleStart,
    HoleEnd,
    InterpEnd,

    #[regex(r"//[^\n]*", logos::skip, allow_greedy = true)]
    LineComment,
    /// A line break (a block comment containing one stands for one).
    #[token("\n")]
    #[token("/*", block_comment)]
    Newline,
    /// A statement separator: a newline that ends a statement ([`layout`]).
    Sep,

    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token(",")]
    Comma,
    #[token(";")]
    Semi,
    #[token(":")]
    Colon,
    #[token(".")]
    Dot,
    #[token("..")]
    DotDot,
    #[token("..=")]
    DotDotEq,
    #[token("?")]
    Question,
    #[token("!")]
    Bang,
    #[token("!=")]
    Ne,
    #[token("=")]
    Assign,
    #[token("==")]
    EqEq,
    #[token("=>")]
    FatArrow,
    #[token("->")]
    Arrow,
    #[token("<-")]
    LArrow,
    #[token("<")]
    Lt,
    #[token("<=")]
    Le,
    #[token("<<")]
    Shl,
    /// `>` only: `>>` and `>=` are two adjacent tokens (see the module docs).
    #[token(">")]
    Gt,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("&")]
    Amp,
    #[token("&&")]
    AndAnd,
    #[token("|")]
    Bar,
    #[token("||")]
    OrOr,
    #[token("|>")]
    Pipe,
    #[token("^")]
    Caret,
    /// Input that is not a token (reported by the lexer).
    Error,
}

impl std::fmt::Display for Tok<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Tok::Word(w) => return write!(f, "`{w}`"),
            Tok::Number(n) | Tok::Int(n) | Tok::Float(n) | Tok::Duration(n) => return write!(f, "`{n}`"),
            Tok::Str(_) | Tok::RawStr(_) | Tok::InterpStart(_) => "a string",
            Tok::Char(_) => "a character",
            Tok::InterpText(_) | Tok::InterpOpen | Tok::InterpRawOpen => "string text",
            Tok::HoleStart => "`{`",
            Tok::HoleEnd => "`}`",
            Tok::InterpEnd => "the end of the string",
            Tok::LineComment => "a comment",
            Tok::Newline | Tok::Sep => "the end of the line",
            Tok::Error => "invalid input",
            t => return write!(f, "`{}`", t.punct()),
        };
        f.write_str(s)
    }
}

impl Tok<'_> {
    /// The text of a punctuation token.
    pub fn punct(&self) -> &'static str {
        match self {
            Tok::LParen => "(",
            Tok::RParen => ")",
            Tok::LBracket => "[",
            Tok::RBracket => "]",
            Tok::LBrace => "{",
            Tok::RBrace => "}",
            Tok::Comma => ",",
            Tok::Semi => ";",
            Tok::Colon => ":",
            Tok::Dot => ".",
            Tok::DotDot => "..",
            Tok::DotDotEq => "..=",
            Tok::Question => "?",
            Tok::Bang => "!",
            Tok::Ne => "!=",
            Tok::Assign => "=",
            Tok::EqEq => "==",
            Tok::FatArrow => "=>",
            Tok::Arrow => "->",
            Tok::LArrow => "<-",
            Tok::Lt => "<",
            Tok::Le => "<=",
            Tok::Shl => "<<",
            Tok::Gt => ">",
            Tok::Plus => "+",
            Tok::Minus => "-",
            Tok::Star => "*",
            Tok::Slash => "/",
            Tok::Percent => "%",
            Tok::Amp => "&",
            Tok::AndAnd => "&&",
            Tok::Bar => "|",
            Tok::OrOr => "||",
            Tok::Pipe => "|>",
            Tok::Caret => "^",
            _ => "",
        }
    }
}

/// Words that cannot name anything. Others, such as `system`, `pipeline`,
/// `map`, `set`, and `ref`, are keywords only in their own constructs.
pub fn is_reserved(w: &str) -> bool {
    matches!(
        w,
        "import"
            | "as"
            | "pub"
            | "type"
            | "struct"
            | "enum"
            | "protocol"
            | "impl"
            | "for"
            | "where"
            | "fn"
            | "once"
            | "let"
            | "var"
            | "const"
            | "if"
            | "else"
            | "match"
            | "while"
            | "loop"
            | "in"
            | "break"
            | "continue"
            | "return"
            | "use"
            | "unsafe"
            | "extern"
            | "dyn"
            | "true"
            | "false"
            | "mut"
    )
}

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// The rest of a numeric literal: identifier characters, `.` before a
/// digit, and an exponent's sign (validated by [`classify_number`]).
fn number<'a>(lex: &mut logos::Lexer<'a, Tok<'a>>) -> &'a str {
    let src = lex.source().as_bytes();
    let start = lex.span().start;
    let mut j = lex.span().end;
    let hex = src.get(start) == Some(&b'0') && matches!(src.get(start + 1), Some(b'x' | b'X'));
    while j < src.len() {
        let d = src[j];
        if is_ident(d) {
            j += 1;
        } else if d == b'.' && src.get(j + 1).is_some_and(|n| n.is_ascii_digit()) {
            j += 1;
        } else if (d == b'+' || d == b'-') && matches!(src[j - 1], b'e' | b'E') && !hex {
            j += 1;
        } else {
            break;
        }
    }
    lex.bump(j - lex.span().end);
    lex.slice()
}

/// A string's text: up to the closing quote, skipping escapes. A line break
/// or the end of input first is an error.
fn string<'a>(lex: &mut logos::Lexer<'a, Tok<'a>>) -> Result<&'a str, ()> {
    let src = lex.source().as_bytes();
    let start = lex.span().end;
    let mut j = start;
    while j < src.len() {
        match src[j] {
            b'\\' => j += 2,
            b'"' => {
                lex.bump(j + 1 - start);
                return Ok(&lex.source()[start..j]);
            }
            b'\n' => break,
            _ => j += 1,
        }
    }
    lex.bump(j.min(src.len()) - start);
    Err(())
}

fn raw_string<'a>(lex: &mut logos::Lexer<'a, Tok<'a>>) -> Result<&'a str, ()> {
    let rest = lex.remainder();
    match rest.find("\"\"\"") {
        Some(i) => {
            lex.bump(i + 3);
            Ok(&rest[..i])
        }
        None => {
            lex.bump(rest.len());
            Err(())
        }
    }
}

/// One character or escape between single quotes.
fn char_lit<'a>(lex: &mut logos::Lexer<'a, Tok<'a>>) -> Result<&'a str, ()> {
    let rest = lex.remainder();
    let mut chars = rest.char_indices();
    let end = match chars.next() {
        Some((_, '\\')) => match chars.next() {
            Some((_, 'u')) => rest.find('}').map(|i| i + 1),
            Some((i, c)) => Some(i + c.len_utf8()),
            None => None,
        },
        Some((_, '\'')) | Some((_, '\n')) | None => None,
        Some((i, c)) => Some(i + c.len_utf8()),
    };
    match end {
        Some(e) if rest[e..].starts_with('\'') => {
            lex.bump(e + 1);
            Ok(&rest[..e])
        }
        _ => {
            // up to the next quote on the line, as one bad token
            let stop = rest.find(['\'', '\n']).map(|i| if rest[i..].starts_with('\'') { i + 1 } else { i });
            lex.bump(stop.unwrap_or(rest.len()));
            Err(())
        }
    }
}

/// A (nested) block comment. One spanning lines stands for a newline;
/// an unterminated one is an error.
fn block_comment<'a>(lex: &mut logos::Lexer<'a, Tok<'a>>) -> FilterResult<(), ()> {
    let b = lex.remainder().as_bytes();
    let mut depth = 1usize;
    let mut j = 0;
    let mut newline = false;
    while j < b.len() {
        if b[j] == b'/' && b.get(j + 1) == Some(&b'*') {
            depth += 1;
            j += 2;
        } else if b[j] == b'*' && b.get(j + 1) == Some(&b'/') {
            depth -= 1;
            j += 2;
            if depth == 0 {
                lex.bump(j);
                return if newline { FilterResult::Emit(()) } else { FilterResult::Skip };
            }
        } else {
            newline |= b[j] == b'\n';
            j += 1;
        }
    }
    lex.bump(b.len());
    FilterResult::Error(())
}

/// A lexical error: what is wrong, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub message: String,
    pub span: Span,
}

/// Is `s` (all digits and `_`, starting with a digit) a run of decimal digits?
fn dec_digits(s: &str) -> bool {
    s.as_bytes().first().is_some_and(u8::is_ascii_digit) && s.bytes().all(|c| c.is_ascii_digit() || c == b'_')
}

/// Split a numeric suffix (`I32`, `U8`, `F64`) off the end of `s`.
fn split_num_suffix(s: &str) -> (&str, Option<&str>) {
    let b = s.as_bytes();
    let mut i = b.len();
    while i > 0 && b[i - 1].is_ascii_digit() {
        i -= 1;
    }
    if i > 0 && i < b.len() && matches!(b[i - 1], b'I' | b'U' | b'F') {
        return (&s[..i - 1], Some(&s[i - 1..]));
    }
    (s, None)
}

/// The kind of a numeric literal, or why it is not one.
fn classify_number(s: &str) -> Result<Tok<'_>, &'static str> {
    // durations: decimal digits then a unit
    for unit in ["ns", "us", "ms", "min", "s", "h"] {
        if let Some(n) = s.strip_suffix(unit)
            && dec_digits(n)
        {
            return Ok(Tok::Duration(s));
        }
    }
    if let Some(h) = s.strip_prefix("0x") {
        // hex digits include `F`: only I/U suffixes
        let digits_end = h.find(|c: char| !(c.is_ascii_hexdigit() || c == '_')).unwrap_or(h.len());
        let (digits, rest) = h.split_at(digits_end);
        let suffix_ok = rest.is_empty()
            || (matches!(rest.as_bytes()[0], b'I' | b'U')
                && rest.len() > 1
                && rest[1..].bytes().all(|c| c.is_ascii_digit()));
        return if !digits.is_empty() && suffix_ok {
            Ok(Tok::Int(s))
        } else {
            Err("invalid hexadecimal literal")
        };
    }
    if let Some(bits) = s.strip_prefix("0b") {
        let (digits, suffix) = split_num_suffix(bits);
        let ok = !digits.is_empty()
            && digits.bytes().all(|c| matches!(c, b'0' | b'1' | b'_'))
            && suffix.is_none_or(|x| !x.starts_with('F'));
        return if ok { Ok(Tok::Int(s)) } else { Err("invalid binary literal") };
    }
    let (body, suffix) = split_num_suffix(s);
    if dec_digits(body) {
        return Ok(if suffix.is_some_and(|x| x.starts_with('F')) { Tok::Float(s) } else { Tok::Int(s) });
    }
    // decimal: digits (`.` digit...)? exponent?
    let (mantissa, exponent) = match body.find(['e', 'E']) {
        Some(i) => (&body[..i], Some(&body[i + 1..])),
        None => (body, None),
    };
    let mantissa_ok = match mantissa.split_once('.') {
        Some((int, frac)) => {
            dec_digits(int) && frac.as_bytes().first().is_some_and(u8::is_ascii_digit) && dec_digits(frac)
        }
        None => dec_digits(mantissa) && exponent.is_some(),
    };
    let exponent_ok = exponent.is_none_or(|e| {
        let e = e.strip_prefix(['+', '-']).unwrap_or(e);
        !e.is_empty() && e.bytes().all(|c| c.is_ascii_digit())
    });
    if mantissa_ok && exponent_ok { Ok(Tok::Float(s)) } else { Err("invalid number literal") }
}

/// An open interpolation hole: whether its string is raw, and how many
/// braces of its own are open.
struct Hole {
    raw: bool,
    depth: usize,
}

/// Lex `src` into tokens (with [`Tok::Newline`] for line breaks; see
/// [`layout`]) and the lexical errors found. Every error leaves a
/// [`Tok::Error`] in the buffer, so the parser reports the rest too.
pub fn lex(src: &str) -> (Vec<(Tok<'_>, Span)>, Vec<LexError>) {
    let mut lexer = Tok::lexer(src);
    let mut out: Vec<(Tok<'_>, Span)> = Vec::with_capacity(src.len() / 4);
    let mut errors = vec![];
    let mut holes: Vec<Hole> = vec![];
    while let Some(r) = lexer.next() {
        let span: Span = lexer.span().into();
        let tok = match r {
            Ok(t) => t,
            Err(()) => {
                let text = &src[span.start..span.end];
                let message = if text.starts_with("/*") {
                    "unterminated block comment".to_string()
                } else if text.starts_with("\"\"\"") {
                    "unterminated raw string: add the closing `\"\"\"`".to_string()
                } else if text.starts_with('"') {
                    "unterminated string: add the closing `\"` on this line".to_string()
                } else if text.starts_with('\'') {
                    "invalid character literal: one character or escape between single quotes".to_string()
                } else {
                    format!("unexpected character `{}`", text.chars().next().unwrap_or(' '))
                };
                let span = if text.starts_with("/*") { sp(span.start, span.start + 2) } else { span };
                errors.push(LexError { message, span });
                out.push((Tok::Error, span));
                continue;
            }
        };
        match tok {
            Tok::InterpOpen | Tok::InterpRawOpen => {
                let raw = tok == Tok::InterpRawOpen;
                out.push((Tok::InterpStart(raw), span));
                interp_text(&mut lexer, raw, &mut out, &mut errors, &mut holes);
            }
            Tok::LBrace if !holes.is_empty() => {
                holes.last_mut().unwrap().depth += 1;
                out.push((tok, span));
            }
            Tok::RBrace if holes.last().is_some_and(|h| h.depth > 0) => {
                holes.last_mut().unwrap().depth -= 1;
                out.push((tok, span));
            }
            Tok::RBrace if !holes.is_empty() => {
                let raw = holes.pop().unwrap().raw;
                out.push((Tok::HoleEnd, span));
                interp_text(&mut lexer, raw, &mut out, &mut errors, &mut holes);
            }
            Tok::Number(text) => {
                // `t.0.1`: tuple fields, not a float
                let after_dot = out.last().is_some_and(|(t, s)| *t == Tok::Dot && s.end == span.start);
                if after_dot && text.contains('.') {
                    let mut at = span.start;
                    for (i, part) in text.split('.').enumerate() {
                        if i > 0 {
                            out.push((Tok::Dot, sp(at, at + 1)));
                            at += 1;
                        }
                        let s = sp(at, at + part.len());
                        out.push((number_token(part, s, &mut errors), s));
                        at += part.len();
                    }
                } else {
                    out.push((number_token(text, span, &mut errors), span));
                }
            }
            _ => out.push((tok, span)),
        }
    }
    if !holes.is_empty() {
        let at = src.len();
        errors.push(LexError {
            message: "unterminated interpolation: close the `{` and the string".into(),
            span: sp(at, at),
        });
    }
    (out, errors)
}

fn number_token<'a>(text: &'a str, span: Span, errors: &mut Vec<LexError>) -> Tok<'a> {
    match classify_number(text) {
        Ok(t) => t,
        Err(message) => {
            errors.push(LexError { message: message.into(), span });
            Tok::Error
        }
    }
}

/// Scan an interpolated string's text from the lexer's position: emit text
/// runs, then either a hole (`HoleStart`; the caller lexes its tokens) or
/// the end of the string.
fn interp_text<'a>(
    lexer: &mut logos::Lexer<'a, Tok<'a>>,
    raw: bool,
    out: &mut Vec<(Tok<'a>, Span)>,
    errors: &mut Vec<LexError>,
    holes: &mut Vec<Hole>,
) {
    let src = lexer.source();
    let base = lexer.span().end;
    let b = src.as_bytes();
    let mut j = base;
    let mut text_start = base;
    let flush = |out: &mut Vec<(Tok<'a>, Span)>, from: usize, to: usize| {
        if to > from {
            out.push((Tok::InterpText(&src[from..to]), sp(from, to)));
        }
    };
    loop {
        if j >= b.len() || (!raw && b[j] == b'\n') {
            flush(out, text_start, j);
            errors.push(LexError {
                message: if raw {
                    "unterminated raw string: add the closing `\"\"\"`".into()
                } else {
                    "unterminated string: add the closing `\"` on this line".into()
                },
                span: sp(j, j),
            });
            out.push((Tok::InterpEnd, sp(j, j)));
            lexer.bump(j - base);
            return;
        }
        let at_close = if raw { src[j..].starts_with("\"\"\"") } else { b[j] == b'"' };
        if at_close {
            flush(out, text_start, j);
            let len = if raw { 3 } else { 1 };
            out.push((Tok::InterpEnd, sp(j, j + len)));
            lexer.bump(j + len - base);
            return;
        }
        match b[j] {
            b'\\' if !raw => j += 2,
            b'{' if b.get(j + 1) == Some(&b'{') => j += 2,
            b'}' if b.get(j + 1) == Some(&b'}') => j += 2,
            b'{' => {
                flush(out, text_start, j);
                out.push((Tok::HoleStart, sp(j, j + 1)));
                holes.push(Hole { raw, depth: 0 });
                lexer.bump(j + 1 - base);
                return;
            }
            b'}' => {
                errors.push(LexError {
                    message: "a lone `}` in an interpolated string: write `}}`".into(),
                    span: sp(j, j + 1),
                });
                j += 1;
                text_start = j;
            }
            _ => j += 1,
        }
        j = j.min(b.len());
    }
}

/// Whether a token can end a statement, continues the previous line, or neither.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ends {
    Yes,
    Continues,
    No,
}

fn ends(t: &Tok<'_>) -> Ends {
    match t {
        Tok::Word("else") => Ends::Continues,
        Tok::Word("true" | "false" | "break" | "continue" | "return") => Ends::Yes,
        // other reserved words never end a statement
        Tok::Word(w) if is_reserved(w) => Ends::No,
        Tok::Word(_)
        | Tok::Int(_)
        | Tok::Float(_)
        | Tok::Duration(_)
        | Tok::Number(_)
        | Tok::Str(_)
        | Tok::RawStr(_)
        | Tok::Char(_)
        | Tok::InterpStart(_)
        | Tok::RParen
        | Tok::RBracket
        | Tok::RBrace
        | Tok::Question => Ends::Yes,
        Tok::Pipe => Ends::Continues,
        _ => Ends::No,
    }
}

/// Brackets nested deeper than this are refused (the parser and every
/// later pass recurse once per level).
pub const MAX_NESTING: usize = 1000;

/// Turn the newlines that end statements into [`Tok::Sep`] and drop the
/// others. A newline ends a statement when the token before it can end
/// one, it is not inside parentheses or brackets (a `{` restores statement
/// boundaries inside them), and the next line does not start with `|>` or
/// `else`. An interpolated string counts as one token.
///
/// Also returns the first bracket opened deeper than [`MAX_NESTING`].
pub fn layout<'a>(tokens: Vec<(Tok<'a>, Span)>) -> (Vec<(Tok<'a>, Span)>, Option<Span>) {
    let mut out = Vec::with_capacity(tokens.len());
    let mut stack: Vec<Tok<'_>> = vec![];
    let mut too_deep = None;
    let mut last_ends = false;
    let mut pending: Option<Span> = None;
    // inside an interpolated string (its holes are part of the string)
    let mut interp = 0usize;
    for (t, s) in tokens {
        if interp > 0 {
            match t {
                Tok::InterpStart(_) => interp += 1,
                Tok::InterpEnd => interp -= 1,
                _ => {}
            }
            if t != Tok::Newline {
                out.push((t, s));
            }
            if interp == 0 {
                last_ends = true;
            }
            continue;
        }
        if t == Tok::Newline {
            if last_ends && matches!(stack.last(), None | Some(Tok::LBrace)) && pending.is_none() {
                pending = Some(sp(s.start, s.start + 1));
            }
            last_ends = false;
            continue;
        }
        let kind = ends(&t);
        if let Some(p) = pending.take()
            && kind != Ends::Continues
        {
            out.push((Tok::Sep, p));
        }
        match t {
            Tok::LParen | Tok::LBracket | Tok::LBrace => {
                stack.push(t);
                if stack.len() > MAX_NESTING && too_deep.is_none() {
                    too_deep = Some(s);
                }
            }
            Tok::RParen | Tok::RBracket | Tok::RBrace => {
                stack.pop();
            }
            Tok::InterpStart(_) => interp = 1,
            _ => {}
        }
        last_ends = kind == Ends::Yes;
        out.push((t, s));
    }
    if let Some(p) = pending {
        out.push((Tok::Sep, p));
    }
    (out, too_deep)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> String {
        let (toks, errs) = lex(src);
        assert!(errs.is_empty(), "{errs:?}");
        let (toks, _) = layout(toks);
        toks.iter()
            .map(|(t, _)| match t {
                Tok::Sep => ";".to_string(),
                Tok::Word(w) => w.to_string(),
                Tok::Int(n) | Tok::Float(n) | Tok::Duration(n) => n.to_string(),
                Tok::Str(s) => format!("\"{s}\""),
                Tok::InterpStart(_) => "$\"".into(),
                Tok::InterpText(s) => s.to_string(),
                Tok::HoleStart => "{".into(),
                Tok::HoleEnd => "}".into(),
                Tok::InterpEnd => "\"".into(),
                t => t.punct().to_string(),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn inserts_after_statement_end() {
        assert_eq!(kinds("let a = 1\nlet b = 2\n"), "let a = 1 ; let b = 2 ;");
    }

    #[test]
    fn no_insert_inside_parens_or_after_operator() {
        assert_eq!(kinds("f(\n a,\n b\n)\n"), "f ( a , b ) ;");
        assert_eq!(kinds("a +\n b\n"), "a + b ;");
    }

    #[test]
    fn pipe_and_else_continue() {
        assert_eq!(kinds("x\n  |> f()\n"), "x |> f ( ) ;");
        assert_eq!(kinds("if a { b }\nelse { c }\n"), "if a { b } else { c } ;");
    }

    #[test]
    fn braces_restore_inside_parens() {
        assert_eq!(kinds("f(fn() {\n a()\n b()\n})\n"), "f ( fn ( ) { a ( ) ; b ( ) ; } ) ;");
    }

    #[test]
    fn comments_and_interpolation() {
        assert_eq!(kinds("a // c\nb"), "a ; b");
        assert_eq!(kinds("a /* x\n */ b"), "a ; b");
        assert_eq!(kinds("a /* x */ b"), "a b");
        assert_eq!(kinds("$\"x {f(\"}\")} y\"\nz"), "$\" x  { f ( \"}\" ) }  y \" ; z");
        assert_eq!(kinds("$\"{{a}} {m[{k: 1}]}\""), "$\" {{a}}  { m [ { k : 1 } ] } \"");
    }

    #[test]
    fn numbers() {
        assert_eq!(
            kinds("t.0.1 1.5 1..2 0xFFU32 2e-3 10ms 3F32"),
            "t . 0 . 1 1.5 1 .. 2 0xFFU32 2e-3 10ms 3F32"
        );
        let (_, errs) = lex("12abc 0x 1.5U8x");
        assert_eq!(errs.len(), 3, "{errs:?}");
    }

    #[test]
    fn unterminated() {
        let (_, errs) = lex("a /* b");
        assert_eq!(errs[0].message, "unterminated block comment");
        assert_eq!(errs[0].span, sp(2, 4));
        let (_, errs) = lex("\"abc\nx");
        assert!(errs[0].message.starts_with("unterminated string"));
    }
}
