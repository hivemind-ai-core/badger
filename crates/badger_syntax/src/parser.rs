//! Parsing: lex ([`crate::token`]), lay out, parse with the chumsky
//! grammar ([`crate::grammar`]), and lower to the AST ([`crate::lower`]).
//!
//! Syntax errors are recovered from: every broken statement and
//! declaration is reported in one pass, and [`parse_recovering`] keeps the
//! partial tree (error nodes stand for what did not parse).

use badger_base::{Diagnostic, FileId, Interner, Span};
use chumsky::error::{RichPattern, RichReason};
use chumsky::prelude::*;

use crate::ast::Ast;
use crate::grammar;
use crate::lower::{Lower, MAX_EXPR_DEPTH};
use crate::token::{self, MAX_NESTING, Tok};

/// The syntax errors of a file (each a diagnostic, E0001 unless noted).
#[derive(Debug)]
pub struct ParseError {
    pub errors: Vec<Diagnostic>,
}

impl ParseError {
    /// Where the first error is.
    pub fn span(&self) -> Span {
        self.errors.first().and_then(|d| d.labels.first().map(|l| l.span)).unwrap_or_else(Span::dummy)
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let first = self.errors.first();
        let msg = first.and_then(|d| d.labels.first().map(|l| l.message.clone())).unwrap_or_default();
        write!(f, "{msg}")
    }
}

impl std::error::Error for ParseError {}

/// Parse a whole file. Syntax errors make it fail with all of them; case
/// rules and other checks on a well-formed tree are reported in `diags`.
pub fn parse(
    file: FileId,
    src: &str,
    interner: &mut Interner,
    diags: &mut Vec<Diagnostic>,
) -> Result<Ast, ParseError> {
    let (ast, errors) = parse_recovering(file, src, interner, diags);
    if errors.is_empty() { Ok(ast) } else { Err(ParseError { errors }) }
}

