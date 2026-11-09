//! A pad keeps its true shape and turns with its footprint and by itself (#680).

use super::*;
use crate::{BoardIndex, ItemRef, ratsnest};
use signex_types::pcb::{PadNet, PadType, PcbBoard, Point};
use uuid::Uuid;

fn pad(n: u128, x: f64, size: (f64, f64), shape: PadShape, rotation: f64) -> Pad {
    Pad {
        uuid: Uuid::from_u128(n),
        number: n.to_string(),
        pad_type: PadType::Smd,
        shape,
        position: Point::new(x, 0.0),
        size: Point::new(size.0, size.1),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: Some(PadNet {
            number: 1,
            name: "N1".into(),
        }),
        roundrect_ratio: 0.25,
        rotation,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn fp(rotation: f64, layer: &str, pads: Vec<Pad>) -> Footprint {
    Footprint {
        uuid: Uuid::from_u128(99),
        reference: "U1".into(),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(0.0, 0.0),
        rotation,
        layer: layer.into(),
        locked: false,
        schematic_link: None,
        pads,
        graphics: Vec::new(),
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn corners(f: &Footprint) -> Vec<PointNm> {
    pad_shape(f, &f.pads[0]).unwrap().outline(8)
}

fn p(x: i64, y: i64) -> PointNm {
    PointNm::new(x, y)
}

fn rect_at(rotation: f64, own: f64) -> Footprint {
    fp(
        rotation,
        "TopCopper",
        vec![pad(1, 0.0, (2.0, 1.0), PadShape::Rect, own)],
    )
}

#[test]
fn a_rect_at_30_degrees_has_exact_corners() {
    let want = vec![
        p(-616_025, -933_013),
        p(1_116_025, 66_987),
        p(616_025, 933_013),
        p(-1_116_025, -66_987),
    ];
    // Footprint turn, pad turn, and their sum all land on the same copper.
    for (footprint, own) in [(30.0, 0.0), (0.0, 30.0), (10.0, 20.0)] {
        assert_eq!(corners(&rect_at(footprint, own)), want, "{footprint}+{own}");
    }
}

#[test]
fn a_rect_at_90_degrees_composed_has_exact_corners() {
    let want = vec![
        p(500_000, -1_000_000),
        p(500_000, 1_000_000),
        p(-500_000, 1_000_000),
        p(-500_000, -1_000_000),
    ];
    assert_eq!(corners(&rect_at(40.0, 50.0)), want);
    assert_eq!(corners(&rect_at(90.0, 0.0)), want);
    assert_eq!(corners(&rect_at(0.0, 90.0)), want);
}

#[test]
fn a_bottom_footprint_reverses_the_pads_own_turn() {
    let f = fp(
        0.0,
        "BottomCopper",
        vec![pad(1, 0.0, (2.0, 1.0), PadShape::Rect, 30.0)],
    );
    assert_eq!(pad_world_rotation_deg(&f, &f.pads[0]), Ok(-30.0));
    let f = fp(
        100.0,
        "BottomCopper",
        vec![pad(1, 0.0, (2.0, 1.0), PadShape::Rect, 30.0)],
    );
    assert_eq!(pad_world_rotation_deg(&f, &f.pads[0]), Ok(70.0));
}

#[test]
fn a_flip_leaves_the_mirrored_pad_where_a_physical_turn_puts_it() {
    use crate::flip_footprint;
    let mut f = fp(
        30.0,
        "TopCopper",
        vec![pad(1, 1.0, (2.0, 1.0), PadShape::Rect, 20.0)],
    );
    let before = pad_shape(&f, &f.pads[0]).unwrap();
    flip_footprint(&mut f);
    let after = pad_shape(&f, &f.pads[0]).unwrap();
    let (
        Shape::Rect {
            center: c0,
            rotation_deg: r0,
            ..
        },
        Shape::Rect {
            center: c1,
            rotation_deg: r1,
            ..
        },
    ) = (before, after)
    else {
        panic!("rects expected");
    };
    // Mirror about the y axis: x flips, the angle negates.
    assert!((c1.x + c0.x).abs() <= 1 && (c1.y - c0.y).abs() <= 1);
    assert!(
        (r1.rem_euclid(360.0) - (-r0).rem_euclid(360.0)).abs() < 1e-9,
        "{r0} -> {r1}"
    );
}

#[test]
fn a_round_rect_keeps_its_radius_under_rotation() {
    for (footprint, own) in [(0.0, 0.0), (30.0, 15.0), (90.0, 0.0)] {
        let f = fp(
            footprint,
            "TopCopper",
            vec![pad(1, 0.0, (2.0, 1.0), PadShape::RoundRect, own)],
        );
        let shape = pad_shape(&f, &f.pads[0]).unwrap();
        // ratio 0.25 of the 1 mm short side.
        assert!(
            matches!(
                shape,
                Shape::RoundRect {
                    radius: 250_000,
                    half_w: 1_000_000,
                    half_h: 500_000,
                    ..
                }
            ),
            "{shape:?}"
        );
    }
}

#[test]
fn an_oval_stays_a_stadium_along_the_turned_long_axis() {
    let f = fp(
        0.0,
        "TopCopper",
        vec![pad(1, 0.0, (3.0, 1.0), PadShape::Oval, 90.0)],
    );
    assert_eq!(
        pad_shape(&f, &f.pads[0]).unwrap(),
        Shape::Capsule {
            a: p(0, -1_000_000),
            b: p(0, 1_000_000),
            radius: 500_000
        }
    );
    let f = fp(
        0.0,
        "TopCopper",
        vec![pad(1, 0.0, (3.0, 1.0), PadShape::Oval, 0.0)],
    );
    assert_eq!(
        pad_shape(&f, &f.pads[0]).unwrap(),
        Shape::Capsule {
            a: p(-1_000_000, 0),
            b: p(1_000_000, 0),
            radius: 500_000
        }
    );
}

#[test]
fn a_circle_is_rotation_invariant() {
    let shape = |footprint, own| {
        let f = fp(
            footprint,
            "TopCopper",
            vec![pad(1, 0.0, (2.0, 1.0), PadShape::Circle, own)],
        );
        pad_shape(&f, &f.pads[0]).unwrap()
    };
    assert_eq!(shape(0.0, 0.0), shape(37.0, 81.0));
}

#[test]
fn hit_testing_reads_the_rotated_rect_not_its_axis_aligned_box() {
    let f = rect_at(45.0, 0.0);
    let shape = pad_shape(&f, &f.pads[0]).unwrap();
    // Inside the turned rect, outside the unturned 2 x 1 box.
    assert!(shape.contains(p(600_000, 600_000), 0));
    // Inside the unturned box, outside the turned rect.
    assert!(!shape.contains(p(900_000, -100_000), 0));
    let index = BoardIndex::build(&board_of(vec![f]));
    let pad_ref = ItemRef::Pad {
        footprint: Uuid::from_u128(99),
        pad: Uuid::from_u128(1),
    };
    assert_eq!(index.hit_point(p(600_000, 600_000), 0), vec![pad_ref]);
    assert!(index.hit_point(p(900_000, -100_000), 0).is_empty());
}

fn board_of(footprints: Vec<Footprint>) -> PcbBoard {
    PcbBoard {
        uuid: Uuid::nil(),
        version: 1,
        generator: String::new(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: Vec::new(),
        setup: None,
        nets: Vec::new(),
        footprints,
        segments: Vec::new(),
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

#[test]
fn the_ratsnest_joins_pads_through_their_turned_shape() {
    // A 3 x 0.4 mm strip at the origin and a 1 x 1 mm pad 1 mm above it:
    // flat, a 0.3 mm gap; turned a quarter, the strip reaches the pad.
    let strip = |own| pad(1, 0.0, (3.0, 0.4), PadShape::Rect, own);
    let mut above = pad(2, 0.0, (1.0, 1.0), PadShape::Rect, 0.0);
    above.position = Point::new(0.0, 1.0);
    let board = |own| board_of(vec![fp(0.0, "TopCopper", vec![strip(own), above.clone()])]);
    assert_eq!(ratsnest(&board(0.0)).len(), 1);
    assert!(ratsnest(&board(90.0)).is_empty());
}

#[test]
fn a_non_finite_own_rotation_is_refused() {
    let f = rect_at(0.0, f64::NAN);
    assert_eq!(
        pad_shape(&f, &f.pads[0]),
        Err(GeomError::Unrepresentable("pad.rotation"))
    );
}
