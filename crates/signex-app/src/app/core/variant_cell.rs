//! The Variant Manager tab's view state (mockup row 32): the selected
//! matrix cell and a parameter value being typed for it. Plain state,
//! UI-free (ADR-0006).

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
    /// The parts on a repeated sheet whose matrix row is open, one sub-row
    /// per channel's copy (#2662), by the part every copy shares.
    pub open_channel_parts: std::collections::HashSet<signex_types::pcb::SymbolLink>,
}
