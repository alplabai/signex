//! The BOM alternates edits (#2938): add an alternate part to a BOM line,
//! take one off, or move one in the line's order. Each is one step on the
//! project's General history; the checks that refuse one live in
//! `signex-types`.

use signex_types::project::{BomAlternateProblem, ProjectData};

/// One edit of a BOM line's alternates. `line` is the line key and `row` a
/// library row key, as `signex_types::project::bom_line_key` and
/// `bom_row_key` read them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectBomCommand {
    /// Add `row` after the line's other alternates.
    AddAlternate {
        line: String,
        row: String,
    },
    RemoveAlternate {
        line: String,
        row: String,
    },
    /// Move `row` to `position`, 1 being the most preferred.
    MoveAlternate {
        line: String,
        row: String,
        position: usize,
    },
}

impl ProjectBomCommand {
    /// Apply to `data`. A refused edit changes nothing.
    pub fn apply(&self, data: &mut ProjectData) -> Result<(), BomAlternateProblem> {
        match self {
            Self::AddAlternate { line, row } => data.add_bom_alternate(line, row),
            Self::RemoveAlternate { line, row } => data.remove_bom_alternate(line, row),
            Self::MoveAlternate {
                line,
                row,
                position,
            } => data.move_bom_alternate(line, row, *position),
        }
    }

    /// The line the edit names, as typed.
    pub fn line(&self) -> &str {
        match self {
            Self::AddAlternate { line, .. }
            | Self::RemoveAlternate { line, .. }
            | Self::MoveAlternate { line, .. } => line,
        }
    }
}
