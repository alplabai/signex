//! Merging the placed units of one multi-unit part into the single entry the
//! BOM lists for it (#691).
//!
//! Every unit of a part is a separate placed symbol, and a user may have typed
//! the MPN or the DNP flag on any of them. The part takes what any unit sets;
//! where two units set different non-empty values for one field the first
//! unit's value is used and the field is reported so the user can pick.

use std::collections::{BTreeMap, BTreeSet};

use signex_types::schematic::Symbol;

/// What the units of one part agree on, in the order they were placed.
pub(super) struct MergedUnits {
    /// Union of the units' `fields`, first non-empty value winning.
    pub fields: BTreeMap<String, String>,
    pub value: String,
    pub footprint: String,
    /// Names of the fields two units disagree on, sorted.
    pub conflicts: Vec<String>,
}

/// Merge `units` (never empty). Fields for which `is_ignored` is true are left
/// out entirely: the fit-state fields are resolved per unit by the caller, not
/// carried as part identity.
pub(super) fn merge_units(units: &[&Symbol], is_ignored: impl Fn(&str) -> bool) -> MergedUnits {
    let mut fields: BTreeMap<String, String> = BTreeMap::new();
    let mut value = String::new();
    let mut footprint = String::new();
    let mut conflicts: BTreeSet<String> = BTreeSet::new();

    for unit in units {
        absorb("Value", &unit.value, &mut value, &mut conflicts);
        absorb("Footprint", &unit.footprint, &mut footprint, &mut conflicts);
        for (key, candidate) in &unit.fields {
            if is_ignored(key) {
                continue;
            }
            let slot = fields.entry(key.clone()).or_default();
            absorb(key, candidate, slot, &mut conflicts);
        }
    }
    // Inclusion flags: the part is listed if ANY unit includes it (see the
    // caller), so a lone excluded unit can never drop a physical part from the
    // BOM. Units that disagree are reported.
    let disagree = |flag: fn(&Symbol) -> bool| units.iter().any(|u| flag(u) != flag(units[0]));
    if disagree(|u| u.in_bom) {
        conflicts.insert("In BOM".to_string());
    }
    if disagree(|u| u.on_board) {
        conflicts.insert("On board".to_string());
    }
    MergedUnits {
        fields,
        value,
        footprint,
        conflicts: conflicts.into_iter().collect(),
    }
}

/// Fold one unit's `candidate` into `slot`: an empty candidate adds nothing, an
/// empty slot takes it, and a different non-empty one is a conflict.
fn absorb(name: &str, candidate: &str, slot: &mut String, conflicts: &mut BTreeSet<String>) {
    let candidate = candidate.trim();
    if candidate.is_empty() {
        return;
    }
    if slot.trim().is_empty() {
        *slot = candidate.to_string();
    } else if slot.trim() != candidate {
        conflicts.insert(name.to_string());
    }
}

/// A fit state for the whole part from its units': not fitted if any unit says
/// so (a DNP unit makes the part DNP), else fitted if any unit says so.
pub(super) fn merge_fit_states(states: impl IntoIterator<Item = Option<bool>>) -> Option<bool> {
    let mut merged = None;
    for state in states {
        match state {
            Some(false) => return Some(false),
            Some(true) => merged = Some(true),
            None => {}
        }
    }
    merged
}
