//! #2373 — the optional `[free]` section of a `.snxpcb`: free arcs, pads
//! and dimensions.

use uuid::Uuid;

use super::tests::empty_board;
use super::*;
use crate::pcb::{DrillDef, Pad, PadNet, PadShape, PadType, Point};
use crate::pcb_arc::TrackArc;
use crate::pcb_dimension::LinearDimension;

const OLD_BOARD: &str = include_str!("../../tests/corpus/pcb/board-outline-only.snxpcb");

fn arc() -> TrackArc {
    TrackArc {
        uuid: Uuid::from_u128(11),
        start: Point::new(1.5, 2.25),
        mid: Point::new(3.0, 0.75),
        end: Point::new(4.5, 2.25),
        width: 0.25,
        layer: "TopCopper".into(),
        net: 1,
    }
}

fn pad() -> Pad {
    Pad {
        uuid: Uuid::from_u128(12),
        number: "MH \"1\"".into(),
        pad_type: PadType::Thru,
        shape: PadShape::RoundRect,
        position: Point::new(-5.5, 10.0),
        size: Point::new(1.5, 2.0),
        drill: Some(DrillDef {
            diameter: 0.75,
            shape: String::new(),
        }),
        layers: vec!["TopCopper".into(), "BottomCopper".into()],
        net: Some(PadNet {
            number: 1,
            name: "GND".into(),
        }),
        roundrect_ratio: 0.25,
        trapezoid_delta: Default::default(),
        custom_outline: Vec::new(),
        rotation: 45.0,
        net_locked: true,
        layer_sizes: None,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn dimension() -> LinearDimension {
    let mut d = LinearDimension::new(
        Uuid::from_u128(13),
        Point::new(0.0, 0.0),
        Point::new(20.0, 0.0),
        -2.5,
    );
    d.precision = 3;
    d
}

#[test]
fn a_board_without_free_primitives_writes_no_section() {
    let board = SnxPcb::parse(OLD_BOARD).expect("parses").board;
    assert!(board.free.is_empty());
    let text = SnxPcb::new(board).write_string().expect("writes");
    assert_eq!(text, OLD_BOARD, "no-edit save must keep every byte");
    assert!(!text.contains("[free"), "{text}");
}

#[test]
fn free_arcs_pads_and_dimensions_round_trip() {
    let mut board = empty_board();
    board.free.arcs.push(arc());
    board.free.pads.push(pad());
    board.free.dimensions.push(dimension());
    let text = SnxPcb::new(board.clone()).write_string().expect("writes");
    assert!(text.contains("[[free.arcs]]"), "{text}");
    assert!(text.contains("[[free.pads]]"), "{text}");
    assert!(text.contains("[[free.dimensions]]"), "{text}");
    let back = SnxPcb::parse(&text).expect("reads back").board;
    assert_eq!(back.free, board.free);
    let again = SnxPcb::new(back).write_string().expect("writes again");
    assert_eq!(again, text, "a second save is byte-identical");
}

#[test]
fn a_pad_without_a_hole_or_net_round_trips() {
    let mut board = empty_board();
    let mut smd = pad();
    smd.pad_type = PadType::Smd;
    smd.drill = None;
    smd.net = None;
    smd.net_locked = false;
    smd.layers = vec!["TopCopper".into()];
    board.free.pads.push(smd);
    let text = SnxPcb::new(board.clone()).write_string().expect("writes");
    let back = SnxPcb::parse(&text).expect("reads back").board;
    assert_eq!(back.free, board.free);
}

#[test]
fn a_non_finite_free_arc_is_refused_on_write() {
    let mut board = empty_board();
    let mut bad = arc();
    bad.mid.x = f64::NAN;
    board.free.arcs.push(bad);
    assert!(matches!(
        SnxPcb::new(board).write_string(),
        Err(FormatError::NonFiniteField { .. })
    ));
}

#[test]
fn a_non_finite_pad_turn_is_refused_on_write() {
    let mut board = empty_board();
    let mut bad = pad();
    bad.rotation = f64::INFINITY;
    board.free.pads.push(bad);
    assert!(matches!(
        SnxPcb::new(board).write_string(),
        Err(FormatError::NonFiniteTomlField { .. })
    ));
}

/// The free items share the board's one id space: a free pad repeating a
/// track's uuid is given a new one on load.
#[test]
fn a_free_item_repeating_an_id_is_reminted_on_load() {
    let mut board = empty_board();
    board.free.arcs.push(arc());
    let mut twin = pad();
    twin.uuid = arc().uuid;
    board.free.pads.push(twin);
    let text = SnxPcb::new(board).write_string().expect("writes");
    let mut back = SnxPcb::parse(&text).expect("reads back").board;
    assert_ne!(back.free.pads[0].uuid, back.free.arcs[0].uuid);
    assert_eq!(back.take_load_repairs().len(), 1);
}

/// A free pad keeps per-layer sizes (#2401) across a save, in the `pads`
/// rows' cell form; a malformed cell is a load error.
#[test]
fn a_free_pad_with_per_layer_sizes_round_trips() {
    let mut board = empty_board();
    let mut stacked = pad();
    stacked.layer_sizes = Some(crate::pcb::LayerSizes::TopMiddleBottom {
        top: Point::new(1.5, 2.0),
        middle: Point::new(1.0, 1.25),
        bottom: Point::new(1.5, 2.0),
    });
    board.free.pads.push(stacked);
    let text = SnxPcb::new(board.clone()).write_string().expect("writes");
    assert!(text.contains("layer_sizes"), "{text}");
    let back = SnxPcb::parse(&text).expect("reads back").board;
    assert_eq!(back.free, board.free);

    let broken = text.replacen("top=", "nowhere=", 1);
    assert!(SnxPcb::parse(&broken).is_err(), "{broken}");
}

#[test]
fn free_trapezoid_and_custom_pads_round_trip_and_bad_ones_fail() {
    // #2242: the `[free]` pads carry the same shape cells as the `pads` rows.
    let mut board = empty_board();
    let mut trapezoid = pad();
    trapezoid.shape = PadShape::Trapezoid;
    trapezoid.trapezoid_delta = Point::new(0.5, 0.0);
    let mut custom = pad();
    custom.uuid = Uuid::from_u128(14);
    custom.shape = PadShape::Custom;
    custom.custom_outline = vec![
        Point::new(0.0, 0.0),
        Point::new(1.0, 0.0),
        Point::new(0.0, 1.0),
    ];
    board.free.pads.extend([trapezoid, custom]);
    let text = SnxPcb::new(board.clone()).write_string().expect("writes");
    assert!(text.contains("trapezoid_delta = \"500000,0\""), "{text}");
    assert!(
        text.contains("custom_outline = \"0,0;1000000,0;0,1000000\""),
        "{text}"
    );
    let back = SnxPcb::parse(&text).expect("reads back").board;
    assert_eq!(back.free, board.free);

    let err = SnxPcb::parse(&text.replacen("0,0;1000000,0;0,1000000", "0,0;1000000,0", 1))
        .unwrap_err()
        .to_string();
    assert!(err.contains("\"custom_outline\""), "{err}");
}
