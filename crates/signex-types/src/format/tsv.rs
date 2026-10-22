//! TSV bulk-block codec: cell encode/decode, TOML-envelope escaping,
//! row splitting, the block reader/writer, the field-level parse
//! helpers, and the numeric formatter.
//!
//! Pure code motion out of `mod.rs`. The cross-module helpers are
//! `pub(in crate::format)` (visible to the whole `format` module tree,
//! exactly as when they lived in the single file); the block
//! reader/writer stay `pub` (part of the crate's public surface via
//! the `format` re-exports). CODE MOTION ONLY — the escaping and
//! quoting rules are byte-for-byte unchanged (a wrong byte here makes a
//! schematic unreloadable, #96).

use super::unknown_columns::check_foreign_names;
use super::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

/// Encode a single TSV cell. Empty strings emit `""` so column
/// boundaries stay legible when split on whitespace.
///
/// A cell is quoted when it contains whitespace, a `"`, a `\`, or is
/// the literal `-` (which `decode_cell` maps to empty per the format
/// spec — without quoting, a real `-` value round-trips to `""`).
/// Inside a quoted cell, backslash and the control characters that
/// would otherwise break TSV row / whitespace splitting are escaped
/// with backslash sequences, and inner quotes are doubled (CSV style,
/// preserved for backward compatibility with existing files).
pub(in crate::format) fn encode_cell(cell: &str) -> String {
    if cell.is_empty() {
        return "\"\"".to_string();
    }
    let needs_quote = cell == "-"
        || cell.contains(char::is_whitespace)
        || cell.contains('"')
        || cell.contains('\\');
    if !needs_quote {
        return cell.to_string();
    }
    let mut escaped = String::with_capacity(cell.len() + 2);
    for ch in cell.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '"' => escaped.push_str("\"\""),
            _ => escaped.push(ch),
        }
    }
    format!("\"{escaped}\"")
}

/// Decode a single TSV cell. `""` and a bare `-` return empty (the
/// latter per the format spec); surrounding double quotes strip, with
/// inner `""` collapsing back to `"` and `\\` / `\n` / `\r` / `\t`
/// reversing the escapes `encode_cell` writes.
pub(in crate::format) fn decode_cell(cell: &str) -> String {
    if cell == "\"\"" || cell == "-" {
        return String::new();
    }
    if cell.starts_with('"') && cell.ends_with('"') && cell.len() >= 2 {
        let inner: Vec<char> = cell[1..cell.len() - 1].chars().collect();
        let mut out = String::with_capacity(inner.len());
        let mut i = 0;
        while i < inner.len() {
            let c = inner[i];
            if c == '"' && i + 1 < inner.len() && inner[i + 1] == '"' {
                // Doubled quote (legacy CSV-style escape).
                out.push('"');
                i += 2;
            } else if c == '\\' && i + 1 < inner.len() {
                match inner[i + 1] {
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    't' => out.push('\t'),
                    '\\' => out.push('\\'),
                    other => {
                        // Unknown escape — keep both chars verbatim so
                        // nothing is silently dropped.
                        out.push('\\');
                        out.push(other);
                    }
                }
                i += 2;
            } else {
                out.push(c);
                i += 1;
            }
        }
        return out;
    }
    cell.to_string()
}

