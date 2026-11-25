//! #2364 — a component placed from the Components panel onto a board tab:
//! its footprint goes on the cursor (drawn by the board renderer), each click
//! places it on the grid as one undo step with the next designator, Space
//! turns and Y flips it, Esc and a right-click end it, the footprint picked in
//! the panel is the one placed, and a part the board cannot hold is refused
//! with its reason in Messages.
//!
//! The real `Stock.snxlib`: STOCK-D's default footprint is
//! Chip-0805-Polarised; STOCK-R offers six alternates.

use std::path::PathBuf;

use pcb_geom::ClickMode;
use signex_app::app::{ContextMenuMsg, EditMsg, FileMsg, InputMsg, Message, Signex};
use signex_app::diagnostics;
use signex_app::library::LibraryMessage;
use signex_app::library::messages::BoardPartError;
use signex_app::library::state::{ComponentsPanelRowKey, ComponentsRowAction};
use signex_app::pcb_canvas::{CarryKind, PcbCanvasMsg};
use signex_app::test_adapter::BoardEngines;
use signex_types::format::SnxPcb;
use signex_types::pcb::{Footprint, PCB_GRID_MM, PcbBoard, Point};
use uuid::Uuid;

use crate::library_footprint_pick::{alternates, key_of, select};
use crate::settle_support::settle;

const D3: Uuid = Uuid::from_u128(0x10);

fn stock_file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/stock-library/Stock.snxlib")
}

/// An empty board but for D3 at (0, 0).
fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "version": 1,
        "footprints": [{"uuid": D3, "reference": "D3", "position": {"x": 0.0, "y": 0.0},
                        "layer": "TopCopper"}]
    }))
    .unwrap()
}

pub(crate) struct Fixture {
    pub(crate) app: Signex,
    pub(crate) _tmp: tempfile::TempDir,
}

/// The board open in the active tab, with Stock mounted.
pub(crate) fn open() -> Fixture {
    let _ = diagnostics::init_logging();
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("board.snxpcb");
    std::fs::write(&path, SnxPcb::new(board()).write_string().unwrap()).unwrap();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let task = app.update(Message::File(FileMsg::Opened(Some(path))));
    crate::drive::drive(&mut app, task);
    let stock = stock_file();
    app.library.stock_library_root = Some(stock.clone());
    app.library
        .open_library_read_only(stock)
        .expect("mount stock");
    Fixture { app, _tmp: tmp }
}

fn live(app: &Signex) -> PcbBoard {
    let docs = &app.document_state;
    docs.tab_board_engine(docs.active_tab)
        .map(|engine| engine.board())
        .expect("a board is open")
        .clone()
}

pub(crate) fn text(app: &Signex) -> String {
    serde_json::to_string(&live(app)).unwrap()
}

/// What the board holds besides D3.
pub(crate) fn placed(app: &Signex) -> Vec<Footprint> {
    live(app)
        .footprints
        .into_iter()
        .filter(|f| f.uuid != D3)
        .collect()
}

/// Right-click the row and pick Place, as the panel does.
fn place(app: &mut Signex, key: &ComponentsPanelRowKey) {
    settle(
        app,
        Message::Library(LibraryMessage::ComponentsPanelRowRightClicked(key.clone())),
    );
    settle(
        app,
        Message::Library(LibraryMessage::ComponentsPanelRowMenuPicked(
            ComponentsRowAction::Place,
        )),
    );
}

pub(crate) fn carry(app: &Signex) -> Option<CarryKind> {
    app.interaction_state.pcb_carry.as_ref().map(|c| c.kind)
}

pub(crate) fn point_at(app: &mut Signex, x: f64, y: f64) {
    settle(
        app,
        Message::PcbCanvas(PcbCanvasMsg::PointerMoved {
            at: Point::new(x, y),
            zoom_pct: 100.0,
            tolerance_mm: 0.05,
        }),
    );
}

pub(crate) fn click(app: &mut Signex, x: f64, y: f64) {
    settle(
        app,
        Message::PcbCanvas(PcbCanvasMsg::Clicked {
            at: Point::new(x, y),
            tolerance_mm: 0.05,
            mode: ClickMode::Replace,
        }),
    );
}

pub(crate) fn on_grid(v: f64) -> f64 {
    (v / PCB_GRID_MM).round() * PCB_GRID_MM
}

fn close(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
}

