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
use super::{Follow, Move, Policy, WireJoin, WirePoint};

/// A point where something joins a selected wire, read before the move.
#[derive(Debug, Clone, Copy)]
pub struct WireContact {
    wire: Uuid,
    /// The wire's start before the move: the contact moves as the wire does.
    wire_start: Point,
    old: Point,
}

impl WireContact {
    /// Where the contact is before the move.
    #[must_use]
    pub fn old_key(&self) -> Key {
        pt_key(&self.old)
    }
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

/// The points the selected wires carry, read with the move applied to
/// `document`: each contact from where it was to where its wire put it. A
/// point that did not move, or that a selected wire still ends on, stays
/// joined and is no contact; the old points in `pin_owned` belong to the pin
/// pass (a pin moving by another delta keeps them).
#[must_use]
pub fn carried_points(
    document: &SchematicSheet,
    contacts: &[WireContact],
    items: &[SelectedItem],
    pin_owned: &HashSet<Key>,
) -> Vec<WirePoint> {
    let selected = selected_of(items);
    let mut starts: HashMap<Uuid, Point> = HashMap::new();
    let mut held: HashSet<Key> = HashSet::new();
    for wire in &document.wires {
        if !selected.contains(&wire.uuid) {
            continue;
        }
        starts.entry(wire.uuid).or_insert(wire.start);
        held.extend([pt_key(&wire.start), pt_key(&wire.end)]);
    }
    let mut points = Vec::new();
    for contact in contacts {
        let Some(start) = starts.get(&contact.wire) else {
            continue;
        };
        let new = Point::new(
            contact.old.x + start.x - contact.wire_start.x,
            contact.old.y + start.y - contact.wire_start.y,
        );
        let old = pt_key(&contact.old);
        if pt_key(&new) == old || held.contains(&old) || pin_owned.contains(&old) {
            continue;
        }
        points.push(WirePoint {
            wire: contact.wire,
            old: contact.old,
            new,
        });
    }
    points
}

/// The selected wires among `points` that cannot be carried whole, read with
/// the move applied to `document`: a point whose carry would make the
/// reconcile dot a contact that had no dot (`carried_point_bails`). Such a
/// wire moves plainly at every point; carrying only its other points could
/// reroute a wire both share and part a net the plain move keeps. Judged with
/// the bends `policy` asks for (its drag wiring mode), as the carry makes them.
#[must_use]
pub fn bailing_wires(
    document: &SchematicSheet,
    points: &[WirePoint],
    items: &[SelectedItem],
    policy: &Policy,
) -> Vec<Uuid> {
    if points.is_empty() {
        return Vec::new();
    }
    let moves: Vec<Move> = points.iter().map(WirePoint::as_move).collect();
    let env = carry_env(document, &moves, selected_of(items), policy.clone());
    let mut out: Vec<Uuid> = bailing(&env, &moves).into_iter().collect();
    out.sort_unstable();
    out
}

/// Carry `points` (none of a bailing wire) by the rules of the module docs,
/// with the move applied to `document`, under the guard's `policy`.
pub fn carry_points(
    document: &mut SchematicSheet,
    points: &[WirePoint],
    items: &[SelectedItem],
    policy: &Policy,
) -> Follow {
    let moves: Vec<Move> = points.iter().map(WirePoint::as_move).collect();
    carry(document, &moves, selected_of(items), policy)
}

/// Carry the contacts of the selected wires after the move has been applied
/// to `document`, wires that cannot be carried whole moving plainly: the
/// unguarded pass of a grip drag's square wires.
pub fn follow_moved_wires(
    document: &mut SchematicSheet,
    contacts: &[WireContact],
    items: &[SelectedItem],
    pin_owned: &HashSet<Key>,
) -> Follow {
    let points = carried_points(document, contacts, items, pin_owned);
    // A grip drag has no wiring mode, by design: the automatic bends.
    let policy = Policy::default();
    let bailed = bailing_wires(document, &points, items, &policy);
    let live: Vec<WirePoint> = (points.into_iter())
        .filter(|point| !bailed.contains(&point.wire))
        .collect();
    let mut out = carry_points(document, &live, items, &policy);
    out.bailed = bailed;
    out
}

/// What a carried point that ends plain held before the move, named for the
/// user ("R7 pin 1", "label VCC", "a wire at (10, 20)"); `None` when nothing
/// joined the dragged wire there. `moving` are the selected items, whose pins
/// and wires moved too.
#[must_use]
pub fn held_at(before: &SchematicSheet, at: Point, moving: &HashSet<Uuid>) -> Option<String> {
    let k = pt_key(&at);
    for symbol in before.symbols.iter().filter(|s| !moving.contains(&s.uuid)) {
        let item = SelectedItem::new(symbol.uuid, SelectedKind::Symbol);
        let tips = super::moved_pin_tips(before, &[item]);
        if let Some(tip) = tips.iter().find(|tip| tip.old_key() == k) {
            return Some(tip.label().to_string());
        }
    }
    let sheet_pin = (before.child_sheets.iter())
        .filter(|c| !moving.contains(&c.uuid))
        .flat_map(|c| c.pins.iter().map(move |p| (c, p)))
        .find(|(_, p)| !moving.contains(&p.uuid) && pt_key(&p.position) == k);
    if let Some((child, pin)) = sheet_pin {
        return Some(format!("{} pin {}", child.name, pin.name));
    }
    let label =
        (before.labels.iter()).find(|l| !moving.contains(&l.uuid) && pt_key(&l.position) == k);
    if let Some(label) = label {
        return Some(format!("label {}", label.text));
    }
    let dotted = before.junctions.iter().any(|j| pt_key(&j.position) == k);
    let wired = (before.wires.iter())
        .filter(|w| !moving.contains(&w.uuid))
        .any(|w| {
            let (a, b) = (pt_key(&w.start), pt_key(&w.end));
            a == k || b == k || (dotted && point_on_segment(k, a, b))
        });
    wired.then(|| format!("a wire at ({}, {})", at.x, at.y))
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
/// end itself. Without `squares` every attached wire follows the end.
pub fn follow_wire_grip(
    document: &mut SchematicSheet,
    wire: Uuid,
    start: bool,
    old: Point,
    squares: bool,
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
    let square: Vec<SelectedItem> = match wire_axis(old, other).filter(|_| squares) {
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
        let mut originals: Vec<(Uuid, Point, Point)> = Vec::new();
        for w in &mut document.wires {
            if !ids.contains(&w.uuid) {
                continue;
            }
            originals.push((w.uuid, w.start, w.end));
            w.start = Point::new(w.start.x + dx, w.start.y + dy);
            w.end = Point::new(w.end.x + dx, w.end.y + dy);
        }
        let mut items = square.clone();
        items.push(SelectedItem::new(wire, SelectedKind::Wire));
        let carried = follow_moved_wires(document, &contacts, &items, &HashSet::new());
        // A square wire the carry could not carry whole is not shifted: it goes
        // back where it was and follows the end like any attached wire, so its
        // own joins are never parted.
        for w in &mut document.wires {
            if let Some((_, start, end)) =
                (originals.iter()).find(|(id, _, _)| *id == w.uuid && carried.bailed.contains(id))
            {
                w.start = *start;
                w.end = *end;
            }
        }
        let shifted: Vec<SelectedItem> = (square.into_iter())
            .filter(|item| !carried.bailed.contains(&item.uuid))
            .collect();
        out.touched.extend(shifted.iter().copied());
        out.touched.extend(carried.touched);
        selected.extend(shifted.iter().map(|item| item.uuid));
    }
    let moves = [Move {
        old,
        new,
        sheet_pin: false,
        wire: Some(WireJoin::Grip(wire)),
    }];
    // A grip drag has no wiring mode, by design: the automatic bends.
    let grip = carry(document, &moves, selected, &Policy::default());
    out.touched.extend(grip.touched);
    out
}

fn selected_of(items: &[SelectedItem]) -> HashSet<Uuid> {
    items.iter().map(|item| item.uuid).collect()
}

/// The working copy a carry runs on, wires in `selected` left alone.
fn carry_env(
    document: &SchematicSheet,
    moves: &[Move],
    selected: HashSet<Uuid>,
    policy: Policy,
) -> Env {
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
    Env::new(document, selected, moving, old_tips, policy)
}

/// The wires of `moves` one of whose points cannot be carried without making
/// a T dot (`carried_point_bails`). Decided before any edit, while the
/// attachments are the sheet's own.
fn bailing(env: &Env, moves: &[Move]) -> HashSet<Uuid> {
    (moves.iter())
        .filter(|mv| env.carried_point_bails(pt_key(&mv.old)))
        .filter_map(|mv| mv.wire.map(WireJoin::wire))
        .collect()
}

/// Run the follow pass for `moves`, wires in `selected` left alone. A wire
/// that bails (see [`bailing`]) moves plainly at every point.
fn carry(
    document: &mut SchematicSheet,
    moves: &[Move],
    selected: HashSet<Uuid>,
    policy: &Policy,
) -> Follow {
    if moves.is_empty() {
        return Follow::default();
    }
    let mut env = carry_env(document, moves, selected, policy.clone());
    let mut out = Follow::default();
    let bails = bailing(&env, moves);
    out.bailed = bails.iter().copied().collect();
    out.bailed.sort_unstable();
    let mut by_tip: BTreeMap<Key, Vec<&Move>> = BTreeMap::new();
    for mv in moves {
        if mv.wire.is_some_and(|join| bails.contains(&join.wire())) {
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
