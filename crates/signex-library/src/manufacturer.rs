//! Manufacturer-part + supply-chain types.
//!
//! Per `v0.9-library-refactor-plan.md` §2.4, every `Revision` carries a
//! `primary_mpn: ManufacturerPart`, a ranked list of `alternates`, and a list
//! of `DistributorListing` entries describing where the part can be sourced.
//!
//! These types intentionally live in a separate module from the legacy
//! `distributor` adapters: an MPN/AVL entry is a *static* property of a
//! component revision, while distributor-cache and live-pricing belong to the
//! `distributor.rs` runtime adapters.

use serde::{Deserialize, Serialize};

/// Approval status for a manufacturer part. Mirrors Altium's
/// approved-vendor-list semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum AlternateStatus {
    /// The headline part — the BOM picks this by default.
    Primary,
    /// Approved alternate — drop-in replacement, BOM may auto-substitute.
    Approved,
    /// Approved with caveats — operator must confirm before substitution.
    Conditional,
    /// Disqualified — must NOT be substituted; carried for audit history.
    Disqualified,
}

/// One manufacturer part — primary or alternate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ManufacturerPart {
    pub manufacturer: String,
    pub mpn: String,
    pub status: AlternateStatus,
    #[serde(default)]
    pub notes: Option<String>,
}

impl ManufacturerPart {
    /// Convenience: an in-progress draft part. `Primary` status, no notes.
    pub fn draft(manufacturer: impl Into<String>, mpn: impl Into<String>) -> Self {
        Self {
            manufacturer: manufacturer.into(),
            mpn: mpn.into(),
            status: AlternateStatus::Primary,
            notes: None,
        }
    }
}

/// One distributor listing — where this MPN can be sourced.
///
/// Lifted onto `Revision` directly per the refactor plan. Pricing snapshots
/// (the live cache that `DistributorAdapter` populates) stay in
/// `distributor.rs` and are referenced by mpn rather than embedded here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DistributorListing {
    /// Distributor name — "DigiKey", "Mouser", "LCSC", etc.
    pub distributor: String,
    /// Distributor SKU / part number (their internal identifier).
    pub sku: String,
    /// Optional vendor URL for the part page.
    #[serde(default)]
    pub url: Option<String>,
    /// Optional minimum order quantity (for distributors with reels/packaging).
    #[serde(default)]
    pub moq: Option<u32>,
}

impl DistributorListing {
    /// Convenience constructor — distributor + SKU only.
    pub fn new(distributor: impl Into<String>, sku: impl Into<String>) -> Self {
        Self {
            distributor: distributor.into(),
            sku: sku.into(),
            url: None,
            moq: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manufacturer_part_round_trip() {
        let p = ManufacturerPart {
            manufacturer: "Yageo".into(),
            mpn: "RC0805FR-0710KL".into(),
            status: AlternateStatus::Primary,
            notes: Some("preferred vendor".into()),
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: ManufacturerPart = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn alternate_status_round_trip_all_variants() {
        for s in [
            AlternateStatus::Primary,
            AlternateStatus::Approved,
            AlternateStatus::Conditional,
            AlternateStatus::Disqualified,
        ] {
            let json = serde_json::to_string(&s).unwrap();
            let back: AlternateStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(s, back);
        }
    }

    #[test]
    fn distributor_listing_round_trip() {
        let l = DistributorListing {
            distributor: "DigiKey".into(),
            sku: "311-10.0KCRCT-ND".into(),
            url: Some("https://example.com/dk/311-10.0KCRCT-ND".into()),
            moq: Some(1),
        };
        let json = serde_json::to_string(&l).unwrap();
        let back: DistributorListing = serde_json::from_str(&json).unwrap();
        assert_eq!(l, back);
    }

    #[test]
    fn manufacturer_part_draft_is_primary() {
        let p = ManufacturerPart::draft("Acme", "ACM-001");
        assert_eq!(p.status, AlternateStatus::Primary);
        assert_eq!(p.manufacturer, "Acme");
        assert_eq!(p.mpn, "ACM-001");
        assert!(p.notes.is_none());
    }
}
