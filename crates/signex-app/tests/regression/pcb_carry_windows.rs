//! #2271 review — a footprint drag ends when its window loses focus or
//! closes, a locked footprint refuses at the start of a drag, and the
//! status-bar layer pill of a board window steps that window's board
//! (#2272).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use pcb_geom::ClickMode;
use signex_app::app::{
    ContextMenuMsg, FileMsg, Message, PcbMsg, Signex, StatusBarRequest, TabDocument, WindowMsg,
};
use signex_app::diagnostics;
use signex_app::keymap::AppCommandId;
use signex_app::menu_bar::MenuMessage;
use signex_app::pcb_canvas::{CarryKind, PcbCanvasMsg};
use signex_app::tab_bar::TabMessage;
use signex_types::format::SnxPcb;
use signex_types::pcb::{Footprint, LayerDef, Pad, PadShape, PadType, PcbBoard, Point};
use signex_types::pcb_layer::CopperLayer;
use uuid::Uuid;

use crate::drive::drive;

/// The locked part's reference, unique to this file: the diagnostics ring
/// is shared by every test thread.
const LOCKED_REF: &str = "LK2271";

fn footprint(id: u128, reference: &str, x: f64, locked: bool) -> Footprint {
    let pad = |n: u128, dx: f64| Pad {
        uuid: Uuid::from_u128(id + n),
        number: n.to_string(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: Point::new(dx, 0.0),
        size: Point::new(1.0, 1.0),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: None,
        roundrect_ratio: 0.0,
        rotation: 0.0,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    Footprint {
        uuid: Uuid::from_u128(id),
        reference: reference.into(),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(x, 10.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked,
        schematic_link: None,
        library_ref: None,
        pads: vec![pad(1, 2.0), pad(2, -2.0)],
        graphics: Vec::new(),
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

/// U1 at (10, 10); the locked part at (30, 10). Two copper layers.
fn board() -> PcbBoard {
    let layer = |id: u8, name: &str| LayerDef {
        id,
        name: name.to_owned(),
        layer_type: "signal".to_owned(),
        display_name: String::new(),
    };
    PcbBoard {
        sketch: None,
        uuid: Uuid::now_v7(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: vec![layer(0, "TopCopper"), layer(31, "BottomCopper")],
        setup: None,
        nets: Vec::new(),
        footprints: vec![
            footprint(0x10, "U1", 10.0, false),
            footprint(0x20, LOCKED_REF, 30.0, true),
        ],
        segments: Vec::new(),
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

struct Rig {
    app: Signex,
    main: iced::window::Id,
    /// Undocked by [`undock_a`].
    a: PathBuf,
    /// Shown by the main window.
    b: PathBuf,
    _tmp: tempfile::TempDir,
}

fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    drive(app, task);
}

/// Boards A and B open; B shown in the main window.
fn rig() -> Rig {
    let _ = diagnostics::init_logging();
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    let mut open = |name: &str| {
        let path = tmp.path().join(name);
        std::fs::write(&path, SnxPcb::new(board()).write_string().unwrap()).unwrap();
        send(&mut app, Message::File(FileMsg::Opened(Some(path.clone()))));
        path
    };
    let a = open("a.snxpcb");
    let b = open("b.snxpcb");
    let mut rig = Rig {
        app,
        main,
        a,
        b,
        _tmp: tmp,
    };
    show_b(&mut rig);
    rig
}

fn tab_index(app: &Signex, path: &Path) -> usize {
    app.document_state
        .tabs
        .iter()
        .position(|t| t.path == path)
        .expect("an open tab")
}

fn show_b(rig: &mut Rig) {
    let index = tab_index(&rig.app, &rig.b);
    let window_id = rig.main;
    send(
        &mut rig.app,
        Message::Tab {
            window_id,
            msg: TabMessage::Select(index),
        },
    );
    assert_eq!(rig.app.document_state.active_tab, index, "B is shown");
}

/// Board A in its own window, B still in the main one.
fn undock_a(rig: &mut Rig) -> iced::window::Id {
    let index = tab_index(&rig.app, &rig.a);
    let before: HashSet<iced::window::Id> = rig.app.ui_state.windows.keys().copied().collect();
    let _ = rig.app.update(Message::Window(WindowMsg::UndockTab(index)));
    let window = *rig
        .app
        .ui_state
        .windows
        .keys()
        .find(|id| !before.contains(id))
        .expect("an undocked window");
    let _ = rig
        .app
        .update(Message::Window(WindowMsg::UndockedTabOpened {
            path: rig.a.clone(),
            id: window,
        }));
    show_b(rig);
    window
}

fn click(x: f64, y: f64) -> PcbCanvasMsg {
    PcbCanvasMsg::Clicked {
        at: Point::new(x, y),
        tolerance_mm: 0.05,
        mode: ClickMode::Replace,
    }
}

fn drag_to(x: f64) -> PcbCanvasMsg {
    PcbCanvasMsg::FootprintDragged {
        from: Point::new(10.0, 10.0),
        to: Point::new(x, 10.0),
    }
}

fn in_window(app: &mut Signex, window: iced::window::Id, msg: PcbCanvasMsg) {
    send(app, Message::Pcb(PcbMsg::CanvasInWindow { window, msg }));
}

fn unfocus(app: &mut Signex, window: iced::window::Id) {
    send(app, Message::Window(WindowMsg::WindowUnfocused(window)));
}

fn board_text(app: &Signex, path: &Path) -> String {
    let tab = &app.document_state.tabs[tab_index(app, path)];
    let board = tab.cached_document.as_ref().and_then(TabDocument::as_pcb);
    serde_json::to_string(board.expect("a board")).unwrap()
}

fn carry_kind(app: &Signex) -> Option<CarryKind> {
    app.interaction_state.pcb_carry.as_ref().map(|c| c.kind)
}

/// A drag of U1 in the main window, in progress.
fn main_drag(rig: &mut Rig) {
    send(&mut rig.app, Message::PcbCanvas(click(10.0, 10.0)));
    send(&mut rig.app, Message::PcbCanvas(drag_to(15.0)));
    assert_eq!(carry_kind(&rig.app), Some(CarryKind::Drag));
    let ghost = &rig.app.interaction_state.pcb_canvas.highlight().ghost;
    assert!(!ghost.is_empty(), "the drag is previewed");
}

/// A drag of U1 in the board window, in progress.
fn window_drag(rig: &mut Rig, window: iced::window::Id) {
    in_window(&mut rig.app, window, click(10.0, 10.0));
    in_window(&mut rig.app, window, drag_to(15.0));
    assert_eq!(carry_kind(&rig.app), Some(CarryKind::Drag));
}

#[test]
fn focus_loss_mid_drag_ends_it_and_the_right_click_opens_the_board_menu() {
    let mut rig = rig();
    let before = board_text(&rig.app, &rig.b);
    main_drag(&mut rig);
    unfocus(&mut rig.app, rig.main);
    assert_eq!(carry_kind(&rig.app), None, "the drag ended");
    let ghost = &rig.app.interaction_state.pcb_canvas.highlight().ghost;
    assert!(ghost.is_empty(), "no ghost is left painted");
    assert_eq!(board_text(&rig.app, &rig.b), before, "nothing moved");
    send(
        &mut rig.app,
        Message::ContextMenu(ContextMenuMsg::Show(5.0, 5.0)),
    );
    assert!(
        rig.app.interaction_state.context_menu.is_some(),
        "the right-click opens the board menu"
    );
}

#[test]
fn focus_loss_ends_only_the_drag_of_the_window_that_lost_it() {
    let mut rig = rig();
    let window = undock_a(&mut rig);
    let before = board_text(&rig.app, &rig.a);
    window_drag(&mut rig, window);
    let ghost = |app: &Signex| {
        app.interaction_state.pcb_canvases[&window]
            .highlight()
            .ghost
            .len()
    };
    assert!(ghost(&rig.app) > 0, "the window previews its drag");
    unfocus(&mut rig.app, rig.main);
    assert_eq!(carry_kind(&rig.app), Some(CarryKind::Drag), "main lost it");
    unfocus(&mut rig.app, window);
    assert_eq!(carry_kind(&rig.app), None);
    assert_eq!(ghost(&rig.app), 0, "the window's ghost is gone");
    assert_eq!(board_text(&rig.app, &rig.a), before, "nothing moved");
}

#[test]
fn an_armed_move_waits_across_a_focus_change() {
    let mut rig = rig();
    send(&mut rig.app, Message::PcbCanvas(click(10.0, 10.0)));
    let id = AppCommandId::new("move_selection").expect("a catalog id");
    send(&mut rig.app, Message::Menu(MenuMessage::Command(id)));
    assert_eq!(carry_kind(&rig.app), Some(CarryKind::Move));
    unfocus(&mut rig.app, rig.main);
    assert_eq!(carry_kind(&rig.app), Some(CarryKind::Move));
}

#[test]
fn reattaching_a_board_window_drops_its_drag() {
    let mut rig = rig();
    let window = undock_a(&mut rig);
    window_drag(&mut rig, window);
    send(
        &mut rig.app,
        Message::Window(WindowMsg::ReattachTab(window)),
    );
    assert_eq!(carry_kind(&rig.app), None);
}

#[test]
fn a_board_window_closed_by_the_os_drops_its_drag() {
    let mut rig = rig();
    let window = undock_a(&mut rig);
    window_drag(&mut rig, window);
    send(
        &mut rig.app,
        Message::Window(WindowMsg::SecondaryWindowClosed(window)),
    );
    assert_eq!(carry_kind(&rig.app), None);
}

fn refusals_since(since: u64) -> usize {
    let wanted = format!("Move: {LOCKED_REF} is locked");
    diagnostics::recent_entries()
        .iter()
        .filter(|e| e.id > since && e.message.contains(&wanted))
        .count()
}

#[test]
fn a_locked_footprint_refuses_the_drag_at_its_start_once() {
    let mut rig = rig();
    let before = board_text(&rig.app, &rig.b);
    send(&mut rig.app, Message::PcbCanvas(click(30.0, 10.0)));
    let since = diagnostics::newest_entry_id().unwrap_or(0);
    let (from, to) = (Point::new(30.0, 10.0), Point::new(35.0, 10.0));
    let dragged = |x: f64| PcbCanvasMsg::FootprintDragged {
        from,
        to: Point::new(x, 10.0),
    };
    send(&mut rig.app, Message::PcbCanvas(dragged(32.0)));
    assert_eq!(refusals_since(since), 1, "refused as the drag starts");
    let ghost = &rig.app.interaction_state.pcb_canvas.highlight().ghost;
    assert!(ghost.is_empty(), "the locked part is not previewed");
    send(&mut rig.app, Message::PcbCanvas(dragged(34.0)));
    let dropped = PcbCanvasMsg::FootprintDropped { from, to };
    send(&mut rig.app, Message::PcbCanvas(dropped));
    assert_eq!(refusals_since(since), 1, "said once, not per move or drop");
    assert_eq!(board_text(&rig.app, &rig.b), before, "nothing moved");
    assert_eq!(carry_kind(&rig.app), None);
}

fn active_layer(app: &Signex, path: &Path) -> Option<CopperLayer> {
    app.ui_state.pcb_layer_views.get(path).map(|v| v.active())
}

#[test]
fn the_layer_pill_of_a_board_window_steps_that_windows_board() {
    let mut rig = rig();
    let window = undock_a(&mut rig);
    let b_before = active_layer(&rig.app, &rig.b);
    let pill = StatusBarRequest::ActiveLayerStepRequested;
    send(
        &mut rig.app,
        Signex::status_bar_message(window, None, pill.clone()),
    );
    assert_eq!(active_layer(&rig.app, &rig.a), Some(CopperLayer::Bottom));
    assert_eq!(active_layer(&rig.app, &rig.b), b_before, "B untouched");

    send(&mut rig.app, Signex::status_bar_message(rig.main, None, pill));
    assert_eq!(active_layer(&rig.app, &rig.b), Some(CopperLayer::Bottom));
    assert_eq!(
        active_layer(&rig.app, &rig.a),
        Some(CopperLayer::Bottom),
        "the main pill leaves A"
    );
}
