//! Symbol editor — rotate / delete / pin-field transform update logic.

use super::{SymEditor, mark_dirty, push_undo, rotate_pivot_msg_to_state};
use crate::library::messages::SymbolEditorMsg;

pub(super) fn apply_symbol_transform(editor: &mut SymEditor, msg: SymbolEditorMsg) {
    match msg {
        SymbolEditorMsg::RotateSelected { clockwise, pivot } => {
            push_undo(editor);
            let selected = editor.selected.clone();
            let pivot_mode = rotate_pivot_msg_to_state(pivot);
            crate::library::editor::symbol::state::rotate_selected_with_pivot(
                editor.primitive_mut(),
                selected,
                clockwise,
                pivot_mode,
            );
            mark_dirty(editor);
        }
        SymbolEditorMsg::DeleteSelected => {
            push_undo(editor);
            let selected = editor.selected.clone();
            if let Some(new_sel) = crate::library::editor::symbol::state::delete_selected(
                editor.primitive_mut(),
                selected,
            ) {
                editor.selected = new_sel;
                mark_dirty(editor);
            }
        }
        SymbolEditorMsg::SetPinNumber { idx, number } => {
            push_undo(editor);
            if let Some(pin) = editor.primitive_mut().pins.get_mut(idx) {
                pin.number = number;
                editor.dirty = true;
            }
        }
        SymbolEditorMsg::SetPinName { idx, name } => {
            push_undo(editor);
            if let Some(pin) = editor.primitive_mut().pins.get_mut(idx) {
                pin.name = name;
                editor.dirty = true;
            }
        }
        _ => {}
    }
}
