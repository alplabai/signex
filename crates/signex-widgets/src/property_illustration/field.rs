//! The id that links a drawing part to a Properties field.

use std::borrow::Cow;

/// Id of a Properties field (or of a feature switch such as "paste on").
///
/// The same id sits on the field row and on every drawing part that shows
/// that field, so a hover on either side can light up the other.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FieldId(Cow<'static, str>);

impl FieldId {
    /// A field id from a static key, usable in `const` items.
    pub const fn new(id: &'static str) -> Self {
        Self(Cow::Borrowed(id))
    }

    /// A field id built at runtime (for example per pad-stack layer).
    pub fn owned(id: impl Into<String>) -> Self {
        Self(Cow::Owned(id.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&'static str> for FieldId {
    fn from(id: &'static str) -> Self {
        Self::new(id)
    }
}

impl std::fmt::Display for FieldId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
