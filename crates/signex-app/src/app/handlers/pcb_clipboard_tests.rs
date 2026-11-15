//! #2361: Select All, Copy, Cut, Paste, Paste Special and Duplicate on a
//! board, driven through `Signex::update` as the menu, keys and context
//! menu drive them.

use std::path::{Path, PathBuf};

use pcb_geom::{ClickMode, ItemRef};
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use crate::app::command::enablement::is_enabled;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;
use crate::pcb_canvas::{CarryKind, PcbCanvasMsg};

const U1: Uuid = Uuid::from_u128(0x10);
const U2: Uuid = Uuid::from_u128(0x20);
const TRACK: Uuid = Uuid::from_u128(0x30);
const VIA: Uuid = Uuid::from_u128(0x40);

/// U1 at (10, 10) with two pads, U2 locked at (30, 10), a GND track and a
/// via.
fn board() -> PcbBoard {
    let footprint = |id: Uuid, x: f64, locked: bool, pad: u128| {
        serde_json::json!({
            "uuid": id, "reference": if locked { "U2" } else { "U1" },
            "position": {"x": x, "y": 10.0}, "rotation": 0.0, "layer": "TopCopper",
            "locked": locked,
            "pads": [
                {"uuid": Uuid::from_u128(pad), "number": "1", "pad_type": "smd",
                 "shape": "rect", "position": {"x": 2.0, "y": 0.0},
                 "size": {"x": 1.0, "y": 1.0}, "drill": null, "layers": ["TopCopper"],
                 "net": {"number": 1, "name": "GND"}},
                {"uuid": Uuid::from_u128(pad + 1), "number": "2", "pad_type": "smd",
                 "shape": "rect", "position": {"x": -2.0, "y": 0.0},
                 "size": {"x": 1.0, "y": 1.0}, "drill": null, "layers": ["TopCopper"]}
            ]
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "nets": [{"number": 1, "name": "GND"}],
        "footprints": [footprint(U1, 10.0, false, 0x11), footprint(U2, 30.0, true, 0x21)],
        "segments": [{"uuid": TRACK, "start": {"x": 40.0, "y": 0.0},
                      "end": {"x": 50.0, "y": 0.0}, "width": 0.25,
                      "layer": "TopCopper", "net": 1}],
        "vias": [{"uuid": VIA, "position": {"x": 60.0, "y": 0.0}, "diameter": 0.6, "net": 1}]
    }))
    .unwrap()
}

fn app_with_board(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board());
    (app, path)
}

fn live(app: &Signex, path: &Path) -> String {
    serde_json::to_string(app.document_state.pcb_board(path).expect("board")).unwrap()
}

fn live_board<'a>(app: &'a Signex, path: &Path) -> &'a PcbBoard {
    app.document_state.pcb_board(path).expect("board")
}

fn command(id: &str) -> AppCommandId {
    AppCommandId::new(id).expect("a catalog id")
}

fn run(app: &mut Signex, id: &str) {
    let _ = app.update(Message::Menu(MenuMessage::Command(command(id))));
}

fn enabled(app: &Signex, id: &str) -> bool {
    is_enabled(&command(id), &app.command_context(None))
}

fn click(app: &mut Signex, x: f64, y: f64) {
    let _ = app.update(Message::PcbCanvas(PcbCanvasMsg::Clicked {
        at: Point::new(x, y),
        tolerance_mm: 0.05,
        mode: ClickMode::Replace,
    }));
}

fn point_at(app: &mut Signex, x: f64, y: f64) {
    let _ = app.update(Message::PcbCanvas(PcbCanvasMsg::PointerMoved {
        at: Point::new(x, y),
        zoom_pct: 100.0,
        tolerance_mm: 0.05,
    }));
}

fn on_grid(v: f64) -> f64 {
    (v / signex_types::pcb::PCB_GRID_MM).round() * signex_types::pcb::PCB_GRID_MM
}

#[test]
fn select_all_selects_every_component_track_and_via() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_board(dir.path());
    run(&mut app, "select_all");
    let selection = app.interaction_state.pcb_selection.selection();
    assert_eq!(selection.footprints(), [U1, U2].into());
    assert!(selection.contains(ItemRef::Track(TRACK)));
    assert!(selection.contains(ItemRef::Via(VIA)));
}

#[test]
fn the_clipboard_rows_are_disabled_with_an_empty_selection() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    for id in ["cut", "copy", "duplicate", "paste", "smart_paste"] {
        assert!(!enabled(&app, id), "{id} is live with nothing selected");
    }
    assert!(enabled(&app, "select_all"));
    // The keys do nothing either.
    for id in ["cut", "copy", "duplicate", "paste", "smart_paste"] {
        run(&mut app, id);
    }
    assert_eq!(live(&app, &path), before);
    assert!(app.interaction_state.pcb_clipboard.is_none());
    assert!(
        app.interaction_state.pcb_carry.is_none(),
        "nothing to paste"
    );

    click(&mut app, 10.0, 10.0);
    for id in ["cut", "copy", "duplicate"] {
        assert!(enabled(&app, id), "{id} is greyed with U1 selected");
    }
    assert!(!enabled(&app, "paste"), "nothing copied yet");
    run(&mut app, "copy");
    assert!(enabled(&app, "paste") && enabled(&app, "smart_paste"));
}

