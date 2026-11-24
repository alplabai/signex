//! The Variant Manager tab's own state and messages (mockup row 32): the
//! selected matrix cell, a parameter value being typed for it, and the
//! edits the tab adds to the Variants section's (cell fit, parameter
//! values, Duplicate, Set as default).

/// A matrix cell: one part in one variant (`None` is Base).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantCell {
    pub variant: Option<String>,
    pub designator: String,
}

/// A parameter value typed for the selected cell, kept until Enter or until
/// the typing moves to another parameter or cell, which store it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverrideDraft {
    pub parameter: String,
    pub text: String,
}

/// View state of the Variant Manager tab. Not project data: it is never
/// saved and an undo leaves the selection where it is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VariantManagerUi {
    /// The cell whose part the right pane shows.
    pub cell: Option<VariantCell>,
    /// The value being typed in the right pane's overrides.
    pub draft: Option<OverrideDraft>,
}

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
