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
//! This module is the *file format*; commit / branch handling lives in the
//! LocalGit adapter (WS-2). The unit tests here only exercise the
//! TSV serialisation contract.

use std::path::Path;

use chrono::{DateTime, Utc};
use csv::{QuoteStyle, ReaderBuilder, WriterBuilder};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::adapter::LibraryError;
use crate::component::{ComponentRow, DatasheetRef, PinPadOverride, PlmReserved};
use crate::identity::{ComponentClass, InternalPn, RowId};
use crate::lifecycle::LifecycleState;
use crate::manufacturer::{DistributorListing, ManufacturerPart};
use crate::param::ParamMap;
use crate::primitive::PrimitiveRef;

/// Header row — the canonical column ordering for every adapter.
///
/// Renaming or reordering this is a wire-format break: existing TSV files
/// would parse into the wrong fields. Add new columns at the end with a
/// `#[serde(default)]` on the row struct.
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
];

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
        if headers.len() != TABLE_HEADER.len() {
            return Err(LibraryError::Backend(format!(
                "table schema mismatch: {} columns, expected {}",
                headers.len(),
                TABLE_HEADER.len()
            )));
        }
        for (got, want) in headers.iter().zip(TABLE_HEADER.iter()) {
            if got != *want {
                return Err(LibraryError::Backend(format!(
                    "table schema mismatch: column {got:?}, expected {want:?}"
                )));
            }
        }
    }

    let mut rows = Vec::new();
    for record in rdr.records() {
        let record =
            record.map_err(|e| LibraryError::Backend(format!("read row: {e}")))?;
        rows.push(record_to_row(&record)?);
    }
    Ok(rows)
}

/// Replace the contents of `path` with `rows`. Creates parent directories
/// if they don't exist.
pub fn write_table(path: &Path, rows: &[ComponentRow]) -> Result<(), LibraryError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut wtr = WriterBuilder::new()
        .delimiter(b'\t')
        .quote_style(QuoteStyle::Necessary)
        .from_writer(Vec::<u8>::new());
    wtr.write_record(TABLE_HEADER)
        .map_err(|e| LibraryError::Backend(format!("write header: {e}")))?;
    for row in rows {
        wtr.write_record(row_to_record(row)?)
            .map_err(|e| LibraryError::Backend(format!("write row: {e}")))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| LibraryError::Backend(format!("flush table: {e}")))?;
    std::fs::write(path, bytes)?;
    Ok(())
}

/// Append one row to the table. Header is written if `path` doesn't exist.
pub fn append_row(path: &Path, row: &ComponentRow) -> Result<(), LibraryError> {
    let mut rows = read_table(path)?;
    rows.push(row.clone());
    write_table(path, &rows)
}

/// Remove the row whose `row_id` matches. Returns `NotFound` if no such row
/// exists in the table.
pub fn delete_row(path: &Path, row_id: RowId) -> Result<(), LibraryError> {
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
    write_table(path, &rows)
}

/// Replace the row whose `row_id` matches. Returns `NotFound` if no such
/// row exists in the table.
pub fn update_row(path: &Path, row: &ComponentRow) -> Result<(), LibraryError> {
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
    write_table(path, &rows)
}

// ── (de)serialisation helpers ─────────────────────────────────────────────

fn json_cell<T: Serialize>(value: &T) -> Result<String, LibraryError> {
    serde_json::to_string(value)
        .map_err(|e| LibraryError::Backend(format!("serialise cell: {e}")))
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
    if s.len() != 64 {
        return Err(LibraryError::Backend(format!(
            "content_hash must be 64 hex chars, got {}",
            s.len()
        )));
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        let hex = &s[i * 2..i * 2 + 2];
        *byte = u8::from_str_radix(hex, 16)
            .map_err(|e| LibraryError::Backend(format!("hash hex: {e}")))?;
    }
    Ok(out)
}

fn row_to_record(row: &ComponentRow) -> Result<Vec<String>, LibraryError> {
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
    ])
}

