//! Why an extension point refused an addition.

use std::fmt;

/// Why an extension point refused an addition: the one refusal type of every
/// point, catalog commands included. Each variant names the id as the
/// extension gave it. A refused addition is recorded too, and
/// [`ExtensionHost::install`](crate::extension::ExtensionHost::install) then
/// refuses the whole set, so a binary never runs with half an extension.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RegisterError {
    /// The id is not under the owner's `ext.<name>.` prefix.
    NotOwned(String),
    /// The owner already added this id.
    Duplicate(String),
    /// The id is under the owner's prefix but is not an extension id:
    /// `ext.<name>.` and exactly one snake_case segment.
    InvalidId(String),
    /// The id is the owner's but was never added: a preference saved under
    /// a key `Preferences::add` did not add, or a menu or Active Bar row
    /// that runs a command `CatalogCommands::add` did not add.
    NotRegistered(String),
    /// A command id is a built-in command id, or its `<command>` part is one
    /// (`ext.sample.undo`), so it would read as the built-in command.
    ShadowsBuiltIn(String),
    /// A command row's flags do not say whether it runs headless: exactly
    /// one of `gui_only` and `runs_headless` is set.
    HeadlessUndecided(String),
    /// A command row has operands; an extension command takes none in this
    /// version.
    TakesOperands(String),
    /// A command row mutates a document; an extension command cannot in this
    /// version.
    MutatesDocument(String),
    /// A message file does not parse; the reason.
    InvalidMessages(String),
    /// A preference's label or help is not one of the owner's own messages
    /// (`ext-<name>-…`); the message id.
    ForeignMessage(String),
}

impl fmt::Display for RegisterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotOwned(id) => write!(f, "{id:?} is not under the extension's own prefix"),
            Self::Duplicate(id) => write!(f, "{id:?} was added twice"),
            Self::InvalidId(id) => write!(
                f,
                "{id:?} is not an extension id (`ext.<name>.<snake_case>`)"
            ),
            Self::NotRegistered(id) => write!(f, "{id:?} was never added by the extension"),
            Self::ShadowsBuiltIn(id) => write!(f, "{id:?} shadows a built-in command"),
            Self::HeadlessUndecided(id) => write!(
                f,
                "{id:?} must set exactly one of `gui_only` and `runs_headless`"
            ),
            Self::TakesOperands(id) => {
                write!(f, "{id:?} has operands; an extension command takes none")
            }
            Self::MutatesDocument(id) => {
                write!(f, "{id:?} mutates a document; an extension command cannot")
            }
            Self::InvalidMessages(why) => f.write_str(why),
            Self::ForeignMessage(id) => {
                write!(f, "{id:?} is not one of the extension's own messages")
            }
        }
    }
}

impl std::error::Error for RegisterError {}
