//! The connection rule of a headless part, sheet symbol or sheet entry
//! placement (#2199, decision comment 6769277106): an edit from operands never
//! connects anything the caller did not name, so a new pin tip or entry point
//! that lands on a wire or bus (end or run), a junction, a label, a sheet
//! entry, a no-connect flag or another pin tip is refused, and the caller wires
//! it afterwards. Rules:
//! `docs/CLI.md`, "Placing parts and hierarchy".
//!
//! The caller applies the placement's real engine commands to a copy of the
//! sheet and passes both, as for [`super::placement_intent`]. What is new is
//! read from the copy: the pin tips of symbols the sheet did not have, and the
//! sheet entries that are new or moved. A body over a wire is not a
//! connection and is not judged here.

use std::collections::HashSet;

use signex_types::schematic::{Point, SchematicSheet, pin_tips};
use uuid::Uuid;

use super::wire_intent::{ExtraItem, ExtraKind};
use super::{point_on_segment, pt_key};
use crate::uf::Key;

/// The items of `before` that a connection point new in `after` lands on, one
/// per item, sorted by kind and identity.
pub fn placement_contacts(before: &SchematicSheet, after: &SchematicSheet) -> Vec<ExtraItem> {
    let mut found = Vec::new();
    for p in new_points(before, after) {
        found.extend(items_at(before, p));
    }
    found.sort_by(|x, y| (x.kind, &x.identity).cmp(&(y.kind, &y.identity)));
    found.dedup_by(|x, y| x.kind == y.kind && x.identity == y.identity);
    found
}

/// The pin tips and sheet entries `after` has and `before` does not, one per
/// point: a symbol by uuid, an entry by uuid and position.
fn new_points(before: &SchematicSheet, after: &SchematicSheet) -> Vec<Point> {
    let symbols: HashSet<Uuid> = before.symbols.iter().map(|s| s.uuid).collect();
    let entries: HashSet<(Uuid, Key)> = sheet_entries(before)
        .map(|(uuid, p, _)| (uuid, pt_key(&p)))
        .collect();
    let mut points: Vec<Point> = sheet_entries(after)
        .filter(|(uuid, p, _)| !entries.contains(&(*uuid, pt_key(p))))
        .map(|(_, p, _)| p)
        .collect();
    for sym in after.symbols.iter().filter(|s| !symbols.contains(&s.uuid)) {
        if let Some(lib) = after.resolve_lib_symbol(sym) {
            points.extend(pin_tips(&lib, sym).map(|(_, tip)| tip));
        }
    }
    let mut seen = HashSet::new();
    points.retain(|p| seen.insert(pt_key(p)));
    points
}

/// Every sheet entry of `sheet`: its uuid, position and `<entry> of <sheet>`.
fn sheet_entries(sheet: &SchematicSheet) -> impl Iterator<Item = (Uuid, Point, String)> + '_ {
    sheet.child_sheets.iter().flat_map(|cs| {
        cs.pins
            .iter()
            .map(move |sp| (sp.uuid, sp.position, format!("{} of {}", sp.name, cs.name)))
    })
}

/// What sits at `at` on `sheet`: what a connection point there would join.
/// A wire is reported at `at`, where it is touched.
fn items_at(sheet: &SchematicSheet, at: Point) -> Vec<ExtraItem> {
    let key = pt_key(&at);
    let item = |kind, identity: String, position| ExtraItem {
        kind,
        identity,
        position,
    };
    let mut found = Vec::new();
    for sym in &sheet.symbols {
        let Some(lib) = sheet.resolve_lib_symbol(sym) else {
            continue;
        };
        for (lp, tip) in pin_tips(&lib, sym).filter(|(_, tip)| pt_key(tip) == key) {
            let pin = if lp.pin.number.is_empty() {
                &lp.pin.name
            } else {
                &lp.pin.number
            };
            found.push(item(
                ExtraKind::Pin,
                format!("{}.{pin}", sym.reference),
                tip,
            ));
        }
    }
    let on = |w: &&signex_types::schematic::Wire| {
        point_on_segment(key, pt_key(&w.start), pt_key(&w.end))
    };
    for w in sheet.wires.iter().filter(on) {
        found.push(item(ExtraKind::Wire, w.uuid.to_string(), at));
    }
    for j in sheet
        .junctions
        .iter()
        .filter(|j| pt_key(&j.position) == key)
    {
        found.push(item(ExtraKind::Junction, j.uuid.to_string(), j.position));
    }
    for l in sheet.labels.iter().filter(|l| pt_key(&l.position) == key) {
        found.push(item(ExtraKind::Label, l.text.clone(), l.position));
    }
    for (_, p, name) in sheet_entries(sheet).filter(|(_, p, _)| pt_key(p) == key) {
        found.push(item(ExtraKind::SheetPin, name, p));
    }
    // A sheet entry on a bus is a bus port: it joins the bus's members.
    for b in sheet
        .buses
        .iter()
        .filter(|b| point_on_segment(key, pt_key(&b.start), pt_key(&b.end)))
    {
        found.push(item(ExtraKind::Bus, b.uuid.to_string(), at));
    }
    // A pin or entry on a no-connect flag would be marked as left open.
    for n in sheet
        .no_connects
        .iter()
        .filter(|n| pt_key(&n.position) == key)
    {
        found.push(item(ExtraKind::NoConnect, n.uuid.to_string(), n.position));
    }
    found
}
