//! Live ERC (#1288): which findings an edit's re-check replaces.
//!
//! A live check runs the whole ERC in the background, as Run ERC does, but
//! only the findings on what the edit touched are taken from it: the objects
//! that came, went or changed, and every object of a net whose contents
//! changed. Every other finding stays as the last check left it, so an edit
//! never brings up findings on parts of the design nobody touched; Run ERC
//! still checks everything. The rule is in `docs/ERC_RULES.md`, "Live ERC".

use std::collections::{BTreeSet, HashMap, HashSet};

use signex_net::uf::Key;
use signex_net::{NetSignature, net_members, pt_key};
use signex_types::schematic::{Point, SchematicSheet};
use uuid::Uuid;

use crate::{LocationSpace, Violation};

/// What an edit changed on one sheet, as a live check reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditScope {
    /// No earlier copy of the sheet to compare with, or a change the scope
    /// cannot place (its library symbols, harness or sheet variables): every
    /// finding of the sheet is re-checked.
    pub whole_sheet: bool,
    /// The objects that came, went or changed, and every object on a net
    /// whose contents changed.
    pub objects: HashSet<Uuid>,
    /// The points those objects sit on, before and after the edit.
    pub points: HashSet<Key>,
    /// The names of the changed nets, before and after: a finding another
    /// sheet files on such a net is re-checked too.
    pub net_names: BTreeSet<String>,
}

impl EditScope {
    /// A scope that re-checks every finding of its sheet.
    #[must_use]
    pub fn whole_sheet() -> Self {
        Self {
            whole_sheet: true,
            ..Self::default()
        }
    }

    /// A scope for a sheet the edit did not change: only findings on the
    /// nets named `names` (changed on another sheet) are re-checked.
    #[must_use]
    pub fn named(names: BTreeSet<String>) -> Self {
        Self {
            net_names: names,
            ..Self::default()
        }
    }

    /// True when nothing on the sheet is re-checked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.whole_sheet
            && self.objects.is_empty()
            && self.points.is_empty()
            && self.net_names.is_empty()
    }

    /// Whether `finding` is on what the edit touched.
    #[must_use]
    pub fn covers(&self, finding: &Violation) -> bool {
        if self.whole_sheet {
            return true;
        }
        let own_point = matches!(
            finding.location_space,
            LocationSpace::ThisSheet | LocationSpace::Net
        );
        [finding.primary, finding.peer]
            .iter()
            .flatten()
            .any(|item| self.objects.contains(&item.uuid))
            || (own_point && self.points.contains(&pt_key(&finding.location)))
            || finding
                .net
                .as_ref()
                .is_some_and(|net| !net.name.is_empty() && self.net_names.contains(&net.name))
    }
}

/// What changed from `before` to `after`; `None` before: the whole sheet.
#[must_use]
pub fn edit_scope(before: Option<&SchematicSheet>, after: &SchematicSheet) -> EditScope {
    let Some(before) = before else {
        return EditScope::whole_sheet();
    };
    let library_changed = !before.lib_symbols.shares_storage_with(&after.lib_symbols)
        && *before.lib_symbols != *after.lib_symbols;
    let unplaceable =
        library_changed || before.harness != after.harness || before.variables != after.variables;
    if unplaceable {
        return EditScope::whole_sheet();
    }
    let mut scope = EditScope::default();
    changed_objects(before, after, &mut scope);
    changed_objects(after, before, &mut scope);
    changed_nets(before, after, &mut scope);
    scope
}

/// `cached` with the findings `scope` covers replaced by the ones of `fresh`
/// it covers, in the engine's report order.
#[must_use]
pub fn merge_live(cached: &[Violation], fresh: &[Violation], scope: &EditScope) -> Vec<Violation> {
    let merged: Vec<Violation> = cached
        .iter()
        .filter(|v| !scope.covers(v))
        .chain(fresh.iter().filter(|v| scope.covers(v)))
        .cloned()
        .collect();
    // No override changes a severity here; the call only sorts.
    crate::apply_overrides(merged, &HashMap::new())
}