/// Escape a rendered TSV block so it can be embedded inside a TOML
/// multi-line *basic* string (`"""..."""`). TOML treats `\` as an
/// escape introducer and ends the string on `"""`, so a cell holding
/// a Windows path (`C:\...`), a literal quote, or an inch value like
/// `1/4"` would otherwise produce a file that can never be reopened.
///
/// Backslashes are doubled and any run of three or more quotes is
/// broken with `\"` (still a literal quote to TOML) so it cannot be
/// read as the closing delimiter. Interior newlines and tabs are
/// valid literally in a multi-line basic string and are preserved so
/// the block stays line-diffable. Every other C0 control byte (and
/// DEL) is illegal unescaped in a TOML basic string, so it is written
/// as `\uXXXX` — otherwise a cell holding e.g. a BEL or VT byte saves
/// fine but `toml::from_str` rejects the file on reopen (#386).
pub(in crate::format) fn escape_tsv_body_for_toml(body: &str) -> String {
    let mut out = String::with_capacity(body.len() + 8);
    let mut quote_run = 0usize;
    for ch in body.chars() {
        if ch == '"' {
            quote_run += 1;
            if quote_run >= 3 {
                out.push_str("\\\"");
                quote_run = 0;
            } else {
                out.push('"');
            }
            continue;
        }
        quote_run = 0;
        match ch {
            '\\' => out.push_str("\\\\"),
            '\r' => out.push_str("\\r"),
            '\n' | '\t' => out.push(ch),
            c if (c as u32) < 0x20 || c as u32 == 0x7F => {
                out.push_str(&format!("\\u{:04X}", c as u32));
            }
            _ => out.push(ch),
        }
    }
    out
}

/// Split a TSV row on whitespace, honouring `"` quoting so cells
/// containing spaces are kept atomic.
fn split_row(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut buf = String::new();
    let mut in_quotes = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_quotes {
            buf.push(c);
            if c == '"' {
                if i + 1 < chars.len() && chars[i + 1] == '"' {
                    // doubled quote — keep advancing, leave to decode_cell
                    buf.push('"');
                    i += 2;
                    continue;
                }
                in_quotes = false;
            }
            i += 1;
        } else if c == '"' {
            buf.push(c);
            in_quotes = true;
            i += 1;
        } else if c.is_whitespace() {
            if !buf.is_empty() {
                cells.push(std::mem::take(&mut buf));
            }
            i += 1;
        } else {
            buf.push(c);
            i += 1;
        }
    }
    if !buf.is_empty() {
        cells.push(buf);
    }
    cells
}

/// Write a TSV block: header row + one row per item, columns aligned
/// to the longest cell per column for legibility (matches the
/// `.snxlib` writer's whitespace-flexible style). Foreign columns
/// follow the known ones — see [`UnknownColumns`] for the union rule.
///
/// Fails with the same caps the reader enforces (see
/// `unknown_columns`) when the rows' foreign-column union is too wide,
/// before anything is written — never truncating.
pub fn write_tsv_block<R: SnxTable>(block: &str, rows: &[R]) -> Result<String, FormatError> {
    let columns = R::columns();
    if columns.is_empty() {
        return Ok(String::new());
    }

    let foreign = foreign_column_names(rows);
    check_foreign_names(block, &foreign)?;
    if let Some(shadow) = foreign.iter().find(|n| columns.contains(&n.as_str())) {
        return Err(FormatError::TsvForeignColumnShadowsKnown {
            block: block.to_string(),
            column: shadow.clone(),
        });
    }

    // Pre-compute every row's encoded cell so we can pad to the
    // widest column.
    let mut all_rows: Vec<Vec<String>> = Vec::with_capacity(rows.len() + 1);
    let mut header: Vec<String> = columns.iter().map(|c| (*c).to_string()).collect();
    header.extend(foreign.iter().map(|name| encode_cell(name)));
    all_rows.push(header);
    for row in rows {
        let mut cells = row.to_row();
        debug_assert_eq!(
            cells.len(),
            columns.len(),
            "to_row produced {} cells but columns has {}",
            cells.len(),
            columns.len()
        );
        // One map per row (<= MAX_FOREIGN_COLUMNS entries) keeps the
        // writer linear instead of columns x columns per row.
        let unknown: HashMap<&str, &str> = row.unknown_columns().iter().collect();
        cells.extend(foreign.iter().map(|name| {
            unknown
                .get(name.as_str())
                .copied()
                .unwrap_or_default()
                .to_string()
        }));
        all_rows.push(cells.into_iter().map(|c| encode_cell(&c)).collect());
    }

    // Compute per-column widths. Two-space separator between columns.
    let mut widths = vec![0usize; columns.len() + foreign.len()];
    for row in &all_rows {
        for (idx, cell) in row.iter().enumerate() {
            if idx < widths.len() && cell.chars().count() > widths[idx] {
                widths[idx] = cell.chars().count();
            }
        }
    }

    let mut out = String::new();
    for row in &all_rows {
        for (idx, cell) in row.iter().enumerate() {
            if idx > 0 {
                out.push_str("  ");
            }
            // Don't pad the last column — avoids trailing spaces.
            if idx + 1 == row.len() {
                out.push_str(cell);
            } else {
                let pad = widths[idx].saturating_sub(cell.chars().count());
                out.push_str(cell);
                for _ in 0..pad {
                    out.push(' ');
                }
            }
        }
        out.push('\n');
    }
    Ok(out)
}

