//! A via dragged with the Select tool, driven from real pointer events on
//! the board canvas into `Signex::update`.

use std::path::{Path, PathBuf};

use iced::keyboard::Modifiers;
use iced::widget::canvas;
use iced::{Event, Rectangle, mouse};
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::pcb_canvas::{
    PcbCanvasState, board_without, carried_board, followed_arcs, with_stretched_tracks,
};

const U1: Uuid = Uuid::from_u128(0x10);
const FREE: Uuid = Uuid::from_u128(0x40);
const LOCKED: Uuid = Uuid::from_u128(0x41);
const LONE: Uuid = Uuid::from_u128(0x42);
const TRACK: Uuid = Uuid::from_u128(0x50);
const OTHER_NET: Uuid = Uuid::from_u128(0x51);
const LOCKED_TRACK: Uuid = Uuid::from_u128(0x52);
const ON_PAD: Uuid = Uuid::from_u128(0x43);
const ARC: Uuid = Uuid::from_u128(0x53);
const BOUNDS: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 1600.0,
    height: 1200.0,
};

/// U1 at (10, 10); a free via at (20, 10) with a track ending on it; a locked
/// via at (20, 20). All on a 0.5 mm grid.
fn board() -> PcbBoard {
    let via = |id: Uuid, y: f64, locked: bool| {
        serde_json::json!({
            "uuid": id, "position": {"x": 20.0, "y": y}, "diameter": 0.8, "drill": 0.4,
            "layers": ["TopCopper", "BottomCopper"], "net": 1, "locked": locked
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 0.5},
        "footprints": [{
            "uuid": U1, "reference": "U1", "position": {"x": 10.0, "y": 10.0},
            "rotation": 0.0, "layer": "TopCopper", "locked": false,
            "pads": [{"uuid": Uuid::from_u128(0x11), "number": "1", "pad_type": "smd",
                      "shape": "rect", "position": {"x": 0.0, "y": 0.0},
                      "size": {"x": 1.0, "y": 1.0}, "drill": null,
                      "layers": ["TopCopper"]}]
        }],
        "vias": [via(FREE, 10.0, false), via(LOCKED, 20.0, true)],
        "segments": [{"uuid": TRACK, "start": {"x": 15.0, "y": 10.0},
                      "end": {"x": 20.0, "y": 10.0}, "width": 0.25,
                      "layer": "TopCopper", "net": 1}]
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
    rig_on(board())
}

fn rig_on(board: PcbBoard) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    let path = dir.path().join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board);
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
    /// The canvas pixel of a board point.
    fn pixel(&self, at: Point) -> iced::Point {
        self.app
            .interaction_state
            .pcb_canvas
            .camera_ref()
            .world_to_screen(iced::Point::new(at.x as f32, at.y as f32))
    }

    /// One pointer event through the canvas, its message through the app.
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
        let moved = mouse::Event::CursorMoved { position: pixel };
        self.send(Event::Mouse(moved), pixel);
        let event = mouse::Event::ButtonPressed(mouse::Button::Left);
        self.send(Event::Mouse(event), pixel);
    }

    fn move_to(&mut self, at: Point) {
        let pixel = self.pixel(at);
        let event = mouse::Event::CursorMoved { position: pixel };
        self.send(Event::Mouse(event), pixel);
    }

    fn release(&mut self, at: Point) {
        let pixel = self.pixel(at);
        let event = mouse::Event::ButtonReleased(mouse::Button::Left);
        self.send(Event::Mouse(event), pixel);
    }

    fn drag(&mut self, from: Point, to: Point) {
        self.press(from);
        self.move_to(to);
        self.release(to);
    }

    fn board(&self) -> &PcbBoard {
        self.app
            .document_state
            .pcb_board(&self.path)
            .expect("board")
    }

    fn via(&self, id: Uuid) -> &signex_types::pcb::Via {
        self.board().vias.iter().find(|v| v.uuid == id).unwrap()
    }

    fn live(&self) -> String {
        live(&self.app, &self.path)
    }
}

