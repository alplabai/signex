//! `Command::ReplaceLibSymbol` (#912): swap a sheet's embedded library
//! symbol for the library's current one, refusing when that would drop
//! connectivity.

use crate::*;
use signex_net::{point_on_segment, pt_key};
use signex_types::schematic::{LibPin, LibSymbol, Point, SymbolTransform};

/// A pin's identity across two versions of one symbol: its number, else
/// its name (the same rule the netlist uses for terminals).
fn pin_key(lp: &LibPin) -> &str {
    if lp.pin.number.is_empty() {
        &lp.pin.name
    } else {
        &lp.pin.number
    }
}

/// Whether a wire (endpoint or passing through) or a no-connect sits on `at`.
fn has_connection(doc: &SchematicSheet, at: &Point) -> bool {
    let key = pt_key(at);
    doc.no_connects.iter().any(|n| pt_key(&n.position) == key)
        || doc
            .wires
            .iter()
            .any(|w| point_on_segment(key, pt_key(&w.start), pt_key(&w.end)))
}

/// `"<reference> pin <key>"` for every connected pin of a placed instance
/// of `fresh.id` that `fresh` no longer has at the same spot.
fn connected_pins_lost(doc: &SchematicSheet, fresh: &LibSymbol) -> Vec<String> {
    let Some(old) = doc.lib_symbols.get(&fresh.id) else {
        return Vec::new();
    };
    let mut lost = Vec::new();
    for sym in doc.symbols.iter().filter(|s| s.lib_id == fresh.id) {
        let xform = SymbolTransform::from_symbol(sym);
        for lp in old
            .pins
            .iter()
            .filter(|p| p.unit == 0 || p.unit == sym.unit)
        {
            let at = xform.apply(lp.pin.position);
            if !has_connection(doc, &at) {
                continue;
            }
            let kept = fresh.pins.iter().any(|np| {
                np.unit == lp.unit
                    && pin_key(np) == pin_key(lp)
                    && pt_key(&xform.apply(np.pin.position)) == pt_key(&at)
            });
            if !kept {
                lost.push(format!("{} pin {}", sym.reference, pin_key(lp)));
            }
        }
    }
    lost
}

impl Engine {
    pub(super) fn replace_lib_symbol(
        &mut self,
        before: SchematicSheet,
        lib_symbol: LibSymbol,
    ) -> Result<CommandResult, EngineError> {
        let lost = connected_pins_lost(&self.document, &lib_symbol);
        if !lost.is_empty() {
            return Err(EngineError::LibSymbolPinsConnected {
                lib_id: lib_symbol.id,
                pins: lost,
            });
        }
        // `LibSymbol` has no `PartialEq`; `Debug` is exhaustive and ordered.
        if self
            .document
            .lib_symbols
            .get(&lib_symbol.id)
            .is_some_and(|old| format!("{old:?}") == format!("{lib_symbol:?}"))
        {
            return Ok(CommandResult::unchanged());
        }
        self.document
            .lib_symbols
            .insert(lib_symbol.id.clone(), lib_symbol);
        let patch_pair = PatchPair {
            semantic: SemanticPatch::SymbolFieldsUpdated,
            document: DocumentPatch::LIB_SYMBOLS | DocumentPatch::SYMBOLS,
        };
        self.record_history(before, patch_pair);
        Ok(CommandResult::changed(patch_pair))
    }
}
