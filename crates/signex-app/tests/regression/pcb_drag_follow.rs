//! #2379 — dragging a footprint takes the track ends on its pads along, in
//! the same undo step as the move, and the ratsnest follows the carried
//! pads while the drag is in progress. Driven by real pointer events
//! through the board canvas program.

use iced::widget::canvas::Program;
use iced::{Event, Point as ScreenPoint, Rectangle, mouse};
use signex_app::app::{EditMsg, FileMsg, Message, Signex, TabDocument};
use signex_app::diagnostics;
use signex_app::pcb_canvas::PcbCanvasState;
use signex_types::format::SnxPcb;
use signex_types::pcb::{
    Footprint, NetDef, Pad, PadNet, PadShape, PadType, PcbBoard, Point, Segment,
};
use uuid::Uuid;

const U1: Uuid = Uuid::from_u128(1);
const U2: Uuid = Uuid::from_u128(3);
const TRACK: Uuid = Uuid::from_u128(10);
const BOUNDS: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 800.0,
    height: 600.0,
};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// A footprint at (`x`, 12.7) with one 2 x 1 mm pad of net 1 on its origin.
fn footprint(uuid: Uuid, reference: &str, x: f64) -> Footprint {
    let pad = Pad {
        uuid: Uuid::from_u128(uuid.as_u128() + 1),
        number: "1".into(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: pt(0.0, 0.0),
        size: pt(2.0, 1.0),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: Some(PadNet {
            number: 1,
            name: "N1".into(),
        }),
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

/// U1 at (12.7, 12.7) and U2 at (25.4, 12.7), both on net 1 and not
/// joined, so one ratsnest line runs between their pads. A net 1 track
/// leaves U1's pad at its left edge, (11.9, 12.7), for (5.08, 12.7).
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
        nets: vec![NetDef {
            number: 1,
            name: "N1".into(),
        }],
        footprints: vec![footprint(U1, "U1", 12.7), footprint(U2, "U2", 25.4)],
        segments: vec![Segment {
            uuid: TRACK,
            start: pt(11.9, 12.7),
            end: pt(5.08, 12.7),
            width: 0.25,
            layer: "TopCopper".into(),
            net: 1,
            unknown_columns: Default::default(),
        }],
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        free: Default::default(),
        unknown: Default::default(),
    }
}

struct Fixture {
    app: Signex,
    canvas: PcbCanvasState,
    _tmp: tempfile::TempDir,
}

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
    f
}

impl Fixture {
    fn send(&mut self, message: Message) {
        let task = self.app.update(message);
        crate::drive::drive(&mut self.app, task);
    }

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

    fn screen(&self, x: f64, y: f64) -> ScreenPoint {
        self.app
            .interaction_state
            .pcb_canvas
            .board_to_screen(pt(x, y))
    }

    fn scale(&self) -> f32 {
        self.app.interaction_state.pcb_canvas.live_camera().2
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

    fn can_undo(&self) -> bool {
        let docs = &self.app.document_state;
        docs.tabs[docs.active_tab]
            .cached_document
            .as_ref()
            .and_then(TabDocument::as_pcb_engine)
            .expect("a board engine")
            .can_undo()
    }

    /// Press on the right half of U1's pad, clear of the track, and move
    /// 2.6 mm to the right without releasing. U1 lands on 15.24, two grid
    /// steps on.
    fn drag_u1_in_progress(&mut self) -> ScreenPoint {
        let grab = self.screen(13.2, 12.7);
        let to = ScreenPoint::new(grab.x + 2.6 * self.scale(), grab.y);
        self.pointer(mouse::Event::ButtonPressed(mouse::Button::Left), grab);
        self.pointer(mouse::Event::CursorMoved { position: to }, to);
        assert!(self.app.interaction_state.pcb_canvas.is_carrying());
        to
    }
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

/// Whether `lines` hold one from (`x0`, `y`) to (`x1`, `y`), either way round.
fn has_line(
    lines: impl IntoIterator<Item = ([f32; 2], [f32; 2])>,
    x0: f32,
    x1: f32,
    y: f32,
) -> bool {
    lines.into_iter().any(|(p0, p1)| {
        let along = |a: [f32; 2], b: [f32; 2]| near(a[0], x0) && near(b[0], x1);
        near(p0[1], y) && near(p1[1], y) && (along(p0, p1) || along(p1, p0))
    })
}

#[test]
fn a_dragged_footprint_takes_its_track_end_along_in_one_undo_step() {
    let mut f = open_board();
    let before = f.snapshot();
    let to = f.drag_u1_in_progress();
    f.pointer(mouse::Event::ButtonReleased(mouse::Button::Left), to);

    let board = f.live();
    let u1 = board.footprints.iter().find(|fp| fp.uuid == U1).unwrap();
    let delta = pt(u1.position.x - 12.7, u1.position.y - 12.7);
    assert!((u1.position.x - 15.24).abs() < 1e-9, "{:?}", u1.position);
    let track = board.segments.iter().find(|s| s.uuid == TRACK).unwrap();
    assert!(
        (track.start.x - (11.9 + delta.x)).abs() < 1e-9
            && (track.start.y - (12.7 + delta.y)).abs() < 1e-9,
        "the end on the pad moved with it: {:?}",
        track.start
    );
    assert_eq!(track.end, pt(5.08, 12.7), "the far end stays");

    f.send(Message::Edit(EditMsg::Undo));
    assert_eq!(f.snapshot(), before, "one undo puts U1 and the track back");
    assert!(!f.can_undo(), "it was a single step");
}

#[test]
fn during_a_drag_the_ratsnest_ends_sit_on_the_carried_pads() {
    let mut f = open_board();
    let ratsnest_of = |f: &Fixture| -> Vec<([f32; 2], [f32; 2])> {
        let canvas = &f.app.interaction_state.pcb_canvas;
        let snapshot = canvas.active_renderer_snapshot().expect("a board snapshot");
        snapshot
            .ratsnest_lines
            .iter()
            .map(|l| (l.p0, l.p1))
            .collect()
    };
    assert!(
        has_line(ratsnest_of(&f), 12.7, 25.4, 12.7),
        "before the drag the line joins U1 and U2"
    );

    let to = f.drag_u1_in_progress();
    assert!(
        !has_line(ratsnest_of(&f), 12.7, 25.4, 12.7),
        "no line is left on U1's old place"
    );
    let canvas = &f.app.interaction_state.pcb_canvas;
    let moving = canvas.drag_preview_scene().expect("U1 drawn on the move");
    let lines = moving.overlay_lines.iter().map(|l| (l.p0, l.p1));
    assert!(
        has_line(lines, 15.24, 25.4, 12.7),
        "the ratsnest runs from U1's carried pad to U2"
    );
    let snapshot = canvas.active_renderer_snapshot().expect("a board snapshot");
    assert!(
        snapshot.traces.is_empty(),
        "the track that follows the pad is drawn with it, not left behind"
    );

    f.pointer(mouse::Event::ButtonReleased(mouse::Button::Left), to);
    assert!(
        has_line(ratsnest_of(&f), 15.24, 25.4, 12.7),
        "after the drop the line stays on U1's new place"
    );
}
