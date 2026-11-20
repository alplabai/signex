//! Arrange in a box (#2409): footprints packed inside a rectangle in rows,
//! left to right from its top-left corner (model y grows down), then down,
//! each origin on the grid and each footprint clear of the others by the
//! gap. Deterministic: the same inputs give the same origins.

use signex_types::pcb::Footprint;

use crate::rect::RectNm;
use crate::staging::{footprint_extent, snap_up};
use crate::transform::{Placement, Side};
use crate::units::{GeomError, PointNm};

/// The bounds of `footprint` about its own origin as it sits on the board:
/// turned by its rotation and mirrored when on the bottom side.
pub fn placed_extent(footprint: &Footprint) -> Result<RectNm, GeomError> {
    let local = footprint_extent(footprint)?;
    if !footprint.rotation.is_finite() {
        return Err(GeomError::Unrepresentable("footprint.rotation"));
    }
    let frame = Placement {
        origin: PointNm::ORIGIN,
        rotation_deg: footprint.rotation,
        side: Side::of_layer(&footprint.layer),
    };
    let corners = local.corners().map(|c| frame.to_world(c));
    RectNm::bounding(corners).ok_or(GeomError::Unrepresentable("footprint extent"))
}

/// Origins that pack footprints with the placed `extents`, in order, inside
/// `area`; `None` when they do not all fit. `grid` (at least 1 nm) is the
/// step every origin lies on, `gap` the clearance between two footprints
/// and between a footprint and the box edge.
pub fn arrange_in_box(
    area: RectNm,
    extents: &[RectNm],
    gap: i64,
    grid: i64,
) -> Option<Vec<PointNm>> {
    let origins = pack_in_box(area, &[], extents, gap, grid);
    let gap = gap.max(0);
    let fits = |(origin, extent): (&PointNm, &RectNm)| {
        let max = origin.plus(extent.max);
        max.x <= area.max.x - gap && max.y <= area.max.y - gap
    };
    origins.iter().zip(extents).all(fits).then_some(origins)
}

/// [`arrange_in_box`] that also keeps every footprint clear of the
/// `occupied` boxes by `gap`, and never refuses: what does not fit goes on
/// in rows below the box, or past its right edge when wider than the box.
/// Rooms pack their parts with it (`signex_engine::pcb::rooms`).
pub fn pack_in_box(
    area: RectNm,
    occupied: &[RectNm],
    extents: &[RectNm],
    gap: i64,
    grid: i64,
) -> Vec<PointNm> {
    // Enough steps past the occupied boxes of a dense room.
    const MAX_STEPS: usize = 4096;
    let (grid, gap) = (grid.max(1), gap.max(0));
    let (left, right) = (area.min.x + gap, area.max.x - gap);
    let mut cursor = PointNm::new(left, area.min.y + gap);
    let mut row_height = 0i64;
    let mut origins = Vec::with_capacity(extents.len());
    for extent in extents {
        let mut origin = place(cursor, extent, grid);
        for _ in 0..MAX_STEPS {
            if cursor.x > left && origin.x + extent.max.x > right {
                cursor = PointNm::new(left, cursor.y + row_height + gap);
                row_height = 0;
                origin = place(cursor, extent, grid);
            }
            let placed = RectNm::new(origin.plus(extent.min), origin.plus(extent.max));
            let Some(block) = occupied.iter().find(|o| !clear_by(o, &placed, gap)) else {
                break;
            };
            // Step past the box; the row is at least as deep as it.
            row_height = row_height.max(block.max.y - cursor.y);
            cursor = PointNm::new(block.max.x + gap, cursor.y);
            origin = place(cursor, extent, grid);
        }
        origins.push(origin);
        let placed_max = origin.plus(extent.max);
        row_height = row_height.max(placed_max.y - cursor.y);
        cursor = PointNm::new(placed_max.x + gap, cursor.y);
    }
    origins
}

