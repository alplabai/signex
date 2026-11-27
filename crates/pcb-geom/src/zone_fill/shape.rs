//! A pour's corners (#2578) and hatch (#2579), applied to its fill.
//!
//! Corners shape the convex corners of the pour's outline only: a concave
//! corner, a hole drawn in the pour and every cut-out around other nets'
//! copper keep the shape the outline and the clearance give them, so the
//! fill never reaches past the outline nor nearer other copper than the
//! clearance. A hatch is a square grid of lines, aligned to the board
//! origin, inside a border that follows every edge of the fill. Rules:
//! `docs/PCB_TOOLS.md`, "Polygon Pour".

use signex_types::pcb::Point;
use signex_types::zone_pour_shape::{PourCorners, PourHatch};

use crate::region::{OFFSET_ARC_POINTS, Polygon, difference, intersection, offset, union};
use crate::units::{PointNm, mm_to_nm};

/// The most hatch lines a fill draws in each direction; a finer hatch is
/// refused rather than drawn slowly.
pub(super) const MAX_HATCH_LINES: i64 = 2_000;

/// `area` with the convex corners of its outer ring shaped as `corners` of
/// `size` nm, clipped to `area` so no rounding moves copper past it.
pub(super) fn shaped_area(area: &Polygon, corners: PourCorners, size: i64) -> Vec<Polygon> {
    if corners == PourCorners::Sharp || size <= 0 {
        return vec![area.clone()];
    }
    let outer = shaped_ring(&area.outer, corners, size as f64);
    intersection(
        &[Polygon::new(outer, area.holes.clone())],
        std::slice::from_ref(area),
    )
}

/// `outline` (mm) with its convex corners shaped as `corners` of `size_mm`,
/// by the rule a board pour's fill follows ([`shaped_area`]); a footprint
/// sketch pour bakes its outline with it (#2598). Sharp, or a size that is
/// not a positive number, gives the outline as drawn. `None` when a point
/// or the size is past what the geometry holds, or nothing is left.
pub fn shaped_outline_mm(
    outline: &[Point],
    corners: PourCorners,
    size_mm: f64,
) -> Option<Vec<Point>> {
    if corners == PourCorners::Sharp || !(size_mm.is_finite() && size_mm > 0.0) {
        return Some(outline.to_vec());
    }
    let ring = outline
        .iter()
        .map(|&p| PointNm::from_mm(p, "pour.outline"))
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let size = mm_to_nm(size_mm, "pour.corner_size").ok()?;
    let area = Polygon::new(ring, Vec::new());
    shaped_area(&area, corners, size)
        .into_iter()
        .max_by_key(Polygon::twice_area)
        .map(|piece| piece.outer.into_iter().map(PointNm::to_mm).collect())
}

/// `ring` with each convex corner cut back by `size` along its edges
/// (Chamfer) or replaced by an arc of radius `size` tangent to them
/// (Rounded). A corner takes at most half of each edge, so neighbouring
/// corners never cross; a Rounded corner that would need more is given the
/// radius that fits.
fn shaped_ring(ring: &[PointNm], corners: PourCorners, size: f64) -> Vec<PointNm> {
    let mut points: Vec<PointNm> = ring.to_vec();
    points.dedup();
    if points.len() > 1 && points.first() == points.last() {
        points.pop();
    }
    let n = points.len();
    if n < 3 {
        return points;
    }
    let ccw = crate::region::twice_signed_area(&points) > 0;
    let mut out = Vec::with_capacity(n * 2);
    for i in 0..n {
        let (a, c, b) = (points[(i + n - 1) % n], points[i], points[(i + 1) % n]);
        let turn = c.minus(a).cross(b.minus(c));
        if turn == 0 || (turn > 0) != ccw {
            out.push(c);
            continue;
        }
        shape_corner(&mut out, a, c, b, corners, size);
    }
    out.dedup();
    if out.len() > 1 && out.first() == out.last() {
        out.pop();
    }
    out
}

