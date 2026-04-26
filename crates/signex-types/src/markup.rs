//! Rich text markup parser for Standard text rendering.
//!
//! Standard uses markup in text fields:
//! - `V_{CC}` → "V" normal + "CC" subscript
//! - `V^{+}` → "V" normal + "+" superscript
//! - `~{CS}` → "CS" with overbar
//! - Combinations: `~{WR}_{0}` → "WR" overbar + "0" subscript

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RichSegment {
    Normal(String),
    Subscript(String),
    Superscript(String),
    Overbar(String),
}

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

/// Expand Standard `{name}` escape tokens to their literal characters.
pub fn expand_standard_char_escapes(input: &str) -> String {
    if !input.contains('{') {
        return input.to_string();
    }

    input
        .replace("{slash}", "/")
        .replace("{backslash}", "\\")
        .replace("{tilde}", "~")
        .replace("{colon}", ":")
        .replace("{dollar}", "$")
        .replace("{space}", " ")
        .replace("{dblquote}", "\"")
        .replace("{lt}", "<")
        .replace("{gt}", ">")
        .replace("{bar}", "|")
}

/// Evaluate a subset of Standard/Altium-style expression variables.
///
/// Supported today:
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

        if bytes[i] == b'$'
            && i + 1 < bytes.len()
            && bytes[i + 1] == b'{'
            && let Some((expr, next_index)) = read_braced(input, i + 2)
        {
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

        if bytes[i] == b'@'
            && i + 1 < bytes.len()
            && bytes[i + 1] == b'{'
            && let Some((expr, next_index)) = read_braced(input, i + 2)
        {
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

        if starts_with_ascii_ci(bytes, i, b"CELL()") {
            if let Some(cell) = ctx.cell {
                out.push_str(cell);
            } else {
                out.push_str("CELL()");
            }
            i += "CELL()".len();
            continue;
        }

        if starts_with_ascii_ci(bytes, i, b"NET_NAME(")
            && let Some((arg, next_index)) =
                read_parenthesized(input, i + "NET_NAME(".len())
        {
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

        let ch = input[i..].chars().next().unwrap_or('\0');
        if ch == '\0' {
            break;
        }
        out.push(ch);
        i += ch.len_utf8();
    }

    out
}

/// Standard-style unnamed net fallback: `Net-(<refdes>-Pad<pin>)`.
///
/// Uses lexicographically smallest `(refdes, pin)` pair for deterministic
/// naming, matching the netlist side policy.
pub fn standard_auto_net_name_from_pins(pins: &[(String, String)]) -> Option<String> {
    pins.iter()
        .min_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)))
        .map(|(r, p)| format!("Net-({r}-Pad{p})"))
}

/// Parse Standard markup into rich text segments.
pub fn parse_markup(input: &str) -> Vec<RichSegment> {
    if input.is_empty() {
        return vec![];
    }

    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut segments = Vec::new();
    let mut i = 0;
    let mut normal_buf = String::new();

    while i < len {
        match bytes[i] {
            // Escaped markup sigils: \~ \_ \^ \$ \@ => literal sigil.
            b'\\' if i + 1 < len => {
                let next = bytes[i + 1];
                if matches!(next, b'~' | b'_' | b'^' | b'$' | b'@' | b'{' | b'}') {
                    normal_buf.push(next as char);
                    i += 2;
                    continue;
                }

                // Keep the backslash when this is not a recognised escaped sigil.
                normal_buf.push('\\');
                i += 1;
            }
            // Overbar: ~{...}
            b'~' if i + 1 < len && bytes[i + 1] == b'{' => {
                if !normal_buf.is_empty() {
                    segments.push(RichSegment::Normal(std::mem::take(&mut normal_buf)));
                }
                i += 2; // skip ~{
                let content = read_group_content(bytes, &mut i, b'{', b'}');
                segments.push(RichSegment::Overbar(content));
            }
            // Overbar alternative: ~(...) (Standard syntax help allows this form).
            b'~' if i + 1 < len && bytes[i + 1] == b'(' => {
                if !normal_buf.is_empty() {
                    segments.push(RichSegment::Normal(std::mem::take(&mut normal_buf)));
                }
                i += 2; // skip ~(
                let content = read_group_content(bytes, &mut i, b'(', b')');
                segments.push(RichSegment::Overbar(content));
            }
            // Subscript: _{...}
            b'_' if i + 1 < len && bytes[i + 1] == b'{' => {
                if !normal_buf.is_empty() {
                    segments.push(RichSegment::Normal(std::mem::take(&mut normal_buf)));
                }
                i += 2; // skip _{
                let content = read_group_content(bytes, &mut i, b'{', b'}');
                segments.push(RichSegment::Subscript(content));
            }
            // Superscript: ^{...}
            b'^' if i + 1 < len && bytes[i + 1] == b'{' => {
                if !normal_buf.is_empty() {
                    segments.push(RichSegment::Normal(std::mem::take(&mut normal_buf)));
                }
                i += 2; // skip ^{
                let content = read_group_content(bytes, &mut i, b'{', b'}');
                segments.push(RichSegment::Superscript(content));
            }
            _ => {
                // Regular character — handle multi-byte UTF-8
                let ch = input[i..].chars().next().unwrap();
                normal_buf.push(ch);
                i += ch.len_utf8();
            }
        }
    }

    if !normal_buf.is_empty() {
        segments.push(RichSegment::Normal(normal_buf));
    }

    segments
}

