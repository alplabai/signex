//! #1250 — a locked symbol refuses delete and move from the app, and says
//! why in the Messages panel.

use signex_app::app::{EditMsg, Message, MoveSelectionMsg, Signex};
use signex_app::diagnostics;
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind, Symbol};
use std::path::PathBuf;

const REFERENCE: &str = "RLOCK1250";

fn locked_symbol_app() -> (Signex, uuid::Uuid) {
    let _ = diagnostics::init_logging();
    let uuid = uuid::Uuid::new_v4();
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        symbols: vec![Symbol {
            uuid,
            lib_id: "Device:R".to_string(),
            reference: REFERENCE.to_string(),
            position: Point::new(50.0, 0.0),
            locked: true,
            ..Default::default()
        }],
        ..Default::default()
    };
    let path = PathBuf::from("symbol-locked.snxsch");
    let (mut app, _boot) = Signex::new();
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "symbol-locked".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(uuid, SelectedKind::Symbol)];
    (app, uuid)
}

fn symbols(app: &Signex) -> Vec<Symbol> {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .symbols
        .clone()
}

fn refusal_logged() -> bool {
    diagnostics::recent_entries().iter().any(|entry| {
        entry
            .message
            .contains(&format!("{REFERENCE} is locked and cannot be"))
    })
}

#[test]
fn delete_is_refused_and_reported() {
    let (mut app, _) = locked_symbol_app();

    let _ = app.update(Message::Edit(EditMsg::DeleteSelected));

    assert_eq!(symbols(&app).len(), 1, "the locked symbol was deleted");
    assert!(refusal_logged(), "no Messages-panel note for the refusal");
}

#[test]
fn the_move_dialog_is_refused_and_reported() {
    let (mut app, _) = locked_symbol_app();
    app.ui_state.move_selection.dx = "10".to_string();
    app.ui_state.move_selection.dy = "0".to_string();

    let _ = app.update(Message::MoveSelection(MoveSelectionMsg::Apply));

    let after = symbols(&app);
    assert!(
        (after[0].position.x - 50.0).abs() < 1e-9,
        "the locked symbol moved to {}",
        after[0].position.x
    );
    assert!(refusal_logged(), "no Messages-panel note for the refusal");
}
