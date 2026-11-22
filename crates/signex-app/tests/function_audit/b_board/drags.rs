//! The 2026-11-21 decisions on dragging a part or a via: attached tracks
//! follow, Ctrl leaves them and latches for the rest of the drag, a drop is
//! one undo step, and a following arc that would collapse refuses the drop.

use signex_app::app::Message;
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::support::{Bench, assert_at, open};

const PART: Uuid = Uuid::from_u128(0xC01);
const PAD: Uuid = Uuid::from_u128(0xC02);
const VIA: Uuid = Uuid::from_u128(0xC03);
const LOCKED_VIA: Uuid = Uuid::from_u128(0xC04);
const ARC_VIA: Uuid = Uuid::from_u128(0xC05);
const TO_VIA: Uuid = Uuid::from_u128(0xC06);
const TO_PAD: Uuid = Uuid::from_u128(0xC07);
const ARC: Uuid = Uuid::from_u128(0xC08);

/// On a 1 mm grid: a free via at (20, 10) with a track ending on it; a part
/// at (40, 10) whose pad has a track ending on it; a locked via at (20, 20);
/// a via at (20, 30) with a copper arc from it to (24, 30).
fn drag_board() -> PcbBoard {
    let via = |id: Uuid, y: f64, locked: bool| {
        serde_json::json!({
            "uuid": id, "position": {"x": 20.0, "y": y}, "diameter": 0.8, "drill": 0.4,
            "layers": ["TopCopper", "BottomCopper"], "net": 1, "locked": locked
        })
    };
    let track = |id: Uuid, a: (f64, f64), z: (f64, f64)| {
        serde_json::json!({
            "uuid": id, "start": {"x": a.0, "y": a.1}, "end": {"x": z.0, "y": z.1},
            "width": 0.25, "layer": "TopCopper", "net": 1
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(0xC00),
        "setup": {"grid_size": 1.0},
        "nets": [{"number": 1, "name": "GND"}],
        "footprints": [{
            "uuid": PART, "reference": "U1", "position": {"x": 40.0, "y": 10.0},
            "rotation": 0.0, "layer": "TopCopper", "locked": false,
            "pads": [{"uuid": PAD, "number": "1", "pad_type": "smd", "shape": "rect",
                      "position": {"x": 0.0, "y": 0.0}, "size": {"x": 1.0, "y": 1.0},
                      "drill": null, "layers": ["TopCopper"],
                      "net": {"number": 1, "name": "GND"}}]
        }],
        "vias": [via(VIA, 10.0, false), via(LOCKED_VIA, 20.0, true), via(ARC_VIA, 30.0, false)],
        "segments": [track(TO_VIA, (15.0, 10.0), (20.0, 10.0)),
                     track(TO_PAD, (35.0, 10.0), (40.0, 10.0))],
        "free": {
            "arcs": [{"uuid": ARC, "start": {"x": 20.0, "y": 30.0},
                      "mid": {"x": 22.0, "y": 32.0}, "end": {"x": 24.0, "y": 30.0},
                      "width": 0.25, "layer": "TopCopper", "net": 1}]
        }
    }))
    .expect("the drag board")
}

fn drag(b: &mut Bench, from: Point, to: Point, leaves_tracks: bool) {
    b.send(Message::PcbCanvas(PcbCanvasMsg::FootprintDragged {
        from,
        to,
        tolerance_mm: 0.1,
        leaves_tracks,
    }));
}

fn drop_at(b: &mut Bench, from: Point, to: Point) {
    b.send(Message::PcbCanvas(PcbCanvasMsg::FootprintDropped {
        from,
        to,
    }));
}

fn via_at(board: &PcbBoard, id: Uuid) -> Point {
    board
        .vias
        .iter()
        .find(|via| via.uuid == id)
        .expect("the via")
        .position
}

fn track_end(board: &PcbBoard, id: Uuid) -> Point {
    board
        .segments
        .iter()
        .find(|track| track.uuid == id)
        .expect("the track")
        .end
}

fn part_at(board: &PcbBoard) -> Point {
    board.footprints[0].position
}

const VIA_FROM: Point = Point { x: 20.0, y: 10.0 };
const VIA_TO: Point = Point { x: 25.0, y: 12.0 };
const PART_FROM: Point = Point { x: 40.0, y: 10.0 };
const PART_TO: Point = Point { x: 44.0, y: 13.0 };

