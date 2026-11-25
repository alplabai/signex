//! Typst writing helpers for the report. Every piece of project text goes in
//! as a quoted string in code (`#("...")`, a table cell `"..."`), never as
//! markup, so a name with `*`, `#` or `]` prints as written and cannot break
//! the file.

use crate::reference::quote;

/// A paragraph of plain text: the string on its own line, then a blank line.
pub(super) fn paragraph(text: &str) -> String {
    format!("#({})\n\n", quote(text))
}

/// A bulleted list of plain text items; nothing when there are none.
pub(super) fn bullets(items: &[String]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let cells: Vec<String> = items.iter().map(|item| quote(item)).collect();
    format!("#list({})\n", cells.join(", "))
}

/// A table with a bold header row and `caption`, every cell plain text.
/// Rows shorter than the header are padded with empty cells.
pub(super) fn table(caption: &str, header: &[&str], rows: &[Vec<String>]) -> String {
    let columns = header.len().max(1);
    let mut out = String::from("#figure(\n  table(\n");
    out.push_str(&format!("    columns: {columns},\n"));
    out.push_str("    align: left,\n");
    let head: Vec<String> = header
        .iter()
        .map(|cell| format!("strong({})", quote(cell)))
        .collect();
    out.push_str(&format!("    table.header({}),\n", head.join(", ")));
    for row in rows {
        let mut cells: Vec<String> = row.iter().take(columns).map(|c| quote(c)).collect();
        cells.resize(columns, quote(""));
        out.push_str(&format!("    {},\n", cells.join(", ")));
    }
    out.push_str("  ),\n");
    out.push_str(&format!("  caption: {},\n", quote(caption)));
    out.push_str(")\n");
    out
}

/// `text` safe for a `//` comment line: no line breaks.
pub(super) fn comment_text(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// `items` joined with commas, or a dash when there are none.
pub(super) fn listed(items: &[String]) -> String {
    if items.is_empty() {
        "—".to_string()
    } else {
        items.join(", ")
    }
}

/// `count` with the noun in the singular or plural (`1 error`, `2 errors`).
pub(super) fn counted(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}