/// Parse a whole file, recovering from syntax errors: the tree (with error
/// nodes where something did not parse) and the syntax errors.
pub fn parse_recovering(
    file: FileId,
    src: &str,
    interner: &mut Interner,
    diags: &mut Vec<Diagnostic>,
) -> (Ast, Vec<Diagnostic>) {
    // the headline says what is wrong; the label, what belongs there
    let syntax = |span: token::Span, message: String| {
        let (head, label) = match message.split_once(": ") {
            Some((h, l)) => (h.to_string(), l.to_string()),
            None => match message.split_once(", found ") {
                Some((expected, _)) => (message.clone(), expected.to_string()),
                None => (message.clone(), "here".to_string()),
            },
        };
        Diagnostic::error(head).with_code("E0001").with_label(Span::new(file, span.start, span.end), label)
    };
    let (tokens, lex_errors) = token::lex(src);
    let mut errors: Vec<Diagnostic> = lex_errors.into_iter().map(|e| syntax(e.span, e.message)).collect();
    // unbalanced brackets explain everything after them: report them alone,
    // with the tree of the text as if they were balanced (for the editor's
    // symbols, hover, and completion while one is being typed)
    let brackets = bracket_errors(file, src, &tokens);
    if !brackets.is_empty() {
        errors.extend(brackets);
        let (tokens, too_deep) = token::layout(repair_brackets(src, tokens));
        if too_deep.is_some() {
            return (Ast::default(), errors);
        }
        let eoi: token::Span = (src.len()..src.len()).into();
        let input = tokens.as_slice().split_token_span(eoi);
        let (items, _) = grammar::program::<_, grammar::Error<'_, '_>>().parse(input).into_output_errors();
        // the repaired tree's own findings are left out, as its errors are
        let mut scratch = vec![];
        let mut lower = Lower::new(file, src, interner, &mut scratch);
        if let Some(items) = items {
            lower.items(items);
        }
        return (lower.ast, errors);
    }
    let (tokens, too_deep) = token::layout(tokens);
    if let Some(at) = too_deep {
        errors.push(syntax(
            at,
            format!("brackets nested more than {MAX_NESTING} deep: split the expression into named parts"),
        ));
        return (Ast::default(), errors);
    }
    let eoi: token::Span = (src.len()..src.len()).into();
    let input = tokens.as_slice().split_token_span(eoi);
    // the fast pass tracks no errors; a file that does not parse is parsed
    // again for the report (with recovery, so every error is found)
    let fast = grammar::program::<_, chumsky::error::EmptyErr>().parse(input).into_output_errors();
    let (items, parse_errors) = match fast {
        (Some(items), errs) if errs.is_empty() => (Some(items), vec![]),
        _ => grammar::program::<_, grammar::Error<'_, '_>>().parse(input).into_output_errors(),
    };
    for e in parse_errors {
        let mut d = syntax(*e.span(), describe(&e));
        // an operator at the end of a line continues the expression onto
        // the next: say so where that is why the next line is unexpected
        let at = tokens.partition_point(|(_, s)| s.start < e.span().start);
        if at > 0
            && let Some((prev, ps)) = tokens.get(at - 1)
            && continues(prev)
            && src[ps.end..e.span().start].contains('\n')
        {
            d = d.with_label(
                Span::new(file, ps.start, ps.end),
                format!("this {prev} continues the expression onto the next line"),
            );
        }
        if !errors.iter().any(|x| x.labels.first().map(|l| l.span) == d.labels.first().map(|l| l.span)) {
            errors.push(d);
        }
    }
    let mut lower = Lower::new(file, src, interner, diags);
    if let Some(items) = items {
        lower.items(items);
    }
    if let Some(span) = lower.too_deep {
        errors.push(
            Diagnostic::error("syntax error").with_code("E0001").with_label(
                span,
                format!(
                    "expression nested more than {MAX_EXPR_DEPTH} deep (a long operator chain or `else if` ladder counts each step): split it into named parts"
                ),
            ),
        );
    }
    (lower.ast, errors)
}

/// Does a token at the end of a line continue the statement (a binary
/// operator, or a separator inside a list)?
fn continues(t: &Tok<'_>) -> bool {
    matches!(
        t,
        Tok::Plus
            | Tok::Minus
            | Tok::Star
            | Tok::Slash
            | Tok::Percent
            | Tok::AndAnd
            | Tok::OrOr
            | Tok::EqEq
            | Tok::Ne
            | Tok::Lt
            | Tok::Le
            | Tok::Gt
            | Tok::Shl
            | Tok::Amp
            | Tok::Bar
            | Tok::Caret
            | Tok::DotDot
            | Tok::DotDotEq
            | Tok::Assign
            | Tok::PlusAssign
            | Tok::MinusAssign
            | Tok::StarAssign
            | Tok::SlashAssign
            | Tok::PercentAssign
    )
}

/// The message for a parse error: what was expected, and what was found.
fn describe(e: &grammar::Error<'_, '_>) -> String {
    match e.reason() {
        RichReason::Custom(msg) => msg.to_string(),
        RichReason::ExpectedFound { expected, found } => {
            let mut names: Vec<String> = expected
                .iter()
                .filter_map(|p| match p {
                    RichPattern::Token(t) => Some(t.to_string()),
                    RichPattern::Label(l) => Some(l.to_string()),
                    RichPattern::Identifier(i) => Some(format!("`{i}`")),
                    RichPattern::EndOfInput => Some("the end of the file".into()),
                    _ => None,
                })
                .collect();
            // an operand can start with a prefix operator: "expression" says it
            if names.iter().any(|n| n == "expression") {
                names.retain(|n| !matches!(n.as_str(), "`!`" | "`&`" | "`-`"));
            }
            // a list of every operator that could continue: "an operator"
            const OPERATORS: &[&str] = &[
                "`!=`", "`%`", "`&&`", "`&`", "`*`", "`+`", "`-`", "`..=`", "`..`", "`/`", "`<<`", "`<=`",
                "`<`", "`==`", "`^`", "`|>`", "`|`", "`||`", "`>`", "`.`", "`?`", "`(`", "`[`",
            ];
            if names.iter().filter(|n| OPERATORS.contains(&n.as_str())).count() >= 4 {
                names.retain(|n| !OPERATORS.contains(&n.as_str()));
                names.push("an operator".into());
            }
            names.sort();
            names.dedup();
            let found = match found {
                Some(t) => t.to_string(),
                None => "the end of the file".into(),
            };
            match names.len() {
                0 => format!("unexpected {found}"),
                1 => format!("expected {}, found {found}", names[0]),
                n => format!("expected {} or {}, found {found}", names[..n - 1].join(", "), names[n - 1]),
            }
        }
    }
}

