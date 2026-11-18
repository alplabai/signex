//! A dragged wire keeps its connections.
//!
//! What joins a selected wire is carried with it by the rubber-band rules of
//! the parent module: its two ends, and every junction dot and child-sheet pin
//! on its span (a pin tip mid-span joins only through a dot there, as in the
//! netlist). Each such point is a [`Move`] from where it was to where the wire
//! put it. Unselected wire ends there follow (stretching, shrinking, bending
//! or folding away at zero length); a pin, label or wire that holds the old
//! place keeps it and gets a new orthogonal path to the wire. A point the
//! selection still holds with a wire end needs nothing. A grip drag of one
//! end ([`follow_wire_grip`]) leaves what the end sat on and only brings the
//! wires attached there.

use std::collections::{BTreeMap, HashMap, HashSet};

use signex_net::{point_on_segment, pt_key, uf::Key};
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind, Wire};
use uuid::Uuid;

use super::env::Env;
use super::{Follow, Move, Policy, WireJoin};

/// A point where something joins a selected wire, read before the move.
#[derive(Debug, Clone, Copy)]
pub struct WireContact {
    wire: Uuid,
    /// The wire's start before the move: the contact moves as the wire does.
    wire_start: Point,
    old: Point,
}

/// The dots and child-sheet pins strictly inside `wire`'s span.
fn span_points(document: &SchematicSheet, wire: &Wire) -> Vec<Point> {
    let (a, b) = (pt_key(&wire.start), pt_key(&wire.end));
    let inside = |p: &Point| {
        let k = pt_key(p);
        k != a && k != b && point_on_segment(k, a, b)
    };
    let mut points: Vec<Point> = Vec::new();
    let dots = document.junctions.iter().map(|j| j.position);
    let sheet_pins = (document.child_sheets.iter())
        .flat_map(|c| c.pins.iter())
        .map(|p| p.position);
    for p in dots.chain(sheet_pins) {
        if inside(&p) && !points.iter().any(|q| pt_key(q) == pt_key(&p)) {
            points.push(p);
        }
    }
    points
}

/// The selected wires of `items`, in sheet order.
fn selected_wires<'a>(document: &'a SchematicSheet, items: &[SelectedItem]) -> Vec<&'a Wire> {
    let ids: HashSet<Uuid> = (items.iter())
        .filter(|item| item.kind == SelectedKind::Wire)
        .map(|item| item.uuid)
        .collect();
    if ids.is_empty() {
        return Vec::new();
    }
    (document.wires.iter())
        .filter(|w| ids.contains(&w.uuid))
        .collect()
}

/// Contacts of the selected wires in `items`, read before they move.
#[must_use]
pub fn moved_wire_contacts(document: &SchematicSheet, items: &[SelectedItem]) -> Vec<WireContact> {
    let mut out = Vec::new();
    for wire in selected_wires(document, items) {
        let contact = |old: Point| WireContact {
            wire: wire.uuid,
            wire_start: wire.start,
            old,
        };
        out.push(contact(wire.start));
        out.push(contact(wire.end));
        out.extend(span_points(document, wire).into_iter().map(contact));
    }
    out
}

/// Where something joins a selected wire of `items` mid-span (a dot or a
/// child-sheet pin): with the two ends, the points a drag of the wire carries.
#[must_use]
pub fn wire_contact_points(document: &SchematicSheet, items: &[SelectedItem]) -> Vec<Point> {
    (selected_wires(document, items).into_iter())
        .flat_map(|wire| span_points(document, wire))
        .collect()
}

/// Carry the contacts of the selected wires after the move has been applied
/// to `document`. See the module docs for the rules.
pub fn follow_moved_wires(
    document: &mut SchematicSheet,
    contacts: &[WireContact],
    items: &[SelectedItem],
) -> Follow {
    let selected: HashSet<Uuid> = items.iter().map(|item| item.uuid).collect();
    let mut starts: HashMap<Uuid, Point> = HashMap::new();
    let mut held: HashSet<Key> = HashSet::new();
    for wire in &document.wires {
        if !selected.contains(&wire.uuid) {
            continue;
        }
        starts.entry(wire.uuid).or_insert(wire.start);
        held.extend([pt_key(&wire.start), pt_key(&wire.end)]);
    }
    let mut moves = Vec::new();
    for contact in contacts {
        let Some(start) = starts.get(&contact.wire) else {
            continue;
        };
        let new = Point::new(
            contact.old.x + start.x - contact.wire_start.x,
            contact.old.y + start.y - contact.wire_start.y,
        );
        let old = pt_key(&contact.old);
        // Unmoved, or a selected wire still ends there: still joined.
        if pt_key(&new) == old || held.contains(&old) {
            continue;
        }
        moves.push(Move {
            old: contact.old,
            new,
            sheet_pin: false,
            wire: Some(WireJoin::Carried(contact.wire)),
        });
    }
    carry(document, &moves, selected)
}

