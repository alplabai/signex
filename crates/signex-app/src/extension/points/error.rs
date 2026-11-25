//! Why an extension point refused an addition.

use std::fmt;

/// Why an extension point refused an addition. A refused addition is
/// recorded too, and [`ExtensionHost::install`](crate::extension::ExtensionHost::install)
/// then refuses the whole set, so a binary never runs with half an
/// extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterError {
    /// The id is not under the owner's `ext.<name>.` prefix.
    NotOwned(String),
    /// The owner already added this id.
    Duplicate(String),
    /// The id is not a valid command id (`ext.<name>.<snake_case>`).
    InvalidCommand(String),
}

impl fmt::Display for RegisterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotOwned(id) => write!(f, "{id:?} is not under the extension's own prefix"),
            Self::Duplicate(id) => write!(f, "{id:?} was added twice"),
            Self::InvalidCommand(id) => write!(f, "{id:?} is not a valid command id"),
        }
    }
}

impl std::error::Error for RegisterError {}
