//! #2240 review — a drag or Move belongs to the window and the board it
//! started on: Esc in an undocked board window ends its own, a stale or
//! foreign carry never acts, a waiting Move grabs nothing, and an edit in
//! one window repaints the selection in the others.

use std::path::{Path, PathBuf};

use pcb_geom::ClickMode;
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::WindowKind;
use crate::app::{ContextMenuMsg, InputMsg, Message, PcbMsg, Signex, UiMsg};
use crate::keymap::{AppCommandId, KeyStroke, KeyToken, Modifiers};
use crate::menu_bar::MenuMessage;
use crate::pcb_canvas::PcbCanvasMsg;

const U1: Uuid = Uuid::from_u128(0x10);

/// U1 at (10, 10) with two pads; U2, locked, at (30, 10).
fn board() -> PcbBoard {
    let footprint = |id: u128, x: f64, locked: bool| {
        serde_json::json!({
            "uuid": Uuid::from_u128(id), "reference": format!("U{}", id >> 4),
            "position": {"x": x, "y": 10.0}, "rotation": 0.0, "layer": "TopCopper",
            "locked": locked,
            "pads": [
                {"uuid": Uuid::from_u128(id + 1), "number": "1", "pad_type": "smd",
                 "shape": "rect", "position": {"x": 2.0, "y": 0.0},
                 "size": {"x": 1.0, "y": 1.0}, "drill": null, "layers": ["TopCopper"]},
                {"uuid": Uuid::from_u128(id + 2), "number": "2", "pad_type": "smd",
                 "shape": "rect", "position": {"x": -2.0, "y": 0.0},
                 "size": {"x": 1.0, "y": 1.0}, "drill": null, "layers": ["TopCopper"]}
            ]
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "footprints": [footprint(0x10, 10.0, false), footprint(0x20, 30.0, true)]
    }))
    .unwrap()
}

struct Rig {
    app: Signex,
    /// Shown by the undocked window.
    a: PathBuf,
    /// Shown by the main window unless a test switches it to `a`.
    b: PathBuf,
    window: iced::window::Id,
    _dir: tempfile::TempDir,
}

/// Board A undocked in `window`, board B active in the main window.
fn rig() -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    app.ui_state
        .keymap_profiles
        .set_active_profile("altium")
        .expect("altium profile");
    app.ui_state.active_keymap = app.ui_state.keymap_profiles.compile_active().into();
    let a = dir.path().join("a.snxpcb");
    let b = dir.path().join("b.snxpcb");
    app.open_pcb_tab(a.clone(), "a".into(), board());
    app.open_pcb_tab(b.clone(), "b".into(), board());
    show_in_main(&mut app, &b);
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    app.ui_state.windows.insert(
        window,
        WindowKind::UndockedTab {
            path: a.clone(),
            title: "a".into(),
        },
    );
    app.interaction_state
        .canvases
        .insert(window, crate::canvas::CanvasSlot::new());
    let canvas = app.new_pcb_canvas_for(&a).expect("a PCB canvas");
    app.interaction_state.pcb_canvases.insert(window, canvas);
    Rig {
        app,
        a,
        b,
        window,
        _dir: dir,
    }
}

fn show_in_main(app: &mut Signex, path: &Path) {
    let index = app.document_state.tabs.iter().position(|t| t.path == path);
    app.document_state.active_tab = index.expect("an open tab");
}

fn live(app: &Signex, path: &Path) -> String {
    serde_json::to_string(app.document_state.pcb_board(path).expect("board")).unwrap()
}

fn click(x: f64, y: f64) -> PcbCanvasMsg {
    PcbCanvasMsg::Clicked {
        at: Point::new(x, y),
        tolerance_mm: 0.05,
        mode: ClickMode::Replace,
    }
}

fn in_window(app: &mut Signex, window: iced::window::Id, msg: PcbCanvasMsg) {
    let _ = app.update(Message::Pcb(PcbMsg::CanvasInWindow { window, msg }));
}

