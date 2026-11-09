use super::*;

const MM: i64 = 1_000_000;

fn p(x: i64, y: i64) -> PointNm {
    PointNm::new(x, y)
}

fn r(x0: i64, y0: i64, x1: i64, y1: i64) -> RectNm {
    RectNm::new(p(x0, y0), p(x1, y1))
}

fn track() -> Shape {
    Shape::Capsule {
        a: p(0, 0),
        b: p(10 * MM, 0),
        radius: MM / 2,
    }
}

#[test]
fn disc_contains_up_to_its_radius_plus_tolerance() {
    let disc = Shape::Disc {
        center: p(0, 0),
        radius: 300,
    };
    assert!(disc.contains(p(300, 0), 0));
    assert!(!disc.contains(p(301, 0), 0));
    assert!(disc.contains(p(301, 0), 1));
    assert!(!disc.contains(p(301, 0), -5));
    assert_eq!(disc.aabb(), r(-300, -300, 300, 300));
}

#[test]
fn capsule_measures_to_the_segment_not_its_line() {
    let t = track();
    assert!(t.contains(p(5 * MM, MM / 2), 0));
    assert!(!t.contains(p(5 * MM, MM / 2 + 1), 0));
    // Past the end the round cap applies.
    assert!(t.contains(p(10 * MM + MM / 2, 0), 0));
    assert!(!t.contains(p(10 * MM + 400_000, 400_000), 0));
    assert_eq!(t.aabb(), r(-MM / 2, -MM / 2, 10 * MM + MM / 2, MM / 2));
}

#[test]
fn diagonal_capsule_rounds_the_foot_within_a_nanometre() {
    let t = Shape::Capsule {
        a: p(0, 0),
        b: p(3 * MM, 4 * MM),
        radius: MM,
    };
    // The midpoint (1.5, 2) mm plus 1 mm along the unit normal (0.8, -0.6).
    let on_edge = p(1_500_000 + 800_000, 2_000_000 - 600_000);
    assert!(t.contains(on_edge, 1));
    assert!(!t.contains(p(1_500_000 + 801_000, 2_000_000 - 601_000), 0));
}

#[test]
fn zero_length_capsule_is_a_disc() {
    let t = Shape::Capsule {
        a: p(5, 5),
        b: p(5, 5),
        radius: 10,
    };
    assert!(t.contains(p(15, 5), 0));
    assert!(!t.contains(p(16, 5), 0));
}

#[test]
fn rotated_rect_hit_and_bounds() {
    let pad = Shape::Rect {
        center: p(0, 0),
        half_w: 2 * MM,
        half_h: MM / 2,
        rotation_deg: 90.0,
    };
    assert!(pad.contains(p(0, 2 * MM), 0));
    assert!(!pad.contains(p(2 * MM, 0), 0));
    assert_eq!(pad.aabb(), r(-MM / 2, -2 * MM, MM / 2, 2 * MM));
    let tilted = Shape::Rect {
        center: p(0, 0),
        half_w: MM,
        half_h: MM,
        rotation_deg: 45.0,
    };
    // The 45° square's corner sits on the x axis at √2 mm.
    assert!(tilted.contains(p(1_414_000, 0), 0));
    assert!(!tilted.contains(p(1_000_000, 1_000_000), 0));
    let aabb = tilted.aabb();
    assert!((aabb.max.x - 1_414_214).abs() <= 1);
}

#[test]
fn round_rect_hit_and_bounds() {
    let rr = |rotation_deg| Shape::RoundRect {
        center: p(0, 0),
        half_w: 2 * MM,
        half_h: MM,
        radius: MM / 2,
        rotation_deg,
    };
    assert!(rr(0.0).contains(p(2 * MM, 0), 0));
    assert!(rr(0.0).contains(p(0, MM), 0));
    // The square corner is cut off: inside the box, outside the arc.
    assert!(!rr(0.0).contains(p(2 * MM - 10_000, MM - 10_000), 0));
    assert_eq!(rr(0.0).aabb(), r(-2 * MM, -MM, 2 * MM, MM));
    assert_eq!(rr(90.0).aabb(), r(-MM, -2 * MM, MM, 2 * MM));
    assert!(rr(90.0).contains(p(0, 2 * MM), 0));
    assert!(!rr(90.0).contains(p(2 * MM, 0), 0));
}

#[test]
fn disc_touches_rect_by_its_nearest_point() {
    let disc = Shape::Disc {
        center: p(0, 0),
        radius: 100,
    };
    assert!(disc.touches_rect(&r(100, -5, 200, 5)));
    assert!(!disc.touches_rect(&r(71, 71, 200, 200)));
    assert!(disc.touches_rect(&r(70, 70, 200, 200)));
}

#[test]
fn capsule_touches_rect_crossing_inside_or_near() {
    let t = track();
    // Crossing through without an endpoint inside.
    assert!(t.touches_rect(&r(4 * MM, -3 * MM, 6 * MM, 3 * MM)));
    // Wholly inside.
    assert!(t.touches_rect(&r(-MM, -MM, 11 * MM, MM)));
    // Only the copper edge reaches it.
    assert!(t.touches_rect(&r(4 * MM, MM / 2, 6 * MM, MM)));
    assert!(!t.touches_rect(&r(4 * MM, MM / 2 + 1, 6 * MM, MM)));
    // Only the end cap reaches it.
    assert!(t.touches_rect(&r(10 * MM + MM / 2, -MM, 12 * MM, MM)));
    assert!(!t.touches_rect(&r(10 * MM + 400_000, 400_000, 12 * MM, MM)));
}

#[test]
fn rotated_rect_touches_rect_by_separating_axes() {
    let tilted = Shape::Rect {
        center: p(0, 0),
        half_w: MM,
        half_h: MM,
        rotation_deg: 45.0,
    };
    // Inside the AABB corner but outside the diamond.
    assert!(!tilted.touches_rect(&r(800_000, 800_000, 2 * MM, 2 * MM)));
    assert!(tilted.touches_rect(&r(700_000, 700_000, 2 * MM, 2 * MM)));
    assert!(tilted.touches_rect(&r(-10, -10, 10, 10)));
}
