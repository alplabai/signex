mod command;
mod error;
mod patch;

mod annotation;
mod history;
mod selection;
mod sheet;
mod transform;

use std::path::{Path, PathBuf};

pub use command::{
    AnnotateMode, Command, CommandKind, MirrorAxis, ReorderDirection, SheetPort, SymbolTextField,
    TextTarget,
};
pub use error::EngineError;
use history::HistoryEntry;
pub use patch::{CommandResult, DocumentPatch, PatchPair, SemanticPatch};
pub use selection::{ClipboardSelection, SelectionAnchor, SelectionDetails};
use signex_types::schematic::SchematicSheet;

const JUNCTION_TOLERANCE_MM: f64 = 0.01;

#[derive(Debug)]
pub struct Engine {
    document: SchematicSheet,
    path: Option<PathBuf>,
    history: Vec<HistoryEntry>,
    redo_stack: Vec<HistoryEntry>,
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

    /// Open a `.snxsch` file from disk.
    ///
    /// Files in foreign formats (e.g. Standard's `.standard_sch`) are not
    /// readable directly by Signex Community; users with Standard
    /// projects run the optional `signex-standard-import` GPL-3.0
    /// companion tool to convert their files first.
    pub fn open(path: &Path) -> Result<Self, EngineError> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| EngineError::OpenFailed(anyhow::Error::msg(error.to_string())))?;
        let snx = signex_types::format::SnxSchematic::parse(&text)
            .map_err(|error| EngineError::OpenFailed(anyhow::Error::msg(error.to_string())))?;

        Ok(Self {
            document: snx.sheet,
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
        // TODO(v0.9.1 perf): replace clone+serialise+write with a
        // borrow-based path that runs off the UI thread. The clone
        // here is ~50–100 ms on huge PCBs; serialisation is
        // 200–500 ms; and the std::fs::write is sync. For v0.9.0 this
        // ships as-is — large-PCB save still blocks the UI for the
        // full duration. Async + zero-clone serialise are a
        // documented follow-up tracked in CHANGELOG.md and
        // project_issue_62_licensing.md memory.
        let snx = signex_types::format::SnxSchematic::new(self.document.clone());
        let content = snx
            .write_string()
            .map_err(|error| EngineError::SaveFailed(std::io::Error::other(error.to_string())))?;
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
                        transform::needed_junction(point, &self.document, JUNCTION_TOLERANCE_MM)
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
            Command::UpdateChildSheetStyle {
                sheet_id,
                stroke_width,
                stroke_color,
                fill_color,
            } => {
                let Some(sheet) = self
                    .document
                    .child_sheets
                    .iter_mut()
                    .find(|cs| cs.uuid == sheet_id)
                else {
                    return Ok(CommandResult::unchanged());
                };
                let mut changed = false;
                if let Some(w) = stroke_width
                    && (sheet.stroke_width - w).abs() > f64::EPSILON
                {
                    sheet.stroke_width = w;
                    changed = true;
                }
                if let Some(c) = stroke_color
                    && sheet.stroke_color != c
                {
                    sheet.stroke_color = c;
                    changed = true;
                }
                if let Some(c) = fill_color
                    && sheet.fill_color != c
                {
                    sheet.fill_color = c;
                    changed = true;
                }
                if !changed {
                    return Ok(CommandResult::unchanged());
                }
                let patch_pair = PatchPair {
                    semantic: SemanticPatch::TextUpdated,
                    document: DocumentPatch::CHILD_SHEETS,
                };
                self.record_history(before, patch_pair);
                Ok(CommandResult::changed(patch_pair))
            }
            Command::AnnotateAll { mode } => {
                use crate::command::AnnotateMode;
                // Power ports (is_power == true, or reference starting with '#')
                // are Standard net anchors, not components. Their references
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
                        // z-ordered in Standard schematic (no explicit
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
            Command::ReconcileChildSheetPins {
                child_filename,
                ports,
            } => {
                let mut changed = false;
                for child in self
                    .document
                    .child_sheets
                    .iter_mut()
                    .filter(|child| child.filename == child_filename)
                {
                    changed |= sheet::reconcile_child_sheet_pins(child, &ports);
                }

                if !changed {
                    return Ok(CommandResult::unchanged());
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SheetPinsReconciled,
                    document: DocumentPatch::CHILD_SHEETS,
                };
                self.record_history(before, patch_pair);
                Ok(CommandResult::changed(patch_pair))
            }
        }
    }

    pub fn document(&self) -> &SchematicSheet {
        &self.document
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{
        ChildSheet, FillType, GRID_MM, Label, LabelType, Point, SelectedItem, SelectedKind,
        SheetPin,
    };

    fn test_sheet() -> SchematicSheet {
        SchematicSheet {
            uuid: uuid::Uuid::new_v4(),
            version: 0,
            generator: String::new(),
            generator_version: String::new(),
            paper_size: "A4".to_string(),
            root_sheet_page: "1".to_string(),
            symbols: Vec::new(),
            wires: Vec::new(),
            junctions: Vec::new(),
            labels: Vec::new(),
            child_sheets: Vec::new(),
            no_connects: Vec::new(),
            text_notes: Vec::new(),
            buses: Vec::new(),
            bus_entries: Vec::new(),
            drawings: Vec::new(),
            no_erc_directives: Vec::new(),
            title_block: std::collections::HashMap::new(),
            lib_symbols: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn collect_exposed_sheet_ports_prefers_hierarchical_over_global() {
        let mut document = test_sheet();
        document.labels.push(Label {
            uuid: uuid::Uuid::new_v4(),
            text: "ALERT".to_string(),
            position: Point::new(0.0, 0.0),
            rotation: 0.0,
            label_type: LabelType::Global,
            shape: "output".to_string(),
            font_size: 1.27,
            justify: signex_types::schematic::HAlign::Left,
            justify_v: signex_types::schematic::VAlign::Bottom,
        });
        document.labels.push(Label {
            uuid: uuid::Uuid::new_v4(),
            text: "ALERT".to_string(),
            position: Point::new(1.0, 1.0),
            rotation: 0.0,
            label_type: LabelType::Hierarchical,
            shape: "input".to_string(),
            font_size: 1.27,
            justify: signex_types::schematic::HAlign::Left,
            justify_v: signex_types::schematic::VAlign::Bottom,
        });

        let engine = Engine::new(document).unwrap();
        let ports = engine.collect_exposed_sheet_ports();

        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].name, "ALERT");
        assert_eq!(ports[0].direction, "input");
    }

    #[test]
    fn reconcile_child_sheet_pins_adds_new_and_removes_stale_auto_generated() {
        let mut document = test_sheet();
        document.child_sheets.push(ChildSheet {
            uuid: uuid::Uuid::new_v4(),
            name: "Child".to_string(),
            filename: "child.standard_sch".to_string(),
            position: Point::new(10.0, 20.0),
            size: (30.0, 30.0),
            stroke_width: 0.12,
            fill: FillType::None, stroke_color: None, fill_color: None,
            fields_autoplaced: false,
            pins: vec![
                SheetPin {
                    uuid: uuid::Uuid::new_v4(),
                    name: "OLD_AUTO".to_string(),
                    direction: "input".to_string(),
                    position: Point::new(10.0, 22.0),
                    rotation: 0.0,
                    auto_generated: true,
                    user_moved: false,
                },
                SheetPin {
                    uuid: uuid::Uuid::new_v4(),
                    name: "MANUAL".to_string(),
                    direction: "input".to_string(),
                    position: Point::new(10.0, 24.0),
                    rotation: 0.0,
                    auto_generated: false,
                    user_moved: false,
                },
            ],
            instances: Vec::new(),
        });

        let mut engine = Engine::new(document).unwrap();
        let result = engine
            .execute(Command::ReconcileChildSheetPins {
                child_filename: "child.standard_sch".to_string(),
                ports: vec![
                    SheetPort {
                        name: "SDA".to_string(),
                        direction: "input".to_string(),
                    },
                    SheetPort {
                        name: "SCL".to_string(),
                        direction: "output".to_string(),
                    },
                ],
            })
            .unwrap();

        assert!(result.changed);
        let pins = &engine.document().child_sheets[0].pins;
        assert!(
            pins.iter()
                .any(|pin| pin.name == "MANUAL" && !pin.auto_generated)
        );
        assert!(
            pins.iter()
                .any(|pin| pin.name == "SDA" && pin.auto_generated)
        );
        assert!(
            pins.iter()
                .any(|pin| pin.name == "SCL" && pin.auto_generated)
        );
        assert!(!pins.iter().any(|pin| pin.name == "OLD_AUTO"));
    }

    #[test]
    fn reconcile_preserves_position_for_user_moved_pin() {
        let mut document = test_sheet();
        let moved_uuid = uuid::Uuid::new_v4();
        document.child_sheets.push(ChildSheet {
            uuid: uuid::Uuid::new_v4(),
            name: "Child".to_string(),
            filename: "child.standard_sch".to_string(),
            position: Point::new(10.0, 20.0),
            size: (30.0, 30.0),
            stroke_width: 0.12,
            fill: FillType::None, stroke_color: None, fill_color: None,
            fields_autoplaced: false,
            pins: vec![SheetPin {
                uuid: moved_uuid,
                name: "SDA".to_string(),
                direction: "input".to_string(),
                position: Point::new(25.0, 33.0),
                rotation: 90.0,
                auto_generated: true,
                user_moved: true,
            }],
            instances: Vec::new(),
        });

        let mut engine = Engine::new(document).unwrap();
        let _ = engine
            .execute(Command::ReconcileChildSheetPins {
                child_filename: "child.standard_sch".to_string(),
                ports: vec![SheetPort {
                    name: "SDA".to_string(),
                    direction: "output".to_string(),
                }],
            })
            .unwrap();

        let pin = engine.document().child_sheets[0]
            .pins
            .iter()
            .find(|pin| pin.uuid == moved_uuid)
            .unwrap();
        assert_eq!(pin.position, Point::new(25.0, 33.0));
        assert_eq!(pin.rotation, 90.0);
        assert_eq!(pin.direction, "output");
    }

    #[test]
    fn moving_sheet_pin_locks_to_nearest_sheet_edge() {
        let mut document = test_sheet();
        let pin_uuid = uuid::Uuid::new_v4();
        let sheet_uuid = uuid::Uuid::new_v4();

        document.child_sheets.push(ChildSheet {
            uuid: sheet_uuid,
            name: "Child".to_string(),
            filename: "child.standard_sch".to_string(),
            position: Point::new(10.0, 20.0),
            size: (30.0, 30.0),
            stroke_width: 0.12,
            fill: FillType::None, stroke_color: None, fill_color: None,
            fields_autoplaced: false,
            pins: vec![SheetPin {
                uuid: pin_uuid,
                name: "SDA".to_string(),
                direction: "input".to_string(),
                position: Point::new(10.0, 25.0),
                rotation: 0.0,
                auto_generated: true,
                user_moved: false,
            }],
            instances: Vec::new(),
        });

        let mut engine = Engine::new(document).unwrap();

        let _ = engine
            .execute(Command::MoveSelection {
                items: vec![SelectedItem::new(pin_uuid, SelectedKind::SheetPin)],
                dx: 35.0,
                dy: -100.0,
            })
            .unwrap();

        let moved = engine.document().child_sheets[0]
            .pins
            .iter()
            .find(|pin| pin.uuid == pin_uuid)
            .unwrap();

        assert_eq!(moved.position.x, 40.0);
        assert_eq!(moved.rotation, 180.0);
        assert_eq!(moved.position.y, 20.0 + GRID_MM);
        assert!(moved.user_moved);
    }
}
