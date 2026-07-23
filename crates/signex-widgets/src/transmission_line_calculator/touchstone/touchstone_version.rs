/// Identifies the syntax generation used by a parsed Touchstone document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TouchstoneVersion {
    Version1,
    Version2,
}
