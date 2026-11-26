//! Mockup row 65, decided 2026-11-24 (docs/DECISIONS.md): a click outside
//! the in-place text editor applies the edit as one undo step, as the panel
//! fields do.

use std::path::PathBuf;

use signex_types::schematic::{Label, LabelType, Point, SchematicSheet};
use uuid::Uuid;

use crate::app::{EditMsg, Message, Signex, TabInfo, TabKind, TextEditMsg};
use crate::canvas::CanvasEvent;

/// An app whose active sheet is `sheet`, rendered so hit-testing works.
fn app_with(sheet: SchematicSheet) -> Signex {
    let path = PathBuf::from("field-edit.snxsch");
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet.clone()).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "field-edit".to_string(),
        path: path.clone(),
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(
            crate::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
    app
}

fn doc(app: &Signex) -> &SchematicSheet {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
}

#[test]
fn a_click_outside_the_in_place_editor_applies_the_edit_as_one_undo_step() {
    let mut sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    sheet.labels = vec![Label {
        justify_v: Default::default(),
        ..Label::new("CLK", Point::new(20.0, 100.0), LabelType::Net)
    }];
    let mut app = app_with(sheet);
    let _ = app.update(Message::CanvasEvent(CanvasEvent::DoubleClicked {
        world_x: 20.0,
        world_y: 100.0,
        screen_x: 0.0,
        screen_y: 0.0,
    }));
    assert!(app.interaction_state.editing_text.is_some(), "editor open");
    let _ = app.update(Message::TextEdit(TextEditMsg::Changed("CLK2".into())));

    let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
        world_x: 150.0,
        world_y: 40.0,
    }));

    assert!(
        app.interaction_state.editing_text.is_none(),
        "editor closed"
    );
    assert_eq!(doc(&app).labels[0].text, "CLK2", "the typed text applied");
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(doc(&app).labels[0].text, "CLK", "one undo step");
}
