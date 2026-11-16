//! The board options (#2374): guides, extra snap grids, the sheet and the
//! naming options. [`BoardCommand::SetBoardOptions`] replaces them whole,
//! so its inverse is exact; every check runs before the write.

use std::collections::BTreeSet;

use signex_types::pcb::PcbBoard;
use signex_types::pcb_options::BoardOptions;

use super::command::{Applied, BoardCommand, BoardPatch, BoardPatchKind};
use super::error::BoardCommandError;

pub(super) fn set(
    board: &mut PcbBoard,
    options: BoardOptions,
) -> Result<Applied, BoardCommandError> {
    check(&options)?;
    let previous = std::mem::replace(&mut board.options, options);
    Ok(Applied {
        inverse: BoardCommand::SetBoardOptions {
            options: Box::new(previous),
        },
        patch: BoardPatch {
            kind: BoardPatchKind::OptionsChanged,
            items: Vec::new(),
        },
    })
}

fn check(options: &BoardOptions) -> Result<(), BoardCommandError> {
    let finite = |v: f64, field: &'static str| {
        v.is_finite()
            .then_some(())
            .ok_or(BoardCommandError::NonFinite { field })
    };
    let mut seen = BTreeSet::new();
    for guide in &options.guides {
        finite(guide.at.x, "guide x")?;
        finite(guide.at.y, "guide y")?;
        if !seen.insert(guide.uuid) {
            return Err(BoardCommandError::DuplicateOption(guide.uuid));
        }
    }
    for grid in &options.grids {
        finite(grid.step_mm, "grid step")?;
        finite(grid.origin.x, "grid origin x")?;
        finite(grid.origin.y, "grid origin y")?;
        if grid.step_mm <= 0.0 {
            return Err(BoardCommandError::NotPositive { field: "grid step" });
        }
        if !seen.insert(grid.uuid) {
            return Err(BoardCommandError::DuplicateOption(grid.uuid));
        }
    }
    finite(options.origin.at.x, "origin x")?;
    finite(options.origin.at.y, "origin y")?;
    let sheet = &options.sheet;
    finite(sheet.origin.x, "sheet x")?;
    finite(sheet.origin.y, "sheet y")?;
    finite(sheet.width, "sheet width")?;
    finite(sheet.height, "sheet height")?;
    if sheet.width <= 0.0 {
        return Err(BoardCommandError::NotPositive {
            field: "sheet width",
        });
    }
    if sheet.height <= 0.0 {
        return Err(BoardCommandError::NotPositive {
            field: "sheet height",
        });
    }
    // The panel rides along whole; it keeps its own rules (#2410).
    if let Some(panel) = &options.panel {
        panel.check().map_err(BoardCommandError::Panel)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "options_tests.rs"]
mod tests;
