//! TSV reader/writer for component tables (Altium DBLib model).
//!
//! Per `v0.9-refactor-2-plan.md` §2.4, every adapter (LocalGit, Database,
//! future flavours) shares one column schema:
//!
//! ```text
//! row_id  internal_pn  class  datasheet  state  symbol_ref  footprint_ref
//! sim_ref  primary_mpn  alternates  supply  parameters  pin_map_overrides
//! created  updated  content_hash
//! ```
//!
//! Scalar columns (`row_id`, `internal_pn`, `class`, `state`, `created`,
//! `updated`, `content_hash`) are written as plain strings. Nested columns
//! (`PrimitiveRef`, `ManufacturerPart`, `Vec<…>`, `ParamMap`, `PlmReserved`)
//! are JSON-encoded inside the cell.
//!
//! `Option<PrimitiveRef>` cells: empty string = `None`, JSON = `Some`.
//!
//! This module is the *file format*; commit / branch handling lives
//! in [`crate::adapters::local_git::LocalGitAdapter`]. The unit
//! tests here only exercise the TSV serialisation contract.

use std::path::Path;

use chrono::{DateTime, Utc};
use csv::{QuoteStyle, ReaderBuilder, WriterBuilder};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::adapter::LibraryError;
use crate::component::{ComponentRow, DatasheetRef, PinPadOverride, PlmReserved};
use crate::identity::{ComponentClass, InternalPn, RowId};
use crate::lifecycle::LifecycleState;
use crate::manufacturer::{DistributorListing, ManufacturerPart};
use crate::param::ParamMap;
use crate::primitive::PrimitiveRef;
use crate::validate::validate_row_for_write;

/// Header row — the canonical column ordering for every adapter.
///
/// Renaming or reordering this is a wire-format break: existing TSV files
/// would parse into the wrong fields. Add new columns at the end with a
/// `#[serde(default)]` on the row struct.
///
/// An optional 22nd column, [`ALTERNATE_SYMBOLS_COLUMN`], follows these 21.
/// Readers accept both widths; writers emit it only when a row has
/// alternate symbols, so tables without them stay byte-identical.
pub const TABLE_HEADER: &[&str] = &[
    "row_id",
    "internal_pn",
    "class",
    "datasheet",
    "state",
    "symbol_ref",
    "footprint_ref",
    "sim_ref",
    "primary_mpn",
    "alternates",
    "supply",
    "parameters",
    "pin_map_overrides",
    "created",
    "updated",
    "content_hash",
    // Stage 14 of v0.9-snxlib-as-file: entity versioning + released
    // flag. Appended at the end so pre-Stage-14 fixtures (which can't
    // round-trip these columns) need updating but the v0.9 wire
    // format stays additive — not a renumbering of existing columns.
    "version",
    "released",
    "symbol_version",
    "footprint_version",
    "sim_version",
];

mod alternate_symbols;
mod splice;
pub use alternate_symbols::ALTERNATE_SYMBOLS_COLUMN;
use alternate_symbols::alternate_symbols_from_cell;
pub(crate) use alternate_symbols::{
    alternate_symbols_cell, check_columns, repair_short_component_row,
};

/// Schema descriptor — held on the side for callers that need to reflect
/// over the column list (e.g. UI grid header rendering).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableSchema {
    pub columns: &'static [&'static str],
}

impl TableSchema {
    /// The canonical row schema. Adapters use this so renderers can lay out
    /// columns without hard-coding the strings inline.
    pub const ROW: TableSchema = TableSchema {
        columns: TABLE_HEADER,
    };
}

// ── Public API ────────────────────────────────────────────────────────────

/// Read every row from `path`. Empty file (header-only) yields an empty `Vec`.
///
/// Errors: `Io` on I/O failure, `Backend` on a malformed cell or schema
/// mismatch.
pub fn read_table(path: &Path) -> Result<Vec<ComponentRow>, LibraryError> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(LibraryError::Io(e)),
    };
    if bytes.is_empty() {
        return Ok(Vec::new());
    }

    let mut rdr = ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(true)
        .from_reader(&bytes[..]);

    // Verify header order. We don't accept reorderings — that would silently
    // miscolumn data. The error mode is `Backend(...)` so the caller surfaces
    // the schema mismatch explicitly.
    {
        let headers = rdr
            .headers()
            .map_err(|e| LibraryError::Backend(format!("read headers: {e}")))?;
        let columns: Vec<&str> = headers.iter().collect();
        check_columns(&columns)
            .map_err(|e| LibraryError::Backend(format!("table schema mismatch: {e}")))?;
    }

    let mut rows = Vec::new();
    for record in rdr.records() {
        let record = record.map_err(|e| LibraryError::Backend(format!("read row: {e}")))?;
        rows.push(record_to_row(&record)?);
    }
    Ok(rows)
}

