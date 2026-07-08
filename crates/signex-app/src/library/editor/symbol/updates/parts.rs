//! Symbol editor — multi-part (prev/next/new/remove) update logic.

use super::{SymEditor, mark_dirty, push_undo};
use crate::library::messages::PrimitiveEditorMsg;

pub(super) fn apply_symbol_parts(editor: &mut SymEditor, msg: PrimitiveEditorMsg) {
    match msg {
        PrimitiveEditorMsg::SymbolPrevPart => {
            if editor.active_part > 1 {
                editor.active_part -= 1;
                editor.canvas_cache.clear();
            }
        }
        PrimitiveEditorMsg::SymbolNextPart => {
            let max = crate::library::editor::symbol::state::max_part_number(editor.primitive());
            if editor.active_part < max {
                editor.active_part += 1;
                editor.canvas_cache.clear();
            }
        }
        PrimitiveEditorMsg::SymbolNewPart => {
            push_undo(editor);
            let new_part =
                crate::library::editor::symbol::state::max_part_number(editor.primitive()) + 1;
            editor.active_part = new_part;
            mark_dirty(editor);
        }
        PrimitiveEditorMsg::SymbolRemovePart => {
            let max = crate::library::editor::symbol::state::max_part_number(editor.primitive());
            if max <= 1 || editor.active_part <= 1 {
                tracing::debug!(
                    target: "signex::library",
                    active = editor.active_part,
                    max,
                    "SymbolRemovePart: refusing to remove the only part"
                );
                return;
            }
            push_undo(editor);
            let to_remove = editor.active_part;
            crate::library::editor::symbol::state::demote_part_pins_to_part_one(
                editor.primitive_mut(),
                to_remove,
            );
            editor.active_part = 1;
            mark_dirty(editor);
        }
        _ => {}
    }
}
