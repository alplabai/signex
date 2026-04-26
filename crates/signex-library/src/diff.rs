//! Pure-data diff between two revisions of the same component.
//!
//! Per `v0.9-library-refactor-plan.md` §7 step B5, the diff now operates on
//! primitive *references* and binding fields — geometry-level diffs (pin
//! moves, pad changes) live with the primitive editors and are out of scope
//! here. This crate's `RevisionDiff` answers:
//! "did the symbol primitive UUID change? did the MPN swap? did supply
//!  listings move? did the datasheet repoint?"
//!
//! The diff is the data backbone for:
//! * the visual diff renderer (drawn by signex-app — out of scope here),
//! * the auto-bump heuristic — call [`auto_bump_kind`] to decide whether a
//!   save should be a `.minor` or `.major` version bump.

use std::collections::BTreeSet;

use crate::component::{DatasheetRef, PinPadOverride, PlmReserved, Revision};
use crate::lifecycle::LifecycleState;
use crate::manufacturer::{DistributorListing, ManufacturerPart};
use crate::param::ParamMap;

/// Field-level changed flags + grouped detail. Mirrors plan §7 step B5.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RevisionDiff {
    pub symbol_changed: bool,
    pub footprint_changed: bool,
    pub sim_changed: bool,
    pub pin_map_changed: bool,
    pub params_changed: bool,
    pub mpn_changed: bool,
    pub alternates_changed: bool,
    pub supply_changed: bool,
    pub datasheet_changed: bool,
    pub lifecycle_changed: bool,
    pub plm_changed: bool,

    /// Detail rows — populated only for the dimensions actually changed.
    pub parameters: ParameterDiff,
    pub pin_map: PinMapDiff,
    pub alternates_detail: ListDiff,
    pub supply_detail: ListDiff,
    pub lifecycle_detail: LifecycleDiff,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParameterDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<(String, String, String)>, // key, old, new
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PinMapDiff {
    pub added: Vec<PinPadOverride>,
    pub removed: Vec<PinPadOverride>,
    pub changed: Vec<(String, String, String)>, // pin_number, old_pad, new_pad
}

/// Identity-keyed diff over a list — Strings keys (e.g. `manufacturer:mpn`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LifecycleDiff {
    pub from: Option<LifecycleState>,
    pub to: Option<LifecycleState>,
}

/// Auto-bump heuristic — major when the binding shape changes (symbol,
/// footprint, sim, or pin map), minor otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BumpKind {
    Minor,
    Major,
}

/// Decide whether the change between two revisions is a `Minor` or `Major`
/// bump. Per the plan §7 step B5, any change to a primitive ref or pin-map
/// override is a major bump.
pub fn auto_bump_kind(diff: &RevisionDiff) -> BumpKind {
    if diff.symbol_changed
        || diff.footprint_changed
        || diff.sim_changed
        || diff.pin_map_changed
    {
        BumpKind::Major
    } else {
        BumpKind::Minor
    }
}

/// Compute the diff from `a` to `b`.
pub fn diff_revisions(a: &Revision, b: &Revision) -> RevisionDiff {
    let parameters = diff_parameters(&a.parameters, &b.parameters);
    let pin_map = diff_pin_map(&a.pin_map_overrides, &b.pin_map_overrides);
    let alternates_detail = diff_alternates(&a.alternates, &b.alternates);
    let supply_detail = diff_supply(&a.supply, &b.supply);
    let lifecycle_detail = diff_lifecycle(a.state, b.state);

    RevisionDiff {
        symbol_changed: a.symbol_ref != b.symbol_ref,
        footprint_changed: a.footprint_ref != b.footprint_ref,
        sim_changed: a.sim_ref != b.sim_ref,
        pin_map_changed: !pin_map.added.is_empty()
            || !pin_map.removed.is_empty()
            || !pin_map.changed.is_empty(),
        params_changed: !parameters.added.is_empty()
            || !parameters.removed.is_empty()
            || !parameters.changed.is_empty(),
        mpn_changed: mpn_key(&a.primary_mpn) != mpn_key(&b.primary_mpn),
        alternates_changed: !alternates_detail.added.is_empty()
            || !alternates_detail.removed.is_empty(),
        supply_changed: !supply_detail.added.is_empty()
            || !supply_detail.removed.is_empty(),
        datasheet_changed: a.datasheet != b.datasheet,
        lifecycle_changed: lifecycle_detail.from.is_some(),
        plm_changed: a.plm != b.plm,
        parameters,
        pin_map,
        alternates_detail,
        supply_detail,
        lifecycle_detail,
    }
}

fn mpn_key(p: &ManufacturerPart) -> String {
    format!("{}:{}", p.manufacturer, p.mpn)
}

