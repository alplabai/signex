//! #2316 — the board view turned on screen, driven through the messages the
//! UI sends: the View menu rows, the palette, the status bar's angle field,
//! and real pointer events through the canvas program. A turn is view state:
//! no undo step, no dirty mark; picks, boxes, drags, snapping and fit follow
//! it.

use iced::widget::canvas::Program;
use iced::{Event, Point as ScreenPoint, Rectangle, mouse};
use pcb_geom::{ClickMode, ItemRef};
use signex_app::app::{CommandPaletteMsg, FileMsg, Message, Signex, StatusBarRequest, TabDocument};
use signex_app::canvas::CanvasEvent;
use signex_app::keymap::AppCommandId;
use signex_app::menu_bar::MenuMessage;
use signex_app::pcb_canvas::{PcbCanvasMsg, PcbCanvasState};
use signex_types::format::SnxPcb;
use signex_types::pcb::{
    Footprint, Pad, PadShape, PadType, PcbBoard, Point, Segment, Via, ViaType,
};
use uuid::Uuid;

const U1: Uuid = Uuid::from_u128(1);
const PAD: ItemRef = ItemRef::Pad {
    footprint: U1,
    pad: Uuid::from_u128(2),
};
const FOOTPRINT: ItemRef = ItemRef::Footprint(U1);
const BOUNDS: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 800.0,
    height: 600.0,
};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// U1 on the 1.27 mm grid at (12.7, 12.7) with one 2 x 1 mm pad on its
/// origin (x 11.7..13.7, y 12.2..13.2); a track (0,0)-(25,0), 0.5 mm wide;
/// a 0.6 mm via at (30, 0).
fn board() -> PcbBoard {
    let pad = Pad {
        uuid: Uuid::from_u128(2),
        number: "1".into(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: pt(0.0, 0.0),
        size: pt(2.0, 1.0),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: None,
        roundrect_ratio: 0.0,
        rotation: 0.0,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    let footprint = Footprint {
        uuid: U1,
        reference: "U1".into(),
        value: String::new(),
        footprint_id: String::new(),
        position: pt(12.7, 12.7),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        schematic_link: None,
        library_ref: None,
        pads: vec![pad],
        graphics: Vec::new(),
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    let segment = Segment {
        uuid: Uuid::from_u128(3),
        start: pt(0.0, 0.0),
        end: pt(25.0, 0.0),
        width: 0.5,
        layer: "TopCopper".into(),
        net: 1,
        unknown_columns: Default::default(),
    };
    let via = Via {
        uuid: Uuid::from_u128(4),
        position: pt(30.0, 0.0),
        diameter: 0.6,
        drill: 0.3,
        layers: vec!["TopCopper".into(), "BottomCopper".into()],
        net: 1,
        via_type: ViaType::Through,
        unknown_columns: Default::default(),
    };
    PcbBoard {
        sketch: None,
        uuid: Uuid::nil(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: Vec::new(),
        setup: None,
        nets: Vec::new(),
        footprints: vec![footprint],
        segments: vec![segment],
        vias: vec![via],
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

struct Fixture {
    app: Signex,
    path: std::path::PathBuf,
    main: iced::window::Id,
    canvas: PcbCanvasState,
    _tmp: tempfile::TempDir,
}

/// The board opened from disk, and the canvas given its size by a first
/// pointer event (which also lands the fit the open queued).
fn open_board() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("board.snxpcb");
    std::fs::write(&path, SnxPcb::new(board()).write_string().unwrap()).unwrap();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    let task = app.update(Message::File(FileMsg::Opened(Some(path.clone()))));
    crate::drive::drive(&mut app, task);
    let mut f = Fixture {
        app,
        path,
        main,
        canvas: PcbCanvasState::default(),
        _tmp: tmp,
    };
    f.pointer(
        mouse::Event::CursorMoved {
            position: ScreenPoint::new(400.0, 300.0),
        },
        ScreenPoint::new(400.0, 300.0),
    );
    f
}

impl Fixture {
    fn send(&mut self, message: Message) {
        let task = self.app.update(message);
        crate::drive::drive(&mut self.app, task);
    }

    /// One pointer event through the board canvas program, at `at` (canvas
    /// px); what the canvas publishes goes to the app. Returns it.
    fn pointer(&mut self, event: mouse::Event, at: ScreenPoint) -> Option<Message> {
        let action = self.app.interaction_state.pcb_canvas.update(
            &mut self.canvas,
            &Event::Mouse(event),
            BOUNDS,
            mouse::Cursor::Available(at),
        );
        let message = action?.into_inner().0?;
        self.send(message.clone());
        Some(message)
    }

    /// Press and release the left button, moving between them.
    fn left_drag(&mut self, from: ScreenPoint, to: ScreenPoint) -> Option<Message> {
        self.pointer(mouse::Event::ButtonPressed(mouse::Button::Left), from);
        self.pointer(mouse::Event::CursorMoved { position: to }, to);
        self.pointer(mouse::Event::ButtonReleased(mouse::Button::Left), to)
    }

    fn click(&mut self, at: ScreenPoint) -> Option<Message> {
        self.left_drag(at, at)
    }

    /// The View menu row running `id`.
    fn menu(&mut self, id: &str) {
        let id = AppCommandId::new(id).expect("a catalog id");
        self.send(Message::Menu(MenuMessage::Command(id)));
    }

    /// The status bar of the main window asked `request`.
    fn status_bar(&mut self, request: StatusBarRequest) {
        let message = Signex::status_bar_message(self.main, None, request);
        self.send(message);
    }

    fn angle(&self) -> f32 {
        self.app.interaction_state.pcb_canvas.view_rotation_deg()
    }

    fn screen(&self, x: f64, y: f64) -> ScreenPoint {
        self.app
            .interaction_state
            .pcb_canvas
            .board_to_screen(pt(x, y))
    }

    fn selected(&self) -> Vec<ItemRef> {
        self.app
            .interaction_state
            .pcb_selection
            .selection()
            .items()
            .collect()
    }

    fn live(&self) -> PcbBoard {
        let docs = &self.app.document_state;
        docs.tabs[docs.active_tab]
            .cached_document
            .as_ref()
            .and_then(TabDocument::as_pcb)
            .expect("a board is open")
            .clone()
    }

    fn can_undo(&self) -> bool {
        let docs = &self.app.document_state;
        docs.tabs[docs.active_tab]
            .cached_document
            .as_ref()
            .and_then(TabDocument::as_pcb_engine)
            .expect("a board engine")
            .can_undo()
    }

    fn is_dirty(&self) -> bool {
        self.app.document_state.is_document_dirty(&self.path)
    }
}

#[test]
fn the_view_rows_turn_the_view_with_no_undo_step_and_no_dirty_mark() {
    let mut f = open_board();
    let board = serde_json::to_string(&f.live()).unwrap();
    f.menu("view_rotate");
    assert_eq!(f.angle(), 90.0);
    f.menu("view_rotate");
    assert_eq!(f.angle(), 180.0);
    f.menu("view_rotate_ccw");
    assert_eq!(f.angle(), 90.0);
    f.menu("view_rotate_180");
    assert_eq!(f.angle(), 270.0);
    f.menu("view_rotate_reset");
    assert_eq!(f.angle(), 0.0);
    assert!(!f.can_undo(), "a turn makes no undo step");
    assert!(!f.is_dirty(), "a turn does not mark the board dirty");
    assert_eq!(serde_json::to_string(&f.live()).unwrap(), board);
}

#[test]
fn the_palette_runs_the_turn() {
    let mut f = open_board();
    f.send(Message::CommandPalette(CommandPaletteMsg::Open));
    f.send(Message::CommandPalette(CommandPaletteMsg::QueryChanged(
        "Rotate View 180".into(),
    )));
    f.send(Message::CommandPalette(CommandPaletteMsg::ExecuteSelected));
    assert_eq!(f.angle(), 180.0);
}

/// The readout opens a field holding the angle; Enter applies any angle,
/// and text that is not one keeps the field open.
#[test]
fn the_status_bar_field_sets_any_angle() {
    let mut f = open_board();
    f.menu("view_rotate");
    f.status_bar(StatusBarRequest::ViewAngleClicked);
    let entry = f.app.ui_state.pcb_view_angle_entry.clone().expect("open");
    assert_eq!((entry.window, entry.text.as_str()), (f.main, "90"));

    f.status_bar(StatusBarRequest::ViewAngleEdited("ninety".into()));
    f.status_bar(StatusBarRequest::ViewAngleSubmitted);
    assert_eq!(f.angle(), 90.0, "not an angle: nothing turns");
    assert!(f.app.ui_state.pcb_view_angle_entry.is_some(), "still open");

    f.status_bar(StatusBarRequest::ViewAngleEdited("37°".into()));
    f.status_bar(StatusBarRequest::ViewAngleSubmitted);
    assert_eq!(f.angle(), 37.0);
    assert!(f.app.ui_state.pcb_view_angle_entry.is_none(), "closed");
    assert!(!f.can_undo() && !f.is_dirty());

    // A second click on the readout closes an open field unchanged.
    f.status_bar(StatusBarRequest::ViewAngleClicked);
    f.status_bar(StatusBarRequest::ViewAngleClicked);
    assert!(f.app.ui_state.pcb_view_angle_entry.is_none());
    assert_eq!(f.angle(), 37.0);
}

#[test]
fn a_screen_point_maps_back_to_its_board_point_at_0_37_90_and_180() {
    let mut f = open_board();
    for degrees in [0.0_f32, 37.0, 90.0, 180.0] {
        f.status_bar(StatusBarRequest::ViewAngleClicked);
        f.status_bar(StatusBarRequest::ViewAngleEdited(degrees.to_string()));
        f.status_bar(StatusBarRequest::ViewAngleSubmitted);
        assert_eq!(f.angle(), degrees);
        let screen = ScreenPoint::new(321.0, 123.0);
        let board = f
            .app
            .interaction_state
            .pcb_canvas
            .screen_to_world(screen, BOUNDS);
        let back = f.app.interaction_state.pcb_canvas.board_to_screen(board);
        assert!(
            (back.x - screen.x).abs() < 1e-2 && (back.y - screen.y).abs() < 1e-2,
            "{degrees}: {screen:?} came back as {back:?}"
        );
    }
}

#[test]
fn a_click_on_the_pad_at_37_degrees_selects_it() {
    let mut f = open_board();
    let upright = f.screen(12.7, 12.7);
    f.status_bar(StatusBarRequest::ViewAngleClicked);
    f.status_bar(StatusBarRequest::ViewAngleEdited("37".into()));
    f.status_bar(StatusBarRequest::ViewAngleSubmitted);
    let turned = f.screen(12.7, 12.7);
    assert!(
        (turned.x - upright.x).abs() > 1.0 || (turned.y - upright.y).abs() > 1.0,
        "the turn moved the pad on screen"
    );
    let Some(Message::PcbCanvas(PcbCanvasMsg::Clicked { at, .. })) = f.click(turned) else {
        panic!("a click");
    };
    assert!(
        (at.x - 12.7).abs() < 1e-3 && (at.y - 12.7).abs() < 1e-3,
        "{at:?}"
    );
    assert_eq!(f.selected(), vec![PAD]);
}

/// At 90 degrees a box dragged left to right and upward on screen runs
/// along board -y and -x: a window by the screen, so the track it cuts is
/// left out, as the upright rule would not.
#[test]
fn a_box_at_90_degrees_selects_by_the_screen() {
    let mut f = open_board();
    f.menu("view_rotate");
    let (from, to) = (f.screen(15.0, 14.0), f.screen(10.0, -1.0));
    assert!(to.x > from.x && to.y < from.y, "left to right and upward");
    let Some(Message::PcbCanvas(PcbCanvasMsg::BoxSelected {
        view_rotation_deg,
        mode,
        ..
    })) = f.left_drag(from, to)
    else {
        panic!("a box");
    };
    assert_eq!((view_rotation_deg, mode), (90.0, ClickMode::Replace));
    assert_eq!(f.selected(), vec![PAD, FOOTPRINT]);

    // Right to left over the same box: a crossing, the track too.
    f.left_drag(to, from);
    assert_eq!(f.selected().len(), 3);
    assert!(f.selected().contains(&ItemRef::Track(Uuid::from_u128(3))));
}

/// At 90 degrees a drag to the right on screen moves U1 along board -y, and
/// its origin lands on the board grid.
#[test]
fn a_drag_at_90_degrees_moves_along_the_board_axis_and_snaps_to_the_grid() {
    let mut f = open_board();
    f.menu("view_rotate");
    let grab = f.screen(12.7, 12.7);
    f.click(grab);
    assert_eq!(f.selected(), vec![PAD]);
    let scale = f.app.interaction_state.pcb_canvas.live_camera().2;
    let to = ScreenPoint::new(grab.x + 4.9 * scale, grab.y);
    let Some(Message::PcbCanvas(PcbCanvasMsg::FootprintDropped { .. })) = f.left_drag(grab, to)
    else {
        panic!("a drop");
    };
    let at = f.live().footprints[0].position;
    // 12.7 - 4.9 = 7.8 snaps to 7.62, six 1.27 mm steps.
    assert!(
        (at.x - 12.7).abs() < 1e-9 && (at.y - 7.62).abs() < 1e-9,
        "{at:?}"
    );
    assert!(f.can_undo(), "the move is the board edit, one step");
}

/// Fit on a quarter-turned view frames the whole board.
#[test]
fn fit_at_90_degrees_frames_the_whole_board() {
    let mut f = open_board();
    f.menu("view_rotate");
    f.send(Message::CanvasEvent(CanvasEvent::FitAll));
    // The canvas lands a queued fit on its next event.
    f.pointer(
        mouse::Event::CursorMoved {
            position: ScreenPoint::new(10.0, 10.0),
        },
        ScreenPoint::new(10.0, 10.0),
    );
    for (x, y) in [(-0.25, -0.3), (30.3, -0.3), (-0.25, 13.2), (30.3, 13.2)] {
        let s = f.screen(x, y);
        assert!(
            s.x >= 0.0 && s.x <= BOUNDS.width && s.y >= 0.0 && s.y <= BOUNDS.height,
            "({x}, {y}) is off screen at {s:?}"
        );
    }
    assert_eq!(f.angle(), 90.0, "fit keeps the turn");
}
