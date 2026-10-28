//! Deterministic content hashing for component rows.
//!
//! Hash is computed over a canonical JSON serialisation of the row's
//! binding fields (primitive refs, MPN, supply, parameters, …). Sorted-key
//! `BTreeMap`s keep output byte-stable across runs.
//!
//! Excluded from the canon view (intentionally): `created`, `updated`,
//! `content_hash`. Those are bookkeeping that changes on every save and
//! would defeat the "did the technical content actually change?" question
//! the hash answers.
//!
//! The pattern is canonical JSON over a `Serialize` view of `ComponentRow`.

use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::adapter::LibraryError;
use crate::alternate_symbol::AlternateSymbol;
use crate::component::{ComponentRow, DatasheetDoc, DatasheetRef, PinPadOverride, PlmReserved};
use crate::identity::{ComponentClass, InternalPn};
use crate::lifecycle::LifecycleState;
use crate::manufacturer::{DistributorListing, ManufacturerPart};
use crate::param::{ParamMap, ParamValue};
use crate::primitive::PrimitiveRef;

/// Canonical serialisation view — only the fields the hash should care about.
///
/// `row_id` IS hashed: a row's identity is part of its content. `internal_pn`
/// and `class` are user-renameable fields and so they ARE hashed (changing
/// either is a real content change). Bookkeeping (`created` / `updated` /
/// `content_hash`) is omitted.
#[derive(Serialize)]
struct CanonView<'a> {
    row_id: &'a Uuid,
    internal_pn: &'a InternalPn,
    class: &'a ComponentClass,
    state: &'a LifecycleState,
    datasheet: &'a DatasheetRef,
    // Skipped while empty so rows from before #1231 keep their hash.
    #[serde(skip_serializing_if = "str::is_empty")]
    datasheet_label: &'a str,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    more_datasheets: &'a [DatasheetDoc],
    symbol_ref: &'a PrimitiveRef,
    footprint_ref: Option<&'a PrimitiveRef>,
    sim_ref: Option<&'a PrimitiveRef>,
    pin_map_overrides: &'a [PinPadOverride],
    primary_mpn: &'a ManufacturerPart,
    alternates: &'a [ManufacturerPart],
    // Omitted when empty so rows without alternate symbols keep their
    // pre-existing hash.
    #[serde(skip_serializing_if = "<[AlternateSymbol]>::is_empty")]
    alternate_symbols: &'a [AlternateSymbol],
    supply: &'a [DistributorListing],
    parameters: &'a ParamMap,
    plm: &'a PlmReserved,
}

impl<'a> CanonView<'a> {
    fn from_row(row: &'a ComponentRow) -> Self {
        Self {
            row_id: &row.row_id,
            internal_pn: &row.internal_pn,
            class: &row.class,
            state: &row.state,
            datasheet: &row.datasheet,
            datasheet_label: &row.datasheet_label,
            more_datasheets: &row.more_datasheets,
            symbol_ref: &row.symbol_ref,
            footprint_ref: row.footprint_ref.as_ref(),
            sim_ref: row.sim_ref.as_ref(),
            pin_map_overrides: &row.pin_map_overrides,
            primary_mpn: &row.primary_mpn,
            alternates: &row.alternates,
            alternate_symbols: &row.alternate_symbols,
            supply: &row.supply,
            parameters: &row.parameters,
            plm: &row.plm,
        }
    }
}

