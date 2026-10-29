//! #1666 — the junction tool: a click drops a junction at the snapped point,
//! it undoes, and the Active Bar button and the Place menu row arm the same
//! tool (the `place_junction` key resolves to the button, pinned in the
//! bridge tests).

use signex_app::active_bar::{ActiveBarAction, ActiveBarMsg};
use signex_app::app::{EditMsg, Message, Signex, TabInfo, TabKind, Tool};
use signex_app::canvas::CanvasEvent;
use signex_app::menu_bar::MenuMessage;
use signex_types::schematic::SchematicSheet;
use std::path::PathBuf;

fn app_with_empty_sheet() -> Signex {
    let path = PathBuf::from("junction.snxsch");
    let sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let engine = signex_engine::Engine::new(sheet).expect("engine");

    let (mut app, _initial_task) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "junction".to_string(),
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

#[test]
fn the_button_and_the_menu_row_arm_the_same_tool() {
    let mut app = app_with_empty_sheet();
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(
        ActiveBarAction::PlaceJunction,
    )));
    assert_eq!(app.interaction_state.current_tool, Tool::Junction);

    let mut app = app_with_empty_sheet();
    let _ = app.update(Message::Menu(MenuMessage::PlaceJunction));
    assert_eq!(app.interaction_state.current_tool, Tool::Junction);
}

#[test]
fn a_click_places_a_junction_at_the_point_and_undo_removes_it() {
    let mut app = app_with_empty_sheet();
    let _ = app.update(Message::Menu(MenuMessage::PlaceJunction));
    click(&mut app, 10.0, 20.0);
    // The click snaps to the grid before the tool sees it.

    let junctions = &doc(&app).junctions;
    assert_eq!(junctions.len(), 1);
    assert_eq!(
        (junctions[0].position.x, junctions[0].position.y),
        (10.16, 20.32)
    );
    assert!(
        !junctions[0].minted,
        "a placed dot is the user's, not minted"
    );

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(doc(&app).junctions.is_empty());
}

#[test]
fn a_second_click_on_the_same_point_leaves_one_junction() {
    let mut app = app_with_empty_sheet();
    let _ = app.update(Message::Menu(MenuMessage::PlaceJunction));
    click(&mut app, 10.0, 20.0);
    click(&mut app, 10.0, 20.0);
    assert_eq!(doc(&app).junctions.len(), 1);
    // The tool stays armed: a different point still places.
    click(&mut app, 30.0, 20.0);
    assert_eq!(doc(&app).junctions.len(), 2);
}

/// A dot off every wire joins nothing: the net list and the ERC output are
/// the same with and without it.
#[test]
fn a_junction_off_any_wire_changes_neither_nets_nor_erc() {
    let mut app = app_with_empty_sheet();
    let before_nets = format!("{:?}", signex_net::build_netlist(doc(&app)));
    let before_erc = format!("{:?}", signex_erc::run(doc(&app)));
    let _ = app.update(Message::Menu(MenuMessage::PlaceJunction));
    click(&mut app, 10.0, 20.0);
    assert_eq!(doc(&app).junctions.len(), 1);
    assert_eq!(
        format!("{:?}", signex_net::build_netlist(doc(&app))),
        before_nets
    );
    assert_eq!(format!("{:?}", signex_erc::run(doc(&app))), before_erc);
}
