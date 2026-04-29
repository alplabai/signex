//! Signex schematic-text markup.
//!
//! Markdown-extension style — a small subset of standard Markdown plus
//! Signex-specific extensions for technical typography:
//!
//!   `**bold**`           — bold span
//!   `*italic*`           — italic span
//!   `~~strike~~`         — strikethrough
//!   `^superscript^`      — superscript (Signex extension; not GFM)
//!   `~subscript~`        — subscript (Signex extension; not GFM)
//!   `_~overbar~_`        — overbar (Signex extension; for active-low signal naming)
//!   `[label](url)`       — link
//!   `\X`                 — literal X (escape any sigil)
//!
//! Returns a flat `Vec<RichSegment>`. Spans don't nest in this version
//! (matching the practical needs of schematic labels, component
//! refdes/value/comments, pin names, and net names — none of which
//! typically use nested formatting). If nesting becomes useful, the
//! parser can be upgraded to a span tree without changing the public
//! enum's variant set.
//!
//! Auto net names use the format `unnamed-<sheet>:<ref>:<pin>`. This
//! is the canonical Signex spelling — it does not match any other
//! EDA tool's auto-net format.
//!
//! Expression substitution (`${refdes:...}`, `@{...}`, `CELL()`,
//! `NET_NAME(...)`) is preserved from the previous module — it is
//! Altium-flavoured and was already independent of the KiCad markup
//! syntax.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Rich text segments
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RichSegment {
    Normal(String),
    Bold(String),
    Italic(String),
    Strike(String),
    Superscript(String),
    Subscript(String),
    Overbar(String),
    Link { label: String, url: String },
}

// ---------------------------------------------------------------------------
// Expression substitution context (unchanged from before — Altium-flavoured)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct ExpressionEvalContext<'a> {
    pub current_refdes: Option<&'a str>,
    pub current_value: Option<&'a str>,
    pub current_pin: Option<&'a str>,
    pub cell: Option<&'a str>,
    pub at_variables: Option<&'a HashMap<String, String>>,
    pub refdes_variables: Option<&'a HashMap<String, String>>,
    pub net_name_by_pin: Option<&'a HashMap<String, String>>,
}

// ---------------------------------------------------------------------------
// Auto net name — Signex format, not derived from any other EDA tool
// ---------------------------------------------------------------------------

/// Default name for an unnamed net.
///
/// Format: `unnamed-<sheet>:<ref>:<pin>`. Picks the lexicographically-
/// smallest `(refdes, pin)` for determinism. Sheet defaults to empty
/// string when the caller doesn't have a sheet context.
pub fn auto_net_name(sheet: &str, pins: &[(String, String)]) -> Option<String> {
    pins.iter()
        .min_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)))
        .map(|(r, p)| {
            if sheet.is_empty() {
                format!("unnamed-{r}:{p}")
            } else {
                format!("unnamed-{sheet}:{r}:{p}")
            }
        })
}

// ---------------------------------------------------------------------------
// Expression evaluator
// ---------------------------------------------------------------------------

