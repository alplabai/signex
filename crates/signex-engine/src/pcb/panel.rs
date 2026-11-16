//! The board's panel (#2410). [`BoardCommand::SetBoardPanel`] replaces it
//! whole, so its inverse is exact; every rule of the panel is checked before
//! the write. Whether it lays out on the board's outline is not: the outline
//! may change after, and an undo must put back what was there. Geometry:
//! `pcb_geom::panel`; rules: `docs/PANELISATION.md`.

use signex_types::pcb::PcbBoard;
use signex_types::pcb_panel::BoardPanel;

use super::command::{Applied, BoardCommand, BoardPatch, BoardPatchKind};
use super::error::BoardCommandError;

pub(super) fn set(
    board: &mut PcbBoard,
    panel: Option<BoardPanel>,
) -> Result<Applied, BoardCommandError> {
    if let Some(panel) = &panel {
        panel.check().map_err(BoardCommandError::Panel)?;
    }
    let previous = std::mem::replace(&mut board.options.panel, panel);
    Ok(Applied {
        inverse: BoardCommand::SetBoardPanel {
            panel: previous.map(Box::new),
        },
        patch: BoardPatch {
            kind: BoardPatchKind::PanelChanged,
            items: Vec::new(),
        },
    })
}

#[cfg(test)]
#[path = "panel_tests.rs"]
mod tests;
