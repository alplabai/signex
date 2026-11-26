//! #2460 — the board counters of the status bar, driven through the messages
//! the bar and its popovers send: a counter opens its popover over the
//! window it was clicked in, Esc and a second click close it, the Grid
//! popover sets the grid (a bad Custom… pitch changes nothing), and Clear
//! deselects. On a sheet the Grid read-out still toggles the grid.

use std::collections::HashSet;
use std::path::PathBuf;

use pcb_geom::ClickMode;
use signex_app::app::{FileMsg, InputMsg, Message, Signex, StatusBarRequest, UiMsg, WindowMsg};
use signex_app::diagnostics;
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_app::status_bar::{BoardCounter, BoardPopoverEvent, BoardPopoverKind};
use signex_types::format::SnxPcb;
use signex_types::pcb::{Footprint, LayerDef, Pad, PadShape, PadType, PcbBoard, Point};
use uuid::Uuid;

use crate::drive::drive;

fn part(id: u128, reference: &str, x: f64) -> Footprint {
    let pad = Pad {
        uuid: Uuid::from_u128(id + 1),
        number: "1".into(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: Point::new(0.0, 0.0),
        size: Point::new(1.0, 1.0),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: None,
        roundrect_ratio: 0.0,
        trapezoid_delta: Default::default(),
        custom_outline: Vec::new(),
        rotation: 0.0,
        layer_sizes: None,
        net_locked: false,
        thermal: Default::default(),
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
        locked: false,
        schematic_link: None,
        library_ref: None,
        component: Default::default(),
        pads: vec![pad],
        graphics: Vec::new(),
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

/// R1 at (10, 10) and R2 at (30, 10); two copper layers.
fn board() -> PcbBoard {
    let layer = |id: u8, name: &str| LayerDef {
        id,
        name: name.to_owned(),
        layer_type: "signal".to_owned(),
        display_name: String::new(),
    };
    PcbBoard {
        sketch: None,
        stackup: None,
        rooms: Vec::new(),
        options: Default::default(),
        uuid: Uuid::now_v7(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: vec![layer(0, "TopCopper"), layer(31, "BottomCopper")],
        setup: None,
        nets: Vec::new(),
        footprints: vec![part(0x10, "R1", 10.0), part(0x20, "R2", 30.0)],
        segments: Vec::new(),
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        free: Default::default(),
        unknown: Default::default(),
    }
}

struct Rig {
    app: Signex,
    main: iced::window::Id,
    a: PathBuf,
    _tmp: tempfile::TempDir,
}

fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    drive(app, task);
}

/// Boards A and B open; B is shown in the main window.
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
    let _b = open("b.snxpcb");
    Rig {
        app,
        main,
        a,
        _tmp: tmp,
    }
}

/// Board A in its own window.
fn undock_a(rig: &mut Rig) -> iced::window::Id {
    let index = rig
        .app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == rig.a)
        .expect("tab A");
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
    window
}

fn click(app: &mut Signex, window: iced::window::Id, counter: BoardCounter) {
    let request = StatusBarRequest::BoardCounterClicked {
        window: None,
        counter,
    };
    send(app, Signex::status_bar_message(window, None, request));
}

fn popover_event(app: &mut Signex, event: BoardPopoverEvent) {
    send(
        app,
        Message::Ui(UiMsg::StatusBar(StatusBarRequest::BoardPopover(event))),
    );
}

fn open_popover(app: &Signex) -> Option<(iced::window::Id, BoardPopoverKind)> {
    let popover = app.ui_state.board_status.popover.as_ref()?;
    Some((popover.window, popover.kind))
}

#[test]
fn a_counter_opens_its_popover_over_its_own_window_and_esc_closes_it() {
    let mut rig = rig();
    let window = undock_a(&mut rig);
    click(&mut rig.app, window, BoardCounter::Drc);
    assert_eq!(
        open_popover(&rig.app),
        Some((window, BoardPopoverKind::Drc))
    );
    // Esc in that window closes it and leaves its tool alone.
    send(
        &mut rig.app,
        Message::Input(InputMsg::EscapePressed {
            window: Some(window),
        }),
    );
    assert_eq!(open_popover(&rig.app), None);
    // The main window's counter opens over the main window; a second click
    // there closes it.
    click(&mut rig.app, rig.main, BoardCounter::Unrouted);
    assert_eq!(
        open_popover(&rig.app),
        Some((rig.main, BoardPopoverKind::Unrouted))
    );
    click(&mut rig.app, rig.main, BoardCounter::Routed);
    assert_eq!(open_popover(&rig.app), None, "Routed shares the popover");
}

/// Mockup row 164: on a board a right-click on the Grid read-out opens the Grid
/// popover and leaves the grid shown; a preset and a Custom… pitch set it, a
/// bad pitch is refused. A left click still shows and hides the grid.
#[test]
fn the_board_grid_popover_sets_the_grid() {
    let mut rig = rig();
    let shown = rig.app.ui_state.grid_visible;
    click(&mut rig.app, rig.main, BoardCounter::Grid);
    assert_eq!(
        rig.app.ui_state.grid_visible, shown,
        "opening the popover toggles nothing"
    );
    assert_eq!(
        open_popover(&rig.app),
        Some((rig.main, BoardPopoverKind::Grid))
    );
    popover_event(&mut rig.app, BoardPopoverEvent::GridPresetPicked(0.25));
    assert_eq!(rig.app.status_grid(rig.main).size_mm, 0.25);
    popover_event(
        &mut rig.app,
        BoardPopoverEvent::CustomGridEdited("0.2".into()),
    );
    popover_event(&mut rig.app, BoardPopoverEvent::CustomGridSubmitted);
    assert_eq!(rig.app.status_grid(rig.main).size_mm, 0.2);
    popover_event(
        &mut rig.app,
        BoardPopoverEvent::CustomGridEdited("-3".into()),
    );
    popover_event(&mut rig.app, BoardPopoverEvent::CustomGridSubmitted);
    assert_eq!(
        rig.app.status_grid(rig.main).size_mm,
        0.2,
        "refused, unchanged"
    );
    // A left click on the read-out (and View ▸ Toggle Grid) still shows and
    // hides it.
    send(
        &mut rig.app,
        Message::Ui(UiMsg::StatusBar(StatusBarRequest::ToggleGrid)),
    );
    assert_eq!(rig.app.ui_state.grid_visible, !shown);
}

/// Caner, 2026-11-21: an undocked board shows its own grid, not the main
/// window's.
#[test]
fn an_undocked_board_shows_its_own_grid() {
    let mut rig = rig();
    let window = undock_a(&mut rig);
    let own = rig.app.status_grid(window).size_mm;
    click(&mut rig.app, rig.main, BoardCounter::Grid);
    let other = if own == 0.25 { 0.5 } else { 0.25 };
    popover_event(&mut rig.app, BoardPopoverEvent::GridPresetPicked(other));
    assert_eq!(rig.app.status_grid(rig.main).size_mm, other);
    assert_eq!(rig.app.status_grid(window).size_mm, own);
}

#[test]
fn clear_deselects_the_board_in_view() {
    let mut rig = rig();
    send(
        &mut rig.app,
        Message::PcbCanvas(PcbCanvasMsg::Clicked {
            at: Point::new(10.0, 10.0),
            tolerance_mm: 0.05,
            mode: ClickMode::Replace,
        }),
    );
    let selected = |app: &Signex| app.interaction_state.pcb_selection.selection().len();
    assert_eq!(selected(&rig.app), 1, "R1 is selected");
    click(&mut rig.app, rig.main, BoardCounter::Selected);
    popover_event(&mut rig.app, BoardPopoverEvent::SelectionCleared);
    assert_eq!(selected(&rig.app), 0);
    assert_eq!(open_popover(&rig.app), None);
}