/// Evaluate a subset of Altium-style expression variables.
///
/// Supported:
/// - `${refdes:<key>}`
/// - `@{<name>}`
/// - `CELL()`
/// - `NET_NAME(<pin>)`
///
/// Unresolved expressions are preserved verbatim to avoid destructive output.
pub fn evaluate_expressions(input: &str, ctx: &ExpressionEvalContext<'_>) -> String {
    if input.is_empty() {
        return String::new();
    }

    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            let next = bytes[i + 1] as char;
            if next == '$' || next == '@' {
                out.push(next);
                i += 2;
                continue;
            }
            out.push('\\');
            i += 1;
            continue;
        }

        if bytes[i] == b'$' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
            if let Some((expr, next_index)) = read_braced(input, i + 2) {
                if let Some(value) = eval_dollar_expression(expr.trim(), ctx) {
                    out.push_str(&value);
                } else {
                    out.push_str("${");
                    out.push_str(expr);
                    out.push('}');
                }
                i = next_index;
                continue;
            }
        }

        if bytes[i] == b'@' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
            if let Some((expr, next_index)) = read_braced(input, i + 2) {
                if let Some(value) = eval_at_expression(expr.trim(), ctx) {
                    out.push_str(&value);
                } else {
                    out.push_str("@{");
                    out.push_str(expr);
                    out.push('}');
                }
                i = next_index;
                continue;
            }
        }

        if starts_with_ascii_ci(bytes, i, b"CELL()") {
            if let Some(cell) = ctx.cell {
                out.push_str(cell);
            } else {
                out.push_str("CELL()");
            }
            i += "CELL()".len();
            continue;
        }

        if starts_with_ascii_ci(bytes, i, b"NET_NAME(") {
            if let Some((arg, next_index)) = read_parenthesized(input, i + "NET_NAME(".len()) {
                if let Some(value) = eval_net_name(arg.trim(), ctx) {
                    out.push_str(&value);
                } else {
                    out.push_str("NET_NAME(");
                    out.push_str(arg);
                    out.push(')');
                }
                i = next_index;
                continue;
            }
        }

        let ch = input[i..].chars().next().unwrap_or('\0');
        if ch == '\0' {
            break;
        }
        out.push(ch);
        i += ch.len_utf8();
    }

    out
}

// ---------------------------------------------------------------------------
// Markup parser
// ---------------------------------------------------------------------------

/// Parse Signex markup into a flat list of rich segments.
///
/// Sigils are consumed in order; the parser is single-pass and does not
/// handle nested formatting (e.g. `**_~OE~_**` produces a Bold segment
/// containing the literal text `_~OE~_`). Use whichever decoration
/// matters most semantically.
pub fn parse_signex_markup(input: &str) -> Vec<RichSegment> {
    if input.is_empty() {
        return vec![];
    }

    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut segments = Vec::new();
    let mut normal_buf = String::new();
    let mut i = 0usize;

    while i < len {
        // Escape sequence: \X produces a literal X for any sigil character.
        if bytes[i] == b'\\' && i + 1 < len {
            let next = bytes[i + 1];
            if matches!(next, b'*' | b'~' | b'^' | b'_' | b'\\' | b'[' | b']' | b'(' | b')') {
                normal_buf.push(next as char);
                i += 2;
                continue;
            }
            normal_buf.push('\\');
            i += 1;
            continue;
        }

        // Overbar: _~text~_ (Signex extension). Check before subscript ~text~
        // because the underscore disambiguates the longer form.
        if bytes[i] == b'_' && i + 1 < len && bytes[i + 1] == b'~' {
            if let Some((content, next_index)) = read_overbar(input, i + 2) {
                flush_normal(&mut segments, &mut normal_buf);
                segments.push(RichSegment::Overbar(content));
                i = next_index;
                continue;
            }
        }

        // Bold: **text**
        if bytes[i] == b'*' && i + 1 < len && bytes[i + 1] == b'*' {
            if let Some((content, next_index)) = read_paired_double(input, i + 2, b'*') {
                flush_normal(&mut segments, &mut normal_buf);
                segments.push(RichSegment::Bold(content));
                i = next_index;
                continue;
            }
        }

        // Italic: *text*
        if bytes[i] == b'*' {
            if let Some((content, next_index)) = read_paired_single(input, i + 1, b'*') {
                flush_normal(&mut segments, &mut normal_buf);
                segments.push(RichSegment::Italic(content));
                i = next_index;
                continue;
            }
        }

        // Strikethrough: ~~text~~
        if bytes[i] == b'~' && i + 1 < len && bytes[i + 1] == b'~' {
            if let Some((content, next_index)) = read_paired_double(input, i + 2, b'~') {
                flush_normal(&mut segments, &mut normal_buf);
                segments.push(RichSegment::Strike(content));
                i = next_index;
                continue;
            }
        }

        // Subscript: ~text~
        if bytes[i] == b'~' {
            if let Some((content, next_index)) = read_paired_single(input, i + 1, b'~') {
                flush_normal(&mut segments, &mut normal_buf);
                segments.push(RichSegment::Subscript(content));
                i = next_index;
                continue;
            }
        }

        // Superscript: ^text^
        if bytes[i] == b'^' {
            if let Some((content, next_index)) = read_paired_single(input, i + 1, b'^') {
                flush_normal(&mut segments, &mut normal_buf);
                segments.push(RichSegment::Superscript(content));
                i = next_index;
                continue;
            }
        }

        // Link: [label](url)
        if bytes[i] == b'[' {
            if let Some((label, after_label)) = read_paired_bracket(input, i + 1) {
                if after_label < len && bytes[after_label] == b'(' {
                    if let Some((url, next_index)) = read_paired_paren(input, after_label + 1) {
                        flush_normal(&mut segments, &mut normal_buf);
                        segments.push(RichSegment::Link { label, url });
                        i = next_index;
                        continue;
                    }
                }
            }
        }

        // Plain character — multi-byte UTF-8 safe.
        let ch = input[i..].chars().next().unwrap_or('\0');
        if ch == '\0' {
            break;
        }
        normal_buf.push(ch);
        i += ch.len_utf8();
    }

    flush_normal(&mut segments, &mut normal_buf);
    segments
}

