//! Pre-flight for switching a placed symbol to another drawing of its
//! library row (#1230): which connected pins would move.
//!
//! Two drawings of one part share their pin numbers but not necessarily
//! their pin positions. A pin whose end has a wire, junction or label on it
//! and moves would leave that connection dangling and change the netlist
//! without a word, so the switch is refused instead.

use super::replace_lib_symbol::{has_connection, pin_key};
use crate::SchematicSheet;
use signex_net::{connected_points, pt_key};
use signex_types::schematic::{LibPin, LibSymbol, SymbolTransform};

/// `"<pin>"` for every pin of placed symbol `symbol_id` that has a
/// connection on its end under the current drawing and would not have the
/// same pin at the same point under `new_drawing`. Empty when the symbol is
/// unknown, has no embedded drawing yet, or nothing connected would move.
///
/// Pin-to-pin contact is not a connection (ADR-0002 D5.1), so a pin that
/// only touches another symbol's pin is free to move.
pub fn pins_disconnected_by_switch(
    doc: &SchematicSheet,
    symbol_id: uuid::Uuid,
    new_drawing: &LibSymbol,
) -> Vec<String> {
    let Some(sym) = doc.symbols.iter().find(|s| s.uuid == symbol_id) else {
        return Vec::new();
    };
    let Some(old) = doc.lib_symbols.get(&sym.lib_id) else {
        return Vec::new();
    };
    let gate = connected_points(doc);
    let xform = SymbolTransform::from_symbol(sym);
    let on_unit = |p: &&LibPin| p.unit == 0 || p.unit == sym.unit;
    old.pins
        .iter()
        .filter(on_unit)
        .filter(|lp| {
            let at = xform.apply(lp.pin.position);
            has_connection(doc, &gate, &at)
                && !new_drawing.pins.iter().any(|np| {
                    np.unit == lp.unit
                        && pin_key(np) == pin_key(lp)
                        && pt_key(&xform.apply(np.pin.position)) == pt_key(&at)
                })
        })
        .map(|lp| pin_key(lp).to_string())
        .collect()
}