/// Every object of `from` that `to` lacks or holds changed, with its points.
fn changed_objects(from: &SchematicSheet, to: &SchematicSheet, scope: &mut EditScope) {
    let mut found = Vec::new();
    found.extend(differing(
        &from.wires,
        &to.wires,
        |w| w.uuid,
        |w| vec![w.start, w.end],
    ));
    found.extend(differing(
        &from.buses,
        &to.buses,
        |b| b.uuid,
        |b| vec![b.start, b.end],
    ));
    found.extend(differing(
        &from.symbols,
        &to.symbols,
        |s| s.uuid,
        |s| vec![s.position],
    ));
    found.extend(differing(
        &from.labels,
        &to.labels,
        |l| l.uuid,
        |l| vec![l.position],
    ));
    found.extend(differing(
        &from.junctions,
        &to.junctions,
        |j| j.uuid,
        |j| vec![j.position],
    ));
    found.extend(differing(
        &from.no_connects,
        &to.no_connects,
        |n| n.uuid,
        |n| vec![n.position],
    ));
    found.extend(differing(
        &from.bus_entries,
        &to.bus_entries,
        |e| e.uuid,
        |e| vec![e.position],
    ));
    found.extend(differing(
        &from.no_erc_directives,
        &to.no_erc_directives,
        |d| d.uuid,
        |d| vec![d.position],
    ));
    found.extend(differing(
        &from.directives,
        &to.directives,
        |d| d.uuid(),
        |_| Vec::new(),
    ));
    // A changed sheet symbol takes its entries with it.
    for child in &from.child_sheets {
        if to.child_sheets.iter().any(|other| other == child) {
            continue;
        }
        found.push((child.uuid, child.pins.iter().map(|p| p.position).collect()));
        scope.objects.extend(child.pins.iter().map(|p| p.uuid));
    }
    for (uuid, points) in found {
        scope.objects.insert(uuid);
        scope.points.extend(points.iter().map(pt_key));
    }
}

/// `(id, points)` of every item of `from` that `to` lacks or holds changed.
fn differing<T: PartialEq>(
    from: &[T],
    to: &[T],
    id: impl Fn(&T) -> Uuid,
    points: impl Fn(&T) -> Vec<Point>,
) -> Vec<(Uuid, Vec<Point>)> {
    let others: HashMap<Uuid, &T> = to.iter().map(|item| (id(item), item)).collect();
    from.iter()
        .filter(|item| others.get(&id(item)) != Some(item))
        .map(|item| (id(item), points(item)))
        .collect()
}

/// Every object and wire end of a net whose contents differ between the two
/// sheets, and its name.
fn changed_nets(before: &SchematicSheet, after: &SchematicSheet, scope: &mut EditScope) {
    let old: BTreeSet<_> = net_members(before).into_iter().collect();
    let new: BTreeSet<_> = net_members(after).into_iter().collect();
    let changed = old
        .difference(&new)
        .map(|net| (net, before))
        .chain(new.difference(&old).map(|net| (net, after)));
    for ((signature, wires), sheet) in changed {
        note_net(signature, scope);
        for wire in sheet.wires.iter().filter(|w| wires.contains(&w.uuid)) {
            scope.objects.insert(wire.uuid);
            scope.points.insert(pt_key(&wire.start));
            scope.points.insert(pt_key(&wire.end));
        }
    }
}

fn note_net(net: &NetSignature, scope: &mut EditScope) {
    scope
        .objects
        .extend(net.terminals.iter().map(|(symbol, _)| *symbol));
    scope.objects.extend(net.labels.iter().copied());
    scope.objects.extend(net.sheet_pins.iter().copied());
    if !net.name.is_empty() {
        scope.net_names.insert(net.name.clone());
    }
}

#[cfg(test)]
mod tests;
