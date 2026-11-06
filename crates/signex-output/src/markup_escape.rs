//! The one escaper the HTML and SVG emitters share.

/// Escape `s` for HTML or XML text and double-quoted attribute content.
/// `&#39;` is valid in both, so one table serves every emitter.
pub(crate) fn escape_markup(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_every_special_and_keeps_multibyte() {
        assert_eq!(
            escape_markup("<a href=\"x\">R&D 日本é</a>"),
            "&lt;a href=&quot;x&quot;&gt;R&amp;D 日本é&lt;/a&gt;"
        );
        assert_eq!(escape_markup(""), "");
    }

    #[test]
    fn apostrophe_is_numeric_in_html_and_svg_alike() {
        assert_eq!(escape_markup("it's"), "it&#39;s");
    }
}
