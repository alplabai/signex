//! #2390 — the board 3D view, driven through the messages the UI sends: the
//! View menu row, the palette, the view's bar and real pointer events
//! through the view's shader program. The view is view state: no undo step,
//! no dirty mark; a click in 3D selects in the flat view.

use std::sync::Arc;

use iced::widget::shader::Program;
use iced::{Event, Point as ScreenPoint, Rectangle, mouse, window};
use pcb_geom::ItemRef;
use signex_app::app::{
    Board3dGesture, Board3dMsg, CommandPaletteMsg, ContextMenuMsg, FileMsg, Message, PcbMsg,
    RenameDialogState, RenameMsg, Signex, TabContextAction,
};
use signex_app::board_3d_shader::{Board3dPointer, Board3dProgram};
use signex_app::canvas::CanvasEvent;
use signex_app::keymap::AppCommandId;
use signex_app::menu_bar::MenuMessage;
use signex_gfx::camera3d::{OrbitCamera, ViewPreset};
use signex_renderer::board3d::BoardScene3d;
use signex_types::format::SnxPcb;
use signex_types::part::FootprintSourceRef;
use signex_types::pcb::{Footprint, Pad, PadShape, PadType, PcbBoard, Point};
use uuid::Uuid;

pub(crate) const U1: Uuid = Uuid::from_u128(1);
pub(crate) const U2: Uuid = Uuid::from_u128(5);
pub(crate) const BOUNDS: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 800.0,
    height: 600.0,
};
/// The board is 1.6 mm thick and a part with no height stands 1 mm tall.
pub(crate) const PART_TOP_MM: f32 = 2.6;

