//! `Command::ReplaceLibSymbol` (#912): swap a sheet's embedded library
//! symbol for the library's current one, refusing when that would drop
//! connectivity.

use crate::*;
use signex_net::{ConnectedPoints, connected_points, point_on_segment, pt_key};
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

/// `"<reference> pin <key>"` for every pin of a placed instance of
/// `fresh.id` that would lose its connection (an old connected pin that
/// `fresh` no longer has at the same spot) or silently gain one (a fresh pin
/// landing on a connection no old pin occupied).
fn connected_pins_changed(doc: &SchematicSheet, fresh: &LibSymbol) -> Vec<String> {
    let Some(old) = doc.lib_symbols.get(&fresh.id) else {
        return Vec::new();
    };
    let gate = connected_points(doc);
    let mut lost = Vec::new();
    for sym in doc.symbols.iter().filter(|s| s.lib_id == fresh.id) {
        let xform = SymbolTransform::from_symbol(sym);
        let on_unit = |p: &&LibPin| p.unit == 0 || p.unit == sym.unit;
        for lp in old.pins.iter().filter(on_unit) {
            let at = xform.apply(lp.pin.position);
            if !has_connection(doc, &gate, &at) {
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
        for np in fresh.pins.iter().filter(on_unit) {
            let at = xform.apply(np.pin.position);
            let was_a_pin = old
                .pins
                .iter()
                .filter(on_unit)
                .any(|lp| pt_key(&xform.apply(lp.pin.position)) == pt_key(&at));
            if !was_a_pin && has_connection(doc, &gate, &at) {
                lost.push(format!("{} pin {}", sym.reference, pin_key(np)));
            }
        }
    }
    lost
}

/// `"<reference> unit <n>"` for every placed instance of `fresh.id` whose
/// unit has neither pins nor graphics in `fresh` (the unit was removed).
fn removed_units(doc: &SchematicSheet, fresh: &LibSymbol) -> Vec<String> {
    doc.symbols
        .iter()
        .filter(|s| s.lib_id == fresh.id && s.unit != 0)
        .filter(|s| {
            !fresh.pins.iter().any(|p| p.unit == 0 || p.unit == s.unit)
                && !fresh
                    .graphics
                    .iter()
                    .any(|g| g.unit == 0 || g.unit == s.unit)
        })
        .map(|s| format!("{} unit {}", s.reference, s.unit))
        .collect()
}

impl Engine {
    pub(super) fn replace_lib_symbol(
        &mut self,
        before: SchematicSheet,
        lib_symbol: LibSymbol,
        library_versions: Vec<(uuid::Uuid, String)>,
    ) -> Result<CommandResult, EngineError> {
        let gone = removed_units(&self.document, &lib_symbol);
        if !gone.is_empty() {
            return Err(EngineError::LibSymbolUnitRemoved {
                lib_id: lib_symbol.id,
                parts: gone,
            });
        }
        let lost = connected_pins_changed(&self.document, &lib_symbol);
        if !lost.is_empty() {
            return Err(EngineError::LibSymbolPinsConnected {
                lib_id: lib_symbol.id,
                pins: lost,
            });
        }
        // `LibSymbol` has no `PartialEq`; `Debug` is exhaustive and ordered.
        let same_copy = self
            .document
            .lib_symbols
            .get(&lib_symbol.id)
            .is_some_and(|old| format!("{old:?}") == format!("{lib_symbol:?}"));
        let mut stamped = false;
        for sym in &mut self.document.symbols {
            if sym.lib_id != lib_symbol.id {
                continue;
            }
            if let Some((_, v)) = library_versions.iter().find(|(u, _)| *u == sym.uuid)
                && sym.library_version != *v
            {
                sym.library_version = v.clone();
                stamped = true;
            }
        }
        if same_copy && !stamped {
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
