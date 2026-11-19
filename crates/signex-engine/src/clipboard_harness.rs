//! What copy, paste and the placement payload do with signal harness objects
//! (#1280): a copied connector carries its entries, a harness wire its two
//! ends, and a paste gives every one of them (entries included) a fresh id.
//! A harness-typed sheet entry stays with its sheet symbol, which the
//! clipboard does not carry.

use signex_types::object_id::IdSource;
use signex_types::schematic::{Aabb, SelectedItem, SelectedKind, SheetHarness};
use uuid::Uuid;

use crate::Command;

/// The harness part of a copy of `items` from `harness`: the selected
/// connectors (whole, entries included) and wires. Entry types are not
/// carried: they belong to sheet entries.
pub(crate) fn copied_harness(harness: &SheetHarness, items: &[SelectedItem]) -> SheetHarness {
    let has = |kind: SelectedKind, id: Uuid| items.iter().any(|i| i.kind == kind && i.uuid == id);
    SheetHarness {
        connectors: harness
            .connectors
            .iter()
            .filter(|c| has(SelectedKind::HarnessConnector, c.uuid))
            .cloned()
            .collect(),
        wires: harness
            .wires
            .iter()
            .filter(|w| has(SelectedKind::HarnessWire, w.uuid))
            .cloned()
            .collect(),
        ..Default::default()
    }
}

/// Shift every harness point by `(dx, dy)`.
pub(crate) fn shift_harness(harness: &mut SheetHarness, dx: f64, dy: f64) {
    harness
        .connectors
        .iter_mut()
        .for_each(|c| c.translate(dx, dy));
    harness.wires.iter_mut().for_each(|w| w.translate(dx, dy));
}

/// Every harness coordinate, for the finiteness check.
pub(crate) fn harness_coordinates(harness: &SheetHarness) -> Vec<f64> {
    let mut out = Vec::new();
    for c in &harness.connectors {
        out.extend([c.position.x, c.position.y, c.width, c.entry_pitch]);
    }
    for w in &harness.wires {
        out.extend([w.start.x, w.start.y, w.end.x, w.end.y]);
    }
    out
}

/// The harness items of a fragment: its connectors and wires.
pub(crate) fn harness_items(harness: &SheetHarness) -> Vec<SelectedItem> {
    harness
        .connectors
        .iter()
        .map(|c| SelectedItem::new(c.uuid, SelectedKind::HarnessConnector))
        .chain(
            harness
                .wires
                .iter()
                .map(|w| SelectedItem::new(w.uuid, SelectedKind::HarnessWire)),
        )
        .collect()
}

/// The extent of the fragment's harness objects.
pub(crate) fn harness_bounds(harness: &SheetHarness) -> Option<Aabb> {
    harness
        .connectors
        .iter()
        .map(|c| c.bounds())
        .chain(harness.wires.iter().map(|w| w.bounds()))
        .reduce(|a, b| a.union(&b))
}

/// One place command per connector and wire, each under fresh ids from
/// `ids` (a connector's entries too).
pub(crate) fn harness_place_commands(
    harness: &SheetHarness,
    ids: &mut dyn IdSource,
) -> Vec<Command> {
    let mut commands = Vec::new();
    for connector in &harness.connectors {
        let mut connector = connector.clone();
        connector.uuid = ids.next_id();
        for entry in &mut connector.entries {
            entry.uuid = ids.next_id();
        }
        commands.push(Command::PlaceHarnessConnector { connector });
    }
    for wire in &harness.wires {
        let mut wire = wire.clone();
        wire.uuid = ids.next_id();
        commands.push(Command::PlaceHarnessWire { wire });
    }
    commands
}

/// Clear the locks of the harness objects and return the ids that held one.
pub(crate) fn take_harness_locks(harness: &mut SheetHarness) -> Vec<Uuid> {
    let mut locked = Vec::new();
    for c in harness.connectors.iter_mut().filter(|c| c.locked) {
        c.locked = false;
        locked.push(c.uuid);
    }
    for w in harness.wires.iter_mut().filter(|w| w.locked) {
        w.locked = false;
        locked.push(w.uuid);
    }
    locked
}

/// Put back the locks [`take_harness_locks`] cleared.
pub(crate) fn restore_harness_locks(
    harness: &mut SheetHarness,
    locked: &std::collections::HashSet<Uuid>,
) {
    for c in &mut harness.connectors {
        c.locked = locked.contains(&c.uuid);
    }
    for w in &mut harness.wires {
        w.locked = locked.contains(&w.uuid);
    }
}

#[cfg(test)]
#[path = "clipboard_harness_tests.rs"]
mod tests;