fn live(app: &Signex, path: &Path) -> String {
    serde_json::to_string(app.document_state.pcb_board(path).expect("board")).unwrap()
}

fn close(a: Point, x: f64, y: f64) -> bool {
    (a.x - x).abs() < 1e-6 && (a.y - y).abs() < 1e-6
}

#[test]
fn a_dragged_via_lands_on_the_grid_keeps_its_net_and_undoes_in_one_step() {
    let mut rig = rig();
    let before = rig.live();
    rig.press(Point::new(20.0, 10.0));
    rig.move_to(Point::new(24.1, 13.2));
    let carry = rig.app.interaction_state.pcb_carry.as_ref().expect("carry");
    assert!(carry.vias.contains(&FREE), "the via is carried");
    rig.assert_via_moves_itself(Point::new(20.0, 10.0), Point::new(24.0, 13.0));
    assert_eq!(rig.live(), before, "nothing moves before the release");
    rig.release(Point::new(24.1, 13.2));

    let via = rig.via(FREE);
    assert!(close(via.position, 24.0, 13.0), "{:?}", via.position);
    assert_eq!(via.net, 1, "the net is unchanged");
    // Caner, 2026-11-25 (#2513): the track to the moved via bends at one 45°
    // corner, straight from (15, 10) to (21, 10), then a new leg to (24, 13).
    let track = &rig.board().segments[0];
    assert!(
        close(track.end, 21.0, 10.0),
        "it keeps its fixed leg: {track:?}"
    );
    assert!(
        close(track.start, 15.0, 10.0),
        "its far end stays: {track:?}"
    );
    assert!(
        rig.has_track((21.0, 10.0), (24.0, 13.0)),
        "the new 45° leg ends on the via"
    );

    let _ = rig.app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(rig.live(), before, "one undo restores via and track");
}

/// `board()` with tracks ending on the free via that must not follow it (one
/// of another net, one locked), and a via at (30, 10) with no track.
fn board_with_other_tracks() -> PcbBoard {
    let mut board = board();
    let mut other_net = board.segments[0].clone();
    other_net.uuid = OTHER_NET;
    other_net.start = Point::new(20.0, 5.0);
    other_net.net = 2;
    let mut locked = board.segments[0].clone();
    locked.uuid = LOCKED_TRACK;
    locked.start = Point::new(20.0, 15.0);
    locked.locked = true;
    board.segments.extend([other_net, locked]);
    let mut lone = board.vias[0].clone();
    lone.uuid = LONE;
    lone.position = Point::new(30.0, 10.0);
    board.vias.push(lone);
    board
}

impl Rig {
    fn track(&self, id: Uuid) -> &signex_types::pcb::Segment {
        let segments = &self.board().segments;
        segments.iter().find(|s| s.uuid == id).unwrap()
    }

    /// Whether a track of the board runs from `a` to `b`.
    fn has_track(&self, a: (f64, f64), b: (f64, f64)) -> bool {
        (self.board().segments.iter()).any(|s| close(s.start, a.0, a.1) && close(s.end, b.0, b.1))
    }

    /// Whether a track of the board ends at `at`, its stretched end (#2513).
    fn a_track_ends_at(&self, at: Point) -> bool {
        (self.board().segments.iter()).any(|s| close(s.end, at.x, at.y))
    }

    /// `modifiers` pressed or released with the pointer at `at`.
    fn hold(&mut self, modifiers: Modifiers, at: Point) {
        let pixel = self.pixel(at);
        let event = Event::Keyboard(iced::keyboard::Event::ModifiersChanged(modifiers));
        self.send(event, pixel);
    }

    /// Mid-drag the via is drawn at `to` in its real look: no ghost, and
    /// nothing at `from`.
    fn assert_via_moves_itself(&self, from: Point, to: Point) {
        let near = |c: [f32; 2], p: Point| {
            (f64::from(c[0]) - p.x).abs() < 1e-3 && (f64::from(c[1]) - p.y).abs() < 1e-3
        };
        let canvas = &self.app.interaction_state.pcb_canvas;
        assert!(canvas.highlight().ghost.is_empty(), "no ghost");
        let base = canvas.active_renderer_snapshot().expect("a board");
        assert!(
            !base.vias.iter().any(|via| near(via.center, from)),
            "nothing is left at the old place"
        );
        let moving = canvas.drag_preview_scene().expect("a moving scene");
        assert!(
            moving.circles.iter().any(|c| near(c.center, to)),
            "the via is drawn at the new place"
        );
    }
}

