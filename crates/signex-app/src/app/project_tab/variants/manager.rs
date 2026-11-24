//! The Variant Manager tab's own state and messages (mockup row 32): the
//! selected matrix cell, a parameter value being typed for it, and the
//! edits the tab adds to the Variants section's (cell fit, parameter
//! values, Duplicate, Set as default).

// The tab's view state is plain data, kept in the UI-free core (ADR-0006).
pub use crate::app::core::variant_cell::{OverrideDraft, VariantCell};

/// Messages of the Variant Manager tab, namespaced under
/// `ProjectTabMsg::VariantManager`.
#[derive(Debug, Clone)]
pub enum VariantManagerMsg {
    /// A matrix cell was clicked: the right pane follows it.
    CellSelected(VariantCell),
    /// The right pane's Fitting switch: `Some` fits or leaves out the part
    /// in the cell's variant, `None` follows the sheets again.
    FitPicked {
        cell: VariantCell,
        fitted: Option<bool>,
    },
    /// A parameter value as typed for the selected cell.
    OverrideEdited { parameter: String, text: String },
    /// Enter in a parameter value: the typed value is stored; an empty one
    /// is the Base value again.
    OverrideSubmitted { cell: VariantCell },
    /// The reset arrow of a parameter: the Base value again.
    OverrideReset {
        cell: VariantCell,
        parameter: String,
    },
    /// Duplicate: a copy of the variant (`None` is Base), its name opened
    /// for typing.
    DuplicateRequested { from: Option<String> },
    /// Set as default; `None` is Base.
    DefaultPicked { name: Option<String> },
}
