//! The board sketch as one board command (#2234).
//!
//! [`BoardCommand::SetBoardSketch`] swaps the stored sketch, with its baked
//! keepouts, cutouts, holes and V-scores, and the outline when the sketch
//! owns it, in one step. Values move by `mem::replace`, so undo restores
//! them bit for bit. Nothing else writes them; `SetBoardOutline` is refused
//! while the sketch owns the outline. See `docs/PCB_SKETCH.md`.

use signex_types::numeric::first_non_finite_path;
use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_sketch::{BakedBoard, BoardSketch};
use signex_types::sketch::SketchData;

use super::command::{Applied, BoardCommand, BoardPatch, BoardPatchKind};
use super::error::BoardCommandError;

impl BoardCommand {
    /// Store `sketch` and what it baked to (`signex_bake::bake_board`) as
    /// the board's sketch, and write the baked outline when the sketch owns
    /// one.
    pub fn set_board_sketch(sketch: SketchData, baked: BakedBoard) -> Self {
        let (stored, outline) = BoardSketch::from_bake(sketch, baked);
        BoardCommand::SetBoardSketch {
            sketch: Some(Box::new(stored)),
            outline,
        }
    }

    /// Take the sketch off the board, keepouts, cutouts, holes and V-scores
    /// with it. The outline stays as the last bake left it, and is the
    /// board's own to edit again.
    pub fn remove_board_sketch() -> Self {
        BoardCommand::SetBoardSketch {
            sketch: None,
            outline: None,
        }
    }
}

pub(super) fn set(
    board: &mut PcbBoard,
    sketch: Option<Box<BoardSketch>>,
    outline: Option<Vec<Point>>,
) -> Result<Applied, BoardCommandError> {
    if let Some(stored) = sketch.as_deref() {
        refuse_non_finite(stored)?;
    }
    if let Some(points) = outline.as_ref()
        && points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite())
    {
        return Err(BoardCommandError::NonFinite { field: "outline" });
    }
    let previous = std::mem::replace(&mut board.sketch, sketch.map(|stored| *stored));
    let previous_outline = outline.map(|points| std::mem::replace(&mut board.outline, points));
    Ok(Applied {
        inverse: BoardCommand::SetBoardSketch {
            sketch: previous.map(Box::new),
            outline: previous_outline,
        },
        patch: BoardPatch {
            kind: BoardPatchKind::BoardSketchReplaced,
            items: Vec::new(),
        },
    })
}

/// A NaN or infinity anywhere in the sketch or its baked geometry would make
/// the board unsaveable, so it is refused before anything is written.
fn refuse_non_finite(stored: &BoardSketch) -> Result<(), BoardCommandError> {
    match first_non_finite_path(stored) {
        Ok(None) => Ok(()),
        Ok(Some(path)) => Err(BoardCommandError::NonFiniteSketch(path)),
        Err(error) => Err(BoardCommandError::NonFiniteSketch(error.to_string())),
    }
}
