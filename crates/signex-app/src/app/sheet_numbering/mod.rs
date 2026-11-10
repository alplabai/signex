//! Tools ▸ Annotation ▸ Number Schematic Sheets (#2247): the editable sheet
//! numbering dialog over `signex_net::sheet_numbering`.
//!
//! The project's pages are read off the UI thread and listed in hierarchy
//! order with their number. Apply writes the manual numbers into the parent
//! sheets' sheet symbols as one undo step across the project; a parent sheet
//! that is not open is loaded into a tabless engine and left unsaved, so
//! nothing reaches the disk until the user saves. Rules: docs/MENU_BAR.md,
//! "Number Schematic Sheets".

mod apply;
mod read;
mod state;
mod update;

#[cfg(test)]
mod tests;

use std::sync::Arc;

pub(crate) use read::{NumberingRead, NumberingRow};
pub(crate) use state::NumberedRow;
pub use state::SheetNumberingState;

impl From<SheetNumberingMsg> for crate::app::Message {
    fn from(msg: SheetNumberingMsg) -> Self {
        Self::Project(crate::app::ProjectMsg::SheetNumbering(msg))
    }
}

/// What happened in the Number Schematic Sheets dialog.
#[derive(Debug, Clone)]
pub enum SheetNumberingMsg {
    /// The menu row, palette or a key asked for the dialog.
    Requested,
    /// Read `generation` of the project's pages finished.
    Read {
        generation: u64,
        result: Result<Arc<NumberingRead>, String>,
    },
    /// The number of row `row` was typed.
    NumberEdited {
        row: usize,
        text: String,
    },
    /// Auto-number: every page back to its position in the hierarchy.
    AutoNumberRequested,
    ApplyRequested,
    CancelRequested,
}