fn footprint(uuid: Uuid, at: (f64, f64)) -> Footprint {
    let pad = Pad {
        uuid: Uuid::from_u128(uuid.as_u128() + 100),
        number: "1".into(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: Point::new(0.0, 0.0),
        size: Point::new(2.0, 1.0),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: None,
        roundrect_ratio: 0.0,
        trapezoid_delta: Default::default(),
        custom_outline: Vec::new(),
        rotation: 0.0,
        layer_sizes: None,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    Footprint {
        uuid,
        reference: "U1".into(),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(at.0, at.1),
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

/// A 40 x 30 mm board; U1 at (12.7, 12.7) with no library footprint; U2
/// at (30, 20) naming a library footprint in a library that is not open.
fn board() -> PcbBoard {
    let outline = [(0.0, 0.0), (40.0, 0.0), (40.0, 30.0), (0.0, 30.0)];
    let mut linked = footprint(U2, (30.0, 20.0));
    linked.library_ref = Some(FootprintSourceRef {
        library: Uuid::from_u128(77),
        footprint: Uuid::from_u128(78),
        version: "1".into(),
    });
    PcbBoard {
        sketch: None,
        stackup: None,
        rooms: Vec::new(),
        options: Default::default(),
        uuid: Uuid::nil(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: outline.iter().map(|&(x, y)| Point::new(x, y)).collect(),
        layers: Vec::new(),
        setup: None,
        nets: Vec::new(),
        footprints: vec![footprint(U1, (12.7, 12.7)), linked],
        segments: Vec::new(),
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        free: Default::default(),
        unknown: Default::default(),
    }
}

pub(crate) struct Fixture {
    pub(crate) app: Signex,
    pub(crate) path: std::path::PathBuf,
    pointer: Board3dPointer,
    _tmp: tempfile::TempDir,
}

pub(crate) fn open_board() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("board.snxpcb");
    std::fs::write(&path, SnxPcb::new(board()).write_string().unwrap()).unwrap();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    let task = app.update(Message::File(FileMsg::Opened(Some(path.clone()))));
    crate::drive::drive(&mut app, task);
    Fixture {
        app,
        path,
        pointer: Board3dPointer::default(),
        _tmp: tmp,
    }
}

impl Fixture {
    pub(crate) fn send(&mut self, message: Message) {
        let task = self.app.update(message);
        crate::drive::drive(&mut self.app, task);
    }

    pub(crate) fn menu(&mut self, id: &str) {
        let id = AppCommandId::new(id).expect("a catalog id");
        self.send(Message::Menu(MenuMessage::Command(id)));
    }

    pub(crate) fn view_3d(&mut self, event: Board3dMsg) {
        self.send(Message::Pcb(PcbMsg::Board3d(event)));
    }

    pub(crate) fn in_3d(&self) -> bool {
        self.app
            .interaction_state
            .board_3d
            .as_ref()
            .is_some_and(|session| session.board_path == self.path)
    }

    pub(crate) fn camera(&self) -> OrbitCamera {
        self.app
            .interaction_state
            .board_3d
            .as_ref()
            .expect("the board is in 3D")
            .camera
    }

    /// One pointer event through the view's shader program at `at`; what it
    /// publishes goes to the app.
    pub(crate) fn pointer(&mut self, event: mouse::Event, at: ScreenPoint) -> Option<Message> {
        self.through_program(&Event::Mouse(event), BOUNDS, at)
    }

    /// One redraw of the view at `bounds`; what it publishes goes to the app.
    pub(crate) fn redraw(&mut self, bounds: Rectangle) -> Option<Message> {
        let event = Event::Window(window::Event::RedrawRequested(std::time::Instant::now()));
        self.through_program(&event, bounds, ScreenPoint::ORIGIN)
    }

    fn through_program(
        &mut self,
        event: &Event,
        bounds: Rectangle,
        at: ScreenPoint,
    ) -> Option<Message> {
        let session = self.app.interaction_state.board_3d.as_ref()?;
        let program = Board3dProgram {
            scene: Arc::new(BoardScene3d::default()),
            generation: 0,
            camera: session.camera,
            highlights: Vec::new(),
            highlight: [0.0; 4],
            viewport: session.viewport,
            origin: session.origin,
            lighting: session.lighting,
        };
        let action = program.update(
            &mut self.pointer,
            event,
            bounds,
            mouse::Cursor::Available(at),
        );
        let message = action?.into_inner().0?;
        self.send(message.clone());
        Some(message)
    }

    pub(crate) fn click(&mut self, at: ScreenPoint) -> Option<Message> {
        self.pointer(mouse::Event::ButtonPressed(mouse::Button::Left), at);
        self.pointer(mouse::Event::ButtonReleased(mouse::Button::Left), at)
    }

    /// Where board point `(x, y)` at height `z` shows in the view.
    pub(crate) fn screen(&self, x: f32, y: f32, z: f32) -> ScreenPoint {
        let at = self
            .camera()
            .project([x, -y, z], BOUNDS.width, BOUNDS.height)
            .expect("in front of the eye");
        ScreenPoint::new(at[0], at[1])
    }

    pub(crate) fn selected(&self) -> Vec<ItemRef> {
        self.app
            .interaction_state
            .pcb_selection
            .selection()
            .items()
            .collect()
    }

    pub(crate) fn live(&self) -> PcbBoard {
        let docs = &self.app.document_state;
        docs.tab_board_engine(docs.active_tab)
            .map(|engine| engine.board())
            .expect("a board is open")
            .clone()
    }

    pub(crate) fn can_undo(&self) -> bool {
        let docs = &self.app.document_state;
        docs.tab_board_engine(docs.active_tab)
            .expect("a board engine")
            .can_undo()
    }
}

/// Size the view the way its first redraw does, so the frame the test
/// projects with is the one a click is read with.
pub(crate) fn sized(f: &mut Fixture) {
    f.redraw(BOUNDS);
    let session = f.app.interaction_state.board_3d.as_ref().expect("in 3D");
    assert_eq!(session.viewport, [BOUNDS.width, BOUNDS.height]);
}

#[test]
fn the_view_row_shows_the_board_in_3d_and_flat_again_with_no_undo_step() {
    let mut f = open_board();
    let before = serde_json::to_string(&f.live()).unwrap();
    f.menu("view_board_3d");
    assert!(f.in_3d());
    f.menu("view_board_3d");
    assert!(!f.in_3d());
    assert!(!f.can_undo(), "the view makes no undo step");
    assert!(!f.app.document_state.is_document_dirty(&f.path));
    assert_eq!(serde_json::to_string(&f.live()).unwrap(), before);
}

#[test]
fn the_palette_runs_the_view_and_the_bar_leaves_it() {
    let mut f = open_board();
    f.send(Message::CommandPalette(CommandPaletteMsg::Open));
    f.send(Message::CommandPalette(CommandPaletteMsg::QueryChanged(
        "Show the board in 3D".into(),
    )));
    f.send(Message::CommandPalette(CommandPaletteMsg::ExecuteSelected));
    assert!(f.in_3d());
    f.view_3d(Board3dMsg::LeaveRequested);
    assert!(!f.in_3d());
}

/// The view opens looking from Iso at the board, framed on it (mockup
/// row 175).
#[test]
fn the_view_opens_on_iso_framed_on_the_board() {
    let mut f = open_board();
    f.menu("view_board_3d");
    sized(&mut f);
    let camera = f.camera();
    let mut iso = OrbitCamera::default();
    iso.set_preset(ViewPreset::Iso);
    assert_eq!(
        (camera.yaw_deg, camera.pitch_deg),
        (iso.yaw_deg, iso.pitch_deg)
    );
    for (x, y) in [(0.0, 0.0), (40.0, 30.0)] {
        let at = f.screen(x, y, 1.6);
        assert!(BOUNDS.contains(at), "corner ({x}, {y}) at {at:?}");
    }
}

/// A click on a part selects its footprint in the flat view; a click on
/// the bare board clears the selection.
#[test]
fn a_click_on_a_part_selects_its_footprint() {
    let mut f = open_board();
    f.menu("view_board_3d");
    sized(&mut f);
    let part = f.screen(12.7, 12.7, PART_TOP_MM);
    let sent = f.click(part);
    assert!(matches!(
        sent,
        Some(Message::Pcb(PcbMsg::Board3d(Board3dMsg::Pointer {
            gesture: Board3dGesture::Clicked(_),
            ..
        })))
    ));
    assert_eq!(f.selected(), vec![ItemRef::Footprint(U1)]);

    let bare = f.screen(5.0, 25.0, 1.6);
    f.click(bare);
    assert!(f.selected().is_empty());
    assert!(!f.can_undo());
}

/// A part whose library is not open still stands, as its body box.
#[test]
fn a_part_whose_model_cannot_be_read_is_a_plain_body() {
    let mut f = open_board();
    f.menu("view_board_3d");
    sized(&mut f);
    let part = f.screen(30.0, 20.0, PART_TOP_MM);
    f.click(part);
    assert_eq!(f.selected(), vec![ItemRef::Footprint(U2)]);
}

/// A left drag turns the view, the wheel zooms, a preset looks from there
/// and Fit All frames the 3D view.
#[test]
fn drag_wheel_presets_and_fit_move_the_camera_only() {
    let mut f = open_board();
    f.menu("view_board_3d");
    sized(&mut f);
    let start = f.camera();

    f.pointer(
        mouse::Event::ButtonPressed(mouse::Button::Left),
        ScreenPoint::new(400.0, 300.0),
    );
    f.pointer(
        mouse::Event::CursorMoved {
            position: ScreenPoint::new(450.0, 280.0),
        },
        ScreenPoint::new(450.0, 280.0),
    );
    f.pointer(
        mouse::Event::ButtonReleased(mouse::Button::Left),
        ScreenPoint::new(450.0, 280.0),
    );
    let turned = f.camera();
    assert_ne!(turned.yaw_deg, start.yaw_deg, "a drag turns the view");
    assert!(f.selected().is_empty(), "a drag is not a click");

    f.pointer(
        mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
        },
        ScreenPoint::new(400.0, 300.0),
    );
    assert!(f.camera().distance < turned.distance, "the wheel zooms in");

    f.view_3d(Board3dMsg::PresetPicked(ViewPreset::Bottom));
    assert_eq!(f.camera().pitch_deg, -90.0);
    let bottom = f.camera();

    f.send(Message::CanvasEvent(CanvasEvent::FitAll));
    let fitted = f.camera();
    assert_eq!(
        fitted.pitch_deg, bottom.pitch_deg,
        "Fit keeps the direction"
    );
    assert!(f.in_3d());
    assert!(!f.can_undo());
}

/// A tall, narrow view: framing it needs the board further away than the
/// assumed wide shape does, so a refit shows in the camera.
const NARROW: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 300.0,
    height: 600.0,
};

/// The first redraw reports the view's real size and frames the board for
/// it; a redraw at the same size reports nothing more.
#[test]
fn the_first_redraw_frames_the_view_for_its_real_shape() {
    let mut f = open_board();
    f.menu("view_board_3d");
    let assumed = f.camera();
    let sent = f.redraw(NARROW);
    assert!(
        matches!(
            sent,
            Some(Message::Pcb(PcbMsg::Board3d(Board3dMsg::ViewportSized(size))))
                if size == [NARROW.width, NARROW.height]
        ),
        "{sent:?}"
    );
    assert!(
        f.camera().distance > assumed.distance,
        "framed for the narrow shape"
    );
    assert!(f.redraw(NARROW).is_none(), "the size is reported once");
    assert!(!f.can_undo());
}

/// A gesture before any redraw does not frame the view again: the wheel
/// step of zero leaves the camera exactly where it was.
#[test]
fn a_first_gesture_does_not_reframe_the_view() {
    let mut f = open_board();
    f.menu("view_board_3d");
    let before = f.camera();
    f.view_3d(Board3dMsg::Pointer {
        viewport: [NARROW.width, NARROW.height],
        gesture: Board3dGesture::Zoomed(0.0),
    });
    assert_eq!(f.camera(), before);
    f.redraw(NARROW);
    assert_eq!(f.camera(), before, "nor does a redraw after it");
}

/// A renamed board stays in 3D under its new name.
#[test]
fn a_renamed_board_keeps_its_3d_view() {
    let mut f = open_board();
    f.menu("view_board_3d");
    let renamed = f.path.with_file_name("renamed.snxpcb");
    f.app.ui_state.rename_dialog = Some(Box::new(RenameDialogState {
        project_id: None,
        target_path: f.path.clone(),
        tree_path: vec![0],
        buffer: "renamed.snxpcb".to_string(),
        error: None,
        is_project_rename: false,
        placing_symbols: Vec::new(),
        name_clash: None,
    }));
    crate::settle_support::settle(&mut f.app, Message::Rename(RenameMsg::Submit));
    assert!(renamed.exists(), "the file moved");
    f.path = renamed;
    assert!(f.in_3d(), "the view follows the file");
    let docs = &f.app.document_state;
    assert_eq!(
        docs.tabs[docs.active_tab].path, f.path,
        "and the tab it is shown on"
    );
}

/// Closing the board's tab ends its 3D view: the file opens flat again.
#[test]
fn a_closed_board_opens_flat_again() {
    let mut f = open_board();
    f.menu("view_board_3d");
    let idx = f.app.document_state.active_tab;
    f.send(Message::ContextMenu(ContextMenuMsg::TabAction(
        TabContextAction::Close(idx),
    )));
    assert!(f.app.interaction_state.board_3d.is_none());
    f.send(Message::File(FileMsg::Opened(Some(f.path.clone()))));
    assert!(!f.in_3d(), "the board opens flat");
}
