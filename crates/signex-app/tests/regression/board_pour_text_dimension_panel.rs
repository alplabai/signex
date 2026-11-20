//! #2523: the Copper card of a polygon pour (Shelve, Repour, the board-wide
//! menu), a string's quick rotation and a dimension's precision steps,
//! driven through `Signex::update` with the messages the panels send. A
//! shelved pour keeps its outline and settings, holds no copper, and is
//! saved with the board.

use pcb_geom::{ClickMode, ItemRef};
use signex_app::app::{EditMsg, FileMsg, Message, Signex, TabDocument};
use signex_app::diagnostics;
use signex_app::dock::DockMessage;
use signex_app::panels::pcb_primitive_properties::{
    PcbPrimitiveFacts, PourMsg, PrimitiveField, PrimitiveTarget, ZoneFacts,
};
use signex_app::panels::{PanelMsg, PcbPrimitiveMsg};
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_types::format::SnxPcb;
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

const POUR_A: Uuid = Uuid::from_u128(0x501);
const POUR_B: Uuid = Uuid::from_u128(0x502);
const TEXT: Uuid = Uuid::from_u128(0x503);
const DIM: Uuid = Uuid::from_u128(0x504);

fn square(x: f64, y: f64, side: f64) -> serde_json::Value {
    serde_json::json!([
        {"x": x, "y": y}, {"x": x + side, "y": y},
        {"x": x + side, "y": y + side}, {"x": x, "y": y + side}
    ])
}

/// Two GND pours on the top copper, 10 mm squares at (50, 50) and (70, 50);
/// a string at (5, 30) and a 10 mm dimension along y = 10, far from them.
fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 1.0, "trace_width": 0.3},
        "nets": [{"number": 1, "name": "GND"}],
        "zones": [
            {"uuid": POUR_A, "net": 1, "net_name": "GND", "layer": "TopCopper",
             "fill_type": "pour", "outline": square(50.0, 50.0, 10.0)},
            {"uuid": POUR_B, "net": 1, "net_name": "GND", "layer": "TopCopper",
             "fill_type": "pour", "outline": square(70.0, 50.0, 10.0)}
        ],
        "texts": [{"uuid": TEXT, "text": "A", "position": {"x": 5.0, "y": 30.0},
                   "layer": "TopSilkscreen", "font_size": 1.0, "rotation": 0.0}],
        "free": {
            "dimensions": [{"uuid": DIM, "start": {"x": 30.0, "y": 10.0},
                            "end": {"x": 40.0, "y": 10.0}, "offset": 2.0,
                            "layer": "Dimensions", "line_width": 0.1,
                            "text_height": 1.0, "precision": 2}]
        }
    }))
    .unwrap()
}

struct Fixture {
    app: Signex,
    path: std::path::PathBuf,
    _tmp: tempfile::TempDir,
}

fn open_board() -> Fixture {
    let _ = diagnostics::init_logging();
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("board.snxpcb");
    std::fs::write(&path, SnxPcb::new(board()).write_string().unwrap()).unwrap();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let task = app.update(Message::File(FileMsg::Opened(Some(path.clone()))));
    crate::drive::drive(&mut app, task);
    Fixture {
        app,
        path,
        _tmp: tmp,
    }
}

fn send(f: &mut Fixture, message: Message) {
    let task = f.app.update(message);
    crate::drive::drive(&mut f.app, task);
}

fn panel(msg: PcbPrimitiveMsg) -> Message {
    Message::Dock(DockMessage::Panel(PanelMsg::PcbPrimitive(msg)))
}

fn pour(msg: PourMsg) -> Message {
    panel(PcbPrimitiveMsg::Pour(msg))
}

fn live(f: &Fixture) -> PcbBoard {
    let docs = &f.app.document_state;
    docs.tabs[docs.active_tab]
        .cached_document
        .as_ref()
        .and_then(TabDocument::as_pcb)
        .expect("a board is open")
        .clone()
}

fn snapshot(f: &Fixture) -> String {
    serde_json::to_string(&live(f)).unwrap()
}

fn on_disk(f: &Fixture) -> PcbBoard {
    SnxPcb::parse(&std::fs::read_to_string(&f.path).unwrap())
        .unwrap()
        .board
}

fn click(f: &mut Fixture, at: Point) {
    send(
        f,
        Message::PcbCanvas(PcbCanvasMsg::Clicked {
            at,
            tolerance_mm: 0.05,
            mode: ClickMode::Replace,
        }),
    );
}

fn facts(f: &Fixture) -> PcbPrimitiveFacts {
    f.app
        .document_state
        .panel_ctx
        .pcb_board
        .as_ref()
        .and_then(|b| b.primitive.clone())
        .expect("the primitive panel")
}

