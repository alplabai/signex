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

    /// The text before the argument value `value`, the value itself and the
    /// text after it, for drawing the value as its own widget (a key cap
    /// inside a sentence). The isolated placeable is looked for first, so a
    /// value that also occurs in the words is found where the argument is;
    /// the isolation marks are dropped with the split. `None` when the
    /// translation leaves the argument out or `value` is empty.
    pub fn split_around(&self, value: &str) -> Option<(Localized, Localized, Localized)> {
        if value.is_empty() {
            return None;
        }
        let isolated = format!("\u{2068}{value}\u{2069}");
        let (before, after) = self
            .0
            .split_once(&isolated)
            .or_else(|| self.0.split_once(value))?;
        Some((
            Self(before.to_owned()),
            Self(value.to_owned()),
            Self(after.to_owned()),
        ))
    }
}

impl fmt::Display for Localized {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
