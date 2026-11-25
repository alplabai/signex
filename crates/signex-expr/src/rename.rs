//! Token-aware edits of an expression's source text: find the names it
//! reads, rename one, or put a value in its place.
//!
//! The scan follows the lexer's rules (`lex.rs`) without its errors,
//! so a source that does not parse yet still has its references found:
//!
//! - a run that starts with a digit is a quantity and is skipped whole, unit
//!   suffix and RKM digits included (`2mm`, `4k7`), so a name never matches
//!   inside a literal;
//! - an identifier is `[a-zA-Z_][a-zA-Z0-9_]*`, dotted segments included
//!   (`R1.Value` is one name), so `pitch` never matches inside `pitch2`;
//! - an identifier followed by `(` is a function call and a reserved constant
//!   (`pi`) is a literal: neither is a reference.
//!
//! Only the matched spans change: a leading `=`, the spacing and everything
//! else in the source are kept byte for byte.

use std::ops::Range;

use crate::parse::is_reserved_constant;
use crate::source::expression_body;
use crate::symbols::{is_unit_char, prefix_factor};

/// True for a plain name a host can bind: `[a-zA-Z_][a-zA-Z0-9_]*`, no
/// dots. Says nothing about reserved names; the host checks those.
pub fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// True when `source` reads the name `name` as a reference.
pub fn reads_reference(source: &str, name: &str) -> bool {
    reference_spans(source)
        .into_iter()
        .any(|span| &source[span] == name)
}

