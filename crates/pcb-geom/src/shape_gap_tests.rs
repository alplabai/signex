use super::*;

fn p(x: i64, y: i64) -> PointNm {
    PointNm::new(x, y)
}

fn disc(x: i64, y: i64, radius: i64) -> Shape {
    Shape::Disc {
        center: p(x, y),
        radius,
    }
}

fn rect(x: i64, y: i64, half_w: i64, half_h: i64, rotation_deg: f64) -> Shape {
    Shape::Rect {
        center: p(x, y),
        half_w,
        half_h,
        rotation_deg,
    }
}

#[test]
fn two_discs_are_the_centre_distance_less_both_radii() {
    let sep = disc(0, 0, 100).separation(&disc(1_000, 0, 200));
    assert_eq!(sep.distance_nm(), 700);
    assert!(sep.is_under(701));
    assert!(
        !sep.is_under(700),
        "a gap equal to the limit is not under it"
    );
    assert!(!sep.is_under(0));
}

#[test]
fn overlapping_and_touching_shapes_are_zero_apart() {
    assert_eq!(
        disc(0, 0, 100).separation(&disc(150, 0, 100)).distance_nm(),
        0
    );
    assert_eq!(
        disc(0, 0, 100).separation(&disc(200, 0, 100)).distance_nm(),
        0
    );
    let crossing = Shape::Capsule {
        a: p(-500, 0),
        b: p(500, 0),
        radius: 10,
    };
    let other = Shape::Capsule {
        a: p(0, -500),
        b: p(0, 500),
        radius: 10,
    };
    assert_eq!(crossing.separation(&other).distance_nm(), 0);
    assert!(crossing.separation(&other).is_under(1));
}

#[test]
fn a_track_beside_a_rectangle_is_measured_to_its_edge() {
    let pad = rect(0, 0, 500, 500, 0.0);
    let track = Shape::Capsule {
        a: p(-2_000, 1_000),
        b: p(2_000, 1_000),
        radius: 100,
    };
    // Edge at y = 500, track copper starts at y = 900.
    let sep = pad.separation(&track);
    assert_eq!(sep.distance_nm(), 400);
    assert!(sep.is_under(401) && !sep.is_under(400));
    let (on_pad, on_track) = sep.between;
    assert_eq!((on_pad.y, on_track.y), (500, 1_000));
}

#[test]
fn a_track_across_a_rectangle_and_one_inside_it_are_zero_apart() {
    let pad = rect(0, 0, 500, 500, 0.0);
    let across = Shape::Capsule {
        a: p(-2_000, 0),
        b: p(2_000, 0),
        radius: 1,
    };
    let inside = Shape::Capsule {
        a: p(-10, 0),
        b: p(10, 0),
        radius: 1,
    };
    assert_eq!(pad.separation(&across).distance_nm(), 0);
    assert_eq!(pad.separation(&inside).distance_nm(), 0);
}

#[test]
fn two_rectangles_are_measured_edge_to_edge_and_a_turned_one_by_its_corner() {
    let a = rect(0, 0, 500, 500, 0.0);
    assert_eq!(
        a.separation(&rect(2_000, 0, 500, 500, 0.0)).distance_nm(),
        1_000
    );
    assert_eq!(
        a.separation(&rect(300, 300, 100, 100, 0.0)).distance_nm(),
        0
    );
    // A square turned 45 degrees reaches 707 along an axis; its tip is
    // 2_000 - 707 = 1_293 from the origin, 793 from a's edge.
    let turned = rect(2_000, 0, 500, 500, 45.0);
    let gap = a.separation(&turned).distance_nm();
    assert!((792..=793).contains(&gap), "{gap}");
}
