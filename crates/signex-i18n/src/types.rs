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

/// A count, size or percentage shown in the UI language; never a length,
/// angle or component value (those stay `.` in every locale and are passed
/// as text from Signex's own formatter).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalNumber(pub f64);

/// A message argument: text, or a number formatted for the locale. There is
/// no `From<f64>`: a bare float cannot reach the locale number formatter.
///
/// ```compile_fail
/// use signex_i18n::{I18n, MessageId, t};
/// let i18n = I18n::en_us();
/// let _ = t!(i18n, MessageId::new("some-id"), "value" => 1.27_f64);
/// ```
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

impl From<LocalNumber> for Arg {
    fn from(number: LocalNumber) -> Self {
        Self::Number(number.0)
    }
}

/// Integer counts are the one bare number a message takes.
macro_rules! count_arg {
    ($($int:ty),+) => {$(
        impl From<$int> for Arg {
            fn from(count: $int) -> Self {
                Self::Number(count as f64)
            }
        }
    )+};
}
count_arg!(i32, u32, i64, u64, usize);
