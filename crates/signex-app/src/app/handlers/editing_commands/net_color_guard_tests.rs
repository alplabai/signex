//! #661 slice 6: net-colour behaviour that must not change when colours move
//! onto the engine timeline.

use super::net_color_driver::*;
use signex_types::schematic::{SelectedItem, SelectedKind};

#[test]
fn saving_writes_the_painted_colours_and_drops_deleted_elements() {
    let (mut app, path) = app_with_sheet("guard-save.snxsch");
    let wire = wire_of(&app, &path);
    let gone = uuid::Uuid::now_v7();
    paint(&mut app, &[wire, gone], color(7));

    let saved = saved_colours(&app, &path);

    assert_eq!(saved.get(&wire), Some(&color(7)));
    assert!(
        !saved.contains_key(&gone),
        "no wire or junction has that id"
    );
}

#[test]
fn a_deleted_coloured_wire_comes_back_coloured_after_undo() {
    let (mut app, path) = app_with_sheet("guard-delete.snxsch");
    let wire = wire_of(&app, &path);
    paint(&mut app, &[wire], color(7));
    let deleted = app.apply_engine_command(
        signex_engine::Command::DeleteSelection {
            items: vec![SelectedItem::new(wire, SelectedKind::Wire)],
        },
        true,
        true,
    );
    assert!(deleted);
    assert!(
        saved_colours(&app, &path).is_empty(),
        "nothing left to paint"
    );

    app.handle_undo_requested();

    assert_eq!(app.document_state.engines[&path].document().wires.len(), 1);
    assert_eq!(colour_of(&app, wire), Some(color(7)));
    assert_eq!(saved_colours(&app, &path).get(&wire), Some(&color(7)));
}

#[test]
fn a_colour_step_is_one_undo_and_redo_brings_it_back() {
    let (mut app, path) = app_with_sheet("guard-undo.snxsch");
    let wire = wire_of(&app, &path);
    paint(&mut app, &[wire], color(3));

    app.handle_undo_requested();
    assert_eq!(colour_of(&app, wire), None);
    app.handle_redo_requested();
    assert_eq!(colour_of(&app, wire), Some(color(3)));
}

#[test]
fn a_colour_undo_follows_a_save_as_rename_of_its_sheet() {
    let (mut app, old) = app_with_sheet("guard-rename.snxsch");
    let wire = wire_of(&app, &old);
    paint(&mut app, &[wire], color(3));

    let new = std::path::PathBuf::from("guard-renamed.snxsch");
    app.rekey_document(&old, &new);
    app.handle_undo_requested();

    assert_eq!(colour_of(&app, wire), None);
    assert!(!is_dirty(&app, &old));
}
