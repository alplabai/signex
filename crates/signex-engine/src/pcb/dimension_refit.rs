//! Radial dimensions follow the arc or circle they measure (#2507). The
//! circles of a board a radial dimension can measure, and the refit: after
//! the commands of a new edit, the board engine puts each radial dimension
//! that is off its circle back on it, in the same undo step, so undo and
//! redo take both back together.

use signex_types::pcb::PcbBoard;
use signex_types::pcb_dimension::LinearDimension;
use signex_types::pcb_dimension_kind::MeasuredCircle;
use signex_types::pcb_graphic::BoardGraphicKind;
use uuid::Uuid;

use super::command::{BoardCommand, BoardPatch};
use super::error::BoardCommandError;
use super::items::BoardItem;

/// Every arc and circle on `board` a radial dimension can measure: the
/// free arcs (copper or not), and the board drawings that are arcs or
/// circles, with the radius the view and the outputs draw
/// (`pcb_geom::circle_radius`: the distance to the rim point).
pub fn board_circles(board: &PcbBoard) -> impl Iterator<Item = MeasuredCircle> + '_ {
    let arcs = board.free.arcs.iter().filter_map(|arc| {
        let sweep = arc.sweep()?;
        Some(MeasuredCircle {
            uuid: arc.uuid,
            centre: sweep.center,
            radius: sweep.radius,
        })
    });
    let drawings = board.graphics.iter().filter_map(|graphic| {
        // An arc's rim is its start; a circle's its end, else its start.
        let rim_end = match graphic.kind()? {
            BoardGraphicKind::Arc => None,
            BoardGraphicKind::Circle => graphic.end,
            _ => return None,
        };
        let centre = graphic.center?;
        let nm = pcb_geom::circle_radius(centre, graphic.radius, graphic.start, rim_end).ok()?;
        let radius = pcb_geom::nm_to_mm(nm);
        (radius > 0.0).then_some(MeasuredCircle {
            uuid: graphic.uuid,
            centre,
            radius,
        })
    });
    arcs.chain(drawings)
}

/// The arc or circle `uuid` names on `board`.
pub fn circle_by_id(board: &PcbBoard, uuid: Uuid) -> Option<MeasuredCircle> {
    board_circles(board).find(|circle| circle.uuid == uuid)
}

/// The arc or circle of `board` whose edge is nearest `at`, when it is
/// within `reach` mm of it.
pub fn circle_near(
    board: &PcbBoard,
    at: signex_types::pcb::Point,
    reach: f64,
) -> Option<MeasuredCircle> {
    board_circles(board)
        .map(|circle| (circle.edge_distance(at), circle))
        .filter(|(distance, _)| *distance <= reach)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, circle)| circle)
}

/// `dimension` put back on the circle it measures on `board`; `None` when
/// it is not radial, its circle is not on `board`, or it already sits on it.
pub fn refit_on(dimension: &LinearDimension, board: &PcbBoard) -> Option<LinearDimension> {
    let circle = circle_by_id(board, dimension.measures?)?;
    dimension.refit_to(&circle)
}

/// The radial dimensions of `board` that are off the circle they measure,
/// each put back on it. A dimension whose circle is gone stays where it is.
pub fn refit_dimensions(board: &PcbBoard) -> Vec<LinearDimension> {
    board
        .free
        .dimensions
        .iter()
        .filter_map(|dimension| refit_on(dimension, board))
        .collect()
}

/// The replace that puts every radial dimension of `board` back on its
/// circle; `None` when each is on it.
pub(super) fn refit_command(board: &PcbBoard) -> Option<BoardCommand> {
    let items: Vec<BoardItem> = refit_dimensions(board)
        .into_iter()
        .map(BoardItem::Dimension)
        .collect();
    (!items.is_empty()).then_some(BoardCommand::ReplaceItems { items })
}

/// The inverses and patches of a run, with the refit of its radial
/// dimensions applied after it when one is owed. All or nothing: a refused
/// refit puts back what the run did.
pub(super) fn refit_after(
    board: &mut PcbBoard,
    mut inverses: Vec<BoardCommand>,
    mut patches: Vec<BoardPatch>,
) -> Result<(Vec<BoardCommand>, Vec<BoardPatch>), BoardCommandError> {
    let Some(refit) = refit_command(board) else {
        return Ok((inverses, patches));
    };
    match refit.apply(board) {
        Ok(applied) => {
            inverses.push(applied.inverse);
            patches.push(applied.patch);
            Ok((inverses, patches))
        }
        Err(error) => {
            for inverse in inverses.into_iter().rev() {
                inverse
                    .apply(board)
                    .map_err(|e| BoardCommandError::RollbackFailed(e.to_string()))?;
            }
            Err(error)
        }
    }
}