fn diff_parameters(a: &ParamMap, b: &ParamMap) -> ParameterDiff {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();
    let a_keys: BTreeSet<&String> = a.keys().collect();
    let b_keys: BTreeSet<&String> = b.keys().collect();
    for k in b_keys.difference(&a_keys) {
        added.push((*k).clone());
    }
    for k in a_keys.difference(&b_keys) {
        removed.push((*k).clone());
    }
    for k in a_keys.intersection(&b_keys) {
        let av = &a[*k];
        let bv = &b[*k];
        if av != bv {
            changed.push(((*k).clone(), av.display(), bv.display()));
        }
    }
    ParameterDiff {
        added,
        removed,
        changed,
    }
}

fn diff_pin_map(a: &[PinPadOverride], b: &[PinPadOverride]) -> PinMapDiff {
    use std::collections::BTreeMap;
    // Index by symbol_pin_number — the unique key.
    let a_map: BTreeMap<&str, &str> = a
        .iter()
        .map(|o| (o.symbol_pin_number.as_str(), o.footprint_pad_number.as_str()))
        .collect();
    let b_map: BTreeMap<&str, &str> = b
        .iter()
        .map(|o| (o.symbol_pin_number.as_str(), o.footprint_pad_number.as_str()))
        .collect();
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();
    for (k, v) in &b_map {
        match a_map.get(k) {
            None => added.push(PinPadOverride::new(*k, *v)),
            Some(av) if av != v => {
                changed.push(((*k).to_string(), (*av).to_string(), (*v).to_string()))
            }
            _ => {}
        }
    }
    for (k, v) in &a_map {
        if !b_map.contains_key(k) {
            removed.push(PinPadOverride::new(*k, *v));
        }
    }
    PinMapDiff {
        added,
        removed,
        changed,
    }
}

fn diff_alternates(a: &[ManufacturerPart], b: &[ManufacturerPart]) -> ListDiff {
    let to_key = mpn_key;
    let a_keys: BTreeSet<String> = a.iter().map(to_key).collect();
    let b_keys: BTreeSet<String> = b.iter().map(to_key).collect();
    ListDiff {
        added: b_keys.difference(&a_keys).cloned().collect(),
        removed: a_keys.difference(&b_keys).cloned().collect(),
    }
}

fn diff_supply(a: &[DistributorListing], b: &[DistributorListing]) -> ListDiff {
    let to_key = |s: &DistributorListing| format!("{}:{}", s.distributor, s.sku);
    let a_keys: BTreeSet<String> = a.iter().map(to_key).collect();
    let b_keys: BTreeSet<String> = b.iter().map(to_key).collect();
    ListDiff {
        added: b_keys.difference(&a_keys).cloned().collect(),
        removed: a_keys.difference(&b_keys).cloned().collect(),
    }
}

fn diff_lifecycle(a: LifecycleState, b: LifecycleState) -> LifecycleDiff {
    if a == b {
        LifecycleDiff::default()
    } else {
        LifecycleDiff {
            from: Some(a),
            to: Some(b),
        }
    }
}