fn in_main(app: &mut Signex, msg: PcbCanvasMsg) {
    let _ = app.update(Message::PcbCanvas(msg));
}

fn escape(app: &mut Signex, window: Option<iced::window::Id>) {
    let _ = app.update(Message::Input(InputMsg::EscapePressed { window }));
}

fn menu_in_window(app: &mut Signex, window: iced::window::Id, id: &str) {
    let msg = MenuMessage::Command(AppCommandId::new(id).expect("a catalog id"));
    let _ = app.update(Message::Ui(UiMsg::MenuInWindow { window, msg }));
}

fn window_selection_is_empty(rig: &Rig) -> bool {
    rig.app.interaction_state.pcb_selections[&rig.window]
        .selection()
        .is_empty()
}

#[test]
fn escape_in_a_board_window_ends_its_drag_so_the_release_moves_nothing() {
    let mut rig = rig();
    in_window(&mut rig.app, rig.window, click(10.0, 10.0));
    let (from, to) = (Point::new(10.0, 10.0), Point::new(15.0, 10.0));
    in_window(
        &mut rig.app,
        rig.window,
        PcbCanvasMsg::FootprintDragged { from, to },
    );
    assert!(rig.app.interaction_state.pcb_carry.is_some());
    let before = live(&rig.app, &rig.a);

    escape(&mut rig.app, Some(rig.window));
    assert!(rig.app.interaction_state.pcb_carry.is_none());
    assert!(!rig.app.interaction_state.pcb_canvases[&rig.window].is_carrying());
    assert!(window_selection_is_empty(&rig));
    in_window(
        &mut rig.app,
        rig.window,
        PcbCanvasMsg::FootprintDropped { from, to },
    );
    assert_eq!(live(&rig.app, &rig.a), before, "U1 stays where it was");
}

#[test]
fn escape_in_a_board_window_ends_its_armed_move() {
    let mut rig = rig();
    in_window(&mut rig.app, rig.window, click(10.0, 10.0));
    menu_in_window(&mut rig.app, rig.window, "move_selection");
    assert_eq!(
        rig.app
            .interaction_state
            .pcb_carry
            .as_ref()
            .map(|c| c.window),
        Some(Some(rig.window)),
        "the Move belongs to the window"
    );
    let before = live(&rig.app, &rig.a);
    escape(&mut rig.app, Some(rig.window));
    assert!(rig.app.interaction_state.pcb_carry.is_none());
    in_window(&mut rig.app, rig.window, click(10.0, 10.0));
    in_window(&mut rig.app, rig.window, click(14.0, 12.0));
    assert_eq!(
        live(&rig.app, &rig.a),
        before,
        "the ended Move drops nothing"
    );
}

#[test]
fn escape_in_a_board_window_leaves_the_main_boards_selection() {
    let mut rig = rig();
    in_main(&mut rig.app, click(10.0, 10.0));
    in_window(&mut rig.app, rig.window, click(10.0, 10.0));
    escape(&mut rig.app, Some(rig.window));
    assert!(window_selection_is_empty(&rig));
    let main = rig.app.interaction_state.pcb_selection.selection();
    assert!(
        main.footprints().contains(&U1),
        "board B keeps its selection"
    );
    assert_eq!(rig.app.active_tab_path().as_deref(), Some(rig.b.as_path()));
}

#[test]
fn a_move_left_on_another_board_does_not_swallow_the_right_click() {
    let mut rig = rig();
    show_in_main(&mut rig.app, &rig.a.clone());
    in_main(&mut rig.app, click(10.0, 10.0));
    let msg = MenuMessage::Command(AppCommandId::new("move_selection").unwrap());
    let _ = rig.app.update(Message::Menu(msg));
    assert!(rig.app.interaction_state.pcb_carry.is_some());
    show_in_main(&mut rig.app, &rig.b.clone());
    let _ = rig
        .app
        .update(Message::ContextMenu(ContextMenuMsg::Show(5.0, 5.0)));
    assert!(
        rig.app.interaction_state.context_menu.is_some(),
        "the menu opens"
    );
    assert!(
        rig.app.interaction_state.pcb_carry.is_none(),
        "the stale Move went"
    );
}

