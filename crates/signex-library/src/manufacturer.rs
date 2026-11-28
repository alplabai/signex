//! Manufacturer-part + supply-chain types.
//!
//! Per `v0.9-refactor-2-plan.md` §2.4, every `Revision` carries a
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

/// Where a manufacturer part is in its maker's production life (#3046).
/// Not the library row's own [`crate::LifecycleState`], which says whether
/// the librarian released the row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ManufacturerLifecycle {
    /// In production.
    Active,
    /// Not recommended for new designs.
    #[serde(rename = "NRND")]
    Nrnd,
    /// End of life: last-time buy, obsolete.
    #[serde(rename = "EOL")]
    Eol,
}

impl ManufacturerLifecycle {
    /// Every value, in the order a picker lists them.
    pub const ALL: [Self; 3] = [Self::Active, Self::Nrnd, Self::Eol];

    /// The token the library file writes (`Active`, `NRND`, `EOL`).
    pub fn token(self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Nrnd => "NRND",
            Self::Eol => "EOL",
        }
    }

    /// Read a distributor's lifecycle text (`"Active"`, `"Not Recommended
    /// for New Designs"`, `"Obsolete"`, ...). `None` for text that says
    /// nothing about the maker's production, such as a distributor no longer
    /// stocking the part.
    pub fn from_distributor_text(text: &str) -> Option<Self> {
        let words = text.trim().to_ascii_lowercase();
        let has = |needle: &str| words.contains(needle);
        if words.is_empty() || has("distributor") || has("at digi") || has("at mouser") {
            return None;
        }
        if has("nrnd") || has("not recommended") || has("not for new") {
            Some(Self::Nrnd)
        } else if has("eol")
            || has("end of life")
            || has("obsolete")
            || has("last time")
            || has("discontinued")
        {
            Some(Self::Eol)
        } else if words == "active" || words.starts_with("active ") || has("new product") {
            Some(Self::Active)
        } else {
            None
        }
    }
}

/// One manufacturer part — primary or alternate.
///
/// `manual_stock` and `manual_lifecycle` are what the librarian entered in
/// the editor's Supply tab; they win over the values a distributor refresh
/// fetched, which live in the table's `alternate_supply` cache
/// ([`crate::alternate_supply`]), never here. Both are written only when set,
/// and a file that holds one is `snxlib/2.1` (`docs/LIBRARY_FORMAT.md` 2.4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ManufacturerPart {
    pub manufacturer: String,
    pub mpn: String,
    pub status: AlternateStatus,
    #[serde(default)]
    pub notes: Option<String>,
    /// Units in stock as entered by hand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_stock: Option<u32>,
    /// The maker's lifecycle as entered by hand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_lifecycle: Option<ManufacturerLifecycle>,
}

impl ManufacturerPart {
    /// Convenience: an in-progress draft part. `Primary` status, no notes.
    pub fn draft(manufacturer: impl Into<String>, mpn: impl Into<String>) -> Self {
        Self {
            manufacturer: manufacturer.into(),
            mpn: mpn.into(),
            status: AlternateStatus::Primary,
            notes: None,
            manual_stock: None,
            manual_lifecycle: None,
        }
    }

    /// Whether the part carries a hand-entered stock or lifecycle, which only
    /// `snxlib/2.1` holds.
    pub fn has_manual_supply(&self) -> bool {
        self.manual_stock.is_some() || self.manual_lifecycle.is_some()
    }
}

/// The JSON keys of [`ManufacturerPart`]'s hand-entered values, as
/// `serde_json` writes them into a row cell.
pub(crate) const MANUAL_SUPPLY_KEYS: [&str; 2] = ["\"manual_stock\":", "\"manual_lifecycle\":"];

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
            manual_stock: None,
            manual_lifecycle: None,
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: ManufacturerPart = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn a_part_without_manual_supply_keeps_its_old_bytes() {
        let p = ManufacturerPart::draft("Example Corp", "EX-1");
        assert_eq!(
            serde_json::to_string(&p).unwrap(),
            r#"{"manufacturer":"Example Corp","mpn":"EX-1","status":"Primary","notes":null}"#
        );
        assert!(!p.has_manual_supply());
    }

    #[test]
    fn manual_supply_is_written_after_notes_and_survives_a_round_trip() {
        let p = ManufacturerPart {
            manual_stock: Some(1200),
            manual_lifecycle: Some(ManufacturerLifecycle::Nrnd),
            ..ManufacturerPart::draft("Example Corp", "EX-1")
        };
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(
            json,
            r#"{"manufacturer":"Example Corp","mpn":"EX-1","status":"Primary","notes":null,"manual_stock":1200,"manual_lifecycle":"NRND"}"#
        );
        for key in MANUAL_SUPPLY_KEYS {
            assert!(json.contains(key), "{key}");
        }
        assert_eq!(serde_json::from_str::<ManufacturerPart>(&json).unwrap(), p);
        assert!(p.has_manual_supply());
    }

    #[test]
    fn lifecycle_reads_distributor_wording() {
        use ManufacturerLifecycle::*;
        for (text, want) in [
            ("Active", Some(Active)),
            ("New Product", Some(Active)),
            ("Not Recommended for New Designs", Some(Nrnd)),
            ("Not For New Designs", Some(Nrnd)),
            ("NRND", Some(Nrnd)),
            ("End of Life", Some(Eol)),
            ("Obsolete", Some(Eol)),
            ("Last Time Buy", Some(Eol)),
            ("Discontinued at Digi-Key", None),
            ("", None),
            ("Preliminary", None),
            ("Inactive", None),
        ] {
            assert_eq!(
                ManufacturerLifecycle::from_distributor_text(text),
                want,
                "{text}"
            );
        }
    }

    #[test]
    fn lifecycle_tokens_are_the_spec_spelling() {
        let tokens: Vec<String> = ManufacturerLifecycle::ALL
            .iter()
            .map(|l| serde_json::to_string(l).unwrap())
            .collect();
        assert_eq!(tokens, [r#""Active""#, r#""NRND""#, r#""EOL""#]);
        for l in ManufacturerLifecycle::ALL {
            assert_eq!(
                serde_json::to_string(&l).unwrap(),
                format!("\"{}\"", l.token())
            );
        }
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
        assert!(p.manual_stock.is_none() && p.manual_lifecycle.is_none());
    }
}
