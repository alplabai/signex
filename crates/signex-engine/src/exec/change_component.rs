//! `Command::ChangeComponent` (#1260): swap a placed symbol for another
//! library part, and the preview that says what the swap does to wires.

use super::pin_changes::{has_connection, shown_pins, unit_missing};
use crate::*;
use signex_net::{connected_points, pt_key};
use signex_types::schematic::{
    LibPin, LibSymbol, PinMapping, Symbol, SymbolTransform, map_pins, pin_key,
};
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
    /// The part's own footprint, datasheet and identity fields. They
    /// replace the placed part's: the old part's footprint is not the new
    /// part's. An empty value clears the field.
    pub footprint: String,
    pub datasheet: String,
    pub fields: Vec<(String, String)>,
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
    /// Part-owned data that changes ("Value", "Footprint", an identity field).
    pub replaced: Vec<String>,
    /// What stays: the designator and parameters the new part lacks.
    pub kept: Vec<String>,
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

/// The placed units of the part `symbol_id` belongs to on this sheet: it and
/// every other symbol with the same designator and drawing. A part without a
/// designator is its own only unit.
fn placed_units(doc: &SchematicSheet, symbol_id: Uuid) -> Vec<&Symbol> {
    let Some(target) = doc.symbols.iter().find(|s| s.uuid == symbol_id) else {
        return Vec::new();
    };
    let reference = target.reference.trim();
    doc.symbols
        .iter()
        .filter(|s| {
            s.uuid == symbol_id
                || (!reference.is_empty()
                    && s.reference.trim() == reference
                    && s.lib_id == target.lib_id)
        })
        .collect()
}

fn push_new(list: &mut Vec<String>, key: String) {
    if !list.contains(&key) {
        list.push(key);
    }
}

