//! Deterministic content hashing for revisions.
//!
//! Hash is computed over a canonical JSON serialisation of the revision's
//! binding fields (primitive refs, MPN, supply, parameters, …). Sorted-key
//! `BTreeMap`s keep output byte-stable across runs.
//!
//! Excluded from the canon view (intentionally): `version`, `state`,
//! `created`, `author`, `message`. Those are bookkeeping that changes on every
//! save and would defeat the "did the technical content actually change?"
//! question the hash answers.

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::component::{DatasheetRef, PinPadOverride, PlmReserved, Revision};
use crate::manufacturer::{DistributorListing, ManufacturerPart};
use crate::param::ParamMap;
use crate::primitive::PrimitiveRef;

/// Canonical serialisation view — only the fields the hash should care about.
#[derive(Serialize)]
struct CanonView<'a> {
    symbol_ref: &'a PrimitiveRef,
    footprint_ref: Option<&'a PrimitiveRef>,
    sim_ref: Option<&'a PrimitiveRef>,
    pin_map_overrides: &'a [PinPadOverride],
    primary_mpn: &'a ManufacturerPart,
    alternates: &'a [ManufacturerPart],
    supply: &'a [DistributorListing],
    datasheet: &'a DatasheetRef,
    parameters: &'a ParamMap,
    plm: &'a PlmReserved,
}

impl<'a> CanonView<'a> {
    fn from_revision(rev: &'a Revision) -> Self {
        Self {
            symbol_ref: &rev.symbol_ref,
            footprint_ref: rev.footprint_ref.as_ref(),
            sim_ref: rev.sim_ref.as_ref(),
            pin_map_overrides: &rev.pin_map_overrides,
            primary_mpn: &rev.primary_mpn,
            alternates: &rev.alternates,
            supply: &rev.supply,
            datasheet: &rev.datasheet,
            parameters: &rev.parameters,
            plm: &rev.plm,
        }
    }
}

/// Compute the canonical content hash of a revision.
pub fn hash_revision_content(rev: &Revision) -> [u8; 32] {
    let canon = serde_json::to_vec(&CanonView::from_revision(rev))
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
    use crate::identity::Version;
    use crate::lifecycle::LifecycleState;
    use uuid::Uuid;

    fn fixture_revision() -> Revision {
        let lib = Uuid::nil();
        Revision {
            version: Version::new(1, 0),
            state: LifecycleState::Released,
            created: chrono::Utc::now(),
            author: "test".into(),
            message: "init".into(),
            symbol_ref: PrimitiveRef::new(lib, Uuid::nil()),
            footprint_ref: None,
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft("Acme", "ACM-001"),
            alternates: Vec::new(),
            supply: Vec::new(),
            datasheet: DatasheetRef::url(""),
            parameters: ParamMap::new(),
            plm: PlmReserved::default(),
            content_hash: [0u8; 32],
        }
    }

    #[test]
    fn hash_is_deterministic() {
        let rev = fixture_revision();
        // Same input → same hash, twice in a row.
        assert_eq!(hash_revision_content(&rev), hash_revision_content(&rev));
    }

    #[test]
    fn hash_changes_when_primary_mpn_changes() {
        let mut a = fixture_revision();
        let mut b = fixture_revision();
        a.primary_mpn = ManufacturerPart::draft("Acme", "A");
        b.primary_mpn = ManufacturerPart::draft("Acme", "B");
        assert_ne!(hash_revision_content(&a), hash_revision_content(&b));
    }

    #[test]
    fn hash_changes_when_symbol_ref_changes() {
        let mut a = fixture_revision();
        let mut b = fixture_revision();
        a.symbol_ref = PrimitiveRef::new(Uuid::nil(), Uuid::nil());
        b.symbol_ref = PrimitiveRef::new(Uuid::nil(), Uuid::now_v7());
        assert_ne!(hash_revision_content(&a), hash_revision_content(&b));
    }

    #[test]
    fn hash_changes_when_pin_map_changes() {
        let mut a = fixture_revision();
        let mut b = fixture_revision();
        b.pin_map_overrides
            .push(PinPadOverride::new("EP", "EP1"));
        assert_ne!(hash_revision_content(&a), hash_revision_content(&b));
    }

    /// Bookkeeping fields (`message`, `author`, etc.) MUST NOT affect the
    /// content hash — that's the whole point of distinguishing technical
    /// content from save metadata.
    #[test]
    fn hash_ignores_message_and_author() {
        let mut a = fixture_revision();
        let mut b = fixture_revision();
        a.author = "alice".into();
        a.message = "first".into();
        b.author = "bob".into();
        b.message = "second".into();
        assert_eq!(hash_revision_content(&a), hash_revision_content(&b));
    }
}
