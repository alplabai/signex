//! View state of the Preferences Rule Profiles page: UI-free (ADR-0006).

/// Which half of the rule editor the page shows (mockup row 148).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProfileEditorTab {
    #[default]
    Severity,
    PinMatrix,
}

/// The library action the page's name field is open for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileNameAction {
    New,
    Duplicate,
    Rename,
}
