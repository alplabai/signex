//! `Command::ReplaceLibSymbol` (#912): swap a sheet's embedded library
//! symbol for the library's current one, refusing when that would drop
//! connectivity.

use super::pin_changes::{instance_pin_changes, unit_missing};
use crate::*;
use signex_net::connected_points;
use signex_types::schematic::{LibPin, LibSymbol};

/// `"<reference> pin <key>"` for every pin of a placed instance of
/// `fresh.id` that would lose its connection (an old connected pin that
/// `fresh` no longer has at the same spot) or silently gain one (a fresh pin
/// landing on a connection no old pin occupied).
fn connected_pins_changed(doc: &SchematicSheet, fresh: &LibSymbol) -> Vec<String> {
    // No embedded copy means no known old pins: a placed instance then draws
    // none, so every fresh pin on a connection is a newly gained one.
    let old_pins: &[LibPin] = doc
        .lib_symbols
        .get(&fresh.id)
        .map_or(&[], |old| old.pins.as_slice());
    let gate = connected_points(doc);
    let mut lost = Vec::new();
    for sym in doc.symbols.iter().filter(|s| s.lib_id == fresh.id) {
        let changes = instance_pin_changes(doc, &gate, sym, old_pins, &fresh.pins);
        lost.extend(
            changes
                .lost
                .iter()
                .chain(changes.gained.iter())
                .map(|key| format!("{} pin {key}", sym.reference)),
        );
    }
    lost
}

/// `"<reference> unit <n>"` for every placed instance of `fresh.id` whose
/// unit has neither pins nor graphics in `fresh` (the unit was removed).
fn removed_units(doc: &SchematicSheet, fresh: &LibSymbol) -> Vec<String> {
    doc.symbols
        .iter()
        .filter(|s| s.lib_id == fresh.id && unit_missing(s, fresh))
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