#[test]
fn a_track_of_another_net_or_a_locked_track_does_not_follow_the_via() {
    let mut rig = rig_on(board_with_other_tracks());
    rig.drag(Point::new(20.0, 10.0), Point::new(24.0, 13.0));
    // Its own track bends at one 45° corner onto the via (Caner, 2026-11-25).
    assert!(close(rig.track(TRACK).end, 21.0, 10.0), "its own track");
    assert!(rig.has_track((21.0, 10.0), (24.0, 13.0)), "and its new leg");
    assert!(close(rig.track(OTHER_NET).end, 20.0, 10.0), "another net");
    assert!(close(rig.track(LOCKED_TRACK).end, 20.0, 10.0), "locked");
}

#[test]
fn a_ctrl_drag_moves_the_via_alone_until_the_drop() {
    let mut rig = rig();
    let before = rig.live();
    let to = Point::new(24.0, 13.0);
    rig.hold(Modifiers::CTRL, Point::new(20.0, 10.0));
    rig.press(Point::new(20.0, 10.0));
    rig.move_to(to);
    rig.assert_via_moves_itself(Point::new(20.0, 10.0), to);
    assert!(
        rig.app
            .interaction_state
            .pcb_canvas
            .drag_preview_following()
            .is_empty(),
        "with Ctrl no track is carried"
    );
    rig.hold(Modifiers::empty(), to);
    assert!(
        rig.app
            .interaction_state
            .pcb_canvas
            .drag_preview_following()
            .is_empty(),
        "letting Ctrl go does not reattach the track"
    );
    rig.move_to(to);
    rig.release(to);
    assert!(close(rig.via(FREE).position, 24.0, 13.0));
    assert!(close(rig.track(TRACK).end, 20.0, 10.0), "the track stays");
    let _ = rig.app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(rig.live(), before, "one undo restores the via");
}

#[test]
fn the_preview_draws_the_via_and_its_track_where_they_are_going() {
    let mut rig = rig();
    let before = rig.board().clone();
    let saved = rig.live();
    rig.press(Point::new(20.0, 10.0));
    rig.move_to(Point::new(24.0, 13.0));
    rig.assert_via_moves_itself(Point::new(20.0, 10.0), Point::new(24.0, 13.0));
    let canvas = &rig.app.interaction_state.pcb_canvas;
    let following = canvas.drag_preview_following().to_vec();
    assert_eq!(following.len(), 1);
    assert_eq!(following[0].track, TRACK);
    let carry = rig.app.interaction_state.pcb_carry.as_ref().expect("carry");
    // What the preview draws: the moved via and its track bent at one 45°
    // corner, as the drop bends it (Caner, 2026-11-25, #2513).
    let drawn = carried_board(&before, &carry.carried(), &following, carry.delta);
    let drawn = with_stretched_tracks(drawn, &before, carry, &following);
    let runs = |a: (f64, f64), b: (f64, f64)| {
        (drawn.segments.iter()).any(|s| close(s.start, a.0, a.1) && close(s.end, b.0, b.1))
    };
    assert_eq!(drawn.segments.len(), 2, "the track and its new leg");
    assert!(
        runs((15.0, 10.0), (21.0, 10.0)),
        "the track keeps its far end"
    );
    assert!(
        runs((21.0, 10.0), (24.0, 13.0)),
        "the moved track ends on the via"
    );
    assert!(close(drawn.vias[0].position, 24.0, 13.0), "the moved via");
    let left = board_without(&before, &carry.carried(), &following);
    assert!(left.segments.is_empty(), "the track left its old place");
    assert!(left.vias.iter().all(|via| via.uuid != FREE));
    assert_eq!(rig.live(), saved, "nothing moves before the release");
}

