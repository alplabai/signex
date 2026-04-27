//! `ComponentRow` — one row of a component table (Altium DBLib model).
//!
//! Per `v0.9-refactor-2-plan.md` §2.1, a "component" is no longer a file
//! holding a chain of revisions. It's a single row inside a category table
//! (`tables/<name>.tsv` for LocalGit; one record in `component_rows` for the
//! database backend). Symbols, footprints, and sim models stay as standalone
//! editable primitive files referenced by `(library_id, uuid)` tuples.
//!
//! Two MPNs sharing a SOIC-8 footprint reference the same primitive UUID
//! rather than carrying their own copy.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::identity::{ComponentClass, InternalPn};
use crate::lifecycle::LifecycleState;
use crate::manufacturer::{DistributorListing, ManufacturerPart};
use crate::param::ParamMap;
use crate::primitive::PrimitiveRef;

/// Reference to a datasheet — either a remote URL or a hash-pinned local PDF.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DatasheetRef {
    Url { url: String },
    HashPinned { hash: String, filename: String },
}

impl DatasheetRef {
    pub fn url(s: impl Into<String>) -> Self {
        Self::Url { url: s.into() }
    }

    pub fn hash_pinned(hash: impl Into<String>, filename: impl Into<String>) -> Self {
        Self::HashPinned {
            hash: hash.into(),
            filename: filename.into(),
        }
    }
}

impl Default for DatasheetRef {
    fn default() -> Self {
        Self::Url { url: String::new() }
    }
}

/// Pin-to-pad override — empty list = default 1:1 binding by number string
/// equality. Non-empty entries override specific pins.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinPadOverride {
    pub symbol_pin_number: String,
    pub footprint_pad_number: String,
}

impl PinPadOverride {
    pub fn new(pin: impl Into<String>, pad: impl Into<String>) -> Self {
        Self {
            symbol_pin_number: pin.into(),
            footprint_pad_number: pad.into(),
        }
    }
}

/// PLM-reserved fields. Inert until v3.0.
///
/// **TSV persistence note:** as of v0.9, `PlmReserved` is dropped at write
/// time — only `PlmReserved::default()` round-trips through `tables::write_table`.
/// Non-default payloads cause `LibraryError::Backend("...")`. v3.0 will add
/// dedicated columns and full round-trip.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlmReserved {
    #[serde(default)]
    pub plm_part_id: Option<String>,
    #[serde(default)]
    pub eco_refs: Vec<String>,
    #[serde(default)]
    pub compliance: BTreeMap<String, String>,
}

/// One row inside a component table — Altium DBLib model.
///
/// Per `v0.9-refactor-2-plan.md` §2.1, a row carries the binding metadata
/// for a manufacturer part: which primitives (symbol/footprint/sim) it
/// points at, the parametric data, the supply chain, and the lifecycle
/// state. The schema is identical across LocalGit (TSV columns) and
/// Database (JSONB payload) backends — one wire format, two storage
/// flavours.
///
/// Past versions of a row are read from `git log` (LocalGit) or the
/// audit trail (Database); there is no per-row revision chain anymore.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComponentRow {
    /// Stable UUID — primary key inside the table. Use `RowId::new()` to
    /// mint a new time-ordered id when inserting.
    pub row_id: Uuid,
    /// Library-internal part number — unique within the library.
    pub internal_pn: InternalPn,
    /// Component class — picks the parameter template ("resistor", "opamp", …).
    pub class: ComponentClass,
    /// URL or hash-pinned PDF. Default is an empty URL.
    #[serde(default)]
    pub datasheet: DatasheetRef,
    /// Lifecycle state (`Released`, `Draft`, …).
    pub state: LifecycleState,

    // ── Primitive bindings ──────────────────────────────────────────────
    pub symbol_ref: PrimitiveRef,
    #[serde(default)]
    pub footprint_ref: Option<PrimitiveRef>,
    #[serde(default)]
    pub sim_ref: Option<PrimitiveRef>,

    // ── Pin map ─────────────────────────────────────────────────────────
    /// Empty list = default 1:1 binding by pin/pad number equality.
    #[serde(default)]
    pub pin_map_overrides: Vec<PinPadOverride>,

    // ── Manufacturer / supply ───────────────────────────────────────────
    pub primary_mpn: ManufacturerPart,
    #[serde(default)]
    pub alternates: Vec<ManufacturerPart>,
    #[serde(default)]
    pub supply: Vec<DistributorListing>,

    // ── Parametric ──────────────────────────────────────────────────────
    /// Schema-validated against the class template via
    /// [`crate::TemplateRegistry::validate_params`].
    #[serde(default)]
    pub parameters: ParamMap,

    // ── PLM-reserved (inert until v3.0) ─────────────────────────────────
    #[serde(default)]
    pub plm: PlmReserved,

    /// Bookkeeping — set on insert.
    pub created: DateTime<Utc>,
    /// Bookkeeping — bumped on every update.
    pub updated: DateTime<Utc>,

    /// SHA-256 of canonicalised JSON over the binding fields. See
    /// [`crate::hash::hash_row_content`].
    #[serde(default)]
    pub content_hash: [u8; 32],
}