fn flush_normal(segments: &mut Vec<RichSegment>, buf: &mut String) {
    if !buf.is_empty() {
        segments.push(RichSegment::Normal(std::mem::take(buf)));
    }
}

/// Find the next single sigil byte and capture the content between.
fn read_paired_single(input: &str, start: usize, sigil: u8) -> Option<(String, usize)> {
    let bytes = input.as_bytes();
    let mut i = start;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        if bytes[i] == sigil {
            let raw = &input[start..i];
            return Some((unescape(raw), i + 1));
        }
        let ch = input[i..].chars().next().unwrap_or('\0');
        if ch == '\0' {
            return None;
        }
        i += ch.len_utf8();
    }
    None
}

/// Find a doubled sigil (e.g. `**` or `~~`) and capture the content between.
fn read_paired_double(input: &str, start: usize, sigil: u8) -> Option<(String, usize)> {
    let bytes = input.as_bytes();
    let mut i = start;
    while i + 1 < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        if bytes[i] == sigil && bytes[i + 1] == sigil {
            let raw = &input[start..i];
            return Some((unescape(raw), i + 2));
        }
        let ch = input[i..].chars().next().unwrap_or('\0');
        if ch == '\0' {
            return None;
        }
        i += ch.len_utf8();
    }
    None
}

/// Read the body of an overbar `_~ ... ~_` (already past the opening `_~`).
fn read_overbar(input: &str, start: usize) -> Option<(String, usize)> {
    let bytes = input.as_bytes();
    let mut i = start;
    while i + 1 < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        if bytes[i] == b'~' && bytes[i + 1] == b'_' {
            let raw = &input[start..i];
            return Some((unescape(raw), i + 2));
        }
        let ch = input[i..].chars().next().unwrap_or('\0');
        if ch == '\0' {
            return None;
        }
        i += ch.len_utf8();
    }
    None
}

fn read_paired_bracket(input: &str, start: usize) -> Option<(String, usize)> {
    let bytes = input.as_bytes();
    let mut i = start;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        if bytes[i] == b']' {
            let raw = &input[start..i];
            return Some((unescape(raw), i + 1));
        }
        let ch = input[i..].chars().next().unwrap_or('\0');
        if ch == '\0' {
            return None;
        }
        i += ch.len_utf8();
    }
    None
}

fn read_paired_paren(input: &str, start: usize) -> Option<(String, usize)> {
    let bytes = input.as_bytes();
    let mut i = start;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        if bytes[i] == b')' {
            let raw = &input[start..i];
            return Some((unescape(raw), i + 1));
        }
        let ch = input[i..].chars().next().unwrap_or('\0');
        if ch == '\0' {
            return None;
        }
        i += ch.len_utf8();
    }
    None
}

