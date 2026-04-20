mod command;
mod error;
mod patch;

use std::path::{Path, PathBuf};

pub use command::{
    AnnotateMode, Command, CommandKind, MirrorAxis, ReorderDirection, SymbolTextField, TextTarget,
};
pub use error::EngineError;
pub use patch::{CommandResult, DocumentPatch, PatchPair, SemanticPatch};
use signex_types::schematic::{
    Bus, Junction, Label, NoConnect, SCHEMATIC_PT_TO_MM, SchematicSheet, SelectedItem,
    SelectedKind, Symbol, TextNote, Wire,
};

const JUNCTION_TOLERANCE_MM: f64 = 0.01;
const MAX_HISTORY_ENTRIES: usize = 100;

#[derive(Debug)]
struct HistoryEntry {
    snapshot: SchematicSheet,
    patch_pair: PatchPair,
}

#[derive(Debug)]
pub struct Engine {
    document: SchematicSheet,
    path: Option<PathBuf>,
    history: Vec<HistoryEntry>,
    redo_stack: Vec<HistoryEntry>,
}

#[derive(Debug, Clone, Default)]
pub struct ClipboardSelection {
    pub wires: Vec<Wire>,
    pub buses: Vec<Bus>,
    pub labels: Vec<Label>,
    pub symbols: Vec<Symbol>,
    pub junctions: Vec<Junction>,
    pub no_connects: Vec<NoConnect>,
    pub text_notes: Vec<TextNote>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectionAnchor {
    pub uuid: uuid::Uuid,
    pub kind: SelectedKind,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone)]
pub struct SelectionDetails {
    pub selected_uuid: uuid::Uuid,
    pub selected_kind: SelectedKind,
    pub info: Vec<(String, String)>,
}

impl Engine {
    pub fn new(document: SchematicSheet) -> Result<Self, EngineError> {
        Self::new_with_path(document, None)
    }

    pub fn new_with_path(
        document: SchematicSheet,
        path: Option<PathBuf>,
    ) -> Result<Self, EngineError> {
        Ok(Self {
            document,
            path,
            history: Vec::new(),
            redo_stack: Vec::new(),
        })
    }

    pub fn open(path: &Path) -> Result<Self, EngineError> {
        let document = kicad_parser::parse_schematic_file(path)
            .map_err(|error| EngineError::OpenFailed(anyhow::Error::msg(error.to_string())))?;

        Ok(Self {
            document,
            path: Some(path.to_path_buf()),
            history: Vec::new(),
            redo_stack: Vec::new(),
        })
    }

    pub fn save(&mut self) -> Result<(), EngineError> {
        let Some(path) = self.path.clone() else {
            return Err(EngineError::MissingPath);
        };

        self.save_as(&path)
    }

    pub fn save_as(&mut self, path: &Path) -> Result<(), EngineError> {
        let content = kicad_writer::write_schematic(&self.document);
        std::fs::write(path, content).map_err(EngineError::SaveFailed)?;
        self.path = Some(path.to_path_buf());
        Ok(())
    }

