//! Pairs of shapes within reach of each other, for the checks that compare
//! objects the copper index does not hold (holes, silkscreen, mask
//! openings, courtyards): a sweep along x over their bounds, so the cost
//! follows what is near each shape, not the square of the count.

use pcb_geom::{Separation, Shape};

/// Every pair `(i, j)`, `i < j`, whose bounds come within `reach` nm, for
/// which `wanted(i, j)` holds. Each pair once.
pub(crate) fn near_pairs(
    shapes: &[Shape],
    reach: i64,
    mut wanted: impl FnMut(usize, usize) -> bool,
) -> Vec<(usize, usize)> {
    let bounds: Vec<_> = shapes.iter().map(|s| s.aabb().inflate(reach)).collect();
    let mut order: Vec<usize> = (0..shapes.len()).collect();
    order.sort_unstable_by_key(|&i| bounds[i].min.x);
    let mut out = Vec::new();
    for (k, &i) in order.iter().enumerate() {
        for &j in &order[k + 1..] {
            if bounds[j].min.x > bounds[i].max.x {
                break;
            }
            let meet = bounds[i].min.y <= bounds[j].max.y && bounds[j].min.y <= bounds[i].max.y;
            if meet && wanted(i.min(j), i.max(j)) {
                out.push((i.min(j), i.max(j)));
            }
        }
    }
    out
}

/// How close `a` and `b` come: `None` when they are at least `limit` nm
/// apart; `Some(0)` when they touch or overlap; else the gap, under
/// `limit`, with the point between them.
pub(crate) fn gap_under(a: &Shape, b: &Shape, limit: i64) -> Option<(i64, pcb_geom::PointNm)> {
    if a.overlaps(b) {
        return Some((0, overlap_point(a, b)));
    }
    let gap: Separation = a.separation(b);
    gap.is_under(limit)
        .then(|| (gap.distance_nm().min(limit - 1).max(0), gap.midpoint()))
}

/// Where two overlapping shapes meet, near enough to point at: the middle
/// of where their bounds meet.
fn overlap_point(a: &Shape, b: &Shape) -> pcb_geom::PointNm {
    let (ra, rb) = (a.aabb(), b.aabb());
    let lo_x = ra.min.x.max(rb.min.x);
    let lo_y = ra.min.y.max(rb.min.y);
    let hi_x = ra.max.x.min(rb.max.x);
    let hi_y = ra.max.y.min(rb.max.y);
    pcb_geom::PointNm::new(lo_x + (hi_x - lo_x) / 2, lo_y + (hi_y - lo_y) / 2)
}
