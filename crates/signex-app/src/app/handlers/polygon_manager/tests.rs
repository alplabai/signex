//! Mockup row 178: the Polygon Manager tab and the PCB panel's Polygons
//! mode through `Signex::update`. One model, the same engine commands as
//! Properties, one undo step per edit.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use pcb_geom::ItemRef;
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex, TabKind};
use crate::dock::DockMessage;
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;
use crate::panels::polygons::{PolygonMsg, PourField, PourState};
use crate::panels::{PanelMsg, PcbPanelMode, PcbPanelMsg};
use crate::pcb_canvas::PcbCanvasMsg;

/// A GND pour on the top copper, one on the bottom, and a VCC pour on the
/// top copper with a higher priority, so it fills first.
const TOP_GND: u128 = 11;
const BOTTOM_GND: u128 = 12;
const TOP_VCC: u128 = 13;

fn pour(n: u128, net: u32, name: &str, layer: &str, priority: u32) -> serde_json::Value {
    serde_json::json!({
        "uuid": Uuid::from_u128(n),
        "net": net,
        "net_name": name,
        "layer": layer,
        "fill_type": "pour",
        "priority": priority,
        "outline": [
            {"x": 0.0, "y": 0.0},
            {"x": 10.0, "y": 0.0},
            {"x": 10.0, "y": 10.0},
            {"x": 0.0, "y": 10.0},
        ],
    })
}

fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "thickness": 1.6,
        "layers": [
            {"id": 0, "name": "TopCopper", "layer_type": "signal"},
            {"id": 31, "name": "BottomCopper", "layer_type": "signal"},
        ],
        "zones": [
            pour(TOP_GND, 1, "GND", "TopCopper", 0),
            pour(BOTTOM_GND, 1, "GND", "BottomCopper", 0),
            pour(TOP_VCC, 2, "VCC", "TopCopper", 5),
        ],
    }))
    .unwrap()
}

fn app_with(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board());
    (app, path)
}

fn run_command(app: &mut Signex) {
    let id = AppCommandId::new("polygon_manager").unwrap();
    let _ = app.update(Message::Menu(MenuMessage::Command(id)));
}

fn send(app: &mut Signex, msg: PolygonMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::PcbPanel(
        PcbPanelMsg::Polygons(msg),
    ))));
}

fn pages(app: &Signex) -> usize {
    let tabs = &app.document_state.tabs;
    tabs.iter().filter(|t| t.kind.is_polygon_manager()).count()
}

fn active_is_page(app: &Signex) -> bool {
    let docs = &app.document_state;
    docs.tabs[docs.active_tab].kind.is_polygon_manager()
}

fn zone(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn zone_of<'a>(app: &'a Signex, path: &Path, n: u128) -> &'a signex_types::pcb::Zone {
    let board = app.document_state.pcb_board(path).unwrap();
    board.zones.iter().find(|z| z.uuid == zone(n)).unwrap()
}

fn tick(app: &mut Signex, path: &Path, n: u128) {
    send(
        app,
        PolygonMsg::RowTicked {
            board: path.to_path_buf(),
            zone: zone(n),
            on: true,
        },
    );
}

#[test]
fn the_command_opens_one_tab_after_the_board_and_focuses_it_again() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path());
    run_command(&mut app);
    assert!(active_is_page(&app));
    let board_at = app
        .document_state
        .tabs
        .iter()
        .position(|t| matches!(t.kind, TabKind::Pcb))
        .unwrap();
    assert_eq!(app.document_state.active_tab, board_at + 1);
    assert_eq!(app.document_state.tabs[board_at + 1].path, path);
    let title = &app.document_state.tabs[board_at + 1].title;
    assert!(title.ends_with("Polygons"), "{title}");

    app.document_state.active_tab = board_at;
    app.sync_active_tab();
    run_command(&mut app);
    assert_eq!(pages(&app), 1, "one tab per board");
    assert!(active_is_page(&app));
}

#[test]
fn the_panel_link_opens_the_same_tab() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with(dir.path());
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::PcbPanel(
        PcbPanelMsg::ModePicked(PcbPanelMode::Polygons),
    ))));
    send(&mut app, PolygonMsg::ManagerOpened);
    assert!(active_is_page(&app));
    send(&mut app, PolygonMsg::ManagerOpened);
    assert_eq!(pages(&app), 1);
}

