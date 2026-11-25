//! #2513 — a press on an unselected track or via and a drag moves it in one
//! gesture, with no click to select it first. The tracks joined to it
//! stretch by their joined end, their other end stays on its pad or via,
//! the drop is what the drag preview drew, and it is one undo step. Driven
//! from real pointer events on the board canvas into `Signex::update`.

use std::path::{Path, PathBuf};

use iced::widget::canvas;
use iced::{Event, Rectangle, mouse};
use signex_types::pcb::{PcbBoard, Point, Segment};
use uuid::Uuid;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::pcb_canvas::{PcbCanvasMsg, PcbCanvasState, carried_board};

const U1: Uuid = Uuid::from_u128(0x2513_0001);
const VIA: Uuid = Uuid::from_u128(0x2513_0002);
/// From U1's pad at (0, 0) to (5, 0).
const ON_PAD: Uuid = Uuid::from_u128(0x2513_0010);
/// The track the tests drag, from (5, 0) to (10, 0).
const DRAGGED: Uuid = Uuid::from_u128(0x2513_0011);
/// From (10, 0) to the via at (15, 0).
const TO_VIA: Uuid = Uuid::from_u128(0x2513_0012);
/// From the via at (15, 0) to (20, 0).
const FROM_VIA: Uuid = Uuid::from_u128(0x2513_0013);
/// A locked track apart from the others, from (30, 0) to (35, 0).
const LOCKED: Uuid = Uuid::from_u128(0x2513_0014);
const BOUNDS: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 1600.0,
    height: 1200.0,
};

fn track(id: Uuid, a: (f64, f64), b: (f64, f64), locked: bool) -> serde_json::Value {
    serde_json::json!({"uuid": id, "start": {"x": a.0, "y": a.1}, "end": {"x": b.0, "y": b.1},
                       "width": 0.25, "layer": "TopCopper", "net": 1, "locked": locked})
}

/// A chain on net 1, on a 0.5 mm grid: U1's pad, three tracks, a via, a
/// fourth track; and a locked track apart from them.
fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 0.5},
        "nets": [{"number": 1, "name": "N"}],
        "footprints": [{
            "uuid": U1, "reference": "U1", "position": {"x": 0.0, "y": 0.0},
            "rotation": 0.0, "layer": "TopCopper", "locked": false,
            "pads": [{"uuid": Uuid::from_u128(0x2513_0003), "number": "1", "pad_type": "smd",
                      "shape": "rect", "position": {"x": 0.0, "y": 0.0},
                      "size": {"x": 1.0, "y": 1.0}, "drill": null,
                      "layers": ["TopCopper"], "net": {"number": 1, "name": "N"}}]
        }],
        "vias": [{"uuid": VIA, "position": {"x": 15.0, "y": 0.0}, "diameter": 0.8,
                  "drill": 0.4, "layers": ["TopCopper", "BottomCopper"], "net": 1}],
        "segments": [
            track(ON_PAD, (0.0, 0.0), (5.0, 0.0), false),
            track(DRAGGED, (5.0, 0.0), (10.0, 0.0), false),
            track(TO_VIA, (10.0, 0.0), (15.0, 0.0), false),
            track(FROM_VIA, (15.0, 0.0), (20.0, 0.0), false),
            track(LOCKED, (30.0, 0.0), (35.0, 0.0), true)
        ]
    }))
    .unwrap()
}

struct Rig {
    app: Signex,
    path: PathBuf,
    state: PcbCanvasState,
    _dir: tempfile::TempDir,
}

fn rig() -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    let path = dir.path().join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board());
    app.ui_state.pcb_snap.objects.clear();
    app.ui_state.pcb_snap.to_guides = false;
    app.ui_state.pcb_snap.to_axes = false;
    app.refresh_pcb_highlight();
    let mut state = PcbCanvasState::default();
    // The first event settles the view's fit, so pixels read after it hold.
    let settled = canvas::Program::update(
        &app.interaction_state.pcb_canvas,
        &mut state,
        &Event::Mouse(mouse::Event::CursorLeft),
        BOUNDS,
        mouse::Cursor::Unavailable,
    );
    if let Some(message) = settled.and_then(|action| action.into_inner().0) {
        let _ = app.update(message);
    }
    Rig {
        app,
        path,
        state,
        _dir: dir,
    }
}

