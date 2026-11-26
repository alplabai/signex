//! A message id, user text, and the arguments a message takes. The third
//! kind of text, `Localized`, is rendered output and lives in `signex-i18n`.

use std::fmt;

use crate::{MessageRenderer, Translatable};

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

    /// The id of a message an extension ships in its own `.ftl` file:
    /// `ext-` and then lowercase ASCII letters, digits, `_` and `-`. `None`
    /// for any other id, so code outside this crate cannot make the id of a
    /// built-in message; no built-in id starts with `ext-`. Which extension
    /// owns the id (`ext-<name>-…`) is checked where the extension registers
    /// its messages. A `const fn`, so an extension's catalog row can name
    /// its label in a `static`.
    pub const fn extension(id: &'static str) -> Option<Self> {
        let bytes = id.as_bytes();
        let prefix = EXTENSION_MESSAGE_PREFIX.as_bytes();
        if bytes.len() <= prefix.len() {
            return None;
        }
        let mut i = 0;
        while i < bytes.len() {
            let byte = bytes[i];
            let fits = if i < prefix.len() {
                byte == prefix[i]
            } else {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
            };
            if !fits {
                return None;
            }
            i += 1;
        }
        Some(Self(id))
    }

    /// Is this an extension's message id (`ext-…`)?
    pub fn is_extension(self) -> bool {
        self.0.starts_with(EXTENSION_MESSAGE_PREFIX)
    }

    /// An id outside the registry, for `signex-i18n`'s tests of a message
    /// the catalog lacks.
    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub const fn unlisted_for_test(id: &'static str) -> Self {
        Self(id)
    }
}

/// The prefix of every message id an extension ships (`ext-<name>-…`).
pub const EXTENSION_MESSAGE_PREFIX: &str = "ext-";

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

/// A message argument: text, a number formatted for the locale, or another
/// message (`From<Translatable>`) that [`Translatable::render`] renders with
/// the same renderer before it fills this one. The variants are private: the
/// only ways in are the text and integer `From` impls, `From<LocalNumber>`
/// and `From<Translatable>`, so a bare float cannot reach the locale number
/// formatter.
///
/// `signex-i18n`'s `t!` carries the same guarantee and its own doctests.
///
/// ```compile_fail,E0277
/// let _ = signex_i18n_ids::Arg::from(1.27_f64);
/// ```
///
/// ```compile_fail,E0599
/// let _ = signex_i18n_ids::Arg::Number(1.27);
/// ```
///
/// The same setup compiles once the number is wrapped, so the two failures
/// above are not caused by a renamed type or constructor:
///
/// ```
/// use signex_i18n_ids::{Arg, LocalNumber};
/// let _ = Arg::from(LocalNumber(1.27));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Arg(ArgKind);

#[derive(Debug, Clone, PartialEq)]
enum ArgKind {
    Text(String),
    Number(f64),
    Message(Box<Translatable>),
}

/// A borrowed read of an [`Arg`], for the formatter in `signex-i18n`. It
/// converts to nothing, so it opens no way in for a bare float.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ArgValue<'a> {
    Text(&'a str),
    Number(f64),
}

impl Arg {
    /// What the argument holds, for the formatter. A renderer passes every
    /// argument through [`Arg::rendered_with`] first; a nested message that
    /// skipped it reads as its id, the way a message missing from the
    /// catalog does.
    #[doc(hidden)]
    pub fn value(&self) -> ArgValue<'_> {
        match &self.0 {
            ArgKind::Text(text) => ArgValue::Text(text),
            ArgKind::Number(number) => ArgValue::Number(*number),
            ArgKind::Message(message) => ArgValue::Text(message.unrendered()),
        }
    }

    /// `self`, with a nested message rendered by `renderer` into text; any
    /// other argument unchanged. Every render path calls it on its
    /// arguments, so the ids crate itself never renders.
    #[doc(hidden)]
    pub fn rendered_with<R: MessageRenderer + ?Sized>(&self, renderer: &R) -> Self {
        match &self.0 {
            ArgKind::Message(message) => Self::from(message.render(renderer)),
            _ => self.clone(),
        }
    }

    /// Whether this argument is a nested message, the only kind
    /// [`Arg::rendered_with`] changes.
    #[doc(hidden)]
    pub fn is_message(&self) -> bool {
        matches!(self.0, ArgKind::Message(_))
    }
}

impl From<Translatable> for Arg {
    fn from(message: Translatable) -> Self {
        Self(ArgKind::Message(Box::new(message)))
    }
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
