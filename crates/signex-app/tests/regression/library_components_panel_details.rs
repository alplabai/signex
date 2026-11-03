//! Components panel: a row click only selects and fills the details section,
//! dragging a row onto the schematic starts the placement, the right-click
//! menu places or opens, and the divider above the details drags and clamps.
//!
//! Real `Stock.snxlib` mounted read-only the way the app mounts it; the
//! tasks every message spawns run to completion and feed back as the iced
//! runtime would.

use std::path::PathBuf;

use signex_app::app::{Message, Signex, UiMsg};
use signex_app::library::LibraryMessage;
use signex_app::library::state::{ComponentsPanelRowKey, ComponentsRowAction, EditorAddress};
use signex_library::RowId;

use crate::library_place_symbol::{PLACE_AT, app_with_empty_sheet, click, doc, run};
use crate::settle_support::{settle, settle_task};

fn stock_file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/stock-library/Stock.snxlib")
}

fn app_with_stock() -> Signex {
    let mut app = app_with_empty_sheet();
    let stock = stock_file();
    app.library.stock_library_root = Some(stock.clone());
    app.library
        .open_library_read_only(stock)
        .expect("mount stock");
    app
}

fn key_of(app: &Signex, pn: &str) -> ComponentsPanelRowKey {
    let (_, row) = app
        .library
        .all_components()
        .into_iter()
        .find(|(_, c)| c.internal_pn.as_str() == pn)
        .unwrap_or_else(|| panic!("Stock lists {pn}"));
    ComponentsPanelRowKey {
        library_path: stock_file(),
        table: app
            .library
            .table_of_row(&stock_file(), row.row_id)
            .expect("table"),
        row_id: RowId::from_uuid(row.row_id),
    }
}

fn press(key: &ComponentsPanelRowKey) -> Message {
    Message::Library(LibraryMessage::ComponentsPanelRowPressed(key.clone()))
}

fn pointer(app: &mut Signex, at: (f32, f32)) {
    let _ = app.update(Message::Ui(UiMsg::DragMove(at.0, at.1)));
}

/// A point inside the schematic canvas of the test window.
fn over_canvas(app: &Signex) -> (f32, f32) {
    (
        app.ui_state.window_size.0 / 2.0,
        app.ui_state.window_size.1 / 2.0,
    )
}

/// Press `key` at `from`, move to `to`, release.
fn drag_row(app: &mut Signex, key: &ComponentsPanelRowKey, from: (f32, f32), to: (f32, f32)) {
    pointer(app, from);
    settle(app, press(key));
    settle(app, Message::Ui(UiMsg::DragMove(to.0, to.1)));
    settle(app, Message::Ui(UiMsg::DragEnd));
}

#[test]
fn a_click_only_selects_and_a_double_click_places_nothing() {
    let mut app = app_with_stock();
    let key = key_of(&app, "STOCK-R");
    let tabs = app.document_state.tabs.len();
    let at = over_canvas(&app);

    for _ in 0..2 {
        pointer(&mut app, at);
        settle(&mut app, press(&key));
        settle(&mut app, Message::Ui(UiMsg::DragEnd));
    }

    assert_eq!(app.library.components_panel.selected.as_ref(), Some(&key));
    assert_eq!(app.document_state.tabs.len(), tabs, "no tab opened");
    assert!(app.library.editors.is_empty(), "no preview opened");
    assert!(app.interaction_state.placement.is_none(), "nothing armed");
    assert!(doc(&app).symbols.is_empty(), "nothing placed");
}

#[test]
fn selecting_stock_r_loads_its_symbol_and_footprint() {
    let mut app = app_with_stock();
    let key = key_of(&app, "STOCK-R");
    settle(&mut app, press(&key));

    let details = &app.library.components_panel.details;
    assert_eq!(details.key.as_ref(), Some(&key));
    let loaded = details.loaded.as_deref().expect("details loaded");
    assert_eq!(loaded.internal_pn, "STOCK-R");
    assert!(loaded.symbol.is_some(), "symbol resolved");
    assert!(loaded.footprint.is_some(), "footprint resolved");
    assert!(details.error.is_none());
}

