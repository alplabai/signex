//! The polygon pad shape (#2242): a trapezoid or a custom outline as a
//! simple counter-clockwise ring, and the convex pieces the overlap and gap
//! tests read it as.

use std::sync::Arc;

use crate::region::{Polygon as Area, chord_sag, offset};
use crate::shape::orientation;
use crate::shape_overlap::inside_convex;
use crate::units::PointNm;

/// `corners` as a counter-clockwise ring, whichever way they wind.
pub fn ccw_ring(mut corners: Vec<PointNm>) -> Arc<[PointNm]> {
    let n = corners.len();
    let twice_area: i128 = (0..n).map(|i| corners[i].cross(corners[(i + 1) % n])).sum();
    if twice_area < 0 {
        corners.reverse();
    }
    corners.into()
}

/// A ring holding every point within `radius` of the polygon `ring`: the
/// ring itself for radius 0, else its outward offset grown by the chord sag
/// of the offset's arcs and 2 nm for rounding, so a clearance cut with it
/// never comes closer than the true shape allows.
pub(crate) fn polygon_covering_ring(ring: &[PointNm], radius: i64) -> Vec<PointNm> {
    if radius <= 0 {
        return ring.to_vec();
    }
    let by = radius + chord_sag(radius) + 2;
    offset(&[Area::new(ring.to_vec(), Vec::new())], by)
        .into_iter()
        .max_by_key(Area::twice_area)
        .map(|area| area.outer)
        .unwrap_or_else(|| ring.to_vec())
}

/// The convex pieces of a simple counter-clockwise ring: the ring itself
/// when it is convex, else the triangles of an ear clipping. Integer nm,
/// so the pieces cover the ring exactly. Empty for a ring with no area.
pub(crate) fn convex_pieces(ring: &[PointNm]) -> Vec<Vec<PointNm>> {
    let mut pts: Vec<PointNm> = ring.to_vec();
    pts.dedup();
    if pts.len() > 1 && pts.first() == pts.last() {
        pts.pop();
    }
    drop_straight_corners(&mut pts);
    if pts.len() < 3 {
        return Vec::new();
    }
    if is_convex(&pts) {
        return vec![pts];
    }
    let mut pieces = Vec::with_capacity(pts.len() - 2);
    while pts.len() > 3 {
        let n = pts.len();
        let Some(i) = (0..n).find(|&i| is_ear(&pts, i)) else {
            // Only a ring that touches itself has no ear: keep the rest
            // whole rather than lose copper.
            break;
        };
        pieces.push(vec![pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]]);
        pts.remove(i);
        drop_straight_corners(&mut pts);
    }
    if pts.len() >= 3 {
        pieces.push(pts);
    }
    pieces
}

/// Remove corners that lie straight between their neighbours.
fn drop_straight_corners(pts: &mut Vec<PointNm>) {
    while pts.len() >= 3 {
        let n = pts.len();
        let straight =
            (0..n).find(|&i| orientation(pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]) == 0);
        match straight {
            Some(i) => {
                pts.remove(i);
            }
            None => break,
        }
    }
}

fn is_convex(pts: &[PointNm]) -> bool {
    let n = pts.len();
    (0..n).all(|i| orientation(pts[i], pts[(i + 1) % n], pts[(i + 2) % n]) >= 0)
}

/// Whether corner `i` of a counter-clockwise ring is an ear: it turns left
/// and no other corner lies in or on the triangle it cuts off.
fn is_ear(pts: &[PointNm], i: usize) -> bool {
    let n = pts.len();
    let (prev, next) = ((i + n - 1) % n, (i + 1) % n);
    let triangle = [pts[prev], pts[i], pts[next]];
    orientation(triangle[0], triangle[1], triangle[2]) > 0
        && (0..n)
            .filter(|&j| j != prev && j != i && j != next)
            .all(|j| !inside_convex(&triangle, pts[j]))
}

#[cfg(test)]
#[path = "shape_polygon_tests.rs"]
mod tests;