/// Push the points that replace the convex corner `c` between `a` and `b`.
fn shape_corner(
    out: &mut Vec<PointNm>,
    a: PointNm,
    c: PointNm,
    b: PointNm,
    corners: PourCorners,
    size: f64,
) {
    let (cx, cy) = (c.x as f64, c.y as f64);
    let (ex, ey) = ((a.x - c.x) as f64, (a.y - c.y) as f64);
    let (fx, fy) = ((b.x - c.x) as f64, (b.y - c.y) as f64);
    let (la, lb) = (ex.hypot(ey), fx.hypot(fy));
    let (ux, uy, vx, vy) = (ex / la, ey / la, fx / lb, fy / lb);
    let reach = (la / 2.0).min(lb / 2.0);
    // The interior angle between the two edges.
    let theta = (ux * vx + uy * vy).clamp(-1.0, 1.0).acos();
    let half_tan = (theta / 2.0).tan();
    let back = match corners {
        PourCorners::Sharp => 0.0,
        PourCorners::Chamfer => size.min(reach),
        PourCorners::Rounded if half_tan > 0.0 => (size / half_tan).min(reach),
        PourCorners::Rounded => 0.0,
    };
    let nm = |x: f64, y: f64| PointNm::new(x.round() as i64, y.round() as i64);
    if back < 1.0 {
        out.push(c);
        return;
    }
    let p1 = (cx + ux * back, cy + uy * back);
    let p2 = (cx + vx * back, cy + vy * back);
    out.push(nm(p1.0, p1.1));
    if corners == PourCorners::Rounded {
        let radius = back * half_tan;
        let (bx, by) = (ux + vx, uy + vy);
        let bl = bx.hypot(by);
        let centre_at = radius / (theta / 2.0).sin();
        let (ox, oy) = (cx + bx / bl * centre_at, cy + by / bl * centre_at);
        let start = (p1.1 - oy).atan2(p1.0 - ox);
        let mut sweep = (p2.1 - oy).atan2(p2.0 - ox) - start;
        if sweep > std::f64::consts::PI {
            sweep -= std::f64::consts::TAU;
        } else if sweep < -std::f64::consts::PI {
            sweep += std::f64::consts::TAU;
        }
        let step = std::f64::consts::TAU / f64::from(OFFSET_ARC_POINTS);
        let steps = (sweep.abs() / step).ceil().max(1.0) as usize;
        for k in 1..steps {
            let angle = start + sweep * k as f64 / steps as f64;
            out.push(nm(ox + radius * angle.cos(), oy + radius * angle.sin()));
        }
    }
    out.push(nm(p2.0, p2.1));
}

/// Why a hatch cannot be drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct HatchTooFine;

/// `pieces` as a hatch: the lines of a square grid `hatch.width_mm` wide
/// with `hatch.gap_mm` between them, aligned to the board origin, joined by
/// a border of the same width inside every edge of the pieces.
pub(super) fn hatched(pieces: &[Polygon], hatch: PourHatch) -> Result<Vec<Polygon>, HatchTooFine> {
    let width = mm_to_nm(hatch.width_mm, "zone.hatch_width").map_err(|_| HatchTooFine)?;
    let gap = mm_to_nm(hatch.gap_mm, "zone.hatch_gap").map_err(|_| HatchTooFine)?;
    if width < 1 || gap < 1 {
        return Err(HatchTooFine);
    }
    let Some(bounds) = pieces
        .iter()
        .filter_map(Polygon::bounds)
        .reduce(|a, b| a.union(&b))
    else {
        return Ok(Vec::new());
    };
    let pitch = width + gap;
    let first = |min: i64| min.div_euclid(pitch) * pitch;
    let (x0, y0) = (first(bounds.min.x), first(bounds.min.y));
    let count = |from: i64, to: i64| (to - from) / pitch + 1;
    if count(x0, bounds.max.x) > MAX_HATCH_LINES || count(y0, bounds.max.y) > MAX_HATCH_LINES {
        return Err(HatchTooFine);
    }
    let rect = |x0: i64, y0: i64, x1: i64, y1: i64| {
        Polygon::new(
            vec![
                PointNm::new(x0, y0),
                PointNm::new(x1, y0),
                PointNm::new(x1, y1),
                PointNm::new(x0, y1),
            ],
            Vec::new(),
        )
    };
    let (lo, hi) = (bounds.min, bounds.max);
    let mut lines = Vec::new();
    let mut x = x0;
    while x <= hi.x {
        lines.push(rect(x, lo.y, x + width, hi.y));
        x += pitch;
    }
    let mut y = y0;
    while y <= hi.y {
        lines.push(rect(lo.x, y, hi.x, y + width));
        y += pitch;
    }
    let border = difference(pieces, &offset(pieces, -width));
    Ok(union(&intersection(pieces, &lines), &border))
}

#[cfg(test)]
#[path = "shape_tests.rs"]
mod tests;
