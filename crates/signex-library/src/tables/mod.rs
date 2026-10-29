//! TSV reader/writer for component tables (Altium DBLib model).
//!
//! Per `v0.9-refactor-2-plan.md` §2.4, every adapter (LocalGit, Database,
//! future flavours) shares one column schema:
//!
//! ```text
//! row_id  internal_pn  class  datasheet  state  symbol_ref  footprint_ref
//! sim_ref  primary_mpn  alternates  supply  parameters  pin_map_overrides
//! created  updated  content_hash  version  released  symbol_version
//! footprint_version  sim_version  description
//! ```
//!
//! Scalar columns (`row_id`, `internal_pn`, `class`, `state`, `created`,
//! `updated`, `content_hash`) are written as plain strings. Nested columns
//! (`PrimitiveRef`, `ManufacturerPart`, `Vec<…>`, `ParamMap`, `PlmReserved`)
//! are JSON-encoded inside the cell.
//!
//! `Option<PrimitiveRef>` cells: empty string = `None`, JSON = `Some`.
//!
//! The `datasheet` cell is one `DatasheetRef` object for a row with a
//! single unlabelled document, and a JSON array of `DatasheetDoc`
//! (primary first) once a document has a label or there is more than one
//! (#1231). Readers accept both.
//!
//! This module is the *cell encoding*: [`row_to_record`] and
//! [`record_to_row`] convert a [`ComponentRow`] to and from one TSV record.
//! Rows live inside the `.snxlib` file; commit / branch handling lives in
//! [`crate::adapters::local_git::LocalGitAdapter`]. The unit tests here
//! only exercise the record encoding contract.

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::adapter::LibraryError;
use crate::component::{ComponentRow, DatasheetDoc, DatasheetRef, PinPadOverride, PlmReserved};
use crate::identity::{ComponentClass, InternalPn};
use crate::lifecycle::LifecycleState;
use crate::manufacturer::{DistributorListing, ManufacturerPart};
use crate::param::ParamMap;
use crate::primitive::PrimitiveRef;

/// Header row — the canonical column ordering for every adapter.
///
/// Renaming or reordering this is a wire-format break: existing TSV files
/// would parse into the wrong fields. New columns go at the end and are
/// required: signex keeps no compatibility with tables written before a
/// column existed.
///
/// An optional 23rd column, [`ALTERNATE_SYMBOLS_COLUMN`], follows these 22.
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
    "description",
];

mod alternate_symbols;
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

// ── (de)serialisation helpers ─────────────────────────────────────────────

fn json_cell<T: Serialize>(value: &T) -> Result<String, LibraryError> {
    serde_json::to_string(value).map_err(|e| LibraryError::Backend(format!("serialise cell: {e}")))
}

fn from_json_cell<T: DeserializeOwned>(s: &str, name: &str) -> Result<T, LibraryError> {
    serde_json::from_str(s).map_err(|e| LibraryError::InvalidData(format!("parse {name}: {e}")))
}

/// The `datasheet` cell holds the single [`DatasheetRef`] object a row has
/// always written, so rows with one unlabelled document stay byte-identical.
/// A label or a further document switches it to a JSON array of
/// [`DatasheetDoc`], primary first (#1231).
fn datasheet_to_cell(row: &ComponentRow) -> Result<String, LibraryError> {
    if row.datasheet_label.is_empty() && row.more_datasheets.is_empty() {
        json_cell(&row.datasheet)
    } else {
        json_cell(&row.documents())
    }
}

fn datasheet_from_cell(s: &str) -> Result<Vec<DatasheetDoc>, LibraryError> {
    if s.is_empty() {
        return Ok(Vec::new());
    }
    if s.trim_start().starts_with('[') {
        return from_json_cell(s, "datasheet");
    }
    let primary: DatasheetRef = from_json_cell(s, "datasheet")?;
    Ok(vec![DatasheetDoc::new("", primary)])
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
            return Err(LibraryError::InvalidData(format!(
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
        .map_err(|e| LibraryError::InvalidData(format!("parse timestamp: {e}")))
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
        return Err(LibraryError::InvalidData(format!(
            "content_hash must be 64 hex chars, got {}",
            bytes.len()
        )));
    }
    if !bytes.iter().all(u8::is_ascii_hexdigit) {
        return Err(LibraryError::InvalidData(
            "content_hash must be 64 hex chars".to_string(),
        ));
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        let hex = &s[i * 2..i * 2 + 2];
        *byte = u8::from_str_radix(hex, 16)
            .map_err(|e| LibraryError::InvalidData(format!("hash hex: {e}")))?;
    }
    Ok(out)
}

/// The 22 schema cells; the optional alternate-symbols cell is separate.
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
        return Err(LibraryError::InvalidData(
            "PlmReserved fields cannot round-trip through TSV until v3.0 ships \
             the dedicated column"
                .into(),
        ));
    }

    Ok(vec![
        row.row_id.to_string(),
        row.internal_pn.as_str().to_string(),
        row.class.as_str().to_string(),
        datasheet_to_cell(row)?,
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
        row.description.clone(),
    ])
}

pub(crate) fn record_to_row(record: &csv::StringRecord) -> Result<ComponentRow, LibraryError> {
    if record.len() != TABLE_HEADER.len() && record.len() != TABLE_HEADER.len() + 1 {
        return Err(LibraryError::InvalidData(format!(
            "row has {} cells, expected {} or {}",
            record.len(),
            TABLE_HEADER.len(),
            TABLE_HEADER.len() + 1
        )));
    }

    let cell = |i: usize| record.get(i).unwrap_or_default();

    let row_id = uuid::Uuid::parse_str(cell(0))
        .map_err(|e| LibraryError::InvalidData(format!("row_id: {e}")))?;
    let internal_pn = InternalPn::new(cell(1));
    let class = ComponentClass::new(cell(2));
    let documents = datasheet_from_cell(cell(3))?;
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
            return Err(LibraryError::InvalidData(format!(
                "released cell expects 'true'/'false'/empty, got {other:?}"
            )));
        }
    };
    let symbol_version = cell(18).to_string();
    let footprint_version = cell(19).to_string();
    let sim_version = cell(20).to_string();
    let description = cell(21).to_string();
    let alternate_symbols = alternate_symbols_from_cell(cell(22))?;

    let mut row = ComponentRow {
        row_id,
        internal_pn,
        class,
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        description,
        more_datasheets: Vec::new(),
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
    };
    row.set_documents(documents);
    Ok(row)
}

#[cfg(test)]
mod tests;