fn area_panel(f: &Fixture) -> ZoneFacts {
    match facts(f) {
        PcbPrimitiveFacts::Zone(z) => z,
        other => panic!("the area panel, not {other:?}"),
    }
}

/// Picks pour A with a click inside it.
fn picked_pour(f: &mut Fixture) -> PrimitiveTarget {
    click(f, Point::new(55.0, 55.0));
    let target = area_panel(f).common.target;
    assert!(
        matches!(target.items.as_slice(), [ItemRef::Zone { zone, .. }] if *zone == POUR_A),
        "{target:?}"
    );
    target
}

/// The edit `run` makes is one undo step: one undo restores the board,
/// redo replays it, and a second undo leaves nothing of it: it lands where
/// one undo from the starting board lands (the starting board itself when
/// there is no earlier step).
fn assert_one_step(f: &mut Fixture, run: impl FnOnce(&mut Fixture)) -> PcbBoard {
    let before = snapshot(f);
    send(f, Message::Edit(EditMsg::Undo));
    let earlier = snapshot(f);
    send(f, Message::Edit(EditMsg::Redo));
    assert_eq!(snapshot(f), before, "redo returns to the starting board");
    run(f);
    let after = snapshot(f);
    assert_ne!(before, after, "the board did not change");
    let edited = live(f);
    send(f, Message::Edit(EditMsg::Undo));
    assert_eq!(snapshot(f), before, "one undo restores the board");
    send(f, Message::Edit(EditMsg::Redo));
    assert_eq!(snapshot(f), after, "redo replays it");
    send(f, Message::Edit(EditMsg::Undo));
    send(f, Message::Edit(EditMsg::Undo));
    assert_eq!(snapshot(f), earlier, "it was a single step");
    edited
}

fn shelve(target: &PrimitiveTarget, shelved: bool) -> Message {
    pour(PourMsg::ShelveToggled {
        target: target.clone(),
        shelved,
    })
}

#[test]
fn shelve_is_one_step_saved_with_the_board_and_unshelve_brings_the_fill_back() {
    let mut f = open_board();
    let target = picked_pour(&mut f);
    let before = area_panel(&f);
    assert_eq!(before.shelved, Some(false));
    let (area, pieces) = before.copper.expect("the fill is computed");
    assert_eq!(pieces, 1);
    assert!((area - 100.0).abs() < 1e-6, "{area}");

    let edited = assert_one_step(&mut f, |f| send(f, shelve(&target, true)));
    assert!(edited.zones[0].shelved && !edited.zones[1].shelved);

    send(&mut f, shelve(&target, true));
    let shelved = area_panel(&f);
    assert_eq!(shelved.shelved, Some(true));
    assert_eq!(shelved.copper, None, "a shelved pour holds no copper");
    let board = live(&f);
    assert!(pcb_geom::zone_fills(&board).of(0).is_none());
    assert_eq!(
        board.zones[0].outline,
        board_outline(),
        "the outline and settings stay"
    );

    send(&mut f, Message::File(FileMsg::Save));
    let saved = on_disk(&f);
    assert!(saved.zones[0].shelved, "saved with the board");
    assert!(!saved.zones[1].shelved);

    send(&mut f, shelve(&target, false));
    assert!(!live(&f).zones[0].shelved);
    let (_, pieces) = area_panel(&f).copper.expect("filled again");
    assert_eq!(pieces, 1, "Unshelve brings the copper back");
}

fn board_outline() -> Vec<Point> {
    board().zones[0].outline.clone()
}

#[test]
fn repour_of_a_shelved_pour_changes_nothing() {
    let mut f = open_board();
    let target = picked_pour(&mut f);
    send(&mut f, shelve(&target, true));
    let before = snapshot(&f);
    send(&mut f, pour(PourMsg::RepourPressed));
    assert_eq!(snapshot(&f), before, "a repour writes nothing");
    assert_eq!(area_panel(&f).copper, None, "and fills no shelved pour");
}

#[test]
fn shelve_all_and_unshelve_all_are_one_step_each_and_close_the_menu() {
    let mut f = open_board();
    picked_pour(&mut f);
    send(&mut f, pour(PourMsg::MenuToggled));
    assert!(area_panel(&f).menu_open);

    let board = f.path.clone();
    let shelve_all = PourMsg::ShelveAllPressed {
        board: board.clone(),
    };
    let edited = assert_one_step(&mut f, |f| send(f, pour(shelve_all.clone())));
    assert!(edited.zones.iter().all(|z| z.shelved), "every pour");
    assert!(!area_panel(&f).menu_open, "a pick closes the menu");

    send(&mut f, pour(shelve_all));
    let unshelve_all = PourMsg::UnshelveAllPressed { board };
    let edited = assert_one_step(&mut f, |f| send(f, pour(unshelve_all)));
    assert!(edited.zones.iter().all(|z| !z.shelved));
}

