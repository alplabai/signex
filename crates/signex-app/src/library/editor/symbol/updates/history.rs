//! Symbol editor — undo / redo / drag-commit update logic.

use super::{SymEditor, close_pickers, mark_dirty};
use crate::library::messages::SymbolEditorMsg;

pub(super) fn apply_symbol_history(editor: &mut SymEditor, msg: SymbolEditorMsg) {
    match msg {
        SymbolEditorMsg::Undo => {
            if let Some(snapshot) = editor.undo_snapshots.pop() {
                let current = editor.primitive().clone();
                editor.redo_snapshots.push(current);
                *editor.primitive_mut() = snapshot;
                editor.mid_drag = false;
                editor.selected = None;
                close_pickers(editor);
                clamp_active_part(editor);
                mark_dirty(editor);
            }
        }
        SymbolEditorMsg::Redo => {
            if let Some(snapshot) = editor.redo_snapshots.pop() {
                let current = editor.primitive().clone();
                editor.undo_snapshots.push(current);
                *editor.primitive_mut() = snapshot;
                editor.mid_drag = false;
                editor.selected = None;
                close_pickers(editor);
                clamp_active_part(editor);
                mark_dirty(editor);
            }
        }
        SymbolEditorMsg::DragCommit => {
            editor.mid_drag = false;
        }
        _ => {}
    }
}

/// Re-clamp the editor's `active_part` into `1..=max_part_number`
/// after a snapshot restore. `active_part` lives on the editor state,
/// not inside the `Symbol` snapshot, so undo/redo can otherwise leave
/// it pointing past the restored unit count (e.g. undoing a New Part).
fn clamp_active_part(editor: &mut SymEditor) {
    let max = crate::library::editor::symbol::state::max_part_number(editor.primitive());
    editor.active_part = editor.active_part.clamp(1, max);
}
