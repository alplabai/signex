//! #962 — under the Altium profile Space rotates the selected symbol 90
//! degrees counter-clockwise and Shift+Space 90 degrees clockwise. Drives
//! the real keymap path (`UiMsg::KeymapStroke` -> `active_keymap` ->
//! command bridge -> engine) and Undo.
//!
//! `Symbol::rotation` is counter-clockwise positive on screen (the
//! y-up library to y-down schematic transform negates the angle), so
//! counter-clockwise adds 90 and clockwise subtracts 90.

use signex_app::app::{EditMsg, Message, Signex, TabInfo, TabKind, UiMsg};
use signex_app::keymap::{KeyStroke, KeyToken, Modifiers};
use signex_types::schematic::{SchematicSheet, SelectedItem, SelectedKind, Symbol};
use std::path::PathBuf;

fn app_with_selected_symbol() -> Signex {
    let symbol = Symbol {
        uuid: uuid::Uuid::new_v4(),
        lib_id: "Device:R".to_string(),
        reference: "R1".to_string(),
        value: "10k".to_string(),
        ..Default::default()
    };
    let item = SelectedItem::new(symbol.uuid, SelectedKind::Symbol);
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        symbols: vec![symbol],
        ..Default::default()
    };
    let path = PathBuf::from("rotate-direction.snxsch");
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let (mut app, _t) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state
        .keymap_profiles
        .set_active_profile("altium")
        .expect("altium profile");
    app.ui_state.active_keymap = app.ui_state.keymap_profiles.compile_active();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "rotate-direction".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app.interaction_state.active_canvas_mut().selected = vec![item];
    app
}

fn press_space(app: &mut Signex, shift: bool) {
    let stroke = KeyStroke {
        modifiers: Modifiers {
            shift,
            ..Modifiers::default()
        },
        key: KeyToken::Space,
    };
    let _ = app.update(Message::Ui(UiMsg::KeymapStroke {
        window: None,
        stroke,
    }));
}

fn rotation_deg(app: &Signex) -> f64 {
    let engine = app.document_state.active_engine().expect("engine");
    engine.document().symbols[0].rotation.rem_euclid(360.0)
}

#[test]
fn space_rotates_counterclockwise_and_shift_space_clockwise() {
    let mut app = app_with_selected_symbol();
    assert_eq!(rotation_deg(&app), 0.0);

    press_space(&mut app, false);
    assert_eq!(rotation_deg(&app), 90.0, "Space = 90 deg counter-clockwise");

    let mut app = app_with_selected_symbol();
    press_space(&mut app, true);
    assert_eq!(rotation_deg(&app), 270.0, "Shift+Space = 90 deg clockwise");
}

#[test]
fn shift_space_after_space_returns_to_start_and_undo_reverts_each_step() {
    let mut app = app_with_selected_symbol();
    press_space(&mut app, false);
    press_space(&mut app, true);
    assert_eq!(rotation_deg(&app), 0.0, "CCW then CW cancels out");

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(rotation_deg(&app), 90.0, "undo reverts the clockwise step");
    // Undo clears the selection; the next rotate would need a reselect,
    // so a second undo is enough to prove the CCW step is its own entry.
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(
        rotation_deg(&app),
        0.0,
        "undo reverts the counter-clockwise step"
    );
}
