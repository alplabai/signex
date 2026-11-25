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
use crate::pcb_canvas::{PcbCanvasMsg, PcbCanvasState, carried_board, with_stretched_tracks};

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

fn near(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6
}

impl Rig {
    /// How many tracks of the board run from `a` to `b`, either way round.
    fn tracks_between(&self, a: Point, b: Point) -> usize {
        (self.board().segments.iter())
            .filter(|t| {
                (near(t.start, a) && near(t.end, b)) || (near(t.start, b) && near(t.end, a))
            })
            .count()
    }

    fn assert_all_at_legal_angles(&self) {
        for t in &self.board().segments {
            let (dx, dy) = ((t.end.x - t.start.x).abs(), (t.end.y - t.start.y).abs());
            let legal = dx < 1e-6 || dy < 1e-6 || (dx - dy).abs() < 1e-6;
            assert!(legal, "{} runs at 0°, 45° or 90°: {t:?}", t.uuid);
        }
    }
}

/// With nothing selected, a press on the track and a drag move it. The
/// track from U1's pad stretches by its joined end and keeps its pad end,
/// the track to the via keeps its via end, and each bends at one 45° corner
/// (Caner, 2026-11-25): the track keeps the leg at its fixed end and a new
/// track runs on to the moved end. The drop is what the preview drew, and
/// one undo puts everything back.
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
        let drawn = carried_board(r.board(), &carry.carried(), &following, carry.delta);
        with_stretched_tracks(drawn, r.board(), carry, &following).segments
    };
    r.release(p(7.5, 3.0));
    assert_eq!(r.track(DRAGGED), (p(5.0, 3.0), p(10.0, 3.0)), "moved whole");
    // (0, 0) to (5, 3): straight to (2, 0), then 45° up.
    assert_eq!(r.track(ON_PAD), (p(0.0, 0.0), p(2.0, 0.0)), "pad end stays");
    assert_eq!(r.tracks_between(p(2.0, 0.0), p(5.0, 3.0)), 1, "its 45° leg");
    // (15, 0) to (10, 3): straight to (13, 0), then 45° up.
    assert_eq!(r.track(TO_VIA), (p(13.0, 0.0), p(15.0, 0.0)), "via end stays");
    assert_eq!(r.tracks_between(p(10.0, 3.0), p(13.0, 0.0)), 1, "its 45° leg");
    assert_eq!(r.track(FROM_VIA), (p(15.0, 0.0), p(20.0, 0.0)), "not joined");
    assert_eq!(r.board().segments.len(), 7, "two new legs");
    r.assert_all_at_legal_angles();
    let via = r.board().vias.iter().find(|v| v.uuid == VIA).unwrap();
    assert_eq!(via.position, p(15.0, 0.0), "the via stays");
    let ids: Vec<Uuid> = ghost.iter().map(|t| t.uuid).collect();
    assert_eq!(ghost.len(), 5, "the track, both joined and their legs: {ids:?}");
    for drawn in &ghost {
        assert_eq!(
            r.tracks_between(drawn.start, drawn.end),
            1,
            "the drop is the ghost: {drawn:?}"
        );
    }
    r.undo();
    assert_eq!(r.live(), before, "one undo step");
}

/// A stretch that lands on an axis or an exact 45° line stays one track
/// (Caner, 2026-11-25). One undo step each.
#[test]
fn a_stretch_on_a_legal_angle_stays_one_track() {
    let mut r = rig();
    let before = r.live();
    // Along the track: the joined tracks shorten and lengthen on the axis.
    r.drag(p(7.5, 0.0), p(5.5, 0.0));
    assert_eq!(r.track(DRAGGED), (p(3.0, 0.0), p(8.0, 0.0)));
    assert_eq!(r.track(ON_PAD), (p(0.0, 0.0), p(3.0, 0.0)));
    assert_eq!(r.track(TO_VIA), (p(8.0, 0.0), p(15.0, 0.0)));
    assert_eq!(r.board().segments.len(), 5, "no corner");
    r.undo();
    assert_eq!(r.live(), before, "one undo step");
    // 5 mm up: both joined tracks run at exactly 45°.
    r.drag(p(7.5, 0.0), p(7.5, 5.0));
    assert_eq!(r.track(ON_PAD), (p(0.0, 0.0), p(5.0, 5.0)));
    assert_eq!(r.track(TO_VIA), (p(10.0, 5.0), p(15.0, 0.0)));
    assert_eq!(r.board().segments.len(), 5, "no corner");
    r.assert_all_at_legal_angles();
    r.undo();
    assert_eq!(r.live(), before, "one undo step");
}

