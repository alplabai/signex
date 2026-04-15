mod command;
mod error;
mod patch;

use std::path::{Path, PathBuf};

pub use command::{Command, CommandKind, MirrorAxis, TextTarget};
pub use error::EngineError;
pub use patch::{CommandResult, DocumentPatch, PatchPair, SemanticPatch};
use signex_types::schematic::{SchematicSheet, SelectedItem, SelectedKind};

#[derive(Debug, Clone)]
struct HistoryEntry {
    before: SchematicSheet,
    after: SchematicSheet,
    patch_pair: PatchPair,
}

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

    pub fn new_with_path(document: SchematicSheet, path: Option<PathBuf>) -> Result<Self, EngineError> {
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
                    document: DocumentPatch::DocumentReplaced,
                };

                self.history.push(HistoryEntry {
                    before,
                    after: self.document.clone(),
                    patch_pair,
                });
                self.redo_stack.clear();

                Ok(CommandResult::changed(patch_pair))
            }
            Command::UpdateText { target, value } => {
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
                    document: DocumentPatch::TextUpdated,
                };

                self.history.push(HistoryEntry {
                    before,
                    after: self.document.clone(),
                    patch_pair,
                });
                self.redo_stack.clear();

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
                    document: DocumentPatch::SymbolFieldsUpdated,
                };

                self.history.push(HistoryEntry {
                    before,
                    after: self.document.clone(),
                    patch_pair,
                });
                self.redo_stack.clear();

                Ok(CommandResult::changed(patch_pair))
            }
            Command::DeleteSelection { items } => {
                let mut changed = false;

                for item in items {
                    changed |= self.remove_selected_item(&item);
                }

                if !changed {
                    return Ok(CommandResult::unchanged());
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SelectionDeleted,
                    document: DocumentPatch::SelectionDeleted,
                };

                self.history.push(HistoryEntry {
                    before,
                    after: self.document.clone(),
                    patch_pair,
                });
                self.redo_stack.clear();

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
                    document: DocumentPatch::SelectionMoved,
                };

                self.history.push(HistoryEntry {
                    before,
                    after: self.document.clone(),
                    patch_pair,
                });
                self.redo_stack.clear();

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
                    document: DocumentPatch::SelectionRotated,
                };

                self.history.push(HistoryEntry {
                    before,
                    after: self.document.clone(),
                    patch_pair,
                });
                self.redo_stack.clear();

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
                    document: DocumentPatch::SelectionMirrored,
                };

                self.history.push(HistoryEntry {
                    before,
                    after: self.document.clone(),
                    patch_pair,
                });
                self.redo_stack.clear();

                Ok(CommandResult::changed(patch_pair))
            }
            Command::PlaceBus { bus } => {
                self.document.buses.push(bus);

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: DocumentPatch::ObjectPlaced,
                };

                self.history.push(HistoryEntry {
                    before,
                    after: self.document.clone(),
                    patch_pair,
                });
                self.redo_stack.clear();

                Ok(CommandResult::changed(patch_pair))
            }
            Command::PlaceSymbol { symbol } => {
                self.document.symbols.push(symbol);

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: DocumentPatch::ObjectPlaced,
                };

                self.history.push(HistoryEntry {
                    before,
                    after: self.document.clone(),
                    patch_pair,
                });
                self.redo_stack.clear();

                Ok(CommandResult::changed(patch_pair))
            }
            Command::PlaceNoConnect { no_connect } => {
                self.document.no_connects.push(no_connect);

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: DocumentPatch::ObjectPlaced,
                };

                self.history.push(HistoryEntry {
                    before,
                    after: self.document.clone(),
                    patch_pair,
                });
                self.redo_stack.clear();

                Ok(CommandResult::changed(patch_pair))
            }
            Command::PlaceBusEntry { bus_entry } => {
                self.document.bus_entries.push(bus_entry);

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: DocumentPatch::ObjectPlaced,
                };

                self.history.push(HistoryEntry {
                    before,
                    after: self.document.clone(),
                    patch_pair,
                });
                self.redo_stack.clear();

                Ok(CommandResult::changed(patch_pair))
            }
            Command::PlaceTextNote { text_note } => {
                self.document.text_notes.push(text_note);

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: DocumentPatch::ObjectPlaced,
                };

                self.history.push(HistoryEntry {
                    before,
                    after: self.document.clone(),
                    patch_pair,
                });
                self.redo_stack.clear();

                Ok(CommandResult::changed(patch_pair))
            }
            other => Err(EngineError::UnsupportedCommand(other.kind())),
        }
    }

    pub fn undo(&mut self) -> Result<Option<PatchPair>, EngineError> {
        let Some(entry) = self.history.pop() else {
            return Ok(None);
        };

        self.document = entry.before.clone();
        let patch_pair = entry.patch_pair;
        self.redo_stack.push(entry);
        Ok(Some(patch_pair))
    }

    pub fn redo(&mut self) -> Result<Option<PatchPair>, EngineError> {
        let Some(entry) = self.redo_stack.pop() else {
            return Ok(None);
        };

        self.document = entry.after.clone();
        let patch_pair = entry.patch_pair;
        self.history.push(entry);
        Ok(Some(patch_pair))
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

    fn remove_selected_item(&mut self, item: &SelectedItem) -> bool {
        match item.kind {
            SelectedKind::Wire => remove_by_uuid(&mut self.document.wires, item.uuid),
            SelectedKind::Bus => remove_by_uuid(&mut self.document.buses, item.uuid),
            SelectedKind::Label => remove_by_uuid(&mut self.document.labels, item.uuid),
            SelectedKind::Junction => remove_by_uuid(&mut self.document.junctions, item.uuid),
            SelectedKind::NoConnect => remove_by_uuid(&mut self.document.no_connects, item.uuid),
            SelectedKind::Symbol => remove_by_uuid(&mut self.document.symbols, item.uuid),
            SelectedKind::TextNote => remove_by_uuid(&mut self.document.text_notes, item.uuid),
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
            SelectedKind::Drawing => false,
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

fn remove_by_uuid<T>(items: &mut Vec<T>, uuid: uuid::Uuid) -> bool
where
    T: HasUuid,
{
    let original_len = items.len();
    items.retain(|item| item.uuid() != uuid);
    original_len != items.len()
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