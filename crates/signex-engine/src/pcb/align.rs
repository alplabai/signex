//! Align the selected footprints on a board (#2373), the board version of
//! the schematic's Align rows: footprints line up by their origins, as
//! symbols line up by their anchors. Each run is one undo step; a locked
//! footprint refuses it whole.

use std::collections::BTreeSet;

use signex_types::pcb::{Footprint, PcbBoard, Point};
use uuid::Uuid;

use super::command::BoardCommand;
use super::placement::{PlacementRefusal, editable_footprints, grid_step_mm};

/// How [`align_commands`] lines the footprints up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignTo {
    Left,
    Right,
    /// The smallest y: the top of the board view.
    Top,
    Bottom,
    /// One x, halfway between the leftmost and rightmost origins.
    HorizontalCenters,
    /// One y, halfway between the top and bottom origins.
    VerticalCenters,
    /// Evenly spaced along x between the outermost two.
    DistributeHorizontally,
    DistributeVertically,
    /// Each origin to the nearest point of the board's grid.
    Grid,
}

/// The moves that align the footprints `ids` names. Fewer than two (three
/// to distribute) is nothing to do, except for [`AlignTo::Grid`].
pub fn align_commands(
    board: &PcbBoard,
    ids: &BTreeSet<Uuid>,
    to: AlignTo,
) -> Result<Vec<BoardCommand>, PlacementRefusal> {
    let footprints = editable_footprints(board, ids)?;
    let needed = match to {
        AlignTo::Grid => 1,
        AlignTo::DistributeHorizontally | AlignTo::DistributeVertically => 3,
        _ => 2,
    };
    if footprints.len() < needed {
        return Ok(Vec::new());
    }
    let targets = targets(board, &footprints, to);
    Ok(footprints
        .iter()
        .zip(targets)
        .filter(|(fp, target)| fp.position != *target)
        .map(|(fp, target)| BoardCommand::MoveFootprint {
            footprint: fp.uuid,
            to: target,
        })
        .collect())
}

/// Where each footprint's origin goes, in the order given.
fn targets(board: &PcbBoard, footprints: &[&Footprint], to: AlignTo) -> Vec<Point> {
    let xs = || footprints.iter().map(|fp| fp.position.x);
    let ys = || footprints.iter().map(|fp| fp.position.y);
    let (min_x, max_x) = (xs().fold(f64::INFINITY, f64::min), xs().fold(f64::NEG_INFINITY, f64::max));
    let (min_y, max_y) = (ys().fold(f64::INFINITY, f64::min), ys().fold(f64::NEG_INFINITY, f64::max));
    let grid = grid_step_mm(board);
    let snap = |v: f64| (v / grid).round() * grid;
    let spread = |values: Vec<f64>, lo: f64, hi: f64| distribute(&values, lo, hi);
    let distributed_x = spread(xs().collect(), min_x, max_x);
    let distributed_y = spread(ys().collect(), min_y, max_y);
    footprints
        .iter()
        .enumerate()
        .map(|(i, fp)| {
            let Point { x, y } = fp.position;
            match to {
                AlignTo::Left => Point::new(min_x, y),
                AlignTo::Right => Point::new(max_x, y),
                AlignTo::Top => Point::new(x, min_y),
                AlignTo::Bottom => Point::new(x, max_y),
                AlignTo::HorizontalCenters => Point::new((min_x + max_x) / 2.0, y),
                AlignTo::VerticalCenters => Point::new(x, (min_y + max_y) / 2.0),
                AlignTo::DistributeHorizontally => Point::new(distributed_x[i], y),
                AlignTo::DistributeVertically => Point::new(x, distributed_y[i]),
                AlignTo::Grid => Point::new(snap(x), snap(y)),
            }
        })
        .collect()
}

/// `values` spread evenly from `lo` to `hi`, each keeping its rank: the
/// outermost two stay where they are.
fn distribute(values: &[f64], lo: f64, hi: f64) -> Vec<f64> {
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|&a, &b| values[a].total_cmp(&values[b]));
    let step = if values.len() > 1 {
        (hi - lo) / (values.len() - 1) as f64
    } else {
        0.0
    };
    let mut out = values.to_vec();
    for (rank, &index) in order.iter().enumerate() {
        out[index] = match rank {
            0 => lo,
            r if r == values.len() - 1 => hi,
            r => lo + step * r as f64,
        };
    }
    out
}

#[cfg(test)]
#[path = "align_tests.rs"]
mod tests;
