//! What replacing the drawing of one placed symbol does to the pins that
//! have something electrical on them. Shared by `ReplaceLibSymbol` ("Update
//! from library") and the switch to another drawing of a row (#1230), so the
//! two cannot disagree about what counts as a connection.

use crate::SchematicSheet;
use signex_net::{ConnectedPoints, connected_points, point_on_segment, pt_key};
use signex_types::schematic::{LibPin, LibSymbol, Point, Symbol, SymbolTransform, instance_pins};

/// A pin's identity across two versions of one symbol: its number, else
/// its name (the same rule the netlist uses for terminals).
pub(super) fn pin_key(lp: &LibPin) -> &str {
    if lp.pin.number.is_empty() {
        &lp.pin.name
    } else {
        &lp.pin.number
    }
}

/// Whether anything electrical sits on `at`: the netlist's own connection
/// set (wire endpoints, junctions, labels, no-connects) or a wire passing
/// through it.
pub(super) fn has_connection(doc: &SchematicSheet, gate: &ConnectedPoints, at: &Point) -> bool {
    let key = pt_key(at);
    gate.contains(at)
        || doc
            .wires
            .iter()
            .any(|w| point_on_segment(key, pt_key(&w.start), pt_key(&w.end)))
}

/// Pin keys of one placed instance whose connection state would change.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PinChanges {
    /// An old connected pin the new drawing no longer has at the same spot.
    pub lost: Vec<String>,
    /// A new pin landing on a connection no old pin occupied.
    pub gained: Vec<String>,
}

/// The pins of `sym` that would lose a connection or silently gain one when
/// the pins it shows go from `old_pins` to `new_pins` (each already filtered
/// to the instance's unit and body style, see `instance_pins`). `gate` is
/// `connected_points(doc)`; no embedded old drawing means no old pins, so
/// every new pin on a connection is a gained one.
pub(super) fn instance_pin_changes(
    doc: &SchematicSheet,
    gate: &ConnectedPoints,
    sym: &Symbol,
    old_pins: &[&LibPin],
    new_pins: &[&LibPin],
) -> PinChanges {
    let xform = SymbolTransform::from_symbol(sym);
    let mut changes = PinChanges::default();
    for lp in old_pins {
        let at = xform.apply(lp.pin.position);
        if !has_connection(doc, gate, &at) {
            continue;
        }
        let kept = new_pins.iter().any(|np| {
            np.unit == lp.unit
                && pin_key(np) == pin_key(lp)
                && pt_key(&xform.apply(np.pin.position)) == pt_key(&at)
        });
        if !kept {
            changes.lost.push(pin_key(lp).to_string());
        }
    }
    for np in new_pins {
        let at = xform.apply(np.pin.position);
        let was_a_pin = old_pins
            .iter()
            .any(|lp| pt_key(&xform.apply(lp.pin.position)) == pt_key(&at));
        if !was_a_pin && has_connection(doc, gate, &at) {
            changes.gained.push(pin_key(np).to_string());
        }
    }
    changes
}

/// The pins `sym` shows from `drawing`, or none without an embedded copy.
pub(super) fn shown_pins<'a>(drawing: Option<&'a LibSymbol>, sym: &'a Symbol) -> Vec<&'a LibPin> {
    drawing.map_or_else(Vec::new, |lib| instance_pins(lib, sym).collect())
}

/// The placed unit of `sym` has neither pins nor graphics in `drawing`.
pub(super) fn unit_missing(sym: &Symbol, drawing: &LibSymbol) -> bool {
    sym.unit != 0
        && !drawing
            .pins
            .iter()
            .any(|p| p.unit == 0 || p.unit == sym.unit)
        && !drawing
            .graphics
            .iter()
            .any(|g| g.unit == 0 || g.unit == sym.unit)
}

/// What stops placed symbol `symbol_id` from taking `new_drawing`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SwitchBlockers {
    /// Pins whose connection would be lost.
    pub lost: Vec<String>,
    /// Pins that would silently gain a connection.
    pub gained: Vec<String>,
    /// The placed unit, when `new_drawing` has nothing for it.
    pub missing_unit: Option<u32>,
}

impl SwitchBlockers {
    pub fn is_empty(&self) -> bool {
        self.lost.is_empty() && self.gained.is_empty() && self.missing_unit.is_none()
    }
}

/// Everything that would break if placed symbol `symbol_id` were re-keyed to
/// `new_drawing`: a missing unit, and connected pins that move away or land
/// on a foreign connection (nets would merge). Empty when the symbol is
/// unknown or nothing breaks.
///
/// Pin-to-pin contact is not a connection (ADR-0002 D5.1), so a pin that
/// only touches another symbol's pin is free to move.
pub fn switch_blockers(
    doc: &SchematicSheet,
    symbol_id: uuid::Uuid,
    new_drawing: &LibSymbol,
) -> SwitchBlockers {
    let Some(sym) = doc.symbols.iter().find(|s| s.uuid == symbol_id) else {
        return SwitchBlockers::default();
    };
    let old_pins = shown_pins(doc.lib_symbols.get(&sym.lib_id), sym);
    let new_pins = shown_pins(Some(new_drawing), sym);
    let gate = connected_points(doc);
    let PinChanges { lost, gained } = instance_pin_changes(doc, &gate, sym, &old_pins, &new_pins);
    SwitchBlockers {
        lost,
        gained,
        missing_unit: unit_missing(sym, new_drawing).then_some(sym.unit),
    }
}
