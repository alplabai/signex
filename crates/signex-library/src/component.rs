//! `Component` is now a **thin binding record** — see
//! `v0.9-library-refactor-plan.md` §2.4.
//!
//! Each `Revision` references reusable primitives (`Symbol`, `Footprint`,
//! `SimModel`) by `(library_id, uuid)` tuples (`PrimitiveRef`) instead of
//! embedding their geometry. Two MPNs sharing a SOIC-8 footprint reference the
//! same primitive UUID rather than carrying their own copy.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::identity::{ComponentClass, ComponentId, InternalPn, Version};
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

/// PLM-reserved fields. Inert in v0.9 / v1.x; populated when the PLM adapter
/// ships in v3.0.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlmReserved {
    #[serde(default)]
    pub plm_part_id: Option<String>,
    #[serde(default)]
    pub eco_refs: Vec<String>,
    #[serde(default)]
    pub compliance: BTreeMap<String, String>,
}

/// One commit's worth of a component — Altium-style binding record.
///
/// Per the refactor plan, a `Revision` no longer embeds the symbol/footprint
/// blob; it points at primitives by `(library_id, uuid)` tuples. The visual
/// diff engine (`diff.rs`) operates over the *binding fields* and asks the
/// adapter to resolve referenced primitives only when a primitive-aware diff
/// is requested.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Revision {
    pub version: Version,
    pub state: LifecycleState,
    pub created: chrono::DateTime<chrono::Utc>,
    pub author: String,
    pub message: String,

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
    #[serde(default)]
    pub datasheet: DatasheetRef,

    // ── Parametric ──────────────────────────────────────────────────────
    /// Schema-validated against the class template via
    /// [`crate::TemplateRegistry::validate_params`].
    #[serde(default)]
    pub parameters: ParamMap,

    // ── PLM-reserved (inert until v3.0) ─────────────────────────────────
    #[serde(default)]
    pub plm: PlmReserved,

    /// SHA-256 of canonicalised JSON over the binding fields. See
    /// [`crate::hash::hash_revision_content`].
    #[serde(default)]
    pub content_hash: [u8; 32],
}

impl Revision {
    /// Recompute and store the content hash from the canonical view.
    pub fn refresh_content_hash(&mut self) {
        self.content_hash = crate::hash::hash_revision_content(self);
    }
}

/// Logical component — a thin binding record holding `internal_pn`, class,
/// category, optional family, and an ordered list of revisions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Component {
    pub uuid: ComponentId,
    pub internal_pn: InternalPn,
    /// Picks the parameter template ("resistor", "opamp", …).
    pub class: ComponentClass,
    /// Tree-style category path ("Passives/Resistors/0805"). Used by the
    /// library picker tree.
    #[serde(default)]
    pub category: PathBuf,
    /// Optional family UUID — groups multi-package siblings (TQFP / QFN / BGA).
    #[serde(default)]
    pub family: Option<Uuid>,
    pub revisions: Vec<Revision>,
    /// The "Released" tip — typically the highest Released revision.
    pub head: Version,
}

impl Component {
    /// Find the head revision. Returns `None` if `head` doesn't reference a
    /// known revision.
    pub fn head_revision(&self) -> Option<&Revision> {
        self.revisions.iter().find(|r| r.version == self.head)
    }

    /// Highest Released revision, or `None` if no Released revision exists.
    pub fn highest_released(&self) -> Option<&Revision> {
        self.revisions
            .iter()
            .filter(|r| r.state == LifecycleState::Released)
            .max_by_key(|r| r.version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manufacturer::ManufacturerPart;

    fn fixture_revision(version: Version, state: LifecycleState) -> Revision {
        let lib = Uuid::new_v4();
        Revision {
            version,
            state,
            created: chrono::Utc::now(),
            author: "test@example.com".into(),
            message: "initial".into(),
            symbol_ref: PrimitiveRef::new(lib, Uuid::new_v4()),
            footprint_ref: Some(PrimitiveRef::new(lib, Uuid::new_v4())),
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft("Acme", "ACM-001"),
            alternates: Vec::new(),
            supply: Vec::new(),
            datasheet: DatasheetRef::url("https://example.com/ds.pdf"),
            parameters: ParamMap::new(),
            plm: PlmReserved::default(),
            content_hash: [0u8; 32],
        }
    }

    #[test]
    fn component_revision_holds_primitive_refs() {
        let lib_id = Uuid::new_v4();
        let sym_uuid = Uuid::new_v4();
        let fpt_uuid = Uuid::new_v4();
        let rev = Revision {
            version: Version::new(0, 1),
            state: LifecycleState::Draft,
            created: chrono::Utc::now(),
            author: "test".into(),
            message: "init".into(),
            symbol_ref: PrimitiveRef::new(lib_id, sym_uuid),
            footprint_ref: Some(PrimitiveRef::new(lib_id, fpt_uuid)),
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft("Acme", "ACM-001"),
            alternates: Vec::new(),
            supply: Vec::new(),
            datasheet: DatasheetRef::url("https://example.com/ds.pdf"),
            parameters: ParamMap::new(),
            plm: PlmReserved::default(),
            content_hash: [0u8; 32],
        };
        assert_eq!(rev.symbol_ref.uuid, sym_uuid);
        assert_eq!(rev.footprint_ref.unwrap().uuid, fpt_uuid);
    }

    #[test]
    fn head_revision_resolves() {
        let c = Component {
            uuid: Uuid::now_v7(),
            internal_pn: InternalPn::new("R_TEST"),
            class: ComponentClass::new("resistor"),
            category: PathBuf::from("Passives/Resistors"),
            family: None,
            revisions: vec![
                fixture_revision(Version::new(1, 0), LifecycleState::Released),
                fixture_revision(Version::new(1, 1), LifecycleState::Released),
            ],
            head: Version::new(1, 1),
        };
        assert_eq!(c.head_revision().unwrap().version, Version::new(1, 1));
    }

    #[test]
    fn highest_released_skips_drafts() {
        let c = Component {
            uuid: Uuid::now_v7(),
            internal_pn: InternalPn::new("R_TEST"),
            class: ComponentClass::new("resistor"),
            category: PathBuf::new(),
            family: None,
            revisions: vec![
                fixture_revision(Version::new(1, 0), LifecycleState::Released),
                fixture_revision(Version::new(1, 1), LifecycleState::Draft),
            ],
            head: Version::new(1, 0),
        };
        assert_eq!(c.highest_released().unwrap().version, Version::new(1, 0));
    }

    #[test]
    fn component_round_trip() {
        let c = Component {
            uuid: Uuid::now_v7(),
            internal_pn: InternalPn::new("R0805_10k"),
            class: ComponentClass::new("resistor"),
            category: PathBuf::from("Passives/Resistors/0805"),
            family: None,
            revisions: vec![fixture_revision(
                Version::new(1, 0),
                LifecycleState::Released,
            )],
            head: Version::new(1, 0),
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: Component = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn refresh_content_hash_populates() {
        let mut rev = fixture_revision(Version::new(1, 0), LifecycleState::Released);
        assert_eq!(rev.content_hash, [0u8; 32]);
        rev.refresh_content_hash();
        assert_ne!(rev.content_hash, [0u8; 32]);
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
}
