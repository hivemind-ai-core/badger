//! The grammar: chumsky parsers over the token buffer (see
//! [`crate::token`]), producing the syntax tree of [`crate::cst`].
//!
//! The rules follow `spec/grammar.ebnf`. Separators are [`Tok::Sep`]
//! (statement-ending newlines) and `;`; inside aggregate literals and
//! declarations, `,` too. Statements and declarations recover from syntax
//! errors: a statement that does not parse is skipped to the next
//! separator (keeping brackets balanced) and becomes an error node, so one
//! pass reports every broken statement.

// combinators are cloned uniformly (which ones are `Copy` depends on their
// type parameters)
#![allow(clippy::clone_on_copy, clippy::needless_borrows_for_generic_args)]

use chumsky::input::ValueInput;
use chumsky::prelude::*;

use crate::cst::*;
use crate::token::{Span, Tok, is_reserved};

/// The error of the reporting parse ([`program`] with rich errors).
pub type Error<'t, 's> = Rich<'t, Tok<'s>, Span>;

/// What the grammar needs of an error type: [`Error`] for reporting, or
/// chumsky's `EmptyErr` for the fast first pass (which tracks nothing).
pub trait ParseErr<'t, 's: 't, I: chumsky::input::Input<'t>>:
    chumsky::error::Error<'t, I>
    + chumsky::label::LabelError<'t, I, chumsky::error::RichPattern<'t, Tok<'s>>>
    + chumsky::label::LabelError<'t, I, &'static str>
{
}

impl<'t, 's: 't, I: chumsky::input::Input<'t>, T> ParseErr<'t, 's, I> for T where
    T: chumsky::error::Error<'t, I>
        + chumsky::label::LabelError<'t, I, chumsky::error::RichPattern<'t, Tok<'s>>>
        + chumsky::label::LabelError<'t, I, &'static str>
{
}

fn first_lower(w: &str) -> bool {
    let b = w.as_bytes();
    b[0].is_ascii_lowercase() || (b[0] == b'_' && b.get(1).is_some_and(u8::is_ascii_lowercase))
}

/// `name`: any identifier-shaped word but `_` and the reserved words
/// (case rules are checked when lowering).
fn is_name(w: &str) -> bool {
    !is_reserved(w) && (w.as_bytes()[0].is_ascii_alphabetic() || first_lower(w))
}

fn is_value_ident(w: &str) -> bool {
    !is_reserved(w) && first_lower(w)
}

fn is_type_ident(w: &str) -> bool {
    w.as_bytes()[0].is_ascii_uppercase()
}

/// After a dot, or as a field or label, a keyword may be a name.
fn is_member(w: &str) -> bool {
    w.as_bytes()[0].is_ascii_alphabetic() || first_lower(w)
}

/// An expected-found error for the custom parsers: `what` was expected
/// (nothing, when another alternative will say what), `found` was found.
fn expected<'t, 's: 't, I, Er>(what: Option<&'static str>, found: Option<Tok<'s>>, span: Span) -> Er
where
    I: ValueInput<'t, Token = Tok<'s>, Span = Span>,
    Er: ParseErr<'t, 's, I>,
{
    use chumsky::label::LabelError;
    use chumsky::util::MaybeRef;
    let patterns = what.map(|w| chumsky::error::RichPattern::Label(w.into()));
    <Er as LabelError<'t, I, chumsky::error::RichPattern<'t, Tok<'s>>>>::expected_found(
        patterns,
        found.map(MaybeRef::Val),
        span,
    )
}

/// The phrase the errors use for a body that must be a block. The grammar
/// and the reporter share it, so the message stays in one piece.
pub const BLOCK_BODY: &str = "a block `{ ... }`";

/// An expected-found error whose expectation is a whole phrase, built at
/// run time (the body of an `if` says which `if`). The phrase is `None` in
/// the fast pass, whose error type keeps nothing.
fn expected_phrase<'t, 's: 't, I, Er>(what: Option<String>, found: Option<Tok<'s>>, span: Span) -> Er
where
    I: ValueInput<'t, Token = Tok<'s>, Span = Span>,
    Er: ParseErr<'t, 's, I>,
{
    use chumsky::label::LabelError;
    use chumsky::util::MaybeRef;
    let patterns = what.into_iter().map(|w| chumsky::error::RichPattern::Label(std::borrow::Cow::Owned(w)));
    <Er as LabelError<'t, I, chumsky::error::RichPattern<'t, Tok<'s>>>>::expected_found(
        patterns,
        found.map(MaybeRef::Val),
        span,
    )
}

/// The body of a control-flow construct: a block, always. A statement
/// without braces (`if x return`) is a common slip, so the error names the
/// block that is missing, at the token that should have opened it.
fn body_block<'t, 's: 't, I, Er>(
    block: impl Parser<'t, I, Block<'s>, extra::Err<Er>> + Clone,
    after: &'static str,
) -> impl Parser<'t, I, Block<'s>, extra::Err<Er>> + Clone
where
    I: ValueInput<'t, Token = Tok<'s>, Span = Span>,
    Er: ParseErr<'t, 's, I> + 't,
{
    custom(move |inp: &mut chumsky::input::InputRef<'t, '_, I, extra::Err<Er>>| {
        if matches!(inp.peek(), Some(Tok::LBrace)) {
            return inp.parse(&block);
        }
        let c = inp.cursor();
        let save = inp.save();
        inp.next();
        let at = inp.span_since(&c);
        inp.rewind(save);
        // the fast pass tracks no errors, so it needs no phrase
        let what = (core::mem::size_of::<Er>() != 0).then(|| format!("{BLOCK_BODY} {after}"));
        Err(expected_phrase::<I, Er>(what, inp.peek(), at))
    })
}

