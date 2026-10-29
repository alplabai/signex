//! #1573 — a locked wire, label, text note or drawing refuses delete from the
//! app and the Messages panel says which ones were skipped, the same way the
//! symbol lock does (#1250).

use signex_app::app::{EditMsg, Message, Signex};
use signex_app::diagnostics;
use signex_app::dock::DockMessage;
use signex_app::panels::PanelMsg;
use signex_types::schematic::{
    HAlign, Label, LabelType, Point, SchDrawing, SchematicSheet, SelectedItem, SelectedKind,
    TextNote, VAlign, Wire,
};
use std::path::PathBuf;

/// `y` makes every name unique to one test: the log ring is shared.
fn app_with_locked_objects(y: f64) -> (Signex, Vec<SelectedItem>) {
    let _ = diagnostics::init_logging();
    let (wire, label, note, drawing) = (
        uuid::Uuid::now_v7(),
        uuid::Uuid::now_v7(),
        uuid::Uuid::now_v7(),
        uuid::Uuid::now_v7(),
    );
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        wires: vec![Wire {
            uuid: wire,
            start: Point::new(0.0, y),
            end: Point::new(10.0, y),
            locked: true,
            ..Default::default()
        }],
        labels: vec![Label {
            uuid: label,
            text: format!("LOCKED{y}"),
            position: Point::new(0.0, y),
            rotation: 0.0,
            label_type: LabelType::Net,
            shape: String::new(),
            font_size: 1.27,
            font_family: None,
            justify: HAlign::Left,
            justify_v: VAlign::Bottom,
            locked: true,
            unknown_columns: Default::default(),
        }],
        text_notes: vec![TextNote {
            uuid: note,
            text: "note".into(),
            position: Point::new(0.0, y),
            rotation: 0.0,
            font_size: 1.27,
            font_family: None,
            justify_h: HAlign::Left,
            justify_v: VAlign::Bottom,
            locked: true,
        }],
        drawings: vec![SchDrawing::Line {
            uuid: drawing,
            start: Point::new(0.0, y),
            end: Point::new(10.0, y),
            width: 0.2,
            stroke_color: None,
            locked: true,
        }],
        ..Default::default()
    };
    let path = PathBuf::from("object-locked.snxsch");
    let (mut app, _boot) = Signex::new();
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "object-locked".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    let items = vec![
        SelectedItem::new(wire, SelectedKind::Wire),
        SelectedItem::new(label, SelectedKind::Label),
        SelectedItem::new(note, SelectedKind::TextNote),
        SelectedItem::new(drawing, SelectedKind::Drawing),
    ];
    app.interaction_state.active_canvas_mut().selected = items.clone();
    (app, items)
}

fn logged(needle: &str) -> usize {
    diagnostics::recent_entries()
        .iter()
        .filter(|entry| entry.message.contains(needle))
        .count()
}

/// Nothing but locked objects reaches the engine, which says why it refused.
#[test]
fn deleting_only_locked_objects_is_refused_and_reported() {
    let (mut app, _) = app_with_locked_objects(1573.5);
    let needle = "are locked and cannot be deleted";
    let before = logged(needle);

    let _ = app.update(Message::Edit(EditMsg::DeleteSelected));

    let doc = app
        .document_state
        .active_engine()
        .expect("engine")
        .document();
    assert_eq!(doc.wires.len(), 1, "the locked wire was deleted");
    assert_eq!(doc.labels.len(), 1, "the locked label was deleted");
    assert_eq!(doc.text_notes.len(), 1, "the locked note was deleted");
    assert_eq!(doc.drawings.len(), 1, "the locked drawing was deleted");
    assert!(logged(needle) > before, "no note for the refusal");
}

/// A free wire in the selection goes; the locked four are skipped and named.
#[test]
fn a_mixed_delete_skips_the_locked_objects_and_removes_the_free_wire() {
    let (mut app, mut items) = app_with_locked_objects(1573.75);
    let free = uuid::Uuid::now_v7();
    let engine = app.document_state.active_engine_mut().expect("engine");
    let mut sheet = engine.document().clone();
    sheet.wires.push(Wire {
        uuid: free,
        start: Point::new(0.0, 99.0),
        end: Point::new(10.0, 99.0),
        ..Default::default()
    });
    engine.set_document(sheet);
    items.push(SelectedItem::new(free, SelectedKind::Wire));
    app.interaction_state.active_canvas_mut().selected = items;
    let needle = "Skipped 4 locked object(s): ";
    let before = logged(needle);

    let _ = app.update(Message::Edit(EditMsg::DeleteSelected));

    let doc = app
        .document_state
        .active_engine()
        .expect("engine")
        .document();
    assert_eq!(doc.wires.len(), 1, "only the locked wire is left");
    assert!(doc.wires[0].locked);
    assert_eq!(
        doc.labels.len() + doc.text_notes.len() + doc.drawings.len(),
        3
    );
    assert!(logged(needle) > before, "no note for the skip");
}

#[test]
fn unlocking_from_the_properties_panel_allows_the_delete() {
    let (mut app, items) = app_with_locked_objects(1573.25);
    let wire = items[0];
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::ToggleItemLocked(wire),
    )));
    let engine = app.document_state.active_engine().expect("engine");
    assert_eq!(engine.item_locked(&wire), Some(false));

    app.interaction_state.active_canvas_mut().selected = vec![wire];
    let _ = app.update(Message::Edit(EditMsg::DeleteSelected));
    let doc = app
        .document_state
        .active_engine()
        .expect("engine")
        .document();
    assert!(doc.wires.is_empty(), "the unlocked wire is deleted");
}
