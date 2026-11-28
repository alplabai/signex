//! What a TOML literal string can hold, for every writer that embeds user
//! text in one (`.snxsym` pin tables, `.snxlib` tables, `.snxsim` bodies).
//!
//! TOML literal strings allow tab but no other control character
//! (U+0000-U+001F, U+007F); a multi-line one also allows a newline (LF or
//! CRLF). A file holding any other one is written fine and then refused by
//! every reader, so the writers refuse such text before they write (#3129).

/// The first character of a TSV cell embedded in a literal string that the
/// file cannot store: any ASCII control character. Tab and LF are included
/// because they separate the cells and rows of the table.
pub fn first_unstorable_in_cell(text: &str) -> Option<char> {
    text.chars().find(char::is_ascii_control)
}

/// The first character of a multi-line literal string's body that TOML
/// forbids: a control character other than tab, LF, or a CR that starts a
/// CRLF.
pub fn first_unstorable_in_multiline(text: &str) -> Option<char> {
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        let allowed = match c {
            '\t' | '\n' => true,
            '\r' => chars.peek() == Some(&'\n'),
            _ => !c.is_ascii_control(),
        };
        if !allowed {
            return Some(c);
        }
    }
    None
}

/// `c` as a message names it: `U+000D`.
pub fn code_point(c: char) -> String {
    format!("U+{:04X}", u32::from(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cell_refuses_every_ascii_control_character_and_keeps_the_rest() {
        for c in (0u8..0x20).chain([0x7f]).map(char::from) {
            assert_eq!(
                first_unstorable_in_cell(&format!("A{c}B")),
                Some(c),
                "{c:?}"
            );
        }
        assert_eq!(first_unstorable_in_cell("VCC \u{2212}IN \u{85}"), None);
        assert_eq!(
            first_unstorable_in_cell("A\u{0}B\rC"),
            Some('\0'),
            "first one"
        );
    }

    #[test]
    fn a_multiline_body_keeps_tab_lf_and_crlf_and_refuses_the_rest() {
        assert_eq!(first_unstorable_in_multiline("a\tb\nc\r\nd"), None);
        assert_eq!(first_unstorable_in_multiline("a\rb"), Some('\r'));
        assert_eq!(first_unstorable_in_multiline("a\r"), Some('\r'));
        assert_eq!(first_unstorable_in_multiline("a\u{0}"), Some('\0'));
        assert_eq!(first_unstorable_in_multiline("a\u{7f}"), Some('\u{7f}'));
    }

    #[test]
    fn a_code_point_is_named_with_four_hex_digits() {
        assert_eq!(code_point('\r'), "U+000D");
        assert_eq!(code_point('\u{7f}'), "U+007F");
        assert_eq!(code_point('\0'), "U+0000");
    }
}