/// The board of the open tab at `path`, active or not.
fn board_at(f: &Fixture, path: &std::path::Path) -> PcbBoard {
    f.app
        .document_state
        .tabs
        .iter()
        .find(|tab| tab.path == path)
        .and_then(|tab| tab.cached_document.as_ref())
        .and_then(TabDocument::as_pcb)
        .expect("the board is open")
        .clone()
}

#[test]
fn the_pour_menu_acts_on_the_board_the_panel_shows_not_the_tab_in_view() {
    let mut f = open_board();
    picked_pour(&mut f);
    send(&mut f, pour(PourMsg::MenuToggled));
    let shown = f.path.clone();
    let other = f._tmp.path().join("other.snxpcb");
    let mut second = board();
    second.uuid = Uuid::from_u128(2);
    std::fs::write(&other, SnxPcb::new(second).write_string().unwrap()).unwrap();
    send(&mut f, Message::File(FileMsg::Opened(Some(other.clone()))));
    let docs = &f.app.document_state;
    assert_eq!(
        docs.tabs[docs.active_tab].path, other,
        "the other board is the tab in view"
    );

    let before = serde_json::to_string(&board_at(&f, &other)).unwrap();
    let repour = PourMsg::RepourAllPressed {
        board: shown.clone(),
    };
    send(&mut f, pour(repour));
    send(
        &mut f,
        pour(PourMsg::ShelveAllPressed {
            board: shown.clone(),
        }),
    );
    assert!(
        board_at(&f, &shown).zones.iter().all(|z| z.shelved),
        "the panel's board is shelved"
    );
    let after = serde_json::to_string(&board_at(&f, &other)).unwrap();
    assert_eq!(after, before, "the tab in view is left as it was");

    send(
        &mut f,
        pour(PourMsg::UnshelveAllPressed {
            board: shown.clone(),
        }),
    );
    assert!(board_at(&f, &shown).zones.iter().all(|z| !z.shelved));
    assert_eq!(
        serde_json::to_string(&board_at(&f, &other)).unwrap(),
        before
    );
}

#[test]
fn the_pour_menu_stays_with_the_pour_it_was_opened_on() {
    let mut f = open_board();
    picked_pour(&mut f);
    send(&mut f, pour(PourMsg::MenuToggled));
    assert!(area_panel(&f).menu_open);

    click(&mut f, Point::new(75.0, 55.0));
    let other = area_panel(&f);
    assert!(
        matches!(other.common.target.items.as_slice(), [ItemRef::Zone { zone, .. }] if *zone == POUR_B),
        "{:?}",
        other.common.target
    );
    assert!(!other.menu_open, "pour B opens with its menu closed");
    picked_pour(&mut f);
    assert!(!area_panel(&f).menu_open, "and pour A's menu stays closed");
}

#[test]
fn a_quick_rotation_step_is_one_undo_step() {
    let mut f = open_board();
    click(&mut f, Point::new(5.0, 30.0));
    let PcbPrimitiveFacts::Text(text) = facts(&f) else {
        panic!("the string panel shows");
    };
    let target = text.common.target.clone();
    assert_eq!(target.items, vec![ItemRef::Text(TEXT)]);
    let edited = assert_one_step(&mut f, |f| {
        send(
            f,
            panel(PcbPrimitiveMsg::ValuePicked {
                target: target.clone(),
                field: PrimitiveField::Rotation,
                value: 90.0,
            }),
        );
    });
    assert_eq!(edited.texts[0].rotation, 90.0);
}

#[test]
fn a_precision_step_is_one_undo_step() {
    let mut f = open_board();
    let drawing = board().free.dimensions[0].drawing().expect("a drawing");
    let (a, b) = drawing.strokes[0];
    click(&mut f, Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0));
    let PcbPrimitiveFacts::Dimension(dimension) = facts(&f) else {
        panic!("the dimension panel shows");
    };
    let target = dimension.common.target.clone();
    assert_eq!(target.items, vec![ItemRef::Dimension(DIM)]);
    let edited = assert_one_step(&mut f, |f| {
        send(
            f,
            panel(PcbPrimitiveMsg::ValuePicked {
                target: target.clone(),
                field: PrimitiveField::Precision,
                value: 3.0,
            }),
        );
    });
    assert_eq!(edited.free.dimensions[0].value_text(), "10.000mm");
}