fn unescape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            out.push(bytes[i + 1] as char);
            i += 2;
            continue;
        }
        let ch = input[i..].chars().next().unwrap_or('\0');
        if ch == '\0' {
            break;
        }
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn read_braced(input: &str, start_index: usize) -> Option<(&str, usize)> {
    let bytes = input.as_bytes();
    let mut i = start_index;
    let mut depth = 1usize;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&input[start_index..i], i + 1));
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn read_parenthesized(input: &str, start_index: usize) -> Option<(&str, usize)> {
    let bytes = input.as_bytes();
    let mut i = start_index;
    let mut depth = 1usize;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&input[start_index..i], i + 1));
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn starts_with_ascii_ci(haystack: &[u8], start: usize, needle: &[u8]) -> bool {
    if start + needle.len() > haystack.len() {
        return false;
    }
    haystack[start..start + needle.len()]
        .iter()
        .zip(needle.iter())
        .all(|(h, n)| h.eq_ignore_ascii_case(n))
}

fn lookup_ci(map: Option<&HashMap<String, String>>, key: &str) -> Option<String> {
    let map = map?;
    if let Some(v) = map.get(key) {
        return Some(v.clone());
    }
    map.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v.clone())
}

fn eval_dollar_expression(expr: &str, ctx: &ExpressionEvalContext<'_>) -> Option<String> {
    if expr.is_empty() {
        return None;
    }

    if let Some((head, tail)) = expr.split_once(':')
        && head.trim().eq_ignore_ascii_case("refdes")
    {
        let key = tail.trim();
        if key.is_empty() || key.eq_ignore_ascii_case("self") || key.eq_ignore_ascii_case("current")
        {
            return ctx.current_refdes.map(ToString::to_string);
        }
        if let Some(v) = lookup_ci(ctx.refdes_variables, key) {
            return Some(v);
        }
        return None;
    }

    if expr.eq_ignore_ascii_case("refdes") || expr.eq_ignore_ascii_case("reference") {
        return ctx.current_refdes.map(ToString::to_string);
    }
    if expr.eq_ignore_ascii_case("value") {
        return ctx.current_value.map(ToString::to_string);
    }

    lookup_ci(ctx.at_variables, expr)
}

fn eval_at_expression(expr: &str, ctx: &ExpressionEvalContext<'_>) -> Option<String> {
    if expr.is_empty() {
        return None;
    }

    if expr.eq_ignore_ascii_case("refdes") || expr.eq_ignore_ascii_case("reference") {
        return ctx.current_refdes.map(ToString::to_string);
    }
    if expr.eq_ignore_ascii_case("value") {
        return ctx.current_value.map(ToString::to_string);
    }
    lookup_ci(ctx.at_variables, expr)
}

