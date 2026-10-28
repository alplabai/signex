//! `ComponentRow` — one row of a component table (Altium DBLib model).
//!
//! Per `v0.9-refactor-2-plan.md` §2.1, a "component" is no longer a file
//! holding a chain of revisions. It's a single row inside a category table
//! (a `[tables.<name>]` block in the `.snxlib` for LocalGit; one record in `component_rows` for the
//! database backend). Symbols, footprints, and sim models stay as standalone
//! editable primitive files referenced by `(library_id, uuid)` tuples.
//!
//! Two MPNs sharing a SOIC-8 footprint reference the same primitive UUID
//! rather than carrying their own copy.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::alternate_symbol::AlternateSymbol;
use crate::identity::{ComponentClass, InternalPn};
use crate::lifecycle::LifecycleState;
use crate::manufacturer::{DistributorListing, ManufacturerPart};
use crate::param::ParamMap;
use crate::primitive::PrimitiveRef;

/// Row wire-schema level. Bump it whenever a `ComponentRow` field is added
/// that an older build would drop on save (1 = implicit, pre-guard;
/// 2 = `alternate_symbols`; 3 = the labelled documents `datasheet_label` /
/// `more_datasheets`, #1231; 4 = `description`, #1474).
pub const ROW_SCHEMA: u32 = 4;

/// `code` in the error body of a row write refused for a schema mismatch.
pub const ROW_SCHEMA_REFUSED_CODE: &str = "row_schema_too_old";

/// Request header carrying the sender's [`ROW_SCHEMA`] on a row write.
pub const ROW_SCHEMA_HEADER: &str = "x-signex-row-schema";

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

impl DatasheetRef {
    /// True for a reference that points nowhere: the empty URL a row starts
    /// with ("no datasheet"), or a pinned PDF not uploaded yet.
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Url { url } => url.trim().is_empty(),
            Self::HashPinned { hash, .. } => hash.is_empty(),
        }
    }
}

/// One document attached to a component row: a datasheet, an errata, an
/// application note. `label` says which; empty means "Datasheet".
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DatasheetDoc {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    #[serde(flatten)]
    pub source: DatasheetRef,
}

impl DatasheetDoc {
    pub fn new(label: impl Into<String>, source: DatasheetRef) -> Self {
        Self {
            label: label.into(),
            source,
        }
    }
}

impl Default for DatasheetRef {
    fn default() -> Self {
        Self::Url { url: String::new() }
    }
}

fn default_row_version() -> String {
    "0.0.1".to_string()
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
    /// The primary document: URL or hash-pinned PDF. Default is an empty
    /// URL.
    #[serde(default)]
    pub datasheet: DatasheetRef,
    /// Label of the primary document; empty means "Datasheet".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub datasheet_label: String,
    /// One-line human description of the part ("100 nF 50 V X7R 0603");
    /// shown and searched in the Place Component picker. Empty = none.
    pub description: String,
    /// Further documents after the primary, in display order (#1231).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub more_datasheets: Vec<DatasheetDoc>,
    /// Lifecycle state (`Released`, `Draft`, …).
    pub state: LifecycleState,

    // ── Primitive bindings ──────────────────────────────────────────────
    /// Default symbol drawing.
    pub symbol_ref: PrimitiveRef,
    /// Other drawings of the same part (IEC vs ANSI, ...); they share the
    /// footprint pin map, so they must expose the default's pin numbers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternate_symbols: Vec<AlternateSymbol>,
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

    // ── Stage 14: entity versioning + released flag ─────────────────────
    /// Semver-style revision string for this row. Defaults to
    /// `"0.0.1"` for new + legacy rows. In Personal workflow mode
    /// (Stage 13) every row save patch-bumps automatically; in Team
    /// mode released rows require an explicit bump via the dialog.
    #[serde(default = "default_row_version")]
    pub version: String,
    /// When `true`, the row is locked from edit-in-place under Team
    /// mode — saves open the bump dialog. Hidden in Personal mode.
    #[serde(default)]
    pub released: bool,
    /// Pinned symbol revision — which version of the bound `symbol_ref`
    /// this row was built against. The cascade engine (Stage 15)
    /// updates this when the user opts in to a primitive bump; the
    /// "Library Updates" dialog (Stage 16) surfaces drift on
    /// schematic open.
    #[serde(default)]
    pub symbol_version: String,
    /// Pinned footprint revision. Empty when `footprint_ref` is `None`.
    #[serde(default)]
    pub footprint_version: String,
    /// Pinned sim revision. Empty when `sim_ref` is `None`.
    #[serde(default)]
    pub sim_version: String,

    /// Bookkeeping — set on insert.
    pub created: DateTime<Utc>,
    /// Bookkeeping — bumped on every update.
    pub updated: DateTime<Utc>,

    /// SHA-256 of canonicalised JSON over the binding fields. See
    /// [`crate::hash::hash_row_content`].
    #[serde(default)]
    pub content_hash: [u8; 32],

    /// Base revision this copy was read at: a server counter for a
    /// database library, a content hash of the stored row for a local one
    /// (see `local_git::helpers::row_base`); `None` for a row being
    /// created. A write sends it as its base and is refused when the
    /// stored row has moved on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_revision: Option<u64>,
}

