//! Filling a closed ring with holes cut out of it: the one place that
//! decides whether an ear clip with holes filled the area it should, and
//! which holes may be cut together. The flat polygon pipeline and the 3D
//! mesh (`scene3d`) both fill through it, so the two cannot disagree.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: signex-sketch geom toolkit (ear clip, even-odd point test).

use rstar::primitives::{GeomWithData, Rectangle};
use rstar::{AABB, RTree};
use signex_sketch::geom::{
    Point2, Segment2, SegmentIntersection, ear_clip_with_holes, point_in_polygon,
    segment_segment_intersection, signed_area,
};

/// A ring point in the geometry toolkit's type.
fn to_point(p: &[f32; 2]) -> Point2 {
    Point2::new(f64::from(p[0]), f64::from(p[1]))
}

fn to_points(ring: &[[f32; 2]]) -> Vec<Point2> {
    ring.iter().map(to_point).collect()
}

/// Triangles filling `outline` minus `holes`: the points and the index
/// triples into them. Holes under 3 points are skipped.
///
/// `None` when the triangles do not add up to the outline less the holes
/// (degenerate, overlapping or outline-touching holes, or a non-finite
/// point); the caller then fills the outline whole, so the shape is never
/// left invisible. The area check is a heuristic: on valid input a wrong
/// fill does not pass it, but a self-intersecting outline with the right
/// total area could.
pub fn triangulate_with_holes(
    outline: &[[f32; 2]],
    holes: &[Vec<[f32; 2]>],
) -> Option<(Vec<[f32; 2]>, Vec<[u32; 3]>)> {
    if outline.len() < 3 {
        return None;
    }
    let outer = to_points(outline);
    let rings: Vec<Vec<Point2>> = holes
        .iter()
        .filter(|hole| hole.len() >= 3)
        .map(|hole| to_points(hole))
        .collect();
    let (merged, triangles) = ear_clip_with_holes(&outer, &rings);
    let expected =
        signed_area(&outer).abs() - rings.iter().map(|h| signed_area(h).abs()).sum::<f64>();
    let filled: f64 = triangles
        .iter()
        .map(|&[a, b, c]| signed_area(&[merged[a], merged[b], merged[c]]).abs())
        .sum();
    // A non-finite vertex makes every comparison false, so it would pass the
    // area check unnoticed; treat it as a failed fill.
    if triangles.is_empty()
        || !filled.is_finite()
        || !expected.is_finite()
        || (filled - expected).abs() > expected.abs() * 1e-6 + 1e-9
    {
        return None;
    }
    let points = merged.iter().map(|p| [p.x as f32, p.y as f32]).collect();
    let triangles = triangles
        .into_iter()
        .map(|t| [t[0] as u32, t[1] as u32, t[2] as u32])
        .collect();
    Some((points, triangles))
}

/// Keep only the holes whose every point lies inside `outline` (even-odd,
/// `signex_sketch::geom::point_in_polygon`): a hole off the shape or across
/// its edge would spoil the fill.
pub fn retain_holes_inside(outline: &[[f32; 2]], holes: &mut Vec<Vec<[f32; 2]>>) {
    let outer = to_points(outline);
    holes.retain(|hole| hole.iter().all(|p| point_in_polygon(to_point(p), &outer)));
}

/// `holes` less every hole that crosses, touches, contains or lies inside
/// a hole kept before it, and every hole with a non-finite point. Holes
/// that meet make [`triangulate_with_holes`] refuse the whole fill, so one
/// bad pair would cost every hole. Holes under 3 points are kept (the fill
/// skips them). Returns the kept holes, in their order, and how many were
/// dropped.
pub fn disjoint_holes(holes: Vec<Vec<[f32; 2]>>) -> (Vec<Vec<[f32; 2]>>, usize) {
    let mut index: RTree<GeomWithData<Rectangle<[f32; 2]>, usize>> = RTree::new();
    let mut kept: Vec<Vec<[f32; 2]>> = Vec::with_capacity(holes.len());
    let mut dropped = 0;
    for hole in holes {
        if hole.len() < 3 {
            kept.push(hole);
            continue;
        }
        let Some((lo, hi)) = extent(&hole) else {
            dropped += 1;
            continue;
        };
        let meets = index
            .locate_in_envelope_intersecting(&AABB::from_corners(lo, hi))
            .any(|other| rings_meet(&kept[other.data], &hole));
        if meets {
            dropped += 1;
            continue;
        }
        index.insert(GeomWithData::new(
            Rectangle::from_corners(lo, hi),
            kept.len(),
        ));
        kept.push(hole);
    }
    (kept, dropped)
}