/// Replace the contents of `path` with `rows`. Creates parent directories
/// if they don't exist.
///
/// Crash-safe: the bytes land via [`signex_types::atomic_io::atomic_write`]
/// (temp file + fsync + rename), so a crash or power loss mid-save leaves
/// either the previous table or the new one — never a truncated file. Every
/// mutator here ([`append_row`], [`delete_row`], [`update_row`]) rewrites the
/// whole table through this one funnel, so they inherit the guarantee.
///
/// Refuses, before touching the file, any row [`validate_row_for_write`]
/// rejects. The file is left as is.
pub fn write_table(path: &Path, rows: &[ComponentRow]) -> Result<(), LibraryError> {
    for row in rows {
        validate_row_for_write(row)?;
    }
    write_rows(path, rows)
}

/// Serialise `rows` without validating them: the mutators below validate
/// only the row they write, so one bad stored row cannot block unrelated
/// appends, updates or deletes.
fn write_rows(path: &Path, rows: &[ComponentRow]) -> Result<(), LibraryError> {
    let mut wtr = WriterBuilder::new()
        .delimiter(b'\t')
        .quote_style(QuoteStyle::Necessary)
        .from_writer(Vec::<u8>::new());
    let with_alternates = rows.iter().any(|r| !r.alternate_symbols.is_empty());
    let mut header: Vec<&str> = TABLE_HEADER.to_vec();
    if with_alternates {
        header.push(ALTERNATE_SYMBOLS_COLUMN);
    }
    wtr.write_record(&header)
        .map_err(|e| LibraryError::Backend(format!("write header: {e}")))?;
    for row in rows {
        let mut record = row_to_record(row)?;
        if with_alternates {
            record.push(alternate_symbols_cell(row)?.unwrap_or_default());
        }
        wtr.write_record(record)
            .map_err(|e| LibraryError::Backend(format!("write row: {e}")))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| LibraryError::Backend(format!("flush table: {e}")))?;
    signex_types::atomic_io::atomic_write(path, &bytes)?;
    Ok(())
}

/// Append a single row to the table file.
///
/// **Cost:** one pass over the file's bytes plus the new record (no per-row
/// parse); the result is still written atomically. For bulk loads (>10 rows),
/// use `write_table` directly.
pub fn append_row(path: &Path, row: &ComponentRow) -> Result<(), LibraryError> {
    validate_row_for_write(row)?;
    if splice::append(path, row)?.is_some() {
        return Ok(());
    }
    let mut rows = read_table(path)?;
    rows.push(row.clone());
    write_rows(path, &rows)
}

/// Remove the row whose `row_id` matches. Returns `NotFound` if no such row
/// exists in the table.
pub fn delete_row(path: &Path, row_id: RowId) -> Result<(), LibraryError> {
    if splice::delete(path, row_id.as_uuid())?.is_some() {
        return Ok(());
    }
    let mut rows = read_table(path)?;
    let target = row_id.as_uuid();
    let before = rows.len();
    rows.retain(|r| r.row_id != target);
    if rows.len() == before {
        return Err(LibraryError::NotFound(format!(
            "row {row_id} not in table {}",
            path.display()
        )));
    }
    write_rows(path, &rows)
}

/// Replace the row whose `row_id` matches. Returns `NotFound` if no such
/// row exists in the table.
pub fn update_row(path: &Path, row: &ComponentRow) -> Result<(), LibraryError> {
    validate_row_for_write(row)?;
    if splice::update(path, row)?.is_some() {
        return Ok(());
    }
    let mut rows = read_table(path)?;
    let target = row.row_id;
    let mut updated = false;
    for r in rows.iter_mut() {
        if r.row_id == target {
            *r = row.clone();
            updated = true;
            break;
        }
    }
    if !updated {
        return Err(LibraryError::NotFound(format!(
            "row {target} not in table {}",
            path.display()
        )));
    }
    write_rows(path, &rows)
}

// ── (de)serialisation helpers ─────────────────────────────────────────────

fn json_cell<T: Serialize>(value: &T) -> Result<String, LibraryError> {
    serde_json::to_string(value).map_err(|e| LibraryError::Backend(format!("serialise cell: {e}")))
}

fn from_json_cell<T: DeserializeOwned>(s: &str, name: &str) -> Result<T, LibraryError> {
    serde_json::from_str(s).map_err(|e| LibraryError::Backend(format!("parse {name}: {e}")))
}

fn datasheet_to_cell(d: &DatasheetRef) -> Result<String, LibraryError> {
    json_cell(d)
}

