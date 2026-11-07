//! #661 slice 6: net colours are steps of the one schematic undo timeline.

use super::net_color_driver::*;
use signex_types::schematic::{SelectedItem, SelectedKind};

fn move_symbol(app: &mut crate::app::Signex, id: uuid::Uuid) {
    let moved = app.apply_engine_command(
        signex_engine::Command::MoveSelection {
            items: vec![SelectedItem::new(id, SelectedKind::Symbol)],
            dx: 10.0,
            dy: 0.0,
            keep_wires: false,
        },
        true,
        true,
    );
    assert!(moved, "fixture: the move must change the document");
}

#[test]
fn undoing_a_colour_back_to_the_saved_colours_leaves_the_sheet_clean() {
    let (mut app, path) = app_with_sheet("timeline-clean.snxsch");
    app.document_state.mark_document_clean(&path);
    let wire = wire_of(&app, &path);

    paint(&mut app, &[wire], color(1));
    assert!(is_dirty(&app, &path), "a colour is an unsaved edit");

    app.handle_undo_requested();
    assert!(!is_dirty(&app, &path), "back at the save point");

    app.handle_redo_requested();
    assert!(is_dirty(&app, &path), "redo leaves the save point again");
}

#[test]
fn colour_steps_share_the_undo_depth_and_fall_off_oldest_first() {
    let (mut app, path) = app_with_sheet("timeline-depth.snxsch");
    app.set_undo_depth(2);
    let (wire, symbol) = (wire_of(&app, &path), symbol_of(&app, &path));

    paint(&mut app, &[wire], color(1)); // the oldest step
    move_symbol(&mut app, symbol);
    move_symbol(&mut app, symbol);

    app.handle_undo_requested();
    app.handle_undo_requested();
    app.handle_undo_requested(); // nothing left to undo

    assert_eq!(
        colour_of(&app, wire),
        Some(color(1)),
        "the colour step fell off the end of the two-step history"
    );
}

#[test]
fn a_colour_step_stays_ordered_against_edits_through_undo_and_redo() {
    let (mut app, path) = app_with_sheet("timeline-order.snxsch");
    let (wire, symbol) = (wire_of(&app, &path), symbol_of(&app, &path));
    let start = app.document_state.engines[&path].document().symbols[0]
        .position
        .x;

    paint(&mut app, &[wire], color(1));
    move_symbol(&mut app, symbol);

    app.handle_undo_requested();
    assert_eq!(colour_of(&app, wire), Some(color(1)), "the move goes first");
    app.handle_undo_requested();
    assert_eq!(colour_of(&app, wire), None, "then the colour");
    app.handle_redo_requested();
    assert_eq!(colour_of(&app, wire), Some(color(1)));
    app.handle_redo_requested();
    let moved = app.document_state.engines[&path].document().symbols[0]
        .position
        .x;
    assert_ne!(moved, start);
}
