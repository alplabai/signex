//! #271 — the Active Bar's "Move" and "Move Selection" rows were one action
//! that opened the dX / dY dialog. They are now two interactive click-click
//! gestures: Move picks one object, Move Selection lifts the current
//! selection; the drop commits through `CanvasEvent::MoveSelected`.

use signex_app::active_bar::{ActiveBarAction, ActiveBarMsg};
use signex_app::app::{Message, Signex, Tool};
use signex_app::canvas::{CanvasEvent, MovePick};
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind, Wire};
use std::collections::HashMap;
use std::path::PathBuf;

fn wire(uuid: uuid::Uuid, y: f64) -> Wire {
    Wire {
        unknown_columns: Default::default(),
        uuid,
        start: Point::new(10.0, y),
        end: Point::new(30.0, y),
        stroke_width: 0.0,
    }
}

fn app_with_wires(wires: Vec<Wire>) -> (Signex, PathBuf) {
    let sheet = SchematicSheet {
        unknown: Default::default(),
        uuid: uuid::Uuid::new_v4(),
        version: 0,
        generator: String::new(),
        generator_version: String::new(),
        paper_size: "A4".to_string(),
        root_sheet_page: "1".to_string(),
        symbols: Vec::new(),
        wires,
        junctions: Vec::new(),
        labels: Vec::new(),
        child_sheets: Vec::new(),
        no_connects: Vec::new(),
        text_notes: Vec::new(),
        buses: Vec::new(),
        bus_entries: Vec::new(),
        drawings: Vec::new(),
        no_erc_directives: Vec::new(),
        title_block: HashMap::new(),
        lib_symbols: HashMap::new(),
    };
    let path = PathBuf::from("click-click-move.snxsch");
    let engine = signex_engine::Engine::new(sheet.clone()).expect("engine");
    let (mut app, _task) = Signex::new();
    // The first-run tour claims Esc ahead of the tool reset.
    app.ui_state.first_run_tour_open = false;
    // Keep the drop delta exact; grid snapping is not under test here.
    app.ui_state.snap_enabled = false;
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "click-click-move".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.clone());
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(
            signex_app::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
    (app, path)
}

fn act(app: &mut Signex, action: ActiveBarAction) {
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(action)));
}

fn selection_of(uuid: uuid::Uuid) -> Vec<SelectedItem> {
    vec![SelectedItem::new(uuid, SelectedKind::Wire)]
}

fn wire_start(app: &Signex, path: &PathBuf, uuid: uuid::Uuid) -> Point {
    app.document_state.engines[path]
        .document()
        .wires
        .iter()
        .find(|w| w.uuid == uuid)
        .expect("wire exists")
        .start
}

#[test]
fn move_arms_the_object_pick_without_needing_a_selection() {
    let (mut app, _path) = app_with_wires(vec![wire(uuid::Uuid::new_v4(), 10.0)]);
    assert!(app.interaction_state.active_canvas().selected.is_empty());

    act(&mut app, ActiveBarAction::Move);

    assert_eq!(app.ui_state.move_pick, Some(MovePick::Object));
    assert_eq!(
        app.interaction_state.active_canvas().move_pick,
        Some(MovePick::Object),
        "the canvas must see the arm too, or the next press selects normally"
    );
    assert_eq!(app.interaction_state.current_tool, Tool::Select);
    assert!(
        !app.ui_state.move_selection.open,
        "Move must not open the dX / dY dialog"
    );
}

