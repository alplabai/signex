//! What an engine command did to the document it ran on.

/// What an engine command did to the active document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MutationOutcome {
    /// The document changed.
    Changed,
    /// The command ran and found nothing to change.
    Unchanged,
    /// The engine rejected the command (already logged) or there was no
    /// active schematic to run it on.
    Failed,
}