fn datasheet_from_cell(s: &str) -> Result<DatasheetRef, LibraryError> {
    if s.is_empty() {
        return Ok(DatasheetRef::default());
    }
    from_json_cell(s, "datasheet")
}

fn lifecycle_to_cell(s: LifecycleState) -> &'static str {
    match s {
        LifecycleState::Draft => "Draft",
        LifecycleState::InReview => "InReview",
        LifecycleState::Released => "Released",
        LifecycleState::Deprecated => "Deprecated",
        LifecycleState::Obsolete => "Obsolete",
    }
}

fn lifecycle_from_cell(s: &str) -> Result<LifecycleState, LibraryError> {
    Ok(match s {
        "Draft" => LifecycleState::Draft,
        "InReview" => LifecycleState::InReview,
        "Released" => LifecycleState::Released,
        "Deprecated" => LifecycleState::Deprecated,
        "Obsolete" => LifecycleState::Obsolete,
        other => {
            return Err(LibraryError::Backend(format!(
                "unknown lifecycle state {other:?}"
            )));
        }
    })
}

fn opt_primitive_to_cell(p: &Option<PrimitiveRef>) -> Result<String, LibraryError> {
    match p {
        None => Ok(String::new()),
        Some(r) => json_cell(r),
    }
}

fn opt_primitive_from_cell(s: &str) -> Result<Option<PrimitiveRef>, LibraryError> {
    if s.is_empty() {
        Ok(None)
    } else {
        Ok(Some(from_json_cell(s, "primitive_ref")?))
    }
}

fn timestamp_to_cell(t: DateTime<Utc>) -> String {
    t.to_rfc3339()
}

fn timestamp_from_cell(s: &str) -> Result<DateTime<Utc>, LibraryError> {
    DateTime::parse_from_rfc3339(s)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|e| LibraryError::Backend(format!("parse timestamp: {e}")))
}

