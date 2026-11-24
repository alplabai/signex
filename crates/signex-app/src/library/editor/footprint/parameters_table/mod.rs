//! The sketch parameters table (mockup row 189), shared by the footprint sketch and
//! the board sketch: every parameter with its expression, value, unit and
//! what reads it; Add parameter; typing re-solves; Rename… and Delete in a
//! row menu, a used parameter's delete asking first (Caner 2026-11-20). Opened by the
//! sketch info bar's Parameters button and by Make parameter on a
//! dimension.
//!
//! - [`state`]: what the table holds open, and the pure rules it shows
//!   (autocomplete, value and unit).
//! - [`update`]: the table's own state transitions; the document edits
//!   are `updates::sketch::parameters`.
//! - [`view`]: the overlay card, with `import_view` (Import from another
//!   footprint) and `checks` (the live checks under the rows).
//!
//! The domain (where a name is used, rename, delete, per-row values) is
//! `signex_sketch::parameter_refs`.

mod checks;
mod import_view;
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
    /// Make parameter was picked on a dimension (mockup row 188).
    ParameterFromConstraintRequested { constraint: ConstraintId },
    /// A footprint was picked for Import from another footprint, with its
    /// parameters as (name, expression).
    ImportSourcePicked {
        source: String,
        parameters: Vec<(String, String)>,
    },
    /// A row of the import list was ticked or unticked.
    ImportRowToggled { name: String },
    /// Import answered the import list.
    ImportConfirmed,
    /// Cancel answered the import list.
    ImportCancelled,
}

impl SketchParametersMsg {
    /// Whether the footprint router records the undo step. Only typing an
    /// expression does, as the Properties list's edit does; the other
    /// edits record their own step and only when they change something.
    pub(crate) fn records_undo_step(&self) -> bool {
        matches!(self, Self::ExpressionEdited { .. })
    }
}
