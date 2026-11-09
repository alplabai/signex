//! Every pad shape and its parameters survive a save and load, and a shape
//! this build cannot represent fails loudly (#680).

use super::tests::empty_board;
use super::*;
use crate::pcb::{Footprint, Pad, PadShape, PadType, Point as PcbPoint};
use uuid::Uuid;

fn board_with(pads: Vec<Pad>) -> crate::pcb::PcbBoard {
    let mut board = empty_board();
    board.footprints.push(Footprint {
        schematic_link: None,
        unknown_columns: Default::default(),
        unknown: Default::default(),
        uuid: Uuid::from_u128(1),
        reference: "U1".into(),
        value: String::new(),
        footprint_id: "x.snxfpt".into(),
        position: PcbPoint { x: 0.0, y: 0.0 },
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        pads,
        graphics: Vec::new(),
        properties: Vec::new(),
    });
    board
}

fn pad(n: u128, shape: PadShape, ratio: f64, rotation: f64) -> Pad {
    Pad {
        unknown_columns: Default::default(),
        unknown: Default::default(),
        uuid: Uuid::from_u128(100 + n),
        number: n.to_string(),
        pad_type: PadType::Smd,
        shape,
        position: PcbPoint { x: 1.0, y: 2.0 },
        size: PcbPoint { x: 1.5, y: 1.0 },
        drill: None,
        layers: vec!["TopCopper".into()],
        net: None,
        roundrect_ratio: ratio,
        rotation,
        net_locked: false,
    }
}

#[test]
fn every_shape_round_trips_with_its_parameters_and_its_own_rotation() {
    let pads = vec![
        pad(1, PadShape::Circle, 0.0, 0.0),
        pad(2, PadShape::Rect, 0.0, 30.0),
        pad(3, PadShape::Oval, 0.0, -90.0),
        pad(4, PadShape::RoundRect, 0.25, 12.5),
    ];
    let text = SnxPcb::new(board_with(pads.clone()))
        .write_string()
        .unwrap();
    assert!(text.starts_with("format = \"snxpcb/1.3\""), "{text}");
    let back = SnxPcb::parse(&text).unwrap();
    assert_eq!(back.board.footprints[0].pads, pads);
}

#[test]
fn a_board_with_no_turned_pad_writes_no_rotation_column() {
    let text = SnxPcb::new(board_with(vec![pad(1, PadShape::Rect, 0.0, 0.0)]))
        .write_string()
        .unwrap();
    let pads = text.split("[pads]").nth(1).unwrap();
    assert!(!pads.contains("rotation"), "{text}");
}

fn text_with_row(shape: &str, ratio: &str) -> String {
    let text = SnxPcb::new(board_with(vec![pad(1, PadShape::RoundRect, 0.25, 0.0)]))
        .write_string()
        .unwrap();
    // Only the shape cell: the `roundrect_ratio` header column stays.
    text.replace("roundrect_ratio", "@@")
        .replace("roundrect", shape)
        .replace("@@", "roundrect_ratio")
        .replace("0.25", ratio)
}

#[test]
fn an_unknown_shape_fails_naming_the_field_and_the_value() {
    for token in ["trapezoid", "custom", "hexagon"] {
        let err = SnxPcb::parse(&text_with_row(token, "0.25"))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("row 0")
                && err.contains("\"shape\"")
                && err.contains(token)
                && err.contains("not a pad shape"),
            "{err}"
        );
    }
}

#[test]
fn a_roundrect_ratio_outside_zero_to_half_fails_naming_the_value() {
    for ratio in ["0.75", "-0.1"] {
        let err = SnxPcb::parse(&text_with_row("roundrect", ratio))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("\"roundrect_ratio\"") && err.contains(ratio),
            "{err}"
        );
    }
}

#[test]
fn a_rotation_cell_that_is_not_a_number_fails_naming_the_value() {
    let turned = SnxPcb::new(board_with(vec![pad(1, PadShape::Rect, 0.0, 30.0)]))
        .write_string()
        .unwrap();
    let err = SnxPcb::parse(&turned.replace(" 30", " thirty"))
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("\"rotation\"") && err.contains("thirty"),
        "{err}"
    );
}
