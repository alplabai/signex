//! #102 — Ctrl+Z / Ctrl+Y interleave net-colour floods and engine edits in
//! true chronological order.

use super::super::super::*;
use signex_types::schematic::{SchematicSheet, SelectedItem, SelectedKind, Symbol};

fn color(r: u8) -> signex_types::theme::Color {
    signex_types::theme::Color {
        r,
        g: 0,
        b: 0,
        a: 255,
    }
}

fn app_with_symbol() -> (Signex, uuid::Uuid) {
    let id = uuid::Uuid::new_v4();
    let path = std::path::PathBuf::from("undo-order.snxsch");
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        symbols: vec![Symbol {
            uuid: id,
            lib_id: "Device:R".to_string(),
            reference: "R1".to_string(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let (mut app, _task) = Signex::new();
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "undo-order".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path);
    (app, id)
}

fn move_symbol(app: &mut Signex, id: uuid::Uuid) {
    let moved = app.apply_engine_command(
        signex_engine::Command::MoveSelection {
            items: vec![SelectedItem::new(id, SelectedKind::Symbol)],
            dx: 10.0,
            dy: 0.0,
        },
        true,
        true,
    );
    assert!(moved, "fixture: the move must change the document");
}

fn symbol_x(app: &Signex) -> f64 {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .symbols[0]
        .position
        .x
}

/// Flood a colour, then move a symbol: Ctrl+Z reverts the move first and
/// the colour second; Ctrl+Y replays them in the opposite order.
#[test]
fn undo_and_redo_follow_chronological_order() {
    let (mut app, id) = app_with_symbol();
    let wire = uuid::Uuid::new_v4();
    let start = symbol_x(&app);

    app.record_net_color_undo();
    app.ui_state.wire_color_overrides.insert(wire, color(9));
    move_symbol(&mut app, id);

    app.handle_undo_requested();
    assert_eq!(symbol_x(&app), start, "first undo reverts the newer move");
    assert!(app.ui_state.wire_color_overrides.contains_key(&wire));

    app.handle_undo_requested();
    assert!(
        app.ui_state.wire_color_overrides.is_empty(),
        "second undo reverts the colour"
    );

    app.handle_redo_requested();
    assert!(
        app.ui_state.wire_color_overrides.contains_key(&wire),
        "redo replays the colour first"
    );
    assert_eq!(symbol_x(&app), start);

    app.handle_redo_requested();
    assert_eq!(symbol_x(&app), start + 10.0, "then the move");
}

/// A new engine edit ends the colour redo branch.
#[test]
fn a_new_engine_edit_clears_colour_redo() {
    let (mut app, id) = app_with_symbol();
    app.record_net_color_undo();
    app.ui_state
        .wire_color_overrides
        .insert(uuid::Uuid::new_v4(), color(1));
    app.handle_undo_requested();
    assert_eq!(app.ui_state.net_color_redo.len(), 1);

    move_symbol(&mut app, id);
    assert!(app.ui_state.net_color_redo.is_empty());
}