/// The names `source` reads, in first-seen order, each once.
pub fn referenced_names(source: &str) -> Vec<&str> {
    let mut out: Vec<&str> = Vec::new();
    for span in reference_spans(source) {
        let name = &source[span];
        if !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

/// `source` with every reference to `old` renamed to `new`. Text that only
/// contains `old` (a longer name, a unit suffix, a function name) is left
/// alone.
pub fn rename_reference(source: &str, old: &str, new: &str) -> String {
    replace_references(source, old, new)
}

/// `source` with every reference to `name` replaced by the expression
/// `value_source` (an optional leading `=` is dropped). A value that is one
/// literal or one name goes in as is; anything longer is parenthesised, so
/// `2 * w` with `w = a + b` becomes `2 * (a + b)`.
pub fn inline_reference(source: &str, name: &str, value_source: &str) -> String {
    let body = expression_body(value_source);
    let replacement = if is_atom(body) {
        body.to_string()
    } else {
        format!("({body})")
    };
    replace_references(source, name, &replacement)
}

fn replace_references(source: &str, name: &str, replacement: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut copied = 0;
    for span in reference_spans(source) {
        if &source[span.clone()] != name {
            continue;
        }
        out.push_str(&source[copied..span.start]);
        out.push_str(replacement);
        copied = span.end;
    }
    out.push_str(&source[copied..]);
    out
}

/// One quantity literal or one name, nothing around it.
fn is_atom(body: &str) -> bool {
    let Some(first) = body.chars().next() else {
        return false;
    };
    if first.is_ascii_digit() {
        quantity_end(body, 0) == body.len()
    } else if first.is_ascii_alphabetic() || first == '_' {
        identifier_end(body, 0) == body.len()
    } else {
        false
    }
}

/// The byte ranges of the references in `source`.
fn reference_spans(source: &str) -> Vec<Range<usize>> {
    let mut spans = Vec::new();
    let mut at = 0;
    while let Some(c) = source[at..].chars().next() {
        if c.is_ascii_digit() {
            at = quantity_end(source, at);
        } else if c.is_ascii_alphabetic() || c == '_' {
            let end = identifier_end(source, at);
            let name = &source[at..end];
            if !is_call(source, end) && !is_reserved_constant(name) {
                spans.push(at..end);
            }
            at = end;
        } else {
            at += c.len_utf8();
        }
    }
    spans
}

/// The end of the run of characters from `at` that `keep` accepts.
fn eat_while(source: &str, at: usize, keep: impl Fn(char) -> bool) -> usize {
    source[at..]
        .char_indices()
        .find(|&(_, c)| !keep(c))
        .map_or(source.len(), |(i, _)| at + i)
}

/// The end of the quantity literal starting at the digit at `at`: digits
/// and dots, a unit suffix, and the digits after an RKM prefix letter.
fn quantity_end(source: &str, at: usize) -> usize {
    let digits_end = eat_while(source, at, |c| c.is_ascii_digit() || c == '.');
    let suffix_end = eat_while(source, digits_end, is_unit_char);
    let marker = &source[digits_end..suffix_end];
    let is_rkm = marker.chars().count() == 1
        && prefix_factor(marker).is_some()
        && source[suffix_end..]
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit());
    if !is_rkm {
        return suffix_end;
    }
    let rkm_digits_end = eat_while(source, suffix_end, |c| c.is_ascii_digit());
    eat_while(source, rkm_digits_end, is_unit_char)
}

/// The end of the identifier starting at `at`, dotted segments included.
fn identifier_end(source: &str, at: usize) -> usize {
    let ident_char = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut end = eat_while(source, at, ident_char);
    let bytes = source.as_bytes();
    while bytes.get(end) == Some(&b'.')
        && bytes
            .get(end + 1)
            .is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_')
    {
        end = eat_while(source, end + 1, ident_char);
    }
    end
}

/// True when the next non-blank character after `end` opens a call.
fn is_call(source: &str, end: usize) -> bool {
    source[end..]
        .trim_start_matches([' ', '\t', '\n', '\r'])
        .starts_with('(')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_whole_names_are_renamed() {
        assert_eq!(
            rename_reference("pitch + pitch2 + _pitch + pitch_x", "pitch", "p"),
            "p + pitch2 + _pitch + pitch_x"
        );
    }

    #[test]
    fn a_leading_eq_and_the_spacing_are_kept() {
        assert_eq!(
            rename_reference("  = pitch *  (rows-1) ", "pitch", "pad_pitch"),
            "  = pad_pitch *  (rows-1) "
        );
        assert_eq!(rename_reference("=pitch", "pitch", "p"), "=p");
    }

    #[test]
    fn unit_suffixes_rkm_literals_and_calls_are_not_names() {
        // `mm` inside `2mm`, `k` inside `4k7`, `max` called: none renamed.
        assert_eq!(rename_reference("2mm + mm", "mm", "x"), "2mm + x");
        assert_eq!(rename_reference("4k7 * k", "k", "kk"), "4k7 * kk");
        assert_eq!(
            rename_reference("max(max, 1mm)", "max", "upper"),
            "max(upper, 1mm)"
        );
        assert_eq!(rename_reference("max (a, b)", "max", "m"), "max (a, b)");
    }

    #[test]
    fn a_dotted_path_is_one_name() {
        assert_eq!(
            rename_reference("R1.Value + R1", "R1", "R2"),
            "R1.Value + R2"
        );
    }

    #[test]
    fn a_reserved_constant_is_not_a_reference() {
        assert!(!reads_reference("2 * pi", "pi"));
        assert_eq!(referenced_names("2 * pi * r + r"), vec!["r"]);
    }

    #[test]
    fn a_source_that_does_not_parse_still_has_its_names() {
        assert!(reads_reference("pitch *", "pitch"));
        assert_eq!(rename_reference("pitch * $", "pitch", "p"), "p * $");
    }

    #[test]
    fn inlining_parenthesises_anything_longer_than_one_token() {
        assert_eq!(inline_reference("2 * w", "w", "0.65mm"), "2 * 0.65mm");
        assert_eq!(inline_reference("2 * w", "w", "= a + b"), "2 * (a + b)");
        assert_eq!(inline_reference("2 * w", "w", "-0.5mm"), "2 * (-0.5mm)");
        assert_eq!(inline_reference("= w", "w", "body"), "= body");
        assert_eq!(inline_reference("w2 + w", "w", "4"), "w2 + 4");
    }

    #[test]
    fn identifiers_are_plain_names() {
        assert!(is_identifier("pad_w"));
        assert!(is_identifier("_x1"));
        assert!(!is_identifier(""));
        assert!(!is_identifier("1x"));
        assert!(!is_identifier("a.b"));
        assert!(!is_identifier("a b"));
    }
}