#[test]
fn move_picked_selects_only_the_object_under_the_cursor_and_disarms() {
    let a = uuid::Uuid::new_v4();
    let b = uuid::Uuid::new_v4();
    let (mut app, path) = app_with_wires(vec![wire(a, 10.0), wire(b, 40.0)]);
    app.interaction_state.active_canvas_mut().selected = selection_of(b);
    act(&mut app, ActiveBarAction::Move);

    let _ = app.update(Message::CanvasEvent(CanvasEvent::MovePicked {
        world_x: 20.0,
        world_y: 10.0,
    }));

    let selected = &app.interaction_state.active_canvas().selected;
    assert_eq!(selected.len(), 1, "exactly the picked object: {selected:?}");
    assert_eq!(selected[0].uuid, a);
    assert_eq!(app.ui_state.move_pick, None, "the pick consumes the arm");
    assert_eq!(app.interaction_state.active_canvas().move_pick, None);

    let _ = app.update(Message::CanvasEvent(CanvasEvent::MoveSelected {
        dx: 2.54,
        dy: 0.0,
    }));
    assert!(
        (wire_start(&app, &path, a).x - 12.54).abs() < 1e-6,
        "the drop must move the picked wire"
    );
    assert!(
        (wire_start(&app, &path, b).x - 10.0).abs() < 1e-6,
        "the previously selected wire must stay put"
    );
}

#[test]
fn move_selection_with_an_empty_selection_does_not_arm() {
    let (mut app, _path) = app_with_wires(vec![wire(uuid::Uuid::new_v4(), 10.0)]);

    act(&mut app, ActiveBarAction::MoveSelection);

    assert_eq!(app.ui_state.move_pick, None);
    assert_eq!(app.interaction_state.active_canvas().move_pick, None);
}

#[test]
fn move_selection_with_a_selection_arms_the_reference_pick() {
    let a = uuid::Uuid::new_v4();
    let (mut app, _path) = app_with_wires(vec![wire(a, 10.0)]);
    app.interaction_state.active_canvas_mut().selected = selection_of(a);

    act(&mut app, ActiveBarAction::MoveSelection);

    assert_eq!(app.ui_state.move_pick, Some(MovePick::Selection));
    assert_eq!(
        app.interaction_state.active_canvas().move_pick,
        Some(MovePick::Selection)
    );
    assert!(!app.ui_state.move_selection.open);
}

#[test]
fn move_selection_by_xy_still_opens_the_dialog() {
    let a = uuid::Uuid::new_v4();
    let (mut app, _path) = app_with_wires(vec![wire(a, 10.0)]);
    app.interaction_state.active_canvas_mut().selected = selection_of(a);

    act(&mut app, ActiveBarAction::MoveSelectionXY);

    assert!(app.ui_state.move_selection.open);
    assert_eq!(app.ui_state.move_pick, None);
}

#[test]
fn escape_clears_an_armed_pick() {
    let (mut app, _path) = app_with_wires(vec![wire(uuid::Uuid::new_v4(), 10.0)]);
    act(&mut app, ActiveBarAction::Move);

    let _ = app.update(Message::EscapePressed { window: None });

    assert_eq!(app.ui_state.move_pick, None);
    assert_eq!(app.interaction_state.active_canvas().move_pick, None);
}

#[test]
fn switching_back_to_select_clears_an_armed_pick() {
    let (mut app, _path) = app_with_wires(vec![wire(uuid::Uuid::new_v4(), 10.0)]);
    act(&mut app, ActiveBarAction::Move);

    let _ = app.update(Message::Tool(signex_app::toolbar::ToolMessage::SelectTool(
        Tool::Select,
    )));

    assert_eq!(app.ui_state.move_pick, None);
    assert_eq!(app.interaction_state.active_canvas().move_pick, None);
}

#[test]
fn right_click_while_armed_cancels_instead_of_opening_the_context_menu() {
    let (mut app, _path) = app_with_wires(vec![wire(uuid::Uuid::new_v4(), 10.0)]);
    act(&mut app, ActiveBarAction::Move);

    let _ = app.update(Message::ContextMenu(signex_app::app::ContextMenuMsg::Show(
        50.0, 50.0,
    )));

    assert_eq!(app.ui_state.move_pick, None);
    assert!(app.interaction_state.context_menu.is_none());
}

