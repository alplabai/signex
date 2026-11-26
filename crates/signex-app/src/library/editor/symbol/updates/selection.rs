//! Symbol editor — selection update logic.

use super::SymEditor;
use crate::library::messages::{SymbolEditorMsg, SymbolSelectionMsg};

pub(super) fn apply_symbol_selection(editor: &mut SymEditor, msg: SymbolEditorMsg) {
    use crate::library::editor::symbol::state::{FieldKey, SymbolSelection};
    match msg {
        SymbolEditorMsg::Select(sel) => {
            editor.gui.selected = Some(match sel {
                SymbolSelectionMsg::Pin(idx) => SymbolSelection::Pin(idx),
                SymbolSelectionMsg::FieldReference => SymbolSelection::Field(FieldKey::Reference),
                SymbolSelectionMsg::FieldValue => SymbolSelection::Field(FieldKey::Value),
                SymbolSelectionMsg::Graphic(idx) => SymbolSelection::Graphic(idx),
                SymbolSelectionMsg::All => SymbolSelection::All,
                SymbolSelectionMsg::Multiple {
                    pin_indices,
                    graphic_indices,
                } => SymbolSelection::Multiple {
                    pin_indices,
                    graphic_indices,
                },
            });
            editor.gui.canvas_cache.clear();
        }
        SymbolEditorMsg::Deselect => {
            editor.gui.selected = None;
            editor.gui.canvas_cache.clear();
        }
        _ => {}
    }
}