#[test]
fn a_click_places_the_footprint_on_the_grid_in_one_step_and_keeps_placing() {
    let Fixture { mut app, _tmp } = open();
    let before = text(&app);
    let key = key_of(&app, "STOCK-D");
    select(&mut app, &key);
    place(&mut app, &key);
    assert_eq!(
        carry(&app),
        Some(CarryKind::Place),
        "the part is on the cursor"
    );
    assert!(
        app.library.components_panel.row_menu.is_none(),
        "the row menu closed"
    );

    point_at(&mut app, 10.1, 20.2);
    assert!(
        app.interaction_state.pcb_canvas.has_part_ghost(),
        "the footprint follows the pointer"
    );
    click(&mut app, 10.1, 20.2);
    let first = placed(&app);
    assert_eq!(first.len(), 1, "one footprint placed");
    let d4 = &first[0];
    let at = Point::new(on_grid(10.1), on_grid(20.2));
    assert!(close(d4.position, at), "{:?} on the grid", d4.position);
    assert_eq!(d4.reference, "D4", "numbered after D3");
    assert_eq!(d4.footprint_id, "Chip-0805-Polarised");
    assert_eq!(d4.pads.len(), 2, "the footprint's pads came with it");
    assert!(
        d4.library_ref.is_some(),
        "it remembers its library footprint"
    );
    assert_eq!(carry(&app), Some(CarryKind::Place), "still placing");

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(text(&app), before, "one undo takes the click back");

    click(&mut app, 10.1, 20.2);
    click(&mut app, 30.0, 20.2);
    let names: Vec<String> = placed(&app).into_iter().map(|f| f.reference).collect();
    assert_eq!(names, ["D4", "D5"], "each click places the next copy");
}

#[test]
fn space_turns_and_y_flips_the_part_on_the_cursor() {
    let Fixture { mut app, _tmp } = open();
    let key = key_of(&app, "STOCK-D");
    select(&mut app, &key);
    place(&mut app, &key);
    settle(&mut app, Message::Edit(EditMsg::RotateSelectedClockwise));
    settle(&mut app, Message::Edit(EditMsg::MirrorSelectedY));
    click(&mut app, 10.0, 10.0);
    let fp = placed(&app).pop().expect("placed");
    assert_eq!(fp.layer, "BottomCopper", "Y put it on the bottom");
    // 90° on top, mirrored to the bottom, reads 270°.
    assert_eq!(fp.rotation, 270.0);
    // The stock pads carry copper, mask and paste: all of them flip.
    assert!(
        fp.pads
            .iter()
            .all(|p| p.layers.iter().any(|l| l == "BottomCopper")
                && !p.layers.iter().any(|l| l.starts_with("Top"))),
        "{:?}",
        fp.pads.iter().map(|p| &p.layers).collect::<Vec<_>>()
    );
}

#[test]
fn esc_and_a_right_click_end_the_placement_and_place_nothing() {
    let Fixture { mut app, _tmp } = open();
    let before = text(&app);
    let key = key_of(&app, "STOCK-D");
    select(&mut app, &key);
    place(&mut app, &key);
    point_at(&mut app, 5.0, 5.0);
    settle(
        &mut app,
        Message::Input(InputMsg::EscapePressed { window: None }),
    );
    assert_eq!(carry(&app), None, "Esc ends it");
    assert!(!app.interaction_state.pcb_canvas.has_part_ghost());
    click(&mut app, 5.0, 5.0);
    assert_eq!(text(&app), before, "a click after Esc places nothing");

    place(&mut app, &key);
    assert_eq!(carry(&app), Some(CarryKind::Place));
    settle(
        &mut app,
        Message::ContextMenu(ContextMenuMsg::Show(5.0, 5.0)),
    );
    assert_eq!(carry(&app), None, "a right-click ends it");
    assert_eq!(text(&app), before);
}

#[test]
fn the_footprint_picked_in_the_panel_is_the_one_placed() {
    let Fixture { mut app, _tmp } = open();
    let key = key_of(&app, "STOCK-R");
    select(&mut app, &key);
    let (alt, alt_name) = alternates(&app).remove(0);
    settle(
        &mut app,
        Message::Library(LibraryMessage::ComponentsPanelFootprintPicked(alt)),
    );
    place(&mut app, &key);
    click(&mut app, 0.0, 10.0);
    let fp = placed(&app).pop().expect("placed");
    assert_eq!(fp.footprint_id, alt_name);
    assert_eq!(fp.reference, "R1");
}

#[test]
fn a_part_the_board_cannot_hold_is_refused_with_its_reason() {
    let Fixture { mut app, _tmp } = open();
    let before = text(&app);
    let board = app.document_state.tabs[app.document_state.active_tab]
        .path
        .clone();
    let since = diagnostics::newest_entry_id().unwrap_or(0);
    let arm_epoch = app.interaction_state.placement_epoch;
    settle(
        &mut app,
        Message::Library(LibraryMessage::BoardPartResolved {
            board,
            window: None,
            arm_epoch,
            result: Err(BoardPartError::Refused {
                part: "STOCK-X".into(),
                reason: "the part binds no footprint".into(),
            }),
        }),
    );
    assert_eq!(carry(&app), None, "nothing goes on the cursor");
    let said = diagnostics::recent_entries().iter().any(|e| {
        e.id > since
            && e.message
                .replace(['\u{2068}', '\u{2069}'], "")
                .contains("STOCK-X cannot be placed on a board: the part binds no footprint")
    });
    assert!(said, "Messages says why");
    click(&mut app, 0.0, 0.0);
    assert_eq!(text(&app), before, "no bare tag or footprint was placed");
}
