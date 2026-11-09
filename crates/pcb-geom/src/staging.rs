//! Where new footprints wait before they are placed (#1312): packed to the
//! right of the board outline, in the order given, snapped to a grid, in
//! integer nm. Deterministic: the same inputs give the same positions, and
//! no two staged footprints overlap each other or the board.

use signex_types::pcb::{Footprint, Point};

use crate::rect::RectNm;
use crate::transform::pad_shape;
use crate::units::{GeomError, PointNm, mm_to_nm};

/// How staged footprints are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Staging {
    /// Grid each footprint origin snaps to; at least 1 nm.
    pub grid: i64,
    /// Clearance between two staged footprints, and between them and the
    /// board.
    pub gap: i64,
    /// Width of one row of staged footprints before the next row starts.
    pub row_width: i64,
}

impl Staging {
    /// A 1.27 mm grid, 2.54 mm apart, in rows 100 mm wide.
    pub const DEFAULT: Staging = Staging {
        grid: 1_270_000,
        gap: 2_540_000,
        row_width: 100_000_000,
    };
}

/// The bounds of `footprint` about its own origin, as placed at rotation 0
/// on the top side: its pads and the points of its graphics. A footprint with
/// neither is the single point at its origin.
pub fn footprint_extent(footprint: &Footprint) -> Result<RectNm, GeomError> {
    let mut local = footprint.clone();
    local.position = Point { x: 0.0, y: 0.0 };
    local.rotation = 0.0;
    local.layer = "TopCopper".into();
    let mut extent = RectNm::point(PointNm::ORIGIN);
    for pad in &local.pads {
        extent = extent.union(&pad_shape(&local, pad)?.aabb());
    }
    for graphic in &local.graphics {
        let half = mm_to_nm(graphic.width, "graphic.width")?.abs() / 2;
        let mut points: Vec<Point> = [graphic.start, graphic.end, graphic.mid, graphic.position]
            .into_iter()
            .flatten()
            .collect();
        points.extend(graphic.points.iter().copied());
        for point in points {
            let p = PointNm::from_mm(point, "graphic point")?;
            extent = extent.union(&RectNm::around(p, half, half));
        }
        if let Some(center) = graphic.center {
            let c = PointNm::from_mm(center, "graphic.center")?;
            let r = mm_to_nm(graphic.radius, "graphic.radius")?.abs() + half;
            extent = extent.union(&RectNm::around(c, r, r));
        }
    }
    Ok(extent)
}

/// Origins for footprints with local `extents`, in order, to the right of
/// `board` (the outline's bounds; `None` packs from the origin). Rows fill
/// left to right from the board's top edge and go down. Each origin lies on
/// the grid, and each footprint's extent, placed at its origin, clears every
/// other one and the board by at least `gap`.
pub fn stage(board: Option<RectNm>, extents: &[RectNm], staging: Staging) -> Vec<PointNm> {
    let grid = staging.grid.max(1);
    let gap = staging.gap.max(0);
    let (left, top) = match board {
        Some(b) => (b.max.x + gap, b.min.y),
        None => (0, 0),
    };
    let row_end = left.saturating_add(staging.row_width.max(0));
    let mut cursor = PointNm::new(left, top);
    let mut row_height = 0i64;
    let mut origins = Vec::with_capacity(extents.len());
    for extent in extents {
        let w = extent.width();
        if cursor.x > left && cursor.x.saturating_add(w) > row_end {
            cursor = PointNm::new(left, cursor.y + row_height + gap);
            row_height = 0;
        }
        // Snap up, so the extent only moves away from what is already placed.
        let origin = PointNm::new(
            snap_up(cursor.x - extent.min.x, grid),
            snap_up(cursor.y - extent.min.y, grid),
        );
        let placed_max = origin.plus(extent.max);
        origins.push(origin);
        row_height = row_height.max(placed_max.y - cursor.y);
        cursor = PointNm::new(placed_max.x + gap, cursor.y);
    }
    origins
}

/// The smallest multiple of `grid` at or above `v`.
fn snap_up(v: i64, grid: i64) -> i64 {
    v.div_euclid(grid) * grid + if v.rem_euclid(grid) == 0 { 0 } else { grid }
}

#[cfg(test)]
#[path = "staging_tests.rs"]
mod tests;