fn hash_to_cell(h: &[u8; 32]) -> String {
    let mut s = String::with_capacity(64);
    for b in h {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn hash_from_cell(s: &str) -> Result<[u8; 32], LibraryError> {
    if s.is_empty() {
        return Ok([0u8; 32]);
    }
    // Validate on bytes before slicing: `s.len()` is a BYTE count, so a
    // hand-edited cell of 61×'a' + '€' is 64 bytes and clears a naive
    // length guard, then `&s[60..62]` splits the 3-byte '€' and panics.
    // Requiring every byte to be ASCII hex makes each `i*2` offset a
    // provable char boundary.
    let bytes = s.as_bytes();
    if bytes.len() != 64 {
        return Err(LibraryError::Backend(format!(
            "content_hash must be 64 hex chars, got {}",
            bytes.len()
        )));
    }
    if !bytes.iter().all(u8::is_ascii_hexdigit) {
        return Err(LibraryError::Backend(
            "content_hash must be 64 hex chars".to_string(),
        ));
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        let hex = &s[i * 2..i * 2 + 2];
        *byte = u8::from_str_radix(hex, 16)
            .map_err(|e| LibraryError::Backend(format!("hash hex: {e}")))?;
    }
    Ok(out)
}

/// The 21 legacy cells; the optional alternate-symbols cell is separate.
pub(crate) fn row_to_record(row: &ComponentRow) -> Result<Vec<String>, LibraryError> {
    let primary_mpn = json_cell(&row.primary_mpn)?;
    let alternates = json_cell(&row.alternates)?;
    let supply = json_cell::<Vec<DistributorListing>>(&row.supply)?;
    let parameters = json_cell::<ParamMap>(&row.parameters)?;
    let pin_map = json_cell::<Vec<PinPadOverride>>(&row.pin_map_overrides)?;
    let plm_unused: PlmReserved = row.plm.clone();
    // PlmReserved travels in the parameters cell? No — it has its own JSON
    // payload alongside content_hash. We hide it inside parameters cell?
    // No again — keep it out of the wire format entirely until v3.0 by
    // dropping into a dedicated JSON column would break the documented
    // schema length. Embed plm into the parameters cell? No.
    //
    // Per the plan §2.4, the documented columns are 16; we keep that. PLM
    // therefore travels inside the `parameters` blob? No — it is a separate
    // optional field. Since the plan §2.4 schema lists 16 specific columns
    // and `plm` isn't one of them, we round-trip `plm` through the
    // `parameters` JSON map only when the user has set it. For now we drop
    // it on serialise: PlmReserved::default() → empty payload, and v3.0
    // adds an explicit column. The unit tests verify default() round-trips.
    // **PLM persist gap.** As of v0.9, only `PlmReserved::default()` round-trips
    // through TSV. Non-default payloads (set by callers that have already
    // started populating `plm_part_id` / `eco_refs` / `compliance` ahead of the
    // v3.0 PLM adapter) reject here so the failure is loud rather than a
    // silent drop. See the `PlmReserved` doc comment in `component.rs` — it
    // mirrors this contract on the type definition. v3.0 will add dedicated
    // columns and remove this guard.
    if plm_unused != PlmReserved::default() {
        return Err(LibraryError::Backend(
            "PlmReserved fields cannot round-trip through TSV until v3.0 ships \
             the dedicated column"
                .into(),
        ));
    }

    Ok(vec![
        row.row_id.to_string(),
        row.internal_pn.as_str().to_string(),
        row.class.as_str().to_string(),
        datasheet_to_cell(&row.datasheet)?,
        lifecycle_to_cell(row.state).to_string(),
        json_cell(&row.symbol_ref)?,
        opt_primitive_to_cell(&row.footprint_ref)?,
        opt_primitive_to_cell(&row.sim_ref)?,
        primary_mpn,
        alternates,
        supply,
        parameters,
        pin_map,
        timestamp_to_cell(row.created),
        timestamp_to_cell(row.updated),
        hash_to_cell(&row.content_hash),
        // Stage 14 versioning + released — empty strings + "false"
        // are the back-compat defaults for rows that haven't been
        // touched since the bump-on-save convention landed.
        row.version.clone(),
        if row.released {
            "true".into()
        } else {
            "false".into()
        },
        row.symbol_version.clone(),
        row.footprint_version.clone(),
        row.sim_version.clone(),
    ])
}

pub(crate) fn record_to_row(record: &csv::StringRecord) -> Result<ComponentRow, LibraryError> {
    if record.len() != TABLE_HEADER.len() && record.len() != TABLE_HEADER.len() + 1 {
        return Err(LibraryError::Backend(format!(
            "row has {} cells, expected {} or {}",
            record.len(),
            TABLE_HEADER.len(),
            TABLE_HEADER.len() + 1
        )));
    }

    let cell = |i: usize| record.get(i).unwrap_or_default();

    let row_id = uuid::Uuid::parse_str(cell(0))
        .map_err(|e| LibraryError::Backend(format!("row_id: {e}")))?;
    let internal_pn = InternalPn::new(cell(1));
    let class = ComponentClass::new(cell(2));
    let datasheet = datasheet_from_cell(cell(3))?;
    let state = lifecycle_from_cell(cell(4))?;
    let symbol_ref: PrimitiveRef = from_json_cell(cell(5), "symbol_ref")?;
    let footprint_ref = opt_primitive_from_cell(cell(6))?;
    let sim_ref = opt_primitive_from_cell(cell(7))?;
    let primary_mpn: ManufacturerPart = from_json_cell(cell(8), "primary_mpn")?;
    let alternates: Vec<ManufacturerPart> = from_json_cell(cell(9), "alternates")?;
    let supply: Vec<DistributorListing> = from_json_cell(cell(10), "supply")?;
    let parameters: ParamMap = from_json_cell(cell(11), "parameters")?;
    let pin_map_overrides: Vec<PinPadOverride> = from_json_cell(cell(12), "pin_map_overrides")?;
    let created = timestamp_from_cell(cell(13))?;
    let updated = timestamp_from_cell(cell(14))?;
    let content_hash = hash_from_cell(cell(15))?;
    // Stage 14 versioning fields — appended after content_hash so the
    // legacy 16-column fixtures pre-versioning could in theory still
    // parse through a tolerant header check (we don't take that route,
    // but the wire format remains additive).
    let version = if cell(16).is_empty() {
        "0.0.1".into()
    } else {
        cell(16).to_string()
    };
    let released = match cell(17) {
        "" | "false" => false,
        "true" => true,
        other => {
            return Err(LibraryError::Backend(format!(
                "released cell expects 'true'/'false'/empty, got {other:?}"
            )));
        }
    };
    let symbol_version = cell(18).to_string();
    let footprint_version = cell(19).to_string();
    let sim_version = cell(20).to_string();
    let alternate_symbols = alternate_symbols_from_cell(cell(21))?;

    Ok(ComponentRow {
        row_id,
        internal_pn,
        class,
        datasheet,
        state,
        symbol_ref,
        alternate_symbols,
        footprint_ref,
        sim_ref,
        pin_map_overrides,
        primary_mpn,
        alternates,
        supply,
        parameters,
        plm: PlmReserved::default(),
        version,
        released,
        symbol_version,
        footprint_version,
        sim_version,
        created,
        updated,
        content_hash,
        server_revision: None,
    })
}

#[cfg(test)]
mod tests;
