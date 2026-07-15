//! Symbol editor — multi-part (prev/next/new/remove) update logic.

use super::{SymEditor, mark_dirty, push_undo};
use crate::library::messages::SymbolEditorMsg;

pub(super) fn apply_symbol_parts(editor: &mut SymEditor, msg: SymbolEditorMsg) {
    match msg {
        SymbolEditorMsg::PrevPart => {
            if editor.active_part > 1 {
                editor.active_part -= 1;
                // Drop any selection so it can't dangle on a graphic that
                // just became hidden on the newly-active unit.
                editor.selected = None;
                editor.canvas_cache.clear();
            }
        }
        SymbolEditorMsg::NextPart => {
            let max = crate::library::editor::symbol::state::max_part_number(editor.primitive());
            if editor.active_part < max {
                editor.active_part += 1;
                // Drop any selection so it can't dangle on a graphic that
                // just became hidden on the newly-active unit.
                editor.selected = None;
                editor.canvas_cache.clear();
            }
        }
        SymbolEditorMsg::NewPart => {
            push_undo(editor);
            let new_part =
                crate::library::editor::symbol::state::max_part_number(editor.primitive())
                    .saturating_add(1);
            // Persist the unit so an empty part (no pins yet) survives
            // navigate + save — the count is now stored, not derived.
            editor.primitive_mut().part_count = new_part;
            editor.active_part = new_part;
            // Drop any selection so a stale index can't act (via keyboard
            // Delete / Rotate) on a graphic hidden by the unit switch.
            editor.selected = None;
            mark_dirty(editor);
        }
        SymbolEditorMsg::RemovePart => {
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
            let new_active = crate::library::editor::symbol::state::delete_unit(
                editor.primitive_mut(),
                to_remove,
            );
            editor.active_part = new_active;
            // Drop any selection so a stale index can't act on geometry
            // shifted or hidden by the delete + renumber.
            editor.selected = None;
            mark_dirty(editor);
        }
        _ => {}
    }
}
