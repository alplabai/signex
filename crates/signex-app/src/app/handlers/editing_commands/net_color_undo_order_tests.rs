//! #102 — Ctrl+Z / Ctrl+Y interleave net-colour floods and engine edits in
//! true chronological order (#661: colours are engine steps, so the engine
//! timeline alone orders them).

use super::net_color_driver::*;
use crate::app::Signex;
use signex_types::schematic::{SelectedItem, SelectedKind};
use std::path::{Path, PathBuf};

const A: &str = "undo-order.snxsch";
const B: &str = "b.snxsch";

fn app_with_sheet_a() -> (Signex, PathBuf) {
    app_with_sheet(A)
}

fn junction_of(app: &Signex, path: &Path) -> uuid::Uuid {
    app.document_state.engines[path].document().junctions[0].uuid
}

fn move_symbol(app: &mut Signex, id: uuid::Uuid) {
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

fn symbol_x(app: &Signex, path: &Path) -> f64 {
    app.document_state.engines[path].document().symbols[0]
        .position
        .x
}

/// Flood a colour, then move a symbol: Ctrl+Z reverts the move first and
/// the colour second; Ctrl+Y replays them in the opposite order.
#[test]
fn undo_and_redo_follow_chronological_order() {
    let (mut app, path) = app_with_sheet_a();
    let (wire, id) = (wire_of(&app, &path), symbol_of(&app, &path));
    let start = symbol_x(&app, &path);

    paint(&mut app, &[wire], color(9));
    move_symbol(&mut app, id);

    app.handle_undo_requested();
    assert_eq!(
        symbol_x(&app, &path),
        start,
        "first undo reverts the newer move"
    );
    assert!(colour_of(&app, wire).is_some());

    app.handle_undo_requested();
    assert_eq!(
        colour_of(&app, wire),
        None,
        "second undo reverts the colour"
    );

    app.handle_redo_requested();
    assert!(colour_of(&app, wire).is_some(), "redo replays the colour");
    assert_eq!(symbol_x(&app, &path), start);

    app.handle_redo_requested();
    assert_eq!(symbol_x(&app, &path), start + 10.0, "then the move");
}

/// A new engine edit ends the colour redo branch.
#[test]
fn a_new_engine_edit_clears_colour_redo() {
    let (mut app, path) = app_with_sheet_a();
    let (wire, id) = (wire_of(&app, &path), symbol_of(&app, &path));
    paint(&mut app, &[wire], color(1));
    app.handle_undo_requested();
    assert!(app.document_state.engines[&path].can_redo());

    move_symbol(&mut app, id);
    assert!(!app.document_state.engines[&path].can_redo());
    app.handle_redo_requested();
    assert_eq!(colour_of(&app, wire), None, "the colour is gone for good");
}

/// A colour on A is untouched by Ctrl+Z in B; switching back undoes it.
#[test]
fn colour_steps_belong_to_their_own_document() {
    let (mut app, a) = app_with_sheet_a();
    let b = PathBuf::from(B);
    add_sheet(&mut app, &b);
    activate(&mut app, &a);
    let wire = wire_of(&app, &a);
    paint(&mut app, &[wire], color(9));

    activate(&mut app, &b);
    let b_symbol = symbol_of(&app, &b);
    move_symbol(&mut app, b_symbol);
    app.handle_undo_requested();
    assert_eq!(symbol_x(&app, &b), 0.0, "B's own edit is undone");
    assert!(colour_of(&app, wire).is_some(), "A's colour is untouched");
    app.handle_undo_requested();
    assert!(colour_of(&app, wire).is_some());

    activate(&mut app, &a);
    app.handle_undo_requested();
    assert_eq!(colour_of(&app, wire), None);
}

/// Redo availability and effect are per document.
#[test]
fn colour_redo_is_enabled_only_for_its_document() {
    let (mut app, a) = app_with_sheet_a();
    let b = PathBuf::from(B);
    add_sheet(&mut app, &b);
    activate(&mut app, &a);
    let wire = wire_of(&app, &a);
    paint(&mut app, &[wire], color(1));
    app.handle_undo_requested();
    assert!(app.document_state.engines[&a].can_redo());

    activate(&mut app, &b);
    assert!(!app.document_state.engines[&b].can_redo());
    app.handle_redo_requested();
    assert_eq!(colour_of(&app, wire), None, "no-op on B");
}

/// Replacing a document's engine drops that document's colour steps only.
#[test]
fn reload_clears_that_documents_colour_steps() {
    let (mut app, a) = app_with_sheet_a();
    let b = PathBuf::from(B);
    add_sheet(&mut app, &b);
    let (wire_a, wire_b) = (wire_of(&app, &a), wire_of(&app, &b));
    paint_on(&mut app, &a, &[wire_a], color(1));
    paint_on(&mut app, &b, &[wire_b], color(2));

    assert!(app.install_schematic_at(&a, fixture_sheet()));

    assert!(!app.document_state.engines[&a].can_undo());
    assert!(app.document_state.engines[&b].can_undo());
}

/// #1005 — colour on A, colour on B, Ctrl+Z in A reverts only A's colour.
#[test]
fn undo_on_one_sheet_keeps_another_sheets_colour() {
    let (mut app, a) = app_with_sheet_a();
    let wire_a = wire_of(&app, &a);
    paint(&mut app, &[wire_a], color(1));

    let b = PathBuf::from(B);
    add_sheet(&mut app, &b);
    let wire_b = wire_of(&app, &b);
    paint(&mut app, &[wire_b], color(2));

    activate(&mut app, &a);
    app.handle_undo_requested();
    assert_eq!(colour_of(&app, wire_a), None);
    assert_eq!(colour_of(&app, wire_b), Some(color(2)));

    app.handle_redo_requested();
    assert_eq!(colour_of(&app, wire_a), Some(color(1)));
    assert_eq!(colour_of(&app, wire_b), Some(color(2)));
}

/// Junction colours belong to their sheet too: one flood is one step for the
/// wire and the junction.
#[test]
fn undo_takes_the_floods_junction_colour_with_it() {
    let (mut app, path) = app_with_sheet_a();
    let (wire, junction) = (wire_of(&app, &path), junction_of(&app, &path));
    paint(&mut app, &[wire, junction], color(1));

    app.handle_undo_requested();
    assert_eq!(colour_of(&app, wire), None);
    assert_eq!(colour_of(&app, junction), None);
    assert!(!app.document_state.engines[&path].can_undo());

    app.handle_redo_requested();
    assert_eq!(colour_of(&app, wire), Some(color(1)));
    assert_eq!(colour_of(&app, junction), Some(color(1)));
}

/// Clear All wipes every sheet, but each sheet's Ctrl+Z restores its own.
#[test]
fn clear_all_is_global_and_undoes_per_sheet() {
    let (mut app, a) = app_with_sheet_a();
    let b = PathBuf::from(B);
    add_sheet(&mut app, &b);
    let (wire_a, wire_b) = (wire_of(&app, &a), wire_of(&app, &b));
    paint_on(&mut app, &a, &[wire_a], color(1));
    paint_on(&mut app, &b, &[wire_b], color(2));

    app.clear_net_colors(None);
    assert_eq!(colour_of(&app, wire_a), None);
    assert_eq!(colour_of(&app, wire_b), None);

    activate(&mut app, &a);
    app.handle_undo_requested();
    assert_eq!(colour_of(&app, wire_a), Some(color(1)));
    assert_eq!(colour_of(&app, wire_b), None);

    activate(&mut app, &b);
    app.handle_undo_requested();
    assert_eq!(colour_of(&app, wire_b), Some(color(2)));
}

/// Closing a sheet drops its colours with its engine.
#[test]
fn closing_a_sheet_drops_its_colours() {
    let (mut app, a) = app_with_sheet_a();
    let b = PathBuf::from(B);
    add_sheet(&mut app, &b);
    let (wire_a, wire_b) = (wire_of(&app, &a), wire_of(&app, &b));
    paint_on(&mut app, &a, &[wire_a], color(1));
    paint_on(&mut app, &b, &[wire_b], color(2));

    app.document_state.remove_engine(&a);

    assert_eq!(colour_of(&app, wire_a), None);
    assert_eq!(colour_of(&app, wire_b), Some(color(2)));
}
