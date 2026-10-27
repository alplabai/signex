//! Cell helpers shared by the symbol and footprint TSV codecs.

/// Format an `f64` for a TSV cell. `0.0` emits literally as `"0"` so
/// the most common default is short; non-zero values use the
/// shortest precision-preserving form via `Display`. Cells must be
/// re-parseable by `f64::from_str`.
///
/// `None` for NaN / ±Inf: `Display` would write `NaN` / `inf`, which
/// the reader refuses (#678), and an empty cell would silently reset the
/// value on reopen. The caller turns it into a save error naming the
/// pin or pad and column (#828).
pub(super) fn fmt_f64(v: f64) -> Option<String> {
    if v == 0.0 {
        Some("0".to_string())
    } else if !v.is_finite() {
        None
    } else {
        Some(format!("{v}"))
    }
}

/// Join list items into one cell with `|` between them. A `|` or `\` inside
/// an item is written `\|` / `\\` so the item survives [`split_list`].
pub(super) fn join_list(items: &[String]) -> String {
    let mut out = String::new();
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push('|');
        }
        for c in item.chars() {
            if matches!(c, '|' | '\\') {
                out.push('\\');
            }
            out.push(c);
        }
    }
    out
}

/// Inverse of [`join_list`]. A backslash escapes only `|` and `\`; any other
/// backslash is kept as written, so cells saved before escaping existed (which
/// never contained an escape) load unchanged. An empty cell is an empty list.
pub(super) fn split_list(cell: &str) -> Vec<String> {
    if cell.is_empty() {
        return Vec::new();
    }
    let mut items = vec![String::new()];
    let mut chars = cell.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '|' => items.push(String::new()),
            '\\' if matches!(chars.peek(), Some('|' | '\\')) => {
                let escaped = chars.next().unwrap_or('\\');
                items.last_mut().unwrap_or(&mut String::new()).push(escaped);
            }
            other => items.last_mut().unwrap_or(&mut String::new()).push(other),
        }
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_round_trips_pipes_and_backslashes() {
        let items: Vec<String> = ["SPI|MOSI", "a\\b", "x\\", "\\|", "plain", ""]
            .map(String::from)
            .into();
        assert_eq!(split_list(&join_list(&items)), items);
    }

    #[test]
    fn unescaped_legacy_cell_splits_as_before() {
        assert_eq!(split_list("SDA|SCL"), ["SDA", "SCL"]);
        assert_eq!(split_list("a\\b|c"), ["a\\b", "c"]);
        assert!(split_list("").is_empty());
    }
}
