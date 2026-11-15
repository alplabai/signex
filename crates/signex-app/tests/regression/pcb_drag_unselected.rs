//! #2358 — a press on an unselected footprint and a drag select it and move
//! it in one gesture, driven by real pointer events through the board
//! canvas program: one undo step, grid snap, a locked part stays put, a
//! plain click only selects and a press on empty space draws a box. While
//! the drag is in progress the board is drawn without the footprint at its
//! old place; it is drawn only where it is going.

use iced::widget::canvas::Program;
use iced::{Event, Point as ScreenPoint, Rectangle, mouse};
use pcb_geom::ItemRef;
use signex_app::app::{EditMsg, FileMsg, InputMsg, Message, Signex, TabDocument};
use signex_app::diagnostics;
use signex_app::pcb_canvas::PcbCanvasState;
use signex_types::format::SnxPcb;
use signex_types::pcb::{Footprint, Pad, PadShape, PadType, PcbBoard, Point};
use uuid::Uuid;

const U1: Uuid = Uuid::from_u128(1);
const U2: Uuid = Uuid::from_u128(3);
const BOUNDS: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 800.0,
    height: 600.0,
};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// A footprint on the 1.27 mm grid at (`x`, 12.7) with one 2 x 1 mm pad on
/// its origin.
fn footprint(uuid: Uuid, reference: &str, x: f64, locked: bool) -> Footprint {
    let pad = Pad {
        uuid: Uuid::from_u128(uuid.as_u128() + 1),
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
    Footprint {
        uuid,
        reference: reference.into(),
        value: String::new(),
        footprint_id: String::new(),
        position: pt(x, 12.7),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked,
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

/// U1 at (12.7, 12.7); U2, locked, at (25.4, 12.7).
fn board() -> PcbBoard {
    PcbBoard {
        sketch: None,
        options: Default::default(),
        uuid: Uuid::nil(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: Vec::new(),
        setup: None,
        nets: Vec::new(),
        footprints: vec![
            footprint(U1, "U1", 12.7, false),
            footprint(U2, "U2", 25.4, true),
        ],
        segments: Vec::new(),
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

struct Fixture {
    app: Signex,
    canvas: PcbCanvasState,
    _tmp: tempfile::TempDir,
}

/// The board opened from disk, nothing selected, and the canvas given its
/// size by a first pointer event (which also lands the queued fit).
fn open_board() -> Fixture {
    let _ = diagnostics::init_logging();
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("board.snxpcb");
    std::fs::write(&path, SnxPcb::new(board()).write_string().unwrap()).unwrap();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    let task = app.update(Message::File(FileMsg::Opened(Some(path))));
    crate::drive::drive(&mut app, task);
    let mut f = Fixture {
        app,
        canvas: PcbCanvasState::default(),
        _tmp: tmp,
    };
    let centre = ScreenPoint::new(400.0, 300.0);
    f.pointer(mouse::Event::CursorMoved { position: centre }, centre);
    assert!(f.selected().is_empty(), "nothing is selected to start with");
    f
}

impl Fixture {
    fn send(&mut self, message: Message) {
        let task = self.app.update(message);
        crate::drive::drive(&mut self.app, task);
    }

    /// One pointer event through the board canvas program at `at` (canvas
    /// px); what the canvas publishes goes to the app.
    fn pointer(&mut self, event: mouse::Event, at: ScreenPoint) {
        let action = self.app.interaction_state.pcb_canvas.update(
            &mut self.canvas,
            &Event::Mouse(event),
            BOUNDS,
            mouse::Cursor::Available(at),
        );
        if let Some(message) = action.and_then(|a| a.into_inner().0) {
            self.send(message);
        }
    }

    /// Press, move in two steps, release.
    fn left_drag(&mut self, from: ScreenPoint, to: ScreenPoint) {
        self.pointer(mouse::Event::ButtonPressed(mouse::Button::Left), from);
        let half = ScreenPoint::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
        self.pointer(mouse::Event::CursorMoved { position: half }, half);
        self.pointer(mouse::Event::CursorMoved { position: to }, to);
        self.pointer(mouse::Event::ButtonReleased(mouse::Button::Left), to);
    }

    fn screen(&self, x: f64, y: f64) -> ScreenPoint {
        self.app
            .interaction_state
            .pcb_canvas
            .board_to_screen(pt(x, y))
    }

    fn scale(&self) -> f32 {
        self.app.interaction_state.pcb_canvas.live_camera().2
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

    fn snapshot(&self) -> String {
        serde_json::to_string(&self.live()).unwrap()
    }

    fn position(&self, id: Uuid) -> Point {
        let board = self.live();
        board
            .footprints
            .iter()
            .find(|f| f.uuid == id)
            .unwrap()
            .position
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
}

#[test]
fn an_unselected_footprint_is_selected_and_moved_by_one_drag_as_one_undo_step() {
    let mut f = open_board();
    let before = f.snapshot();
    let grab = f.screen(12.7, 12.7);
    let to = ScreenPoint::new(grab.x + 2.6 * f.scale(), grab.y);
    f.left_drag(grab, to);
    // 12.7 + 2.6 = 15.3 snaps to 15.24, twelve 1.27 mm steps.
    let at = f.position(U1);
    assert!(
        (at.x - 15.24).abs() < 1e-9 && (at.y - 12.7).abs() < 1e-9,
        "{at:?}"
    );
    assert_eq!(f.position(U2), pt(25.4, 12.7), "only U1 moved");
    assert_eq!(
        f.selected(),
        vec![ItemRef::Footprint(U1)],
        "the drag selected what it moved"
    );
    assert!(f.can_undo(), "the move is an undo step");
    f.send(Message::Edit(EditMsg::Undo));
    assert_eq!(f.snapshot(), before, "one undo puts U1 back");
    assert!(!f.can_undo(), "it was a single step");
}

#[test]
fn a_drag_on_an_unselected_footprint_replaces_another_selection() {
    let mut f = open_board();
    let other = f.screen(25.4, 12.7);
    f.pointer(mouse::Event::ButtonPressed(mouse::Button::Left), other);
    f.pointer(mouse::Event::ButtonReleased(mouse::Button::Left), other);
    assert!(!f.selected().is_empty(), "the click selected U2");
    let grab = f.screen(12.7, 12.7);
    let to = ScreenPoint::new(grab.x + 2.6 * f.scale(), grab.y);
    f.left_drag(grab, to);
    assert_eq!(f.selected(), vec![ItemRef::Footprint(U1)]);
    assert_eq!(f.position(U2), pt(25.4, 12.7), "the old selection stays");
    assert!((f.position(U1).x - 15.24).abs() < 1e-9);
}

#[test]
fn a_locked_unselected_footprint_is_selected_but_not_moved() {
    let mut f = open_board();
    let before = f.snapshot();
    let grab = f.screen(25.4, 12.7);
    let to = ScreenPoint::new(grab.x + 2.6 * f.scale(), grab.y);
    let since = diagnostics::newest_entry_id().unwrap_or(0);
    f.left_drag(grab, to);
    assert_eq!(f.snapshot(), before, "a locked footprint stays put");
    assert_eq!(
        f.selected(),
        vec![ItemRef::Footprint(U2)],
        "but is selected"
    );
    let noticed = diagnostics::recent_entries()
        .iter()
        .any(|e| e.id > since && e.message.contains("Move: U2 is locked"));
    assert!(noticed, "the reason goes to Messages");
    assert!(!f.can_undo(), "no undo step was made");
}

#[test]
fn a_plain_click_on_an_unselected_footprint_only_selects_it() {
    let mut f = open_board();
    let before = f.snapshot();
    let at = f.screen(12.7, 12.7);
    // A jiggle under the drag threshold is still a click.
    let jiggle = ScreenPoint::new(at.x + 2.0, at.y);
    f.pointer(mouse::Event::ButtonPressed(mouse::Button::Left), at);
    f.pointer(mouse::Event::CursorMoved { position: jiggle }, jiggle);
    f.pointer(mouse::Event::ButtonReleased(mouse::Button::Left), jiggle);
    assert_eq!(f.snapshot(), before, "nothing moved");
    assert!(!f.selected().is_empty(), "U1 is selected");
    assert!(!f.can_undo());
}

#[test]
fn a_press_on_empty_space_still_draws_a_box() {
    let mut f = open_board();
    let before = f.snapshot();
    // Right to left from empty space beside U1, a crossing that touches
    // only U1; the fitted view keeps both corners on the canvas.
    f.left_drag(f.screen(14.5, 13.6), f.screen(12.0, 12.4));
    assert_eq!(f.snapshot(), before, "a box moves nothing");
    let selected = f.selected();
    assert!(
        selected.contains(&ItemRef::Footprint(U1)),
        "the box selected U1: {selected:?}"
    );
    assert!(!selected.contains(&ItemRef::Footprint(U2)));
}

/// Whether the board content the canvas draws (the snapshot both the CPU
/// layer and the GPU scene are built from) has a pad centred at (`x`, `y`).
fn draws_pad_at(f: &Fixture, x: f32, y: f32) -> bool {
    let canvas = &f.app.interaction_state.pcb_canvas;
    let snapshot = canvas.active_renderer_snapshot().expect("a board snapshot");
    snapshot
        .pads
        .iter()
        .any(|p| (p.center[0] - x).abs() < 1e-4 && (p.center[1] - y).abs() < 1e-4)
}

/// Press on U1 and move 2.6 mm to the right, without releasing.
fn drag_u1_in_progress(f: &mut Fixture) -> ScreenPoint {
    let grab = f.screen(12.7, 12.7);
    let to = ScreenPoint::new(grab.x + 2.6 * f.scale(), grab.y);
    f.pointer(mouse::Event::ButtonPressed(mouse::Button::Left), grab);
    f.pointer(mouse::Event::CursorMoved { position: to }, to);
    assert!(f.app.interaction_state.pcb_canvas.is_carrying());
    to
}

#[test]
fn during_a_drag_the_footprint_is_drawn_only_where_it_is_going() {
    let mut f = open_board();
    let before = f.snapshot();
    assert!(draws_pad_at(&f, 12.7, 12.7), "U1 is drawn where it is");
    let to = drag_u1_in_progress(&mut f);
    assert_eq!(f.snapshot(), before, "a drag in progress edits nothing");
    assert!(
        !draws_pad_at(&f, 12.7, 12.7),
        "the board content no longer holds U1 at its old place"
    );
    assert!(draws_pad_at(&f, 25.4, 12.7), "U2 is still drawn");
    let canvas = &f.app.interaction_state.pcb_canvas;
    let scene = canvas.gpu_scene().expect("a GPU scene");
    let moving = canvas.drag_preview_scene().expect("U1 drawn on the move");
    // Pads are top-copper polygons. U1's spans x 11.7..13.7 at its old
    // place and 14.24..16.24 on the move; U2's 24.4..26.4.
    let spans = |scene: &signex_gfx::scene::Scene, lo: f32, hi: f32| {
        scene.top_polygons.iter().any(|p| {
            p.vertices
                .iter()
                .all(|v| v[0] >= lo - 0.01 && v[0] <= hi + 0.01 && (v[1] - 12.7).abs() <= 0.51)
        })
    };
    assert!(spans(&scene, 24.4, 26.4), "the GPU scene holds U2's pad");
    assert!(!spans(&scene, 11.7, 13.7), "but no U1 pad at its old place");
    assert!(
        spans(moving, 14.24, 16.24),
        "U1's pad is drawn at the snapped offset"
    );
    // The selection outline goes with it, not left behind.
    let highlight = canvas.highlight();
    assert!(highlight.selected.is_empty() && highlight.bodies.is_empty());
    assert!(!highlight.ghost.is_empty());

    f.pointer(mouse::Event::ButtonReleased(mouse::Button::Left), to);
    assert!(
        f.app
            .interaction_state
            .pcb_canvas
            .drag_preview_scene()
            .is_none()
    );
    assert!(
        draws_pad_at(&f, 15.24, 12.7),
        "U1 is drawn at its new place"
    );
    assert!(!draws_pad_at(&f, 12.7, 12.7), "and not at its old one");
}

#[test]
fn escape_during_a_drag_puts_the_footprint_back_in_the_drawing() {
    let mut f = open_board();
    let before = f.snapshot();
    drag_u1_in_progress(&mut f);
    assert!(!draws_pad_at(&f, 12.7, 12.7));
    f.send(Message::Input(InputMsg::EscapePressed { window: None }));
    assert!(
        f.app
            .interaction_state
            .pcb_canvas
            .drag_preview_scene()
            .is_none()
    );
    assert!(draws_pad_at(&f, 12.7, 12.7), "U1 is drawn where it was");
    assert_eq!(f.snapshot(), before, "nothing moved");
    assert!(!f.can_undo());
}
