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
//! Per `v0.9-refactor-2-plan.md` §6 step 1.6, this replaces the older
//! `hash_revision_content`. The pattern (canonical JSON over a `Serialize`
//! view) is identical; only the input type changed from `Revision` to
//! `ComponentRow`.

use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::component::{ComponentRow, DatasheetRef, PinPadOverride, PlmReserved};
use crate::identity::{ComponentClass, InternalPn};
use crate::lifecycle::LifecycleState;
use crate::manufacturer::{DistributorListing, ManufacturerPart};
use crate::param::ParamMap;
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
    symbol_ref: &'a PrimitiveRef,
    footprint_ref: Option<&'a PrimitiveRef>,
    sim_ref: Option<&'a PrimitiveRef>,
    pin_map_overrides: &'a [PinPadOverride],
    primary_mpn: &'a ManufacturerPart,
    alternates: &'a [ManufacturerPart],
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
            symbol_ref: &row.symbol_ref,
            footprint_ref: row.footprint_ref.as_ref(),
            sim_ref: row.sim_ref.as_ref(),
            pin_map_overrides: &row.pin_map_overrides,
            primary_mpn: &row.primary_mpn,
            alternates: &row.alternates,
            supply: &row.supply,
            parameters: &row.parameters,
            plm: &row.plm,
        }
    }
}

/// Compute the canonical content hash of a row.
pub fn hash_row_content(row: &ComponentRow) -> [u8; 32] {
    let canon = serde_json::to_vec(&CanonView::from_row(row))
        .expect("CanonView must serialise");
    let mut hasher = Sha256::new();
    hasher.update(&canon);
    let result = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
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
            state: LifecycleState::Released,
            symbol_ref: PrimitiveRef::new(lib, Uuid::nil()),
            footprint_ref: None,
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft("Acme", "ACM-001"),
            alternates: Vec::new(),
            supply: Vec::new(),
            parameters: ParamMap::new(),
            plm: PlmReserved::default(),
            created: t,
            updated: t,
            content_hash: [0u8; 32],
        }
    }

    #[test]
    fn hash_is_deterministic() {
        let row = fixture_row();
        assert_eq!(hash_row_content(&row), hash_row_content(&row));
    }

    #[test]
    fn hash_changes_when_primary_mpn_changes() {
        let mut a = fixture_row();
        let mut b = fixture_row();
        a.primary_mpn = ManufacturerPart::draft("Acme", "A");
        b.primary_mpn = ManufacturerPart::draft("Acme", "B");
        assert_ne!(hash_row_content(&a), hash_row_content(&b));
    }

    #[test]
    fn hash_changes_when_symbol_ref_changes() {
        let mut a = fixture_row();
        let mut b = fixture_row();
        a.symbol_ref = PrimitiveRef::new(Uuid::nil(), Uuid::nil());
        b.symbol_ref = PrimitiveRef::new(Uuid::nil(), Uuid::now_v7());
        assert_ne!(hash_row_content(&a), hash_row_content(&b));
    }

    #[test]
    fn hash_changes_when_pin_map_changes() {
        let a = fixture_row();
        let mut b = fixture_row();
        b.pin_map_overrides.push(PinPadOverride::new("EP", "EP1"));
        assert_ne!(hash_row_content(&a), hash_row_content(&b));
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
        assert_eq!(hash_row_content(&a), hash_row_content(&b));
    }

    #[test]
    fn hash_changes_when_class_changes() {
        let mut a = fixture_row();
        let mut b = fixture_row();
        a.class = ComponentClass::new("resistor");
        b.class = ComponentClass::new("capacitor");
        assert_ne!(hash_row_content(&a), hash_row_content(&b));
    }

    #[test]
    fn hash_changes_when_state_changes() {
        let mut a = fixture_row();
        let mut b = fixture_row();
        a.state = LifecycleState::Released;
        b.state = LifecycleState::Deprecated;
        assert_ne!(hash_row_content(&a), hash_row_content(&b));
    }
}
