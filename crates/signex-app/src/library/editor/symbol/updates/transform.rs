//! Symbol editor — rotate / delete / pin-field transform update logic.

use super::{SymEditor, close_pickers, mark_dirty, push_undo, rotate_pivot_msg_to_state};
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
            let selected = editor.selected.clone();
            // Validate before snapshotting: a no-op delete (None / All /
            // Field selection) must not push an undo entry that would
            // evict real history — same discipline as apply_symbol_join.
            if crate::library::editor::symbol::state::selected_is_deletable(&selected) {
                push_undo(editor);
                if let Some(new_sel) = crate::library::editor::symbol::state::delete_selected(
                    editor.primitive_mut(),
                    selected,
                ) {
                    editor.selected = new_sel;
                    close_pickers(editor);
                    mark_dirty(editor);
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{GraphicFillPicker, SymbolEditorState};
    use crate::library::editor::symbol::state::SymbolSelection;
    use signex_library::{Symbol, SymbolFile, SymbolGraphic, SymbolGraphicKind};
    use std::path::PathBuf;

    /// Deleting the selected graphic must close a fill picker that was
    /// open on it, so the transient picker state can't reopen on a
    /// different graphic that later reuses the freed index.
    #[test]
    fn delete_selected_closes_open_fill_picker() {
        let mut sym = Symbol::empty("T");
        sym.graphics.push(SymbolGraphic {
            kind: SymbolGraphicKind::Rectangle {
                from: [0.0, 0.0],
                to: [2.0, 1.0],
            },
            stroke_width: 0.15,
            fill: Some([220, 60, 60, 255]),
            part_number: 0,
        });
        let file = SymbolFile::from_symbol(sym);
        let mut editor = SymbolEditorState::new(PathBuf::from("t.snxsym"), file);
        editor.selected = Some(SymbolSelection::Graphic(0));
        editor.graphic_fill_picker = Some(GraphicFillPicker {
            idx: 0,
            advanced: true,
        });

        apply_symbol_transform(&mut editor, SymbolEditorMsg::DeleteSelected);

        assert!(
            editor.primitive().graphics.is_empty(),
            "graphic should be deleted"
        );
        assert!(
            editor.graphic_fill_picker.is_none(),
            "fill picker must close when its graphic is deleted"
        );
    }
}
