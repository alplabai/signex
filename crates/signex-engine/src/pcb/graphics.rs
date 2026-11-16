//! Add board drawings at the end of `PcbBoard::graphics` and take them back:
//! the edit behind a DXF import onto a layer (#2410). A board drawing has no
//! uuid, so the inverse names the drawings it removes by value and refuses
//! unless the list still ends with exactly them.

use signex_types::format::mm_as_stored_nm;
use signex_types::pcb::{BoardGraphic, PcbBoard, Point};

use super::command::{Applied, BoardCommand, BoardPatch, BoardPatchKind};
use super::error::BoardCommandError;

/// Append `graphics`. Refused for a number the board file cannot hold (not
/// finite, or past its nanometre range), which would fail every save.
pub(super) fn add(
    board: &mut PcbBoard,
    graphics: Vec<BoardGraphic>,
) -> Result<Applied, BoardCommandError> {
    if !graphics.iter().all(is_storable) {
        return Err(BoardCommandError::NotStorable { field: "drawing" });
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

/// Whether every number of `g` saves: finite and inside the board file's
/// nanometre range.
pub(super) fn is_storable(g: &BoardGraphic) -> bool {
    [g.start, g.end, g.center]
        .iter()
        .flatten()
        .all(is_storable_point)
        && g.points.iter().all(is_storable_point)
        && mm_as_stored_nm(g.width).is_some()
        && mm_as_stored_nm(g.radius).is_some()
}

pub(super) fn is_storable_point(p: &Point) -> bool {
    mm_as_stored_nm(p.x).is_some() && mm_as_stored_nm(p.y).is_some()
}
