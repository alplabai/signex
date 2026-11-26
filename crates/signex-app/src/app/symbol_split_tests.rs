//! #2974: the symbol editor is a draft plus a GUI view. Undo and Redo step
//! the draft and touch the view exactly as before the split.

use std::path::PathBuf;

use iced::Point;
use signex_library::{Symbol, SymbolFile};

use super::{SymbolDocument, SymbolViewRequest};
use crate::library::editor::symbol::state::SymbolSelection;

fn edited_editor() -> SymbolDocument {
    let mut editor = SymbolDocument::new(
        PathBuf::from("Lib.snxsym"),
        SymbolFile::from_symbol(Symbol::empty("Sym1")),
    );
    editor.record_edit(None, |e| {
        e.primitive_mut().name = "Renamed".into();
        e.mark_dirty();
        true
    });
    editor
}

/// A panned, selecting, mid-drag view with no camera request pending.
fn touch_the_view(editor: &mut SymbolDocument) -> Point {
    let offset = Point::new(123.0, -45.0);
    editor.gui.camera.offset = offset;
    editor.gui.pending_view = None;
    editor.gui.selected = Some(SymbolSelection::Pin(0));
    editor.gui.mid_drag = true;
    offset
}

#[test]
fn undo_on_the_same_symbol_clears_the_selection_and_drag_but_keeps_the_camera() {
    let mut editor = edited_editor();
    let offset = touch_the_view(&mut editor);

    assert!(editor.undo());

    assert_eq!(editor.primitive().name, "Sym1", "the content stepped back");
    assert_eq!(editor.gui.selected, None);
    assert!(!editor.gui.mid_drag);
    assert_eq!(
        editor.gui.camera.offset, offset,
        "the pan survives the step"
    );
    assert_eq!(editor.gui.pending_view, None, "no refit was queued");
}

#[test]
fn redo_on_the_same_symbol_touches_the_view_like_undo() {
    let mut editor = edited_editor();
    assert!(editor.undo());
    let offset = touch_the_view(&mut editor);

    assert!(editor.redo());

    assert_eq!(editor.primitive().name, "Renamed");
    assert_eq!(editor.gui.selected, None);
    assert!(!editor.gui.mid_drag);
    assert_eq!(editor.gui.camera.offset, offset);
    assert_ne!(
        editor.gui.pending_view,
        Some(SymbolViewRequest::CenterOrigin)
    );
}

#[test]
fn each_history_step_moves_the_generation_once_and_leaves_the_save_stamp() {
    let mut editor = edited_editor();
    editor.draft.save_generation = 7;
    let before = editor.draft.history_generation;

    assert!(editor.undo());
    assert_eq!(editor.draft.history_generation, before + 1);
    assert!(editor.redo());
    assert_eq!(editor.draft.history_generation, before + 2);
    assert_eq!(editor.draft.save_generation, 7, "only a save stamps it");
}