#[test]
fn copy_then_paste_places_the_copy_at_the_cursor_in_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    click(&mut app, 10.0, 10.0);
    run(&mut app, "copy");
    assert_eq!(live(&app, &path), before, "a copy leaves the board alone");
    let origin = app
        .interaction_state
        .pcb_clipboard
        .as_ref()
        .expect("a board copy")
        .origin();

    run(&mut app, "paste");
    let carry = app
        .interaction_state
        .pcb_carry
        .as_ref()
        .expect("on the cursor");
    assert_eq!(carry.kind, CarryKind::Paste);
    point_at(&mut app, 40.3, 20.1);
    let ghost = app.interaction_state.pcb_canvas.highlight().ghost.len();
    assert_eq!(ghost, 3, "the body and both pads follow the pointer");
    assert_eq!(live(&app, &path), before, "nothing lands before the click");

    click(&mut app, 40.3, 20.1);
    assert!(app.interaction_state.pcb_carry.is_none());
    let board = live_board(&app, &path);
    assert_eq!(board.footprints.len(), 3);
    let pasted = &board.footprints[2];
    assert_eq!(
        pasted.reference, "U3",
        "U1 is taken, so the copy is renumbered"
    );
    let expected = Point::new(
        on_grid(40.3) + (10.0 - origin.x),
        on_grid(20.1) + (10.0 - origin.y),
    );
    assert!(
        (pasted.position.x - expected.x).abs() < 1e-9
            && (pasted.position.y - expected.y).abs() < 1e-9,
        "{:?} is not {expected:?}",
        pasted.position
    );
    let net = pasted.pads[0].net.as_ref().expect("GND is on this board");
    assert_eq!(net.name, "GND");
    let selection = app.interaction_state.pcb_selection.selection();
    assert_eq!(
        selection.footprints(),
        [pasted.uuid].into(),
        "the copy is selected"
    );

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(live(&app, &path), before, "one undo takes the paste back");
}

#[test]
fn cut_then_undo_restores_the_board() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    click(&mut app, 10.0, 10.0);
    run(&mut app, "cut");
    let board = live_board(&app, &path);
    assert!(board.footprints.iter().all(|fp| fp.uuid != U1), "U1 is cut");
    assert!(app.interaction_state.pcb_clipboard.is_some());
    assert!(app.interaction_state.pcb_selection.selection().is_empty());

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(live(&app, &path), before, "one undo puts U1 back");
}

#[test]
fn a_locked_footprint_is_neither_cut_nor_copied_by_cut() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    click(&mut app, 30.0, 10.0);
    assert!(!enabled(&app, "cut"));
    assert!(enabled(&app, "copy"), "a locked footprint can be copied");
    run(&mut app, "cut");
    assert_eq!(live(&app, &path), before);
    assert!(app.interaction_state.pcb_clipboard.is_none());
}

#[test]
fn duplicate_and_paste_special_place_a_copy_at_once_in_one_undo_step() {
    for id in ["duplicate", "smart_paste"] {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, path) = app_with_board(dir.path());
        let before = live(&app, &path);
        click(&mut app, 10.0, 10.0);
        if id == "smart_paste" {
            run(&mut app, "copy");
        }
        run(&mut app, id);
        assert!(app.interaction_state.pcb_carry.is_none(), "{id}");
        let board = live_board(&app, &path);
        assert_eq!(board.footprints.len(), 3, "{id} placed no copy");
        assert_eq!(board.footprints[2].reference, "U3", "{id}");
        assert_ne!(board.footprints[2].position, board.footprints[0].position);
        let _ = app.update(Message::Edit(EditMsg::Undo));
        assert_eq!(live(&app, &path), before, "{id}: one undo takes it back");
    }
}

#[test]
fn a_board_copy_replaces_the_sheet_copy_and_esc_drops_a_paste() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    app.interaction_state.clipboard_text = Some("a sheet fragment".into());
    click(&mut app, 10.0, 10.0);
    run(&mut app, "copy");
    assert!(app.interaction_state.clipboard.is_none());
    assert!(app.interaction_state.clipboard_text.is_none());

    run(&mut app, "paste");
    assert!(app.interaction_state.pcb_carry.is_some());
    // The first-run tour follows the shared test prefs; open, it takes the Esc.
    app.ui_state.first_run_tour_open = false;
    let _ = app.update(Message::Input(crate::app::InputMsg::EscapePressed {
        window: None,
    }));
    assert!(
        app.interaction_state.pcb_carry.is_none(),
        "Esc drops the paste"
    );
    assert_eq!(live(&app, &path), before);
}