#[test]
fn a_load_for_a_row_no_longer_selected_is_dropped() {
    let mut app = app_with_stock();
    let first = key_of(&app, "STOCK-R");
    let second = key_of(&app, "STOCK-IC_Generic");

    // Both reads are in flight before either result lands.
    let first_task = app.update(press(&first));
    let second_task = app.update(press(&second));
    let late: Vec<Message> = run(first_task);
    assert!(!late.is_empty(), "the first read finished");
    for message in late {
        let _ = app.update(message);
    }
    assert!(
        app.library.components_panel.details.loaded.is_none(),
        "the stale result must not be shown"
    );

    settle_task(&mut app, second_task);
    let loaded = app
        .library
        .components_panel
        .details
        .loaded
        .as_deref()
        .expect("second row loaded");
    assert_eq!(loaded.internal_pn, "STOCK-IC_Generic");
}

#[test]
fn dragging_the_divider_changes_and_clamps_the_list_height() {
    let mut app = app_with_stock();
    let start = app.library.components_panel.list_height;

    let _ = app.update(Message::Library(
        LibraryMessage::ComponentsPanelSplitDragStarted,
    ));
    pointer(&mut app, (100.0, 400.0));
    pointer(&mut app, (100.0, 440.0));
    assert_eq!(app.library.components_panel.list_height, start + 40.0);

    pointer(&mut app, (100.0, -100_000.0));
    let min = app.library.components_panel.list_height;
    assert!(min < start, "dragging up shrinks the list");
    pointer(&mut app, (100.0, 100_000.0));
    let max = app.library.components_panel.list_height;
    assert!(max > start, "dragging down grows the list");

    let _ = app.update(Message::Ui(UiMsg::DragEnd));
    pointer(&mut app, (100.0, 0.0));
    assert_eq!(
        app.library.components_panel.list_height, max,
        "released: the pointer no longer drives it"
    );
}

#[test]
fn a_row_dropped_on_the_canvas_places_once_at_the_release_point() {
    let mut app = app_with_stock();
    let key = key_of(&app, "STOCK-R");
    let canvas = over_canvas(&app);

    drag_row(&mut app, &key, (20.0, canvas.1), canvas);

    assert!(
        app.interaction_state.placement.is_none(),
        "the tool ends with the drop"
    );
    assert_eq!(doc(&app).symbols.len(), 1, "placed in one gesture");
    assert_eq!(doc(&app).symbols[0].row_id, Some(key.row_id.0));
    let first = doc(&app).symbols[0].position;
    let (_, _, scale) = app.interaction_state.active_canvas().live_camera();
    let grid = (f64::from(app.ui_state.grid_size_mm) * 1000.0).round() / 1000.0;
    assert!(
        (first.x / grid - (first.x / grid).round()).abs() < 1e-6,
        "snapped to the grid: {first:?}"
    );

    // Dropping further right lands further right by the pointer travel.
    drag_row(
        &mut app,
        &key,
        (20.0, canvas.1),
        (canvas.0 + 200.0, canvas.1),
    );
    assert_eq!(doc(&app).symbols.len(), 2);
    let second = doc(&app).symbols[1].position;
    let travel_mm = f64::from(200.0 / scale);
    assert!(
        (second.x - first.x - travel_mm).abs() <= grid,
        "{first:?} -> {second:?}, expected about {travel_mm} mm"
    );

    // One undo step per drop.
    let _ = app.update(Message::Edit(signex_app::app::EditMsg::Undo));
    assert_eq!(doc(&app).symbols.len(), 1);
}

#[test]
fn the_drag_label_shows_while_dragging_and_goes_after_release() {
    let mut app = app_with_stock();
    let key = key_of(&app, "STOCK-R");
    let canvas = over_canvas(&app);
    let shown = |app: &Signex| {
        app.library
            .components_panel
            .drag
            .as_ref()
            .is_some_and(|d| d.started)
    };

    pointer(&mut app, canvas);
    settle(&mut app, press(&key));
    assert!(!shown(&app), "a press alone shows no label");
    settle(
        &mut app,
        Message::Ui(UiMsg::DragMove(canvas.0 + 40.0, canvas.1)),
    );
    assert!(shown(&app), "past the threshold the label follows");
    settle(&mut app, Message::Ui(UiMsg::DragEnd));
    assert!(!shown(&app) && app.library.components_panel.drag.is_none());

    // Esc ends it too.
    pointer(&mut app, canvas);
    settle(&mut app, press(&key));
    settle(
        &mut app,
        Message::Ui(UiMsg::DragMove(canvas.0 + 40.0, canvas.1)),
    );
    let _ = app.update(Message::Input(signex_app::app::InputMsg::EscapePressed {
        window: None,
    }));
    assert!(app.library.components_panel.drag.is_none(), "Esc cancels");
    settle(&mut app, Message::Ui(UiMsg::DragEnd));
    assert!(app.interaction_state.placement.is_none());
    assert_eq!(doc(&app).symbols.len(), 1, "only the first drop placed");
}