fn eval_net_name(expr: &str, ctx: &ExpressionEvalContext<'_>) -> Option<String> {
    let mut pin_key = expr.trim();
    if pin_key.is_empty() || pin_key.eq_ignore_ascii_case("pin") {
        pin_key = ctx.current_pin?;
    }
    lookup_ci(ctx.net_name_by_pin, pin_key)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text() {
        assert_eq!(
            parse_signex_markup("Hello"),
            vec![RichSegment::Normal("Hello".into())]
        );
    }

    #[test]
    fn empty_input() {
        assert_eq!(parse_signex_markup(""), Vec::<RichSegment>::new());
    }

    #[test]
    fn bold() {
        assert_eq!(
            parse_signex_markup("**bold**"),
            vec![RichSegment::Bold("bold".into())]
        );
    }

    #[test]
    fn italic() {
        assert_eq!(
            parse_signex_markup("*italic*"),
            vec![RichSegment::Italic("italic".into())]
        );
    }

    #[test]
    fn strike() {
        assert_eq!(
            parse_signex_markup("~~gone~~"),
            vec![RichSegment::Strike("gone".into())]
        );
    }

    #[test]
    fn superscript() {
        assert_eq!(
            parse_signex_markup("V^+^"),
            vec![
                RichSegment::Normal("V".into()),
                RichSegment::Superscript("+".into()),
            ]
        );
    }

    #[test]
    fn subscript() {
        assert_eq!(
            parse_signex_markup("V~CC~"),
            vec![
                RichSegment::Normal("V".into()),
                RichSegment::Subscript("CC".into()),
            ]
        );
    }

    #[test]
    fn overbar() {
        assert_eq!(
            parse_signex_markup("_~RESET~_"),
            vec![RichSegment::Overbar("RESET".into())]
        );
    }

    #[test]
    fn link() {
        assert_eq!(
            parse_signex_markup("[click](https://example.com)"),
            vec![RichSegment::Link {
                label: "click".into(),
                url: "https://example.com".into(),
            }]
        );
    }

    #[test]
    fn escape_sigils() {
        // Escape sigils with backslash: each escape consumes exactly one
        // sigil character. To escape a multi-char sigil like `~~`, escape
        // each tilde individually.
        assert_eq!(
            parse_signex_markup(r"\*literal\*"),
            vec![RichSegment::Normal("*literal*".into())]
        );
        assert_eq!(
            parse_signex_markup(r"\~\~not strike\~\~"),
            vec![RichSegment::Normal("~~not strike~~".into())]
        );
        assert_eq!(
            parse_signex_markup(r"\_\~not overbar\~\_"),
            vec![RichSegment::Normal("_~not overbar~_".into())]
        );
    }

    #[test]
    fn mixed_overbar_and_subscript() {
        // _~OE~_~0~ → overbar OE + subscript 0
        assert_eq!(
            parse_signex_markup("_~OE~_~0~"),
            vec![
                RichSegment::Overbar("OE".into()),
                RichSegment::Subscript("0".into()),
            ]
        );
    }

    #[test]
    fn unmatched_sigil_is_literal() {
        // No closing sigil → consume as literal.
        assert_eq!(
            parse_signex_markup("a*b"),
            vec![RichSegment::Normal("a*b".into())]
        );
    }

    #[test]
    fn auto_net_name_format_is_signex() {
        let pins = vec![
            ("U2".to_string(), "5".to_string()),
            ("R1".to_string(), "2".to_string()),
            ("R1".to_string(), "1".to_string()),
        ];
        // Format must be "unnamed-<sheet>:<ref>:<pin>" per the Apache-clean
        // remediation. Must NOT match the historical KiCad format string.
        assert_eq!(auto_net_name("", &pins), Some("unnamed-R1:1".to_string()));
        assert_eq!(
            auto_net_name("PowerSupply", &pins),
            Some("unnamed-PowerSupply:R1:1".to_string())
        );
    }

    #[test]
    fn auto_net_name_empty_pins() {
        assert_eq!(auto_net_name("", &[]), None);
    }

    #[test]
    fn evaluates_refdes_and_at_variables() {
        let mut at = HashMap::new();
        at.insert("Comment".to_string(), "Decoupling".to_string());
        let mut refdes = HashMap::new();
        refdes.insert("U1_UUID".to_string(), "U1".to_string());

        let ctx = ExpressionEvalContext {
            current_refdes: Some("U7"),
            at_variables: Some(&at),
            refdes_variables: Some(&refdes),
            ..ExpressionEvalContext::default()
        };

        let out = evaluate_expressions("${refdes:self} @{Comment} ${refdes:U1_UUID}", &ctx);
        assert_eq!(out, "U7 Decoupling U1");
    }

    #[test]
    fn evaluates_cell_and_net_name() {
        let mut nets = HashMap::new();
        nets.insert("A1".to_string(), "ADC_IN".to_string());
        let ctx = ExpressionEvalContext {
            current_pin: Some("A1"),
            cell: Some("2"),
            net_name_by_pin: Some(&nets),
            ..ExpressionEvalContext::default()
        };

        let out = evaluate_expressions("CELL() NET_NAME(pin)", &ctx);
        assert_eq!(out, "2 ADC_IN");
    }

    #[test]
    fn unresolved_expressions_are_preserved() {
        let ctx = ExpressionEvalContext::default();
        let out = evaluate_expressions("${refdes:U1} @{foo} NET_NAME(1)", &ctx);
        assert_eq!(out, "${refdes:U1} @{foo} NET_NAME(1)");
    }
}