impl Rig {
    fn pixel(&self, at: Point) -> iced::Point {
        self.app
            .interaction_state
            .pcb_canvas
            .camera_ref()
            .world_to_screen(iced::Point::new(at.x as f32, at.y as f32))
    }

    fn send(&mut self, event: Event, pixel: iced::Point) {
        let cursor = mouse::Cursor::Available(pixel);
        let action = canvas::Program::update(
            &self.app.interaction_state.pcb_canvas,
            &mut self.state,
            &event,
            BOUNDS,
            cursor,
        );
        if let Some(message) = action.and_then(|action| action.into_inner().0) {
            let _ = self.app.update(message);
        }
    }

    fn press(&mut self, at: Point) {
        let pixel = self.pixel(at);
        self.send(Event::Mouse(mouse::Event::CursorMoved { position: pixel }), pixel);
        let press = mouse::Event::ButtonPressed(mouse::Button::Left);
        self.send(Event::Mouse(press), pixel);
    }

    fn move_to(&mut self, at: Point) {
        let pixel = self.pixel(at);
        self.send(Event::Mouse(mouse::Event::CursorMoved { position: pixel }), pixel);
    }

    fn release(&mut self, at: Point) {
        let pixel = self.pixel(at);
        let release = mouse::Event::ButtonReleased(mouse::Button::Left);
        self.send(Event::Mouse(release), pixel);
    }

    fn drag(&mut self, from: Point, to: Point) {
        self.press(from);
        self.move_to(to);
        self.release(to);
    }

    fn click(&mut self, at: Point, mode: pcb_geom::ClickMode) {
        let _ = self.app.update(Message::PcbCanvas(PcbCanvasMsg::Clicked {
            at,
            tolerance_mm: 0.05,
            mode,
        }));
    }

    fn board(&self) -> &PcbBoard {
        self.app
            .document_state
            .pcb_board(&self.path)
            .expect("board")
    }

    fn track(&self, id: Uuid) -> (Point, Point) {
        let t = self.board().segments.iter().find(|t| t.uuid == id);
        let t = t.expect("the track");
        (t.start, t.end)
    }

    fn live(&self) -> String {
        live(&self.app, &self.path)
    }

    fn undo(&mut self) {
        let _ = self.app.update(Message::Edit(EditMsg::Undo));
    }
}

fn live(app: &Signex, path: &Path) -> String {
    serde_json::to_string(app.document_state.pcb_board(path).expect("board")).unwrap()
}

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// With nothing selected, a press on the track and a drag move it; the
/// track from U1's pad stretches by its joined end and keeps its pad end,
/// the track to the via keeps its via end. The drop is what the preview
/// drew, and one undo puts everything back.
#[test]
fn an_unselected_track_drags_and_stretches_the_tracks_joined_to_it() {
    let mut r = rig();
    let before = r.live();
    assert!(
        r.app.interaction_state.pcb_selection.selection().is_empty(),
        "nothing is selected"
    );
    r.press(p(7.5, 0.0));
    r.move_to(p(7.5, 3.0));
    let ghost: Vec<Segment> = {
        let carry = r.app.interaction_state.pcb_carry.as_ref().expect("a drag");
        assert!(carry.items.contains(&DRAGGED), "the press carries the track");
        let canvas = &r.app.interaction_state.pcb_canvas;
        assert!(canvas.previews_drag(), "the drag previews on the board");
        let following = canvas.drag_preview_following().to_vec();
        carried_board(r.board(), &carry.carried(), &following, carry.delta).segments
    };
    r.release(p(7.5, 3.0));
    assert_eq!(r.track(DRAGGED), (p(5.0, 3.0), p(10.0, 3.0)), "moved whole");
    assert_eq!(r.track(ON_PAD), (p(0.0, 0.0), p(5.0, 3.0)), "pad end stays");
    assert_eq!(r.track(TO_VIA), (p(10.0, 3.0), p(15.0, 0.0)), "via end stays");
    assert_eq!(r.track(FROM_VIA), (p(15.0, 0.0), p(20.0, 0.0)), "not joined");
    let via = r.board().vias.iter().find(|v| v.uuid == VIA).unwrap();
    assert_eq!(via.position, p(15.0, 0.0), "the via stays");
    let ids: Vec<Uuid> = ghost.iter().map(|t| t.uuid).collect();
    assert_eq!(ghost.len(), 3, "the ghost holds the track and both joined: {ids:?}");
    for drawn in &ghost {
        assert_eq!(
            (drawn.start, drawn.end),
            r.track(drawn.uuid),
            "the drop is the ghost"
        );
    }
    r.undo();
    assert_eq!(r.live(), before, "one undo step");
}

