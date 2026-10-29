//! #350 — "Generic No ERC" places a real directive instead of aliasing the
//! no-connect tool: it lands in `no_erc_directives`, undoes, can be selected
//! and deleted.

use signex_app::active_bar::{ActiveBarAction, ActiveBarMsg};
use signex_app::app::{EditMsg, Message, Signex, TabInfo, TabKind, Tool};
use signex_app::canvas::CanvasEvent;
use signex_types::schematic::{SchematicSheet, SelectedKind};
use std::path::PathBuf;

fn app_with_empty_sheet() -> Signex {
    let path = PathBuf::from("no-erc.snxsch");
    let sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let engine = signex_engine::Engine::new(sheet).expect("engine");

    let (mut app, _initial_task) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "no-erc".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app
}

fn doc(app: &Signex) -> &SchematicSheet {
    app.document_state
        .engines
        .values()
        .next()
        .expect("engine")
        .document()
}

fn click(app: &mut Signex, x: f64, y: f64) {
    let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
        world_x: x,
        world_y: y,
    }));
}

fn arm_no_erc(app: &mut Signex) {
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(
        ActiveBarAction::PlaceNoERC,
    )));
}

#[test]
fn generic_no_erc_arms_its_own_tool_not_no_connect() {
    let mut app = app_with_empty_sheet();
    arm_no_erc(&mut app);
    assert_eq!(app.interaction_state.current_tool, Tool::NoErcDirective);
}

#[test]
fn clicking_with_the_tool_places_a_directive_and_undo_removes_it() {
    let mut app = app_with_empty_sheet();
    arm_no_erc(&mut app);
    click(&mut app, 10.0, 20.0);

    assert!(doc(&app).no_connects.is_empty());
    assert_eq!(doc(&app).no_erc_directives.len(), 1);

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(doc(&app).no_erc_directives.is_empty());
}

#[test]
fn a_placed_directive_can_be_selected_and_deleted() {
    let mut app = app_with_empty_sheet();
    arm_no_erc(&mut app);
    click(&mut app, 10.0, 20.0);
    let uuid = doc(&app).no_erc_directives[0].uuid;
    let at = doc(&app).no_erc_directives[0].position;

    let _ = app.update(Message::Tool(signex_app::toolbar::ToolMessage::SelectTool(
        Tool::Select,
    )));
    click(&mut app, at.x, at.y);
    let selected = &app.interaction_state.active_canvas().selected;
    assert!(
        selected
            .iter()
            .any(|i| i.uuid == uuid && i.kind == SelectedKind::NoErcDirective),
        "the directive must be hit-testable, got {selected:?}"
    );

    let _ = app.update(Message::Edit(EditMsg::DeleteSelected));
    assert!(doc(&app).no_erc_directives.is_empty());
}
