//! The sketch parameters table (mockup row 189), shared by the footprint sketch and
//! the board sketch: every parameter with its expression, value, unit and
//! what reads it; Add parameter; typing re-solves; Rename… and Delete in a
//! row menu, a used parameter's delete asking first (fp-07). Opened by the
//! sketch info bar's Parameters button and by Make parameter on a
//! dimension.
//!
//! - [`state`]: what the table holds open, and the pure rules it shows
//!   (autocomplete, value and unit).
//! - [`update`]: the table's own state transitions; the document edits
//!   are `updates::sketch::parameters`.
//! - [`view`]: the overlay card.
//!
//! The domain (where a name is used, rename, delete, per-row values) is
//! `signex_sketch::parameter_refs`.

pub mod state;
pub mod update;
pub mod view;

#[cfg(test)]
mod tests;

pub use state::{ParametersTableState, RenameDraft};

use signex_sketch::id::ConstraintId;

/// What the parameters table and its callers report.
#[derive(Debug, Clone, PartialEq)]
pub enum SketchParametersMsg {
    /// The info bar's Parameters button was pressed: the table opens, or
    /// closes when it is open.
    TableToggled,
    /// The table's close button was pressed.
    TableClosed,
    /// A row's expression was typed in, or an offered name was picked.
    ExpressionEdited { name: String, expr: String },
    /// Add parameter was pressed.
    AddClicked,
    /// A row's ⋯ button was pressed.
    RowMenuToggled { name: String },
    /// Rename… was picked in a row menu.
    RenameStarted { name: String },
    /// The new name was typed in.
    RenameDraftChanged(String),
    /// The new name was submitted.
    RenameSubmitted,
    /// The rename was cancelled.
    RenameCancelled,
    /// Delete was picked in a row menu.
    DeleteRequested { name: String },
    /// "Replace with value" answered the in-use prompt.
    DeleteConfirmed,
    /// "Cancel" answered the in-use prompt.
    DeleteCancelled,
    /// Make parameter was picked on a dimension (m188).
    ParameterFromConstraintRequested { constraint: ConstraintId },
}

impl SketchParametersMsg {
    /// Whether the footprint router records the undo step. Only typing an
    /// expression does, as the Properties list's edit does; the other
    /// edits record their own step and only when they change something.
    pub(crate) fn records_undo_step(&self) -> bool {
        matches!(self, Self::ExpressionEdited { .. })
    }
}
