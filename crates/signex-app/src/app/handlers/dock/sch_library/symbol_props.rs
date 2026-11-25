//! The symbol editor's Properties panel messages (mockup rows 122 and 123):
//! its tab, the Pins tab filter and part chip and the illustration hover
//! change only the panel state; `‹ Pins`, `+ Pin`, the Parts stepper and the
//! Body choice act on the editor through the same paths as the canvas
//! toolbar. `Edit pins in table…` opens the Pin table tab (mockup row
//! 122b), whose `+ Pin`, Delete selected, Apply to selected and Apply paste
//! each change the symbol as one undo step.

use std::collections::BTreeSet;
use std::path::Path;

use super::*;
use crate::library::editor::symbol::{
    PartStep, PinTableMsg, PinTableUi, SymbolPropsMsg, SymbolSelection, add_table_pin, apply_bulk,
    apply_paste, parse_mm, remove_pins,
};
use crate::library::messages::{SymbolEditorMsg, SymbolToolMsg};

/// Whether a typed pin length can be stored, by the Properties field's rule.
fn pin_length_ok(value: f64) -> bool {
    crate::panels::NumericFieldKey::PinLength(0).accepts(value)
}

impl Signex {
    pub(super) fn handle_symbol_props(&mut self, msg: &SymbolPropsMsg) -> Task<Message> {
        // A Pin table tab speaks for its symbol.
        let Some(path) = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .and_then(|t| t.symbol_document())
            .cloned()
        else {
            return Task::none();
        };
        let panel_only = self
            .library
            .symbol_props
            .entry(path.clone())
            .or_default()
            .apply(msg);
        if panel_only {
            self.refresh_panel_ctx();
            return Task::none();
        }
        let editor_msg = match msg {
            SymbolPropsMsg::PinsBackPressed => {
                self.sym_editor_clear_selection();
                return Task::none();
            }
            SymbolPropsMsg::EditPinsInTablePressed => {
                self.open_pin_table_tab(path);
                return Task::none();
            }
            SymbolPropsMsg::PinTable(table_msg) => {
                self.pin_table_edited(&path, table_msg);
                return Task::none();
            }
            SymbolPropsMsg::AddPinPressed => SymbolEditorMsg::SetTool(SymbolToolMsg::AddPin),
            SymbolPropsMsg::PartStepPressed(step) => match step {
                PartStep::Previous => SymbolEditorMsg::PrevPart,
                PartStep::Next => SymbolEditorMsg::NextPart,
                PartStep::Added => SymbolEditorMsg::NewPart,
                PartStep::Removed => SymbolEditorMsg::RemovePart,
            },
            SymbolPropsMsg::BodyPicked(body) => SymbolEditorMsg::BodyStyleSelected(*body),
            SymbolPropsMsg::BodyScopePicked(scope) => {
                let Some(editor) = self.document_state.symbol_editors.get(&path) else {
                    return Task::none();
                };
                SymbolEditorMsg::SelectionBodyAssigned(scope.tag(editor.view()))
            }
            SymbolPropsMsg::TabPicked(_)
            | SymbolPropsMsg::PinFilterChanged(_)
            | SymbolPropsMsg::PartChipPicked(_)
            | SymbolPropsMsg::IllustrationHovered(_) => return Task::none(),
        };
        // The library dispatcher, not `update`: no re-entry into the root
        // loop, and the editor's edit guards still apply.
        self.dispatch_library_message(
            crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                path,
                msg: crate::library::messages::PrimitiveEdit::Symbol(editor_msg),
            },
        )
    }

    /// A Pin table edit of the symbol at `path`. A delete or a paste drops
    /// the editor's pin selection, whose indices may name other pins now.
    fn pin_table_edited(&mut self, path: &Path, msg: &PinTableMsg) {
        let table = self
            .library
            .symbol_props
            .get(path)
            .map(|props| props.table.clone())
            .unwrap_or_default();
        match msg {
            PinTableMsg::AddPinPressed => {
                self.sym_editor_record_edit(None, |editor| {
                    let view = editor.view();
                    add_table_pin(editor.primitive_mut(), view);
                    true
                });
            }
            PinTableMsg::DeleteSelectedPressed => {
                let rows = self.pin_table_acted_rows(path, &table);
                let mut deleted = false;
                self.sym_editor_record_edit(None, |editor| {
                    deleted = remove_pins(editor.primitive_mut(), &rows);
                    if deleted {
                        drop_pin_selection(&mut editor.selected);
                    }
                    deleted
                });
                if deleted {
                    self.with_pin_table(path, |ui| ui.forget_removed(&rows));
                }
            }
            PinTableMsg::BulkApplied => {
                let rows = self.pin_table_acted_rows(path, &table);
                let length = self.bulk_length(&table);
                self.sym_editor_record_edit(None, |editor| {
                    apply_bulk(editor.primitive_mut(), &rows, &table.bulk, length)
                });
                self.with_pin_table(path, |ui| ui.bulk = Default::default());
            }
            PinTableMsg::PasteApplied => {
                let Some(paste) = &table.paste else {
                    return;
                };
                let (rows, mode) = (paste.rows(), paste.mode);
                self.sym_editor_record_edit(None, |editor| {
                    let view = editor.view();
                    let pasted =
                        apply_paste(editor.primitive_mut(), &rows, mode, view, pin_length_ok);
                    if pasted {
                        drop_pin_selection(&mut editor.selected);
                    }
                    pasted
                });
                self.with_pin_table(path, |ui| {
                    ui.paste = None;
                    ui.checked.clear();
                });
            }
            // Tab state, which `PinTableUi::apply` already took.
            PinTableMsg::FilterChanged(_)
            | PinTableMsg::IssuesOnlyToggled
            | PinTableMsg::RowChecked(..)
            | PinTableMsg::AllChecked(..)
            | PinTableMsg::BulkElectricalPicked(_)
            | PinTableMsg::BulkPartChanged(_)
            | PinTableMsg::BulkLengthChanged(_)
            | PinTableMsg::BulkOrientationPicked(_)
            | PinTableMsg::BulkHiddenPicked(_)
            | PinTableMsg::PasteToggled
            | PinTableMsg::PasteEdited(_)
            | PinTableMsg::PasteColumnMapped(..)
            | PinTableMsg::PasteModePicked(_)
            | PinTableMsg::PasteCancelled => {}
        }
        let pins = self
            .document_state
            .symbol_editors
            .get(path)
            .map_or(0, |editor| editor.primitive().pins.len());
        self.with_pin_table(path, |ui| ui.keep_checks_below(pins));
        self.refresh_panel_ctx();
    }

    /// The bulk length to write: `None` when the field is empty, or when it
    /// is not a length a pin can have, which Messages reports.
    fn bulk_length(&self, table: &PinTableUi) -> Option<f64> {
        let typed = table.bulk.length.trim();
        if typed.is_empty() {
            return None;
        }
        let length = parse_mm(typed).filter(|v| pin_length_ok(*v));
        if length.is_none() {
            crate::diagnostics::log_warning(
                signex_i18n::t!(
                    self.ui_state.i18n,
                    signex_i18n::ids::EDITORS_PIN_TABLE_LENGTH_REFUSED,
                    "value" => typed
                )
                .into_string(),
            );
        }
        length
    }

    /// The ticked rows of `path`'s Pin table that it lists now, after the
    /// filter and the issues-only view; none while the panel context shows
    /// another symbol.
    fn pin_table_acted_rows(&self, path: &Path, table: &PinTableUi) -> BTreeSet<usize> {
        self.document_state
            .panel_ctx
            .symbol_editor
            .as_ref()
            .filter(|sym| sym.path == path)
            .map(|sym| crate::panels::pin_table_acted_rows(sym, table, &self.ui_state.i18n))
            .unwrap_or_default()
    }

    fn with_pin_table(&mut self, path: &Path, change: impl FnOnce(&mut PinTableUi)) {
        if let Some(props) = self.library.symbol_props.get_mut(path) {
            change(&mut props.table);
        }
    }

    /// Clear the active symbol editor's selection, so Properties shows the
    /// symbol again.
    fn sym_editor_clear_selection(&mut self) {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return;
        };
        if editor.selected.take().is_some() {
            editor.graphic_fill_picker = None;
            editor.local_color_picker = None;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
    }
}

/// Drop a selection that names pins by index.
fn drop_pin_selection(selected: &mut Option<SymbolSelection>) {
    if matches!(
        selected,
        Some(SymbolSelection::Pin(_) | SymbolSelection::Multiple { .. } | SymbolSelection::All)
    ) {
        *selected = None;
    }
}