/// The pins of one placed unit judged against the replacement drawing.
fn unit_preview(
    doc: &SchematicSheet,
    gate: &signex_net::ConnectedPoints,
    sym: &Symbol,
    new_drawing: &LibSymbol,
) -> ComponentChangePreview {
    let old_pins = shown_pins(doc.lib_symbols.get(&sym.lib_id), sym);
    let new_pins = shown_pins(Some(new_drawing), sym);
    let mapping = map_pins(&old_pins, &new_pins);
    let xform = SymbolTransform::from_symbol(sym);
    let tip = |lp: &LibPin| xform.apply(lp.pin.position);
    let new_pin = |key: &str| new_pins.iter().find(|np| pin_key(np) == key);

    let mut preview = ComponentChangePreview::default();
    let mut connected_old: Vec<&str> = Vec::new();
    for old in &old_pins {
        if !has_connection(doc, gate, &tip(old)) {
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
        if !from_connected_old && has_connection(doc, gate, &tip(np)) {
            preview.gained.push(key.to_string());
        }
    }
    preview.missing_unit = unit_missing(sym, new_drawing).then_some(sym.unit);
    preview.mapping = mapping;
    preview
}

/// What replacing the part data of `target` with `replacement` changes, and
/// what stays.
fn data_changes(target: &Symbol, replacement: &ComponentReplacement) -> (Vec<String>, Vec<String>) {
    let mut replaced = Vec::new();
    let mut check = |label: &str, differs: bool| {
        if differs {
            replaced.push(label.to_string());
        }
    };
    check("Value", target.value != replacement.value);
    check("Footprint", target.footprint != replacement.footprint);
    check("Datasheet", target.datasheet != replacement.datasheet);
    for (key, value) in &replacement.fields {
        let current = target.fields.get(key).map_or("", String::as_str);
        check(key, current != value);
    }
    let mut kept = vec!["Designator".to_string()];
    let mut parameters: Vec<&String> = target
        .fields
        .keys()
        .filter(|k| !replacement.fields.iter().any(|(key, _)| key == *k))
        .collect();
    parameters.sort();
    kept.extend(parameters.into_iter().cloned());
    (replaced, kept)
}

/// What swapping the part of placed symbol `symbol_id` for `replacement`
/// does to the pins of every placed unit of it, judged on the sheet's
/// existing copy of the drawing when it holds one (placement keeps it too),
/// and which data is replaced or kept. Empty when the symbol is unknown.
pub fn change_component_preview(
    doc: &SchematicSheet,
    symbol_id: Uuid,
    replacement: &ComponentReplacement,
) -> ComponentChangePreview {
    let units = placed_units(doc, symbol_id);
    let Some(target) = units.iter().find(|s| s.uuid == symbol_id) else {
        return ComponentChangePreview::default();
    };
    let new_drawing = doc
        .lib_symbols
        .get(&replacement.lib_symbol.id)
        .unwrap_or(&replacement.lib_symbol);
    let gate = connected_points(doc);
    let mut preview = ComponentChangePreview::default();
    for unit in &units {
        let one = unit_preview(doc, &gate, unit, new_drawing);
        one.unmatched_connected
            .into_iter()
            .for_each(|k| push_new(&mut preview.unmatched_connected, k));
        one.moved_connected
            .into_iter()
            .for_each(|k| push_new(&mut preview.moved_connected, k));
        one.gained
            .into_iter()
            .for_each(|k| push_new(&mut preview.gained, k));
        preview.missing_unit = preview.missing_unit.or(one.missing_unit);
        for pair in one.mapping.pairs {
            if !preview.mapping.pairs.contains(&pair) {
                preview.mapping.pairs.push(pair);
            }
        }
        for key in one.mapping.unmatched {
            push_new(&mut preview.mapping.unmatched, key);
        }
    }
    let paired: Vec<String> = preview
        .mapping
        .pairs
        .iter()
        .map(|p| p.old.clone())
        .collect();
    preview.mapping.unmatched.retain(|k| !paired.contains(k));
    (preview.replaced, preview.kept) = data_changes(target, replacement);
    preview
}

/// `unit_missing` for every placed unit: `"<reference> unit <n>"` each.
fn missing_units(units: &[&Symbol], drawing: &LibSymbol) -> Vec<String> {
    units
        .iter()
        .filter(|s| unit_missing(s, drawing))
        .map(|s| format!("{} unit {}", s.reference, s.unit))
        .collect()
}

/// The pin uuids of one unit after the swap: a shown pin keeps its uuid
/// under the number it maps to, a shown pin with no match loses it, and a
/// pin the unit does not show is carried over as it was.
fn remap_pin_uuids(
    sym: &Symbol,
    old_shown: &[&LibPin],
    mapping: &PinMapping,
) -> HashMap<String, Uuid> {
    let shown: Vec<&str> = old_shown.iter().map(|p| pin_key(p)).collect();
    let mut out: HashMap<String, Uuid> = sym
        .pin_uuids
        .iter()
        .filter(|(key, _)| !shown.contains(&key.as_str()))
        .map(|(key, uuid)| (key.clone(), *uuid))
        .collect();
    for pair in &mapping.pairs {
        if let Some(uuid) = sym.pin_uuids.get(&pair.old) {
            out.insert(pair.new.clone(), *uuid);
        }
    }
    out
}

impl Engine {
    pub(super) fn change_component(
        &mut self,
        before: SchematicSheet,
        symbol_id: Uuid,
        replacement: ComponentReplacement,
    ) -> Result<CommandResult, EngineError> {
        let new_id = replacement.lib_symbol.id.clone();
        let doc = &self.document;
        let units: Vec<Uuid> = placed_units(doc, symbol_id)
            .iter()
            .map(|s| s.uuid)
            .collect();
        if units.is_empty() {
            return Ok(CommandResult::unchanged());
        }
        let registered = doc.lib_symbols.contains_key(&new_id);
        let drawing = doc
            .lib_symbols
            .get(&new_id)
            .unwrap_or(&replacement.lib_symbol);
        let placed = placed_units(doc, symbol_id);
        let gone = missing_units(&placed, drawing);
        if !gone.is_empty() {
            return Err(EngineError::ChangeComponentUnitMissing { parts: gone });
        }
        let mut updated: Vec<Symbol> = Vec::new();
        for sym in &placed {
            let old_pins = shown_pins(doc.lib_symbols.get(&sym.lib_id), sym);
            let new_pins = shown_pins(Some(drawing), sym);
            let mapping = map_pins(&old_pins, &new_pins);
            let mut next = (*sym).clone();
            next.lib_id = new_id.clone();
            next.library_id = replacement.library_id;
            next.row_id = replacement.row_id;
            next.library_version = replacement.library_version.clone();
            next.value = replacement.value.clone();
            next.footprint = replacement.footprint.clone();
            next.datasheet = replacement.datasheet.clone();
            for (key, value) in &replacement.fields {
                if value.is_empty() {
                    next.fields.remove(key);
                } else {
                    next.fields.insert(key.clone(), value.clone());
                }
            }
            next.pin_uuids = remap_pin_uuids(sym, &old_pins, &mapping);
            if next != **sym {
                updated.push(next);
            }
        }
        if registered && updated.is_empty() {
            return Ok(CommandResult::unchanged());
        }
        if !registered {
            self.document
                .lib_symbols
                .insert(new_id, replacement.lib_symbol);
        }
        for next in updated {
            if let Some(slot) = self
                .document
                .symbols
                .iter_mut()
                .find(|s| s.uuid == next.uuid)
            {
                *slot = next;
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