/// Union of the rows' foreign column names, first-seen order.
fn foreign_column_names<R: SnxTable>(rows: &[R]) -> Vec<String> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut names: Vec<String> = Vec::new();
    for row in rows {
        for (name, _) in row.unknown_columns().iter() {
            if seen.insert(name) {
                names.push(name.to_string());
            }
        }
    }
    names
}

/// Parse a TSV block. Columns resolve by header name: every column in
/// `R::columns()` must be present (any order); extra columns are
/// captured per row as [`UnknownColumns`] instead of refusing the file.
pub fn parse_tsv_block<R: SnxTable>(block: &str, content: &str) -> Result<Vec<R>, FormatError> {
    // Strip the leading/trailing newlines TOML's literal multi-line
    // string padding adds, but preserve interior newlines.
    let trimmed = content.trim_matches('\n');
    if trimmed.trim().is_empty() {
        return Err(FormatError::TsvEmpty {
            block: block.to_string(),
        });
    }

    let mut lines = trimmed.split('\n').filter(|l| !l.trim().is_empty());
    let header_line = lines.next().ok_or_else(|| FormatError::TsvEmpty {
        block: block.to_string(),
    })?;
    let header: Vec<String> = split_row(header_line)
        .iter()
        .map(|c| decode_cell(c))
        .collect();
    let layout = HeaderLayout::resolve::<R>(block, header)?;

    let mut rows = Vec::new();
    for (idx, line) in lines.enumerate() {
        let cells = split_row(line);
        if cells.len() != layout.header.len() {
            return Err(FormatError::TsvCellCountMismatch {
                block: block.to_string(),
                row: idx,
                got: cells.len(),
                expected: layout.header.len(),
                columns: layout.header,
            });
        }
        let mut decoded: Vec<String> = cells.iter().map(|c| decode_cell(c)).collect();
        let known: Vec<&str> = layout.known.iter().map(|&i| decoded[i].as_str()).collect();
        let parsed = R::from_row(&known, block, idx)?;
        // Move the foreign cells out (no copy); names are shared Arcs.
        let foreign = layout
            .foreign
            .iter()
            .zip(&layout.foreign_names)
            .map(|(&i, name)| (Arc::clone(name), std::mem::take(&mut decoded[i])))
            .collect();
        rows.push(parsed.with_unknown_columns(UnknownColumns::from_pairs(foreign)));
    }
    Ok(rows)
}

/// Where each known column sits in a block's actual header, and which
/// header positions are foreign.
struct HeaderLayout {
    header: Vec<String>,
    /// Header index of each `R::columns()` entry, in that order.
    known: Vec<usize>,
    /// Header indices of the foreign columns, in file order.
    foreign: Vec<usize>,
    /// The foreign column names, allocated once and shared by every row.
    foreign_names: Vec<Arc<str>>,
}

