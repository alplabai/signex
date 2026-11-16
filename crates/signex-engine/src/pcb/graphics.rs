//! Add board drawings at the end of `PcbBoard::graphics` and take them back:
//! the edit behind a DXF import onto a layer (#2410). A board drawing has no
//! uuid, so the inverse names the drawings it removes by value and refuses
//! unless the list still ends with exactly them.

use signex_types::pcb::{BoardGraphic, PcbBoard, Point};

use super::command::{Applied, BoardCommand, BoardPatch, BoardPatchKind};
use super::error::BoardCommandError;

/// Append `graphics`. Refused for a number that is not finite.
pub(super) fn add(
    board: &mut PcbBoard,
    graphics: Vec<BoardGraphic>,
) -> Result<Applied, BoardCommandError> {
    if !graphics.iter().all(is_finite) {
        return Err(BoardCommandError::NonFinite { field: "drawing" });
    }
    board.graphics.extend(graphics.iter().cloned());
    Ok(applied(BoardCommand::RemoveGraphics { graphics }))
}

/// Remove `graphics` from the end of the list, where an add put them.
pub(super) fn remove(
    board: &mut PcbBoard,
    graphics: Vec<BoardGraphic>,
) -> Result<Applied, BoardCommandError> {
    let len = board.graphics.len();
    let Some(from) = len.checked_sub(graphics.len()) else {
        return Err(BoardCommandError::GraphicsMismatch);
    };
    if board.graphics[from..] != graphics[..] {
        return Err(BoardCommandError::GraphicsMismatch);
    }
    board.graphics.truncate(from);
    Ok(applied(BoardCommand::AddGraphics { graphics }))
}

fn applied(inverse: BoardCommand) -> Applied {
    Applied {
        inverse,
        patch: BoardPatch {
            kind: BoardPatchKind::GraphicsChanged,
            items: Vec::new(),
        },
    }
}

fn is_finite(g: &BoardGraphic) -> bool {
    let point = |p: &Point| p.x.is_finite() && p.y.is_finite();
    [g.start, g.end, g.center].iter().flatten().all(point)
        && g.points.iter().all(point)
        && g.width.is_finite()
        && g.radius.is_finite()
}
