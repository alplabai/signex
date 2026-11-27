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
        library_ref: None,
        component: Default::default(),
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
        trapezoid_delta: Default::default(),
        custom_outline: Vec::new(),
        rotation,
        layer_sizes: None,
        net_locked: false,
        thermal: Default::default(),
    }
}

#[test]
fn every_shape_round_trips_with_its_parameters_and_its_own_rotation() {
    let pads = vec![
        pad(1, PadShape::Circle, 0.0, 0.0),
        pad(2, PadShape::Rect, 0.0, 30.0),
        pad(3, PadShape::Oval, 0.0, -90.0),
        pad(4, PadShape::RoundRect, 0.25, 12.5),
        trapezoid(5, PcbPoint { x: 0.0, y: -0.5 }),
        custom(6),
    ];
    let text = SnxPcb::new(board_with(pads.clone()))
        .write_string()
        .unwrap();
    assert!(text.starts_with("format = \"snxpcb/1.4\""), "{text}");
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
    assert!(
        !pads.contains("trapezoid_delta") && !pads.contains("custom_outline"),
        "{text}"
    );
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
    for token in ["hexagon", "octagon"] {
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
fn a_roundrect_ratio_the_reader_refuses_is_refused_on_save() {
    for ratio in [0.6, -0.1, f64::NAN] {
        let err = SnxPcb::new(board_with(vec![pad(1, PadShape::RoundRect, ratio, 0.0)]))
            .write_string()
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("\"pads\"")
                && err.contains("row 0")
                && err.contains("\"roundrect_ratio\""),
            "{err}"
        );
    }
    // Only a round-rect carries the ratio; another shape saves with any value.
    let text = SnxPcb::new(board_with(vec![pad(1, PadShape::Rect, 0.6, 0.0)]))
        .write_string()
        .unwrap();
    SnxPcb::parse(&text).unwrap();
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

fn trapezoid(n: u128, delta: PcbPoint) -> Pad {
    Pad {
        trapezoid_delta: delta,
        ..pad(n, PadShape::Trapezoid, 0.0, 45.0)
    }
}

fn custom(n: u128) -> Pad {
    Pad {
        custom_outline: [(-0.5, -0.25), (0.75, -0.25), (0.75, 0.5), (0.25, 0.125)]
            .map(|(x, y)| PcbPoint { x, y })
            .to_vec(),
        ..pad(n, PadShape::Custom, 0.0, -30.0)
    }
}

#[test]
fn the_shape_columns_hold_whole_nm_in_the_pads_own_frame() {
    let text = SnxPcb::new(board_with(vec![
        trapezoid(1, PcbPoint { x: 0.0, y: -0.5 }),
        custom(2),
    ]))
    .write_string()
    .unwrap();
    let pads = text.split("[pads]").nth(1).unwrap();
    assert!(
        pads.contains("trapezoid_delta") && pads.contains("custom_outline"),
        "{text}"
    );
    assert!(pads.contains("0,-500000"), "{text}");
    assert!(
        pads.contains("-500000,-250000;750000,-250000;750000,500000;250000,125000"),
        "{text}"
    );
}

/// The pads of a board holding one trapezoid and one custom pad, with
/// `from` replaced by `to` in the text.
fn edited(from: &str, to: &str) -> String {
    let text = SnxPcb::new(board_with(vec![
        trapezoid(1, PcbPoint { x: 0.0, y: -0.5 }),
        custom(2),
    ]))
    .write_string()
    .unwrap();
    assert!(text.contains(from), "{text}");
    text.replacen(from, to, 1)
}

#[test]
fn shape_parameters_that_make_no_copper_fail_naming_the_field_and_the_value() {
    let cases = [
        // Both axes set.
        ("0,-500000", "300000,-500000", "trapezoid_delta"),
        // Wider than twice the 1 mm side it narrows.
        ("0,-500000", "0,-2500000", "trapezoid_delta"),
        // Two corners.
        (
            "-500000,-250000;750000,-250000;750000,500000;250000,125000",
            "-500000,-250000;750000,-250000",
            "custom_outline",
        ),
        // A bow tie.
        (
            "-500000,-250000;750000,-250000;750000,500000;250000,125000",
            "0,0;2000000,2000000;2000000,0;0,1000000",
            "custom_outline",
        ),
    ];
    for (from, to, field) in cases {
        let err = SnxPcb::parse(&edited(from, to)).unwrap_err().to_string();
        assert!(
            err.contains(&format!("\"{field}\"")) && err.contains(to),
            "{err}"
        );
    }
}

#[test]
fn a_shape_cell_that_does_not_read_fails_naming_it() {
    let err = SnxPcb::parse(&edited("0,-500000", "wide"))
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("\"trapezoid_delta\"") && err.contains("wide"),
        "{err}"
    );
    let err = SnxPcb::parse(&edited("750000,500000;250000", "750000;x"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("\"custom_outline\""), "{err}");
}

#[test]
fn a_custom_pad_without_an_outline_fails_to_load_and_to_save() {
    let mut bare = custom(1);
    bare.custom_outline.clear();
    let err = SnxPcb::new(board_with(vec![bare]))
        .write_string()
        .unwrap_err()
        .to_string();
    assert!(err.contains("\"custom_outline\""), "{err}");
    // A row whose shape is `custom` with no outline column.
    let err = SnxPcb::parse(&text_with_row("custom", "0.25"))
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("\"custom_outline\"") && err.contains("at least 3 corners"),
        "{err}"
    );
}

#[test]
fn a_trapezoid_row_with_no_delta_fails_rather_than_loading_as_a_rect() {
    // How files before 1.3 wrote a trapezoid: the token, no parameters.
    let err = SnxPcb::parse(&text_with_row("trapezoid", "0.25"))
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("\"trapezoid_delta\"") && err.contains("with none it is a rect"),
        "{err}"
    );
}

#[test]
fn a_per_layer_size_too_small_for_the_trapezoid_delta_fails_to_load_and_to_save() {
    // Each layer's copper is built from its own size, so the delta must
    // fit every per-layer size, not only the pad's own.
    let with_middle = |y: f64| Pad {
        layer_sizes: Some(crate::pcb::LayerSizes::TopMiddleBottom {
            top: PcbPoint { x: 1.5, y: 1.0 },
            middle: PcbPoint { x: 1.5, y },
            bottom: PcbPoint { x: 1.5, y: 1.0 },
        }),
        ..trapezoid(1, PcbPoint { x: 0.0, y: -0.5 })
    };
    let err = SnxPcb::new(board_with(vec![with_middle(0.25)]))
        .write_string()
        .unwrap_err()
        .to_string();
    assert!(err.contains("\"trapezoid_delta\""), "{err}");

    let text = SnxPcb::new(board_with(vec![with_middle(0.3)]))
        .write_string()
        .unwrap();
    let narrowed = "middle=1500000x200000";
    let broken = text.replacen("middle=1500000x300000", narrowed, 1);
    assert!(broken.contains(narrowed), "{text}");
    let err = SnxPcb::parse(&broken).unwrap_err().to_string();
    assert!(
        err.contains("\"trapezoid_delta\"") && err.contains("twice the side"),
        "{err}"
    );
}