/// Compute the canonical content hash of a row.
///
/// Returns `LibraryError::Backend` when the row contains a non-finite float
/// (`NaN` / `±Infinity`) inside any `ParamValue::Number` / `ParamValue::
/// Measurement` reached by the canonical view. Two reasons we trap this here
/// rather than upstream:
///
/// 1. **Hash determinism.** `serde_json` silently encodes `NaN` / `Infinity`
///    as JSON `null`, so a row carrying `Number(NaN)` would hash equal to a
///    row carrying `Number(0.0)` (after the upstream constructor zeroed it
///    out, etc.) — that defeats the "did the technical content actually
///    change?" question content_hash answers.
/// 2. **Less-invasive than upstream validation.** Tightening `ParamValue` to
///    reject non-finite floats at construction would require making variants
///    `#[non_exhaustive]` and rewriting ~50 enum-literal call sites in
///    signex-app and tests. Boundary validation here keeps the change
///    localised to the two functions whose semantics actually depend on it.
pub fn hash_row_content(row: &ComponentRow) -> Result<[u8; 32], LibraryError> {
    check_param_map_finite(&row.parameters)?;

    // serde_json::to_vec on `Serialize` is infallible for the field set used
    // by CanonView (no non-finite floats remain after the check above; all
    // other fields are strings / ints / structs of strings). The match arm
    // exists for defence-in-depth: any future field added to CanonView that
    // can fail to serialise will surface as a `Backend` error rather than a
    // panic.
    let canon = serde_json::to_vec(&CanonView::from_row(row)).map_err(|e| {
        LibraryError::Backend(format!("hash row content: serialise canonical view: {e}"))
    })?;
    let mut hasher = Sha256::new();
    hasher.update(&canon);
    let result = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    Ok(out)
}

