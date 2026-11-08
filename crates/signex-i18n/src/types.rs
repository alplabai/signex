//! The three kinds of text the UI deals with.

use std::fmt;

/// Identifies a catalog message. Ids are semantic and area-prefixed
/// (`menu-file`); an id changes when its meaning changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MessageId(&'static str);

impl MessageId {
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// A rendered message, for display only. It has no `Serialize`, and no
/// equality with `&str`: state, files and undo labels keep the
/// [`MessageId`] and its arguments, never the rendered text.
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

/// Text the user wrote (net names, part descriptions, parameter values).
/// Never translated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserText(String);

impl UserText {
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UserText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A message argument: text, or a number formatted for the locale.
#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    Text(String),
    Number(f64),
}

impl From<&str> for Arg {
    fn from(text: &str) -> Self {
        Self::Text(text.to_owned())
    }
}

impl From<String> for Arg {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}

impl From<f64> for Arg {
    fn from(number: f64) -> Self {
        Self::Number(number)
    }
}

impl From<i64> for Arg {
    fn from(number: i64) -> Self {
        Self::Number(number as f64)
    }
}