/// #1943: once the drag is over the canvas the symbol itself is on the
/// cursor, through the same ghost as Place Component.
#[test]
fn the_symbol_ghost_follows_a_row_dragged_over_the_canvas() {
    let mut app = app_with_stock();
    let key = key_of(&app, "STOCK-R");
    let canvas = over_canvas(&app);
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(window);

    pointer(&mut app, (20.0, canvas.1));
    settle(&mut app, press(&key));
    pointer(&mut app, (20.0 + 40.0, canvas.1));
    assert!(
        app.placement_ghost_for_window(window).is_none(),
        "no ghost while the drag is over the panel"
    );

    // The pointer reaches the canvas: the read runs and the ghost arrives.
    settle(&mut app, Message::Ui(UiMsg::DragMove(canvas.0, canvas.1)));
    let ghost = app
        .placement_ghost_for_window(window)
        .expect("the symbol ghost is on the cursor");
    let parts = &ghost.fragment().symbols;
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].row_id, Some(key.row_id.0), "STOCK-R's symbol");
    assert!(doc(&app).symbols.is_empty(), "a ghost places nothing");

    // Dropping places the part and takes the ghost away.
    settle(&mut app, Message::Ui(UiMsg::DragEnd));
    assert_eq!(doc(&app).symbols.len(), 1);
    assert!(app.placement_ghost_for_window(window).is_none());
}

/// #1943: the ghost goes when the drag ends anywhere but the canvas, and
/// with Esc.
#[test]
fn the_symbol_ghost_goes_when_the_drag_is_released_off_the_canvas_or_escaped() {
    let mut app = app_with_stock();
    let key = key_of(&app, "STOCK-R");
    let canvas = over_canvas(&app);

    pointer(&mut app, (20.0, canvas.1));
    settle(&mut app, press(&key));
    settle(&mut app, Message::Ui(UiMsg::DragMove(canvas.0, canvas.1)));
    assert!(app.interaction_state.placement.is_some(), "ghost armed");
    pointer(&mut app, (canvas.0, 4.0));
    settle(&mut app, Message::Ui(UiMsg::DragEnd));
    assert!(app.interaction_state.placement.is_none(), "ghost gone");
    assert!(doc(&app).symbols.is_empty());

    pointer(&mut app, (20.0, canvas.1));
    settle(&mut app, press(&key));
    settle(&mut app, Message::Ui(UiMsg::DragMove(canvas.0, canvas.1)));
    assert!(app.interaction_state.placement.is_some());
    let _ = app.update(Message::Input(signex_app::app::InputMsg::EscapePressed {
        window: None,
    }));
    assert!(app.interaction_state.placement.is_none(), "Esc drops it");
    assert!(doc(&app).symbols.is_empty());
}

#[test]
fn hover_follows_the_pointer_entering_and_leaving_a_row() {
    let mut app = app_with_stock();
    let a = key_of(&app, "STOCK-R");
    let b = key_of(&app, "STOCK-IC_Generic");
    let send = |app: &mut Signex, m| {
        let _ = app.update(Message::Library(m));
    };

    send(
        &mut app,
        LibraryMessage::ComponentsPanelRowEntered(a.clone()),
    );
    assert_eq!(app.library.components_panel.hovered.as_ref(), Some(&a));
    // Entering the next row before the exit of the previous one arrives.
    send(
        &mut app,
        LibraryMessage::ComponentsPanelRowEntered(b.clone()),
    );
    send(&mut app, LibraryMessage::ComponentsPanelRowExited(a));
    assert_eq!(app.library.components_panel.hovered.as_ref(), Some(&b));
    send(&mut app, LibraryMessage::ComponentsPanelRowExited(b));
    assert!(app.library.components_panel.hovered.is_none());
}

