//! The connection rule of a headless part, sheet symbol or sheet entry
//! placement (#2199, decision comment 6769277106): an edit from operands never
//! connects anything the caller did not name, so a new pin tip or entry point
//! that lands on a wire (end or run), a junction, a label, a sheet entry,
//! a no-connect marker or another pin tip, or a bus entry that lands on a bus
//! (end or run), is refused, and the caller wires it afterwards. Rules:
//! `docs/CLI.md`, "Placing parts and hierarchy".
//!
//! The caller applies the placement's real engine commands to a copy of the
//! sheet and passes both, as for [`super::placement_intent`]. What is new is
//! read from the copy: the pin tips of symbols the sheet did not have, and the
//! sheet entries that are new or moved. A body over a wire is not a
//! connection and is not judged here.

use std::collections::HashSet;

use signex_types::schematic::{
    Bus, Point, SchematicSheet, SheetPin, normalize_port_name, parse_bus_name, pin_tips,
};
use uuid::Uuid;

use super::wire_intent::{ExtraItem, ExtraKind};
use super::{point_on_segment, pt_key};
use crate::uf::Key;

/// The items of `before` that a connection point new in `after` lands on, one
/// per item, sorted by kind and identity.
pub fn placement_contacts(before: &SchematicSheet, after: &SchematicSheet) -> Vec<ExtraItem> {
    let mut found = Vec::new();
    for p in new_points(before, after) {
        found.extend(items_at(before, &p));
    }
    found.sort_by(|x, y| (x.kind, &x.identity).cmp(&(y.kind, &y.identity)));
    found.dedup_by(|x, y| x.kind == y.kind && x.identity == y.identity);
    found
}

/// A connection point new in `after`.
struct NewPoint {
    at: Point,
    /// Whether it is a bus-typed sheet entry, which joins a bus it lies on.
    is_bus_entry: bool,
}

/// The pin tips and sheet entries `after` has and `before` does not, one per
/// point: a symbol by uuid, an entry by uuid and position.
fn new_points(before: &SchematicSheet, after: &SchematicSheet) -> Vec<NewPoint> {
    let symbols: HashSet<Uuid> = before.symbols.iter().map(|s| s.uuid).collect();
    let entries: HashSet<(Uuid, Key)> = sheet_entries(before)
        .map(|(sp, _)| (sp.uuid, pt_key(&sp.position)))
        .collect();
    let mut points: Vec<NewPoint> = sheet_entries(after)
        .filter(|(sp, _)| !entries.contains(&(sp.uuid, pt_key(&sp.position))))
        .map(|(sp, _)| NewPoint {
            at: sp.position,
            is_bus_entry: is_bus_entry(sp),
        })
        .collect();
    // Entries come first, so the dedup below keeps a point's bus contact.
    for sym in after.symbols.iter().filter(|s| !symbols.contains(&s.uuid)) {
        if let Some(lib) = after.resolve_lib_symbol(sym) {
            points.extend(pin_tips(&lib, sym).map(|(_, tip)| NewPoint {
                at: tip,
                is_bus_entry: false,
            }));
        }
    }
    let mut seen = HashSet::new();
    points.retain(|p| seen.insert(pt_key(&p.at)));
    points
}

/// Whether the netlist reads `sp` as a bus pin (`project/buses.rs`,
/// `sheet_buses`): its name parses as a bus name.
fn is_bus_entry(sp: &SheetPin) -> bool {
    normalize_port_name(&sp.name).is_some_and(|name| parse_bus_name(name).is_ok())
}

/// Every sheet entry of `sheet` with its `<entry> of <sheet>` identity.
fn sheet_entries(sheet: &SchematicSheet) -> impl Iterator<Item = (&SheetPin, String)> + '_ {
    sheet.child_sheets.iter().flat_map(|cs| {
        cs.pins
            .iter()
            .map(move |sp| (sp, format!("{} of {}", sp.name, cs.name)))
    })
}

/// What sits at `new.at` on `sheet`: what a connection point there would
/// join. A wire or bus is reported at the point, where it is touched. A bus
/// is joined only by a bus entry, at an end or mid-span; a pin tip on a bus
/// connects nothing (D5.4), so it is not reported.
fn items_at(sheet: &SchematicSheet, new: &NewPoint) -> Vec<ExtraItem> {
    let at = new.at;
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
    for (sp, name) in sheet_entries(sheet).filter(|(sp, _)| pt_key(&sp.position) == key) {
        found.push(item(ExtraKind::SheetPin, name, sp.position));
    }
    for n in sheet
        .no_connects
        .iter()
        .filter(|n| pt_key(&n.position) == key)
    {
        found.push(item(ExtraKind::NoConnect, n.uuid.to_string(), n.position));
    }
    if new.is_bus_entry {
        let on_bus = |b: &&Bus| point_on_segment(key, pt_key(&b.start), pt_key(&b.end));
        for b in sheet.buses.iter().filter(on_bus) {
            found.push(item(ExtraKind::Bus, b.uuid.to_string(), at));
        }
    }
    found
}
