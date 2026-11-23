//! Why the Project tab's Variants parts read gave no matrix (#1293). The
//! view maps each case to its own catalog message; only the path and the
//! system's text are user content.

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartsReadError {
    /// The project names no root sheet.
    NoRootSheet,
    /// The root sheet could not be read.
    RootUnreadable(PathBuf),
    /// The project has repeated sheets and its saved channel naming cannot
    /// be read, so a channel part's name is unknown.
    ChannelNamingUnreadable,
    /// The background read stopped (the runtime's own text).
    Interrupted(String),
}