#[test]
fn a_row_dragged_and_released_off_the_canvas_places_nothing() {
    let mut app = app_with_stock();
    let key = key_of(&app, "STOCK-R");
    let canvas = over_canvas(&app);

    // Released over the menu bar.
    drag_row(&mut app, &key, canvas, (canvas.0 + 200.0, 4.0));

    assert!(app.interaction_state.placement.is_none());
    assert!(doc(&app).symbols.is_empty());
    assert!(app.library.components_panel.drag.is_none(), "drag over");
}

/// #1908: only a schematic tab takes a dropped row; over any other tab the
/// drop places nothing and waits for nothing.
#[test]
fn a_row_dropped_on_a_tab_that_is_not_a_schematic_places_nothing() {
    let mut app = app_with_stock();
    let key = key_of(&app, "STOCK-R");
    let canvas = over_canvas(&app);
    let active = app.document_state.active_tab;
    app.document_state.tabs[active].kind = signex_app::app::TabKind::Pcb;

    drag_row(&mut app, &key, (20.0, canvas.1), canvas);

    assert!(app.interaction_state.placement.is_none());
    assert!(app.library.components_panel.pending_drop.is_none());
    assert!(doc(&app).symbols.is_empty());
    assert!(app.library.components_panel.drag.is_none(), "drag over");
}

#[test]
fn a_press_that_never_leaves_the_threshold_is_a_plain_click() {
    let mut app = app_with_stock();
    let key = key_of(&app, "STOCK-R");
    let canvas = over_canvas(&app);

    drag_row(&mut app, &key, canvas, (canvas.0 + 1.0, canvas.1));

    assert!(app.interaction_state.placement.is_none());
    assert_eq!(app.library.components_panel.selected.as_ref(), Some(&key));
}

#[test]
fn the_row_menu_places_once_and_opens_the_preview() {
    let mut app = app_with_stock();
    let key = key_of(&app, "STOCK-R");

    settle(
        &mut app,
        Message::Library(LibraryMessage::ComponentsPanelRowRightClicked(key.clone())),
    );
    let menu = app
        .library
        .components_panel
        .row_menu
        .as_ref()
        .expect("right-click opens the menu");
    assert_eq!(menu.key, key);
    assert_eq!(app.library.components_panel.selected.as_ref(), Some(&key));
    assert!(app.interaction_state.placement.is_none());

    settle(
        &mut app,
        Message::Library(LibraryMessage::ComponentsPanelRowMenuPicked(
            ComponentsRowAction::Place,
        )),
    );
    assert!(app.library.components_panel.row_menu.is_none());
    assert!(app.interaction_state.placement.is_some());
    click(&mut app, PLACE_AT);
    assert_eq!(doc(&app).symbols.len(), 1, "placed once");

    settle(
        &mut app,
        Message::Library(LibraryMessage::ComponentsPanelRowRightClicked(key.clone())),
    );
    settle(
        &mut app,
        Message::Library(LibraryMessage::ComponentsPanelRowMenuPicked(
            ComponentsRowAction::Open,
        )),
    );
    let address = EditorAddress::new(key.library_path.clone(), key.table.clone(), key.row_id);
    assert!(
        app.library.editors.contains_key(&address),
        "Open starts the Component Preview"
    );
}

fn param_values(app: &Signex) -> Vec<(String, String)> {
    app.library
        .components_panel
        .details
        .loaded
        .as_deref()
        .map(|d| d.parameters.clone())
        .unwrap_or_default()
}

