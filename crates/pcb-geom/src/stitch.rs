//! Via stitching (#2408): where vias of one net fit inside an area on a
//! square grid. A via is placed only where its whole disc lies inside the
//! area and the board outline, clear of copper of other nets by the
//! clearance the caller's rule gives, clear of pads and vias of its own
//! net, and outside every keepout that forbids vias. Rules:
//! `docs/PCB_TOOLS.md`, "Via Stitching".

use signex_types::pcb::PcbBoard;

use crate::obstacle::{NewCopper, clashes, copper_obstacles, disc_inside, keepout_areas};
use crate::region::Polygon;
use crate::shape::Shape;
use crate::units::{PointNm, mm_to_nm};

/// The vias to stitch with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StitchSpec {
    /// The net every via joins; not 0.
    pub net: u32,
    /// Distance between grid points in x and y; at least the diameter.
    pub pitch: i64,
    /// The via's copper diameter.
    pub diameter: i64,
}

/// Every grid point inside `area` where a via of `spec` fits on `board`, in
/// rows top to bottom, each left to right. The grid is the multiples of the
/// pitch, so stitching the same area twice lands on the same points (and
/// the second run, kept off the first run's vias, adds none).
/// `clearance(a, b)` is the gap nets `a` and `b` need between them.
pub fn stitch_points<F: Fn(u32, u32) -> i64>(
    board: &PcbBoard,
    area: &[Polygon],
    spec: StitchSpec,
    clearance: F,
) -> Vec<PointNm> {
    if spec.net == 0 || spec.pitch <= 0 || spec.diameter <= 0 || spec.pitch < spec.diameter {
        return Vec::new();
    }
    let radius = spec.diameter / 2;
    let obstacles = copper_obstacles(board);
    let keepouts = keepout_areas(board);
    let outline = board_area(board);
    let edge = clearance(spec.net, 0).max(0);
    let mut points = Vec::new();
    for piece in area {
        let Some(bounds) = piece.bounds() else {
            continue;
        };
        let first = |v: i64| v.div_euclid(spec.pitch) * spec.pitch;
        let mut y = first(bounds.min.y);
        while y <= bounds.max.y {
            let mut x = first(bounds.min.x);
            while x <= bounds.max.x {
                let center = PointNm::new(x, y);
                let disc = Shape::Disc { center, radius };
                let fits = disc_inside(piece, center, radius, 0)
                    && outline
                        .as_ref()
                        .is_none_or(|board| disc_inside(board, center, radius, edge))
                    && !keepouts
                        .iter()
                        .any(|k| k.bars(NewCopper::Via, u32::MAX, &disc))
                    && !clashes(
                        &disc,
                        NewCopper::Via,
                        spec.net,
                        u32::MAX,
                        &obstacles,
                        &clearance,
                        |_| false,
                    );
                if fits && !points.contains(&center) {
                    points.push(center);
                }
                x += spec.pitch;
            }
            y += spec.pitch;
        }
    }
    points.sort_by_key(|p| (p.y, p.x));
    points
}

/// The board outline as an area; `None` without one (fewer than three
/// corners, or one the geometry cannot hold).
fn board_area(board: &PcbBoard) -> Option<Polygon> {
    if board.outline.len() < 3 {
        return None;
    }
    let ring = board
        .outline
        .iter()
        .map(|&p| PointNm::from_mm(p, "board.outline"))
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    Some(Polygon::new(ring, Vec::new()))
}

/// [`StitchSpec`] from mm values; `None` for a value the geometry cannot
/// hold or that is not above zero.
pub fn stitch_spec_mm(net: u32, pitch_mm: f64, diameter_mm: f64) -> Option<StitchSpec> {
    let pitch = mm_to_nm(pitch_mm, "stitch.pitch").ok().filter(|v| *v > 0)?;
    let diameter = mm_to_nm(diameter_mm, "stitch.diameter")
        .ok()
        .filter(|v| *v > 0)?;
    Some(StitchSpec {
        net,
        pitch,
        diameter,
    })
}

#[cfg(test)]
#[path = "stitch_tests.rs"]
mod tests;