fn read_group_content(bytes: &[u8], i: &mut usize, open: u8, close: u8) -> String {
    let start = *i;
    let mut depth = 1;
    while *i < bytes.len() && depth > 0 {
        match bytes[*i] {
            c if c == open => depth += 1,
            c if c == close => depth -= 1,
            _ => {}
        }
        if depth > 0 {
            *i += 1;
        }
    }
    let content = String::from_utf8_lossy(&bytes[start..*i]).to_string();
    if *i < bytes.len() {
        *i += 1; // skip closing }
    }
    content
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text() {
        assert_eq!(
            parse_markup("Hello"),
            vec![RichSegment::Normal("Hello".into())]
        );
    }

    #[test]
    fn empty() {
        assert_eq!(parse_markup(""), Vec::<RichSegment>::new());
    }

    #[test]
    fn subscript_vcc() {
        assert_eq!(
            parse_markup("V_{CC}"),
            vec![
                RichSegment::Normal("V".into()),
                RichSegment::Subscript("CC".into()),
            ]
        );
    }

    #[test]
    fn overbar_cs() {
        assert_eq!(
            parse_markup("~{CS}"),
            vec![RichSegment::Overbar("CS".into())]
        );
    }

    #[test]
    fn overbar_parentheses_form() {
        assert_eq!(
            parse_markup("~(CLK)"),
            vec![RichSegment::Overbar("CLK".into())]
        );
    }

    #[test]
    fn superscript() {
        assert_eq!(
            parse_markup("V^{+}"),
            vec![
                RichSegment::Normal("V".into()),
                RichSegment::Superscript("+".into()),
            ]
        );
    }

    #[test]
    fn overbar_then_subscript() {
        assert_eq!(
            parse_markup("~{WR}_{0}"),
            vec![
                RichSegment::Overbar("WR".into()),
                RichSegment::Subscript("0".into()),
            ]
        );
    }

    #[test]
    fn mixed_with_plain() {
        assert_eq!(
            parse_markup("I_{OUT} = 1A"),
            vec![
                RichSegment::Normal("I".into()),
                RichSegment::Subscript("OUT".into()),
                RichSegment::Normal(" = 1A".into()),
            ]
        );
    }

    #[test]
    fn no_markup_chars() {
        // Lone ~ _ ^ without { are treated as normal text
        assert_eq!(
            parse_markup("a~b_c^d"),
            vec![RichSegment::Normal("a~b_c^d".into())]
        );
    }

    #[test]
    fn escaped_markup_is_literal() {
        assert_eq!(
            parse_markup("\\_{A} \\^{2} \\~{RST} \\${ROW} \\@{x+y}"),
            vec![RichSegment::Normal("_{A} ^{2} ~{RST} ${ROW} @{x+y}".into())]
        );
    }

    #[test]
    fn expands_standard_char_escapes() {
        assert_eq!(expand_standard_char_escapes("A{slash}B{dollar}C"), "A/B$C");
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

    #[test]
    fn standard_auto_net_name_uses_lowest_pair() {
        let pins = vec![
            ("U2".to_string(), "5".to_string()),
            ("R1".to_string(), "2".to_string()),
            ("R1".to_string(), "1".to_string()),
        ];
        assert_eq!(
            standard_auto_net_name_from_pins(&pins),
            Some("Net-(R1-Pad1)".to_string())
        );
    }
}
