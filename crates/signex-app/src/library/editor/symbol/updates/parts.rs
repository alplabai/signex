//! Symbol editor — multi-part (prev/next/new/remove) update logic.

use super::{SymEditor, close_pickers, commit_or_discard_polygon, mark_dirty, push_undo};
use crate::library::messages::SymbolEditorMsg;

pub(super) fn apply_symbol_parts(editor: &mut SymEditor, msg: SymbolEditorMsg) {
    // Footprint/SetTool parity: flush the in-flight Place Polygon
    // stash BEFORE active_part changes — synchronously, here, not
    // deferred to a later event. Without this, a staged vertex click
    // sequence commits later (PolygonCommit / the next SetTool)
    // reading `editor.active_part` at THAT point instead of the part
    // the user was actually drawing on, landing the shape on the
    // wrong (possibly now-hidden) unit — invisible data loss, not a
    // crash. Safe to call unconditionally for all four variants below:
    // a no-op when the stash is already empty.
    commit_or_discard_polygon(editor);

    match msg {
        SymbolEditorMsg::PrevPart => {
            if editor.active_part > 1 {
                editor.active_part -= 1;
                // Drop any selection so it can't dangle on a graphic that
                // just became hidden on the newly-active unit.
                editor.selected = None;
                close_pickers(editor);
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
                close_pickers(editor);
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
            close_pickers(editor);
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
            close_pickers(editor);
            mark_dirty(editor);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::editor::symbol::canvas::SymbolTool;
    use signex_library::{Symbol, SymbolFile, SymbolGraphicKind};
    use std::path::PathBuf;

    fn new_editor() -> SymEditor {
        SymEditor::new(
            PathBuf::from("t.snxsym"),
            SymbolFile::from_symbol(Symbol::empty("T")),
        )
    }

    /// A staged (>= 3 vertex) Place Polygon click sequence on part 1
    /// commits BEFORE `NextPart` switches the active part, so the
    /// polygon lands on part 1 (the part it was actually drawn on),
    /// not silently landing invisible on part 2 by reading
    /// `active_part` at some later commit point.
    #[test]
    fn next_part_flushes_a_staged_polygon_onto_the_part_it_was_drawn_on() {
        let mut editor = new_editor();
        assert_eq!(editor.active_part, 1);
        editor.tool = SymbolTool::PlacePolygon;
        editor.polygon_vertices = vec![(0.0, 0.0), (4.0, 0.0), (2.0, 3.0)];
        // A second part must exist for NextPart to actually switch.
        editor.primitive_mut().part_count = 2;

        apply_symbol_parts(&mut editor, SymbolEditorMsg::NextPart);

        assert_eq!(editor.active_part, 2, "part switch still happens");
        assert!(editor.polygon_vertices.is_empty(), "stash flushed");
        assert_eq!(editor.primitive().graphics.len(), 1);
        match &editor.primitive().graphics[0].kind {
            SymbolGraphicKind::Polygon { vertices } => assert_eq!(vertices.len(), 3),
            other => panic!("expected Polygon, got {other:?}"),
        }
        assert_eq!(
            editor.primitive().graphics[0].part_number,
            1,
            "polygon lands on the part it was staged on, not the new active part"
        );
    }

    /// Same flush, but for `PrevPart` / `NewPart` / `RemovePart` too —
    /// all four multi-part messages share the same top-of-function
    /// flush, not just `NextPart`.
    #[test]
    fn new_part_flushes_a_staged_polygon_onto_the_part_it_was_drawn_on() {
        let mut editor = new_editor();
        editor.tool = SymbolTool::PlacePolygon;
        editor.polygon_vertices = vec![(0.0, 0.0), (4.0, 0.0), (2.0, 3.0)];

        apply_symbol_parts(&mut editor, SymbolEditorMsg::NewPart);

        assert_eq!(
            editor.active_part, 2,
            "a new part was created and activated"
        );
        assert!(editor.polygon_vertices.is_empty(), "stash flushed");
        assert_eq!(editor.primitive().graphics.len(), 1);
        assert_eq!(
            editor.primitive().graphics[0].part_number,
            1,
            "polygon lands on part 1, where it was staged"
        );
    }

    /// A short (< 3 vertex) stash discards silently on part switch —
    /// no graphic, no undo entry — same as `SetTool`'s flush.
    #[test]
    fn next_part_discards_a_short_staged_polygon() {
        let mut editor = new_editor();
        editor.tool = SymbolTool::PlacePolygon;
        editor.polygon_vertices = vec![(0.0, 0.0), (4.0, 0.0)];
        editor.primitive_mut().part_count = 2;

        apply_symbol_parts(&mut editor, SymbolEditorMsg::NextPart);

        assert_eq!(editor.active_part, 2);
        assert!(editor.polygon_vertices.is_empty());
        assert!(editor.primitive().graphics.is_empty());
    }
}
