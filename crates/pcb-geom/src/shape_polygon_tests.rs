//! The polygon pad's ring and convex pieces (#2242).

use super::*;

fn p(x: i64, y: i64) -> PointNm {
    PointNm::new(x, y)
}

fn twice_area(ring: &[PointNm]) -> i128 {
    (0..ring.len())
        .map(|i| ring[i].cross(ring[(i + 1) % ring.len()]))
        .sum()
}

#[test]
fn a_clockwise_ring_is_turned_counter_clockwise() {
    let clockwise = vec![p(0, 0), p(0, 10), p(10, 10), p(10, 0)];
    let ring = ccw_ring(clockwise);
    assert!(twice_area(&ring) > 0);
    assert_eq!(ring.len(), 4);
}

#[test]
fn a_convex_ring_is_one_piece_without_its_straight_corners() {
    let square = [p(0, 0), p(5, 0), p(10, 0), p(10, 10), p(0, 10)];
    assert_eq!(
        convex_pieces(&square),
        vec![vec![p(0, 0), p(10, 0), p(10, 10), p(0, 10)]]
    );
}

#[test]
fn a_concave_ring_is_triangles_that_cover_its_area_exactly() {
    let l = [p(0, 0), p(10, 0), p(10, 4), p(4, 4), p(4, 10), p(0, 10)];
    let pieces = convex_pieces(&l);
    assert_eq!(pieces.len(), 4);
    let covered: i128 = pieces.iter().map(|t| twice_area(t)).sum();
    assert_eq!(covered, twice_area(&l));
    assert!(pieces.iter().all(|t| twice_area(t) > 0));
}

#[test]
fn a_ring_with_no_area_has_no_pieces() {
    assert!(convex_pieces(&[p(0, 0), p(5, 0), p(10, 0)]).is_empty());
    assert!(convex_pieces(&[]).is_empty());
}

#[test]
fn a_grown_ring_covers_the_polygon_by_at_least_the_radius() {
    let square = [p(0, 0), p(100, 0), p(100, 100), p(0, 100)];
    let grown = polygon_covering_ring(&square, 20);
    let bounds = crate::rect::RectNm::bounding(grown.iter().copied()).unwrap();
    assert!(bounds.min.x <= -20 && bounds.max.x >= 120, "{bounds:?}");
    assert_eq!(polygon_covering_ring(&square, 0), square.to_vec());
}