#[test]
fn a_waiting_move_grabs_nothing_so_a_jiggled_click_stays_a_click() {
    let mut rig = rig();
    in_main(&mut rig.app, click(10.0, 10.0));
    let on_u1 = Point::new(10.0, 10.0);
    assert!(rig.app.interaction_state.pcb_canvas.grabs(on_u1, 0.05));
    let msg = MenuMessage::Command(AppCommandId::new("move_selection").unwrap());
    let _ = rig.app.update(Message::Menu(msg));
    assert!(
        !rig.app.interaction_state.pcb_canvas.grabs(on_u1, 0.05),
        "a press during the Move is a click, never a drag"
    );
    escape(&mut rig.app, None);
    in_main(&mut rig.app, click(10.0, 10.0));
    assert!(rig.app.interaction_state.pcb_canvas.grabs(on_u1, 0.05));
}

#[test]
fn a_move_in_the_main_window_ignores_clicks_in_another_window_on_that_board() {
    let mut rig = rig();
    show_in_main(&mut rig.app, &rig.a.clone());
    in_main(&mut rig.app, click(10.0, 10.0));
    let msg = MenuMessage::Command(AppCommandId::new("move_selection").unwrap());
    let _ = rig.app.update(Message::Menu(msg));
    let before = live(&rig.app, &rig.a);
    in_window(&mut rig.app, rig.window, click(10.0, 10.0));
    in_window(&mut rig.app, rig.window, click(14.0, 12.0));
    assert_eq!(live(&rig.app, &rig.a), before, "the window did not drop it");
    let carry = rig.app.interaction_state.pcb_carry.as_ref();
    assert_eq!(
        carry.map(|c| c.window),
        Some(None),
        "the main Move is armed"
    );
}

#[test]
fn an_edit_in_the_main_window_repaints_the_other_windows_selection() {
    let mut rig = rig();
    show_in_main(&mut rig.app, &rig.a.clone());
    in_window(&mut rig.app, rig.window, click(10.0, 10.0));
    in_main(&mut rig.app, click(10.0, 10.0));
    let painted = |rig: &Rig| {
        rig.app.interaction_state.pcb_canvases[&rig.window]
            .highlight()
            .clone()
    };
    let before = painted(&rig);
    assert!(!before.selected.is_empty(), "the window paints U1's pads");
    let key = Message::Ui(UiMsg::KeymapStroke {
        window: None,
        stroke: KeyStroke {
            modifiers: Modifiers::default(),
            key: KeyToken::Space,
        },
    });
    let _ = rig.app.update(key);
    assert_ne!(painted(&rig), before, "the window follows the turned pads");
    let grab = Point::new(10.0, 12.0);
    let window_canvas = &rig.app.interaction_state.pcb_canvases[&rig.window];
    assert!(
        window_canvas.grabs(grab, 0.05),
        "and grabs them where they are"
    );
}

#[test]
fn a_new_drag_never_reuses_a_carry_left_by_another_press() {
    let mut rig = rig();
    show_in_main(&mut rig.app, &rig.a.clone());
    in_main(&mut rig.app, click(10.0, 10.0));
    let stale = PcbCanvasMsg::FootprintDragged {
        from: Point::new(10.0, 10.0),
        to: Point::new(12.0, 10.0),
    };
    in_main(&mut rig.app, stale);
    // The release never arrived; U1 is no longer selected.
    in_main(&mut rig.app, click(20.0, 25.0));
    let before = live(&rig.app, &rig.a);
    let (from, to) = (Point::new(20.0, 25.0), Point::new(25.0, 25.0));
    in_main(&mut rig.app, PcbCanvasMsg::FootprintDragged { from, to });
    in_main(&mut rig.app, PcbCanvasMsg::FootprintDropped { from, to });
    assert_eq!(live(&rig.app, &rig.a), before, "U1 is not carried along");
}