#[test]
fn the_rows_are_grouped_by_layer_in_pour_order() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with(dir.path());
    run_command(&mut app);
    let facts = app.document_view.panel_ctx.polygons.clone().expect("rows");
    let order: Vec<(Uuid, usize, Option<usize>)> = facts
        .rows
        .iter()
        .map(|row| (row.zone, row.layer_place, row.order))
        .collect();
    assert_eq!(
        order,
        [
            (zone(TOP_VCC), 1, Some(1)),
            (zone(TOP_GND), 1, Some(2)),
            (zone(BOTTOM_GND), 2, Some(1)),
        ]
    );
    let vcc = &facts.rows[0];
    assert_eq!(vcc.net, "VCC");
    assert_eq!(vcc.name, "VCC_L01_P003", "the board's naming scheme");
    assert_eq!(vcc.state, PourState::Poured);
    assert!(vcc.area_mm2.is_some(), "the fill is read on open");
}

#[test]
fn a_typed_clearance_is_one_undo_step_on_the_board() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path());
    run_command(&mut app);
    send(
        &mut app,
        PolygonMsg::FieldTyped {
            board: path.clone(),
            zone: zone(TOP_GND),
            field: PourField::Clearance,
            text: "0.5".into(),
        },
    );
    send(
        &mut app,
        PolygonMsg::FieldSubmitted {
            board: path.clone(),
            zone: zone(TOP_GND),
            field: PourField::Clearance,
        },
    );
    assert_eq!(zone_of(&app, &path, TOP_GND).clearance, 0.5);
    assert!(app.document_state.is_document_dirty(&path));
    let row = app.document_view.panel_ctx.polygons.as_ref().unwrap();
    let row = row.row(zone(TOP_GND)).unwrap();
    assert_eq!(row.clearance_mm, 0.5, "the row follows the board");
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(zone_of(&app, &path, TOP_GND).clearance, 0.0);
    assert!(active_is_page(&app), "undo stays on the tab");
}

#[test]
fn text_that_is_no_number_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path());
    run_command(&mut app);
    let before = serde_json::to_string(app.document_state.pcb_board(&path).unwrap()).unwrap();
    send(
        &mut app,
        PolygonMsg::FieldTyped {
            board: path.clone(),
            zone: zone(TOP_GND),
            field: PourField::MinWidth,
            text: "wide".into(),
        },
    );
    send(
        &mut app,
        PolygonMsg::FieldSubmitted {
            board: path.clone(),
            zone: zone(TOP_GND),
            field: PourField::MinWidth,
        },
    );
    let after = serde_json::to_string(app.document_state.pcb_board(&path).unwrap()).unwrap();
    assert_eq!(before, after);
}

#[test]
fn a_bulk_edit_changes_every_ticked_pour_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path());
    run_command(&mut app);
    tick(&mut app, &path, TOP_GND);
    tick(&mut app, &path, BOTTOM_GND);
    send(
        &mut app,
        PolygonMsg::BulkTyped {
            board: path.clone(),
            field: PourField::MinWidth,
            text: "0.3".into(),
        },
    );
    send(
        &mut app,
        PolygonMsg::BulkSubmitted {
            board: path.clone(),
            field: PourField::MinWidth,
        },
    );
    assert_eq!(zone_of(&app, &path, TOP_GND).min_thickness, 0.3);
    assert_eq!(zone_of(&app, &path, BOTTOM_GND).min_thickness, 0.3);
    assert_eq!(
        zone_of(&app, &path, TOP_VCC).min_thickness,
        0.0,
        "not ticked"
    );
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(zone_of(&app, &path, TOP_GND).min_thickness, 0.0);
    assert_eq!(zone_of(&app, &path, BOTTOM_GND).min_thickness, 0.0);
}

#[test]
fn shelve_selected_shelves_the_ticked_pours_and_frees_their_place() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path());
    run_command(&mut app);
    tick(&mut app, &path, TOP_VCC);
    send(
        &mut app,
        PolygonMsg::ShelveSelectedPressed {
            board: path.clone(),
        },
    );
    assert!(zone_of(&app, &path, TOP_VCC).shelved);
    assert!(!zone_of(&app, &path, TOP_GND).shelved);
    let facts = app.document_view.panel_ctx.polygons.clone().unwrap();
    let vcc = facts.row(zone(TOP_VCC)).unwrap();
    assert_eq!((vcc.state, vcc.order), (PourState::Shelved, None));
    assert_eq!(facts.row(zone(TOP_GND)).unwrap().order, Some(1));
    send(
        &mut app,
        PolygonMsg::UnshelveAllPressed {
            board: path.clone(),
        },
    );
    assert!(!zone_of(&app, &path, TOP_VCC).shelved);
}

#[test]
fn a_click_in_either_view_picks_the_same_pour() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path());
    send(
        &mut app,
        PolygonMsg::RowClicked {
            board: path.clone(),
            zone: zone(BOTTOM_GND),
        },
    );
    let ui = &app.document_view.panel_ctx.pcb_panel.polygons;
    assert_eq!(ui.current, Some(zone(BOTTOM_GND)));
    run_command(&mut app);
    let ui = &app.document_view.panel_ctx.pcb_panel.polygons;
    assert_eq!(ui.current, Some(zone(BOTTOM_GND)), "the tab shows it too");
}

