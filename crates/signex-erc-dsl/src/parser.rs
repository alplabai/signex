//! Chumsky 0.12 parser: DSL source text → `Vec<RuleAst>`.
//!
//! Grammar (whitespace-insensitive between tokens):
//!
//! ```text
//! program   := rule_decl*
//!
//! rule_decl := "rule" IDENT
//!              ("apply_to" applicability)?
//!              "on"    target
//!              ("scope" scope)?
//!              "when"  expr
//!              "then"  severity STRING
//!
//! applicability := "sheets" "tagged" STRING
//!               |  "sheets" "named"  STRING
//!
//! target    := "net" | "pin" | "component" | "sheet"
//! scope     := "local" | "sheet" | "hierarchical" | "global"
//! severity  := "error" | "warning" | "info"
//!
//! expr      := or_expr
//! or_expr   := and_expr  ("or"  and_expr)*
//! and_expr  := not_expr  ("and" not_expr)*
//! not_expr  := "not" not_expr | primary
//!
//! primary   := "(" expr ")"
//!           |  IDENT "." IDENT "(" args? ")" cmp_or_matches
//!           |  IDENT "." IDENT              cmp_or_matches
//!           |  IDENT "(" args? ")"
//!
//! cmp_or_matches := ("==" | "!=") literal
//!                |  "matches" STRING
//!
//! args      := literal ("," literal)*
//! literal   := STRING | "true" | "false" | IDENT
//! ```

use chumsky::prelude::*;
use chumsky::text::ascii;

use crate::ast::*;

type Err<'src> = extra::Err<Simple<'src, char>>;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Parse DSL source text into a list of rule declarations.
/// Returns the list of parse errors (byte offsets) on failure.
pub fn parse(src: &str) -> Result<Vec<RuleAst>, Vec<(usize, String)>> {
    let (rules, errs) = program_parser().parse(src).into_output_errors();
    if errs.is_empty() {
        Ok(rules.unwrap_or_default())
    } else {
        let msgs = errs
            .into_iter()
            .map(|e| {
                let span = e.span();
                (span.start, format!("{e}"))
            })
            .collect();
        Err(msgs)
    }
}

// ---------------------------------------------------------------------------
// Program parser
// ---------------------------------------------------------------------------

fn program_parser<'src>() -> impl Parser<'src, &'src str, Vec<RuleAst>, Err<'src>> {
    rule_parser()
        .padded()
        .repeated()
        .collect()
        .then_ignore(end())
}

// ---------------------------------------------------------------------------
// Rule declaration parser
// ---------------------------------------------------------------------------

fn rule_parser<'src>() -> impl Parser<'src, &'src str, RuleAst, Err<'src>> {
    let kw = |s: &'static str| ascii::keyword(s).padded();

    let id = ascii::ident().padded().map(str::to_string);
    let str_lit = string_lit_parser();

    let applicability = choice((
        kw("sheets")
            .ignore_then(kw("tagged"))
            .ignore_then(str_lit.clone())
            .map(|s| ApplicabilityAst::TaggedSheets(vec![s])),
        kw("sheets")
            .ignore_then(kw("named"))
            .ignore_then(str_lit.clone())
            .map(|s| ApplicabilityAst::NamedSheets(vec![s])),
    ));

    let target = choice((
        kw("net").to(TargetKind::Net),
        kw("pin").to(TargetKind::Pin),
        kw("component").to(TargetKind::Component),
        kw("sheet").to(TargetKind::Sheet),
    ));

    let scope = choice((
        kw("local").to(ScopeKind::Local),
        kw("sheet").to(ScopeKind::Sheet),
        kw("hierarchical").to(ScopeKind::Hierarchical),
        kw("global").to(ScopeKind::Global),
    ));

    let severity = choice((
        kw("error").to(SeverityKind::Error),
        kw("warning").to(SeverityKind::Warning),
        kw("info").to(SeverityKind::Info),
    ));

    let expr = expr_parser();

    kw("rule")
        .ignore_then(id)
        .then(kw("apply_to").ignore_then(applicability).or_not())
        .then(kw("on").ignore_then(target))
        .then(kw("scope").ignore_then(scope).or_not())
        .then(kw("when").ignore_then(expr))
        .then(kw("then").ignore_then(severity).then(str_lit))
        .map(
            |(((((id, app), target), scope), when), (severity, message))| RuleAst {
                id,
                applicability: app.unwrap_or(ApplicabilityAst::All),
                target,
                scope: scope.unwrap_or(ScopeKind::Sheet),
                when,
                severity,
                message,
            },
        )
}

// ---------------------------------------------------------------------------
// Expression parser (precedence: or < and < not < primary)
// ---------------------------------------------------------------------------

