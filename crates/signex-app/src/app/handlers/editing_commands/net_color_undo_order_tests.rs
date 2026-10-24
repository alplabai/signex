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
    let (mut app, _task) = Signex::new();
    let id = add_doc(&mut app, "undo-order.snxsch");
    (app, id)
}

/// Open a one-symbol schematic tab and make it the active document.
fn add_doc(app: &mut Signex, name: &str) -> uuid::Uuid {
    let id = uuid::Uuid::new_v4();
    let path = std::path::PathBuf::from(name);
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
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(crate::app::TabInfo {
        title: name.to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path);
    id
}

fn activate(app: &mut Signex, name: &str) {
    let path = std::path::PathBuf::from(name);
    app.document_state.active_tab = app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == path)
        .expect("tab");
    app.document_state.active_path = Some(path);
}

fn active_steps(
    steps: &std::collections::HashMap<std::path::PathBuf, Vec<crate::app::state::NetColorStep>>,
    app: &Signex,
) -> usize {
    app.document_state
        .active_path
        .as_ref()
        .and_then(|p| steps.get(p))
        .map_or(0, Vec::len)
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
    assert_eq!(active_steps(&app.ui_state.net_color_redo, &app), 1);

    move_symbol(&mut app, id);
    assert_eq!(active_steps(&app.ui_state.net_color_redo, &app), 0);
}

/// A colour on A is untouched by Ctrl+Z in B; switching back undoes it.
#[test]
fn colour_steps_belong_to_their_own_document() {
    let (mut app, _a_sym) = app_with_symbol();
    let b_sym = add_doc(&mut app, "b.snxsch");
    activate(&mut app, "undo-order.snxsch");
    let wire = uuid::Uuid::new_v4();
    app.record_net_color_undo();
    app.ui_state.wire_color_overrides.insert(wire, color(9));

    activate(&mut app, "b.snxsch");
    move_symbol(&mut app, b_sym);
    app.handle_undo_requested();
    assert_eq!(symbol_x(&app), 0.0, "B's own edit is undone");
    assert!(
        app.ui_state.wire_color_overrides.contains_key(&wire),
        "A's colour is not touched from B"
    );
    app.handle_undo_requested();
    assert!(app.ui_state.wire_color_overrides.contains_key(&wire));

    activate(&mut app, "undo-order.snxsch");
    app.handle_undo_requested();
    assert!(app.ui_state.wire_color_overrides.is_empty());
}

/// Redo availability and effect are per document.
#[test]
fn colour_redo_is_enabled_only_for_its_document() {
    let (mut app, _a_sym) = app_with_symbol();
    add_doc(&mut app, "b.snxsch");
    activate(&mut app, "undo-order.snxsch");
    app.record_net_color_undo();
    app.ui_state
        .wire_color_overrides
        .insert(uuid::Uuid::new_v4(), color(1));
    app.handle_undo_requested();
    assert_eq!(active_steps(&app.ui_state.net_color_redo, &app), 1);

    activate(&mut app, "b.snxsch");
    assert_eq!(active_steps(&app.ui_state.net_color_redo, &app), 0);
    app.handle_redo_requested();
    assert!(app.ui_state.wire_color_overrides.is_empty(), "no-op on B");
}

/// Replacing a document's engine drops that document's colour steps only.
#[test]
fn reload_clears_that_documents_colour_steps() {
    let (mut app, _a_sym) = app_with_symbol();
    add_doc(&mut app, "b.snxsch");
    activate(&mut app, "undo-order.snxsch");
    app.record_net_color_undo();
    activate(&mut app, "b.snxsch");
    app.record_net_color_undo();

    app.clear_net_color_steps(std::path::Path::new("undo-order.snxsch"));
    assert!(
        !app.ui_state
            .net_color_undo
            .contains_key(std::path::Path::new("undo-order.snxsch"))
    );
    assert_eq!(active_steps(&app.ui_state.net_color_undo, &app), 1);
}

/// Annotate records engine history outside the gateway; it must still end
/// the colour redo branch.
#[test]
fn annotate_style_dirty_marking_clears_colour_redo() {
    let (mut app, _id) = app_with_symbol();
    app.record_net_color_undo();
    app.ui_state
        .wire_color_overrides
        .insert(uuid::Uuid::new_v4(), color(1));
    app.handle_undo_requested();
    assert_eq!(active_steps(&app.ui_state.net_color_redo, &app), 1);

    app.mark_active_document_dirty();
    assert_eq!(active_steps(&app.ui_state.net_color_redo, &app), 0);
}