#[test]
fn a_via_with_no_track_moves_itself_with_no_ghost() {
    let mut rig = rig_on(board_with_other_tracks());
    let tracks = rig.board().segments.clone();
    rig.press(Point::new(30.0, 10.0));
    rig.move_to(Point::new(33.0, 12.0));
    rig.assert_via_moves_itself(Point::new(30.0, 10.0), Point::new(33.0, 12.0));
    rig.release(Point::new(33.0, 12.0));
    assert!(close(rig.via(LONE).position, 33.0, 12.0));
    assert_eq!(rig.board().segments, tracks);
}

/// `board()` with a via at (10.2, 10.2), on U1's pad.
fn board_with_via_on_u1(locked: bool) -> PcbBoard {
    let mut board = board();
    let mut on_pad = board.vias[0].clone();
    on_pad.uuid = ON_PAD;
    on_pad.position = Point::new(10.2, 10.2);
    on_pad.locked = locked;
    board.vias.push(on_pad);
    board
}

impl Rig {
    fn u1_at(&self) -> Point {
        let footprints = &self.board().footprints;
        footprints.iter().find(|f| f.uuid == U1).unwrap().position
    }
}

/// #2358: a press on an unselected footprint drags the footprint, even with
/// a via, locked or not, under the pointer.
#[test]
fn a_press_on_a_via_inside_an_unselected_footprint_drags_the_footprint() {
    for locked in [false, true] {
        let mut rig = rig_on(board_with_via_on_u1(locked));
        rig.drag(Point::new(10.2, 10.2), Point::new(12.2, 11.2));
        let at = rig.u1_at();
        assert!(close(at, 12.0, 11.0), "locked {locked}: {at:?}");
        let via = rig.via(ON_PAD).position;
        assert!(close(via, 10.2, 10.2), "locked {locked}: {via:?}");
    }
}

#[test]
fn a_via_inside_a_footprint_drags_once_a_click_has_picked_it() {
    let mut rig = rig_on(board_with_via_on_u1(false));
    let on_via = Point::new(10.2, 10.2);
    rig.press(on_via);
    rig.release(on_via);
    let selected: Vec<_> = rig
        .app
        .interaction_state
        .pcb_selection
        .selection()
        .items()
        .collect();
    assert_eq!(selected, [pcb_geom::ItemRef::Via(ON_PAD)]);
    rig.drag(on_via, Point::new(12.2, 11.2));
    let via = rig.via(ON_PAD).position;
    assert!(close(via, 12.0, 11.0), "{via:?}");
    assert!(close(rig.u1_at(), 10.0, 10.0), "the footprint stays");
}

/// `board()` with an arc from the free via over (22, 12) to (24, 10) in
/// place of its track.
fn board_with_arc() -> PcbBoard {
    let mut board = board();
    board.segments.clear();
    board.free.arcs.push(signex_types::pcb_arc::TrackArc {
        uuid: ARC,
        start: Point::new(20.0, 10.0),
        mid: Point::new(22.0, 12.0),
        end: Point::new(24.0, 10.0),
        width: 0.25,
        layer: "TopCopper".into(),
        net: 1,
    });
    board
}

#[test]
fn a_copper_arc_follows_the_via_in_the_preview_and_in_one_undo_step() {
    let mut rig = rig_on(board_with_arc());
    let before = rig.live();
    let sweep = rig.board().free.arcs[0].sweep().expect("an arc").sweep;
    let to = Point::new(20.0, 12.0);
    rig.press(Point::new(20.0, 10.0));
    rig.move_to(to);
    let canvas = &rig.app.interaction_state.pcb_canvas;
    assert!(canvas.previews_drag(), "the arc is carried");
    let carry = rig.app.interaction_state.pcb_carry.as_ref().expect("carry");
    let board = rig.board();
    let drawn = followed_arcs(board, &carry.following_arcs(board), carry.delta);
    assert!(close(drawn[0].start, 20.0, 12.0), "{:?}", drawn[0]);
    rig.release(to);

    let arc = &rig.board().free.arcs[0];
    assert!(close(arc.start, 20.0, 12.0), "{arc:?}");
    assert!(close(arc.end, 24.0, 10.0), "its far end stays: {arc:?}");
    let after = arc.sweep().expect("still an arc").sweep;
    assert!((after - sweep).abs() < 1e-9, "{sweep} {after}");
    let _ = rig.app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(rig.live(), before, "one undo restores via and arc");
}

