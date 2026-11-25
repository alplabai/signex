//! Preferences an extension adds. Each key sits under its owner's
//! `ext.<name>.` prefix, so it never collides with a built-in preference.

/// The preferences one extension adds. No entry type yet: the change that
/// opens this point (#2495) adds it here.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct Preferences {}

impl Preferences {
    /// Were no preferences added?
    pub fn is_empty(&self) -> bool {
        true
    }
}
