//! Rich text markup parser for KiCad text rendering.
//!
//! KiCad uses markup in text fields:
//! - `V_{CC}` → "V" normal + "CC" subscript
//! - `V^{+}` → "V" normal + "+" superscript
//! - `~{CS}` → "CS" with overbar
//! - Combinations: `~{WR}_{0}` → "WR" overbar + "0" subscript

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RichSegment {
    Normal(String),
    Subscript(String),
    Superscript(String),
    Overbar(String),
}

/// Parse KiCad markup into rich text segments.
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
            // Overbar: ~{...}
            b'~' if i + 1 < len && bytes[i + 1] == b'{' => {
                if !normal_buf.is_empty() {
                    segments.push(RichSegment::Normal(std::mem::take(&mut normal_buf)));
                }
                i += 2; // skip ~{
                let content = read_brace_content(bytes, &mut i);
                segments.push(RichSegment::Overbar(content));
            }
            // Subscript: _{...}
            b'_' if i + 1 < len && bytes[i + 1] == b'{' => {
                if !normal_buf.is_empty() {
                    segments.push(RichSegment::Normal(std::mem::take(&mut normal_buf)));
                }
                i += 2; // skip _{
                let content = read_brace_content(bytes, &mut i);
                segments.push(RichSegment::Subscript(content));
            }
            // Superscript: ^{...}
            b'^' if i + 1 < len && bytes[i + 1] == b'{' => {
                if !normal_buf.is_empty() {
                    segments.push(RichSegment::Normal(std::mem::take(&mut normal_buf)));
                }
                i += 2; // skip ^{
                let content = read_brace_content(bytes, &mut i);
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

fn read_brace_content(bytes: &[u8], i: &mut usize) -> String {
    let start = *i;
    let mut depth = 1;
    while *i < bytes.len() && depth > 0 {
        match bytes[*i] {
            b'{' => depth += 1,
            b'}' => depth -= 1,
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
}