/// Whether `a` and `b` are at least `gap` apart along some axis.
fn clear_by(a: &RectNm, b: &RectNm, gap: i64) -> bool {
    a.max.x + gap <= b.min.x
        || b.max.x + gap <= a.min.x
        || a.max.y + gap <= b.min.y
        || b.max.y + gap <= a.min.y
}

/// The grid origin that puts the extent's top-left at or past `cursor`.
fn place(cursor: PointNm, extent: &RectNm, grid: i64) -> PointNm {
    PointNm::new(
        snap_up(cursor.x - extent.min.x, grid),
        snap_up(cursor.y - extent.min.y, grid),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x0: i64, y0: i64, x1: i64, y1: i64) -> RectNm {
        RectNm::new(PointNm::new(x0, y0), PointNm::new(x1, y1))
    }

    const MM: i64 = 1_000_000;

    #[test]
    fn footprints_fill_rows_inside_the_box_without_overlap() {
        let area = rect(0, 0, 20 * MM, 20 * MM);
        let part = rect(-2 * MM, -MM, 2 * MM, MM);
        let origins = arrange_in_box(area, &[part, part, part, part], MM, MM).expect("fits");
        let placed: Vec<RectNm> = origins
            .iter()
            .map(|o| rect(o.x - 2 * MM, o.y - MM, o.x + 2 * MM, o.y + MM))
            .collect();
        for (i, a) in placed.iter().enumerate() {
            assert!(area.contains_rect(a), "{i} inside");
            for b in &placed[i + 1..] {
                assert!(!a.inflate(MM - 1).intersects(b), "kept apart by the gap");
            }
        }
        // Three fit in a 20 mm row (1 + 4 + 1 + 4 + 1 + 4 + 1 = 16 mm); the
        // fourth starts the next row.
        assert_eq!(origins[0], PointNm::new(3 * MM, 2 * MM));
        assert_eq!(origins[1], PointNm::new(8 * MM, 2 * MM));
        assert_eq!(origins[3], PointNm::new(3 * MM, 5 * MM));
    }

    #[test]
    fn packing_steps_past_the_occupied_boxes() {
        let area = rect(0, 0, 20 * MM, 20 * MM);
        let part = rect(-2 * MM, -MM, 2 * MM, MM);
        let taken = rect(MM, MM, 6 * MM, 4 * MM);
        let origins = pack_in_box(area, &[taken], &[part, part], MM, MM);
        for o in &origins {
            let placed = rect(o.x - 2 * MM, o.y - MM, o.x + 2 * MM, o.y + MM);
            assert!(area.contains_rect(&placed));
            assert!(!taken.inflate(MM - 1).intersects(&placed), "clear of it");
        }
        // The first starts a gap past the occupied box, the second follows.
        assert_eq!(origins[0], PointNm::new(9 * MM, 2 * MM));
        assert_eq!(origins[1], PointNm::new(14 * MM, 2 * MM));
    }

    #[test]
    fn packing_goes_on_below_a_full_box() {
        let area = rect(0, 0, 10 * MM, 4 * MM);
        let part = rect(-2 * MM, -MM, 2 * MM, MM);
        let origins = pack_in_box(area, &[], &[part, part, part], MM, MM);
        assert_eq!(arrange_in_box(area, &[part, part, part], MM, MM), None);
        assert_eq!(origins[0], PointNm::new(3 * MM, 2 * MM));
        assert_eq!(origins[1], PointNm::new(3 * MM, 5 * MM), "a row below");
        assert_eq!(origins[2], PointNm::new(3 * MM, 8 * MM));
    }

    #[test]
    fn a_box_too_small_is_refused() {
        let area = rect(0, 0, 5 * MM, 5 * MM);
        let part = rect(-2 * MM, -2 * MM, 2 * MM, 2 * MM);
        assert_eq!(arrange_in_box(area, &[part, part], MM, MM), None);
    }
}