/// The corners of the box round `ring`; `None` when a point is not finite.
fn extent(ring: &[[f32; 2]]) -> Option<([f32; 2], [f32; 2])> {
    let mut out: Option<([f32; 2], [f32; 2])> = None;
    for p in ring {
        if !(p[0].is_finite() && p[1].is_finite()) {
            return None;
        }
        out = Some(match out {
            None => (*p, *p),
            Some((lo, hi)) => (
                [lo[0].min(p[0]), lo[1].min(p[1])],
                [hi[0].max(p[0]), hi[1].max(p[1])],
            ),
        });
    }
    out
}

/// Whether two closed rings share any point: an edge of one meets an edge
/// of the other, or one lies inside the other.
fn rings_meet(a: &[[f32; 2]], b: &[[f32; 2]]) -> bool {
    let (a, b) = (to_points(a), to_points(b));
    let edges = |ring: &[Point2]| -> Vec<Segment2> {
        (0..ring.len())
            .map(|i| Segment2::new(ring[i], ring[(i + 1) % ring.len()]))
            .collect()
    };
    let (edges_a, edges_b) = (edges(&a), edges(&b));
    let crossing = edges_a.iter().any(|&e| {
        edges_b
            .iter()
            .any(|&f| segment_segment_intersection(e, f) != SegmentIntersection::None)
    });
    crossing || point_in_polygon(a[0], &b) || point_in_polygon(b[0], &a)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f32, y: f32, side: f32) -> Vec<[f32; 2]> {
        vec![[x, y], [x + side, y], [x + side, y + side], [x, y + side]]
    }

    const OUTLINE: [[f32; 2]; 4] = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];

    #[test]
    fn a_fill_with_a_hole_leaves_the_hole_empty() {
        let (points, triangles) =
            triangulate_with_holes(&OUTLINE, &[square(4.0, 4.0, 2.0)]).expect("a fill");
        let area: f64 = triangles
            .iter()
            .map(|t| signed_area(&t.map(|i| to_point(&points[i as usize]))).abs())
            .sum();
        assert!((area - 96.0).abs() < 1e-6, "{area}");
    }

    #[test]
    fn overlapping_holes_refuse_the_fill() {
        let holes = [square(2.0, 2.0, 3.0), square(3.0, 3.0, 3.0)];
        assert_eq!(triangulate_with_holes(&OUTLINE, &holes), None);
    }

    /// Two coincident holes, one inside another and one crossing another:
    /// the later of each pair goes, the rest stay, and the fill then cuts
    /// every hole that is left.
    #[test]
    fn holes_that_meet_a_kept_hole_are_dropped() {
        let holes = vec![
            square(1.0, 1.0, 1.0),
            square(1.0, 1.0, 1.0),
            square(4.0, 4.0, 3.0),
            square(5.0, 5.0, 1.0),
            square(6.5, 1.0, 1.0),
            square(7.0, 1.5, 1.0),
        ];
        let (kept, dropped) = disjoint_holes(holes);
        assert_eq!(dropped, 3);
        assert_eq!(
            kept,
            vec![
                square(1.0, 1.0, 1.0),
                square(4.0, 4.0, 3.0),
                square(6.5, 1.0, 1.0)
            ]
        );
        assert!(triangulate_with_holes(&OUTLINE, &kept).is_some());
    }

    #[test]
    fn a_hole_with_a_non_finite_point_is_dropped() {
        let mut bad = square(1.0, 1.0, 1.0);
        bad[2][0] = f32::NAN;
        let (kept, dropped) = disjoint_holes(vec![bad, square(4.0, 4.0, 1.0)]);
        assert_eq!((kept.len(), dropped), (1, 1));
    }

    #[test]
    fn only_holes_wholly_inside_the_outline_are_kept() {
        let mut holes = vec![square(4.0, 4.0, 1.0), square(9.5, 4.0, 1.0)];
        retain_holes_inside(&OUTLINE, &mut holes);
        assert_eq!(holes, vec![square(4.0, 4.0, 1.0)]);
    }
}
