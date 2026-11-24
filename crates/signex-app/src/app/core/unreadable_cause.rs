//! Why a sheet that exists could not be read. Plain state, UI-free
//! (ADR-0006).

/// Why a sheet that exists could not be read, so a list can name it in the
/// UI language (mockup row 28). The reader's English detail travels beside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnreadableCause {
    /// A directory, device or FIFO where a sheet should be.
    NotARegularFile,
    /// Larger than the walk reads.
    TooLarge,
    /// The system refused the read.
    ReadFailed,
    /// Written by a newer Signex.
    NewerFormat,
    /// The text is not a sheet this build reads.
    NotASheet,
    /// The caller's read guard did not allow the read (#2199).
    Refused,
}
