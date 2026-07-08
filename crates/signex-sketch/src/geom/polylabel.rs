//! Pole of inaccessibility — finds the point inside a polygon
//! that is furthest from any polygon edge. Useful for placing pad
//! designators / labels at the visual centre of irregular shapes.
//!
//! Algorithm — quadtree subdivision keyed on a best-case bound:
//!
//! 1. Seed a max-priority queue with the polygon's bounding box
//!    cell and its centre.
//! 2. Repeatedly pop the cell whose `distance + half_diagonal` is
//!    largest (the upper bound on any future cell's centre
//!    distance after subdivision).
//! 3. If `distance` is better than the running best, update best.
//! 4. If the upper bound exceeds best by more than the precision,
//!    subdivide the cell into 4 children and push each.
//! 5. Terminate when the queue is empty or the next upper bound
//!    is within `precision` of the running best.
//!
//! O(n²) worst-case in the polygon vertex count for the
//! point-to-polygon distance step; the recursion depth is
//! bounded by `log2(bbox / precision)` so it converges quickly
//! for typical sketch polygons.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use super::Point2;
use super::predicates::DEFAULT_TOL;

/// Centre point + half-diagonal for one quadtree cell, plus the
/// signed distance from `centre` to the polygon. Stored on the
/// priority queue so the next cell to subdivide is always the
/// one with the highest possible future improvement.
#[derive(Debug, Clone, Copy)]
struct Cell {
    centre: Point2,
    half: f64,
    distance: f64,
    /// `distance + half * sqrt(2)` — the best possible distance
    /// any descendant of this cell could achieve.
    upper_bound: f64,
}

impl PartialEq for Cell {
    fn eq(&self, other: &Self) -> bool {
        self.upper_bound == other.upper_bound
    }
}

impl Eq for Cell {}

impl PartialOrd for Cell {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Cell {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max-heap; we want the cell with the
        // largest upper_bound on top.
        self.upper_bound
            .partial_cmp(&other.upper_bound)
            .unwrap_or(Ordering::Equal)
    }
}

impl Cell {
    fn new(centre: Point2, half: f64, polygon: &[Point2]) -> Self {
        let distance = signed_distance_to_polygon(centre, polygon);
        let upper_bound = distance + half * std::f64::consts::SQRT_2;
        Self {
            centre,
            half,
            distance,
            upper_bound,
        }
    }
}

/// Find the pole of inaccessibility — the point inside the
/// polygon with the maximum minimum-distance-to-edge. Returns
/// `None` for fewer than three vertices.
///
/// `precision` controls when subdivision stops. Smaller =
/// better label position but more work; `0.5 mm` is plenty for
/// PCB designator placement on typical pads.
pub fn pole_of_inaccessibility(polygon: &[Point2], precision: f64) -> Option<Point2> {
    if polygon.len() < 3 {
        return None;
    }
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for p in polygon {
        if p.x < min_x {
            min_x = p.x;
        }
        if p.y < min_y {
            min_y = p.y;
        }
        if p.x > max_x {
            max_x = p.x;
        }
        if p.y > max_y {
            max_y = p.y;
        }
    }
    let width = max_x - min_x;
    let height = max_y - min_y;
    let cell_size = width.min(height);
    if cell_size <= DEFAULT_TOL {
        return None;
    }
    let half = cell_size / 2.0;

    // Seed: bounding-box-centre cell + centroid as a starting
    // best candidate (the centroid is often near optimal for
    // convex polygons so it gives the queue a good lower bound).
    let mut best = Cell::new(
        Point2::new(min_x + width / 2.0, min_y + height / 2.0),
        0.0,
        polygon,
    );
    let centroid = polygon_centroid(polygon);
    let centroid_cell = Cell::new(centroid, 0.0, polygon);
    if centroid_cell.distance > best.distance {
        best = centroid_cell;
    }

    let mut queue: BinaryHeap<Cell> = BinaryHeap::new();
    let mut x = min_x;
    while x < max_x {
        let mut y = min_y;
        while y < max_y {
            let cell = Cell::new(Point2::new(x + half, y + half), half, polygon);
            queue.push(cell);
            y += cell_size;
        }
        x += cell_size;
    }

    let precision = precision.max(DEFAULT_TOL);
    while let Some(cell) = queue.pop() {
        if cell.distance > best.distance {
            best = cell;
        }
        if cell.upper_bound - best.distance <= precision {
            continue;
        }
        let h = cell.half / 2.0;
        for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            let centre = Point2::new(cell.centre.x + sx * h, cell.centre.y + sy * h);
            queue.push(Cell::new(centre, h, polygon));
        }
    }

    Some(best.centre)
}

