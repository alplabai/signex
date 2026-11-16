//! DXF import onto a board (#2410): a file's lines, arcs, circles and
//! polylines become drawings on one layer, or its largest closed shape
//! becomes the board outline, as one [`BoardCommand`] (one undo step).
//! Nothing here does IO: the caller reads the file. Design note:
//! `docs/BOARD_IMPORT_AND_REUSE.md`, "DXF import".

mod parse;
mod shapes;

#[cfg(test)]
mod tests;

pub use parse::{DxfDrawing, DxfEntity, parse_dxf};
pub use shapes::{board_graphics, board_outline};

use serde::Serialize;
use signex_types::format::pcb_layer_token;
use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::PcbLayer;

use super::BoardCommand;

/// Where an import puts the drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DxfTarget {
    /// Every entity as a drawing on this layer.
    Layer(PcbLayer),
    /// The largest closed shape replaces the board outline.
    Outline,
}

/// What an import made, for the Messages panel and an agent's result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DxfImportSummary {
    /// Drawings added to the layer; 0 for an outline import.
    pub graphics: usize,
    /// Corners of the new outline; 0 for a layer import.
    pub outline_points: usize,
    /// Entities of a kind the import does not read.
    pub skipped: usize,
    /// The layer token, or `outline`.
    pub target: String,
}

/// Why a file was not imported. The board is unchanged.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DxfError {
    #[error("the file is not a DXF text file")]
    NotDxf,
    #[error("line {line} is not a DXF group code")]
    BadGroupCode { line: usize },
    #[error("{0:?} is not a finite number")]
    BadNumber(String),
    #[error("a {entity} has no value under group code {code}")]
    MissingValue { entity: &'static str, code: i32 },
    #[error("the drawing units ({0}) are not supported")]
    UnsupportedUnits(String),
    #[error("the file has no ENTITIES section")]
    NoEntities,
    #[error("the file has no line, arc, circle or polyline to import")]
    Empty,
    #[error("the file has no closed shape to use as the board outline")]
    NoClosedShape,
    #[error("the board sketch owns the outline; edit it in Board Sketch")]
    OutlineOwnedBySketch,
}

/// The command importing `text` onto `board` at `target`, and what it makes.
/// Positions are read from the board origin with y up, as the file has them.
pub fn dxf_import_command(
    board: &PcbBoard,
    text: &str,
    target: DxfTarget,
) -> Result<(BoardCommand, DxfImportSummary), DxfError> {
    let drawing = parse_dxf(text)?;
    if drawing.entities.is_empty() {
        return Err(DxfError::Empty);
    }
    let origin = board.options.origin;
    match target {
        DxfTarget::Layer(layer) => {
            let token = pcb_layer_token(layer);
            let graphics = board_graphics(&drawing, origin, &token);
            let summary = DxfImportSummary {
                graphics: graphics.len(),
                outline_points: 0,
                skipped: drawing.skipped,
                target: token,
            };
            Ok((BoardCommand::AddGraphics { graphics }, summary))
        }
        DxfTarget::Outline => {
            if board
                .sketch
                .as_ref()
                .is_some_and(signex_types::pcb_sketch::BoardSketch::owns_outline)
            {
                return Err(DxfError::OutlineOwnedBySketch);
            }
            let outline = board_outline(&drawing, origin).ok_or(DxfError::NoClosedShape)?;
            let summary = DxfImportSummary {
                graphics: 0,
                outline_points: outline.len(),
                skipped: drawing.skipped,
                target: "outline".to_string(),
            };
            Ok((BoardCommand::SetBoardOutline { outline }, summary))
        }
    }
}
