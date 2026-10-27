//! Byte-level fast path for single-row edits (#416).
//!
//! `append_row` / `update_row` / `delete_row` used to parse the whole table
//! into `ComponentRow`s and re-serialise every row (JSON cells included) to
//! change one. Here the file is only *scanned* for record boundaries and the
//! `row_id` cell; untouched rows are copied through as raw bytes and just the
//! edited record is encoded. The result still lands via `atomic_write`
//! (temp + fsync + rename), so crash-safety is unchanged: a crash leaves
//! the old table or the new one, never a torn file.
//!
//! Stored rows are not validated: only the `row_id` cell is read, so a short
//! or otherwise malformed row is copied through byte-for-byte rather than
//! blocking an edit to an unrelated row.
//!
//! Every function returns `Ok(None)` when the file is not in a shape the fast
//! path is sure about (missing, empty, unknown header, unparsable row id);
//! the caller then takes the original read-all/rewrite-all path.

use std::path::Path;

use csv::{ByteRecord, QuoteStyle, ReaderBuilder, WriterBuilder};
use uuid::Uuid;

use super::{ALTERNATE_SYMBOLS_COLUMN, alternate_symbols_cell, check_columns, row_to_record};
use crate::adapter::LibraryError;
use crate::component::ComponentRow;

/// One data record's byte range (terminator included) and its row id.
struct Span {
    start: usize,
    end: usize,
    row_id: Uuid,
}

struct Scan {
    bytes: Vec<u8>,
    /// Byte offset where the first data record starts (end of the header).
    data_start: usize,
    has_alt: bool,
    spans: Vec<Span>,
}

fn scan(path: &Path) -> Result<Option<Scan>, LibraryError> {
    let bytes = match std::fs::read(path) {
        Ok(b) if !b.is_empty() => b,
        Ok(_) => return Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(LibraryError::Io(e)),
    };
    let mut rdr = ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .flexible(true)
        .from_reader(&bytes[..]);
    let mut rec = ByteRecord::new();
    if !matches!(rdr.read_byte_record(&mut rec), Ok(true)) {
        return Ok(None);
    }
    let Ok(header) = rec
        .iter()
        .map(std::str::from_utf8)
        .collect::<Result<Vec<_>, _>>()
    else {
        return Ok(None);
    };
    if check_columns(&header).is_err() {
        return Ok(None);
    }
    let has_alt = header.last() == Some(&ALTERNATE_SYMBOLS_COLUMN);
    // csv's own record positions can lag by the LF of a CRLF terminator, so a
    // record's range is "where the previous one ended" to "where the reader
    // stopped, plus any terminator bytes still pending".
    let end_of = |rdr: &csv::Reader<&[u8]>| {
        let mut e = rdr.position().byte() as usize;
        while matches!(bytes.get(e), Some(&(13 | 10))) {
            e += 1;
        }
        e
    };
    let data_start = end_of(&rdr);
    let mut prev_end = data_start;
    let mut spans: Vec<Span> = Vec::new();
    loop {
        match rdr.read_byte_record(&mut rec) {
            Ok(true) => {}
            Ok(false) => break,
            // A parse error mid-file means the table is not well-formed:
            // let the full path report it exactly as before.
            Err(_) => return Ok(None),
        }
        let Some(row_id) = rec
            .get(0)
            .and_then(|c| std::str::from_utf8(c).ok())
            .and_then(|s| Uuid::parse_str(s).ok())
        else {
            return Ok(None);
        };
        let end = end_of(&rdr);
        spans.push(Span {
            start: prev_end,
            end,
            row_id,
        });
        prev_end = end;
    }
    Ok(Some(Scan {
        bytes,
        data_start,
        has_alt,
        spans,
    }))
}

/// Encode `row` as one TSV record in the column layout of `has_alt`.
fn encode(row: &ComponentRow, has_alt: bool) -> Result<Vec<u8>, LibraryError> {
    let mut record = row_to_record(row)?;
    if has_alt {
        record.push(alternate_symbols_cell(row)?.unwrap_or_default());
    }
    let mut wtr = WriterBuilder::new()
        .delimiter(b'\t')
        .quote_style(QuoteStyle::Necessary)
        .from_writer(Vec::<u8>::new());
    wtr.write_record(record)
        .map_err(|e| LibraryError::Backend(format!("write row: {e}")))?;
    wtr.into_inner()
        .map_err(|e| LibraryError::Backend(format!("flush table: {e}")))
}

fn not_found(id: Uuid, path: &Path) -> LibraryError {
    LibraryError::NotFound(format!("row {id} not in table {}", path.display()))
}

/// `None` when the row cannot be expressed in the file's column layout.
pub(super) fn append(path: &Path, row: &ComponentRow) -> Result<Option<()>, LibraryError> {
    let Some(mut s) = scan(path)? else {
        return Ok(None);
    };
    if !s.has_alt && !row.alternate_symbols.is_empty() {
        return Ok(None);
    }
    if !s.bytes.ends_with(b"\n") {
        s.bytes.push(b'\n');
    }
    s.bytes.extend(encode(row, s.has_alt)?);
    signex_types::atomic_io::atomic_write(path, &s.bytes)?;
    Ok(Some(()))
}

pub(super) fn update(path: &Path, row: &ComponentRow) -> Result<Option<()>, LibraryError> {
    let Some(s) = scan(path)? else {
        return Ok(None);
    };
    if !s.has_alt && !row.alternate_symbols.is_empty() {
        return Ok(None);
    }
    let Some(span) = s.spans.iter().find(|sp| sp.row_id == row.row_id) else {
        return Err(not_found(row.row_id, path));
    };
    let mut out = Vec::with_capacity(s.bytes.len());
    out.extend_from_slice(&s.bytes[..span.start]);
    out.extend(encode(row, s.has_alt)?);
    out.extend_from_slice(&s.bytes[span.end..]);
    signex_types::atomic_io::atomic_write(path, &out)?;
    Ok(Some(()))
}

pub(super) fn delete(path: &Path, row_id: Uuid) -> Result<Option<()>, LibraryError> {
    let Some(s) = scan(path)? else {
        return Ok(None);
    };
    if !s.spans.iter().any(|sp| sp.row_id == row_id) {
        return Err(not_found(row_id, path));
    }
    let mut out = Vec::with_capacity(s.bytes.len());
    out.extend_from_slice(&s.bytes[..s.data_start]);
    for sp in s.spans.iter().filter(|sp| sp.row_id != row_id) {
        out.extend_from_slice(&s.bytes[sp.start..sp.end]);
    }
    signex_types::atomic_io::atomic_write(path, &out)?;
    Ok(Some(()))
}
