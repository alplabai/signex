//! The name an extension is known by, and the id namespace it owns.

use std::fmt;
use std::sync::Arc;

/// The validated name of one installed extension.
///
/// Every id an extension adds (a catalog command, a panel key, a menu row,
/// a Start-screen section, a preference) starts with [`ExtensionId::prefix`],
/// `ext.<name>.`, so it can never collide with a built-in id or with another
/// extension's. See `docs/EXTENSION_API.md`, "Ids".
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExtensionId(Arc<str>);

/// Why a name cannot name an extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidExtensionName(pub String);

impl fmt::Display for InvalidExtensionName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "extension name {:?} must start with a lowercase ASCII letter and use only \
             lowercase ASCII letters, digits and '_'",
            self.0
        )
    }
}

impl std::error::Error for InvalidExtensionName {}

impl ExtensionId {
    /// The id for `name`, which starts with a lowercase ASCII letter and
    /// holds only lowercase ASCII letters, digits and `_`
    /// ([`signex_commands::is_extension_name`]).
    pub fn new(name: &str) -> Result<Self, InvalidExtensionName> {
        if signex_commands::is_extension_name(name) {
            Ok(Self(Arc::from(name)))
        } else {
            Err(InvalidExtensionName(name.to_owned()))
        }
    }

    /// The bare name, as the extension gave it.
    pub fn name(&self) -> &str {
        &self.0
    }

    /// The prefix of every id this extension owns: `ext.<name>.`.
    pub fn prefix(&self) -> String {
        format!("ext.{}.", self.0)
    }

    /// Is `id` in this extension's namespace (`ext.<name>.` and at least
    /// one more character)? Only the namespace: whether the rest is a valid
    /// id is [`signex_commands::is_extension_id`].
    pub fn owns(&self, id: &str) -> bool {
        id.strip_prefix(signex_commands::EXTENSION_ID_PREFIX)
            .and_then(|rest| rest.strip_prefix(self.name()))
            .and_then(|rest| rest.strip_prefix('.'))
            .is_some_and(|local| !local.is_empty())
    }
}

impl fmt::Display for ExtensionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