/// A via 0.3 mm outside U1's pad, at 10 px/mm, is within the 4 px hover
/// reach of the pad but not on it: the press drags the via.
#[test]
fn a_via_just_outside_a_part_drags_itself_and_the_part_stays() {
    let mut board = board();
    let mut near = board.vias[0].clone();
    near.uuid = ON_PAD;
    near.position = Point::new(10.8, 10.0);
    board.vias.push(near);
    let mut rig = rig_on(board);
    let canvas = &rig.app.interaction_state.pcb_canvas;
    let factor = 10.0 / canvas.camera_ref().scale();
    canvas.zoom_by_factor_about_centre(factor);
    canvas.center_on_world(10.8, 10.0);
    assert!((canvas.camera_ref().scale() - 10.0).abs() < 1e-3);
    rig.drag(Point::new(10.8, 10.0), Point::new(12.8, 11.0));
    let via = rig.via(ON_PAD).position;
    assert!(close(via, 13.0, 11.0), "{via:?}");
    assert!(close(rig.u1_at(), 10.0, 10.0), "the part stays");
}

/// The drag of the via at (20, 10) with `board_with_arc`'s 4 mm chord to
/// `to`: the preview's offset, whether the board changed, and the rig.
fn drag_arc_via_to(to: Point) -> (Point, bool, Rig) {
    let mut rig = rig_on(board_with_arc());
    let before = rig.live();
    rig.press(Point::new(20.0, 10.0));
    rig.move_to(to);
    let carry = rig.app.interaction_state.pcb_carry.as_ref().expect("carry");
    let shown = carry.delta;
    rig.release(to);
    let changed = rig.live() != before;
    (shown, changed, rig)
}

#[test]
fn a_via_dropped_on_its_arcs_far_end_is_refused_and_the_board_stays() {
    let (shown, changed, _rig) = drag_arc_via_to(Point::new(24.0, 10.0));
    assert!(!changed, "nothing moved");
    assert!(close(shown, 0.0, 0.0), "the preview stays put: {shown:?}");
}

#[test]
fn an_arc_growing_past_three_times_is_refused_and_below_it_follows() {
    // 12.5 mm from the far end: 3.125 times the 4 mm chord.
    let (shown, changed, _rig) = drag_arc_via_to(Point::new(11.5, 10.0));
    assert!(!changed, "refused");
    assert!(close(shown, 0.0, 0.0), "{shown:?}");
    // 11.5 mm: 2.875 times.
    let (shown, changed, rig) = drag_arc_via_to(Point::new(12.5, 10.0));
    assert!(changed, "allowed");
    assert!(close(shown, -7.5, 0.0), "{shown:?}");
    let arc = &rig.board().free.arcs[0];
    assert!(close(arc.start, 12.5, 10.0), "{arc:?}");
    assert!(close(arc.end, 24.0, 10.0), "{arc:?}");
}

#[test]
fn a_ctrl_drag_leaves_a_copper_arc() {
    let mut rig = rig_on(board_with_arc());
    let arc = rig.board().free.arcs[0].clone();
    rig.hold(Modifiers::CTRL, Point::new(20.0, 10.0));
    rig.drag(Point::new(20.0, 10.0), Point::new(20.0, 12.0));
    assert!(close(rig.via(FREE).position, 20.0, 12.0));
    assert_eq!(rig.board().free.arcs[0], arc);
}

