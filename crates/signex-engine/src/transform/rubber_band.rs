//! Rubber-band wires: keep connections when a move carries symbols away.
//!
//! A plain move translates the selected items and nothing else, so a wire
//! ending on a moved symbol's pin is left behind and the connection silently
//! breaks (#963). When `Command::MoveSelection::keep_wires` is set, the
//! engine runs this pass inside the same command, so the move and the wire
//! edits are one history step and one patch.
//!
//! Which items drag wires: the pins of moved symbols (power ports included)
//! and the pins of moved child sheets. Labels, junctions, no-connects and
//! other loose items do not drag wires: a label moved off a wire is a
//! deliberate disconnect, not a broken connection.
//!
//! What happens to a wire that is not itself selected and has an endpoint on
//! a moved pin tip (matched in the netlist's own 1 um key space):
//!
//! * both endpoints on moved pins: the whole wire translates by the move;
//! * one endpoint on a moved pin: that endpoint follows the pin. A
//!   horizontal wire stays horizontal at its fixed end and a vertical wire
//!   stays vertical, so when the follow would make it diagonal the wire is
//!   split at a corner: horizontal wire -> corner `(moved.x, fixed.y)`,
//!   vertical wire -> corner `(fixed.x, moved.y)`. The original wire keeps
//!   its uuid and the fixed end; a new wire (same stroke) covers
//!   corner -> pin. A diagonal or zero-length wire just has the end moved;
//! * wires that are part of the selection move as they always did and are
//!   never rerouted here.

use std::collections::HashSet;

use signex_net::{pt_key, uf::Key};
use signex_types::schematic::{
    Point, SchematicSheet, SelectedItem, SelectedKind, SymbolTransform, Wire,
};
use uuid::Uuid;

/// World tips (before the move) of every pin carried by the moved items.
pub fn moved_pin_tips(document: &SchematicSheet, items: &[SelectedItem]) -> HashSet<Key> {
    let mut tips = HashSet::new();
    for item in items {
        match item.kind {
            SelectedKind::Symbol => {
                let Some(symbol) = document.symbols.iter().find(|s| s.uuid == item.uuid) else {
                    continue;
                };
                let Some(lib) = document.lib_symbols.get(&symbol.lib_id) else {
                    continue;
                };
                let xform = SymbolTransform::from_symbol(symbol);
                for lib_pin in &lib.pins {
                    if lib_pin.unit != 0 && lib_pin.unit != symbol.unit {
                        continue;
                    }
                    tips.insert(pt_key(&xform.apply(lib_pin.pin.position)));
                }
            }
            SelectedKind::ChildSheet => {
                if let Some(child) = document.child_sheets.iter().find(|c| c.uuid == item.uuid) {
                    tips.extend(child.pins.iter().map(|pin| pt_key(&pin.position)));
                }
            }
            _ => {}
        }
    }
    tips
}

/// Drag the wires attached to `tips` along with a move of `(dx, dy)` that
/// has already been applied to the items. Returns the wires that were
/// edited or created, as selection items, for patch and junction
/// reconciliation.
pub fn follow_moved_pins(
    document: &mut SchematicSheet,
    tips: &HashSet<Key>,
    items: &[SelectedItem],
    dx: f64,
    dy: f64,
) -> Vec<SelectedItem> {
    if tips.is_empty() {
        return Vec::new();
    }
    let selected: HashSet<Uuid> = items
        .iter()
        .filter(|item| item.kind == SelectedKind::Wire)
        .map(|item| item.uuid)
        .collect();

    let mut touched = Vec::new();
    let mut tails: Vec<Wire> = Vec::new();
    for wire in document
        .wires
        .iter_mut()
        .filter(|w| !selected.contains(&w.uuid))
    {
        let start_on_pin = tips.contains(&pt_key(&wire.start));
        let end_on_pin = tips.contains(&pt_key(&wire.end));
        match (start_on_pin, end_on_pin) {
            (false, false) => continue,
            (true, true) => {
                translate(&mut wire.start, dx, dy);
                translate(&mut wire.end, dx, dy);
            }
            (follow_start, follow_end) => {
                debug_assert!(follow_start != follow_end);
                tails.extend(reroute_end(wire, follow_start, dx, dy));
            }
        }
        touched.push(SelectedItem::new(wire.uuid, SelectedKind::Wire));
    }
    touched.extend(
        tails
            .iter()
            .map(|tail| SelectedItem::new(tail.uuid, SelectedKind::Wire)),
    );
    document.wires.extend(tails);
    touched
}

fn translate(point: &mut Point, dx: f64, dy: f64) {
    point.x += dx;
    point.y += dy;
}

/// Move one end of `wire` (its start when `follow_start`) by `(dx, dy)`,
/// bending through a corner when that would leave an orthogonal wire
/// diagonal. Returns the extra corner -> pin segment when one is needed.
fn reroute_end(wire: &mut Wire, follow_start: bool, dx: f64, dy: f64) -> Option<Wire> {
    let (fixed, moving) = if follow_start {
        (wire.end, wire.start)
    } else {
        (wire.start, wire.end)
    };
    let target = Point::new(moving.x + dx, moving.y + dy);
    let (fixed_key, moving_key) = (pt_key(&fixed), pt_key(&moving));
    let corner = if fixed_key.1 == moving_key.1 && fixed_key.0 != moving_key.0 {
        // Horizontal: keep the fixed end's row, turn at the pin's new column.
        Point::new(target.x, fixed.y)
    } else if fixed_key.0 == moving_key.0 && fixed_key.1 != moving_key.1 {
        // Vertical: keep the fixed end's column, turn at the pin's new row.
        Point::new(fixed.x, target.y)
    } else {
        // Diagonal or zero length: nothing orthogonal to preserve.
        set_end(wire, follow_start, target);
        return None;
    };

    let head_empty = pt_key(&corner) == pt_key(&fixed);
    let tail_empty = pt_key(&corner) == pt_key(&target);
    match (head_empty, tail_empty) {
        // Pure stretch or shrink along the wire: no corner needed.
        (_, true) => {
            set_end(wire, follow_start, target);
            None
        }
        // Wire collapsed onto the fixed end's line: it now runs corner -> pin.
        (true, false) => {
            set_end(wire, follow_start, target);
            None
        }
        (false, false) => {
            set_end(wire, follow_start, corner);
            let (start, end) = if follow_start {
                (target, corner)
            } else {
                (corner, target)
            };
            Some(Wire {
                uuid: Uuid::new_v4(),
                start,
                end,
                stroke_width: wire.stroke_width,
                unknown_columns: wire.unknown_columns.clone(),
            })
        }
    }
}

fn set_end(wire: &mut Wire, start: bool, point: Point) {
    if start {
        wire.start = point;
    } else {
        wire.end = point;
    }
}
