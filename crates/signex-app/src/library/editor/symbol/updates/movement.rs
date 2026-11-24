//! Symbol editor — move / drag update logic (coalesced undo per gesture).

use super::{SymEditor, begin_drag_if_needed, graphic_handle_msg_to_state, mark_dirty};
use crate::library::messages::SymbolEditorMsg;
use signex_library::SymbolGraphicKind;

pub(super) fn apply_symbol_move(editor: &mut SymEditor, msg: SymbolEditorMsg) {
    use crate::library::editor::symbol::state::SymbolSelection;
    // A click that never moved a field must not store a position, push an
    // undo entry or dirty the document.
    if let (SymbolEditorMsg::MoveSelected { x, y }, Some(SymbolSelection::Field(key))) =
        (&msg, &editor.selected)
        && crate::library::editor::symbol::state::field_move_is_noop(
            editor.primitive(),
            *key,
            editor.view(),
            *x,
            *y,
        )
    {
        return;
    }
    if !editor.mid_drag {
        editor.drag_corner_radius = None;
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
                crate::library::editor::symbol::state::move_multiple(
                    editor.primitive_mut(),
                    &pins,
                    &graphics,
                    dx,
                    dy,
                );
            }
            _ => {
                let view = editor.view();
                crate::library::editor::symbol::state::move_all(
                    editor.primitive_mut(),
                    view,
                    dx,
                    dy,
                );
            }
        },
        SymbolEditorMsg::MoveGraphicHandle { idx, handle, x, y } => {
            let h = graphic_handle_msg_to_state(handle);
            rewind_drag_radius(editor, idx);
            crate::library::editor::symbol::state::move_graphic_handle(
                editor.primitive_mut(),
                idx,
                h,
                x,
                y,
            );
            note_drag_radius(editor, idx);
        }
        _ => {}
    }
    mark_dirty(editor);
}

fn stored_corner_radius(editor: &SymEditor, idx: usize) -> Option<f64> {
    match editor.primitive().graphics.get(idx)?.kind {
        SymbolGraphicKind::RoundRectangle { corner_radius, .. } => Some(corner_radius),
        _ => None,
    }
}

/// Put a dragged Round Rectangle back on its drag-start radius, so the
/// clamp that follows the move depends only on the new size.
fn rewind_drag_radius(editor: &mut SymEditor, idx: usize) {
    let Some(stored) = stored_corner_radius(editor, idx) else {
        editor.drag_corner_radius = None;
        return;
    };
    let start = match editor.drag_corner_radius {
        Some((i, start, last)) if i == idx && last == stored => start,
        _ => stored,
    };
    editor.drag_corner_radius = Some((idx, start, stored));
    if let Some(SymbolGraphicKind::RoundRectangle { corner_radius, .. }) = editor
        .primitive_mut()
        .graphics
        .get_mut(idx)
        .map(|g| &mut g.kind)
    {
        *corner_radius = start;
    }
}

/// Record the radius this tick wrote.
fn note_drag_radius(editor: &mut SymEditor, idx: usize) {
    let written = stored_corner_radius(editor, idx);
    if let (Some((_, _, last)), Some(written)) = (editor.drag_corner_radius.as_mut(), written) {
        *last = written;
    }
}
