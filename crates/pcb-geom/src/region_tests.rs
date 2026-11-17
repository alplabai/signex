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

fn solid(ring: Vec<PointNm>) -> Polygon {
    Polygon::new(ring, Vec::new())
}

fn total_twice_area(polygons: &[Polygon]) -> i128 {
    polygons.iter().map(Polygon::twice_area).sum()
}

#[test]
fn overlapping_squares_unite_into_one_polygon() {
    let a = solid(square(0, 0, 10 * MM));
    let b = solid(square(5 * MM, 5 * MM, 10 * MM));
    let united = union(&[a], &[b]);
    assert_eq!(united.len(), 1);
    assert!(united[0].holes.is_empty());
    let side = i128::from(10 * MM);
    let overlap = i128::from(5 * MM) * i128::from(5 * MM);
    assert_eq!(united[0].twice_area(), 2 * (2 * side * side - overlap));
}

#[test]
fn a_difference_inside_leaves_a_hole_wound_against_the_outer_ring() {
    let outer = solid(square(0, 0, 10 * MM));
    let inner = solid(square(4 * MM, 4 * MM, 2 * MM));
    let cut = difference(&[outer], &[inner]);
    assert_eq!(cut.len(), 1);
    assert_eq!(cut[0].holes.len(), 1);
    assert!(
        twice_signed_area(&cut[0].outer) > 0,
        "outer counter-clockwise"
    );
    assert!(twice_signed_area(&cut[0].holes[0]) < 0, "hole clockwise");
    let expected = 2 * (i128::from(10 * MM).pow(2) - i128::from(2 * MM).pow(2));
    assert_eq!(cut[0].twice_area(), expected);
}

#[test]
fn a_cut_across_splits_a_polygon_in_two() {
    let outer = solid(square(0, 0, 10 * MM));
    let bar = solid(vec![
        PointNm::new(4 * MM, -MM),
        PointNm::new(6 * MM, -MM),
        PointNm::new(6 * MM, 11 * MM),
        PointNm::new(4 * MM, 11 * MM),
    ]);
    assert_eq!(difference(&[outer], &[bar]).len(), 2);
}

#[test]
fn the_winding_of_an_input_does_not_matter() {
    let ccw = square(0, 0, 10 * MM);
    let cw: Vec<PointNm> = ccw.iter().rev().copied().collect();
    let hole = square(4 * MM, 4 * MM, 2 * MM);
    let hole_ccw = Polygon::new(ccw.clone(), vec![hole.clone()]);
    let hole_cw = Polygon::new(cw, vec![hole.iter().rev().copied().collect()]);
    let other = solid(square(8 * MM, 8 * MM, 4 * MM));
    assert_eq!(
        union(&[hole_ccw], std::slice::from_ref(&other)),
        union(&[hole_cw], &[other])
    );
}

#[test]
fn a_hole_stays_open_where_another_polygon_of_the_same_operand_is_absent() {
    let with_hole = Polygon::new(square(0, 0, 10 * MM), vec![square(4 * MM, 4 * MM, 2 * MM)]);
    let beside = solid(square(20 * MM, 0, MM));
    let both = union(&[with_hole, beside], &[]);
    assert_eq!(both.len(), 2);
    assert_eq!(both.iter().map(|p| p.holes.len()).sum::<usize>(), 1);
}

#[test]
fn intersection_keeps_only_the_shared_part() {
    let a = solid(square(0, 0, 10 * MM));
    let b = solid(square(5 * MM, 5 * MM, 10 * MM));
    let shared = intersection(&[a], &[b]);
    assert_eq!(total_twice_area(&shared), 2 * i128::from(5 * MM).pow(2));
}

#[test]
fn offset_grows_with_round_corners_and_shrinks_with_square_ones() {
    let side = 10 * MM;
    let by = MM;
    let grown = offset(&[solid(square(0, 0, side))], by);
    assert_eq!(grown.len(), 1);
    let bounds = grown[0].bounds().expect("bounds");
    assert_eq!(
        (bounds.min, bounds.max),
        (PointNm::new(-by, -by), PointNm::new(side + by, side + by))
    );
    // The square grown by 1 mm: the sides plus four quarter circles, the
    // arcs a little short of the true circle (chords).
    let exact = (side + 2 * by) as f64 * (side + 2 * by) as f64
        - (4.0 - std::f64::consts::PI) * (by as f64) * (by as f64);
    let area = grown[0].twice_area() as f64 / 2.0;
    assert!(
        area <= exact && exact - area < 0.001 * exact,
        "{area} vs {exact}"
    );

    let shrunk = offset(&[solid(square(0, 0, side))], -by);
    assert_eq!(shrunk.len(), 1);
    assert_eq!(shrunk[0].twice_area(), 2 * i128::from(side - 2 * by).pow(2));
}

#[test]
fn offset_shrinks_the_holes_of_what_it_grows() {
    let with_hole = Polygon::new(square(0, 0, 10 * MM), vec![square(3 * MM, 3 * MM, 4 * MM)]);
    let grown = offset(&[with_hole], MM / 2);
    assert_eq!(grown.len(), 1);
    assert_eq!(grown[0].holes.len(), 1);
    let hole = RectNm::bounding(grown[0].holes[0].iter().copied()).expect("hole");
    assert_eq!(
        (hole.min, hole.max),
        (
            PointNm::new(3 * MM + MM / 2, 3 * MM + MM / 2),
            PointNm::new(7 * MM - MM / 2, 7 * MM - MM / 2)
        )
    );
}

#[test]
fn offset_corners_stay_on_or_inside_the_true_arc_by_at_most_the_sag() {
    let by = 2 * MM;
    let grown = offset(&[solid(square(0, 0, 10 * MM))], by);
    let corner = PointNm::new(10 * MM, 10 * MM);
    for p in &grown[0].outer {
        if p.x > corner.x && p.y > corner.y {
            let d = (p.distance_sq(corner) as f64).sqrt();
            assert!(d <= by as f64 + 2.0, "vertex {p:?} at {d}");
            assert!(
                d >= (by - chord_sag(by)) as f64 - 2.0,
                "vertex {p:?} at {d}"
            );
        }
    }
    assert!(chord_sag(by) > 0 && chord_sag(by) < by / 500);
    assert_eq!(chord_sag(-5), 0);
}

/// #899: a point is in a polygon when it is in the outer ring and in no
/// hole; a hole's edge counts as the hole, a malformed hole as nothing.
#[test]
fn a_point_in_a_hole_is_not_in_the_polygon() {
    let p = |x: i64, y: i64| PointNm::new(x * MM, y * MM);
    let area = Polygon::new(square(0, 0, 10 * MM), vec![square(4 * MM, 4 * MM, 2 * MM)]);
    assert!(area.contains_point(p(2, 2)), "on the copper");
    assert!(area.contains_point(p(0, 5)), "on the outer edge");
    assert!(!area.contains_point(p(5, 5)), "in the hole");
    assert!(!area.contains_point(p(4, 5)), "on the hole's edge");
    assert!(!area.contains_point(p(11, 5)), "outside");
    let degenerate = Polygon::new(square(0, 0, 10 * MM), vec![vec![p(4, 4), p(6, 6)]]);
    assert!(
        degenerate.contains_point(p(5, 5)),
        "a two-point hole cuts nothing"
    );
    assert!(
        !Polygon::default().contains_point(p(0, 0)),
        "an empty polygon holds nothing"
    );
}