fn expr_parser<'src>() -> impl Parser<'src, &'src str, ExprAst, Err<'src>> {
    recursive(|expr| {
        let kw = |s: &'static str| ascii::keyword(s).padded();

        let primary = primary_parser(expr.boxed()).boxed();

        // not_expr := "not" not_expr | primary
        let not_expr = kw("not")
            .repeated()
            .collect::<Vec<_>>()
            .then(primary.clone())
            .map(|(nots, base)| {
                nots.into_iter()
                    .rev()
                    .fold(base, |acc, _| ExprAst::Not(Box::new(acc)))
            });

        // and_expr := not_expr ("and" not_expr)*
        let and_expr = not_expr
            .clone()
            .foldl(kw("and").ignore_then(not_expr).repeated(), |a, b| {
                ExprAst::And(Box::new(a), Box::new(b))
            });

        // or_expr := and_expr ("or" and_expr)*
        and_expr
            .clone()
            .foldl(kw("or").ignore_then(and_expr).repeated(), |a, b| {
                ExprAst::Or(Box::new(a), Box::new(b))
            })
    })
}

// ---------------------------------------------------------------------------
// Primary expression parser
// ---------------------------------------------------------------------------

fn primary_parser<'src>(
    expr: Boxed<'src, 'src, &'src str, ExprAst, Err<'src>>,
) -> impl Parser<'src, &'src str, ExprAst, Err<'src>> {
    let kw = |s: &'static str| ascii::keyword(s).padded();
    let str_lit = string_lit_parser();
    let lit = literal_parser();

    // Argument list: lit ("," lit)*
    let args = lit
        .clone()
        .separated_by(just(',').padded())
        .collect::<Vec<_>>();

    // cmp_or_matches: ("==" | "!=") literal  |  "matches" STRING
    let cmp_op = choice((
        just('=').then(just('=')).padded().to(CmpOp::Eq),
        just('!').then(just('=')).padded().to(CmpOp::Ne),
    ));

    // Parenthesised expression
    let paren = just('(')
        .padded()
        .ignore_then(expr)
        .then_ignore(just(')').padded());

    // ident.ident(args?)  cmp_or_matches   — method call comparison
    // ident.ident         cmp_or_matches   — field access comparison
    // ident(args?)                         — helper call
    //
    // We parse the leading ident then branch on what follows.

    let ident_str = ascii::ident().padded().map(str::to_string);

    let method_or_field_cmp = ident_str
        .clone()
        .then_ignore(just('.').padded())
        .then(ident_str.clone()) // field or method name
        .then(
            // Optional method args
            just('(')
                .padded()
                .ignore_then(args.clone())
                .then_ignore(just(')').padded())
                .or_not(),
        )
        .then(choice((
            cmp_op
                .then(lit.clone())
                .map(|(op, val)| FieldRhs::Cmp(op, val)),
            kw("matches")
                .ignore_then(str_lit.clone())
                .map(FieldRhs::Matches),
        )))
        .map(|(((object, fname), method_args), rhs)| {
            let field = match method_args {
                Some(a) => FieldExprAst::MethodCall {
                    object,
                    method: fname,
                    args: a,
                },
                None => FieldExprAst::Access {
                    object,
                    field: fname,
                },
            };
            match rhs {
                FieldRhs::Cmp(op, value) => ExprAst::FieldCmp { field, op, value },
                FieldRhs::Matches(pattern) => ExprAst::FieldMatches { field, pattern },
            }
        });

    let helper_call = ident_str
        .then_ignore(just('(').padded())
        .then(args)
        .then_ignore(just(')').padded())
        .map(|(name, args)| ExprAst::HelperCall { name, args });

    choice((paren, method_or_field_cmp, helper_call))
}

// ---------------------------------------------------------------------------
// Leaf parsers
// ---------------------------------------------------------------------------

/// Double-quoted string literal, returns the content without quotes.
fn string_lit_parser<'src>() -> impl Parser<'src, &'src str, String, Err<'src>> + Clone {
    just('"')
        .ignore_then(none_of('"').repeated().collect::<String>())
        .then_ignore(just('"'))
        .padded()
}

/// Literal value: `true` | `false` | `"string"` | ident.
fn literal_parser<'src>() -> impl Parser<'src, &'src str, LiteralAst, Err<'src>> + Clone {
    let str_lit = string_lit_parser();
    choice((
        ascii::keyword("true").padded().to(LiteralAst::Bool(true)),
        ascii::keyword("false").padded().to(LiteralAst::Bool(false)),
        str_lit.map(LiteralAst::Str),
        ascii::ident()
            .padded()
            .map(|s: &str| LiteralAst::Ident(s.to_string())),
    ))
}

// ---------------------------------------------------------------------------
// Internal helper enum for the primary parser
// ---------------------------------------------------------------------------

enum FieldRhs {
    Cmp(CmpOp, LiteralAst),
    Matches(String),
}