impl ComponentRow {
    /// True when `reply`, the row a server stored, lost documents this row
    /// carries: a server built before [`ROW_SCHEMA`] 3 drops them.
    pub fn documents_lost_in(&self, reply: &ComponentRow) -> bool {
        self.datasheet_label != reply.datasheet_label
            || self.more_datasheets != reply.more_datasheets
    }

    /// Every document on the row, the primary first.
    pub fn documents(&self) -> Vec<DatasheetDoc> {
        std::iter::once(DatasheetDoc::new(
            self.datasheet_label.clone(),
            self.datasheet.clone(),
        ))
        .chain(self.more_datasheets.iter().cloned())
        .collect()
    }

    /// Drop the further documents with neither a label nor a source, such
    /// as one added and never filled in. The primary always stays.
    pub fn drop_empty_documents(&mut self) {
        self.more_datasheets
            .retain(|doc| !(doc.label.trim().is_empty() && doc.source.is_empty()));
    }

    /// Replace every document; the first becomes the primary. An empty
    /// list leaves the row with no datasheet.
    pub fn set_documents(&mut self, docs: Vec<DatasheetDoc>) {
        let mut docs = docs.into_iter();
        let primary = docs.next().unwrap_or_default();
        self.datasheet = primary.source;
        self.datasheet_label = primary.label;
        self.more_datasheets = docs.collect();
    }

    /// True when the row has no usable symbol binding. A new row starts
    /// with the nil-UUID sentinel because creating a component never
    /// mints a primitive file; the user binds one with Pick Symbol.
    pub fn is_symbol_unbound(&self) -> bool {
        self.symbol_ref.uuid.is_nil()
    }

    /// True when the row has no usable footprint binding (`None` or the
    /// nil-UUID sentinel).
    pub fn is_footprint_unbound(&self) -> bool {
        self.footprint_ref
            .as_ref()
            .is_none_or(|fp| fp.uuid.is_nil())
    }

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
pub(crate) mod tests {
    use super::*;
    use crate::identity::RowId;
    use crate::manufacturer::ManufacturerPart;

    pub(crate) fn fixture_row() -> ComponentRow {
        let lib = Uuid::new_v4();
        ComponentRow {
            alternate_symbols: Vec::new(),
            row_id: Uuid::new_v4(),
            internal_pn: InternalPn::new("R0805_10k"),
            class: ComponentClass::new("resistor"),
            datasheet: DatasheetRef::url("https://example.com"),
            datasheet_label: String::new(),
            description: String::new(),
            more_datasheets: Vec::new(),
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
            version: "0.0.1".into(),
            released: false,
            symbol_version: String::new(),
            footprint_version: String::new(),
            sim_version: String::new(),
            created: Utc::now(),
            updated: Utc::now(),
            content_hash: [0u8; 32],
            server_revision: None,
        }
    }

    /// `ComponentRow` round-trips through JSON without losing any
    /// fields. Foundational test for the rest of the row-tier work.
    #[test]
    fn component_row_json_roundtrip() {
        let row = ComponentRow {
            alternate_symbols: Vec::new(),
            row_id: Uuid::new_v4(),
            internal_pn: "R0805_10k".parse().unwrap(),
            class: ComponentClass("resistor".into()),
            datasheet: DatasheetRef::url("https://example.com"),
            datasheet_label: String::new(),
            description: String::new(),
            more_datasheets: Vec::new(),
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
            version: "0.0.1".into(),
            released: false,
            symbol_version: String::new(),
            footprint_version: String::new(),
            sim_version: String::new(),
            created: Utc::now(),
            updated: Utc::now(),
            content_hash: [0u8; 32],
            server_revision: None,
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
    fn empty_further_documents_are_dropped_and_the_primary_kept() {
        let mut row = fixture_row();
        row.set_documents(vec![
            DatasheetDoc::default(),
            DatasheetDoc::default(),
            DatasheetDoc::new("", DatasheetRef::hash_pinned("", "")),
            DatasheetDoc::new("Errata", DatasheetRef::default()),
            DatasheetDoc::new("", DatasheetRef::url("https://x.example/a.pdf")),
        ]);
        row.drop_empty_documents();
        assert!(row.datasheet.is_empty(), "the primary stays, even empty");
        assert_eq!(
            row.more_datasheets,
            vec![
                DatasheetDoc::new("Errata", DatasheetRef::default()),
                DatasheetDoc::new("", DatasheetRef::url("https://x.example/a.pdf")),
            ]
        );
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

    #[test]
    fn nil_and_missing_refs_are_unbound_and_real_refs_are_bound() {
        let mut row = fixture_row();
        assert!(!row.is_symbol_unbound());
        assert!(!row.is_footprint_unbound());

        row.symbol_ref = PrimitiveRef::new(row.symbol_ref.library_id, Uuid::nil());
        row.footprint_ref = None;
        assert!(row.is_symbol_unbound());
        assert!(row.is_footprint_unbound());

        row.footprint_ref = Some(PrimitiveRef::new(Uuid::new_v4(), Uuid::nil()));
        assert!(row.is_footprint_unbound(), "nil footprint uuid is unbound");
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