#[test]
fn move_pick_honours_the_selection_filter() {
    let a = uuid::Uuid::new_v4();
    let (mut app, _path) = app_with_wires(vec![wire(a, 10.0)]);
    app.interaction_state
        .selection_filters
        .remove(&signex_app::active_bar::SelectionFilter::Wires);
    act(&mut app, ActiveBarAction::Move);

    let _ = app.update(Message::CanvasEvent(CanvasEvent::MovePicked {
        world_x: 20.0,
        world_y: 10.0,
    }));

    assert!(
        app.interaction_state.active_canvas().selected.is_empty(),
        "a wire filtered out of selection must not be picked"
    );
}

#[test]
fn picking_a_placement_tool_clears_an_armed_pick() {
    let (mut app, _path) = app_with_wires(vec![wire(uuid::Uuid::new_v4(), 10.0)]);
    act(&mut app, ActiveBarAction::Move);

    let _ = app.update(Message::Tool(signex_app::toolbar::ToolMessage::SelectTool(
        Tool::Wire,
    )));

    assert_eq!(
        app.ui_state.move_pick, None,
        "the armed Move must not outlive a switch to Place Wire"
    );
    assert_eq!(app.interaction_state.active_canvas().move_pick, None);
}

#[test]
fn switching_tabs_cancels_an_armed_or_floating_move() {
    let (mut app, _path) = app_with_wires(vec![wire(uuid::Uuid::new_v4(), 10.0)]);
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(window);
    let other = PathBuf::from("click-click-move-b.snxsch");
    let sheet = app
        .document_state
        .engines
        .values()
        .next()
        .unwrap()
        .document()
        .clone();
    app.document_state.engines.insert(
        other.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "b".to_string(),
        path: other,
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });
    act(&mut app, ActiveBarAction::Move);
    let epoch = app.interaction_state.active_canvas().move_epoch;

    let _ = app.update(Message::Tab {
        window_id: window,
        msg: signex_app::tab_bar::TabMessage::Select(1),
    });

    assert_eq!(app.document_state.active_tab, 1);
    assert_eq!(
        app.ui_state.move_pick, None,
        "the arm must not follow the tab"
    );
    assert!(
        app.interaction_state.active_canvas().move_epoch > epoch,
        "a float started on tab A must be cancelled for tab B"
    );
}

#[test]
fn an_escape_the_canvas_never_saw_cancels_a_floating_move_via_the_epoch() {
    let (mut app, _path) = app_with_wires(vec![wire(uuid::Uuid::new_v4(), 10.0)]);
    app.ui_state.first_run_tour_open = false;
    act(&mut app, ActiveBarAction::Move);
    let epoch = app.interaction_state.active_canvas().move_epoch;

    let _ = app.update(Message::EscapePressed { window: None });

    assert!(
        app.interaction_state.active_canvas().move_epoch > epoch,
        "Esc handled by the app must also end a float in the canvas widget"
    );
}

#[test]
fn undo_cancels_a_floating_move() {
    let (mut app, _path) = app_with_wires(vec![wire(uuid::Uuid::new_v4(), 10.0)]);
    act(&mut app, ActiveBarAction::Move);
    let epoch = app.interaction_state.active_canvas().move_epoch;

    let _ = app.update(Message::Edit(signex_app::app::EditMsg::Undo));

    assert_eq!(app.ui_state.move_pick, None);
    assert!(app.interaction_state.active_canvas().move_epoch > epoch);
}

#[test]
fn a_pick_with_no_object_under_it_cancels_the_float_instead_of_moving_the_old_selection() {
    let a = uuid::Uuid::new_v4();
    let (mut app, _path) = app_with_wires(vec![wire(a, 10.0)]);
    app.interaction_state.active_canvas_mut().selected = selection_of(a);
    act(&mut app, ActiveBarAction::Move);
    let epoch = app.interaction_state.active_canvas().move_epoch;

    let _ = app.update(Message::CanvasEvent(CanvasEvent::MovePicked {
        world_x: 200.0,
        world_y: 200.0,
    }));

    assert!(
        app.interaction_state.active_canvas().move_epoch > epoch,
        "the canvas float must be cancelled when the app finds nothing to lift"
    );
}