#[test]
fn closing_the_board_closes_its_polygon_manager_tab() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path());
    run_command(&mut app);
    app.open_layer_stack_tab();
    let board_at = app
        .document_state
        .tabs
        .iter()
        .position(|t| matches!(t.kind, TabKind::Pcb))
        .unwrap();
    let _ = app.close_tab_parking_dirty(board_at);
    assert_eq!(pages(&app), 0);
    assert!(
        !app.document_state
            .tabs
            .iter()
            .any(|t| t.kind.is_board_page()),
        "every page of the board closes with it"
    );
    assert!(!app.document_state.has_pcb_tab(&path));
}

/// Type `text` in a cell of pour `n` and press Enter.
fn type_into(app: &mut Signex, path: &Path, n: u128, field: PourField, text: &str) {
    let (board, zone) = (path.to_path_buf(), zone(n));
    let typed = PolygonMsg::FieldTyped {
        board: board.clone(),
        zone,
        field,
        text: text.into(),
    };
    send(app, typed);
    send(app, PolygonMsg::FieldSubmitted { board, zone, field });
}

/// The tab of `path` that is its Polygon Manager (`page`) or its board.
fn tab_of(app: &Signex, path: &Path, page: bool) -> usize {
    let tabs = &app.document_state.tabs;
    let wanted = |kind: &TabKind| {
        if page {
            kind.is_polygon_manager()
        } else {
            matches!(kind, TabKind::Pcb)
        }
    };
    tabs.iter()
        .position(|t| t.path == path && wanted(&t.kind))
        .unwrap()
}

fn show(app: &mut Signex, at: usize) {
    app.document_state.active_tab = at;
    app.sync_active_tab();
}

#[test]
fn coming_back_from_another_board_shows_the_fills_again() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, a) = app_with(dir.path());
    run_command(&mut app);
    let b = dir.path().join("other.snxpcb");
    app.open_pcb_tab(b.clone(), "other".into(), board());
    let at = tab_of(&app, &b, false);
    show(&mut app, at);
    assert!(
        app.current_pour_fills(&a).is_none(),
        "the other board's canvas filled the other board"
    );
    let at = tab_of(&app, &a, true);
    show(&mut app, at);
    let facts = app.document_view.panel_ctx.polygons.clone().expect("rows");
    assert_eq!(facts.board, a);
    assert!(
        facts.rows.iter().all(|row| row.area_mm2.is_some()),
        "every pour shows its fill again"
    );
}

#[test]
fn an_empty_clearance_follows_the_board_clearance_again() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path());
    run_command(&mut app);
    type_into(&mut app, &path, TOP_GND, PourField::Clearance, "0.5");
    assert_eq!(zone_of(&app, &path, TOP_GND).clearance, 0.5);
    type_into(&mut app, &path, TOP_GND, PourField::Clearance, " ");
    assert_eq!(zone_of(&app, &path, TOP_GND).clearance, 0.0);
}

#[test]
fn undo_drops_the_text_typed_and_not_applied() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path());
    run_command(&mut app);
    type_into(&mut app, &path, TOP_GND, PourField::Clearance, "0.5");
    let typed = PolygonMsg::FieldTyped {
        board: path.clone(),
        zone: zone(TOP_GND),
        field: PourField::Clearance,
        text: "0.7".into(),
    };
    send(&mut app, typed);
    let _ = app.update(Message::Edit(EditMsg::Undo));
    let ui = &app.document_view.panel_ctx.pcb_panel.polygons;
    assert_eq!(ui.draft(zone(TOP_GND), PourField::Clearance), None);
    let facts = app.document_view.panel_ctx.polygons.as_ref().unwrap();
    assert_eq!(facts.row(zone(TOP_GND)).unwrap().clearance_mm, 0.0);
}

#[test]
fn a_right_click_on_a_pour_opens_the_manager_with_that_pour_ticked() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path());
    let _ = app.update(Message::PcbCanvas(PcbCanvasMsg::PointerMoved {
        at: Point::new(0.0, 5.0),
        zoom_pct: 100.0,
        tolerance_mm: 0.05,
    }));
    let Some(ItemRef::Zone { zone: hovered, .. }) = app.interaction_state.pcb_selection.hover()
    else {
        panic!("the pointer is on the edge of the pours");
    };
    assert!(app.selected_pours(&path).is_empty(), "nothing is selected");
    run_command(&mut app);
    let ui = &app.document_view.panel_ctx.pcb_panel.polygons;
    assert_eq!(ui.selected, BTreeSet::from([hovered]));
    assert_eq!(ui.current, Some(hovered));
}
