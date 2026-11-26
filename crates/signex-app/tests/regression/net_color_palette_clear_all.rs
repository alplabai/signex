//! #2528 — the net colour palette's "Clear all colours" is one undo step:
//! one Ctrl+Z after it brings every colour back.

use signex_app::app::{EditMsg, Message, NetColorMsg, Signex};
use signex_types::schematic::{Point, SchematicSheet, Wire};
use signex_types::theme::Color;
use std::path::PathBuf;

fn wire(y: f64) -> Wire {
    Wire {
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(0.0, y),
        end: Point::new(10.0, y),
        ..Default::default()
    }
}

#[test]
fn clear_all_net_colours_then_one_undo_restores_them_all() {
    let (a, b) = (wire(0.0), wire(10.0));
    let mut sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        wires: vec![a.clone(), b.clone()],
        ..Default::default()
    };
    sheet
        .net_colors
        .insert(a.uuid, Color::new(0xE0, 0x54, 0x54, 255));
    sheet
        .net_colors
        .insert(b.uuid, Color::new(0x42, 0xB8, 0xE0, 255));
    let before = sheet.net_colors.clone();

    let path = PathBuf::from("palette-clear-all.snxsch");
    let (mut app, _boot) = Signex::new();
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "palette-clear-all".to_string(),
        path: path.clone(),
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.clone());

    let colours = |app: &Signex| {
        app.document_state.engines[&path]
            .document()
            .net_colors
            .clone()
    };
    let _ = app.update(Message::NetColor(NetColorMsg::ClearedAll));
    assert!(
        colours(&app).is_empty(),
        "Clear all colours clears the sheet"
    );

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(colours(&app), before, "one undo restores every colour");
}