/// With nothing selected, a press on the via and a drag move it with both
/// track ends on it; their far ends stay. One undo step.
#[test]
fn an_unselected_via_drags_and_its_tracks_stretch() {
    let mut r = rig();
    let before = r.live();
    r.drag(p(15.0, 0.0), p(15.0, 3.0));
    let via = r.board().vias.iter().find(|v| v.uuid == VIA).unwrap();
    assert_eq!(via.position, p(15.0, 3.0), "the via moved");
    assert_eq!(r.track(TO_VIA), (p(10.0, 0.0), p(15.0, 3.0)));
    assert_eq!(r.track(FROM_VIA), (p(15.0, 3.0), p(20.0, 0.0)));
    assert_eq!(r.track(DRAGGED), (p(5.0, 0.0), p(10.0, 0.0)), "not on the via");
    r.undo();
    assert_eq!(r.live(), before, "one undo step");
}

/// A locked track does not move, and the Messages line says it is locked.
#[test]
fn a_locked_track_stays_and_says_why() {
    let mut r = rig();
    let before = r.live();
    let needle = format!("track {LOCKED} is locked");
    let said = crate::diagnostics::logged_count(&needle);
    r.drag(p(32.5, 0.0), p(32.5, 3.0));
    assert_eq!(r.live(), before, "the locked track stays");
    assert!(
        crate::diagnostics::logged_count(&needle) > said,
        "the refusal names the locked track"
    );
}

/// A drag of a selection holding two joined tracks moves both whole, not
/// also by the end they share, as before #2513; the tracks outside the
/// selection joined to them stretch. One undo step.
#[test]
fn selected_tracks_still_move_whole() {
    let mut r = rig();
    let before = r.live();
    r.click(p(7.5, 0.0), pcb_geom::ClickMode::Replace);
    r.click(p(12.5, 0.0), pcb_geom::ClickMode::Toggle);
    let selection = r.app.interaction_state.pcb_selection.selection().clone();
    assert!(selection.contains(pcb_geom::ItemRef::Track(DRAGGED)));
    assert!(selection.contains(pcb_geom::ItemRef::Track(TO_VIA)));
    r.drag(p(7.5, 0.0), p(7.5, 3.0));
    assert_eq!(r.track(DRAGGED), (p(5.0, 3.0), p(10.0, 3.0)), "whole");
    assert_eq!(r.track(TO_VIA), (p(10.0, 3.0), p(15.0, 3.0)), "whole");
    assert_eq!(r.track(ON_PAD), (p(0.0, 0.0), p(5.0, 3.0)), "stretched");
    assert_eq!(r.track(FROM_VIA), (p(15.0, 3.0), p(20.0, 0.0)), "stretched");
    r.undo();
    assert_eq!(r.live(), before, "one undo step");
}
