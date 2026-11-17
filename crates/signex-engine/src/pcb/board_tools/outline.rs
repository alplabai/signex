//! Board outline from a selection (#2409): selected lines that close one
//! loop become the board outline, one undo step. The lines stay where they
//! are. Refused while the board sketch owns the outline, and for lines that
//! are open, branch or make several loops (`pcb_geom::closed_path`).

use std::collections::BTreeSet;

use pcb_geom::closed_path;
use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_sketch::BoardSketch;
use uuid::Uuid;

use super::{ToolEdit, ToolRefusal, ToolReport};
use crate::pcb::command::BoardCommand;

/// The outline the lines `tracks` names close, as an edit.
pub fn outline_from_tracks(
    board: &PcbBoard,
    tracks: &BTreeSet<Uuid>,
) -> Result<ToolEdit, ToolRefusal> {
    if board.sketch.as_ref().is_some_and(BoardSketch::owns_outline) {
        return Err(ToolRefusal::OutlineOwnedBySketch);
    }
    let outline = closed_lines(board, tracks)?;
    Ok(ToolEdit {
        report: ToolReport {
            outline_points: outline.len(),
            ..ToolReport::default()
        },
        commands: vec![BoardCommand::SetBoardOutline { outline }],
    })
}

/// The corners of the loop the tracks `tracks` names close.
pub(super) fn closed_lines(
    board: &PcbBoard,
    tracks: &BTreeSet<Uuid>,
) -> Result<Vec<Point>, ToolRefusal> {
    let lines: Vec<(Point, Point)> = board
        .segments
        .iter()
        .filter(|s| tracks.contains(&s.uuid))
        .map(|s| (s.start, s.end))
        .collect();
    if lines.is_empty() {
        return Err(ToolRefusal::NothingSelected);
    }
    closed_path(&lines).map_err(|why| ToolRefusal::Path(why.to_string()))
}
