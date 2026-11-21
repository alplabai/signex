//! A directive that stands on a wire by a stem (`SchDirective::stem_foot`)
//! gets a junction dot at its foot, as a wire T does (Caner 2026-11-21,
//! `docs/DECISIONS.md`). The dot never joins anything the wire did not join
//! already: inside a wire it is minted only where that wire alone passes and
//! no pin or wire end stands; on wire ends only where they already meet two
//! wire ends, or a wire end and a pin end.

use std::collections::HashSet;

use signex_net::{point_on_segment, pt_key, uf::Key};
use signex_types::object_id::IdSource;
use signex_types::schematic::{Junction, Point, SchematicSheet, SelectedItem, SelectedKind};

use super::pin_junctions::{pin_dot, symbol_pin_tips};

/// Where the stems of `sheet`'s directives stand.
pub(crate) fn stem_keys(sheet: &SchematicSheet) -> HashSet<Key> {
    (sheet.directives.iter())
        .filter_map(|d| d.stem_foot())
        .map(|p| pt_key(&p))
        .collect()
}

/// The wires passing strictly through `k` and the wire ends on it.
pub(super) fn wires_at(sheet: &SchematicSheet, k: Key) -> (usize, usize) {
    let (mut through, mut ends) = (0, 0);
    for wire in &sheet.wires {
        let (a, b) = (pt_key(&wire.start), pt_key(&wire.end));
        ends += usize::from(a == k) + usize::from(b == k);
        if a != k && b != k && point_on_segment(k, a, b) {
            through += 1;
        }
    }
    (through, ends)
}

/// The dots an edit needs at the stems of the `items` directives and at the
/// stems standing on the `items` wires, where no dot stands yet. Called with
/// the edit applied; the ids come from `ids`.
pub(crate) fn stem_dots_for(
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
    let (directives, wires) = (
        of_kind(SelectedKind::Directive),
        of_kind(SelectedKind::Wire),
    );
    if directives.is_empty() && wires.is_empty() {
        return Vec::new();
    }
    let touched: Vec<(Key, Key)> = (sheet.wires.iter())
        .filter(|wire| wires.contains(&wire.uuid))
        .map(|wire| (pt_key(&wire.start), pt_key(&wire.end)))
        .collect();
    let feet: Vec<Point> = (sheet.directives.iter())
        .filter_map(|d| d.stem_foot().map(|foot| (d.uuid(), foot)))
        .filter(|(id, foot)| {
            let k = pt_key(foot);
            directives.contains(id) || touched.iter().any(|&(a, b)| point_on_segment(k, a, b))
        })
        .map(|(_, foot)| foot)
        .collect();
    if feet.is_empty() {
        return Vec::new();
    }
    let tips: HashSet<Key> = (sheet.symbols.iter())
        .flat_map(|symbol| symbol_pin_tips(sheet, symbol))
        .map(|tip| pt_key(&tip))
        .collect();
    let mut taken: HashSet<Key> = (sheet.junctions.iter())
        .map(|j| pt_key(&j.position))
        .collect();
    (feet.into_iter())
        .filter(|foot| {
            let k = pt_key(foot);
            let pin = tips.contains(&k);
            match wires_at(sheet, k) {
                (1, 0) => !pin,
                (0, ends) => ends >= 2 || (ends == 1 && pin),
                _ => false,
            }
        })
        .filter(|foot| taken.insert(pt_key(foot)))
        .map(|foot| pin_dot(foot, ids))
        .collect()
}