impl ComponentRow {
    /// Recompute and store the content hash from the canonical view.
    ///
    /// Returns `LibraryError::Backend` when the row contains a non-finite
    /// float (`NaN` / `±Infinity`) anywhere reached by the canonical view —
    /// `serde_json` can't encode those, and panicking on save would be an
    /// availability bug. See [`crate::hash::hash_row_content`].
    pub fn refresh_content_hash(&mut self) -> Result<(), crate::adapter::LibraryError> {
        self.content_hash = crate::hash::hash_row_content(self)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::RowId;
    use crate::manufacturer::ManufacturerPart;

    fn fixture_row() -> ComponentRow {
        let lib = Uuid::new_v4();
        ComponentRow {
            row_id: Uuid::new_v4(),
            internal_pn: InternalPn::new("R0805_10k"),
            class: ComponentClass::new("resistor"),
            datasheet: DatasheetRef::url("https://example.com"),
            state: LifecycleState::Draft,
            symbol_ref: PrimitiveRef::new(lib, Uuid::new_v4()),
            footprint_ref: Some(PrimitiveRef::new(lib, Uuid::new_v4())),
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft("Vishay", "CRCW08051002F"),
            alternates: Vec::new(),
            supply: Vec::new(),
            parameters: ParamMap::new(),
            plm: PlmReserved::default(),
            created: Utc::now(),
            updated: Utc::now(),
            content_hash: [0u8; 32],
        }
    }

    /// `ComponentRow` round-trips through JSON without losing any
    /// fields. Foundational test for the rest of the row-tier work.
    #[test]
    fn component_row_json_roundtrip() {
        let row = ComponentRow {
            row_id: Uuid::new_v4(),
            internal_pn: "R0805_10k".parse().unwrap(),
            class: ComponentClass("resistor".into()),
            datasheet: DatasheetRef::url("https://example.com"),
            state: LifecycleState::Draft,
            symbol_ref: PrimitiveRef::new(Uuid::new_v4(), Uuid::new_v4()),
            footprint_ref: Some(PrimitiveRef::new(Uuid::new_v4(), Uuid::new_v4())),
            sim_ref: None,
            pin_map_overrides: vec![],
            primary_mpn: ManufacturerPart::draft("Vishay", "CRCW08051002F"),
            alternates: vec![],
            supply: vec![],
            parameters: ParamMap::new(),
            plm: PlmReserved::default(),
            created: Utc::now(),
            updated: Utc::now(),
            content_hash: [0u8; 32],
        };
        let json = serde_json::to_string(&row).unwrap();
        let back: ComponentRow = serde_json::from_str(&json).unwrap();
        assert_eq!(row, back);
    }

    #[test]
    fn refresh_content_hash_populates() {
        let mut row = fixture_row();
        assert_eq!(row.content_hash, [0u8; 32]);
        row.refresh_content_hash().unwrap();
        assert_ne!(row.content_hash, [0u8; 32]);
    }

    #[test]
    fn datasheet_ref_round_trip_each_variant() {
        let cases = [
            DatasheetRef::url("https://example.com/x.pdf"),
            DatasheetRef::hash_pinned("0123abcd", "datasheet.pdf"),
        ];
        for d in cases {
            let json = serde_json::to_string(&d).unwrap();
            let back: DatasheetRef = serde_json::from_str(&json).unwrap();
            assert_eq!(d, back);
        }
    }

    #[test]
    fn pin_pad_override_round_trip() {
        let o = PinPadOverride::new("EP", "EP1");
        let json = serde_json::to_string(&o).unwrap();
        let back: PinPadOverride = serde_json::from_str(&json).unwrap();
        assert_eq!(o, back);
    }

    #[test]
    fn plm_reserved_defaults_round_trip_clean() {
        let p = PlmReserved::default();
        let json = serde_json::to_string(&p).unwrap();
        let back: PlmReserved = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    /// Verifies that `RowId` wraps cleanly around the row's `row_id` field —
    /// the field is bare `Uuid` for serde-shape compatibility with the plan's
    /// schema test, but consumers can `RowId::from_uuid(row.row_id)` to
    /// get the typed wrapper when needed.
    #[test]
    fn row_id_wraps_bare_uuid_field() {
        let row = fixture_row();
        let id = RowId::from_uuid(row.row_id);
        assert_eq!(id.as_uuid(), row.row_id);
    }
}
