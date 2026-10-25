//! Symbol editor — undo / redo / drag-commit update logic.

use super::{SymEditor, close_pickers, mark_dirty};
use crate::library::messages::SymbolEditorMsg;

pub(super) fn apply_symbol_history(editor: &mut SymEditor, msg: SymbolEditorMsg) {
    match msg {
        SymbolEditorMsg::Undo => {
            if editor.undo() {
                close_pickers(editor);
                mark_dirty(editor);
            }
        }
        SymbolEditorMsg::Redo => {
            if editor.redo() {
                close_pickers(editor);
                mark_dirty(editor);
            }
        }
        SymbolEditorMsg::DragCommit => {
            editor.mid_drag = false;
        }
        _ => {}
    }
}
