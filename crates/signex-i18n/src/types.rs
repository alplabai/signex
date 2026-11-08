//! The three kinds of text the UI deals with.

use std::fmt;

/// Identifies a catalog message. Ids are semantic and area-prefixed
/// (`menu-file`); an id changes when its meaning changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MessageId(&'static str);

impl MessageId {
    pub(crate) const fn new(id: &'static str) -> Self {
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

/// A fractional number shown in the UI language (a percentage, a ratio).
/// Wrapping a value in `LocalNumber` is an explicit opt-in that review
/// guards, not the type system: a length wrapped in it compiles. Lengths,
/// angles and component values stay `.` in every locale and are passed as
/// text from Signex's own formatter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalNumber(pub f64);

/// A message argument: text, or a number formatted for the locale. The
/// variants are private: the only ways in are the text and integer `From`
/// impls and `From<LocalNumber>`, so a bare float cannot reach the locale
/// number formatter.
///
/// ```compile_fail,E0277
/// use signex_i18n::{I18n, ids, t};
/// let i18n = I18n::en_us();
/// let _ = t!(i18n, ids::MENU_FILE, "value" => 1.27_f64);
/// ```
///
/// ```compile_fail,E0599
/// let _ = signex_i18n::Arg::Number(1.27);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Arg(pub(crate) ArgKind);

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ArgKind {
    Text(String),
    Number(f64),
}

impl From<&str> for Arg {
    fn from(text: &str) -> Self {
        Self(ArgKind::Text(text.to_owned()))
    }
}

impl From<String> for Arg {
    fn from(text: String) -> Self {
        Self(ArgKind::Text(text))
    }
}

impl From<LocalNumber> for Arg {
    fn from(number: LocalNumber) -> Self {
        Self(ArgKind::Number(number.0))
    }
}

/// Integer counts are the one bare number a message takes.
macro_rules! count_arg {
    ($($int:ty),+) => {$(
        impl From<$int> for Arg {
            fn from(count: $int) -> Self {
                Self(ArgKind::Number(count as f64))
            }
        }
    )+};
}
count_arg!(i32, u32, i64, u64, usize);
