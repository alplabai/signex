use super::*;
use uuid::Uuid;

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
    vec![
        Point::new(x0, y0),
        Point::new(x1, y0),
        Point::new(x1, y1),
        Point::new(x0, y1),
    ]
}

/// A 10 x 10 mm pour from the origin.
fn pour() -> Zone {
    Zone {
        uuid: Uuid::from_u128(1),
        net: 1,
        net_name: "GND".into(),
        layer: "TopCopper".into(),
        outline: rect(0.0, 0.0, 10.0, 10.0),
        holes: Vec::new(),
        priority: 0,
        fill_type: "pour".into(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.0,
        min_thickness: 0.0,
        min_island_area: 0.0,
        remove_dead_islands: false,
        shelved: false,
        unknown: Default::default(),
    }
}

/// The area of `shape` in mm², from its rings.
fn area_mm2(shape: &AreaShape) -> f64 {
    let ring = |r: &[Point]| {
        let n = r.len();
        ((0..n)
            .map(|i| r[i].x * r[(i + 1) % n].y - r[(i + 1) % n].x * r[i].y)
            .sum::<f64>()
            / 2.0)
            .abs()
    };
    ring(&shape.outline) - shape.holes.iter().map(|h| ring(h)).sum::<f64>()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn a_cutout_inside_the_pour_is_a_hole() {
    let left = cut_area(&pour(), &rect(4.0, 4.0, 6.0, 6.0)).expect("it overlaps");
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].holes.len(), 1);
    assert!(close(area_mm2(&left[0]), 96.0), "{}", area_mm2(&left[0]));
}

#[test]
fn a_cutout_over_the_edge_notches_the_outline() {
    let left = cut_area(&pour(), &rect(8.0, 4.0, 12.0, 6.0)).expect("it overlaps");
    assert_eq!(left.len(), 1);
    assert!(left[0].holes.is_empty());
    assert!(close(area_mm2(&left[0]), 96.0));
}

#[test]
fn a_cutout_across_the_pour_leaves_two_pieces_largest_first() {
    let left = cut_area(&pour(), &rect(3.0, -1.0, 5.0, 11.0)).expect("it overlaps");
    assert_eq!(left.len(), 2);
    assert!(close(area_mm2(&left[0]), 50.0));
    assert!(close(area_mm2(&left[1]), 30.0));
}

#[test]
fn a_cutout_off_the_pour_changes_nothing_and_one_over_it_leaves_none() {
    assert_eq!(cut_area(&pour(), &rect(20.0, 0.0, 22.0, 2.0)), None);
    assert_eq!(
        cut_area(&pour(), &rect(-1.0, -1.0, 11.0, 11.0)),
        Some(Vec::new())
    );
    assert_eq!(
        cut_area(&pour(), &[Point::new(1.0, 1.0), Point::new(2.0, 2.0)]),
        None,
        "two points are no area"
    );
}

/// A cutout keeps the holes the pour had.
#[test]
fn a_cutout_keeps_the_pours_holes() {
    let mut zone = pour();
    zone.holes = vec![rect(1.0, 1.0, 2.0, 2.0)];
    let left = cut_area(&zone, &rect(8.0, 8.0, 9.0, 9.0)).expect("it overlaps");
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].holes.len(), 2);
    assert!(close(area_mm2(&left[0]), 98.0));
}

#[test]
fn a_line_across_the_pour_slices_it_in_two() {
    let pieces = slice_area(&pour(), Point::new(4.0, -1.0), Point::new(4.0, 11.0))
        .expect("the line crosses it");
    assert_eq!(pieces.len(), 2);
    assert!(close(area_mm2(&pieces[0]), 60.0));
    assert!(close(area_mm2(&pieces[1]), 40.0));
    // The pieces meet along the line.
    for piece in &pieces {
        assert!(piece.outline.iter().any(|p| close(p.x, 4.0)));
    }
}

#[test]
fn a_slanted_line_slices_too() {
    let pieces = slice_area(&pour(), Point::new(0.0, 0.0), Point::new(10.0, 10.0))
        .expect("the diagonal crosses it");
    assert_eq!(pieces.len(), 2);
    assert!(close(area_mm2(&pieces[0]), 50.0));
    assert!(close(area_mm2(&pieces[1]), 50.0));
}

#[test]
fn a_line_that_misses_the_pour_or_is_a_point_slices_nothing() {
    let zone = pour();
    assert_eq!(
        slice_area(&zone, Point::new(20.0, 0.0), Point::new(20.0, 10.0)),
        None
    );
    assert_eq!(
        slice_area(&zone, Point::new(5.0, 5.0), Point::new(5.0, 5.0)),
        None
    );
    // Along an edge, the whole pour is on one side.
    assert_eq!(
        slice_area(&zone, Point::new(0.0, -1.0), Point::new(0.0, 11.0)),
        None
    );
}