/// Centroid of a polygon (average of vertex coordinates). Not the
/// same as the centroid-of-area but adequate as a starting seed
/// for the polylabel search.
fn polygon_centroid(polygon: &[Point2]) -> Point2 {
    let n = polygon.len() as f64;
    let sx: f64 = polygon.iter().map(|p| p.x).sum();
    let sy: f64 = polygon.iter().map(|p| p.y).sum();
    Point2::new(sx / n, sy / n)
}

/// Signed distance from `p` to the polygon — positive inside,
/// negative outside, zero on the boundary. Magnitude equals the
/// shortest distance to any polygon edge.
fn signed_distance_to_polygon(p: Point2, polygon: &[Point2]) -> f64 {
    let inside = point_in_polygon(p, polygon);
    let mut min_d = f64::INFINITY;
    let n = polygon.len();
    for i in 0..n {
        let a = polygon[i];
        let b = polygon[(i + 1) % n];
        let d = point_to_segment_distance(p, a, b);
        if d < min_d {
            min_d = d;
        }
    }
    if inside { min_d } else { -min_d }
}

fn point_in_polygon(p: Point2, polygon: &[Point2]) -> bool {
    let mut inside = false;
    let n = polygon.len();
    let mut j = n - 1;
    for i in 0..n {
        let xi = polygon[i].x;
        let yi = polygon[i].y;
        let xj = polygon[j].x;
        let yj = polygon[j].y;
        let denom = yj - yi;
        if denom.abs() < 1e-10 {
            j = i;
            continue;
        }
        let intersect = ((yi > p.y) != (yj > p.y)) && (p.x < (xj - xi) * (p.y - yi) / denom + xi);
        if intersect {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn point_to_segment_distance(p: Point2, a: Point2, b: Point2) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-12 {
        return ((p.x - a.x).powi(2) + (p.y - a.y).powi(2)).sqrt();
    }
    let t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let qx = a.x + t * dx;
    let qy = a.y + t * dy;
    ((p.x - qx).powi(2) + (p.y - qy).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    fn close(a: Point2, b: Point2, tol: f64) -> bool {
        (a.x - b.x).abs() < tol && (a.y - b.y).abs() < tol
    }

    #[test]
    fn empty_polygon_returns_none() {
        assert!(pole_of_inaccessibility(&[], 0.1).is_none());
        assert!(pole_of_inaccessibility(&[p(0.0, 0.0)], 0.1).is_none());
    }

    #[test]
    fn unit_square_pole_at_centre() {
        let sq = [p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
        let pole = pole_of_inaccessibility(&sq, 0.001).unwrap();
        assert!(close(pole, p(0.5, 0.5), 1e-2));
    }

    #[test]
    fn rectangle_pole_at_centre() {
        let r = [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 1.0), p(0.0, 1.0)];
        let pole = pole_of_inaccessibility(&r, 0.001).unwrap();
        // Pole sits anywhere on the central horizontal axis at y=0.5,
        // far from both long edges. Verify y is at the midline.
        assert!((pole.y - 0.5).abs() < 1e-2);
    }

    #[test]
    fn l_shape_pole_in_thicker_arm() {
        // L-shape:
        //   (0,0) - (3,0) - (3,1) - (1,1) - (1,3) - (0,3)
        // The horizontal arm is 3x1 (x: 0..3, y: 0..1).
        // The vertical arm is 1x3 (x: 0..1, y: 0..3).
        // Both arms have equal "thickness" (1 unit) — the pole
        // should land at one of the arm centres or near the
        // junction. Verify at minimum that the pole lies INSIDE
        // the L and not on a coincident edge.
        let l = [
            p(0.0, 0.0),
            p(3.0, 0.0),
            p(3.0, 1.0),
            p(1.0, 1.0),
            p(1.0, 3.0),
            p(0.0, 3.0),
        ];
        let pole = pole_of_inaccessibility(&l, 0.001).unwrap();
        assert!(point_in_polygon(pole, &l), "pole must lie inside polygon");
        // Distance from pole to nearest edge should be > 0.4 (i.e.
        // roughly the half-arm-thickness).
        let d = signed_distance_to_polygon(pole, &l);
        assert!(
            d > 0.4,
            "pole distance {d} should be > 0.4 for L with arm 1"
        );
    }
}
