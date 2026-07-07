//! Symbol editor — undo / redo / drag-commit update logic.

use super::{SymEditor, mark_dirty};
use crate::library::messages::PrimitiveEditorMsg;

pub(super) fn apply_symbol_history(editor: &mut SymEditor, msg: PrimitiveEditorMsg) {
    match msg {
        PrimitiveEditorMsg::SymbolUndo => {
            if let Some(snapshot) = editor.undo_snapshots.pop() {
                let current = editor.primitive().clone();
                editor.redo_snapshots.push(current);
                *editor.primitive_mut() = snapshot;
                editor.mid_drag = false;
                editor.selected = None;
                mark_dirty(editor);
            }
        }
        PrimitiveEditorMsg::SymbolRedo => {
            if let Some(snapshot) = editor.redo_snapshots.pop() {
                let current = editor.primitive().clone();
                editor.undo_snapshots.push(current);
                *editor.primitive_mut() = snapshot;
                editor.mid_drag = false;
                editor.selected = None;
                mark_dirty(editor);
            }
        }
        PrimitiveEditorMsg::SymbolDragCommit => {
            editor.mid_drag = false;
        }
        _ => {}
    }
}
