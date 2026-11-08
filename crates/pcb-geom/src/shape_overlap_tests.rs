use super::*;

const MM: i64 = 1_000_000;

fn p(x: i64, y: i64) -> PointNm {
    PointNm::new(x * MM, y * MM)
}

fn rect(cx: i64, cy: i64, half: i64, rotation_deg: f64) -> Shape {
    Shape::Rect {
        center: p(cx, cy),
        half_w: half * MM,
        half_h: half * MM,
        rotation_deg,
    }
}

#[test]
fn a_track_across_a_rect_overlaps_without_an_end_inside() {
    let pad = rect(0, 0, 1, 0.0);
    let across = Shape::Capsule {
        a: p(-5, 0),
        b: p(5, 0),
        radius: MM / 8,
    };
    assert!(across.overlaps(&pad) && pad.overlaps(&across));
}

#[test]
fn a_track_beside_a_rect_does_not() {
    let pad = rect(0, 0, 1, 0.0);
    let beside = Shape::Capsule {
        a: p(-5, 3),
        b: p(5, 3),
        radius: MM / 8,
    };
    assert!(!beside.overlaps(&pad));
}

#[test]
fn rects_overlap_off_centre_and_not_when_apart() {
    assert!(rect(0, 0, 1, 0.0).overlaps(&rect(1, 1, 1, 0.0)));
    assert!(!rect(0, 0, 1, 0.0).overlaps(&rect(3, 0, 1, 0.0)));
    // A rotated square's corner reaches 1.41 mm; the other's edge is at 2.
    assert!(rect(0, 0, 1, 45.0).overlaps(&rect(2, 0, 1, 0.0)));
    assert!(!rect(0, 0, 1, 45.0).overlaps(&rect(3, 0, 1, 0.0)));
}

#[test]
fn a_rect_swallowed_by_a_bigger_one_overlaps() {
    assert!(rect(0, 0, 5, 0.0).overlaps(&rect(0, 0, 1, 0.0)));
}

#[test]
fn discs_overlap_by_radius_sum() {
    let d = |x| Shape::Disc {
        center: p(x, 0),
        radius: MM,
    };
    assert!(d(0).overlaps(&d(2)));
    assert!(!d(0).overlaps(&d(3)));
}

#[test]
fn an_ellipse_overlaps_a_disc_on_its_long_axis_only() {
    let oval = Shape::Ellipse {
        center: p(0, 0),
        half_w: 3 * MM,
        half_h: MM,
        rotation_deg: 0.0,
    };
    let at = |x, y| Shape::Disc {
        center: p(x, y),
        radius: MM / 2,
    };
    assert!(oval.overlaps(&at(3, 0)));
    assert!(!oval.overlaps(&at(0, 3)));
}
