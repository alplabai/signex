//! #2373: the Arc, Pad and Dimension tools' items, each one undo step.

use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use uuid::Uuid;

use super::BoardEngine;
use super::draw::{
    ArcMode, FREE_PAD_HOLE_MM, FREE_PAD_SIZE_MM, arc_points, new_arc, new_dimension, new_free_pad,
    new_free_pad_on, place_item,
};
use super::items::BoardItem;

/// A board with a net-3 track along y = 0 on the top layer.
fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "nets": [{"number": 3, "name": "GND"}],
        "segments": [{"uuid": Uuid::from_u128(2), "start": {"x": 0.0, "y": 0.0},
                      "end": {"x": 10.0, "y": 0.0}, "width": 0.5,
                      "layer": "TopCopper", "net": 3}]
    }))
    .unwrap()
}

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn close(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
}

#[test]
fn a_centre_arc_runs_counter_clockwise_on_screen_from_start_to_end() {
    // Centre (0, 0), start (5, 0), end toward (0, -5): up on screen.
    let (start, mid, end) =
        arc_points(ArcMode::Centre, &[p(0.0, 0.0), p(5.0, 0.0), p(0.0, -9.0)]).unwrap();
    assert!(close(start, p(5.0, 0.0)));
    assert!(close(end, p(0.0, -5.0)), "{end:?}");
    let h = 5.0 * std::f64::consts::FRAC_1_SQRT_2;
    assert!(
        close(mid, p(h, -h)),
        "the short way, through the upper right: {mid:?}"
    );
}

#[test]
fn an_edge_arc_is_a_quarter_circle_between_its_ends() {
    let (start, mid, end) = arc_points(ArcMode::Edge, &[p(0.0, 0.0), p(2.0, 0.0)]).unwrap();
    assert_eq!((start, end), (p(0.0, 0.0), p(2.0, 0.0)));
    let arc = signex_types::pcb_arc::TrackArc {
        uuid: Uuid::nil(),
        start,
        mid,
        end,
        width: 0.1,
        layer: String::new(),
        net: 0,
    };
    let sweep = arc.sweep().unwrap();
    assert!((sweep.sweep.abs() - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
}

#[test]
fn an_any_angle_arc_bends_through_its_third_click_and_refuses_a_line() {
    let points = arc_points(ArcMode::AnyAngle, &[p(0.0, 0.0), p(4.0, 0.0), p(2.0, 2.0)]);
    assert_eq!(points, Some((p(0.0, 0.0), p(2.0, 2.0), p(4.0, 0.0))));
    let line = arc_points(ArcMode::AnyAngle, &[p(0.0, 0.0), p(4.0, 0.0), p(2.0, 0.0)]);
    assert_eq!(line, None);
    assert_eq!(
        arc_points(ArcMode::Centre, &[p(0.0, 0.0), p(1.0, 0.0)]),
        None
    );
    assert_eq!(ArcMode::Centre.next().next().next(), ArcMode::Centre);
}

#[test]
fn a_copper_arc_takes_the_net_it_starts_on_and_lands_in_one_step() {
    let mut engine = BoardEngine::new(board(), 1);
    let before = serde_json::to_string(engine.board()).unwrap();
    let points = (p(5.0, 0.0), p(7.0, 2.0), p(9.0, 0.0));
    let top = PcbLayer::Copper(CopperLayer::Top);
    let arc = new_arc(engine.board(), Uuid::from_u128(0x20), points, top, 0.3).unwrap();
    assert_eq!(arc.net, 3);
    let overlay = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
    let drawn = new_arc(engine.board(), Uuid::from_u128(0x21), points, overlay, 0.3).unwrap();
    assert_eq!(drawn.net, 0, "an arc off copper has no net");
    engine
        .execute(place_item(BoardItem::Arc(arc.clone())))
        .unwrap();
    assert_eq!(engine.board().free.arcs, [arc]);
    engine.undo().unwrap();
    assert_eq!(serde_json::to_string(engine.board()).unwrap(), before);
}

#[test]
fn a_free_pad_is_a_through_pad_on_the_net_it_lands_on() {
    let mut engine = BoardEngine::new(board(), 1);
    let pad = new_free_pad(
        engine.board(),
        Uuid::from_u128(0x30),
        p(5.0, 0.0),
        "1".into(),
    );
    assert_eq!(pad.net.as_ref().map(|n| n.number), Some(3));
    assert_eq!(pad.size, p(FREE_PAD_SIZE_MM, FREE_PAD_SIZE_MM));
    assert_eq!(
        pad.drill.as_ref().map(|d| d.diameter),
        Some(FREE_PAD_HOLE_MM)
    );
    engine
        .execute(place_item(BoardItem::FreePad(pad.clone())))
        .unwrap();
    assert_eq!(engine.board().free.pads, [pad]);
    assert!(engine.undo().is_ok());
    assert!(engine.board().free.pads.is_empty());
}

/// Caner 2026-11-21: a pad placed with an outer copper layer active is a
/// surface pad of that side; any other layer keeps the through-hole pad.
#[test]
fn a_free_pad_on_outer_copper_is_a_surface_pad_of_that_side() {
    let board = board();
    let id = Uuid::from_u128(0x31);
    let top = new_free_pad_on(&board, id, p(5.0, 0.0), "1".into(), CopperLayer::Top.into());
    assert_eq!(top.pad_type, signex_types::pcb::PadType::Smd);
    assert!(top.drill.is_none());
    assert_eq!(top.layers, ["TopCopper", "TopSolderMask", "TopPaste"]);
    assert_eq!(top.net.as_ref().map(|n| n.number), Some(3));
    let bottom = new_free_pad_on(
        &board,
        id,
        p(5.0, 0.0),
        "1".into(),
        CopperLayer::Bottom.into(),
    );
    assert_eq!(
        bottom.layers,
        ["BottomCopper", "BottomSolderMask", "BottomPaste"]
    );
    assert!(bottom.net.is_none(), "the track is on the top copper only");
    let silk = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
    let through = new_free_pad_on(&board, id, p(5.0, 0.0), "1".into(), silk);
    assert_eq!(through, new_free_pad(&board, id, p(5.0, 0.0), "1".into()));
}

#[test]
fn a_dimension_stands_off_through_its_third_click_on_the_given_layer() {
    let id = Uuid::from_u128(0x40);
    let mech = PcbLayer::Mechanical(signex_types::pcb_layer::MechanicalLayer::new(1).unwrap());
    let d = new_dimension(id, p(0.0, 0.0), p(10.0, 0.0), p(4.0, 3.0), mech).unwrap();
    assert_eq!((d.offset, d.layer.as_str()), (3.0, "Mechanical1"));
    // Caner 2026-11-21: copper included.
    let top = PcbLayer::Copper(CopperLayer::Top);
    let on_copper = new_dimension(id, p(0.0, 0.0), p(10.0, 0.0), p(4.0, -2.0), top).unwrap();
    assert_eq!(on_copper.layer, "TopCopper", "the given copper layer");
    assert_eq!(on_copper.offset, -2.0);
    assert!(new_dimension(id, p(1.0, 1.0), p(1.0, 1.0), p(4.0, 3.0), mech).is_none());
}
