//! The connected-pin comparison shared by every command that swaps which
//! library pins a placed symbol shows (`ReplaceLibSymbol`, `SetSymbolBodyStyle`).

use crate::*;
use signex_net::{ConnectedPoints, connected_points, point_on_segment, pt_key};
use signex_types::schematic::{LibPin, Point, Symbol, SymbolTransform};

/// A pin's identity across two pin sets of one symbol: its number, else its
/// name (the same rule the netlist uses for terminals).
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
fn has_connection(doc: &SchematicSheet, gate: &ConnectedPoints, at: &Point) -> bool {
    let key = pt_key(at);
    gate.contains(at)
        || doc
            .wires
            .iter()
            .any(|w| point_on_segment(key, pt_key(&w.start), pt_key(&w.end)))
}

/// `"<reference> pin <key>"` for every pin of `syms` that would lose its
/// connection (an `old` connected pin that `new` no longer has at the same
/// spot) or silently gain one (a `new` pin landing on a connection no `old`
/// pin occupied). `old` and `new` list the pins one instance shows before and
/// after; `syms` pairs each instance with them.
pub(super) fn connected_pins_changed<'a>(
    doc: &SchematicSheet,
    syms: impl Iterator<Item = (&'a Symbol, Vec<&'a LibPin>, Vec<&'a LibPin>)>,
) -> Vec<String> {
    let gate = connected_points(doc);
    let mut lost = Vec::new();
    for (sym, old, new) in syms {
        let xform = SymbolTransform::from_symbol(sym);
        for lp in &old {
            let at = xform.apply(lp.pin.position);
            if !has_connection(doc, &gate, &at) {
                continue;
            }
            let kept = new.iter().any(|np| {
                pin_key(np) == pin_key(lp) && pt_key(&xform.apply(np.pin.position)) == pt_key(&at)
            });
            if !kept {
                lost.push(format!("{} pin {}", sym.reference, pin_key(lp)));
            }
        }
        for np in &new {
            let at = xform.apply(np.pin.position);
            let was_a_pin = old
                .iter()
                .any(|lp| pt_key(&xform.apply(lp.pin.position)) == pt_key(&at));
            if !was_a_pin && has_connection(doc, &gate, &at) {
                lost.push(format!("{} pin {}", sym.reference, pin_key(np)));
            }
        }
    }
    lost
}
