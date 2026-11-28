//! #3194: an agent's edit arrives while the person is still typing a drawing
//! value in Properties (a preview in the engine). The person's value must be
//! settled as its own step first, never folded into the agent's or left out
//! of the history.

use serde_json::json;
use signex_types::schematic::{FillType, Point, SchDrawing, SelectedItem, SelectedKind};

use super::super::Input;
use super::{app_in, ask_with_task, call, line};
use crate::app::{EditMsg, Message, Signex};
use crate::dock::DockMessage;
use crate::panels::{DrawingFieldId as F, PanelMsg};

fn width(app: &Signex, sheet: &std::path::Path) -> f64 {
    let drawings = &app.document_state.engines[sheet].document().drawings;
    match drawings.last().expect("the rectangle") {
        SchDrawing::Rect { start, end, .. } => (end.x - start.x).abs(),
        other => panic!("not a rectangle: {other:?}"),
    }
}

/// The fixture with a saved rectangle of width 20 on `b`, selected, and "45"
/// typed into its Width box without leaving it.
fn typing_on_b(dir: &std::path::Path) -> (Signex, std::path::PathBuf) {
    let mut app = app_in(dir);
    let b = dir.join("b.snxsch");
    let rect = SchDrawing::Rect {
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(10.0, 10.0),
        end: Point::new(30.0, 20.0),
        width: 0.2,
        fill: FillType::None,
        stroke_color: None,
        locked: false,
    };
    let id = rect.uuid();
    let engine = app.document_state.engines.get_mut(&b).expect("b");
    engine
        .execute(signex_engine::Command::PlaceSchDrawing { drawing: rect })
        .expect("placed");
    engine.mark_saved();
    app.document_state.mark_document_clean(&b);
    let canvas = app.interaction_state.active_canvas_mut();
    canvas.selected = vec![SelectedItem::new(id, SelectedKind::Drawing)];
    app.update_selection_info();
    let typed = PanelMsg::DrawingFieldTyping(F::RectWidth, "45".into());
    let _ = app.update(Message::Dock(DockMessage::Panel(typed)));
    assert_eq!(width(&app, &b), 45.0, "the preview is shown");
    assert!(app.document_state.engines[&b].live_edit_drawing().is_some());
    assert!(!app.document_state.is_document_dirty(&b));
    (app, b)
}

#[test]
fn an_agent_edit_mid_typing_lands_the_persons_value_as_its_own_step_first() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, b) = typing_on_b(dir.path());
    let started = app.document_state.engines[&b].revision();
    let edits = json!([{"id": "place_junction", "args": {"at": [20, 20]}}]);
    let doc = b.display().to_string();
    let applied = call(
        &mut app,
        "apply_edits",
        json!({"document": doc, "expect_revision": started, "edits": edits}),
    );
    assert_eq!(applied["status"], "answered", "{applied}");

    let engine = &app.document_state.engines[&b];
    assert_eq!(engine.live_edit_drawing(), None, "the core settled it");
    assert!(app.document_view.live_drawing_edit.is_none());
    assert!(app.document_state.is_document_dirty(&b));
    assert_eq!(width(&app, &b), 45.0);
    let junctions = engine.document().junctions.len();
    assert!(junctions > 0, "the agent's junction landed");

    // The person's undo: the agent's step first, then their own typed value.
    let _ = app.update(Message::Edit(EditMsg::Undo));
    let left = app.document_state.engines[&b].document().junctions.len();
    assert_eq!(left, junctions - 1, "the agent's step went first");
    assert_eq!(width(&app, &b), 45.0, "the typed value is its own step");
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(width(&app, &b), 30.0 - 10.0);
}

/// An agent's `save_document` while the person types: the preview is settled
/// first, and the file holds what the engine holds. A typed value written is
/// covered by an undo step back to the saved one.
#[test]
fn an_agent_save_mid_typing_settles_the_preview_before_it_writes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, b) = typing_on_b(dir.path());
    let request = line(
        &app,
        "save_document",
        json!({"document": b.display().to_string()}),
    );
    let (saved, task) = ask_with_task(&mut app, Input::Line(request));
    assert_eq!(saved["status"], "ran", "{saved}");
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the save settled");

    assert_eq!(app.document_state.engines[&b].live_edit_drawing(), None);
    let text = std::fs::read_to_string(&b).expect("the sheet was written");
    let file = signex_types::format::SnxSchematic::parse(&text).expect("reopens");
    let written = match file.sheet.drawings.last().expect("the rectangle") {
        SchDrawing::Rect { start, end, .. } => (end.x - start.x).abs(),
        other => panic!("not a rectangle: {other:?}"),
    };
    assert_eq!(written, width(&app, &b), "the file is the engine's sheet");
    if written != 20.0 {
        let _ = app.update(Message::Edit(EditMsg::Undo));
        assert_eq!(width(&app, &b), 20.0, "an undo step covers the typed value");
    }
}
