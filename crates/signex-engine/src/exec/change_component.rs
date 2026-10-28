//! `Command::ChangeComponent` (#1260): swap a placed symbol for another
//! library part, and the preview that says what the swap does to wires.

use super::pin_changes::{has_connection, shown_pins, unit_missing};
use crate::*;
use signex_net::{connected_points, pt_key};
use signex_types::schematic::{LibPin, LibSymbol, PinMapping, SymbolTransform, map_pins, pin_key};
use std::collections::HashMap;
use uuid::Uuid;

/// The part a placed symbol is swapped for: its drawing and the library
/// identity and value a fresh placement of it would carry.
#[derive(Debug, Clone)]
pub struct ComponentReplacement {
    pub lib_symbol: LibSymbol,
    pub library_id: Option<Uuid>,
    pub row_id: Option<Uuid>,
    pub library_version: String,
    pub value: String,
}

/// What swapping a placed symbol for another drawing does to its pins.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ComponentChangePreview {
    pub mapping: PinMapping,
    /// Connected old pins no new pin takes over.
    pub unmatched_connected: Vec<String>,
    /// Connected old pins whose new pin sits somewhere else, so the wire
    /// would no longer reach it.
    pub moved_connected: Vec<String>,
    /// New pins landing on a connection no old pin occupied.
    pub gained: Vec<String>,
    /// The placed unit, when the new drawing has nothing for it.
    pub missing_unit: Option<u32>,
}

impl ComponentChangePreview {
    /// Nothing is lost, moved or gained: the wires stay where they are.
    pub fn is_clean(&self) -> bool {
        self.unmatched_connected.is_empty()
            && self.moved_connected.is_empty()
            && self.gained.is_empty()
            && self.missing_unit.is_none()
    }
}

/// What swapping placed symbol `symbol_id` for `replacement` does to its
/// pins, judged on the sheet's existing copy of the drawing when it holds
/// one (placement keeps it too). Empty when the symbol is unknown.
pub fn change_component_preview(
    doc: &SchematicSheet,
    symbol_id: Uuid,
    replacement: &LibSymbol,
) -> ComponentChangePreview {
    let Some(sym) = doc.symbols.iter().find(|s| s.uuid == symbol_id) else {
        return ComponentChangePreview::default();
    };
    let new_drawing = doc.lib_symbols.get(&replacement.id).unwrap_or(replacement);
    let old_pins = shown_pins(doc.lib_symbols.get(&sym.lib_id), sym);
    let new_pins = shown_pins(Some(new_drawing), sym);
    let mapping = map_pins(&old_pins, &new_pins);
    let gate = connected_points(doc);
    let xform = SymbolTransform::from_symbol(sym);
    let tip = |lp: &LibPin| xform.apply(lp.pin.position);
    let new_pin = |key: &str| new_pins.iter().find(|np| pin_key(np) == key);

    let mut preview = ComponentChangePreview::default();
    let mut connected_old: Vec<&str> = Vec::new();
    for old in &old_pins {
        if !has_connection(doc, &gate, &tip(old)) {
            continue;
        }
        let key = pin_key(old);
        connected_old.push(key);
        match mapping.pairs.iter().find(|p| p.old == key) {
            None => preview.unmatched_connected.push(key.to_string()),
            Some(pair) => {
                let stays =
                    new_pin(&pair.new).is_some_and(|np| pt_key(&tip(np)) == pt_key(&tip(old)));
                if !stays {
                    preview.moved_connected.push(key.to_string());
                }
            }
        }
    }
    for np in &new_pins {
        let key = pin_key(np);
        let from_connected_old = mapping
            .pairs
            .iter()
            .any(|p| p.new == key && connected_old.contains(&p.old.as_str()));
        if !from_connected_old && has_connection(doc, &gate, &tip(np)) {
            preview.gained.push(key.to_string());
        }
    }
    preview.missing_unit = unit_missing(sym, new_drawing).then_some(sym.unit);
    preview.mapping = mapping;
    preview
}

impl Engine {
    pub(super) fn change_component(
        &mut self,
        before: SchematicSheet,
        symbol_id: Uuid,
        replacement: ComponentReplacement,
    ) -> Result<CommandResult, EngineError> {
        let Some(sym) = self.document.symbols.iter().find(|s| s.uuid == symbol_id) else {
            return Ok(CommandResult::unchanged());
        };
        let new_id = replacement.lib_symbol.id.clone();
        let new_drawing = self
            .document
            .lib_symbols
            .get(&new_id)
            .unwrap_or(&replacement.lib_symbol);
        let old_pins = shown_pins(self.document.lib_symbols.get(&sym.lib_id), sym);
        let new_pins = shown_pins(Some(new_drawing), sym);
        let mapping = map_pins(&old_pins, &new_pins);
        let pin_uuids: HashMap<String, Uuid> = mapping
            .pairs
            .iter()
            .filter_map(|p| Some((p.new.clone(), *sym.pin_uuids.get(&p.old)?)))
            .collect();
        let registered = self.document.lib_symbols.contains_key(&new_id);
        let unchanged = registered
            && sym.lib_id == new_id
            && sym.library_id == replacement.library_id
            && sym.row_id == replacement.row_id
            && sym.library_version == replacement.library_version
            && sym.value == replacement.value
            && sym.pin_uuids == pin_uuids;
        if unchanged {
            return Ok(CommandResult::unchanged());
        }
        if !registered {
            self.document
                .lib_symbols
                .insert(new_id.clone(), replacement.lib_symbol);
        }
        if let Some(sym) = self
            .document
            .symbols
            .iter_mut()
            .find(|s| s.uuid == symbol_id)
        {
            sym.lib_id = new_id;
            sym.library_id = replacement.library_id;
            sym.row_id = replacement.row_id;
            sym.library_version = replacement.library_version;
            sym.value = replacement.value;
            sym.pin_uuids = pin_uuids;
        }
        let patch_pair = PatchPair {
            semantic: SemanticPatch::SymbolFieldsUpdated,
            document: DocumentPatch::LIB_SYMBOLS | DocumentPatch::SYMBOLS,
        };
        self.record_history(before, patch_pair);
        Ok(CommandResult::changed(patch_pair))
    }
}
