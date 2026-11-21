//! Two wire ends that meet on a pin end are three connections at one point,
//! so the point gets a junction dot as a T of wires does (Caner 2026-11-21,
//! `docs/DECISIONS.md`). A power port's pin counts as a part's pin does. The
//! wires and the pin already join there, so the dot changes no net.

use std::collections::{HashMap, HashSet};

use signex_net::{pt_key, uf::Key};
use signex_types::object_id::IdSource;
use signex_types::schematic::{Junction, Point, SchematicSheet, SelectedItem, SelectedKind};

use super::pin_junctions::{pin_dot, symbol_pin_tips};
use super::stem_dots::stem_dots_for;

/// Add to `dots` (the pin dots an edit mints) the corner dots of this module
/// and the stem dots of `stem_dots`, one per point.
pub(crate) fn add_meeting_dots(
    sheet: &SchematicSheet,
    items: &[SelectedItem],
    ids: &mut dyn IdSource,
    dots: &mut Vec<Junction>,
) {
    let mut taken: HashSet<Key> = dots.iter().map(|dot| pt_key(&dot.position)).collect();
    let corners = corner_dots_for(sheet, items, ids);
    let stems = stem_dots_for(sheet, items, ids);
    dots.extend(
        (corners.into_iter().chain(stems)).filter(|dot| taken.insert(pt_key(&dot.position))),
    );
}

/// How many wire ends of `sheet` sit on each point.
fn wire_end_counts(sheet: &SchematicSheet) -> HashMap<Key, usize> {
    let mut counts: HashMap<Key, usize> = HashMap::new();
    for wire in &sheet.wires {
        for end in [&wire.start, &wire.end] {
            *counts.entry(pt_key(end)).or_default() += 1;
        }
    }
    counts
}

/// The dots an edit needs where two wire ends now meet on a pin end: at the
/// ends of the `items` wires and the pin ends of the `items` symbols, where
/// no dot stands yet. Called with the edit applied; the ids come from `ids`.
fn corner_dots_for(
    sheet: &SchematicSheet,
    items: &[SelectedItem],
    ids: &mut dyn IdSource,
) -> Vec<Junction> {
    let of_kind = |kind: SelectedKind| -> HashSet<uuid::Uuid> {
        (items.iter())
            .filter(|item| item.kind == kind)
            .map(|item| item.uuid)
            .collect()
    };
    let (wires, symbols) = (of_kind(SelectedKind::Wire), of_kind(SelectedKind::Symbol));
    if wires.is_empty() && symbols.is_empty() {
        return Vec::new();
    }
    let counts = wire_end_counts(sheet);
    let meets = |p: &Point| counts.get(&pt_key(p)).is_some_and(|&n| n >= 2);
    // A moved or placed part's own pin end is a pin there.
    let mut corners: Vec<Point> = (sheet.symbols.iter())
        .filter(|symbol| symbols.contains(&symbol.uuid))
        .flat_map(|symbol| symbol_pin_tips(sheet, symbol))
        .filter(|tip| meets(tip))
        .collect();
    let ends: Vec<Point> = (sheet.wires.iter())
        .filter(|wire| wires.contains(&wire.uuid))
        .flat_map(|wire| [wire.start, wire.end])
        .filter(|end| meets(end))
        .collect();
    if !ends.is_empty() {
        let tips: HashSet<Key> = (sheet.symbols.iter())
            .flat_map(|symbol| symbol_pin_tips(sheet, symbol))
            .map(|tip| pt_key(&tip))
            .collect();
        corners.extend(ends.into_iter().filter(|end| tips.contains(&pt_key(end))));
    }
    let mut taken: HashSet<Key> = (sheet.junctions.iter())
        .map(|j| pt_key(&j.position))
        .collect();
    (corners.into_iter())
        .filter(|at| taken.insert(pt_key(at)))
        .map(|at| pin_dot(at, ids))
        .collect()
}