    pub fn execute(&mut self, cmd: Command) -> Result<CommandResult, EngineError> {
        let before = self.document.clone();

        match cmd {
            Command::ReplaceDocument { document } => {
                self.document = document;

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::DocumentReplaced,
                    document: DocumentPatch::FULL,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::UpdateText { target, value } => {
                let document_patch = match target {
                    TextTarget::Label(_) => DocumentPatch::LABELS,
                    TextTarget::TextNote(_) => DocumentPatch::TEXT_NOTES,
                    TextTarget::SymbolReference(_) | TextTarget::SymbolValue(_) => {
                        DocumentPatch::SYMBOLS
                    }
                };

                let changed = match target {
                    TextTarget::Label(uuid) => self
                        .document
                        .labels
                        .iter_mut()
                        .find(|label| label.uuid == uuid)
                        .map(|label| {
                            if label.text == value {
                                false
                            } else {
                                label.text = value;
                                true
                            }
                        })
                        .unwrap_or(false),
                    TextTarget::TextNote(uuid) => self
                        .document
                        .text_notes
                        .iter_mut()
                        .find(|text_note| text_note.uuid == uuid)
                        .map(|text_note| {
                            if text_note.text == value {
                                false
                            } else {
                                text_note.text = value;
                                true
                            }
                        })
                        .unwrap_or(false),
                    TextTarget::SymbolReference(uuid) => self
                        .document
                        .symbols
                        .iter_mut()
                        .find(|symbol| symbol.uuid == uuid)
                        .map(|symbol| {
                            if symbol.reference == value {
                                false
                            } else {
                                symbol.reference = value;
                                true
                            }
                        })
                        .unwrap_or(false),
                    TextTarget::SymbolValue(uuid) => self
                        .document
                        .symbols
                        .iter_mut()
                        .find(|symbol| symbol.uuid == uuid)
                        .map(|symbol| {
                            if symbol.value == value {
                                false
                            } else {
                                symbol.value = value;
                                true
                            }
                        })
                        .unwrap_or(false),
                };

                if !changed {
                    return Ok(CommandResult::unchanged());
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::TextUpdated,
                    document: document_patch,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::UpdateLabelProps {
                label_id,
                font_size_mm,
                justify,
                rotation_degrees,
            } => {
                let changed = self
                    .document
                    .labels
                    .iter_mut()
                    .find(|l| l.uuid == label_id)
                    .map(|l| {
                        let mut any = false;
                        if let Some(fs) = font_size_mm
                            && (l.font_size - fs).abs() > 1e-6
                        {
                            l.font_size = fs;
                            any = true;
                        }
                        if let Some(j) = justify
                            && l.justify != j
                        {
                            l.justify = j;
                            any = true;
                        }
                        if let Some(r) = rotation_degrees
                            && (l.rotation - r).abs() > 1e-6
                        {
                            l.rotation = r;
                            any = true;
                        }
                        any
                    })
                    .unwrap_or(false);

                if !changed {
                    return Ok(CommandResult::unchanged());
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::LabelsMutated,
                    document: DocumentPatch::LABELS,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::SetSymbolRotation {
                symbol_id,
                rotation_degrees,
            } => {
                let changed = self
                    .document
                    .symbols
                    .iter_mut()
                    .find(|s| s.uuid == symbol_id)
                    .map(|s| {
                        if (s.rotation - rotation_degrees).abs() < 1e-6 {
                            false
                        } else {
                            s.rotation = rotation_degrees;
                            true
                        }
                    })
                    .unwrap_or(false);

                if !changed {
                    return Ok(CommandResult::unchanged());
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SelectionRotated,
                    document: DocumentPatch::SYMBOLS,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::UpdateSymbolTextSize {
                symbol_id,
                field,
                font_size_mm,
            } => {
                use command::SymbolTextField;
                let changed = self
                    .document
                    .symbols
                    .iter_mut()
                    .find(|s| s.uuid == symbol_id)
                    .map(|s| {
                        let tp = match field {
                            SymbolTextField::Reference => s.ref_text.as_mut(),
                            SymbolTextField::Value => s.val_text.as_mut(),
                        };
                        if let Some(tp) = tp
                            && (tp.font_size - font_size_mm).abs() > 1e-6
                        {
                            tp.font_size = font_size_mm;
                            true
                        } else {
                            false
                        }
                    })
                    .unwrap_or(false);

                if !changed {
                    return Ok(CommandResult::unchanged());
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SymbolFieldsUpdated,
                    document: DocumentPatch::SYMBOLS,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::UpdateSymbolLibId { symbol_id, lib_id } => {
                let changed = self
                    .document
                    .symbols
                    .iter_mut()
                    .find(|s| s.uuid == symbol_id)
                    .map(|s| {
                        if s.lib_id == lib_id {
                            false
                        } else {
                            s.lib_id = lib_id;
                            true
                        }
                    })
                    .unwrap_or(false);

                if !changed {
                    return Ok(CommandResult::unchanged());
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SymbolFieldsUpdated,
                    document: DocumentPatch::SYMBOLS,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::UpdateSymbolFootprint {
                symbol_id,
                footprint,
            } => {
                let changed = self
                    .document
                    .symbols
                    .iter_mut()
                    .find(|symbol| symbol.uuid == symbol_id)
                    .map(|symbol| {
                        if symbol.footprint == footprint {
                            false
                        } else {
                            symbol.footprint = footprint;
                            true
                        }
                    })
                    .unwrap_or(false);

                if !changed {
                    return Ok(CommandResult::unchanged());
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SymbolFieldsUpdated,
                    document: DocumentPatch::SYMBOLS,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::SetSymbolField {
                symbol_id,
                key,
                value,
            } => {
                // Reject overwrites of reserved keys — those have their
                // own commands so ref/value/footprint moves stay tracked.
                match key.to_lowercase().as_str() {
                    "reference" | "value" | "footprint" => {
                        return Ok(CommandResult::unchanged());
                    }
                    _ => {}
                }
                let changed = self
                    .document
                    .symbols
                    .iter_mut()
                    .find(|s| s.uuid == symbol_id)
                    .map(|symbol| {
                        if value.is_empty() {
                            symbol.fields.remove(&key).is_some()
                        } else if symbol.fields.get(&key) == Some(&value) {
                            false
                        } else {
                            symbol.fields.insert(key, value);
                            true
                        }
                    })
                    .unwrap_or(false);
                if !changed {
                    return Ok(CommandResult::unchanged());
                }
                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SymbolFieldsUpdated,
                    document: DocumentPatch::SYMBOLS,
                };
                self.record_history(before, patch_pair);
                Ok(CommandResult::changed(patch_pair))
            }
            Command::UpdateSymbolFields {
                symbol_id,
                reference,
                value,
                footprint,
            } => {
                let changed = self
                    .document
                    .symbols
                    .iter_mut()
                    .find(|symbol| symbol.uuid == symbol_id)
                    .map(|symbol| {
                        let changed = symbol.reference != reference
                            || symbol.value != value
                            || symbol.footprint != footprint;

                        if changed {
                            symbol.reference = reference;
                            symbol.value = value;
                            symbol.footprint = footprint;
                        }

                        changed
                    })
                    .unwrap_or(false);

                if !changed {
                    return Ok(CommandResult::unchanged());
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SymbolFieldsUpdated,
                    document: DocumentPatch::SYMBOLS,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::DeleteSelection { items } => {
                let mut changed = false;

                for item in &items {
                    changed |= self.remove_selected_item(item);
                }

                if !changed {
                    return Ok(CommandResult::unchanged());
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SelectionDeleted,
                    document: DocumentPatch::from_selected_items(&items),
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::MoveSelection { items, dx, dy } => {
                let mut changed = false;

                for item in &items {
                    changed |= self.move_selected_item(item, dx, dy);
                }

                if !changed {
                    return Ok(CommandResult::unchanged());
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SelectionMoved,
                    document: DocumentPatch::from_selected_items(&items),
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::RotateSelection {
                items,
                angle_degrees,
            } => {
                let mut changed = false;

                for item in &items {
                    changed |= self.rotate_selected_item(item, angle_degrees);
                }

                if !changed {
                    return Ok(CommandResult::unchanged());
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SelectionRotated,
                    document: DocumentPatch::from_selected_items(&items),
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::MirrorSelection { items, axis } => {
                let mut changed = false;

                for item in &items {
                    changed |= self.mirror_selected_item(item, axis);
                }

                if !changed {
                    return Ok(CommandResult::unchanged());
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SelectionMirrored,
                    document: DocumentPatch::from_selected_items(&items),
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::PlaceBus { bus } => {
                self.document.buses.push(bus);

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: DocumentPatch::BUSES,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::PlaceLabel { label } => {
                self.document.labels.push(label);

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: DocumentPatch::LABELS,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::PlaceSymbol { symbol } => {
                self.document.symbols.push(symbol);

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: DocumentPatch::SYMBOLS,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::PlaceWireSegment { wire } => {
                self.document.wires.push(wire.clone());

                for point in [wire.start, wire.end] {
                    if let Some(junction) =
                        needed_junction(point, &self.document, JUNCTION_TOLERANCE_MM)
                    {
                        self.document.junctions.push(junction);
                    }
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: DocumentPatch::WIRES | DocumentPatch::JUNCTIONS,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::PlaceJunction { junction } => {
                self.document.junctions.push(junction);

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: DocumentPatch::JUNCTIONS,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::PlaceNoConnect { no_connect } => {
                self.document.no_connects.push(no_connect);

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: DocumentPatch::NO_CONNECTS,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::PlaceBusEntry { bus_entry } => {
                self.document.bus_entries.push(bus_entry);

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: DocumentPatch::BUS_ENTRIES,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::PlaceTextNote { text_note } => {
                self.document.text_notes.push(text_note);

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: DocumentPatch::TEXT_NOTES,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::PlaceSchDrawing { drawing } => {
                self.document.drawings.push(drawing);

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: DocumentPatch::DRAWINGS,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::UpdateSchDrawing { drawing } => {
                use signex_types::schematic::SchDrawing;
                let target_uuid = match &drawing {
                    SchDrawing::Line { uuid, .. }
                    | SchDrawing::Rect { uuid, .. }
                    | SchDrawing::Circle { uuid, .. }
                    | SchDrawing::Arc { uuid, .. }
                    | SchDrawing::Polyline { uuid, .. } => *uuid,
                };
                let mut changed = false;
                for d in self.document.drawings.iter_mut() {
                    let u = match d {
                        SchDrawing::Line { uuid, .. }
                        | SchDrawing::Rect { uuid, .. }
                        | SchDrawing::Circle { uuid, .. }
                        | SchDrawing::Arc { uuid, .. }
                        | SchDrawing::Polyline { uuid, .. } => *uuid,
                    };
                    if u == target_uuid {
                        *d = drawing.clone();
                        changed = true;
                        break;
                    }
                }
                if !changed {
                    return Ok(CommandResult::unchanged());
                }
                let patch_pair = PatchPair {
                    semantic: SemanticPatch::TextUpdated,
                    document: DocumentPatch::DRAWINGS,
                };
                self.record_history(before, patch_pair);
                Ok(CommandResult::changed(patch_pair))
            }
            Command::AnnotateAll { mode } => {
                use crate::command::AnnotateMode;
                // Power ports (is_power == true, or reference starting with '#')
                // are KiCad net anchors, not components. Their references
                // carry the net name, not a designator. Skip them in every
                // phase so annotation only touches real parts.
                let is_designator_target = |sym: &signex_types::schematic::Symbol| -> bool {
                    !sym.is_power && !sym.reference.starts_with('#')
                };
                // Phase 1: optionally reset existing numbers back to '?'.
                if matches!(
                    mode,
                    AnnotateMode::ResetOnly | AnnotateMode::ResetAndRenumber
                ) {
                    for symbol in self.document.symbols.iter_mut() {
                        if !is_designator_target(symbol) {
                            continue;
                        }
                        // Keep the alphabetic prefix; drop the digit tail.
                        let prefix: String = symbol
                            .reference
                            .chars()
                            .take_while(|c| c.is_ascii_alphabetic())
                            .collect();
                        if !prefix.is_empty() {
                            symbol.reference = format!("{prefix}?");
                        }
                    }
                }
                if matches!(mode, AnnotateMode::ResetOnly) {
                    let patch_pair = PatchPair {
                        semantic: SemanticPatch::SymbolFieldsUpdated,
                        document: DocumentPatch::SYMBOLS,
                    };
                    self.record_history(before, patch_pair);
                    return Ok(CommandResult::changed(patch_pair));
                }

                // Phase 2: find the max number per prefix from already-annotated
                // symbols so Incremental doesn't collide. Skip power ports
                // — their numbers use a different (#PWR) namespace.
                let mut next_by_prefix: std::collections::HashMap<String, u32> =
                    std::collections::HashMap::new();
                for symbol in &self.document.symbols {
                    if !is_designator_target(symbol) {
                        continue;
                    }
                    let prefix: String = symbol
                        .reference
                        .chars()
                        .take_while(|c| c.is_ascii_alphabetic())
                        .collect();
                    if prefix.is_empty() {
                        continue;
                    }
                    let rest = &symbol.reference[prefix.len()..];
                    if let Ok(n) = rest.parse::<u32>() {
                        let entry = next_by_prefix.entry(prefix).or_insert(0);
                        if n > *entry {
                            *entry = n;
                        }
                    }
                }

                // Phase 3: iterate symbols in a stable order (by position,
                // then uuid) and assign sequential numbers to any '?' tails.
                let mut order: Vec<usize> = (0..self.document.symbols.len()).collect();
                order.sort_by(|a, b| {
                    let sa = &self.document.symbols[*a];
                    let sb = &self.document.symbols[*b];
                    sa.position
                        .y
                        .partial_cmp(&sb.position.y)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then(
                            sa.position
                                .x
                                .partial_cmp(&sb.position.x)
                                .unwrap_or(std::cmp::Ordering::Equal),
                        )
                        .then(sa.uuid.cmp(&sb.uuid))
                });

                for idx in order {
                    let symbol = &mut self.document.symbols[idx];
                    if !is_designator_target(symbol) {
                        continue;
                    }
                    if !symbol.reference.ends_with('?') {
                        continue;
                    }
                    let prefix: String = symbol
                        .reference
                        .chars()
                        .take_while(|c| c.is_ascii_alphabetic())
                        .collect();
                    if prefix.is_empty() {
                        continue;
                    }
                    let next = next_by_prefix.entry(prefix.clone()).or_insert(0);
                    *next += 1;
                    symbol.reference = format!("{prefix}{next}");
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SymbolFieldsUpdated,
                    document: DocumentPatch::SYMBOLS,
                };
                self.record_history(before, patch_pair);
                Ok(CommandResult::changed(patch_pair))
            }
            Command::MoveSymbolAbsolute { symbol_id, x, y } => {
                let Some(symbol) = self
                    .document
                    .symbols
                    .iter_mut()
                    .find(|s| s.uuid == symbol_id)
                else {
                    return Ok(CommandResult::unchanged());
                };
                let dx = x - symbol.position.x;
                let dy = y - symbol.position.y;
                if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
                    return Ok(CommandResult::unchanged());
                }
                symbol.position.x = x;
                symbol.position.y = y;
                if let Some(rt) = symbol.ref_text.as_mut() {
                    rt.position.x += dx;
                    rt.position.y += dy;
                }
                if let Some(vt) = symbol.val_text.as_mut() {
                    vt.position.x += dx;
                    vt.position.y += dy;
                }
                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SelectionMoved,
                    document: DocumentPatch::SYMBOLS,
                };
                self.record_history(before, patch_pair);
                Ok(CommandResult::changed(patch_pair))
            }
            Command::ReorderObjects { items, direction } => {
                use crate::command::ReorderDirection;
                use signex_types::schematic::SelectedKind;
                if items.is_empty() {
                    return Ok(CommandResult::unchanged());
                }

                // Helper: given a Vec and a reference uuid, return the
                // insert position for JustAbove (ref_idx + 1) or
                // JustBelow (ref_idx). Returns None when the reference
                // uuid isn't in the Vec — caller falls back to
                // no-change.
                fn reorder_slot<T>(
                    vec: &[T],
                    uuid_of: impl Fn(&T) -> uuid::Uuid,
                    direction: ReorderDirection,
                ) -> Option<usize> {
                    match direction {
                        ReorderDirection::ToFront => Some(vec.len()),
                        ReorderDirection::ToBack => Some(0),
                        ReorderDirection::JustAbove(target) => {
                            vec.iter().position(|x| uuid_of(x) == target).map(|i| i + 1)
                        }
                        ReorderDirection::JustBelow(target) => {
                            vec.iter().position(|x| uuid_of(x) == target)
                        }
                    }
                }

                let mut changed = false;
                for item in &items {
                    match item.kind {
                        SelectedKind::Symbol => {
                            if let Some(idx) = self
                                .document
                                .symbols
                                .iter()
                                .position(|s| s.uuid == item.uuid)
                            {
                                let sym = self.document.symbols.remove(idx);
                                if let Some(mut slot) =
                                    reorder_slot(&self.document.symbols, |s| s.uuid, direction)
                                {
                                    slot = slot.min(self.document.symbols.len());
                                    self.document.symbols.insert(slot, sym);
                                    changed = true;
                                } else {
                                    // Reference uuid missing — restore item at its
                                    // original slot so we don't drop it silently.
                                    self.document.symbols.insert(idx, sym);
                                }
                            }
                        }
                        SelectedKind::Wire => {
                            if let Some(idx) =
                                self.document.wires.iter().position(|w| w.uuid == item.uuid)
                            {
                                let w = self.document.wires.remove(idx);
                                if let Some(mut slot) =
                                    reorder_slot(&self.document.wires, |x| x.uuid, direction)
                                {
                                    slot = slot.min(self.document.wires.len());
                                    self.document.wires.insert(slot, w);
                                    changed = true;
                                } else {
                                    self.document.wires.insert(idx, w);
                                }
                            }
                        }
                        SelectedKind::Label => {
                            if let Some(idx) = self
                                .document
                                .labels
                                .iter()
                                .position(|l| l.uuid == item.uuid)
                            {
                                let l = self.document.labels.remove(idx);
                                if let Some(mut slot) =
                                    reorder_slot(&self.document.labels, |x| x.uuid, direction)
                                {
                                    slot = slot.min(self.document.labels.len());
                                    self.document.labels.insert(slot, l);
                                    changed = true;
                                } else {
                                    self.document.labels.insert(idx, l);
                                }
                            }
                        }
                        SelectedKind::TextNote => {
                            if let Some(idx) = self
                                .document
                                .text_notes
                                .iter()
                                .position(|t| t.uuid == item.uuid)
                            {
                                let t = self.document.text_notes.remove(idx);
                                if let Some(mut slot) =
                                    reorder_slot(&self.document.text_notes, |x| x.uuid, direction)
                                {
                                    slot = slot.min(self.document.text_notes.len());
                                    self.document.text_notes.insert(slot, t);
                                    changed = true;
                                } else {
                                    self.document.text_notes.insert(idx, t);
                                }
                            }
                        }
                        // Drawings, junctions, NC, bus entries aren't
                        // z-ordered in KiCad schematic (no explicit
                        // stacking). Left out intentionally.
                        _ => {}
                    }
                }
                if !changed {
                    return Ok(CommandResult::unchanged());
                }
                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SelectionMoved,
                    document: DocumentPatch::FULL,
                };
                self.record_history(before, patch_pair);
                Ok(CommandResult::changed(patch_pair))
            }
        }
    }

    /// Is there at least one entry in the undo history? Used by the
    /// context-aware menu to grey out Edit → Undo when nothing's
    /// pending.
    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    /// Is there at least one entry in the redo stack?
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) -> Result<Option<PatchPair>, EngineError> {
        let Some(entry) = self.history.pop() else {
            return Ok(None);
        };

        let patch_pair = entry.patch_pair;
        let redo_snapshot = std::mem::replace(&mut self.document, entry.snapshot);
        self.redo_stack.push(HistoryEntry {
            snapshot: redo_snapshot,
            patch_pair,
        });
        Ok(Some(patch_pair))
    }

    pub fn redo(&mut self) -> Result<Option<PatchPair>, EngineError> {
        let Some(entry) = self.redo_stack.pop() else {
            return Ok(None);
        };

        let patch_pair = entry.patch_pair;
        let undo_snapshot = std::mem::replace(&mut self.document, entry.snapshot);
        self.history.push(HistoryEntry {
            snapshot: undo_snapshot,
            patch_pair,
        });
        Ok(Some(patch_pair))
    }

    pub fn document(&self) -> &SchematicSheet {
        &self.document
    }

    /// Cross-sheet annotation variant. Like `Command::AnnotateAll` but uses
    /// (and updates) an externally-owned per-prefix counter, so every
    /// sheet in a project can share one global numbering pass. Returns
    /// whether anything changed.
    pub fn annotate_with_seed(
        &mut self,
        mode: crate::command::AnnotateMode,
        next_by_prefix: &mut std::collections::HashMap<String, u32>,
    ) -> Result<bool, EngineError> {
        self.annotate_with_seed_and_locks(mode, next_by_prefix, &std::collections::HashSet::new())
    }

    /// Same as `annotate_with_seed`, but skips every symbol whose uuid
    /// appears in `locked`. Used by the Annotate dialog's per-row lock
    /// checkboxes so the user can exclude individual designators from
    /// reannotation (Altium's "Lock" column behaviour).
    pub fn annotate_with_seed_and_locks(
        &mut self,
        mode: crate::command::AnnotateMode,
        next_by_prefix: &mut std::collections::HashMap<String, u32>,
        locked: &std::collections::HashSet<uuid::Uuid>,
    ) -> Result<bool, EngineError> {
        use crate::command::AnnotateMode;
        let before = self.document.clone();
        let is_designator_target = |sym: &signex_types::schematic::Symbol| -> bool {
            !sym.is_power && !sym.reference.starts_with('#') && !locked.contains(&sym.uuid)
        };

        // Phase 1: reset to '?' if requested.
        if matches!(
            mode,
            AnnotateMode::ResetOnly | AnnotateMode::ResetAndRenumber
        ) {
            for symbol in self.document.symbols.iter_mut() {
                if !is_designator_target(symbol) {
                    continue;
                }
                let prefix: String = symbol
                    .reference
                    .chars()
                    .take_while(|c| c.is_ascii_alphabetic())
                    .collect();
                if !prefix.is_empty() {
                    symbol.reference = format!("{prefix}?");
                }
            }
        }
        if matches!(mode, AnnotateMode::ResetOnly) {
            let patch_pair = PatchPair {
                semantic: SemanticPatch::SymbolFieldsUpdated,
                document: DocumentPatch::SYMBOLS,
            };
            self.record_history(before, patch_pair);
            return Ok(true);
        }

        // Phase 2: merge this sheet's existing numbers into the shared map
        // so later sheets don't collide.
        for symbol in &self.document.symbols {
            if !is_designator_target(symbol) {
                continue;
            }
            let prefix: String = symbol
                .reference
                .chars()
                .take_while(|c| c.is_ascii_alphabetic())
                .collect();
            if prefix.is_empty() {
                continue;
            }
            if let Ok(n) = symbol.reference[prefix.len()..].parse::<u32>() {
                let entry = next_by_prefix.entry(prefix).or_insert(0);
                if n > *entry {
                    *entry = n;
                }
            }
        }

        // Phase 3: renumber '?' symbols using the shared counter.
        let mut order: Vec<usize> = (0..self.document.symbols.len()).collect();
        order.sort_by(|a, b| {
            let sa = &self.document.symbols[*a];
            let sb = &self.document.symbols[*b];
            sa.position
                .y
                .partial_cmp(&sb.position.y)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(
                    sa.position
                        .x
                        .partial_cmp(&sb.position.x)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
                .then(sa.uuid.cmp(&sb.uuid))
        });

        let mut changed = false;
        for idx in order {
            let symbol = &mut self.document.symbols[idx];
            if !is_designator_target(symbol) {
                continue;
            }
            if !symbol.reference.ends_with('?') {
                continue;
            }
            let prefix: String = symbol
                .reference
                .chars()
                .take_while(|c| c.is_ascii_alphabetic())
                .collect();
            if prefix.is_empty() {
                continue;
            }
            let next = next_by_prefix.entry(prefix.clone()).or_insert(0);
            *next += 1;
            symbol.reference = format!("{prefix}{next}");
            changed = true;
        }

        if !changed {
            return Ok(false);
        }
        let patch_pair = PatchPair {
            semantic: SemanticPatch::SymbolFieldsUpdated,
            document: DocumentPatch::SYMBOLS,
        };
        self.record_history(before, patch_pair);
        Ok(true)
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn set_path(&mut self, path: Option<PathBuf>) {
        self.path = path;
    }

    pub fn set_document(&mut self, document: SchematicSheet) {
        self.document = document;
    }

    pub fn has_selected_items(&self, items: &[SelectedItem]) -> bool {
        items.iter().any(|item| self.contains_selected_item(item))
    }

    pub fn selection_is_single_symbol(&self, items: &[SelectedItem]) -> bool {
        matches!(items, [item] if item.kind == SelectedKind::Symbol && self.contains_selected_item(item))
    }

    pub fn collect_selection_clipboard(&self, items: &[SelectedItem]) -> ClipboardSelection {
        let mut clipboard = ClipboardSelection::default();

        for item in items {
            match item.kind {
                SelectedKind::Wire => {
                    if let Some(wire) = self
                        .document
                        .wires
                        .iter()
                        .find(|wire| wire.uuid == item.uuid)
                    {
                        clipboard.wires.push(wire.clone());
                    }
                }
                SelectedKind::Bus => {
                    if let Some(bus) = self.document.buses.iter().find(|bus| bus.uuid == item.uuid)
                    {
                        clipboard.buses.push(bus.clone());
                    }
                }
                SelectedKind::Label => {
                    if let Some(label) = self
                        .document
                        .labels
                        .iter()
                        .find(|label| label.uuid == item.uuid)
                    {
                        clipboard.labels.push(label.clone());
                    }
                }
                SelectedKind::Symbol => {
                    if let Some(symbol) = self
                        .document
                        .symbols
                        .iter()
                        .find(|symbol| symbol.uuid == item.uuid)
                    {
                        clipboard.symbols.push(symbol.clone());
                    }
                }
                SelectedKind::Junction => {
                    if let Some(junction) = self
                        .document
                        .junctions
                        .iter()
                        .find(|junction| junction.uuid == item.uuid)
                    {
                        clipboard.junctions.push(junction.clone());
                    }
                }
                SelectedKind::NoConnect => {
                    if let Some(no_connect) = self
                        .document
                        .no_connects
                        .iter()
                        .find(|no_connect| no_connect.uuid == item.uuid)
                    {
                        clipboard.no_connects.push(no_connect.clone());
                    }
                }
                SelectedKind::TextNote => {
                    if let Some(text_note) = self
                        .document
                        .text_notes
                        .iter()
                        .find(|text_note| text_note.uuid == item.uuid)
                    {
                        clipboard.text_notes.push(text_note.clone());
                    }
                }
                _ => {}
            }
        }

        clipboard
    }

    pub fn selection_anchors(&self, items: &[SelectedItem]) -> Vec<SelectionAnchor> {
        let mut anchors = Vec::new();

        for item in items {
            let position = match item.kind {
                SelectedKind::Symbol => self
                    .document
                    .symbols
                    .iter()
                    .find(|symbol| symbol.uuid == item.uuid)
                    .map(|symbol| (symbol.position.x, symbol.position.y)),
                SelectedKind::Label => self
                    .document
                    .labels
                    .iter()
                    .find(|label| label.uuid == item.uuid)
                    .map(|label| (label.position.x, label.position.y)),
                SelectedKind::Junction => self
                    .document
                    .junctions
                    .iter()
                    .find(|junction| junction.uuid == item.uuid)
                    .map(|junction| (junction.position.x, junction.position.y)),
                SelectedKind::NoConnect => self
                    .document
                    .no_connects
                    .iter()
                    .find(|no_connect| no_connect.uuid == item.uuid)
                    .map(|no_connect| (no_connect.position.x, no_connect.position.y)),
                SelectedKind::TextNote => self
                    .document
                    .text_notes
                    .iter()
                    .find(|text_note| text_note.uuid == item.uuid)
                    .map(|text_note| (text_note.position.x, text_note.position.y)),
                SelectedKind::Wire => self
                    .document
                    .wires
                    .iter()
                    .find(|wire| wire.uuid == item.uuid)
                    .map(|wire| {
                        (
                            (wire.start.x + wire.end.x) / 2.0,
                            (wire.start.y + wire.end.y) / 2.0,
                        )
                    }),
                SelectedKind::Bus => self
                    .document
                    .buses
                    .iter()
                    .find(|bus| bus.uuid == item.uuid)
                    .map(|bus| {
                        (
                            (bus.start.x + bus.end.x) / 2.0,
                            (bus.start.y + bus.end.y) / 2.0,
                        )
                    }),
                _ => None,
            };

            if let Some((x, y)) = position {
                anchors.push(SelectionAnchor {
                    uuid: item.uuid,
                    kind: item.kind,
                    x,
                    y,
                });
            }
        }

        anchors
    }

    pub fn describe_single_selection(&self, items: &[SelectedItem]) -> Option<SelectionDetails> {
        let [item] = items else {
            return None;
        };

        let h_align_label = |align| match align {
            signex_types::schematic::HAlign::Left => "Left",
            signex_types::schematic::HAlign::Center => "Center",
            signex_types::schematic::HAlign::Right => "Right",
        };
        let v_align_label = |align| match align {
            signex_types::schematic::VAlign::Top => "Top",
            signex_types::schematic::VAlign::Center => "Center",
            signex_types::schematic::VAlign::Bottom => "Bottom",
        };
        let fill_type_label = |fill| match fill {
            signex_types::schematic::FillType::None => "None",
            signex_types::schematic::FillType::Outline => "Outline",
            signex_types::schematic::FillType::Background => "Background",
        };

        let mut info = Vec::new();

        match item.kind {
            SelectedKind::Symbol => {
                let symbol = self
                    .document
                    .symbols
                    .iter()
                    .find(|symbol| symbol.uuid == item.uuid)?;
                let type_label = if symbol.is_power {
                    "Power Port"
                } else {
                    "Symbol"
                };
                info.push(("Type".into(), type_label.into()));
                info.push(("Reference".into(), symbol.reference.clone()));
                info.push(("Value".into(), symbol.value.clone()));
                // Description is pulled from the library entry — gives users
                // the Altium-style "IC XLTR VL BIDIR 8-X2SON" line without a
                // roundtrip to the datasheet.
                if let Some(lib_sym) = self.document.lib_symbols.get(&symbol.lib_id)
                    && !lib_sym.description.is_empty()
                {
                    info.push(("Description".into(), lib_sym.description.clone()));
                }
                info.push(("Library ID".into(), symbol.lib_id.clone()));
                info.push(("Footprint".into(), symbol.footprint.clone()));
                if !symbol.datasheet.is_empty() {
                    info.push(("Datasheet".into(), symbol.datasheet.clone()));
                }
                info.push((
                    "Position".into(),
                    format!("{:.2}, {:.2} mm", symbol.position.x, symbol.position.y),
                ));
                info.push(("Rotation".into(), format!("{:.0}\u{00b0}", symbol.rotation)));
                if symbol.mirror_x {
                    info.push(("Mirror".into(), "X".into()));
                }
                if symbol.mirror_y {
                    info.push(("Mirror".into(), "Y".into()));
                }
                if symbol.unit > 1 {
                    info.push(("Unit".into(), symbol.unit.to_string()));
                }
                info.push((
                    "Locked".into(),
                    if symbol.locked { "Yes" } else { "No" }.into(),
                ));
                info.push(("DNP".into(), if symbol.dnp { "Yes" } else { "No" }.into()));
                // Custom parameters (deterministic order by key).
                let mut custom: Vec<(&String, &String)> = symbol.fields.iter().collect();
                custom.sort_by(|a, b| a.0.cmp(b.0));
                for (name, value) in custom {
                    info.push((format!("Param: {name}"), value.clone()));
                }
            }
            SelectedKind::Wire => {
                let wire = self
                    .document
                    .wires
                    .iter()
                    .find(|wire| wire.uuid == item.uuid)?;
                let dx = wire.end.x - wire.start.x;
                let dy = wire.end.y - wire.start.y;
                let len = (dx * dx + dy * dy).sqrt();
                info.push(("Type".into(), "Wire".into()));
                info.push((
                    "Start".into(),
                    format!("{:.2}, {:.2}", wire.start.x, wire.start.y),
                ));
                info.push((
                    "End".into(),
                    format!("{:.2}, {:.2}", wire.end.x, wire.end.y),
                ));
                info.push(("Length".into(), format!("{:.2} mm", len)));
            }
            SelectedKind::Label => {
                let label = self
                    .document
                    .labels
                    .iter()
                    .find(|label| label.uuid == item.uuid)?;
                info.push(("Type".into(), format!("{:?} Label", label.label_type)));
                info.push(("Text".into(), label.text.clone()));
                info.push(("Net Name".into(), label.text.clone()));
                info.push((
                    "Position".into(),
                    format!("{:.2}, {:.2}", label.position.x, label.position.y),
                ));
                info.push(("Rotation".into(), format!("{:.0}°", label.rotation)));
                info.push((
                    "Text Size".into(),
                    format!(
                        "{}",
                        (label.font_size.max(0.0) / SCHEMATIC_PT_TO_MM).round() as i32
                    ),
                ));
                info.push((
                    "Horizontal Justification".into(),
                    h_align_label(label.justify).into(),
                ));
            }
            SelectedKind::Junction => {
                let junction = self
                    .document
                    .junctions
                    .iter()
                    .find(|junction| junction.uuid == item.uuid)?;
                info.push(("Type".into(), "Junction".into()));
                info.push((
                    "Position".into(),
                    format!("{:.2}, {:.2}", junction.position.x, junction.position.y),
                ));
            }
            SelectedKind::NoConnect => {
                let no_connect = self
                    .document
                    .no_connects
                    .iter()
                    .find(|no_connect| no_connect.uuid == item.uuid)?;
                info.push(("Type".into(), "No Connect".into()));
                info.push((
                    "Position".into(),
                    format!("{:.2}, {:.2}", no_connect.position.x, no_connect.position.y),
                ));
            }
            SelectedKind::TextNote => {
                let text_note = self
                    .document
                    .text_notes
                    .iter()
                    .find(|text_note| text_note.uuid == item.uuid)?;
                info.push(("Type".into(), "Text Note".into()));
                info.push(("Text".into(), text_note.text.clone()));
                info.push((
                    "Position".into(),
                    format!("{:.2}, {:.2}", text_note.position.x, text_note.position.y),
                ));
                info.push(("Rotation".into(), format!("{:.0}°", text_note.rotation)));
                info.push((
                    "Text Size".into(),
                    format!(
                        "{}",
                        (text_note.font_size.max(0.0) / SCHEMATIC_PT_TO_MM).round() as i32
                    ),
                ));
                info.push((
                    "Horizontal Justification".into(),
                    h_align_label(text_note.justify_h).into(),
                ));
                info.push((
                    "Vertical Justification".into(),
                    v_align_label(text_note.justify_v).into(),
                ));
            }
            SelectedKind::ChildSheet => {
                let child_sheet = self
                    .document
                    .child_sheets
                    .iter()
                    .find(|child_sheet| child_sheet.uuid == item.uuid)?;
                info.push(("Type".into(), "Hierarchical Sheet".into()));
                info.push(("Name".into(), child_sheet.name.clone()));
                info.push(("File".into(), child_sheet.filename.clone()));
                info.push((
                    "Position".into(),
                    format!(
                        "{:.2}, {:.2}",
                        child_sheet.position.x, child_sheet.position.y
                    ),
                ));
                info.push((
                    "Size".into(),
                    format!("{:.1} x {:.1} mm", child_sheet.size.0, child_sheet.size.1),
                ));
            }
            SelectedKind::Bus => {
                let bus = self
                    .document
                    .buses
                    .iter()
                    .find(|bus| bus.uuid == item.uuid)?;
                info.push(("Type".into(), "Bus".into()));
                info.push((
                    "Start".into(),
                    format!("{:.2}, {:.2}", bus.start.x, bus.start.y),
                ));
                info.push(("End".into(), format!("{:.2}, {:.2}", bus.end.x, bus.end.y)));
            }
            SelectedKind::BusEntry => {
                info.push(("Type".into(), "Bus Entry".into()));
            }
            SelectedKind::Drawing => {
                use signex_types::schematic::SchDrawing;
                let d = self.document.drawings.iter().find(|d| {
                    let u = match d {
                        SchDrawing::Line { uuid, .. }
                        | SchDrawing::Rect { uuid, .. }
                        | SchDrawing::Circle { uuid, .. }
                        | SchDrawing::Arc { uuid, .. }
                        | SchDrawing::Polyline { uuid, .. } => *uuid,
                    };
                    u == item.uuid
                })?;
                match d {
                    SchDrawing::Line {
                        start, end, width, ..
                    } => {
                        info.push(("Type".into(), "Line".into()));
                        info.push(("Start".into(), format!("{:.2}, {:.2}", start.x, start.y)));
                        info.push(("End".into(), format!("{:.2}, {:.2}", end.x, end.y)));
                        info.push(("Width".into(), format!("{width:.3}")));
                    }
                    SchDrawing::Rect {
                        start,
                        end,
                        width,
                        fill,
                        ..
                    } => {
                        info.push(("Type".into(), "Rectangle".into()));
                        let x0 = start.x.min(end.x);
                        let y0 = start.y.min(end.y);
                        let w = (end.x - start.x).abs();
                        let h = (end.y - start.y).abs();
                        info.push(("Position".into(), format!("{x0:.2}, {y0:.2}")));
                        info.push(("Width".into(), format!("{w:.2}")));
                        info.push(("Height".into(), format!("{h:.2}")));
                        info.push(("Border".into(), format!("{width:.3}")));
                        info.push(("Fill".into(), fill_type_label(*fill).into()));
                    }
                    SchDrawing::Circle {
                        center,
                        radius,
                        width,
                        fill,
                        ..
                    } => {
                        info.push(("Type".into(), "Circle".into()));
                        info.push(("Center".into(), format!("{:.2}, {:.2}", center.x, center.y)));
                        info.push(("Radius".into(), format!("{radius:.3}")));
                        info.push(("Border".into(), format!("{width:.3}")));
                        info.push(("Fill".into(), fill_type_label(*fill).into()));
                    }
                    SchDrawing::Arc {
                        start,
                        mid,
                        end,
                        width,
                        ..
                    } => {
                        info.push(("Type".into(), "Arc".into()));
                        if let Some((cx, cy, radius)) =
                            circumcircle((start.x, start.y), (mid.x, mid.y), (end.x, end.y))
                        {
                            let sa: f64 = (start.y - cy).atan2(start.x - cx);
                            let ea: f64 = (end.y - cy).atan2(end.x - cx);
                            let norm = |a: f64| -> f64 {
                                let mut t = a.to_degrees() % 360.0;
                                if t < 0.0 {
                                    t += 360.0;
                                }
                                t
                            };
                            info.push(("Center".into(), format!("{cx:.2}, {cy:.2}")));
                            info.push(("Radius".into(), format!("{radius:.3}")));
                            info.push(("Start Angle".into(), format!("{:.3}", norm(sa))));
                            info.push(("End Angle".into(), format!("{:.3}", norm(ea))));
                        } else {
                            info.push(("Start".into(), format!("{:.2}, {:.2}", start.x, start.y)));
                            info.push(("Mid".into(), format!("{:.2}, {:.2}", mid.x, mid.y)));
                            info.push(("End".into(), format!("{:.2}, {:.2}", end.x, end.y)));
                        }
                        info.push(("Width".into(), format!("{width:.3}")));
                    }
                    SchDrawing::Polyline {
                        points,
                        width,
                        fill,
                        ..
                    } => {
                        info.push(("Type".into(), "Polygon".into()));
                        info.push(("Vertices".into(), format!("{}", points.len())));
                        info.push(("Border".into(), format!("{width:.3}")));
                        info.push(("Fill".into(), fill_type_label(*fill).into()));
                    }
                }
            }
            SelectedKind::SymbolRefField => {
                let symbol = self
                    .document
                    .symbols
                    .iter()
                    .find(|symbol| symbol.uuid == item.uuid)?;
                let ref_text = symbol.ref_text.as_ref()?;
                info.push(("Type".into(), "Reference Field".into()));
                info.push(("Text".into(), symbol.reference.clone()));
                info.push(("Reference".into(), symbol.reference.clone()));
                info.push((
                    "Position".into(),
                    format!("{:.2}, {:.2} mm", ref_text.position.x, ref_text.position.y),
                ));
                info.push(("Rotation".into(), format!("{:.0}°", ref_text.rotation)));
                info.push((
                    "Text Size".into(),
                    format!(
                        "{}",
                        (ref_text.font_size.max(0.0) / SCHEMATIC_PT_TO_MM).round() as i32
                    ),
                ));
                info.push((
                    "Horizontal Justification".into(),
                    h_align_label(ref_text.justify_h).into(),
                ));
                info.push((
                    "Vertical Justification".into(),
                    v_align_label(ref_text.justify_v).into(),
                ));
                info.push((
                    "Visible".into(),
                    if ref_text.hidden { "No" } else { "Yes" }.into(),
                ));
                info.push((
                    "Fields Autoplaced".into(),
                    if symbol.fields_autoplaced {
                        "Yes"
                    } else {
                        "No"
                    }
                    .into(),
                ));
            }
            SelectedKind::SymbolValField => {
                let symbol = self
                    .document
                    .symbols
                    .iter()
                    .find(|symbol| symbol.uuid == item.uuid)?;
                let value_text = symbol.val_text.as_ref()?;
                info.push(("Type".into(), "Value Field".into()));
                info.push(("Text".into(), symbol.value.clone()));
                info.push(("Value".into(), symbol.value.clone()));
                info.push((
                    "Position".into(),
                    format!(
                        "{:.2}, {:.2} mm",
                        value_text.position.x, value_text.position.y
                    ),
                ));
                info.push(("Rotation".into(), format!("{:.0}°", value_text.rotation)));
                info.push((
                    "Text Size".into(),
                    format!(
                        "{}",
                        (value_text.font_size.max(0.0) / SCHEMATIC_PT_TO_MM).round() as i32
                    ),
                ));
                info.push((
                    "Horizontal Justification".into(),
                    h_align_label(value_text.justify_h).into(),
                ));
                info.push((
                    "Vertical Justification".into(),
                    v_align_label(value_text.justify_v).into(),
                ));
                info.push((
                    "Visible".into(),
                    if value_text.hidden { "No" } else { "Yes" }.into(),
                ));
                info.push((
                    "Fields Autoplaced".into(),
                    if symbol.fields_autoplaced {
                        "Yes"
                    } else {
                        "No"
                    }
                    .into(),
                ));
            }
        }

        Some(SelectionDetails {
            selected_uuid: item.uuid,
            selected_kind: item.kind,
            info,
        })
    }

    fn record_history(&mut self, snapshot: SchematicSheet, patch_pair: PatchPair) {
        if self.history.len() >= MAX_HISTORY_ENTRIES {
            self.history.remove(0);
        }
        self.history.push(HistoryEntry {
            snapshot,
            patch_pair,
        });
        self.redo_stack.clear();
    }

    fn contains_selected_item(&self, item: &SelectedItem) -> bool {
        match item.kind {
            SelectedKind::Wire => self
                .document
                .wires
                .iter()
                .any(|wire| wire.uuid == item.uuid),
            SelectedKind::Bus => self.document.buses.iter().any(|bus| bus.uuid == item.uuid),
            SelectedKind::Label => self
                .document
                .labels
                .iter()
                .any(|label| label.uuid == item.uuid),
            SelectedKind::Junction => self
                .document
                .junctions
                .iter()
                .any(|junction| junction.uuid == item.uuid),
            SelectedKind::NoConnect => self
                .document
                .no_connects
                .iter()
                .any(|no_connect| no_connect.uuid == item.uuid),
            SelectedKind::Symbol => self
                .document
                .symbols
                .iter()
                .any(|symbol| symbol.uuid == item.uuid),
            SelectedKind::TextNote => self
                .document
                .text_notes
                .iter()
                .any(|text_note| text_note.uuid == item.uuid),
            SelectedKind::Drawing => {
                use signex_types::schematic::SchDrawing;
                self.document.drawings.iter().any(|d| {
                    let u = match d {
                        SchDrawing::Line { uuid, .. }
                        | SchDrawing::Rect { uuid, .. }
                        | SchDrawing::Circle { uuid, .. }
                        | SchDrawing::Arc { uuid, .. }
                        | SchDrawing::Polyline { uuid, .. } => *uuid,
                    };
                    u == item.uuid
                })
            }
            _ => false,
        }
    }

    fn remove_selected_item(&mut self, item: &SelectedItem) -> bool {
        match item.kind {
            SelectedKind::Wire => remove_by_uuid(&mut self.document.wires, item.uuid),
            SelectedKind::Bus => remove_by_uuid(&mut self.document.buses, item.uuid),
            SelectedKind::Label => remove_by_uuid(&mut self.document.labels, item.uuid),
            SelectedKind::Junction => remove_by_uuid(&mut self.document.junctions, item.uuid),
            SelectedKind::NoConnect => remove_by_uuid(&mut self.document.no_connects, item.uuid),
            SelectedKind::Symbol => remove_by_uuid(&mut self.document.symbols, item.uuid),
            SelectedKind::TextNote => remove_by_uuid(&mut self.document.text_notes, item.uuid),
            SelectedKind::Drawing => {
                use signex_types::schematic::SchDrawing;
                let before_len = self.document.drawings.len();
                self.document.drawings.retain(|d| {
                    let u = match d {
                        SchDrawing::Line { uuid, .. }
                        | SchDrawing::Rect { uuid, .. }
                        | SchDrawing::Circle { uuid, .. }
                        | SchDrawing::Arc { uuid, .. }
                        | SchDrawing::Polyline { uuid, .. } => *uuid,
                    };
                    u != item.uuid
                });
                self.document.drawings.len() != before_len
            }
            _ => false,
        }
    }

    fn move_selected_item(&mut self, item: &SelectedItem, dx: f64, dy: f64) -> bool {
        match item.kind {
            SelectedKind::Symbol => self
                .document
                .symbols
                .iter_mut()
                .find(|symbol| symbol.uuid == item.uuid)
                .map(|symbol| {
                    symbol.position.x += dx;
                    symbol.position.y += dy;
                    if let Some(ref mut ref_text) = symbol.ref_text {
                        ref_text.position.x += dx;
                        ref_text.position.y += dy;
                    }
                    if let Some(ref mut val_text) = symbol.val_text {
                        val_text.position.x += dx;
                        val_text.position.y += dy;
                    }
                    true
                })
                .unwrap_or(false),
            SelectedKind::Wire => self
                .document
                .wires
                .iter_mut()
                .find(|wire| wire.uuid == item.uuid)
                .map(|wire| {
                    wire.start.x += dx;
                    wire.start.y += dy;
                    wire.end.x += dx;
                    wire.end.y += dy;
                    true
                })
                .unwrap_or(false),
            SelectedKind::Bus => self
                .document
                .buses
                .iter_mut()
                .find(|bus| bus.uuid == item.uuid)
                .map(|bus| {
                    bus.start.x += dx;
                    bus.start.y += dy;
                    bus.end.x += dx;
                    bus.end.y += dy;
                    true
                })
                .unwrap_or(false),
            SelectedKind::Label => self
                .document
                .labels
                .iter_mut()
                .find(|label| label.uuid == item.uuid)
                .map(|label| {
                    label.position.x += dx;
                    label.position.y += dy;
                    true
                })
                .unwrap_or(false),
            SelectedKind::Junction => self
                .document
                .junctions
                .iter_mut()
                .find(|junction| junction.uuid == item.uuid)
                .map(|junction| {
                    junction.position.x += dx;
                    junction.position.y += dy;
                    true
                })
                .unwrap_or(false),
            SelectedKind::NoConnect => self
                .document
                .no_connects
                .iter_mut()
                .find(|no_connect| no_connect.uuid == item.uuid)
                .map(|no_connect| {
                    no_connect.position.x += dx;
                    no_connect.position.y += dy;
                    true
                })
                .unwrap_or(false),
            SelectedKind::TextNote => self
                .document
                .text_notes
                .iter_mut()
                .find(|text_note| text_note.uuid == item.uuid)
                .map(|text_note| {
                    text_note.position.x += dx;
                    text_note.position.y += dy;
                    true
                })
                .unwrap_or(false),
            SelectedKind::ChildSheet => self
                .document
                .child_sheets
                .iter_mut()
                .find(|child_sheet| child_sheet.uuid == item.uuid)
                .map(|child_sheet| {
                    child_sheet.position.x += dx;
                    child_sheet.position.y += dy;
                    true
                })
                .unwrap_or(false),
            SelectedKind::BusEntry => self
                .document
                .bus_entries
                .iter_mut()
                .find(|bus_entry| bus_entry.uuid == item.uuid)
                .map(|bus_entry| {
                    bus_entry.position.x += dx;
                    bus_entry.position.y += dy;
                    true
                })
                .unwrap_or(false),
            SelectedKind::SymbolRefField => self
                .document
                .symbols
                .iter_mut()
                .find(|symbol| symbol.uuid == item.uuid)
                .map(|symbol| {
                    let (field_dx, field_dy) = inverse_field_display_delta(dx, dy);
                    if let Some(ref mut ref_text) = symbol.ref_text {
                        ref_text.position.x += field_dx;
                        ref_text.position.y += field_dy;
                        true
                    } else {
                        false
                    }
                })
                .unwrap_or(false),
            SelectedKind::SymbolValField => self
                .document
                .symbols
                .iter_mut()
                .find(|symbol| symbol.uuid == item.uuid)
                .map(|symbol| {
                    let (field_dx, field_dy) = inverse_field_display_delta(dx, dy);
                    if let Some(ref mut val_text) = symbol.val_text {
                        val_text.position.x += field_dx;
                        val_text.position.y += field_dy;
                        true
                    } else {
                        false
                    }
                })
                .unwrap_or(false),
            SelectedKind::Drawing => self
                .document
                .drawings
                .iter_mut()
                .find(|d| {
                    let u = match d {
                        signex_types::schematic::SchDrawing::Line { uuid, .. }
                        | signex_types::schematic::SchDrawing::Rect { uuid, .. }
                        | signex_types::schematic::SchDrawing::Circle { uuid, .. }
                        | signex_types::schematic::SchDrawing::Arc { uuid, .. }
                        | signex_types::schematic::SchDrawing::Polyline { uuid, .. } => *uuid,
                    };
                    u == item.uuid
                })
                .map(|d| {
                    use signex_types::schematic::SchDrawing;
                    match d {
                        SchDrawing::Line { start, end, .. } => {
                            start.x += dx;
                            start.y += dy;
                            end.x += dx;
                            end.y += dy;
                        }
                        SchDrawing::Rect { start, end, .. } => {
                            start.x += dx;
                            start.y += dy;
                            end.x += dx;
                            end.y += dy;
                        }
                        SchDrawing::Circle { center, .. } => {
                            center.x += dx;
                            center.y += dy;
                        }
                        SchDrawing::Arc {
                            start, mid, end, ..
                        } => {
                            start.x += dx;
                            start.y += dy;
                            mid.x += dx;
                            mid.y += dy;
                            end.x += dx;
                            end.y += dy;
                        }
                        SchDrawing::Polyline { points, .. } => {
                            for p in points {
                                p.x += dx;
                                p.y += dy;
                            }
                        }
                    }
                    true
                })
                .unwrap_or(false),
        }
    }

    fn rotate_selected_item(&mut self, item: &SelectedItem, angle_degrees: f64) -> bool {
        match item.kind {
            SelectedKind::Symbol => self
                .document
                .symbols
                .iter_mut()
                .find(|symbol| symbol.uuid == item.uuid)
                .map(|symbol| {
                    symbol.rotation = (symbol.rotation + angle_degrees) % 360.0;
                    true
                })
                .unwrap_or(false),
            _ => false,
        }
    }

    fn mirror_selected_item(&mut self, item: &SelectedItem, axis: MirrorAxis) -> bool {
        match item.kind {
            SelectedKind::Symbol => self
                .document
                .symbols
                .iter_mut()
                .find(|symbol| symbol.uuid == item.uuid)
                .map(|symbol| {
                    match axis {
                        MirrorAxis::Horizontal => symbol.mirror_y = !symbol.mirror_y,
                        MirrorAxis::Vertical => symbol.mirror_x = !symbol.mirror_x,
                    }
                    true
                })
                .unwrap_or(false),
            _ => false,
        }
    }
}

fn inverse_field_display_delta(dx: f64, dy: f64) -> (f64, f64) {
    (dx, dy)
}

fn point_on_wire_interior(
    point: signex_types::schematic::Point,
    wire: &signex_types::schematic::Wire,
    tolerance: f64,
) -> bool {
    let (ax, ay) = (wire.start.x, wire.start.y);
    let (bx, by) = (wire.end.x, wire.end.y);
    let (px, py) = (point.x, point.y);
    let (abx, aby) = (bx - ax, by - ay);
    let (apx, apy) = (px - ax, py - ay);
    let len_sq = abx * abx + aby * aby;

    if len_sq < tolerance * tolerance {
        return false;
    }

    let cross = abx * apy - aby * apx;
    if (cross * cross) > tolerance * tolerance * len_sq {
        return false;
    }

    let t = (apx * abx + apy * aby) / len_sq;
    let margin = tolerance / len_sq.sqrt();
    t > margin && t < 1.0 - margin
}

fn needed_junction(
    point: signex_types::schematic::Point,
    document: &SchematicSheet,
    tolerance: f64,
) -> Option<signex_types::schematic::Junction> {
    let already_present = document.junctions.iter().any(|junction| {
        (junction.position.x - point.x).abs() < tolerance
            && (junction.position.y - point.y).abs() < tolerance
    });
    if already_present {
        return None;
    }

    let on_wire_interior = document
        .wires
        .iter()
        .any(|wire| point_on_wire_interior(point, wire, tolerance));
    if on_wire_interior {
        return Some(signex_types::schematic::Junction {
            uuid: uuid::Uuid::new_v4(),
            position: point,
            diameter: 0.0,
        });
    }

    let endpoint_count = document
        .wires
        .iter()
        .filter(|wire| {
            let at_start = (wire.start.x - point.x).abs() < tolerance
                && (wire.start.y - point.y).abs() < tolerance;
            let at_end = (wire.end.x - point.x).abs() < tolerance
                && (wire.end.y - point.y).abs() < tolerance;
            at_start || at_end
        })
        .count();
    if endpoint_count >= 3 {
        return Some(signex_types::schematic::Junction {
            uuid: uuid::Uuid::new_v4(),
            position: point,
            diameter: 0.0,
        });
    }

    None
}

fn remove_by_uuid<T>(items: &mut Vec<T>, uuid: uuid::Uuid) -> bool
where
    T: HasUuid,
{
    let original_len = items.len();
    items.retain(|item| item.uuid() != uuid);
    original_len != items.len()
}

/// Circle through three non-collinear points — used by the Arc
/// selection info + properties panel to convert KiCad's
/// (start, mid, end) storage into Altium's (center, radius, start
/// angle, end angle) representation.
fn circumcircle(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> Option<(f64, f64, f64)> {
    let (ax, ay) = a;
    let (bx, by) = b;
    let (cx, cy) = c;
    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-9 {
        return None;
    }
    let ux = ((ax * ax + ay * ay) * (by - cy)
        + (bx * bx + by * by) * (cy - ay)
        + (cx * cx + cy * cy) * (ay - by))
        / d;
    let uy = ((ax * ax + ay * ay) * (cx - bx)
        + (bx * bx + by * by) * (ax - cx)
        + (cx * cx + cy * cy) * (bx - ax))
        / d;
    let r = ((ax - ux) * (ax - ux) + (ay - uy) * (ay - uy)).sqrt();
    Some((ux, uy, r))
}

trait HasUuid {
    fn uuid(&self) -> uuid::Uuid;
}

macro_rules! impl_has_uuid {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl HasUuid for $ty {
                fn uuid(&self) -> uuid::Uuid {
                    self.uuid
                }
            }
        )+
    };
}

impl_has_uuid!(
    signex_types::schematic::Wire,
    signex_types::schematic::Bus,
    signex_types::schematic::Label,
    signex_types::schematic::Junction,
    signex_types::schematic::NoConnect,
    signex_types::schematic::Symbol,
    signex_types::schematic::TextNote,
);
