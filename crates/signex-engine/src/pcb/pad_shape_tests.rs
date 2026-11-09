//! Rotate and Flip turn a footprint's pads as true shapes (#680): the board
//! after the command reads, through `pcb_geom`, as the rotated or mirrored copper.

use pcb_geom::{PointNm, Shape, pad_shape};
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::BoardCommand;

const FP: &str = "00000000-0000-0000-0000-00000000000a";

fn board(shape: &str) -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": "00000000-0000-0000-0000-000000000001",
        "footprints": [{
            "uuid": FP, "reference": "U1", "position": {"x": 0.0, "y": 0.0},
            "rotation": 0.0, "layer": "TopCopper",
            "pads": [{
                "uuid": "00000000-0000-0000-0000-0000000000a1", "number": "1",
                "pad_type": "smd", "shape": shape, "roundrect_ratio": 0.25,
                "position": {"x": 2.0, "y": 0.0}, "size": {"x": 3.0, "y": 1.0},
                "rotation": 0.0, "drill": null, "layers": ["TopCopper"], "net": null
            }]
        }]
    }))
    .unwrap()
}

fn copper(board: &PcbBoard) -> Shape {
    pad_shape(&board.footprints[0], &board.footprints[0].pads[0]).unwrap()
}

#[test]
fn rotate_turns_an_oval_pad_into_a_vertical_stadium_about_the_origin() {
    let mut b = board("oval");
    let id = Uuid::parse_str(FP).unwrap();
    BoardCommand::Rotate {
        footprint: id,
        degrees: 90.0,
    }
    .apply(&mut b)
    .unwrap();
    // Centre (2, 0) -> (0, 2); the 3 x 1 stadium's cap centres run along y.
    assert_eq!(
        copper(&b),
        Shape::Capsule {
            a: PointNm::new(0, 1_000_000),
            b: PointNm::new(0, 3_000_000),
            radius: 500_000
        }
    );
}

#[test]
fn rotate_keeps_a_round_rect_radius_and_flip_mirrors_its_angle() {
    let mut b = board("round_rect");
    let id = Uuid::parse_str(FP).unwrap();
    BoardCommand::Rotate {
        footprint: id,
        degrees: 30.0,
    }
    .apply(&mut b)
    .unwrap();
    let Shape::RoundRect {
        radius,
        rotation_deg,
        ..
    } = copper(&b)
    else {
        panic!("round-rect expected");
    };
    assert_eq!((radius, rotation_deg), (250_000, 30.0));

    let flipped = BoardCommand::Flip { footprint: id }.apply(&mut b).unwrap();
    let Shape::RoundRect {
        center,
        radius,
        rotation_deg,
        ..
    } = copper(&b)
    else {
        panic!("round-rect expected");
    };
    // Mirrored about the y axis through the origin, turned -30 (= 330) in all.
    assert_eq!((radius, rotation_deg), (250_000, 330.0));
    assert_eq!(center, PointNm::new(-1_732_051, 1_000_000));
    assert_ne!(flipped.inverse, BoardCommand::Flip { footprint: id });
}
