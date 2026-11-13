//! Chumsky 0.12 parser: rule text -> `Vec<RuleAst>`. The grammar is in
//! `docs/architecture/pcb-drc.md`, "Rule language":
//!
//! ```text
//! program  := rule*
//! rule     := "rule" NAME "on" TARGET pair? when? require "then" severity STRING?
//! pair     := "between" OBJECT "and" OBJECT
//! when     := "when" cond ("and" cond)*
//! cond     := SUBJECT ("==" | "matches") (STRING | WORD)
//! require  := "require" limit ("and" limit)*
//! limit    := FIELD (">=" | "<=") NUMBER UNIT?      (a space before UNIT only for a known unit)
//! severity := "error" | "warning" | "info"
//! ```

use chumsky::prelude::*;
use chumsky::text::ascii;

use crate::ast::*;

type Extra<'src> = extra::Err<Rich<'src, char>>;

/// Units a length may take after a space (attached, any word is read as
/// a unit and the validator names one it does not know).
const KNOWN_UNITS: [&str; 6] = ["mm", "mil", "um", "\u{b5}m", "\u{3bc}m", "nm"];

/// Parse rule text; on failure, each error's span and message.
pub fn parse(src: &str) -> Result<Vec<RuleAst>, Vec<(Span, String)>> {
    let (rules, errors) = program().parse(src).into_output_errors();
    if errors.is_empty() {
        return Ok(rules.unwrap_or_default());
    }
    Err(errors
        .into_iter()
        .map(|e| (range(*e.span()), e.to_string()))
        .collect())
}

fn range(span: SimpleSpan) -> Span {
    span.start..span.end
}

fn program<'src>() -> impl Parser<'src, &'src str, Vec<RuleAst>, Extra<'src>> {
    rule().repeated().collect().padded().then_ignore(end())
}

fn kw<'src>(word: &'static str) -> impl Parser<'src, &'src str, (), Extra<'src>> + Clone {
    ascii::keyword(word).ignored().padded()
}

fn word<'src>() -> impl Parser<'src, &'src str, Word, Extra<'src>> + Clone {
    ascii::ident()
        .map_with(|text: &str, e| Word {
            text: text.to_string(),
            span: range(e.span()),
        })
        .padded()
}

/// A double-quoted string, without its quotes; the span covers them.
fn string<'src>() -> impl Parser<'src, &'src str, Word, Extra<'src>> + Clone {
    none_of('"')
        .repeated()
        .collect::<String>()
        .delimited_by(just('"'), just('"'))
        .map_with(|text, e| Word {
            text,
            span: range(e.span()),
        })
        .padded()
}

fn limit<'src>() -> impl Parser<'src, &'src str, LimitAst, Extra<'src>> + Clone {
    let bound = choice((just(">=").to(Bound::AtLeast), just("<=").to(Bound::AtMost))).padded();
    let number = text::int(10)
        .then(just('.').then(text::digits(10)).or_not())
        .to_slice()
        .map_with(|text: &str, e| Word {
            text: text.to_string(),
            span: range(e.span()),
        });
    let unit = any()
        .filter(|c: &char| c.is_alphabetic())
        .repeated()
        .at_least(1)
        .to_slice()
        .map_with(|text: &str, e| Word {
            text: text.to_string(),
            span: range(e.span()),
        });
    // After a space only a known unit counts, so `0.2 and` is not `0.2and`.
    let spaced_unit = text::inline_whitespace()
        .at_least(1)
        .ignore_then(unit.try_map(|w: Word, span| {
            if KNOWN_UNITS.contains(&w.text.as_str()) {
                Ok(w)
            } else {
                Err(Rich::custom(span, "not a unit"))
            }
        }));
    word()
        .then(bound)
        .then(number.then(unit.or(spaced_unit).or_not()).padded())
        .map(|((field, bound), (number, unit))| LimitAst {
            field,
            bound,
            number,
            unit,
        })
}

fn rule<'src>() -> impl Parser<'src, &'src str, RuleAst, Extra<'src>> {
    let pair = kw("between")
        .ignore_then(word())
        .then_ignore(kw("and"))
        .then(word())
        .map(|(a, b)| [a, b]);
    let op = choice((
        just("==").padded().to(CondOp::Is),
        kw("matches").to(CondOp::Matches),
    ));
    let cond = word()
        .then(op)
        .then(choice((string(), word())))
        .map(|((subject, op), value)| CondAst { subject, op, value });
    let when = kw("when").ignore_then(cond.separated_by(kw("and")).at_least(1).collect());
    let require = kw("require").ignore_then(limit().separated_by(kw("and")).at_least(1).collect());
    let severity = choice((
        kw("error").to(SeverityAst::Error),
        kw("warning").to(SeverityAst::Warning),
        kw("info").to(SeverityAst::Info),
    ));
    kw("rule")
        .ignore_then(word())
        .then(kw("on").ignore_then(word()))
        .then(pair.or_not())
        .then(when.or_not())
        .then(require)
        .then(kw("then").ignore_then(severity))
        .then(string().or_not())
        .map(
            |((((((name, target), pair), when), require), severity), message)| RuleAst {
                name,
                target,
                pair,
                when: when.unwrap_or_default(),
                require,
                severity,
                message: message.map(|m| m.text),
            },
        )
}