/// Brackets that do not pair up: a closer with nothing open, a closer of
/// the wrong kind, and openers never closed. Each error points at both
/// ends where there are two.
fn bracket_errors(file: FileId, src: &str, tokens: &[(Tok<'_>, token::Span)]) -> Vec<Diagnostic> {
    let sp = |s: token::Span| Span::new(file, s.start, s.end);
    let mut stack: Vec<(Tok<'_>, token::Span)> = vec![];
    let mut out = vec![];
    let close_of = |t: Tok<'_>| match t {
        Tok::LParen => Tok::RParen,
        Tok::LBracket => Tok::RBracket,
        _ => Tok::RBrace,
    };
    for &(t, s) in tokens {
        match t {
            Tok::LParen | Tok::LBracket | Tok::LBrace => stack.push((t, s)),
            Tok::RParen | Tok::RBracket | Tok::RBrace => match stack.pop() {
                Some((open, _)) if close_of(open) == t => {}
                Some((open, os)) => {
                    out.push(
                        Diagnostic::error(format!("mismatched {t}"))
                            .with_code("E0001")
                            .with_label(sp(s), format!("expected {} here", close_of(open)))
                            .with_label(sp(os), format!("to close this {open}")),
                    );
                    return out;
                }
                None => {
                    out.push(
                        Diagnostic::error(format!("unexpected {t}"))
                            .with_code("E0001")
                            .with_label(sp(s), "nothing is open here"),
                    );
                    return out;
                }
            },
            _ => {}
        }
    }
    if let Some((open, os)) = stack.pop() {
        let end = src.len();
        out.push(
            Diagnostic::error(format!("unclosed {open}"))
                .with_code("E0001")
                .with_label(sp(os), "this is never closed")
                .with_label(Span::new(file, end, end), format!("the file ends here: add {}", close_of(open))),
        );
    }
    out
}

/// The tokens with their brackets balanced, guessing where the missing
/// ones go: a stray closer is dropped, a mismatched one closes what is
/// open up to its opener, and what is still open closes before the next
/// line starting at column 0 (the next declaration) or at the end.
fn repair_brackets<'a>(src: &str, tokens: Vec<(Tok<'a>, token::Span)>) -> Vec<(Tok<'a>, token::Span)> {
    let close_of = |t: Tok<'_>| match t {
        Tok::LParen => Tok::RParen,
        Tok::LBracket => Tok::RBracket,
        _ => Tok::RBrace,
    };
    let at = |p: usize| -> token::Span { (p..p).into() };
    let mut stack: Vec<Tok<'a>> = vec![];
    let mut out = Vec::with_capacity(tokens.len() + 4);
    for (t, s) in tokens {
        let line_start = s.start == 0 || src.as_bytes().get(s.start - 1) == Some(&b'\n');
        let closer = matches!(t, Tok::RParen | Tok::RBracket | Tok::RBrace);
        if line_start && !closer && !stack.is_empty() {
            while let Some(open) = stack.pop() {
                out.push((close_of(open), at(s.start)));
            }
        }
        match t {
            Tok::LParen | Tok::LBracket | Tok::LBrace => stack.push(t),
            Tok::RParen | Tok::RBracket | Tok::RBrace => {
                let Some(depth) = stack.iter().rposition(|o| close_of(*o) == t) else { continue };
                while stack.len() > depth + 1 {
                    let open = stack.pop().unwrap();
                    out.push((close_of(open), at(s.start)));
                }
                stack.pop();
            }
            _ => {}
        }
        out.push((t, s));
    }
    while let Some(open) = stack.pop() {
        out.push((close_of(open), at(src.len())));
    }
    out
}

