use super::*;

const MM: i64 = 1_000_000;

fn square(x0: i64, y0: i64, side: i64) -> Vec<PointNm> {
    vec![
        PointNm::new(x0, y0),
        PointNm::new(x0 + side, y0),
        PointNm::new(x0 + side, y0 + side),
        PointNm::new(x0, y0 + side),
    ]
}

/// A 10 mm square with a 4 mm hole in its middle.
fn holed() -> Polygon {
    Polygon::new(square(0, 0, 10 * MM), vec![square(3 * MM, 3 * MM, 4 * MM)])
}

fn disc(x: i64, y: i64, radius: i64) -> Shape {
    Shape::Disc {
        center: PointNm::new(x, y),
        radius,
    }
}

#[test]
fn copper_wholly_in_a_hole_does_not_meet_the_area() {
    assert!(!shape_meets_area(&disc(5 * MM, 5 * MM, MM), &holed()));
}

#[test]
fn copper_touching_a_hole_edge_or_on_the_copper_meets_it() {
    assert!(
        shape_meets_area(&disc(5 * MM, 5 * MM, 2 * MM), &holed()),
        "reaches the hole's edge"
    );
    assert!(
        shape_meets_area(&disc(MM, MM, MM / 2), &holed()),
        "on the copper"
    );
    assert!(
        !shape_meets_area(&disc(20 * MM, MM, MM), &holed()),
        "outside"
    );
}

#[test]
fn an_area_in_another_areas_hole_does_not_meet_it_unless_it_touches() {
    let inside = Polygon::new(square(4 * MM, 4 * MM, 2 * MM), Vec::new());
    assert!(!areas_meet(&inside, &holed()));
    assert!(!areas_meet(&holed(), &inside));
    let touching = Polygon::new(square(3 * MM, 4 * MM, 2 * MM), Vec::new());
    assert!(areas_meet(&touching, &holed()));
    let overlapping = Polygon::new(square(8 * MM, 8 * MM, 4 * MM), Vec::new());
    assert!(areas_meet(&overlapping, &holed()));
}