#[test]
fn a_dragged_via_takes_its_track_end_along_in_one_undo_step() {
    let mut b = open(drag_board());
    let edited = b.one_step(|b| {
        drag(b, VIA_FROM, VIA_TO, false);
        drop_at(b, VIA_FROM, VIA_TO);
    });
    assert_at(via_at(&edited, VIA), 25.0, 12.0);
    assert_at(track_end(&edited, TO_VIA), 25.0, 12.0);
}

#[test]
fn a_ctrl_drag_moves_the_via_alone() {
    let mut b = open(drag_board());
    let edited = b.one_step(|b| {
        drag(b, VIA_FROM, VIA_TO, true);
        drop_at(b, VIA_FROM, VIA_TO);
    });
    assert_at(via_at(&edited, VIA), 25.0, 12.0);
    assert_at(track_end(&edited, TO_VIA), 20.0, 10.0);
}

#[test]
fn ctrl_latches_for_the_rest_of_a_via_drag_even_after_release() {
    let mut b = open(drag_board());
    drag(&mut b, VIA_FROM, Point::new(22.0, 10.0), true);
    drag(&mut b, VIA_FROM, VIA_TO, false);
    drop_at(&mut b, VIA_FROM, VIA_TO);
    let board = b.board();
    assert_at(via_at(&board, VIA), 25.0, 12.0);
    assert_at(track_end(&board, TO_VIA), 20.0, 10.0);
}

#[test]
fn the_next_drag_after_a_ctrl_drag_starts_unlatched() {
    let mut b = open(drag_board());
    drag(&mut b, VIA_FROM, VIA_TO, true);
    drop_at(&mut b, VIA_FROM, VIA_TO);
    b.undo();
    drag(&mut b, VIA_FROM, VIA_TO, false);
    drop_at(&mut b, VIA_FROM, VIA_TO);
    assert_at(track_end(&b.board(), TO_VIA), 25.0, 12.0);
}

#[test]
fn a_locked_via_refuses_a_drag() {
    let mut b = open(drag_board());
    let from = Point::new(20.0, 20.0);
    let to = Point::new(26.0, 20.0);
    b.refused(|b| {
        drag(b, from, to, false);
        drop_at(b, from, to);
    });
}

#[test]
fn a_cancelled_via_drag_changes_nothing() {
    let mut b = open(drag_board());
    b.refused(|b| {
        drag(b, VIA_FROM, VIA_TO, false);
        b.send(Message::PcbCanvas(PcbCanvasMsg::FootprintDragCancelled));
    });
}

#[test]
fn a_drop_that_would_land_a_following_arc_on_its_fixed_end_is_refused() {
    let mut b = open(drag_board());
    let from = Point::new(20.0, 30.0);
    let to = Point::new(24.0, 30.0);
    b.refused(|b| {
        drag(b, from, to, false);
        drop_at(b, from, to);
    });
}

#[test]
fn a_dragged_part_takes_its_track_end_along_in_one_undo_step() {
    let mut b = open(drag_board());
    let edited = b.one_step(|b| {
        drag(b, PART_FROM, PART_TO, false);
        drop_at(b, PART_FROM, PART_TO);
    });
    assert_at(part_at(&edited), 44.0, 13.0);
    assert_at(track_end(&edited, TO_PAD), 44.0, 13.0);
}

#[test]
fn a_ctrl_drag_moves_the_part_alone() {
    let mut b = open(drag_board());
    let edited = b.one_step(|b| {
        drag(b, PART_FROM, PART_TO, true);
        drop_at(b, PART_FROM, PART_TO);
    });
    assert_at(part_at(&edited), 44.0, 13.0);
    assert_at(track_end(&edited, TO_PAD), 40.0, 10.0);
}

#[test]
fn ctrl_latches_for_the_rest_of_a_part_drag_even_after_release() {
    let mut b = open(drag_board());
    drag(&mut b, PART_FROM, Point::new(42.0, 10.0), true);
    drag(&mut b, PART_FROM, PART_TO, false);
    drop_at(&mut b, PART_FROM, PART_TO);
    let board = b.board();
    assert_at(part_at(&board), 44.0, 13.0);
    assert_at(track_end(&board, TO_PAD), 40.0, 10.0);
}

#[test]
fn a_cancelled_part_drag_changes_nothing() {
    let mut b = open(drag_board());
    b.refused(|b| {
        drag(b, PART_FROM, PART_TO, false);
        b.send(Message::PcbCanvas(PcbCanvasMsg::FootprintDragCancelled));
    });
}