/// Reject any `ParamValue::Number` / `ParamValue::Measurement` whose float
/// payload is not finite (`NaN`, `Infinity`, `-Infinity`).
pub(crate) fn check_param_map_finite(params: &ParamMap) -> Result<(), LibraryError> {
    for (key, value) in params {
        match value {
            ParamValue::Number(n) if !n.is_finite() => {
                return Err(LibraryError::Backend(format!(
                    "parameter {key:?} is non-finite ({n}); refused: NaN / Infinity cannot be stored"
                )));
            }
            ParamValue::Measurement { value: n, .. } if !n.is_finite() => {
                return Err(LibraryError::Backend(format!(
                    "parameter {key:?} measurement value is non-finite ({n}); refused: NaN / Infinity cannot be stored"
                )));
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::PinPadOverride;
    use chrono::{TimeZone, Utc};

    fn fixture_row() -> ComponentRow {
        let lib = Uuid::nil();
        let t = Utc.with_ymd_and_hms(2026, 4, 1, 12, 0, 0).unwrap();
        ComponentRow {
            row_id: Uuid::nil(),
            internal_pn: InternalPn::new("R10K"),
            class: ComponentClass::new("resistor"),
            datasheet: DatasheetRef::url(""),
            datasheet_label: String::new(),
            more_datasheets: Vec::new(),
            state: LifecycleState::Released,
            symbol_ref: PrimitiveRef::new(lib, Uuid::nil()),
            alternate_symbols: Vec::new(),
            footprint_ref: None,
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft("Acme", "ACM-001"),
            alternates: Vec::new(),
            supply: Vec::new(),
            parameters: ParamMap::new(),
            plm: PlmReserved::default(),
            version: "0.0.1".into(),
            released: false,
            symbol_version: String::new(),
            footprint_version: String::new(),
            sim_version: String::new(),
            created: t,
            updated: t,
            content_hash: [0u8; 32],
            server_revision: None,
        }
    }

    /// Pins the canonical encoding: a row without alternate symbols must
    /// keep the hash stored before that field existed.
    #[test]
    fn hash_of_row_without_alternate_symbols_is_stable() {
        let h = hash_row_content(&fixture_row()).unwrap();
        let hex: String = h.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            hex,
            "d8cc43c86aa2c893a5371ba6aa1969184a3a71306d14c8907debdc374f62a174"
        );
    }

    #[test]
    fn hash_changes_when_alternate_symbol_added() {
        let a = fixture_row();
        let mut b = fixture_row();
        b.alternate_symbols.push(AlternateSymbol::new(
            PrimitiveRef::new(Uuid::nil(), Uuid::now_v7()),
            "0.0.1",
            "IEC",
        ));
        assert_ne!(hash_row_content(&a).unwrap(), hash_row_content(&b).unwrap());
    }

    #[test]
    fn hash_is_deterministic() {
        let row = fixture_row();
        assert_eq!(
            hash_row_content(&row).unwrap(),
            hash_row_content(&row).unwrap()
        );
    }

    /// Pinned: the document fields added in #1231 are left out of the
    /// canonical view while empty, so a row without them keeps the hash
    /// it had before.
    #[test]
    fn hash_of_a_row_without_extra_documents_is_unchanged() {
        assert_eq!(
            hex(&hash_row_content(&fixture_row()).unwrap()),
            "d8cc43c86aa2c893a5371ba6aa1969184a3a71306d14c8907debdc374f62a174"
        );
    }

    #[test]
    fn hash_changes_when_documents_change() {
        let a = fixture_row();
        let mut b = fixture_row();
        b.datasheet_label = "TI".into();
        assert_ne!(hash_row_content(&a).unwrap(), hash_row_content(&b).unwrap());
        let mut c = fixture_row();
        c.more_datasheets.push(crate::component::DatasheetDoc::new(
            "Errata",
            DatasheetRef::url("e.pdf"),
        ));
        assert_ne!(hash_row_content(&a).unwrap(), hash_row_content(&c).unwrap());
    }

    fn hex(bytes: &[u8; 32]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn hash_changes_when_primary_mpn_changes() {
        let mut a = fixture_row();
        let mut b = fixture_row();
        a.primary_mpn = ManufacturerPart::draft("Acme", "A");
        b.primary_mpn = ManufacturerPart::draft("Acme", "B");
        assert_ne!(hash_row_content(&a).unwrap(), hash_row_content(&b).unwrap());
    }

    #[test]
    fn hash_changes_when_symbol_ref_changes() {
        let mut a = fixture_row();
        let mut b = fixture_row();
        a.symbol_ref = PrimitiveRef::new(Uuid::nil(), Uuid::nil());
        b.symbol_ref = PrimitiveRef::new(Uuid::nil(), Uuid::now_v7());
        assert_ne!(hash_row_content(&a).unwrap(), hash_row_content(&b).unwrap());
    }

    #[test]
    fn hash_changes_when_pin_map_changes() {
        let a = fixture_row();
        let mut b = fixture_row();
        b.pin_map_overrides.push(PinPadOverride::new("EP", "EP1"));
        assert_ne!(hash_row_content(&a).unwrap(), hash_row_content(&b).unwrap());
    }

    /// Bookkeeping fields (`created`, `updated`, `content_hash`) MUST NOT
    /// affect the content hash — that's the whole point of distinguishing
    /// technical content from save metadata.
    #[test]
    fn hash_ignores_timestamps_and_self_hash() {
        let mut a = fixture_row();
        let mut b = fixture_row();
        a.created = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        b.created = Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap();
        a.updated = Utc::now();
        b.updated = Utc::now() + chrono::Duration::days(99);
        a.content_hash = [0xAA; 32];
        b.content_hash = [0xBB; 32];
        assert_eq!(hash_row_content(&a).unwrap(), hash_row_content(&b).unwrap());
    }

    #[test]
    fn hash_changes_when_class_changes() {
        let mut a = fixture_row();
        let mut b = fixture_row();
        a.class = ComponentClass::new("resistor");
        b.class = ComponentClass::new("capacitor");
        assert_ne!(hash_row_content(&a).unwrap(), hash_row_content(&b).unwrap());
    }

    #[test]
    fn hash_changes_when_state_changes() {
        let mut a = fixture_row();
        let mut b = fixture_row();
        a.state = LifecycleState::Released;
        b.state = LifecycleState::Deprecated;
        assert_ne!(hash_row_content(&a).unwrap(), hash_row_content(&b).unwrap());
    }

    /// `serde_json` cannot encode `NaN` / `±Infinity`. The hash function
    /// surfaces that as `LibraryError::Backend` instead of panicking.
    #[test]
    fn hash_returns_backend_error_on_non_finite_float() {
        use crate::param::ParamValue;
        let mut row = fixture_row();
        row.parameters
            .insert("bad".into(), ParamValue::Number(f64::NAN));
        let err = hash_row_content(&row).unwrap_err();
        assert!(matches!(err, LibraryError::Backend(_)), "got {err:?}");

        let mut row = fixture_row();
        row.parameters
            .insert("bad".into(), ParamValue::Number(f64::INFINITY));
        let err = hash_row_content(&row).unwrap_err();
        assert!(matches!(err, LibraryError::Backend(_)), "got {err:?}");
    }
}