/// Caner, 2026-11-21: Ctrl down at the press or mid-drag leaves the track
/// until the drop, even once released; after an undo, a plain drag carries
/// it again. `at` reads the object, `track` ends on it at `from`.
fn assert_ctrl_latches(board: PcbBoard, track: Uuid, from: Point, at: fn(&Rig) -> Point) {
    let to = Point::new(from.x + 4.0, from.y + 3.0);
    let beyond = Point::new(from.x + 5.0, from.y + 3.0);
    for ctrl_at_press in [true, false] {
        let mut rig = rig_on(board.clone());
        if ctrl_at_press {
            rig.hold(Modifiers::CTRL, from);
        }
        rig.press(from);
        rig.move_to(to);
        if !ctrl_at_press {
            rig.hold(Modifiers::CTRL, to);
        }
        rig.hold(Modifiers::empty(), to);
        rig.move_to(beyond);
        assert!(
            rig.app
                .interaction_state
                .pcb_canvas
                .drag_preview_following()
                .is_empty(),
            "the preview keeps the track behind"
        );
        rig.release(beyond);
        assert!(!close(at(&rig), from.x, from.y), "the object moved");
        assert!(close(rig.track(track).end, from.x, from.y), "track stays");

        let _ = rig.app.update(Message::Edit(EditMsg::Undo));
        rig.drag(from, to);
        let moved = at(&rig);
        assert!(!close(moved, from.x, from.y), "the plain drag moved it");
        // The track bends at one 45° corner (Caner, 2026-11-25): its new leg
        // ends where the object went.
        assert!(rig.a_track_ends_at(moved), "it follows");
    }
}

#[test]
fn ctrl_latches_for_a_via_drag() {
    assert_ctrl_latches(board(), TRACK, Point::new(20.0, 10.0), |rig| {
        rig.via(FREE).position
    });
}

#[test]
fn ctrl_latches_for_a_footprint_drag() {
    const PAD_TRACK: Uuid = Uuid::from_u128(0x54);
    let mut with_track = board();
    let mut on_pad = with_track.segments[0].clone();
    on_pad.uuid = PAD_TRACK;
    on_pad.start = Point::new(5.0, 10.0);
    on_pad.end = Point::new(10.0, 10.0);
    on_pad.net = 0;
    with_track.segments.push(on_pad);
    assert_ctrl_latches(with_track, PAD_TRACK, Point::new(10.0, 10.0), Rig::u1_at);
}

#[test]
fn ctrl_released_mid_drag_leaves_a_copper_arc() {
    let mut rig = rig_on(board_with_arc());
    let arc = rig.board().free.arcs[0].clone();
    let (from, to) = (Point::new(20.0, 10.0), Point::new(20.0, 12.0));
    rig.hold(Modifiers::CTRL, from);
    rig.press(from);
    rig.move_to(to);
    rig.hold(Modifiers::empty(), to);
    rig.release(to);
    assert!(close(rig.via(FREE).position, 20.0, 12.0));
    assert_eq!(rig.board().free.arcs[0], arc);
}

/// The via dragged to just off its own track, with Track Vertices and Track
/// Lines on and the grid off.
fn drag_along_own_track(ctrl: bool) -> Point {
    use signex_engine::pcb::snap::SnapObject;
    let mut rig = rig();
    rig.app.ui_state.pcb_snap.to_grids = false;
    let objects = &mut rig.app.ui_state.pcb_snap.objects;
    objects.extend([SnapObject::TrackVertices, SnapObject::TrackLines]);
    let from = Point::new(20.0, 10.0);
    if ctrl {
        rig.hold(Modifiers::CTRL, from);
    }
    rig.drag(from, Point::new(18.0, 10.05));
    rig.via(FREE).position
}

#[test]
fn a_via_does_not_snap_to_the_track_following_it() {
    let at = drag_along_own_track(false);
    assert!((at.y - 10.05).abs() < 1e-3, "off its own track: {at:?}");
}

#[test]
fn with_ctrl_the_track_stays_and_the_via_can_snap_to_it() {
    let at = drag_along_own_track(true);
    assert!((at.y - 10.0).abs() < 1e-6, "onto the track: {at:?}");
}

