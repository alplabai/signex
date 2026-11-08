//! `Command::ReplaceLibSymbol` (#912, #2209): move placed instances to the
//! library's current version of their symbol, refusing when that would drop
//! connectivity.

use super::pin_changes::{instance_pin_changes, shown_pins, unit_missing};
use crate::*;
use signex_net::connected_points;
use signex_types::schematic::{LibSymbol, LinkStamp};
use std::collections::HashSet;
use uuid::Uuid;

/// The instances of `fresh.id` an update moves: all of them, or the parts of
/// the `only` symbols (every unit of a part moves together).
fn targets(doc: &SchematicSheet, fresh: &LibSymbol, only: Option<&[Uuid]>) -> Vec<usize> {
    let of_symbol = |s: &signex_types::schematic::Symbol| s.lib_id == fresh.id;
    let Some(only) = only else {
        return (0..doc.symbols.len())
            .filter(|&i| of_symbol(&doc.symbols[i]))
            .collect();
    };
    let parts = signex_types::schematic::group_into_parts(
        doc.symbols
            .iter()
            .map(signex_types::schematic::Symbol::part_key),
    );
    parts
        .into_iter()
        .filter(|part| part.iter().any(|&i| only.contains(&doc.symbols[i].uuid)))
        .flatten()
        .filter(|&i| of_symbol(&doc.symbols[i]))
        .collect()
}

/// `"<reference> pin <key>"` for every pin of a moved instance that would lose
/// its connection (an old connected pin that `fresh` no longer has at the same
/// spot) or silently gain one (a fresh pin landing on a connection no old pin
/// occupied). Each instance is judged against the body it draws with now.
fn connected_pins_changed(doc: &SchematicSheet, fresh: &LibSymbol, moved: &[usize]) -> Vec<String> {
    let gate = connected_points(doc);
    let mut lost = Vec::new();
    for &i in moved {
        let sym = &doc.symbols[i];
        let old_pins = shown_pins(doc.body_of(sym), sym);
        let new_pins = shown_pins(Some(fresh), sym);
        let changes = instance_pin_changes(doc, &gate, sym, &old_pins, &new_pins);
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

/// `"<reference> unit <n>"` for every moved instance whose unit has neither
/// pins nor graphics in `fresh` (the unit was removed).
fn removed_units(doc: &SchematicSheet, fresh: &LibSymbol, moved: &[usize]) -> Vec<String> {
    moved
        .iter()
        .map(|&i| &doc.symbols[i])
        .filter(|s| unit_missing(s, fresh))
        .map(|s| format!("{} unit {}", s.reference, s.unit))
        .collect()
}

impl Engine {
    pub(super) fn replace_lib_symbol(
        &mut self,
        before: SchematicSheet,
        lib_symbol: LibSymbol,
        library_versions: Vec<(Uuid, LinkStamp)>,
        only: Option<Vec<Uuid>>,
    ) -> Result<CommandResult, EngineError> {
        let moved = targets(&self.document, &lib_symbol, only.as_deref());
        let gone = removed_units(&self.document, &lib_symbol, &moved);
        if !gone.is_empty() {
            return Err(EngineError::LibSymbolUnitRemoved {
                lib_id: lib_symbol.id,
                parts: gone,
            });
        }
        let lost = connected_pins_changed(&self.document, &lib_symbol, &moved);
        if !lost.is_empty() {
            return Err(EngineError::LibSymbolPinsConnected {
                lib_id: lib_symbol.id,
                pins: lost,
            });
        }
        let new_ref = lib_symbol.body_ref();
        let mut changed = false;
        let mut left: HashSet<String> = HashSet::new();
        for &i in &moved {
            let sym = &mut self.document.symbols[i];
            if let Some((_, link)) = library_versions.iter().find(|(u, _)| *u == sym.uuid) {
                changed |= sym.apply_link_stamp(link);
            }
            let draws_fresh = sym.body_key() == new_ref
                || self.document.lib_symbols.get(sym.body_key()) == Some(&lib_symbol);
            if !draws_fresh {
                left.insert(sym.body_key().to_owned());
                sym.body_ref = new_ref.clone();
                changed = true;
            }
        }
        if !changed {
            return Ok(CommandResult::unchanged());
        }
        self.document
            .lib_symbols
            .entry(new_ref)
            .or_insert(lib_symbol);
        // A version nothing draws with any more is dropped; undo restores it
        // from the history snapshot.
        for key in left {
            if !self.document.symbols.iter().any(|s| s.body_key() == key) {
                self.document.lib_symbols.remove(&key);
            }
        }
        let patch_pair = PatchPair {
            semantic: SemanticPatch::SymbolFieldsUpdated,
            document: DocumentPatch::LIB_SYMBOLS | DocumentPatch::SYMBOLS,
        };
        self.record_history(before, patch_pair);
        Ok(CommandResult::changed(patch_pair))
    }
}
