//! `Command::ReplaceLibSymbol` (#912): swap a sheet's embedded library
//! symbol for the library's current one, refusing when that would drop
//! connectivity.

use super::connected_pins::connected_pins_changed;
use crate::*;
use signex_types::schematic::{LibSymbol, instance_pins};

/// Connected pins that swapping the embedded copy of `fresh.id` would lose
/// or gain, per placed instance, judged on the pins each instance shows.
fn lib_swap_pins_changed(doc: &SchematicSheet, fresh: &LibSymbol) -> Vec<String> {
    // No embedded copy means no known old pins: a placed instance then draws
    // none, so every fresh pin on a connection is a newly gained one.
    let old_lib = doc.lib_symbols.get(&fresh.id);
    let syms = doc
        .symbols
        .iter()
        .filter(|s| s.lib_id == fresh.id)
        .map(|s| {
            let old = old_lib.map_or_else(Vec::new, |l| instance_pins(l, s).collect());
            (s, old, instance_pins(fresh, s).collect())
        });
    connected_pins_changed(doc, syms)
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
        let lost = lib_swap_pins_changed(&self.document, &lib_symbol);
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
