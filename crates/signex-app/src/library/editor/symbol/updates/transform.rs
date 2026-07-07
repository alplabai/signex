//! Symbol editor — rotate / delete / pin-field transform update logic.

use super::{SymEditor, mark_dirty, push_undo, rotate_pivot_msg_to_state};
use crate::library::messages::PrimitiveEditorMsg;

pub(super) fn apply_symbol_transform(editor: &mut SymEditor, msg: PrimitiveEditorMsg) {
    match msg {
        PrimitiveEditorMsg::SymbolRotateSelected { clockwise, pivot } => {
            push_undo(editor);
            let selected = editor.selected.clone();
            let pivot_mode = rotate_pivot_msg_to_state(pivot);
            crate::library::editor::symbol::state::rotate_selected_with_pivot(
                editor.primitive_mut(), selected, clockwise, pivot_mode,
            );
            mark_dirty(editor);
        }
        PrimitiveEditorMsg::SymbolDeleteSelected => {
            push_undo(editor);
            let selected = editor.selected.clone();
            if let Some(new_sel) = crate::library::editor::symbol::state::delete_selected(
                editor.primitive_mut(), selected,
            ) {
                editor.selected = new_sel;
                mark_dirty(editor);
            }
        }
        PrimitiveEditorMsg::SymbolSetPinNumber { idx, number } => {
            push_undo(editor);
            if let Some(pin) = editor.primitive_mut().pins.get_mut(idx) {
                pin.number = number;
                editor.dirty = true;
            }
        }
        PrimitiveEditorMsg::SymbolSetPinName { idx, name } => {
            push_undo(editor);
            if let Some(pin) = editor.primitive_mut().pins.get_mut(idx) {
                pin.name = name;
                editor.dirty = true;
            }
        }
        _ => {}
    }
}