#[test]
fn a_via_pressed_off_centre_lands_with_its_centre_on_the_grid() {
    let mut rig = rig();
    rig.drag(Point::new(20.2, 10.1), Point::new(25.2, 12.1));
    assert!(
        close(rig.via(FREE).position, 25.0, 12.0),
        "{:?}",
        rig.via(FREE).position
    );
}

#[test]
fn with_grid_snap_off_a_via_moves_by_the_exact_drag() {
    let mut rig = rig();
    rig.app.ui_state.pcb_snap.to_grids = false;
    let (from, to) = (Point::new(20.0, 10.0), Point::new(23.0, 12.0));
    rig.drag(from, to);
    let at = rig.via(FREE).position;
    // The pointer goes through f32 pixels, so the drag is exact to a micron.
    assert!(
        (at.x - 23.0).abs() < 1e-3 && (at.y - 12.0).abs() < 1e-3,
        "{at:?}"
    );
}

#[test]
fn a_via_does_not_snap_to_its_own_centre() {
    let mut rig = rig();
    rig.app
        .ui_state
        .pcb_snap
        .objects
        .insert(signex_engine::pcb::snap::SnapObject::ViaCenters);
    rig.drag(Point::new(20.0, 10.0), Point::new(20.4, 10.0));
    assert!(
        !close(rig.via(FREE).position, 20.0, 10.0),
        "the via left its place: {:?}",
        rig.via(FREE).position
    );
}

#[test]
fn a_locked_via_stays_and_the_drag_carries_nothing() {
    let mut rig = rig();
    let before = rig.live();
    rig.press(Point::new(20.0, 20.0));
    rig.move_to(Point::new(25.0, 25.0));
    let carry = rig.app.interaction_state.pcb_carry.as_ref().expect("carry");
    assert!(carry.vias.is_empty(), "a refused drag carries nothing");
    let canvas = &rig.app.interaction_state.pcb_canvas;
    assert!(
        canvas.highlight().ghost.is_empty() && !canvas.previews_drag(),
        "and paints no preview"
    );
    rig.release(Point::new(25.0, 25.0));
    assert_eq!(rig.live(), before, "the locked via stays");
    assert!(rig.app.interaction_state.pcb_carry.is_none());
}

#[test]
fn a_click_on_a_via_only_selects_it() {
    let mut rig = rig();
    let before = rig.live();
    let on_via = Point::new(20.0, 10.0);
    rig.press(on_via);
    rig.release(on_via);
    assert_eq!(rig.live(), before);
    assert!(rig.app.interaction_state.pcb_carry.is_none());
    let selected: Vec<_> = rig
        .app
        .interaction_state
        .pcb_selection
        .selection()
        .items()
        .collect();
    assert_eq!(selected, [pcb_geom::ItemRef::Via(FREE)]);
}

#[test]
fn a_drag_from_empty_space_still_draws_a_box() {
    let mut rig = rig();
    let before = rig.live();
    rig.press(Point::new(40.0, 40.0));
    rig.move_to(Point::new(45.0, 45.0));
    assert!(rig.app.interaction_state.pcb_carry.is_none(), "no carry");
    rig.release(Point::new(45.0, 45.0));
    assert_eq!(rig.live(), before);
}

#[test]
fn a_footprint_drag_still_moves_the_footprint() {
    let mut rig = rig();
    rig.drag(Point::new(10.0, 10.0), Point::new(12.0, 11.0));
    let u1 = rig
        .board()
        .footprints
        .iter()
        .find(|f| f.uuid == U1)
        .unwrap();
    assert!(close(u1.position, 12.0, 11.0), "{:?}", u1.position);
    assert!(close(rig.via(FREE).position, 20.0, 10.0), "the via stays");
}

#[test]
fn turning_vias_off_in_the_filter_leaves_a_press_on_one_to_draw_a_box() {
    let mut rig = rig();
    rig.app
        .ui_state
        .pcb_pick_filter
        .set_on_kinds([pcb_geom::PickKind::Tracks]);
    rig.app.refresh_pcb_highlight();
    let on_via = Point::new(20.0, 20.0);
    assert!(!rig.app.interaction_state.pcb_canvas.grabs(on_via, 0.05));
}
