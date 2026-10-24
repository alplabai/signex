//! Symbol editor — move / drag update logic (coalesced undo per gesture).

use super::{SymEditor, begin_drag_if_needed, graphic_handle_msg_to_state, mark_dirty};
use crate::library::messages::SymbolEditorMsg;

pub(super) fn apply_symbol_move(editor: &mut SymEditor, msg: SymbolEditorMsg) {
    use crate::library::editor::symbol::state::SymbolSelection;
    // A click that never moved a field must not store a position, push an
    // undo entry or dirty the document.
    if let (SymbolEditorMsg::MoveSelected { x, y }, Some(SymbolSelection::Field(key))) =
        (&msg, &editor.selected)
        && crate::library::editor::symbol::state::field_move_is_noop(
            editor.primitive(),
            *key,
            editor.active_part,
            *x,
            *y,
        )
    {
        return;
    }
    begin_drag_if_needed(editor);
    match msg {
        SymbolEditorMsg::MoveSelected { x, y } => {
            let selected = editor.selected.clone();
            crate::library::editor::symbol::state::move_selected(
                editor.primitive_mut(),
                selected,
                x,
                y,
            );
        }
        SymbolEditorMsg::MoveAll { dx, dy } => match &editor.selected {
            Some(SymbolSelection::Multiple {
                pin_indices,
                graphic_indices,
            }) => {
                let pins = pin_indices.clone();
                let graphics = graphic_indices.clone();
                let active_part = editor.active_part;
                crate::library::editor::symbol::state::move_multiple(
                    editor.primitive_mut(),
                    active_part,
                    &pins,
                    &graphics,
                    dx,
                    dy,
                );
            }
            _ => {
                crate::library::editor::symbol::state::move_all(editor.primitive_mut(), dx, dy);
            }
        },
        SymbolEditorMsg::MoveGraphicHandle { idx, handle, x, y } => {
            let h = graphic_handle_msg_to_state(handle);
            crate::library::editor::symbol::state::move_graphic_handle(
                editor.primitive_mut(),
                idx,
                h,
                x,
                y,
            );
        }
        _ => {}
    }
    mark_dirty(editor);
}
