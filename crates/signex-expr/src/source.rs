//! The one on-disk rule for a text that may hold an expression.
//!
//! - A leading `=` marks an expression: `= R_load * 2`.
//! - `==` escapes a literal leading `=`: `==A` is the text `=A`.
//! - The saved value is always the source text, never the evaluated
//!   result; a parse or evaluation error leaves the text untouched.
//!
//! The sketch's parameter table is older than this rule and reads every
//! value as an expression with an optional `=` ([`expression_body`]); its
//! on-disk form does not change.

/// What a stored text is under the `=` / `==` rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldText<'a> {
    /// The expression after the leading `=`, trimmed.
    Expression(&'a str),
    /// A literal text, with an escaping `=` removed.
    Literal(&'a str),
}

/// Classify a stored text under the `=` / `==` rule. Leading whitespace
/// before the `=` is ignored.
pub fn classify(text: &str) -> FieldText<'_> {
    match text.trim_start().strip_prefix('=') {
        Some(rest) if rest.starts_with('=') => FieldText::Literal(rest),
        Some(rest) => FieldText::Expression(rest.trim()),
        None => FieldText::Literal(text),
    }
}

/// The expression in a sketch-style value, where every value is an
/// expression and the leading `=` is optional: trims the text and strips
/// one leading `=` (`"  = pad_w "` → `"pad_w"`).
pub fn expression_body(src: &str) -> &str {
    let s = src.trim();
    s.strip_prefix('=').map(|s| s.trim_start()).unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_leading_eq_marks_an_expression() {
        assert_eq!(
            classify("= R_load * 2"),
            FieldText::Expression("R_load * 2")
        );
        assert_eq!(
            classify("  =3.3V/10mA "),
            FieldText::Expression("3.3V/10mA")
        );
    }

    #[test]
    fn a_double_eq_escapes_a_literal_eq() {
        assert_eq!(classify("==A"), FieldText::Literal("=A"));
        assert_eq!(classify("== 5"), FieldText::Literal("= 5"));
    }

    #[test]
    fn text_without_eq_is_literal_and_unchanged() {
        assert_eq!(classify(" 4.7k "), FieldText::Literal(" 4.7k "));
        assert_eq!(classify("a = b"), FieldText::Literal("a = b"));
    }

    #[test]
    fn expression_body_strips_one_eq() {
        assert_eq!(expression_body("  = pad_w "), "pad_w");
        assert_eq!(expression_body("pad_w"), "pad_w");
    }
}
