//! Renames that would put a designator on two parts (AV-11, #2533): such a
//! row is flagged on itself and starts unticked. It is never blocked: the
//! user may still tick it, and the shared-designator note then says so
//! (decided on #1312, 2026-11-11).

use std::collections::{BTreeMap, HashMap};

use signex_engine::pcb::eco::{BackChange, BackChangeList, BackKey};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

/// A part, by its placement and units.
pub type PartKey = (InstancePath, Vec<Uuid>);

/// Why a rename row is flagged: its new designator and a part that holds
/// that designator after the rows that apply now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateRename {
    pub designator: String,
    pub holder: PartKey,
}

/// Every open rename row whose new designator another part holds once the
/// rows that apply now are carried out, ticked or not.
pub fn duplicate_renames(list: &BackChangeList) -> BTreeMap<BackKey, DuplicateRename> {
    let renamed: HashMap<(&InstancePath, &[Uuid]), &str> = list
        .rows
        .iter()
        .filter(|r| r.applies())
        .filter_map(|r| match &r.change {
            BackChange::Designator {
                sheet, units, to, ..
            } => Some(((sheet, units.as_slice()), to.as_str())),
            _ => None,
        })
        .collect();
    let mut holders: HashMap<&str, Vec<(&InstancePath, &[Uuid])>> = HashMap::new();
    for (sheet, units, designator) in &list.designators {
        let key = (sheet, units.as_slice());
        let now = renamed.get(&key).copied().unwrap_or(designator.as_str());
        holders.entry(now).or_default().push(key);
    }
    let mut flagged = BTreeMap::new();
    for row in list.rows.iter().filter(|r| r.blocked.is_none()) {
        let BackChange::Designator {
            sheet, units, to, ..
        } = &row.change
        else {
            continue;
        };
        let own = (sheet, units.as_slice());
        let other = holders
            .get(to.as_str())
            .and_then(|parts| parts.iter().find(|part| **part != own));
        if let Some((holder_sheet, holder_units)) = other {
            flagged.insert(
                row.key,
                DuplicateRename {
                    designator: to.clone(),
                    holder: (InstancePath::clone(holder_sheet), holder_units.to_vec()),
                },
            );
        }
    }
    flagged
}

/// Untick every flagged rename that the user has not chosen yet: a row the
/// list before a Refresh (`previous`) held with the same change keeps its
/// tick. Unticking one rename can flag another (a part that now keeps its
/// designator), so this runs until nothing more is unticked.
pub fn untick_duplicate_renames(list: &mut BackChangeList, previous: Option<&BackChangeList>) {
    let chosen = |key: &BackKey, list: &BackChangeList| {
        previous.is_some_and(|old| {
            old.row(key)
                .zip(list.row(key))
                .is_some_and(|(old, now)| old.change == now.change)
        })
    };
    loop {
        let flagged: Vec<BackKey> = duplicate_renames(list)
            .into_keys()
            .filter(|key| list.row(key).is_some_and(|r| r.on) && !chosen(key, list))
            .collect();
        if flagged.is_empty() {
            return;
        }
        for key in flagged {
            list.set_on(&key, false);
        }
    }
}

#[cfg(test)]
#[path = "duplicates_tests.rs"]
mod tests;
