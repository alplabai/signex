//! Symbol editor — rotate / delete / pin-field transform update logic.

use super::{
    SymEditor, close_pickers, mark_dirty, push_undo, push_undo_snapshot, rotate_pivot_msg_to_state,
};
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
        SymbolEditorMsg::AlignSelectedToGrid => {
            // #426/#477 — mirrors the footprint editor's
            // `ActiveBarAlignSelectionToGrid`: gate the undo snapshot
            // on the selection actually being alignable (None/Field
            // are clean no-ops), same discipline as DeleteSelected
            // above, then snap onto the exact grid the canvas already
            // places/drags onto. `All`/`Multiple` are alignable-shaped
            // even when there's nothing to actually snap (e.g. Ctrl+A
            // on an empty symbol), so mark_dirty — and the undo push
            // itself — are gated on align_selected_to_grid's own "did
            // anything move" return value, not just the shape of the
            // selection.
            //
            // Snapshot BEFORE mutating and commit it with
            // `push_undo_snapshot` only once `changed` is known, rather
            // than `push_undo` (which clears `redo_snapshots`
            // unconditionally) followed by a pop on the no-op path: the
            // pop used to restore `undo_snapshots` but never restored
            // `redo_snapshots`, so a no-op align (e.g. Select-All on an
            // empty symbol, or a selection already on-grid) silently
            // destroyed the user's redo stack.
            let selected = editor.selected.clone();
            if crate::library::editor::symbol::state::selected_is_alignable(&selected) {
                let snapshot = editor.primitive().clone();
                let changed = crate::library::editor::symbol::state::align_selected_to_grid(
                    editor.primitive_mut(),
                    &selected,
                    crate::library::editor::symbol::canvas::SNAP_GRID_MM,
                );
                if changed {
                    push_undo_snapshot(editor, snapshot);
                    mark_dirty(editor);
                }
            }
            editor.active_bar_menu = None;
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

    fn new_editor() -> SymbolEditorState {
        SymbolEditorState::new(
            PathBuf::from("t.snxsym"),
            SymbolFile::from_symbol(Symbol::empty("T")),
        )
    }

    /// #426 — no selection is a clean no-op: no dirty flag, no undo
    /// snapshot. Mirrors the footprint editor's
    /// `issue_146_align_to_grid_with_no_selection_stays_clean`.
    #[test]
    fn align_selected_to_grid_with_no_selection_stays_clean() {
        let mut editor = new_editor();
        editor.selected = None;

        apply_symbol_transform(&mut editor, SymbolEditorMsg::AlignSelectedToGrid);

        assert!(!editor.dirty, "no-op align must not dirty the document");
        assert!(
            editor.undo_snapshots.is_empty(),
            "no-op align must not stack undo history"
        );
    }

    /// #426 — a real selection snaps onto the 1.27 mm symbol-canvas
    /// grid and dirties/snapshots exactly once (dispatch test, driving
    /// the message through the same `apply_symbol_transform` entry
    /// point the active bar uses).
    #[test]
    fn align_selected_to_grid_snaps_the_selected_pin_and_dirties_once() {
        let mut sym = Symbol::empty("T");
        let idx = crate::library::editor::symbol::state::add_pin(&mut sym, 1.0, 1.0, 1);
        let mut editor =
            SymbolEditorState::new(PathBuf::from("t.snxsym"), SymbolFile::from_symbol(sym));
        editor.selected = Some(SymbolSelection::Pin(idx));
        editor.redo_snapshots.push(editor.primitive().clone());

        apply_symbol_transform(&mut editor, SymbolEditorMsg::AlignSelectedToGrid);

        assert!(editor.dirty, "a real align dirties the document");
        assert_eq!(
            editor.undo_snapshots.len(),
            1,
            "exactly one undo snapshot per real align (no double-push)"
        );
        assert!(
            editor.redo_snapshots.is_empty(),
            "a real align still clears the redo stack, same as every other mutation"
        );
        assert_eq!(
            editor.primitive().pins[idx].position,
            [1.27, 1.27],
            "pin lands on the 1.27 mm snap grid"
        );
    }

    /// #477 — `All` is alignable-shaped even on an empty symbol
    /// (Ctrl+A with nothing placed yet), but `align_selected_to_grid`
    /// snaps zero pins/graphics, so the handler must not leave a
    /// spurious undo snapshot or dirty flag behind — and, the actual
    /// bug, must not destroy the redo stack either. `push_undo` clears
    /// `redo_snapshots` unconditionally; the earlier fix popped the
    /// undo snapshot back off on a no-op but never restored
    /// `redo_snapshots`, so an ordinary no-op Align-To-Grid click (e.g.
    /// Ctrl+A on an empty symbol, or a selection already on-grid)
    /// silently wiped the user's redo history. Seed one redo entry up
    /// front and assert it survives the no-op.
    #[test]
    fn align_selected_to_grid_with_all_selection_on_empty_symbol_stays_clean() {
        let mut editor = new_editor();
        editor.selected = Some(SymbolSelection::All);
        editor.redo_snapshots.push(editor.primitive().clone());

        apply_symbol_transform(&mut editor, SymbolEditorMsg::AlignSelectedToGrid);

        assert!(
            !editor.dirty,
            "no-op align on an empty symbol must not dirty the document"
        );
        assert!(
            editor.undo_snapshots.is_empty(),
            "no-op align on an empty symbol must not stack undo history"
        );
        assert_eq!(
            editor.redo_snapshots.len(),
            1,
            "no-op align must not clear the redo stack"
        );
    }

    /// #426 — `All` is alignable (unlike Delete, snapping never
    /// destroys data), so a full-symbol Align To Grid actually snaps
    /// every pin and graphic rather than silently no-op'ing.
    #[test]
    fn align_selected_to_grid_with_all_selection_snaps_every_pin() {
        let mut sym = Symbol::empty("T");
        crate::library::editor::symbol::state::add_pin(&mut sym, 1.0, 1.0, 1);
        crate::library::editor::symbol::state::add_pin(&mut sym, 2.5, 2.5, 1);
        let mut editor =
            SymbolEditorState::new(PathBuf::from("t.snxsym"), SymbolFile::from_symbol(sym));
        editor.selected = Some(SymbolSelection::All);

        apply_symbol_transform(&mut editor, SymbolEditorMsg::AlignSelectedToGrid);

        assert!(editor.dirty);
        assert_eq!(editor.primitive().pins[0].position, [1.27, 1.27]);
        assert_eq!(editor.primitive().pins[1].position, [2.54, 2.54]);
    }
}
