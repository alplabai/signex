//! #2326: a drag in the symbol editor is one undo step, the "open move
//! run". These pin when that run opens and closes, through the editor's
//! own messages, so moving the marker between structs changes nothing.

use std::path::PathBuf;

use signex_library::{Symbol, SymbolFile};

use super::apply_symbol_primitive_edit;
use crate::app::SymbolDocument;
use crate::library::editor::symbol::SymbolSelection;
use crate::library::messages::{SymbolEditorMsg, SymbolRotatePivotMsg};

/// An editor with one pin off the origin, selected, so Rotate moves it.
fn editor_with_selected_pin() -> SymbolDocument {
    let mut symbol = Symbol::empty("Sym1");
    let mut pin = signex_library::SymbolPin::new("1", "PIN1");
    pin.position = [2.54, 0.0];
    symbol.pins.push(pin);
    let mut editor =
        SymbolDocument::new(PathBuf::from("Lib.snxsym"), SymbolFile::from_symbol(symbol));
    editor.gui.selected = Some(SymbolSelection::Pin(0));
    editor
}

fn tick(editor: &mut SymbolDocument) {
    apply_symbol_primitive_edit(editor, SymbolEditorMsg::MoveAll { dx: 2.54, dy: 0.0 });
}

fn undo_depth(editor: &SymbolDocument) -> usize {
    editor.draft.history.undo_len()
}

#[test]
fn the_first_movement_tick_records_one_step_and_later_ticks_none_until_the_commit() {
    let mut editor = editor_with_selected_pin();

    tick(&mut editor);
    assert_eq!(undo_depth(&editor), 1, "the first tick snapshots once");
    tick(&mut editor);
    tick(&mut editor);
    assert_eq!(undo_depth(&editor), 1, "later ticks join the open run");

    apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::DragCommit);
    tick(&mut editor);
    assert_eq!(undo_depth(&editor), 2, "the commit closed the run");
}

#[test]
fn undo_during_a_drag_closes_the_run_so_the_next_tick_records_a_new_step() {
    let mut editor = editor_with_selected_pin();
    tick(&mut editor);

    apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::Undo);
    assert_eq!(undo_depth(&editor), 0);
    tick(&mut editor);

    assert_eq!(undo_depth(&editor), 1, "the same drag opened a new run");
}

#[test]
fn a_tick_after_a_redo_records_a_new_step() {
    let mut editor = editor_with_selected_pin();
    tick(&mut editor);
    apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::DragCommit);
    apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::Undo);
    tick(&mut editor);
    apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::Undo);
    assert_eq!(undo_depth(&editor), 0);

    apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::Redo);
    assert_eq!(undo_depth(&editor), 1);
    tick(&mut editor);

    assert_eq!(undo_depth(&editor), 2, "the redo closed the run");
}

#[test]
fn redo_with_nothing_to_redo_during_a_drag_leaves_the_run_open() {
    let mut editor = editor_with_selected_pin();
    tick(&mut editor);

    apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::Redo);

    assert!(editor.draft.is_moving(), "no step, so the run stays open");
    tick(&mut editor);
    assert_eq!(undo_depth(&editor), 1, "the drag still joins its step");
}

#[test]
fn a_redo_step_during_an_open_run_closes_it() {
    let mut editor = editor_with_selected_pin();
    tick(&mut editor);
    apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::Undo);
    // A movement tick would clear the redo stack, so the run is opened
    // directly to put an open run and a redo step side by side.
    editor.draft.begin_move_run();

    apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::Redo);

    assert_eq!(undo_depth(&editor), 1, "the redo step was applied");
    assert!(!editor.draft.is_moving(), "the step closed the run");
}

#[test]
fn a_drag_that_ends_without_a_commit_leaves_the_next_drag_in_the_same_step() {
    let mut editor = editor_with_selected_pin();
    tick(&mut editor);

    // No DragCommit: the second gesture starts with the run still open.
    tick(&mut editor);

    assert_eq!(undo_depth(&editor), 1, "no new snapshot for the next drag");
}

#[test]
fn a_rotate_during_a_drag_records_its_step_but_leaves_the_run_open() {
    let mut editor = editor_with_selected_pin();
    tick(&mut editor);

    apply_symbol_primitive_edit(
        &mut editor,
        SymbolEditorMsg::RotateSelected {
            clockwise: true,
            pivot: SymbolRotatePivotMsg::WorldOrigin,
        },
    );
    assert_eq!(undo_depth(&editor), 2, "the rotate is its own step");
    tick(&mut editor);

    assert_eq!(undo_depth(&editor), 2, "the run stayed open");
}