/// The tokens of a source that parses, as `(kind, text)` pairs: a
/// whitespace-insensitive fingerprint of its structure (used to check that
/// formatting preserves meaning). A trailing comma before a closer is left
/// out, since formatting may drop it.
pub fn leaf_tokens(src: &str) -> Result<Vec<(String, String)>, String> {
    let mut sm = badger_base::SourceMap::default();
    let file = sm.add("fingerprint", src.to_string());
    if let Err(e) = parse(file, src, &mut Interner::new(), &mut vec![]) {
        return Err(e.to_string());
    }
    let (tokens, _) = token::lex(src);
    let (tokens, _) = token::layout(tokens);
    let mut out: Vec<(String, String)> = vec![];
    for (i, (t, s)) in tokens.iter().enumerate() {
        let closer = |j: usize| matches!(tokens.get(j), Some((Tok::RParen | Tok::RBracket | Tok::RBrace, _)));
        if *t == Tok::Comma && closer(i + 1) {
            continue;
        }
        let kind = format!("{t:?}");
        let kind = kind.split('(').next().unwrap_or("").to_string();
        out.push((kind, src[s.start..s.end].to_string()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;
    use badger_base::SourceMap;

    fn parse_ok(src: &str) -> (Ast, Interner) {
        let mut sm = SourceMap::default();
        let f = sm.add("t", src);
        let mut int = Interner::new();
        let mut diags = vec![];
        let ast = parse(f, &sm.get(f).text, &mut int, &mut diags).unwrap_or_else(|e| {
            panic!("parse errors: {:?}", e.errors);
        });
        assert!(diags.is_empty(), "{diags:?}");
        (ast, int)
    }

    fn errors(src: &str) -> Vec<(u32, String)> {
        let mut sm = SourceMap::default();
        let f = sm.add("t", src);
        let err = parse(f, &sm.get(f).text, &mut Interner::new(), &mut vec![]).unwrap_err();
        err.errors
            .iter()
            .map(|d| (d.labels[0].span.start, format!("{}: {}", d.message, d.labels[0].message)))
            .collect()
    }

    fn body_expr(ast: &Ast) -> ExprId {
        let ItemKind::Global { value, .. } = ast.items[0].kind else { panic!() };
        value
    }

    #[test]
    fn precedence() {
        let (ast, _) = parse_ok("let x = 1 + 2 * 3");
        let root = body_expr(&ast);
        let ExprKind::Binary { op: BinOp::Add, rhs, .. } = ast.exprs[root].kind else {
            panic!("expected + at root");
        };
        assert!(matches!(ast.exprs[rhs].kind, ExprKind::Binary { op: BinOp::Mul, .. }));
    }

    #[test]
    fn pipe_is_weakest() {
        let (ast, _) = parse_ok("let x = a + 1 |> f(b) |> g()");
        let root = body_expr(&ast);
        let ExprKind::Binary { op: BinOp::Pipe, lhs, .. } = ast.exprs[root].kind else { panic!() };
        assert!(matches!(ast.exprs[lhs].kind, ExprKind::Binary { op: BinOp::Pipe, .. }));
    }

    #[test]
    fn record_vs_block() {
        let (ast, _) = parse_ok("let a = { value }\nlet b = { x + 1 }\nlet c = {}\nlet d = { ..p, age: 1 }");
        let kinds: Vec<_> = ast
            .items
            .iter()
            .map(|i| match i.kind {
                ItemKind::Global { value, .. } => match ast.exprs[value].kind {
                    ExprKind::Record { .. } => "record",
                    ExprKind::Block { .. } => "block",
                    _ => "other",
                },
                _ => "item",
            })
            .collect();
        assert_eq!(kinds, ["record", "block", "record", "record"]);
    }

    #[test]
    fn generic_call_comparison_and_shifts() {
        let (ast, _) = parse_ok(
            "let a = f<Int>(1)\nlet b = x < y\nlet c = x >> 2\nlet d = x >= y\nlet e = f<List<Int>>(x)",
        );
        let value = |i: usize| match ast.items[i].kind {
            ItemKind::Global { value, .. } => value,
            _ => panic!(),
        };
        assert!(
            matches!(&ast.exprs[value(0)].kind, ExprKind::Call { type_args, .. } if type_args.len() == 1)
        );
        assert!(matches!(ast.exprs[value(1)].kind, ExprKind::Binary { op: BinOp::Lt, .. }));
        assert!(matches!(ast.exprs[value(2)].kind, ExprKind::Binary { op: BinOp::Shr, .. }));
        assert!(matches!(ast.exprs[value(3)].kind, ExprKind::Binary { op: BinOp::Ge, .. }));
        assert!(
            matches!(&ast.exprs[value(4)].kind, ExprKind::Call { type_args, .. } if type_args.len() == 1)
        );
    }

    #[test]
    fn literals() {
        let (ast, int) = parse_ok("let a = [0xFFU32, 12U8, 0.5F32, 1_000, 100ms, 'A', #app/db, $\"n={n}\"]");
        let ExprKind::List(items) = &ast.exprs[body_expr(&ast)].kind else { panic!() };
        let ExprKind::Lit(Lit::Int { value: 255, suffix: Some(s) }) = ast.exprs[items[0]].kind else {
            panic!()
        };
        assert_eq!(int.resolve(s), "U32");
        assert!(matches!(ast.exprs[items[2]].kind, ExprKind::Lit(Lit::Float { .. })));
        assert!(matches!(ast.exprs[items[3]].kind, ExprKind::Lit(Lit::Int { value: 1000, suffix: None })));
        assert!(matches!(
            ast.exprs[items[4]].kind,
            ExprKind::Lit(Lit::Duration { value: 100, unit: DurationUnit::Ms })
        ));
        assert!(matches!(ast.exprs[items[5]].kind, ExprKind::Lit(Lit::Char('A'))));
        let ExprKind::Lit(Lit::Symbol(s)) = ast.exprs[items[6]].kind else { panic!() };
        assert_eq!(int.resolve(s), "app/db");
        assert!(matches!(&ast.exprs[items[7]].kind, ExprKind::Interp(p) if p.len() == 2));
    }

    #[test]
    fn symbols_and_colons() {
        // `x: Int` is an annotation, `{a:b}` a field, `#b` a symbol
        let (ast, _) = parse_ok("fn f(x: Int) -> Int { x }\nlet r = {a:b}\nlet s = { k: #v }");
        let ItemKind::Global { value, .. } = ast.items[1].kind else { panic!() };
        let ExprKind::Record { entries, .. } = &ast.exprs[value].kind else { panic!() };
        let RecordEntry::Field { value, .. } = &entries[0] else { panic!() };
        assert!(matches!(ast.exprs[*value].kind, ExprKind::Name(_)));
        let ItemKind::Global { value, .. } = ast.items[2].kind else { panic!() };
        let ExprKind::Record { entries, .. } = &ast.exprs[value].kind else { panic!() };
        let RecordEntry::Field { value, .. } = &entries[0] else { panic!() };
        assert!(matches!(ast.exprs[*value].kind, ExprKind::Lit(Lit::Symbol(_))));
    }

    #[test]
    fn statements_and_semicolons() {
        let (ast, _) = parse_ok("fn f() {\n  let a = 1\n  a;\n}\nfn g() { 1 }");
        let ItemKind::Fn(f) = &ast.items[0].kind else { panic!() };
        let ExprKind::Block { stmts } = &ast.exprs[f.body.unwrap()].kind else { panic!() };
        assert!(matches!(stmts[1], Stmt::Expr { semi: true, .. }));
        let ItemKind::Fn(g) = &ast.items[1].kind else { panic!() };
        let ExprKind::Block { stmts } = &ast.exprs[g.body.unwrap()].kind else { panic!() };
        assert!(matches!(stmts[0], Stmt::Expr { semi: false, .. }));
    }

    #[test]
    fn case_rules() {
        let mut sm = SourceMap::default();
        let f = sm.add("t", "type person = { name: String }\nlet Count = 1\nenum Reply { ok(value: Int) }");
        let mut diags = vec![];
        parse(f, &sm.get(f).text, &mut Interner::new(), &mut diags).unwrap();
        let codes: Vec<_> = diags.iter().map(|d| d.code.unwrap()).collect();
        assert_eq!(codes, ["E0101", "E0102"]);
    }

    #[test]
    fn reports_error_span() {
        let errs = errors("let x = 1 + ");
        assert_eq!(errs[0].0, 12, "{errs:?}");
    }

    #[test]
    fn reports_every_broken_statement() {
        let src =
            "fn f() {\n    let a = 1 +\n    let b = (2,,)\n    let c = 3\n    g(,)\n}\nfn h() { ok(]) }\n";
        let errs = errors(src);
        assert_eq!(errs.len(), 1, "an unbalanced bracket is reported alone: {errs:?}");
        let src =
            "fn f() {\n    let a = 1 +\n    let b = (2,,)\n    let c = 3\n    g(,)\n}\nfn h() { ok(1 2) }\n";
        let errs = errors(src);
        // `1 +` continues on the next line: `let a` and `let b` are one statement
        assert_eq!(errs.len(), 3, "{errs:?}");
        assert!(errs[0].1.starts_with("expected expression, found `let`"), "{errs:?}");
        assert!(errs[2].1.starts_with("expected `)`, `,` or an operator, found `2`"), "{errs:?}");
        let errs = errors("fn f() {\n    let a = 1 + * 2\n    let b = 3\n    let c = ]\n}\n");
        assert!(
            errs.iter().any(|(_, m)| m.contains("nothing is open") || m.contains("mismatched")),
            "{errs:?}"
        );
        let errs = errors("fn f() {\n    let a = 1 + * 2\n    let b = 3\n    let c = * 4\n}\nlet d = = 5\n");
        assert_eq!(errs.len(), 3, "{errs:?}");
    }

    #[test]
    fn unclosed_bracket_points_at_both_ends() {
        let mut sm = SourceMap::default();
        let f = sm.add("t", "fn f() {\n    g(1\n}\n");
        let err = parse(f, &sm.get(f).text, &mut Interner::new(), &mut vec![]).unwrap_err();
        let d = &err.errors[0];
        assert_eq!(d.message, "mismatched `}`");
        assert_eq!(d.labels.len(), 2);
    }

    #[test]
    fn design_examples_parse() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/design");
        for file in ["normalize.bdg", "batch_jobs.bdg", "prefix_views.bdg", "service/src/main.bdg"] {
            let src = std::fs::read_to_string(format!("{root}/{file}")).unwrap();
            let mut sm = SourceMap::default();
            let f = sm.add(file, src);
            let mut diags = vec![];
            if let Err(e) = parse(f, &sm.get(f).text, &mut Interner::new(), &mut diags) {
                panic!("{file}: {:?}", e.errors);
            }
            assert!(diags.is_empty(), "{file}: {diags:?}");
        }
    }
}
