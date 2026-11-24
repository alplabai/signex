//! A dragged or moved harness entry slides along its connector's flat side
//! (`HarnessConnector::slide_entry`, docs/DECISIONS.md, 2026-11-24). With
//! "keep wires" the wires on every entry whose slot changed follow it.

use std::collections::HashMap;

use signex_net::pt_key;
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind};
use uuid::Uuid;

/// The point of every entry on a connector that holds one of the moved
/// `items`, read before the move.
pub(super) fn entry_points(sheet: &SchematicSheet, items: &[SelectedItem]) -> Vec<(Uuid, Point)> {
    let mut out = Vec::new();
    for connector in &sheet.harness.connectors {
        let holds_a_moved_entry = items.iter().any(|item| {
            item.kind == SelectedKind::HarnessEntry && connector.entry_index(item.uuid).is_some()
        });
        if holds_a_moved_entry {
            out.extend(
                (connector.entries.iter().enumerate())
                    .map(|(i, entry)| (entry.uuid, connector.entry_point(i))),
            );
        }
    }
    out
}

/// Bring each wire end that sat on an entry's point in `before` to where
/// that entry is now. Whether a wire changed.
pub(super) fn follow_entry_wires(sheet: &mut SchematicSheet, before: &[(Uuid, Point)]) -> bool {
    let moved: HashMap<(i64, i64), Point> = before
        .iter()
        .filter_map(|(id, old)| {
            let (connector, index) = sheet.harness.entry(*id)?;
            let new = connector.entry_point(index);
            (pt_key(old) != pt_key(&new)).then_some((pt_key(old), new))
        })
        .collect();
    if moved.is_empty() {
        return false;
    }
    let mut changed = false;
    for wire in &mut sheet.wires {
        for end in [&mut wire.start, &mut wire.end] {
            if let Some(new) = moved.get(&pt_key(end)) {
                *end = *new;
                changed = true;
            }
        }
    }
    changed
}