/// Whether the span `a`-`b` runs horizontally (`Some(true)`), vertically
/// (`Some(false)`), or neither.
#[must_use]
pub fn wire_axis(a: Point, b: Point) -> Option<bool> {
    let (ka, kb) = (pt_key(&a), pt_key(&b));
    if ka == kb {
        None
    } else if ka.1 == kb.1 {
        Some(true)
    } else if ka.0 == kb.0 {
        Some(false)
    } else {
        None
    }
}

/// Bring the wires attached at the end of `wire` (its start when `start`) that
/// a grip drag took from `old` to where it is now in `document`. A free wire
/// square to the dragged one there moves sideways with the end, carrying what
/// joins it as a dragged wire does; the other wires attached there follow the
/// end itself.
pub fn follow_wire_grip(
    document: &mut SchematicSheet,
    wire: Uuid,
    start: bool,
    old: Point,
) -> Follow {
    let Some(dragged) = document.wires.iter().find(|w| w.uuid == wire) else {
        return Follow::default();
    };
    let (new, other) = if start {
        (dragged.start, dragged.end)
    } else {
        (dragged.end, dragged.start)
    };
    let k = pt_key(&old);
    if pt_key(&new) == k {
        return Follow::default();
    }
    let square: Vec<SelectedItem> = match wire_axis(old, other) {
        Some(horizontal) => (document.wires.iter())
            .filter(|w| w.uuid != wire && !w.locked)
            .filter(|w| pt_key(&w.start) == k || pt_key(&w.end) == k)
            .filter(|w| wire_axis(w.start, w.end) == Some(!horizontal))
            .map(|w| SelectedItem::new(w.uuid, SelectedKind::Wire))
            .collect(),
        None => Vec::new(),
    };
    let mut out = Follow::default();
    let mut selected: HashSet<Uuid> = HashSet::from([wire]);
    if !square.is_empty() {
        // The old end itself is the grip's to handle, below.
        let contacts: Vec<WireContact> = moved_wire_contacts(document, &square)
            .into_iter()
            .filter(|c| pt_key(&c.old) != k)
            .collect();
        let ids: HashSet<Uuid> = square.iter().map(|item| item.uuid).collect();
        let (dx, dy) = (new.x - old.x, new.y - old.y);
        for w in &mut document.wires {
            if !ids.contains(&w.uuid) {
                continue;
            }
            w.start = Point::new(w.start.x + dx, w.start.y + dy);
            w.end = Point::new(w.end.x + dx, w.end.y + dy);
        }
        out.touched.extend(square.iter().copied());
        let mut items = square;
        items.push(SelectedItem::new(wire, SelectedKind::Wire));
        let carried = follow_moved_wires(document, &contacts, &items);
        out.touched.extend(carried.touched);
        selected.extend(ids);
    }
    let moves = [Move {
        old,
        new,
        sheet_pin: false,
        wire: Some(WireJoin::Grip(wire)),
    }];
    let grip = carry(document, &moves, selected);
    out.touched.extend(grip.touched);
    out
}

/// Run the follow pass for `moves`, wires in `selected` left alone.
fn carry(document: &mut SchematicSheet, moves: &[Move], selected: HashSet<Uuid>) -> Follow {
    if moves.is_empty() {
        return Follow::default();
    }
    let old_tips = moves.iter().map(|mv| pt_key(&mv.old)).collect();
    // The pins of moved parts, child sheets and sheet pins are not what holds
    // a carried point: a group drag can put one where the wire was, and a
    // path to it would join a pin the wire never touched.
    let mut moving = selected.clone();
    for child in &document.child_sheets {
        if selected.contains(&child.uuid) {
            moving.extend(child.pins.iter().map(|pin| pin.uuid));
        }
    }
    let mut env = Env::new(document, selected, moving, old_tips, Policy::default());
    let mut out = Follow::default();
    // A wire one of whose points cannot be carried without making a T dot
    // (`carried_point_bails`) moves plainly at every point: carrying only its
    // other points could reroute a wire both share and part a net the plain
    // move keeps. Decided before any edit, while the attachments are the
    // sheet's own.
    let bailing: HashSet<Uuid> = (moves.iter())
        .filter(|mv| env.carried_point_bails(pt_key(&mv.old)))
        .filter_map(|mv| mv.wire.map(WireJoin::wire))
        .collect();
    let mut by_tip: BTreeMap<Key, Vec<&Move>> = BTreeMap::new();
    for mv in moves {
        if mv.wire.is_some_and(|join| bailing.contains(&join.wire())) {
            continue;
        }
        by_tip.entry(pt_key(&mv.old)).or_default().push(mv);
    }
    let translated = env.translate_whole_wires(&by_tip, &mut out);
    out.edited_tips.extend(translated);
    for (key, group) in &by_tip {
        let before = out.touched.len();
        env.reroute_tip(*key, group, &mut out);
        if out.touched.len() > before {
            out.edited_tips.push(*key);
        }
    }
    env.merge_overlaps(&mut out);
    env.write_back(document);
    out
}
