//! Symbol editor — undo / redo / drag-commit update logic.

use super::{SymEditor, close_pickers};
use crate::library::messages::SymbolEditorMsg;

pub(super) fn apply_symbol_history(editor: &mut SymEditor, msg: SymbolEditorMsg) {
    match msg {
        SymbolEditorMsg::Undo => {
            if editor.undo() {
                close_pickers(editor);
                // `undo()`/`redo()` own the dirty flag and the save point.
                editor.canvas_cache.clear();
            }
        }
        SymbolEditorMsg::Redo => {
            if editor.redo() {
                close_pickers(editor);
                // `undo()`/`redo()` own the dirty flag and the save point.
                editor.canvas_cache.clear();
            }
        }
        SymbolEditorMsg::DragCommit => {
            editor.mid_drag = false;
        }
        _ => {}
    }
}