/// With nothing selected, a press on the via and a drag move it with both
/// track ends on it; their far ends stay, and each bends at one 45° corner,
/// as a track stretched by a dragged track does (Caner, 2026-11-25). One
/// undo step.
#[test]
fn an_unselected_via_drags_and_its_tracks_stretch() {
    let mut r = rig();
    let before = r.live();
    r.drag(p(15.0, 0.0), p(15.0, 3.0));
    let via = r.board().vias.iter().find(|v| v.uuid == VIA).unwrap();
    assert_eq!(via.position, p(15.0, 3.0), "the via moved");
    // (10, 0) to the via at (15, 3): straight to (12, 0), then 45°.
    assert_eq!(r.track(TO_VIA), (p(10.0, 0.0), p(12.0, 0.0)));
    assert_eq!(r.tracks_between(p(12.0, 0.0), p(15.0, 3.0)), 1, "its leg");
    // (20, 0) to the via: straight to (18, 0), then 45°.
    assert_eq!(r.track(FROM_VIA), (p(18.0, 0.0), p(20.0, 0.0)));
    assert_eq!(r.tracks_between(p(15.0, 3.0), p(18.0, 0.0)), 1, "its leg");
    assert_eq!(r.track(DRAGGED), (p(5.0, 0.0), p(10.0, 0.0)), "not on the via");
    r.assert_all_at_legal_angles();
    r.undo();
    assert_eq!(r.live(), before, "one undo step");
}

/// With nothing selected, a press on U1's pad and a drag move the part; the
/// track on its pad keeps its far end and bends at one 45° corner (Caner,
/// 2026-11-25). One undo step.
#[test]
fn an_unselected_part_drags_and_the_track_on_its_pad_bends_at_45_degrees() {
    let mut r = rig();
    let before = r.live();
    // On the pad, clear of the track along y = 0.
    r.drag(p(0.0, -0.4), p(0.0, 2.6));
    let u1 = r.board().footprints.iter().find(|fp| fp.uuid == U1);
    assert_eq!(u1.expect("U1").position, p(0.0, 3.0), "the part moved");
    // (5, 0) to the pad at (0, 3): straight to (3, 0), then 45°.
    assert_eq!(r.track(ON_PAD), (p(3.0, 0.0), p(5.0, 0.0)), "far end stays");
    assert_eq!(r.tracks_between(p(0.0, 3.0), p(3.0, 0.0)), 1, "its leg");
    assert_eq!(r.track(DRAGGED), (p(5.0, 0.0), p(10.0, 0.0)), "not on the pad");
    r.assert_all_at_legal_angles();
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
/// also by the end they share, as before #2513; the unselected tracks
/// joined to them stretch with a 45° corner (Caner, 2026-11-25). One undo
/// step.
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
    assert_eq!(r.track(ON_PAD), (p(0.0, 0.0), p(2.0, 0.0)), "stretched");
    assert_eq!(r.tracks_between(p(2.0, 0.0), p(5.0, 3.0)), 1, "its leg");
    // (20, 0) to (15, 3): straight to (18, 0), then 45° up.
    assert_eq!(r.track(FROM_VIA), (p(18.0, 0.0), p(20.0, 0.0)), "stretched");
    assert_eq!(r.tracks_between(p(15.0, 3.0), p(18.0, 0.0)), 1, "its leg");
    r.assert_all_at_legal_angles();
    r.undo();
    assert_eq!(r.live(), before, "one undo step");
}