#[test]
fn the_details_follow_a_saved_row_and_a_click_retries_after_an_error() {
    use crate::library_row_base_local::{
        PARAM, TABLE, library_with_a_measurement_row, open_app_with_preview, save_preview,
    };
    let tmp = tempfile::Builder::new()
        .prefix("signex-details-")
        .tempdir()
        .expect("tempdir");
    let (snxlib, row_id) = library_with_a_measurement_row(tmp.path());
    let (mut app, address) = open_app_with_preview(&snxlib, row_id);
    let key = ComponentsPanelRowKey {
        library_path: snxlib.clone(),
        table: TABLE.to_string(),
        row_id,
    };

    settle(&mut app, press(&key));
    let shown = param_values(&app);
    assert!(
        shown.iter().any(|(n, v)| n == PARAM && v.contains("3.3")),
        "{shown:?}"
    );

    // Edit and save through the Component Preview; any library message
    // after the refresh lands lets the pane notice the row changed.
    let state = app.library.editors.get_mut(&address).expect("preview");
    state.row.parameters.insert(
        PARAM.into(),
        signex_library::ParamValue::Text("9.99 V".into()),
    );
    state.dirty = true;
    save_preview(&mut app, &address);
    drop(app.land_pending_refresh(&snxlib));
    settle(
        &mut app,
        Message::Library(LibraryMessage::ComponentsPanelRowEntered(key.clone())),
    );
    let shown = param_values(&app);
    assert!(
        shown.iter().any(|(n, v)| n == PARAM && v.contains("9.99")),
        "the pane must show the saved value: {shown:?}"
    );

    // A failed read is retried by clicking the row again.
    let _ = app.update(Message::Library(
        LibraryMessage::ComponentsPanelDetailsLoaded {
            key: key.clone(),
            generation: app.library.components_panel.details.generation,
            result: Err("boom".into()),
        },
    ));
    assert!(app.library.components_panel.details.error.is_some());
    settle(&mut app, press(&key));
    settle(&mut app, Message::Ui(UiMsg::DragEnd));
    let details = &app.library.components_panel.details;
    assert!(details.error.is_none() && details.loaded.is_some());
}

#[test]
fn escape_closes_the_row_menu_and_leaves_the_tool_alone() {
    let mut app = app_with_stock();
    let key = key_of(&app, "STOCK-R");
    settle(
        &mut app,
        Message::Library(LibraryMessage::ComponentsPanelRowRightClicked(key)),
    );
    assert!(app.library.components_panel.row_menu.is_some());
    app.interaction_state.current_tool = signex_app::app::Tool::Wire;

    let _ = app.update(Message::Input(signex_app::app::InputMsg::EscapePressed {
        window: None,
    }));

    assert!(
        app.library.components_panel.row_menu.is_none(),
        "menu closed"
    );
    assert_eq!(
        app.interaction_state.current_tool,
        signex_app::app::Tool::Wire,
        "the canvas tool was not reset"
    );
}

#[test]
fn pressing_the_shown_row_again_keeps_its_details_and_reads_nothing() {
    let mut app = app_with_stock();
    let key = key_of(&app, "STOCK-R");
    settle(&mut app, press(&key));
    let generation = app.library.components_panel.details.generation;
    assert!(app.library.components_panel.details.loaded.is_some());

    // The press that starts a drag lands on the selected row.
    let task = app.update(press(&key));

    let details = &app.library.components_panel.details;
    assert!(details.loaded.is_some(), "no Loading state in between");
    assert_eq!(details.generation, generation, "no new read");
    assert!(run(task).is_empty(), "nothing was spawned");
}

#[test]
fn an_older_read_of_a_row_selected_again_cannot_overwrite_the_newer_one() {
    let mut app = app_with_stock();
    let a = key_of(&app, "STOCK-R");
    let b = key_of(&app, "STOCK-IC_Generic");

    let first_a = app.update(press(&a));
    let _b = app.update(press(&b));
    let second_a = app.update(press(&a));
    let newest = app.library.components_panel.details.generation;

    // The newer read of A lands first, then the older one.
    for message in run(second_a) {
        let _ = app.update(message);
    }
    for message in run(first_a) {
        let _ = app.update(message);
    }
    let _ = app.update(Message::Library(
        LibraryMessage::ComponentsPanelDetailsLoaded {
            key: a.clone(),
            generation: newest - 2,
            result: Err("older read".into()),
        },
    ));

    let details = &app.library.components_panel.details;
    assert_eq!(details.generation, newest);
    assert!(details.error.is_none(), "the older result was dropped");
    assert_eq!(
        details.loaded.as_deref().map(|d| d.internal_pn.as_str()),
        Some("STOCK-R")
    );
}
