//! #2240 — board placement through the app: Edit ▸ Move click to click,
//! enablement that follows the selection and its lock, a parked board that
//! no stale selection reaches, an undocked window that edits its own board,
//! and a right-click that ends a Move.

use std::path::{Path, PathBuf};

use pcb_geom::ClickMode;
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use crate::app::command::enablement::is_enabled;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::WindowKind;
use crate::app::{ContextMenuMsg, EditMsg, Message, PcbMsg, Signex, UiMsg};
use crate::keymap::{AppCommandId, KeyStroke, KeyToken, Modifiers};
use crate::menu_bar::MenuMessage;
use crate::pcb_canvas::PcbCanvasMsg;

const U1: Uuid = Uuid::from_u128(0x10);
const U2: Uuid = Uuid::from_u128(0x20);

/// U1 at (10, 10) with two pads; U2, locked, at (30, 10).
fn board() -> PcbBoard {
    let footprint = |id: Uuid, x: f64, locked: bool, pad: u128| {
        serde_json::json!({
            "uuid": id, "reference": if locked { "U2" } else { "U1" },
            "position": {"x": x, "y": 10.0}, "rotation": 0.0, "layer": "TopCopper",
            "locked": locked,
            "pads": [
                {"uuid": Uuid::from_u128(pad), "number": "1", "pad_type": "smd",
                 "shape": "rect", "position": {"x": 2.0, "y": 0.0},
                 "size": {"x": 1.0, "y": 1.0}, "drill": null, "layers": ["TopCopper"]},
                {"uuid": Uuid::from_u128(pad + 1), "number": "2", "pad_type": "smd",
                 "shape": "rect", "position": {"x": -2.0, "y": 0.0},
                 "size": {"x": 1.0, "y": 1.0}, "drill": null, "layers": ["TopCopper"]}
            ]
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "footprints": [footprint(U1, 10.0, false, 0x11), footprint(U2, 30.0, true, 0x21)]
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

fn position(app: &Signex, path: &Path, id: Uuid) -> Point {
    let board = app.document_state.pcb_board(path).expect("board");
    board
        .footprints
        .iter()
        .find(|f| f.uuid == id)
        .unwrap()
        .position
}

fn click_message(x: f64, y: f64) -> PcbCanvasMsg {
    PcbCanvasMsg::Clicked {
        at: Point::new(x, y),
        tolerance_mm: 0.05,
        mode: ClickMode::Replace,
    }
}

fn click(app: &mut Signex, x: f64, y: f64) {
    let _ = app.update(Message::PcbCanvas(click_message(x, y)));
}

fn command(id: &str) -> AppCommandId {
    AppCommandId::new(id).expect("a catalog id")
}

fn enabled(app: &Signex, id: &str) -> bool {
    is_enabled(&command(id), &app.command_context(None))
}

fn rotate_key(window: Option<iced::window::Id>) -> Message {
    Message::Ui(UiMsg::KeymapStroke {
        window,
        stroke: KeyStroke {
            modifiers: Modifiers {
                shift: true,
                ..Modifiers::default()
            },
            key: KeyToken::Space,
        },
    })
}

fn altium(app: &mut Signex) {
    app.ui_state
        .keymap_profiles
        .set_active_profile("altium")
        .expect("altium profile");
    app.ui_state.active_keymap = app.ui_state.keymap_profiles.compile_active().into();
}

#[test]
fn edit_move_picks_a_footprint_then_drops_it_on_the_grid_as_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    let _ = app.update(Message::Menu(MenuMessage::Command(command("move_object"))));
    assert!(app.interaction_state.pcb_carry.is_some(), "Move is armed");
    // The first click picks U1 by its body and anchors there.
    click(&mut app, 10.0, 10.0);
    assert_eq!(live(&app, &path), before);
    let over = PcbCanvasMsg::PointerMoved {
        at: Point::new(14.0, 12.4),
        zoom_pct: 100.0,
        tolerance_mm: 0.05,
    };
    let _ = app.update(Message::PcbCanvas(over));
    let ghost = app.interaction_state.pcb_canvas.highlight().ghost.len();
    assert_eq!(ghost, 3, "the body and both pads follow the pointer");
    click(&mut app, 14.0, 12.4);
    assert!(app.interaction_state.pcb_carry.is_none());
    let moved = position(&app, &path, U1);
    // (14, 12.4) on the 1.27 mm grid is (13.97, 12.7).
    assert!(
        (moved.x - 13.97).abs() < 1e-9 && (moved.y - 12.7).abs() < 1e-9,
        "{moved:?}"
    );
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(live(&app, &path), before, "one undo puts it back");
}

#[test]
fn enablement_follows_the_board_selection_and_its_lock() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with_board(dir.path());
    let ids = [
        "rotate_clockwise",
        "mirror_x",
        "flip_selection",
        "move_selection",
        "delete_selection",
    ];
    assert!(ids.iter().all(|id| !enabled(&app, id)), "nothing selected");
    click(&mut app, 10.0, 10.0);
    assert!(ids.iter().all(|id| enabled(&app, id)), "U1 selected");
    click(&mut app, 30.0, 10.0);
    assert!(ids.iter().all(|id| !enabled(&app, id)), "U2 is locked");
    // Move object picks what the click lands on, so it stays enabled.
    assert!(enabled(&app, "move_object"));
}

#[test]
fn a_sheet_keeps_rotate_enabled_with_nothing_selected() {
    let dir = tempfile::tempdir().unwrap();
    let app = app_workspace(&dir.path().to_string_lossy(), &[]);
    assert!(enabled(&app, "rotate_clockwise"));
    assert!(
        !enabled(&app, "flip_selection"),
        "Flip Selection is for a board"
    );
}

#[test]
fn a_parked_board_is_not_reached_by_its_old_selection() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    altium(&mut app);
    click(&mut app, 10.0, 10.0);
    let _ = app.update(rotate_key(None));
    let edited = live(&app, &path);
    let _ = app.close_tab_parking_dirty(0);
    assert!(app.document_state.parked_pcb_boards.contains_key(&path));
    assert!(!app.command_context(None).pcb.shown);

    let _ = app.update(rotate_key(None));
    let _ = app.update(Message::Edit(EditMsg::DeleteSelected));
    let _ = app.update(Message::Edit(EditMsg::FlipSelection));
    assert_eq!(live(&app, &path), edited, "the parked board is untouched");
    let refused = app.execute_board_commands(
        &path,
        vec![signex_engine::pcb::BoardCommand::Flip { footprint: U1 }],
        None,
    );
    assert_eq!(
        refused,
        Err(crate::app::state::pcb_edit::BoardEditError::Parked(
            path.clone()
        ))
    );
}

#[test]
fn an_undocked_window_selects_and_edits_its_own_board() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, a) = app_with_board(dir.path());
    altium(&mut app);
    let b = dir.path().join("other.snxpcb");
    app.open_pcb_tab(b.clone(), "other".into(), board());
    let active = app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == b)
        .unwrap();
    app.document_state.active_tab = active;
    let main = iced::window::Id::unique();
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    app.ui_state.windows.insert(
        window,
        WindowKind::UndockedTab {
            path: a.clone(),
            title: "board".into(),
        },
    );
    app.interaction_state
        .canvases
        .insert(window, crate::canvas::CanvasSlot::new());
    let canvas = app.new_pcb_canvas_for(&a).expect("a PCB canvas");
    assert!(canvas.selectable, "an undocked board takes clicks");
    app.interaction_state.pcb_canvases.insert(window, canvas);

    let clicked = PcbMsg::CanvasInWindow {
        window,
        msg: click_message(10.0, 10.0),
    };
    let _ = app.update(Message::Pcb(clicked));
    assert!(
        app.interaction_state.pcb_selection.selection().is_empty(),
        "the main window's selection is its own"
    );
    assert!(
        app.interaction_state.pcb_selections[&window]
            .selection()
            .footprints()
            .contains(&U1)
    );
    assert!(app.command_context(Some(window)).pcb.placeable);

    let (before_a, before_b) = (live(&app, &a), live(&app, &b));
    let _ = app.update(rotate_key(Some(window)));
    assert_ne!(live(&app, &a), before_a, "the window's board turned");
    assert_eq!(live(&app, &b), before_b, "the main board did not");
    assert_eq!(app.document_state.active_tab, active, "the main tab stays");
}

#[test]
fn a_right_click_ends_a_move_instead_of_opening_the_menu() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    click(&mut app, 10.0, 10.0);
    let _ = app.update(Message::Menu(MenuMessage::Command(command(
        "move_selection",
    ))));
    assert!(app.interaction_state.pcb_carry.is_some());
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::Show(5.0, 5.0)));
    assert!(app.interaction_state.pcb_carry.is_none());
    assert!(app.interaction_state.context_menu.is_none());
    let before = live(&app, &path);
    click(&mut app, 20.0, 20.0);
    assert_eq!(live(&app, &path), before, "the ended Move drops nothing");
    // With no Move armed, the right-click opens the board's menu.
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::Show(5.0, 5.0)));
    assert!(app.interaction_state.context_menu.is_some());
}
