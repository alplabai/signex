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

use super::*;
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
/// `.snxlib` writer's whitespace-flexible style).
pub fn write_tsv_block<R: SnxTable>(rows: &[R]) -> String {
    let columns = R::columns();
    if columns.is_empty() {
        return String::new();
    }

    // Pre-compute every row's encoded cell so we can pad to the
    // widest column.
    let mut all_rows: Vec<Vec<String>> = Vec::with_capacity(rows.len() + 1);
    all_rows.push(columns.iter().map(|c| (*c).to_string()).collect());
    for row in rows {
        let cells = row.to_row();
        debug_assert_eq!(
            cells.len(),
            columns.len(),
            "to_row produced {} cells but columns has {}",
            cells.len(),
            columns.len()
        );
        all_rows.push(cells.into_iter().map(|c| encode_cell(&c)).collect());
    }

    // Compute per-column widths. Two-space separator between columns.
    let mut widths = vec![0usize; columns.len()];
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
    out
}

/// Parse a TSV block: validate the header against `R::columns()`,
/// then parse each data row through `R::from_row`.
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
    let header_cells = split_row(header_line);
    let expected: Vec<String> = R::columns().iter().map(|c| (*c).to_string()).collect();
    if header_cells != expected {
        return Err(FormatError::TsvHeaderMismatch {
            block: block.to_string(),
            got: header_cells,
            expected,
        });
    }

    let mut rows = Vec::new();
    for (idx, line) in lines.enumerate() {
        let cells = split_row(line);
        if cells.len() != expected.len() {
            return Err(FormatError::TsvCellCountMismatch {
                block: block.to_string(),
                row: idx,
                got: cells.len(),
                expected: expected.len(),
                columns: expected,
            });
        }
        let decoded: Vec<String> = cells.iter().map(|c| decode_cell(c)).collect();
        let refs: Vec<&str> = decoded.iter().map(String::as_str).collect();
        rows.push(R::from_row(&refs, block, idx)?);
    }
    Ok(rows)
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

pub(in crate::format) fn write_tsv_section<R: SnxTable>(out: &mut String, name: &str, rows: &[R]) {
    let body = write_tsv_block(rows);
    out.push_str(&format!("\n[{name}]\n"));
    out.push_str("content = \"\"\"\n");
    out.push_str(&escape_tsv_body_for_toml(&body));
    out.push_str("\"\"\"\n");
}