/// Touch the unused-warning placeholder so `PlmReserved` and `DatasheetRef`
/// stay imported even when the public API changes (silences "unused import"
/// after a refactor).
#[allow(dead_code)]
fn _phantom(_: &PlmReserved, _: &DatasheetRef) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::PinPadOverride;
    use crate::identity::Version;
    use crate::manufacturer::ManufacturerPart;
    use crate::param::ParamValue;
    use crate::primitive::PrimitiveRef;
    use uuid::Uuid;

    fn rev() -> Revision {
        Revision {
            version: Version::new(1, 0),
            state: LifecycleState::Released,
            created: chrono::Utc::now(),
            author: "test".into(),
            message: "init".into(),
            symbol_ref: PrimitiveRef::new(Uuid::nil(), Uuid::nil()),
            footprint_ref: None,
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft("Acme", "A"),
            alternates: Vec::new(),
            supply: Vec::new(),
            datasheet: DatasheetRef::url(""),
            parameters: ParamMap::new(),
            plm: PlmReserved::default(),
            content_hash: [0u8; 32],
        }
    }

    #[test]
    fn diff_detects_symbol_ref_change() {
        let mut a = rev();
        let mut b = rev();
        let lib = Uuid::new_v4();
        a.symbol_ref = PrimitiveRef::new(lib, Uuid::new_v4());
        b.symbol_ref = PrimitiveRef::new(lib, Uuid::new_v4());
        let d = diff_revisions(&a, &b);
        assert!(d.symbol_changed);
        assert!(!d.footprint_changed);
        assert_eq!(auto_bump_kind(&d), BumpKind::Major);
    }

    #[test]
    fn diff_detects_footprint_ref_change() {
        let mut a = rev();
        let mut b = rev();
        let lib = Uuid::new_v4();
        a.footprint_ref = Some(PrimitiveRef::new(lib, Uuid::new_v4()));
        b.footprint_ref = Some(PrimitiveRef::new(lib, Uuid::new_v4()));
        let d = diff_revisions(&a, &b);
        assert!(d.footprint_changed);
        assert_eq!(auto_bump_kind(&d), BumpKind::Major);
    }

    #[test]
    fn diff_detects_sim_ref_change() {
        let mut a = rev();
        let mut b = rev();
        let lib = Uuid::new_v4();
        a.sim_ref = None;
        b.sim_ref = Some(PrimitiveRef::new(lib, Uuid::new_v4()));
        let d = diff_revisions(&a, &b);
        assert!(d.sim_changed);
    }

    #[test]
    fn diff_detects_mpn_change() {
        let mut a = rev();
        let mut b = rev();
        a.primary_mpn = ManufacturerPart::draft("Acme", "A");
        b.primary_mpn = ManufacturerPart::draft("Acme", "B");
        let d = diff_revisions(&a, &b);
        assert!(d.mpn_changed);
        // MPN swap with no other change is a *minor* bump.
        assert_eq!(auto_bump_kind(&d), BumpKind::Minor);
    }

    #[test]
    fn diff_detects_pin_map_added_and_changed() {
        let mut a = rev();
        let mut b = rev();
        a.pin_map_overrides.push(PinPadOverride::new("1", "A"));
        b.pin_map_overrides.push(PinPadOverride::new("1", "B"));
        b.pin_map_overrides.push(PinPadOverride::new("EP", "EP1"));
        let d = diff_revisions(&a, &b);
        assert!(d.pin_map_changed);
        assert_eq!(d.pin_map.changed.len(), 1);
        assert_eq!(d.pin_map.added.len(), 1);
        assert_eq!(auto_bump_kind(&d), BumpKind::Major);
    }

    #[test]
    fn diff_detects_parameter_added_removed_changed() {
        let mut a = rev();
        let mut b = rev();
        a.parameters
            .insert("value".into(), ParamValue::Text("10k".into()));
        a.parameters
            .insert("tolerance".into(), ParamValue::Text("1%".into()));
        b.parameters
            .insert("value".into(), ParamValue::Text("10k".into()));
        b.parameters
            .insert("package".into(), ParamValue::Text("0805".into()));
        b.parameters
            .insert("tolerance".into(), ParamValue::Text("0.1%".into()));

        let d = diff_revisions(&a, &b);
        assert!(d.params_changed);
        assert_eq!(d.parameters.added, vec!["package".to_string()]);
        assert!(d.parameters.removed.is_empty());
        assert_eq!(d.parameters.changed.len(), 1);
        assert_eq!(d.parameters.changed[0].0, "tolerance");
    }

    #[test]
    fn diff_detects_supply_added() {
        let mut a = rev();
        let mut b = rev();
        b.supply.push(DistributorListing::new("DigiKey", "DK-1"));
        let d = diff_revisions(&a, &b);
        assert!(d.supply_changed);
        assert_eq!(d.supply_detail.added, vec!["DigiKey:DK-1".to_string()]);
    }

    #[test]
    fn diff_detects_alternates_added() {
        let mut a = rev();
        let mut b = rev();
        b.alternates
            .push(ManufacturerPart::draft("AlternateCorp", "ALT-001"));
        let d = diff_revisions(&a, &b);
        assert!(d.alternates_changed);
    }

    #[test]
    fn diff_detects_datasheet_change() {
        let mut a = rev();
        let mut b = rev();
        a.datasheet = DatasheetRef::url("a.pdf");
        b.datasheet = DatasheetRef::url("b.pdf");
        let d = diff_revisions(&a, &b);
        assert!(d.datasheet_changed);
    }

    #[test]
    fn diff_detects_lifecycle_change() {
        let mut a = rev();
        let mut b = rev();
        a.state = LifecycleState::Released;
        b.state = LifecycleState::Deprecated;
        let d = diff_revisions(&a, &b);
        assert!(d.lifecycle_changed);
        assert_eq!(d.lifecycle_detail.from, Some(LifecycleState::Released));
        assert_eq!(d.lifecycle_detail.to, Some(LifecycleState::Deprecated));
    }

    #[test]
    fn auto_bump_minor_when_only_metadata_changes() {
        let mut a = rev();
        let mut b = rev();
        b.primary_mpn = ManufacturerPart::draft("Acme", "B");
        b.parameters
            .insert("value".into(), ParamValue::Text("10k".into()));
        let d = diff_revisions(&a, &b);
        assert_eq!(auto_bump_kind(&d), BumpKind::Minor);
        // sanity: no binding change
        let _ = (&a, &b);
    }
}