fn record_to_row(record: &csv::StringRecord) -> Result<ComponentRow, LibraryError> {
    if record.len() != TABLE_HEADER.len() {
        return Err(LibraryError::Backend(format!(
            "row has {} cells, expected {}",
            record.len(),
            TABLE_HEADER.len()
        )));
    }

    let cell = |i: usize| record.get(i).unwrap_or_default();

    let row_id =
        uuid::Uuid::parse_str(cell(0)).map_err(|e| LibraryError::Backend(format!("row_id: {e}")))?;
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

    Ok(ComponentRow {
        row_id,
        internal_pn,
        class,
        datasheet,
        state,
        symbol_ref,
        footprint_ref,
        sim_ref,
        pin_map_overrides,
        primary_mpn,
        alternates,
        supply,
        parameters,
        plm: PlmReserved::default(),
        created,
        updated,
        content_hash,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manufacturer::ManufacturerPart;
    use chrono::{Duration, TimeZone};
    use uuid::Uuid;

    fn mk_row(internal_pn: &str, class: &str) -> ComponentRow {
        let lib = Uuid::nil();
        // Anchor timestamps so equality checks aren't flaky under
        // sub-millisecond drift between `Utc::now()` calls.
        let created = Utc.with_ymd_and_hms(2026, 4, 1, 12, 0, 0).unwrap();
        let updated = created + Duration::hours(1);
        ComponentRow {
            row_id: Uuid::new_v4(),
            internal_pn: InternalPn::new(internal_pn),
            class: ComponentClass::new(class),
            datasheet: DatasheetRef::url("https://example.com/ds.pdf"),
            state: LifecycleState::Released,
            symbol_ref: PrimitiveRef::new(lib, Uuid::new_v4()),
            footprint_ref: Some(PrimitiveRef::new(lib, Uuid::new_v4())),
            sim_ref: None,
            pin_map_overrides: vec![PinPadOverride::new("EP", "EP1")],
            primary_mpn: ManufacturerPart::draft("Acme", "ACM-001"),
            alternates: Vec::new(),
            supply: vec![DistributorListing::new("DigiKey", "DK-1")],
            parameters: ParamMap::new(),
            plm: PlmReserved::default(),
            created,
            updated,
            content_hash: [0u8; 32],
        }
    }

    /// Plan §6 step 1.3 — the foundational TSV round-trip.
    #[test]
    fn tsv_roundtrip_preserves_row() {
        let rows = vec![mk_row("R10K", "resistor"), mk_row("C100N", "capacitor")];
        let tmp = tempfile::NamedTempFile::new().unwrap();
        write_table(tmp.path(), &rows).unwrap();
        let back = read_table(tmp.path()).unwrap();
        assert_eq!(rows, back);
    }

    /// Empty file (no header) reads back as empty vec.
    #[test]
    fn read_missing_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does-not-exist.tsv");
        let rows = read_table(&path).unwrap();
        assert!(rows.is_empty());
    }

    /// `append_row` honours an empty file, then keeps growing it.
    #[test]
    fn append_row_grows_the_file() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let a = mk_row("R1", "resistor");
        let b = mk_row("R2", "resistor");
        append_row(tmp.path(), &a).unwrap();
        append_row(tmp.path(), &b).unwrap();
        let back = read_table(tmp.path()).unwrap();
        assert_eq!(back, vec![a, b]);
    }

    /// `delete_row` removes the matching id, leaves others alone.
    #[test]
    fn delete_row_removes_only_matching_id() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let a = mk_row("R1", "resistor");
        let b = mk_row("R2", "resistor");
        write_table(tmp.path(), &[a.clone(), b.clone()]).unwrap();
        delete_row(tmp.path(), RowId::from_uuid(a.row_id)).unwrap();
        let back = read_table(tmp.path()).unwrap();
        assert_eq!(back, vec![b]);
    }

    #[test]
    fn delete_row_missing_returns_not_found() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let a = mk_row("R1", "resistor");
        write_table(tmp.path(), &[a]).unwrap();
        let err = delete_row(tmp.path(), RowId::new()).unwrap_err();
        assert!(matches!(err, LibraryError::NotFound(_)));
    }

    /// `update_row` replaces a row in-place by id.
    #[test]
    fn update_row_replaces_in_place() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut a = mk_row("R1", "resistor");
        let b = mk_row("R2", "resistor");
        write_table(tmp.path(), &[a.clone(), b.clone()]).unwrap();
        a.internal_pn = InternalPn::new("R1_RENAMED");
        update_row(tmp.path(), &a).unwrap();
        let back = read_table(tmp.path()).unwrap();
        assert_eq!(back, vec![a, b]);
    }

    #[test]
    fn update_row_missing_returns_not_found() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let r = mk_row("R1", "resistor");
        let err = update_row(tmp.path(), &r).unwrap_err();
        assert!(matches!(err, LibraryError::NotFound(_)));
    }

    /// `none` footprint and sim refs encode as empty strings; round-trip
    /// preserves the `None` shape.
    #[test]
    fn none_primitive_refs_encode_empty() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut r = mk_row("RX", "resistor");
        r.footprint_ref = None;
        r.sim_ref = None;
        write_table(tmp.path(), &[r.clone()]).unwrap();
        let back = read_table(tmp.path()).unwrap();
        assert_eq!(back, vec![r]);
    }

    /// Hash hex encode/decode is bit-exact.
    #[test]
    fn hash_round_trip_preserves_bytes() {
        let mut h = [0u8; 32];
        for (i, b) in h.iter_mut().enumerate() {
            *b = i as u8;
        }
        let s = hash_to_cell(&h);
        let back = hash_from_cell(&s).unwrap();
        assert_eq!(h, back);
    }

    #[test]
    fn schema_constant_matches_header_length() {
        assert_eq!(TABLE_HEADER.len(), TableSchema::ROW.columns.len());
    }
}
