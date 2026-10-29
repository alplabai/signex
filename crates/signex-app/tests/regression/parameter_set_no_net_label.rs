//! #1248 — the Parameter Set directive must never place a net label: a
//! `LabelType::Net` "PARAM=VALUE" renames the wire it lands on and joins
//! same-text labels on one sheet. Until a non-electrical directive object
//! exists the tool is inert.

use signex_app::active_bar::{ActiveBarAction, ActiveBarMsg};
use signex_app::app::{Message, Signex, TabInfo, TabKind, Tool};
use signex_app::canvas::CanvasEvent;
use signex_types::schematic::SchematicSheet;
use std::path::PathBuf;

fn app_with_empty_sheet() -> Signex {
    let path = PathBuf::from("param-set.snxsch");
    let sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let (mut app, _task) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "param-set".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app
}

#[test]
fn parameter_set_places_no_label_and_arms_no_tool() {
    let mut app = app_with_empty_sheet();
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(
        ActiveBarAction::PlaceParameterSet,
    )));
    assert_ne!(app.interaction_state.current_tool, Tool::Label);
    assert!(
        app.interaction_state
            .active_canvas_mut()
            .ghost_label
            .is_none()
    );

    for (x, y) in [(10.0, 10.0), (30.0, 30.0)] {
        let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
            world_x: x,
            world_y: y,
        }));
    }
    let doc = app
        .document_state
        .engines
        .values()
        .next()
        .unwrap()
        .document();
    assert!(
        doc.labels.is_empty(),
        "no label may be placed: {:?}",
        doc.labels
    );
}