/// Items, then the end of input.
pub fn program<'t, 's: 't, I, Er>() -> impl Parser<'t, I, Vec<Item<'s>>, extra::Err<Er>> + Clone
where
    I: ValueInput<'t, Token = Tok<'s>, Span = Span>,
    Er: ParseErr<'t, 's, I> + 't,
{
    // The fast pass (an error type that records nothing) dispatches on the
    // next token in custom parsers; the reporting pass uses combinators,
    // whose errors say where they are.
    #[allow(non_snake_case)]
    let FAST = core::mem::size_of::<Er>() == 0;

    // ------------------------------------------------------------ tokens
    let kw = |w: &'static str| just(Tok::Word(w)).ignored();
    let ident = |f: fn(&str) -> bool, what: &'static str| {
        any()
            .filter(move |t: &Tok<'s>| matches!(t, Tok::Word(w) if f(w)))
            .map_with(|t, e| match t {
                Tok::Word(text) => Ident { text, span: e.span() },
                _ => unreachable!(),
            })
            .labelled(what)
    };
    let name = ident(is_name, "identifier");
    let value_ident = ident(is_value_ident, "identifier");
    let type_ident = ident(is_type_ident, "type name");
    let member = any()
        .filter(|t: &Tok<'s>| match t {
            Tok::Word(w) => is_member(w),
            Tok::Int(n) => n.bytes().all(|c| c.is_ascii_digit()),
            _ => false,
        })
        .map_with(|t, e| match t {
            Tok::Word(text) | Tok::Int(text) => Ident { text, span: e.span() },
            _ => unreachable!(),
        })
        .labelled("field name");
    let discard = kw("_");
    let sep = just(Tok::Sep).ignored();
    let sepc = choice((just(Tok::Comma), just(Tok::Sep), just(Tok::Semi))).ignored();
    let nls = just(Tok::Sep).repeated();
    let csep = nls.clone().then(just(Tok::Comma)).then(nls.clone()).ignored();
    let trail = nls.clone().then(just(Tok::Comma).then(nls.clone()).or_not()).ignored();

    // `#name` or `#ns/name` (one token)
    let symbol = select! { Tok::Symbol(s) => s }.map_with(|s, e| (s, e.span())).labelled("symbol");

    // `@name` / `@name(a, b)` / `@name(key = value)`
    let attr_word = any()
        .filter(|t: &Tok<'s>| matches!(t, Tok::Word(_)))
        .map_with(|t, e| match t {
            Tok::Word(text) => Ident { text, span: e.span() },
            _ => unreachable!(),
        })
        .labelled("attribute name");
    let attr_scalar = choice((
        select! { Tok::Symbol(s) => s }.map_with(|s, e| AttrArg::Symbol(s, e.span())),
        select! { Tok::Str(s) => s }.map_with(|s, e| AttrArg::Str(s, e.span())),
        select! { Tok::Int(n) => n }.map_with(|n, e| AttrArg::Int(n, e.span())),
        select! { Tok::Number(n) => n }.map_with(|n, e| AttrArg::Int(n, e.span())),
        select! { Tok::Float(n) => n }.map_with(|n, e| AttrArg::Float(n, false, e.span())),
        select! { Tok::Duration(n) => n }.map_with(|n, e| AttrArg::Duration(n, e.span())),
        just(Tok::Minus).ignore_then(choice((
            select! { Tok::Int(n) => n }.map_with(|n, e| AttrArg::NegInt(n, e.span())),
            select! { Tok::Number(n) => n }.map_with(|n, e| AttrArg::NegInt(n, e.span())),
            select! { Tok::Float(n) => n }.map_with(|n, e| AttrArg::Float(n, true, e.span())),
        ))),
        attr_word.clone().map(AttrArg::Name),
    ));
    // `[a, b]`: a list of values (no nesting)
    let attr_value = choice((
        attr_scalar
            .clone()
            .separated_by(just(Tok::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(just(Tok::LBracket), just(Tok::RBracket))
            .map_with(|xs, e| AttrArg::List(xs, e.span())),
        attr_scalar,
    ));
    let attr_arg = attr_word
        .clone()
        .then_ignore(just(Tok::Assign))
        .then(attr_value.clone())
        .map(|(k, v)| AttrArg::KeyValue(k, Box::new(v)))
        .or(attr_value.clone());
    // `@lib.name`: a library's attribute, named after its module
    let attr_path = attr_word
        .clone()
        .then(just(Tok::Dot).ignore_then(attr_word.clone()).repeated().collect::<Vec<_>>())
        .map(|(first, rest)| {
            let mut all = vec![first];
            all.extend(rest);
            let name = all.pop().unwrap();
            (all, name)
        });
    let attribute = just(Tok::At)
        .ignore_then(attr_path)
        .then(
            just(Tok::LParen)
                .ignore_then(attr_arg.separated_by(just(Tok::Comma)).allow_trailing().collect::<Vec<_>>())
                .then_ignore(just(Tok::RParen))
                .or_not(),
        )
        .map_with(|((namespace, name), args), e| Attribute {
            namespace,
            name,
            args: args.unwrap_or_default(),
            span: e.span(),
        });
    // A run of attributes prefixes a declaration (newline-separated or not).
    let attributes = attribute.clone().then_ignore(sep.clone().repeated()).repeated().collect::<Vec<_>>();

    let lit = select! {
        Tok::Int(n) => Lit::Int(n),
        Tok::Float(n) => Lit::Float(n),
        Tok::Duration(n) => Lit::Duration(n),
        Tok::Str(s) => Lit::Str(s),
        Tok::RawStr(s) => Lit::RawStr(s),
        Tok::Char(c) => Lit::Char(c),
        Tok::Word("true") => Lit::Bool(true),
        Tok::Word("false") => Lit::Bool(false),
    }
    .or(symbol.clone().map(|(s, _)| Lit::Symbol(s)));
    let string = select! { Tok::Str(s) => Lit::Str(s), Tok::RawStr(s) => Lit::RawStr(s) }.labelled("string");

    let label = choice((
        member.clone().map(Label::Name),
        symbol
            .clone()
            .delimited_by(just(Tok::LBracket), just(Tok::RBracket))
            .map_with(|(s, _), e| Label::Symbol(s, e.span())),
    ))
    .labelled("label");
    let label_path = label.clone().separated_by(just(Tok::Dot)).at_least(1).collect::<Vec<_>>();

    // ------------------------------------------------------------ types
    let mut ty = Recursive::declare();
    let effect = just(Tok::Bang)
        .ignore_then(choice((
            ty.clone()
                .separated_by(just(Tok::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(Tok::LBrace), just(Tok::RBrace))
                .map(Ok),
            type_ident.clone().map(Err),
        )))
        .map_with(|x: Result<Vec<T<'s>>, Ident<'s>>, e| match x {
            Ok(ts) => Effect::Set(ts, e.span()),
            Err(v) => Effect::Var(v),
        })
        .labelled("effect");
    let generics = {
        let param = name
            .clone()
            .then(
                just(Tok::Colon)
                    .ignore_then(ty.clone().separated_by(just(Tok::Plus)).at_least(1).collect())
                    .or_not(),
            )
            .map(|(name, bounds): (Ident<'s>, Option<Vec<T<'s>>>)| Generic {
                name,
                bounds: bounds.unwrap_or_default(),
            });
        param
            .separated_by(just(Tok::Comma))
            .allow_trailing()
            .at_least(1)
            .collect::<Vec<_>>()
            .delimited_by(just(Tok::Lt), just(Tok::Gt))
    };
    let type_args = choice((effect.clone().map_with(|f, e| (Ty::Effect(f), e.span())), ty.clone()))
        .separated_by(just(Tok::Comma))
        .allow_trailing()
        .at_least(1)
        .collect::<Vec<_>>()
        .delimited_by(just(Tok::Lt), just(Tok::Gt));
    let path_type = value_ident
        .clone()
        .then_ignore(just(Tok::Dot))
        .repeated()
        .collect::<Vec<_>>()
        .then(type_ident.clone())
        .then(just(Tok::Dot).ignore_then(type_ident.clone()).or_not())
        .then(type_args.clone().or_not())
        .map_with(|(((module, first), second), args), e| {
            let mut names = vec![first];
            names.extend(second);
            (Ty::Path { module, names, args: args.unwrap_or_default() }, e.span())
        });
    let where_clause = kw("where").ignore_then(
        ty.clone()
            .then_ignore(just(Tok::Colon))
            .then(ty.clone().separated_by(just(Tok::Plus)).at_least(1).collect::<Vec<_>>())
            .map(|(ty, bounds)| WherePred { ty, bounds })
            .separated_by(just(Tok::Comma))
            .at_least(1)
            .collect::<Vec<_>>(),
    );
    {
        let once = kw("once").or_not().map(|o| o.is_some());
        let fn_type = once
            .clone()
            .then_ignore(kw("fn"))
            .then(
                ty.clone()
                    .separated_by(just(Tok::Comma))
                    .allow_trailing()
                    .collect::<Vec<_>>()
                    .delimited_by(just(Tok::LParen), just(Tok::RParen)),
            )
            .then_ignore(just(Tok::Arrow))
            .then(ty.clone())
            .then(effect.clone().or_not())
            .map(|(((once, params), ret), effect)| Ty::Fn { once, params, ret: Box::new(ret), effect });
        let forall = kw("for")
            .ignore_then(generics.clone())
            .then(ty.clone())
            .map(|(generics, body)| Ty::Forall { generics, body: Box::new(body) });
        let reference = just(Tok::Amp)
            .ignore_then(kw("mut").or_not())
            .then(ty.clone())
            .map(|(m, inner)| Ty::Ref { mutable: m.is_some(), inner: Box::new(inner) });
        let dyn_ty = kw("dyn").ignore_then(path_type.clone()).map(|p| Ty::Dyn(Box::new(p)));
        let field = attributes
            .clone()
            .then(label.clone())
            .then_ignore(just(Tok::Colon))
            .then(ty.clone())
            .map(|((attrs, l), t)| (attrs, l, t));
        let row_rest = just(Tok::DotDot).ignore_then(type_ident.clone());
        let record = just(Tok::LBrace)
            .ignore_then(nls.clone())
            .ignore_then(
                choice((
                    field
                        .clone()
                        .then(csep.clone().ignore_then(field.clone()).repeated().collect::<Vec<_>>())
                        .then(csep.clone().ignore_then(row_rest.clone()).or_not())
                        .map(|((f, mut fs), rest)| {
                            fs.insert(0, f);
                            (fs, rest)
                        }),
                    row_rest.clone().map(|r| (vec![], Some(r))),
                ))
                .or_not(),
            )
            .then_ignore(trail.clone())
            .then_ignore(just(Tok::RBrace))
            .map(|r| {
                let (fields, rest) = r.unwrap_or_default();
                Ty::Record { fields, rest }
            });
        let unit = just(Tok::LParen).then(just(Tok::RParen)).to(Ty::Unit);
        let group = just(Tok::LParen)
            .ignore_then(ty.clone())
            .then(
                just(Tok::Comma)
                    .ignore_then(
                        ty.clone().separated_by(just(Tok::Comma)).allow_trailing().collect::<Vec<_>>(),
                    )
                    .or_not(),
            )
            .then_ignore(just(Tok::RParen));
        ty.define(
            choice((
                discard.clone().to(Ty::Hole).map_with(|t, e| (t, e.span())),
                fn_type.map_with(|t, e| (t, e.span())),
                forall.map_with(|t, e| (t, e.span())),
                reference.map_with(|t, e| (t, e.span())),
                dyn_ty.map_with(|t, e| (t, e.span())),
                record.map_with(|t, e| (t, e.span())),
                unit.map_with(|t, e| (t, e.span())),
                group.map_with(|(first, rest), e| match rest {
                    None => first,
                    Some(mut rest) => {
                        rest.insert(0, first);
                        (Ty::Tuple(rest), e.span())
                    }
                }),
                path_type.clone(),
            ))
            .labelled("type")
            .boxed(),
        );
    }

    // ------------------------------------------------------------ patterns
    let pattern = recursive(|pat| {
        let lit_pat = choice((
            just(Tok::Minus)
                .or_not()
                .then(select! { Tok::Int(n) => Lit::Int(n), Tok::Float(n) => Lit::Float(n) })
                .map(|(neg, lit)| Pat::Lit { negative: neg.is_some(), lit }),
            lit.clone().map(|lit| Pat::Lit { negative: false, lit }),
        ));
        let unit = just(Tok::LParen).then(just(Tok::RParen)).to(Pat::Unit);
        let group = just(Tok::LParen)
            .ignore_then(pat.clone())
            .then(
                just(Tok::Comma)
                    .ignore_then(
                        pat.clone().separated_by(just(Tok::Comma)).allow_trailing().collect::<Vec<_>>(),
                    )
                    .or_not(),
            )
            .then_ignore(just(Tok::RParen));
        let field = label.clone().then(just(Tok::Colon).ignore_then(pat.clone()).or_not());
        let rest = just(Tok::DotDot);
        let record = just(Tok::LBrace)
            .ignore_then(nls.clone())
            .ignore_then(
                choice((
                    field
                        .clone()
                        .then(csep.clone().ignore_then(field.clone()).repeated().collect::<Vec<_>>())
                        .then(csep.clone().ignore_then(rest.clone()).or_not())
                        .map(|((f, mut fs), r)| {
                            fs.insert(0, f);
                            (fs, r.is_some())
                        }),
                    rest.clone().to((vec![], true)),
                ))
                .or_not(),
            )
            .then_ignore(trail.clone())
            .then_ignore(just(Tok::RBrace))
            .map(|r| {
                let (fields, open) = r.unwrap_or_default();
                Pat::Record { fields, open }
            });
        let variant_type = value_ident
            .clone()
            .then_ignore(just(Tok::Dot))
            .repeated()
            .collect::<Vec<_>>()
            .then(type_ident.clone());
        let variant = variant_type
            .or_not()
            .then_ignore(just(Tok::Dot))
            .then(member.clone())
            .then(
                pat.clone()
                    .separated_by(just(Tok::Comma))
                    .allow_trailing()
                    .collect::<Vec<_>>()
                    .delimited_by(just(Tok::LParen), just(Tok::RParen))
                    .or_not(),
            )
            .map(|((ty, name), args)| Pat::Variant { ty, name, args });
        choice((
            discard.clone().to(Pat::Discard).map_with(|p, e| (p, e.span())),
            lit_pat.map_with(|p, e| (p, e.span())),
            unit.map_with(|p, e| (p, e.span())),
            group.map_with(|(first, rest), e| match rest {
                None => first,
                Some(mut rest) => {
                    rest.insert(0, first);
                    (Pat::Tuple(rest), e.span())
                }
            }),
            record.map_with(|p, e| (p, e.span())),
            variant.map_with(|p, e| (p, e.span())),
            name.clone().map(Pat::Bind).map_with(|p, e| (p, e.span())),
        ))
        .labelled("pattern")
        .boxed()
    });

    // ------------------------------------------------------------ expressions
    let mut expr = Recursive::declare();
    let mut block = Recursive::declare();
    let params = attributes
        .clone()
        .then(pattern.clone())
        .then(just(Tok::Colon).ignore_then(ty.clone()).or_not())
        .map(|((attributes, pat), ty)| Param { attributes, pat, ty })
        .separated_by(just(Tok::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Tok::LParen), just(Tok::RParen));
    let ret = just(Tok::Arrow).ignore_then(ty.clone());
    // `{ NL* (ITEM (, ITEM)*)? NL* ,? NL* }`
    fn braced<'t, 's: 't, I, O, Er>(
        item: impl Parser<'t, I, O, extra::Err<Er>> + Clone,
    ) -> impl Parser<'t, I, Vec<O>, extra::Err<Er>> + Clone
    where
        I: ValueInput<'t, Token = Tok<'s>, Span = Span>,
        Er: ParseErr<'t, 's, I> + 't,
    {
        let nls = just(Tok::Sep).repeated();
        let csep = nls.clone().then(just(Tok::Comma)).then(nls.clone()).ignored();
        let trail = nls.clone().then(just(Tok::Comma).then(nls.clone()).or_not()).ignored();
        just(Tok::LBrace)
            .ignore_then(nls)
            .ignore_then(
                item.clone()
                    .then(csep.ignore_then(item).repeated().collect::<Vec<_>>())
                    .map(|(a, mut v)| {
                        v.insert(0, a);
                        v
                    })
                    .or_not(),
            )
            .then_ignore(trail)
            .then_ignore(just(Tok::RBrace))
            .map(Option::unwrap_or_default)
    }

    let cond = choice((
        kw("let")
            .ignore_then(pattern.clone())
            .then_ignore(just(Tok::Assign))
            .then(expr.clone())
            .map(|(p, e)| Cond::Let(p, Box::new(e))),
        expr.clone().map(|e| Cond::Expr(Box::new(e))),
    ));

    let primary = {
        let interp = select! { Tok::InterpStart(raw) => raw }
            .then(
                choice((
                    select! { Tok::InterpText(t) => t }.map_with(|t, e| InterpPart::Text(t, e.span())),
                    expr.clone().delimited_by(just(Tok::HoleStart), just(Tok::HoleEnd)).map(InterpPart::Hole),
                ))
                .repeated()
                .collect::<Vec<_>>(),
            )
            .then_ignore(just(Tok::InterpEnd))
            .map(|(raw, parts)| Expr::Interp(raw, parts));
        let lambda = kw("once")
            .or_not()
            .then_ignore(kw("fn"))
            .then(params.clone())
            .then(ret.clone().or_not())
            .then(effect.clone().or_not())
            .then(choice((
                block.clone().map_with(|b, e| (Expr::Block(b), e.span())),
                just(Tok::FatArrow).ignore_then(expr.clone()),
            )))
            .map(|((((once, params), ret), effect), body)| {
                Expr::Lambda(Lambda { once: once.is_some(), params, ret, effect, body: Box::new(body) })
            });
        let if_expr = recursive(|if_| {
            kw("if")
                .ignore_then(cond.clone())
                .then(body_block(block.clone(), "after the `if` condition"))
                .then(
                    sep.clone()
                        .or_not()
                        .ignore_then(kw("else"))
                        .ignore_then(choice((
                            if_,
                            body_block(block.clone(), "after `else`")
                                .map_with(|b, e| (Expr::Block(b), e.span())),
                        )))
                        .or_not(),
                )
                .map_with(|((cond, then), els), e| {
                    (Expr::If { cond, then, els: els.map(Box::new) }, e.span())
                })
        });
        let arm = pattern
            .clone()
            .then(just(Tok::Bar).ignore_then(pattern.clone()).repeated().collect::<Vec<_>>())
            .then(kw("if").ignore_then(expr.clone()).or_not())
            .then_ignore(just(Tok::FatArrow))
            .then(expr.clone())
            .map_with(|(((pat, alts), guard), body), e| Arm { pat, alts, guard, body, span: e.span() });
        let arms = sepc
            .clone()
            .repeated()
            .ignore_then(
                arm.clone()
                    .then(sepc.clone().repeated().at_least(1).ignore_then(arm).repeated().collect::<Vec<_>>())
                    .map(|(a, mut v): (Arm<'s>, Vec<Arm<'s>>)| {
                        v.insert(0, a);
                        v
                    })
                    .or_not(),
            )
            .then_ignore(sepc.clone().repeated())
            .delimited_by(just(Tok::LBrace), just(Tok::RBrace))
            .map(Option::unwrap_or_default);
        let match_expr = kw("match")
            .ignore_then(expr.clone())
            .then(arms)
            .map(|(s, arms)| Expr::Match { scrutinee: Box::new(s), arms });
        let while_expr = kw("while")
            .ignore_then(cond.clone())
            .then(body_block(block.clone(), "after the `while` condition"))
            .map(|(cond, body)| Expr::While { cond, body });
        let for_expr = kw("for")
            .ignore_then(pattern.clone())
            .then_ignore(kw("in"))
            .then(expr.clone())
            .then(body_block(block.clone(), "after the `for` header"))
            .map(|((pat, iter), body)| Expr::For { pat, iter: Box::new(iter), body });
        let loop_expr = kw("loop").ignore_then(body_block(block.clone(), "after `loop`")).map(Expr::Loop);
        let system = kw("system")
            .ignore_then(braced(label.clone().then_ignore(just(Tok::Colon)).then(expr.clone())))
            .map(Expr::System);
        let pipeline = kw("pipeline").ignore_then(braced(expr.clone())).map(Expr::Pipeline);
        let map_lit = kw("map")
            .ignore_then(braced(expr.clone().then_ignore(just(Tok::FatArrow)).then(expr.clone())))
            .map(Expr::Map);
        let set_lit = kw("set").ignore_then(braced(expr.clone())).map(Expr::Set);
        let ref_expr = kw("ref")
            .ignore_then(symbol.clone().delimited_by(just(Tok::LParen), just(Tok::RParen)))
            .map(|(s, span)| Expr::Ref(s, span));
        let unsafe_expr =
            kw("unsafe").ignore_then(body_block(block.clone(), "after `unsafe`")).map(Expr::Unsafe);
        let region_expr = kw("region")
            .ignore_then(type_ident.clone())
            .then(body_block(block.clone(), "after the region's name"))
            .map(|(n, b)| Expr::Region(n, b));
        let return_expr =
            kw("return").ignore_then(expr.clone().or_not()).map(|e| Expr::Return(e.map(Box::new)));
        let break_expr = kw("break").ignore_then(expr.clone().or_not()).map(|e| Expr::Break(e.map(Box::new)));
        let continue_expr = kw("continue").to(Expr::Continue);

        // `{` starts a record when what follows reads as one
        let record_head = choice((
            just(Tok::DotDot).ignored(),
            label_path.clone().then(just(Tok::Colon)).ignored(),
            value_ident
                .clone()
                .then(choice((
                    just(Tok::Comma).ignored(),
                    sepc.clone().repeated().then(just(Tok::RBrace)).ignored(),
                )))
                .ignored(),
        ));
        let entry = choice((
            just(Tok::DotDot).ignore_then(expr.clone()).map_with(|e, x| RecordEntry::Spread(e, x.span())),
            label_path
                .clone()
                .then_ignore(just(Tok::Colon))
                .then(expr.clone())
                .map(|(p, v)| RecordEntry::Field(p, v)),
            value_ident.clone().map(RecordEntry::Shorthand),
        ));
        let record = choice((
            just(Tok::LBrace).then(nls.clone()).then(just(Tok::RBrace)).to(Expr::Record(vec![])),
            just(Tok::LBrace)
                .ignore_then(nls.clone())
                .ignore_then(record_head.rewind())
                .ignore_then(
                    entry.clone().then(csep.clone().ignore_then(entry).repeated().collect::<Vec<_>>()).map(
                        |(a, mut v)| {
                            v.insert(0, a);
                            v
                        },
                    ),
                )
                .then_ignore(trail.clone())
                .then_ignore(just(Tok::RBrace))
                .map(Expr::Record),
        ));
        let unit = just(Tok::LParen).then(just(Tok::RParen)).to(Expr::Unit);
        let group = just(Tok::LParen)
            .ignore_then(expr.clone())
            .then(
                just(Tok::Comma)
                    .ignore_then(
                        expr.clone().separated_by(just(Tok::Comma)).allow_trailing().collect::<Vec<_>>(),
                    )
                    .or_not(),
            )
            .then_ignore(just(Tok::RParen))
            .map(|(first, rest)| match rest {
                None => Expr::Paren(Box::new(first)),
                Some(mut rest) => {
                    rest.insert(0, first);
                    Expr::Tuple(rest)
                }
            });
        let list = expr
            .clone()
            .separated_by(just(Tok::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(just(Tok::LBracket), just(Tok::RBracket))
            .map(Expr::List);
        // dispatch on the first token (trying every alternative in turn
        // made a plain name, the most common operand, the slowest)
        let lit = lit.clone().map(Expr::Lit);
        let if_expr = if_expr.map(|(e, _)| e);
        let block_e = block.clone().map(Expr::Block);
        let name_e = value_ident.clone().map(Expr::Name);
        let hole = kw("_").to(Expr::Hole);
        let type_e = type_ident.clone().map(Expr::TypeName);
        let brace = choice((record, block_e));
        let paren = choice((unit, group));
        // (each falls back to a plain name: these words are contextual; the
        // reporting pass below tries them all before `name_e`)
        let system_or_name = choice((system.clone(), name_e.clone()));
        let pipeline_or_name = choice((pipeline.clone(), name_e.clone()));
        let map_or_name = choice((map_lit.clone(), name_e.clone()));
        let set_or_name = choice((set_lit.clone(), name_e.clone()));
        let ref_or_name = choice((ref_expr.clone(), name_e.clone()));
        let region_or_name = choice((region_expr.clone(), name_e.clone()));
        // the reporting pass tries the alternatives as combinators (a
        // custom parser reports its errors where it started, not where
        // they are)
        let rich = choice((
            interp.clone(),
            lit.clone(),
            lambda.clone(),
            if_expr.clone(),
            match_expr.clone(),
            while_expr.clone(),
            for_expr.clone(),
            loop_expr.clone(),
            system.clone(),
            pipeline.clone(),
            map_lit.clone(),
            set_lit.clone(),
            ref_expr.clone(),
            unsafe_expr.clone(),
            region_expr.clone(),
            return_expr.clone(),
            break_expr.clone(),
            continue_expr.clone(),
            brace.clone(),
            paren.clone(),
            list.clone(),
            type_e.clone(),
            hole.clone(),
            name_e.clone(),
        ))
        .map_with(|x, e| (x, e.span()))
        .labelled("expression")
        .boxed();
        let fast = custom(move |inp| {
            let found = inp.peek();
            match found {
                Some(Tok::InterpStart(_)) => inp.parse(&interp),
                Some(
                    Tok::Int(_)
                    | Tok::Float(_)
                    | Tok::Duration(_)
                    | Tok::Str(_)
                    | Tok::RawStr(_)
                    | Tok::Char(_)
                    | Tok::Symbol(_)
                    | Tok::Word("true" | "false"),
                ) => inp.parse(&lit),
                Some(Tok::Word("fn" | "once")) => inp.parse(&lambda),
                Some(Tok::Word("if")) => inp.parse(&if_expr),
                Some(Tok::Word("match")) => inp.parse(&match_expr),
                Some(Tok::Word("while")) => inp.parse(&while_expr),
                Some(Tok::Word("for")) => inp.parse(&for_expr),
                Some(Tok::Word("loop")) => inp.parse(&loop_expr),
                Some(Tok::Word("unsafe")) => inp.parse(&unsafe_expr),
                Some(Tok::Word("return")) => inp.parse(&return_expr),
                Some(Tok::Word("break")) => inp.parse(&break_expr),
                Some(Tok::Word("continue")) => inp.parse(&continue_expr),
                Some(Tok::Word("system")) => inp.parse(&system_or_name),
                Some(Tok::Word("pipeline")) => inp.parse(&pipeline_or_name),
                Some(Tok::Word("map")) => inp.parse(&map_or_name),
                Some(Tok::Word("set")) => inp.parse(&set_or_name),
                Some(Tok::Word("ref")) => inp.parse(&ref_or_name),
                Some(Tok::Word("region")) => inp.parse(&region_or_name),
                Some(Tok::Word("_")) => inp.parse(&hole),
                Some(Tok::Word(w)) if is_type_ident(w) => inp.parse(&type_e),
                Some(Tok::Word(_)) => inp.parse(&name_e),
                Some(Tok::LBrace) => inp.parse(&brace),
                Some(Tok::LParen) => inp.parse(&paren),
                Some(Tok::LBracket) => inp.parse(&list),
                _ => {
                    let c = inp.cursor();
                    let save = inp.save();
                    inp.next();
                    let at = inp.span_since(&c);
                    inp.rewind(save);
                    Err(expected::<I, Er>(Some("expression"), found, at))
                }
            }
        })
        .map_with(|x, e| (x, e.span()))
        .labelled("expression")
        .boxed();
        if FAST { fast } else { rich }
    };

    #[derive(Clone)]
    enum Post<'t, 's> {
        Call(Vec<T<'s>>, Vec<E<'s>>),
        Field(Ident<'s>),
        Index(E<'s>),
        Try,
        #[allow(dead_code)]
        Phantom(std::marker::PhantomData<&'t ()>),
    }
    let call = type_args
        .clone()
        .or_not()
        .then(
            expr.clone()
                .separated_by(just(Tok::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(Tok::LParen), just(Tok::RParen)),
        )
        .map(|(ta, args)| Post::Call(ta.unwrap_or_default(), args));
    let field = just(Tok::Dot).ignore_then(member.clone()).map(Post::Field);
    let index = expr.clone().delimited_by(just(Tok::LBracket), just(Tok::RBracket)).map(Post::Index);
    let try_ = just(Tok::Question).to(Post::Try);
    let postop = choice((call, field, index, try_)).map_with(|p, e| (p, e.span()));

    // Operators by precedence climbing over the token buffer: prefix
    // operators, an operand, its postfix operators, then infix operators by
    // binding power (all left-associative). Postfix operators are tried only
    // where one can start, and infix ones are found by a table lookup.
    type In<'t, 'p, I, Er> = chumsky::input::InputRef<'t, 'p, I, extra::Err<Er>>;

    /// The infix operator at the input (`>>` and `>=` are two adjacent
    /// tokens): the operator, its binding power, and its length in tokens.
    fn peek_infix<'t, 's: 't, I, Er>(inp: &mut In<'t, '_, I, Er>) -> Option<(BinOp, u8, usize)>
    where
        I: ValueInput<'t, Token = Tok<'s>, Span = Span>,
        Er: ParseErr<'t, 's, I> + 't,
    {
        Some(match inp.peek()? {
            Tok::Pipe => (BinOp::Pipe, 1, 1),
            Tok::OrOr => (BinOp::Or, 2, 1),
            Tok::AndAnd => (BinOp::And, 3, 1),
            Tok::EqEq => (BinOp::Eq, 4, 1),
            Tok::Ne => (BinOp::Ne, 4, 1),
            Tok::Le => (BinOp::Le, 4, 1),
            Tok::Lt => (BinOp::Lt, 4, 1),
            Tok::Gt => {
                let save = inp.save();
                let c = inp.cursor();
                inp.next();
                let a: Span = inp.span_since(&c);
                let c = inp.cursor();
                let next = inp.next();
                let b: Span = inp.span_since(&c);
                inp.rewind(save);
                match next {
                    Some(Tok::Gt) if a.end == b.start => (BinOp::Shr, 9, 2),
                    Some(Tok::Assign) if a.end == b.start => (BinOp::Ge, 4, 2),
                    _ => (BinOp::Gt, 4, 1),
                }
            }
            Tok::DotDot => (BinOp::Range, 5, 1),
            Tok::DotDotEq => (BinOp::RangeIncl, 5, 1),
            Tok::Bar => (BinOp::BitOr, 6, 1),
            Tok::Caret => (BinOp::BitXor, 7, 1),
            Tok::Amp => (BinOp::BitAnd, 8, 1),
            Tok::Shl => (BinOp::Shl, 9, 1),
            Tok::Plus => (BinOp::Add, 10, 1),
            Tok::Minus => (BinOp::Sub, 10, 1),
            Tok::Star => (BinOp::Mul, 11, 1),
            Tok::Slash => (BinOp::Div, 11, 1),
            Tok::Percent => (BinOp::Rem, 11, 1),
            _ => return None,
        })
    }

    /// Prefix operators, an operand, and its postfix operators.
    fn unary<'t, 's: 't, I, Er, A, B>(inp: &mut In<'t, '_, I, Er>, atom: &A, post: &B) -> Result<E<'s>, Er>
    where
        I: ValueInput<'t, Token = Tok<'s>, Span = Span>,
        Er: ParseErr<'t, 's, I> + 't,
        A: Parser<'t, I, E<'s>, extra::Err<Er>>,
        B: Parser<'t, I, (Post<'t, 's>, Span), extra::Err<Er>>,
    {
        let before = inp.cursor();
        let op = match inp.peek() {
            Some(Tok::Minus) => Some(UnOp::Neg),
            Some(Tok::Bang) => Some(UnOp::Not),
            Some(Tok::Amp) => Some(UnOp::Ref),
            _ => None,
        };
        if let Some(mut op) = op {
            inp.next();
            if op == UnOp::Ref {
                let save = inp.save();
                if inp.next() == Some(Tok::Word("mut")) {
                    op = UnOp::RefMut;
                } else {
                    inp.rewind(save);
                }
            }
            let start = inp.span_since(&before).start;
            let operand = unary(inp, atom, post)?;
            let span = Span::from(start..operand.1.end);
            return Ok((Expr::Unary { op, operand: Box::new(operand) }, span));
        }
        let mut e = inp.parse(atom)?;
        while matches!(inp.peek(), Some(Tok::LParen | Tok::Lt | Tok::Dot | Tok::LBracket | Tok::Question)) {
            let save = inp.save();
            match inp.parse(post) {
                Ok((p, s)) => {
                    let span = Span::from(e.1.start..s.end);
                    let base = Box::new(e);
                    let kind = match p {
                        Post::Call(type_args, args) => Expr::Call { callee: base, type_args, args },
                        Post::Field(name) => Expr::Field { base, name },
                        Post::Index(i) => Expr::Index { base, index: Box::new(i) },
                        Post::Try => Expr::Try(base),
                        Post::Phantom(_) => unreachable!(),
                    };
                    e = (kind, span);
                }
                // `a < b` is a comparison, not type arguments
                Err(_) => {
                    inp.rewind(save);
                    break;
                }
            }
        }
        Ok(e)
    }

    /// Operands joined by infix operators binding at least `min`.
    fn climb<'t, 's: 't, I, Er, A, B>(
        inp: &mut In<'t, '_, I, Er>,
        min: u8,
        atom: &A,
        post: &B,
    ) -> Result<E<'s>, Er>
    where
        I: ValueInput<'t, Token = Tok<'s>, Span = Span>,
        Er: ParseErr<'t, 's, I> + 't,
        A: Parser<'t, I, E<'s>, extra::Err<Er>>,
        B: Parser<'t, I, (Post<'t, 's>, Span), extra::Err<Er>>,
    {
        let mut lhs = unary(inp, atom, post)?;
        while let Some((op, bp, n)) = peek_infix(inp) {
            if bp < min {
                break;
            }
            for _ in 0..n {
                inp.next();
            }
            let rhs = climb(inp, bp + 1, atom, post)?;
            let span = Span::from(lhs.1.start..rhs.1.end);
            lhs = (Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) }, span);
        }
        Ok(lhs)
    }

    let operators = {
        let (primary, postop) = (primary.clone(), postop.clone());
        custom(move |inp| climb(inp, 0, &primary, &postop)).labelled("expression").boxed()
    };
    // the reporting pass: the same operators as a Pratt parser
    let pratt = {
        use chumsky::pratt::*;
        let adjacent = |first: Tok<'s>, second: Tok<'s>| {
            any()
                .filter(move |t: &Tok<'s>| *t == first)
                .map_with(|_, e| e.span())
                .then(any().filter(move |t: &Tok<'s>| *t == second).map_with(|_, e| e.span()))
                .filter(|(a, b): &(Span, Span)| a.end == b.start)
                .ignored()
        };
        let gt_alone = just(Tok::Gt)
            .and_is(adjacent(Tok::Gt, Tok::Gt).not())
            .and_is(adjacent(Tok::Gt, Tok::Assign).not());
        let op = |t: Tok<'s>, op: BinOp| just(t).to(op);
        let bin = |lhs: E<'s>, op: BinOp, rhs: E<'s>| {
            let span = Span::from(lhs.1.start..rhs.1.end);
            (Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) }, span)
        };
        primary
            .pratt((
                postfix(13, postop, |lhs: E<'s>, (p, s): (Post<'t, 's>, Span), _| {
                    let span = Span::from(lhs.1.start..s.end);
                    let base = Box::new(lhs);
                    let e = match p {
                        Post::Call(type_args, args) => Expr::Call { callee: base, type_args, args },
                        Post::Field(name) => Expr::Field { base, name },
                        Post::Index(i) => Expr::Index { base, index: Box::new(i) },
                        Post::Try => Expr::Try(base),
                        Post::Phantom(_) => unreachable!(),
                    };
                    (e, span)
                }),
                prefix(
                    12,
                    choice((
                        just(Tok::Minus).to(UnOp::Neg),
                        just(Tok::Bang).to(UnOp::Not),
                        just(Tok::Amp).then(kw("mut")).to(UnOp::RefMut),
                        just(Tok::Amp).to(UnOp::Ref),
                    ))
                    .map_with(|o, e| (o, e.span())),
                    |(op, s): (UnOp, Span), operand: E<'s>, _| {
                        let span = Span::from(s.start..operand.1.end);
                        (Expr::Unary { op, operand: Box::new(operand) }, span)
                    },
                ),
                infix(
                    left(11),
                    choice((
                        op(Tok::Star, BinOp::Mul),
                        op(Tok::Slash, BinOp::Div),
                        op(Tok::Percent, BinOp::Rem),
                    )),
                    move |l, o, r, _| bin(l, o, r),
                ),
                infix(
                    left(10),
                    choice((op(Tok::Plus, BinOp::Add), op(Tok::Minus, BinOp::Sub))),
                    move |l, o, r, _| bin(l, o, r),
                ),
                infix(
                    left(9),
                    choice((op(Tok::Shl, BinOp::Shl), adjacent(Tok::Gt, Tok::Gt).to(BinOp::Shr))),
                    move |l, o, r, _| bin(l, o, r),
                ),
                infix(left(8), op(Tok::Amp, BinOp::BitAnd), move |l, o, r, _| bin(l, o, r)),
                infix(left(7), op(Tok::Caret, BinOp::BitXor), move |l, o, r, _| bin(l, o, r)),
                infix(left(6), op(Tok::Bar, BinOp::BitOr), move |l, o, r, _| bin(l, o, r)),
                infix(
                    left(5),
                    choice((op(Tok::DotDotEq, BinOp::RangeIncl), op(Tok::DotDot, BinOp::Range))),
                    move |l, o, r, _| bin(l, o, r),
                ),
                infix(
                    left(4),
                    choice((
                        op(Tok::EqEq, BinOp::Eq),
                        op(Tok::Ne, BinOp::Ne),
                        op(Tok::Le, BinOp::Le),
                        adjacent(Tok::Gt, Tok::Assign).to(BinOp::Ge),
                        op(Tok::Lt, BinOp::Lt),
                        gt_alone.to(BinOp::Gt),
                    )),
                    move |l, o, r, _| bin(l, o, r),
                ),
                infix(left(3), op(Tok::AndAnd, BinOp::And), move |l, o, r, _| bin(l, o, r)),
                infix(left(2), op(Tok::OrOr, BinOp::Or), move |l, o, r, _| bin(l, o, r)),
                infix(left(1), op(Tok::Pipe, BinOp::Pipe), move |l, o, r, _| bin(l, o, r)),
            ))
            .labelled("expression")
            .boxed()
    };
    expr.define(if FAST { operators } else { pratt });

    // ------------------------------------------------------------ statements
    let let_kind =
        choice((kw("let").to(LetKind::Let), kw("var").to(LetKind::Var), kw("const").to(LetKind::Const)));
    let let_stmt = let_kind
        .clone()
        .then(pattern.clone())
        .then(just(Tok::Colon).ignore_then(ty.clone()).or_not())
        .then_ignore(just(Tok::Assign))
        .then(expr.clone())
        .then(kw("else").ignore_then(body_block(block.clone(), "after `else`")).map(Box::new).or_not())
        .map_with(|((((kind, pat), ty), value), els), e| Stmt::Let {
            kind,
            pat,
            ty,
            value,
            els,
            span: e.span(),
        });
    let use_stmt = kw("use")
        .ignore_then(pattern.clone().separated_by(just(Tok::Comma)).at_least(1).collect::<Vec<_>>())
        .then_ignore(just(Tok::LArrow))
        .then(expr.clone())
        .map_with(|(pats, call), e| Stmt::Use { pats, call, span: e.span() });
    let place_elem = choice((
        just(Tok::Dot).ignore_then(member.clone()).map(PlaceElem::Field),
        expr.clone().delimited_by(just(Tok::LBracket), just(Tok::RBracket)).map(PlaceElem::Index),
    ));
    let assign_op = choice((
        just(Tok::Assign).to(None),
        just(Tok::PlusAssign).to(Some(BinOp::Add)),
        just(Tok::MinusAssign).to(Some(BinOp::Sub)),
        just(Tok::StarAssign).to(Some(BinOp::Mul)),
        just(Tok::SlashAssign).to(Some(BinOp::Div)),
        just(Tok::PercentAssign).to(Some(BinOp::Rem)),
    ));
    let assign = value_ident
        .clone()
        .then(place_elem.repeated().collect::<Vec<_>>())
        .then(assign_op)
        .then(expr.clone())
        .map_with(|(((base, path), op), value), e| Stmt::Assign { base, path, op, value, span: e.span() });
    // a statement that does not parse is skipped to the next separator
    // (or the block's `}`), keeping brackets balanced
    let skip_balanced = |stop: &'static [Tok<'static>]| {
        choice((
            nested_delimiters(
                Tok::LParen,
                Tok::RParen,
                [(Tok::LBracket, Tok::RBracket), (Tok::LBrace, Tok::RBrace)],
                |_| (),
            ),
            nested_delimiters(
                Tok::LBracket,
                Tok::RBracket,
                [(Tok::LParen, Tok::RParen), (Tok::LBrace, Tok::RBrace)],
                |_| (),
            ),
            nested_delimiters(
                Tok::LBrace,
                Tok::RBrace,
                [(Tok::LParen, Tok::RParen), (Tok::LBracket, Tok::RBracket)],
                |_| (),
            ),
            any().filter(move |t: &Tok<'s>| !stop.contains(t)).ignored(),
        ))
        .repeated()
        .at_least(1)
    };
    let expr_stmt = expr.clone().map(Stmt::Expr);
    let rich_stmt = choice((let_stmt.clone(), use_stmt.clone(), assign.clone(), expr_stmt.clone())).boxed();
    let assign_or_expr = choice((assign, expr_stmt.clone()));
    let fast_stmt = custom(move |inp| match inp.peek() {
        Some(Tok::Word("let" | "var" | "const")) => inp.parse(&let_stmt),
        Some(Tok::Word("use")) => inp.parse(&use_stmt),
        Some(Tok::Word(w)) if is_value_ident(w) => inp.parse(&assign_or_expr),
        _ => inp.parse(&expr_stmt),
    })
    .boxed();
    // a statement ends at a newline, a `;`, or the block's end: `price qty`
    // is two statements only if they are on separate lines
    let stmt_end =
        choice((sep.clone(), just(Tok::Semi).ignored().rewind(), just(Tok::RBrace).ignored().rewind()))
            .labelled("a new line or `;`");
    let stmt = if FAST { fast_stmt } else { rich_stmt }
        .labelled("statement")
        .then_ignore(stmt_end)
        .recover_with(via_parser(
            skip_balanced(&[Tok::Sep, Tok::Semi, Tok::RBrace])
                .map_with(|_, e| Stmt::Expr((Expr::Error, e.span()))),
        ));

    // ------------------------------------------------------------ items
    let is_pub = kw("pub").or_not().map(|p| p.is_some());
    let contract = choice((kw("requires").to(false), kw("ensures").to(true)))
        .then(expr.clone())
        .then(just(Tok::Comma).ignore_then(string.clone()).or_not())
        .map_with(|((ensures, cond), message), e| Contract { ensures, cond, message, span: e.span() });
    let contracts = contract.then_ignore(sep.clone().repeated().at_least(1)).repeated().collect::<Vec<_>>();
    // (a handle to the block parser, defined below with local functions)
    let fn_block = block.clone();
    let fn_decl = |fn_name: chumsky::Boxed<'t, 't, I, Ident<'s>, extra::Err<Er>>| {
        contracts
            .clone()
            .then(is_pub.clone())
            .then_ignore(kw("fn"))
            .then(fn_name)
            .then(generics.clone().or_not())
            .then(params.clone())
            .then(ret.clone().or_not())
            .then(effect.clone().or_not())
            .then(where_clause.clone().or_not())
            // `=> expr` is a body of one expression, as a lambda's
            .then(
                choice((
                    fn_block.clone(),
                    just(Tok::FatArrow)
                        .ignore_then(expr.clone())
                        .map(|e: E<'s>| Block { span: e.1, items: vec![BlockItem::Stmt(Stmt::Expr(e))] }),
                ))
                .or_not(),
            )
            .map_with(
                |(
                    (((((((contracts, is_pub), name), generics), params), ret), effect), where_preds),
                    body,
                ),
                 e| FnDecl {
                    attributes: vec![],
                    contracts,
                    is_pub,
                    name,
                    generics: generics.unwrap_or_default(),
                    params,
                    ret,
                    effect,
                    where_preds: where_preds.unwrap_or_default(),
                    body,
                    span: e.span(),
                },
            )
    };
    // a local function: `fn name<T>(...) { ... }` inside a body (a lambda is
    // `fn(`, so the name tells them apart)
    let local_fn = fn_decl(name.clone().boxed())
        .then_ignore(
            choice((sep.clone(), just(Tok::Semi).ignored().rewind(), just(Tok::RBrace).ignored().rewind()))
                .labelled("a new line or `;`"),
        )
        .map(|d| Stmt::Fn(Box::new(d)));
    // a statement's or local function's attributes, on the lines before it
    let tagged = |tags: Vec<Attribute<'s>>, s: Stmt<'s>| match s {
        _ if tags.is_empty() => s,
        Stmt::Fn(mut d) => {
            d.attributes = tags;
            Stmt::Fn(d)
        }
        s => Stmt::Attributed(tags, Box::new(s)),
    };
    block.define(
        choice((
            sep.clone().to(None),
            just(Tok::Semi).to(Some(BlockItem::Semi)),
            attributes
                .clone()
                .then(choice((local_fn, stmt)))
                .map(move |(tags, s)| Some(BlockItem::Stmt(tagged(tags, s)))),
        ))
        .repeated()
        .collect::<Vec<_>>()
        .delimited_by(just(Tok::LBrace), just(Tok::RBrace))
        .map_with(|items, e| Block { items: items.into_iter().flatten().collect(), span: e.span() })
        .labelled("block")
        .boxed(),
    );
    let assoc_type =
        kw("type").ignore_then(name.clone()).then(just(Tok::Assign).ignore_then(ty.clone()).or_not());
    // members on one line are separated, as statements are
    let member_end =
        choice((sep.clone(), just(Tok::Semi).ignored().rewind(), just(Tok::RBrace).ignored().rewind()))
            .labelled("a new line or `;`");
    let members = choice((
        sep.clone().to(None),
        just(Tok::Semi).to(None),
        attributes.clone().then(fn_decl(member.clone().boxed())).then_ignore(member_end.clone()).map(
            |(tags, mut f)| {
                f.attributes = tags;
                Some(Member::Fn(f))
            },
        ),
        assoc_type.then_ignore(member_end.clone()).map(|(n, t)| Some(Member::AssocType(n, t))),
    ))
    .repeated()
    .collect::<Vec<_>>()
    .delimited_by(just(Tok::LBrace), just(Tok::RBrace))
    .map(|ms| ms.into_iter().flatten().collect::<Vec<_>>());
    // `{ SEPC* (ITEM (SEPC+ ITEM)*)? SEPC* }`
    fn sepc_list<'t, 's: 't, I, O, Er>(
        item: impl Parser<'t, I, O, extra::Err<Er>> + Clone,
    ) -> impl Parser<'t, I, Vec<O>, extra::Err<Er>> + Clone
    where
        I: ValueInput<'t, Token = Tok<'s>, Span = Span>,
        Er: ParseErr<'t, 's, I> + 't,
    {
        let sepc = choice((just(Tok::Comma), just(Tok::Sep), just(Tok::Semi))).ignored();
        sepc.clone()
            .repeated()
            .ignore_then(
                item.clone()
                    .then(
                        sepc.clone().repeated().at_least(1).ignore_then(item).repeated().collect::<Vec<_>>(),
                    )
                    .map(|(a, mut v)| {
                        v.insert(0, a);
                        v
                    })
                    .or_not(),
            )
            .then_ignore(sepc.repeated())
            .delimited_by(just(Tok::LBrace), just(Tok::RBrace))
            .map(Option::unwrap_or_default)
    }

    let import = kw("import")
        .ignore_then(name.clone().separated_by(just(Tok::Dot)).at_least(1).collect::<Vec<_>>())
        .then(choice((
            kw("as").ignore_then(name.clone()).map(|a| (Some(a), None)),
            just(Tok::Dot)
                .ignore_then(sepc_list(name.clone().then(kw("as").ignore_then(name.clone()).or_not())))
                .map(|items| (None, Some(items))),
            empty().to((None, None)),
        )))
        .map(|(path, (alias, items))| (false, ItemKind::Import { path, alias, items }));
    let extern_type = is_pub
        .clone()
        .then_ignore(kw("extern"))
        .then_ignore(kw("type"))
        .then(name.clone())
        .then(generics.clone().or_not())
        .map(|((p, name), g)| (p, ItemKind::ExternType { name, generics: g.unwrap_or_default() }));
    let type_decl = is_pub
        .clone()
        .then_ignore(kw("type"))
        .then(name.clone())
        .then(generics.clone().or_not())
        .then_ignore(just(Tok::Assign))
        .then(ty.clone())
        .map(|(((p, name), g), ty)| (p, ItemKind::TypeAlias { name, generics: g.unwrap_or_default(), ty }));
    let struct_field = attributes
        .clone()
        .then(is_pub.clone())
        .then(member.clone())
        .then_ignore(just(Tok::Colon))
        .then(ty.clone())
        .map(|(((a, p), n), t)| (a, p, n, t));
    let struct_decl = is_pub
        .clone()
        .then(kw("repr").then(kw("c")).or_not().map(|r| r.is_some()))
        .then_ignore(kw("struct"))
        .then(name.clone())
        .then(generics.clone().or_not())
        .then(sepc_list(struct_field.clone()))
        .map(|((((p, repr_c), name), g), fields)| {
            (p, ItemKind::Struct { repr_c, name, generics: g.unwrap_or_default(), fields })
        });
    // `attribute NAME on POSITION, ... { FIELD: TYPE, ... }`
    let attr_position = choice((
        kw("type").ignore_then(just(Tok::Colon).ignore_then(ty.clone()).or_not()).map(AttrPosition::Type),
        kw("field").to(AttrPosition::Field),
        kw("variant").to(AttrPosition::Variant),
        kw("value").to(AttrPosition::Value),
        // a function position names the signature the functions have
        choice((kw("fn"), kw("once"))).rewind().ignore_then(ty.clone()).map(AttrPosition::Fn),
    ))
    .map_with(|p, e| (p, e.span()))
    .labelled("an attribute position (`type`, `fn(...) -> ...`, `field`, `variant`, or `value`)");
    let attribute_decl = is_pub
        .clone()
        .then_ignore(kw("attribute"))
        .then(value_ident.clone())
        .then_ignore(kw("on"))
        .then(attr_position.separated_by(just(Tok::Comma)).at_least(1).collect::<Vec<_>>())
        .then(sepc_list(struct_field.clone()))
        .map(|(((p, name), positions), fields)| (p, ItemKind::Attribute { name, positions, fields }));
    let variant_field = member.clone().then_ignore(just(Tok::Colon)).or_not().then(ty.clone());
    // a variant may carry attributes (`@http.status(404) not_found(id: Int)`)
    let variant = attributes
        .clone()
        .then(member.clone())
        .then(
            variant_field
                .separated_by(just(Tok::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(Tok::LParen), just(Tok::RParen))
                .or_not(),
        )
        .map_with(|((attributes, name), fields), e| Variant {
            attributes,
            name,
            fields: fields.unwrap_or_default(),
            span: e.span(),
        });
    let enum_decl = is_pub
        .clone()
        .then_ignore(kw("enum"))
        .then(name.clone())
        .then(generics.clone().or_not())
        .then(sepc_list(variant))
        .map(|(((p, name), g), variants)| {
            (p, ItemKind::Enum { name, generics: g.unwrap_or_default(), variants })
        });
    let protocol = is_pub
        .clone()
        .then_ignore(kw("protocol"))
        .then(name.clone())
        .then(generics.clone().or_not())
        .then(
            just(Tok::Colon)
                .ignore_then(ty.clone().separated_by(just(Tok::Plus)).at_least(1).collect::<Vec<_>>())
                .or_not(),
        )
        .then(members.clone())
        .map(|((((p, name), g), supers), members)| {
            (
                p,
                ItemKind::Protocol {
                    name,
                    generics: g.unwrap_or_default(),
                    supers: supers.unwrap_or_default(),
                    members,
                },
            )
        });
    let impl_decl = kw("impl")
        .ignore_then(generics.clone().or_not())
        .then(ty.clone())
        .then(kw("for").ignore_then(ty.clone()).or_not())
        .then(where_clause.clone().or_not())
        .then(members)
        .map(|((((g, first), for_ty), w), members)| {
            (
                false,
                ItemKind::Impl {
                    generics: g.unwrap_or_default(),
                    first,
                    for_ty,
                    where_preds: w.unwrap_or_default(),
                    members,
                },
            )
        });
    let fn_item = fn_decl(name.clone().boxed()).map(|d| (d.is_pub, ItemKind::Fn(d)));
    let extern_decl = is_pub
        .clone()
        .then_ignore(kw("extern"))
        .then(select! { Tok::Str(s) => s }.map_with(|s, e| (s, e.span())).labelled("string"))
        .then_ignore(kw("fn"))
        .then(name.clone())
        .then(generics.clone().or_not())
        .then(params.clone())
        .then(ret.clone())
        .then(effect.clone().or_not())
        .then(where_clause.clone().or_not())
        .map_with(|(((((((p, abi), name), g), params), ret), effect), w), e| {
            let decl = FnDecl {
                attributes: vec![],
                contracts: vec![],
                is_pub: p,
                name,
                generics: g.unwrap_or_default(),
                params,
                ret: Some(ret),
                effect,
                where_preds: w.unwrap_or_default(),
                body: None,
                span: e.span(),
            };
            (p, ItemKind::Extern { abi, decl })
        });
    let global = is_pub
        .clone()
        .then(let_kind)
        .then(pattern.clone())
        .then(just(Tok::Colon).ignore_then(ty.clone()).or_not())
        .then_ignore(just(Tok::Assign))
        .then(expr.clone())
        .map(|((((p, kind), pat), ty), value)| (p, ItemKind::Global { kind, pat, ty, value }));
    // `provider` and `stage` are keywords only where a declaration starts
    let alias = kw("as").ignore_then(symbol.clone()).or_not();
    let provider_item = contracts
        .clone()
        .then(is_pub.clone())
        .then_ignore(kw("provider"))
        .then(name.clone())
        .then(alias.clone())
        .then(params.clone().map_with(|p, e| (p, e.span())))
        .then(ret.clone().or_not())
        .then(effect.clone().or_not())
        .then(block.clone())
        .map(|(((((((contracts, p), name), alias), (params, params_span)), ret), effect), body)| {
            (p, ItemKind::Provider { contracts, name, alias, params, params_span, ret, effect, body })
        });
    let member = choice((
        kw("enter").to("enter"),
        kw("leave").to("leave"),
        kw("error").to("error"),
        kw("around").to("around"),
    ))
    .map_with(|text, e| Ident { text, span: e.span() })
    .labelled("a stage callback (`enter`, `leave`, `error`, or `around`)")
    .then(params.clone())
    .then(ret.clone().or_not())
    .then(choice((
        block.clone().map_with(|b, e| (Expr::Block(b), e.span())),
        just(Tok::FatArrow).ignore_then(expr.clone()),
    )))
    .map(|(((kind, params), ret), body)| StageMember { kind, params, ret, body });
    let stage_item = contracts
        .clone()
        .then(is_pub.clone())
        .then_ignore(kw("stage"))
        .then(name.clone())
        .then(alias)
        .then(generics.clone().or_not())
        .then(params.clone())
        .then(ret.clone().or_not())
        .then(
            choice((
                sep.clone().to(None),
                just(Tok::Semi).to(None),
                member
                    .then_ignore(
                        choice((
                            sep.clone(),
                            just(Tok::Semi).ignored().rewind(),
                            just(Tok::RBrace).ignored().rewind(),
                        ))
                        .labelled("a new line or `;`"),
                    )
                    .map(Some),
            ))
            .repeated()
            .collect::<Vec<_>>()
            .delimited_by(just(Tok::LBrace), just(Tok::RBrace))
            .map_with(|ms, e| (ms.into_iter().flatten().collect::<Vec<_>>(), e.span())),
        )
        .map(|(((((((contracts, p), name), alias), g), params), ret), (members, body_span))| {
            (
                p,
                ItemKind::Stage {
                    contracts,
                    name,
                    alias,
                    generics: g.unwrap_or_default(),
                    params,
                    ret,
                    members,
                    body_span,
                },
            )
        });
    let item = attributes
        .then(choice((
            import,
            extern_type,
            type_decl,
            struct_decl,
            attribute_decl,
            enum_decl,
            protocol,
            impl_decl,
            provider_item,
            stage_item,
            fn_item,
            extern_decl,
            global,
        )))
        .map_with(|(attributes, (is_pub, kind)), e| Item { kind, span: e.span(), is_pub, attributes })
        .labelled("declaration")
        // declarations, too, are separated by newlines or `;`
        .then_ignore(
            choice((sep.clone(), just(Tok::Semi).ignored().rewind(), end().rewind()))
                .labelled("a new line or `;`"),
        )
        .recover_with(via_parser(skip_balanced(&[Tok::Sep]).map_with(|_, e| Item {
            kind: ItemKind::Error,
            span: e.span(),
            is_pub: false,
            attributes: vec![],
        })));
    choice((sep.clone().to(None), just(Tok::Semi).to(None), item.map(Some)))
        .repeated()
        .collect::<Vec<_>>()
        .map(|items| items.into_iter().flatten().collect())
        .then_ignore(end())
}
