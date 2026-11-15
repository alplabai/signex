//! Rendered text, the one kind of text only this crate makes.

use std::fmt;

/// A rendered message, for display only. It has no `Serialize`, and no
/// equality with `&str`: state, files and undo labels keep the
/// [`MessageId`](crate::MessageId) and its arguments, never the rendered
/// text.
#[derive(Debug, Clone)]
pub struct Localized(String);

impl Localized {
    pub(crate) fn new(text: String) -> Self {
        Self(text)
    }

    /// Hand the text to a widget.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for Localized {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