impl HeaderLayout {
    /// Validate the header in O(columns): duplicates and known-column
    /// lookup share one `HashMap`, then the foreign-column caps (see
    /// `unknown_columns`) apply.
    fn resolve<R: SnxTable>(block: &str, header: Vec<String>) -> Result<Self, FormatError> {
        let mut index_of: HashMap<&str, usize> = HashMap::with_capacity(header.len());
        for (i, name) in header.iter().enumerate() {
            if index_of.insert(name.as_str(), i).is_some() {
                return Err(FormatError::TsvDuplicateColumn {
                    block: block.to_string(),
                    column: name.clone(),
                });
            }
        }
        let known: Option<Vec<usize>> = R::columns()
            .iter()
            .map(|c| index_of.get(c).copied())
            .collect();
        let Some(known) = known else {
            return Err(FormatError::TsvHeaderMismatch {
                block: block.to_string(),
                got: header,
                expected: R::columns().iter().map(|c| (*c).to_string()).collect(),
            });
        };
        let known_set: HashSet<usize> = known.iter().copied().collect();
        let foreign: Vec<usize> = (0..header.len())
            .filter(|i| !known_set.contains(i))
            .collect();
        let names: Vec<&str> = foreign.iter().map(|&i| header[i].as_str()).collect();
        check_foreign_names(block, &names)?;
        let foreign_names: Vec<Arc<str>> = names.into_iter().map(Arc::from).collect();
        Ok(Self {
            header,
            known,
            foreign,
            foreign_names,
        })
    }
}

// ---------------------------------------------------------------------------
// Field-level parse helpers
// ---------------------------------------------------------------------------

pub(in crate::format) fn parse_i64(
    value: &str,
    block: &str,
    row: usize,
    field: &str,
) -> Result<i64, FormatError> {
    value
        .parse()
        .map_err(|e: std::num::ParseIntError| FormatError::TsvFieldParse {
            block: block.to_string(),
            row,
            field: field.to_string(),
            message: e.to_string(),
        })
}

pub(in crate::format) fn parse_f64(
    value: &str,
    block: &str,
    row: usize,
    field: &str,
) -> Result<f64, FormatError> {
    if value.is_empty() {
        return Ok(0.0);
    }
    value
        .parse()
        .map_err(|e: std::num::ParseFloatError| FormatError::TsvFieldParse {
            block: block.to_string(),
            row,
            field: field.to_string(),
            message: e.to_string(),
        })
}

pub(in crate::format) fn parse_uuid(
    value: &str,
    block: &str,
    row: usize,
    field: &str,
) -> Result<Uuid, FormatError> {
    // MD-7: an empty UUID cell is corruption, not "orphan" — surface it
    // so a pad row with a missing uuid cell can't silently merge with
    // the synthetic orphan-pad footprint that `SnxPcb::parse` builds at
    // line 1688 (that footprint constructs its own `Uuid::nil()`; this
    // helper does not need to provide one).
    if value.is_empty() {
        return Err(FormatError::TsvFieldParse {
            block: block.to_string(),
            row,
            field: field.to_string(),
            message: "empty uuid cell".to_string(),
        });
    }
    Uuid::parse_str(value).map_err(|e| FormatError::TsvFieldParse {
        block: block.to_string(),
        row,
        field: field.to_string(),
        message: e.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Numeric formatting
// ---------------------------------------------------------------------------

/// Format an `f64` for TSV: trailing zeros stripped to keep diffs
/// minimal. Whole numbers emit as `0` rather than `0.0`.
///
/// HI-13: the previous `< 1e15` guard was looser than the actual
/// `i64::MAX as f64` boundary, and the `as i64` cast would wrap on
/// the gap between them. Use the real cast bound and route non-finite
/// inputs through `format!("{f}")` (which produces `"NaN"` / `"inf"`
/// — visible at parse time rather than silently corrupted).
pub(in crate::format) fn format_f64(f: f64) -> String {
    if f == 0.0 {
        return "0".to_string();
    }
    if f.is_finite() && f.fract() == 0.0 && f.abs() < (i64::MAX as f64) {
        return format!("{}", f as i64);
    }
    format!("{f}")
}

// ---------------------------------------------------------------------------
// TSV section writer (TOML `[name]` + literal multi-line `content`)
// ---------------------------------------------------------------------------

pub(in crate::format) fn write_tsv_section<R: SnxTable>(
    out: &mut String,
    name: &str,
    rows: &[R],
) -> Result<(), FormatError> {
    let body = write_tsv_block(name, rows)?;
    out.push_str(&format!("\n[{name}]\n"));
    out.push_str("content = \"\"\"\n");
    out.push_str(&escape_tsv_body_for_toml(&body));
    out.push_str("\"\"\"\n");
    Ok(())
}
