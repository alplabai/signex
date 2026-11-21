//! Text kept in state and rendered at view time (epic #2053, decision 4).

use std::sync::Arc;

use crate::{Arg, MessageId, UserText};

/// Renders a catalog message as plain text. `signex-i18n`'s `I18n` is the
/// renderer; this crate only names it, so it stays free of any locale
/// dependency.
pub trait MessageRenderer {
    /// `id` with `args`, without bidi isolation marks.
    fn message_plain(&self, id: MessageId, args: &[(&str, Arg)]) -> String;
}

/// The app state and `I18n::en_us` hand out a shared `Arc<I18n>`.
impl<R: MessageRenderer + ?Sized> MessageRenderer for Arc<R> {
    fn message_plain(&self, id: MessageId, args: &[(&str, Arg)]) -> String {
        (**self).message_plain(id, args)
    }
}

/// A message held in state: a catalog id with owned arguments, or text that
/// is not ours (an OS error line, a path, user content) shown verbatim. The
/// view renders it with the active `I18n`, so a language switch re-renders
/// it; output that leaves the screen (logs, agent replies) renders it with
/// `I18n::en_us` so it stays locale-invariant.
#[derive(Debug, Clone, PartialEq)]
pub struct Translatable(Inner);

#[derive(Debug, Clone, PartialEq)]
enum Inner {
    Catalog {
        id: MessageId,
        args: Vec<(&'static str, Arg)>,
    },
    Verbatim(UserText),
    LegacyEnglish(String),
}

impl Translatable {
    /// The catalog message `id`, without arguments so far.
    pub fn new(id: MessageId) -> Self {
        Self(Inner::Catalog {
            id,
            args: Vec::new(),
        })
    }

    /// `self` with the argument `name` set to `value`.
    #[must_use]
    pub fn arg(self, name: &'static str, value: impl Into<Arg>) -> Self {
        match self.0 {
            Inner::Catalog { id, mut args } => {
                args.push((name, value.into()));
                Self(Inner::Catalog { id, args })
            }
            other => Self(other),
        }
    }

    /// Text that is not a catalog message, shown as it is.
    pub fn verbatim(text: UserText) -> Self {
        Self(Inner::Verbatim(text))
    }

    /// App-authored English that has no catalog id yet (#2326 M2): shown as
    /// it is in every language until its area moves to catalog ids (#2644).
    /// Not for user text, which is [`Self::verbatim`].
    pub fn legacy_english(text: impl Into<String>) -> Self {
        Self(Inner::LegacyEnglish(text.into()))
    }

    /// The catalog id, `None` for verbatim or legacy English text.
    pub fn id(&self) -> Option<MessageId> {
        match &self.0 {
            Inner::Catalog { id, .. } => Some(*id),
            Inner::Verbatim(_) | Inner::LegacyEnglish(_) => None,
        }
    }

    /// The text in `i18n`'s language. Plain text without bidi isolation
    /// marks ([`MessageRenderer::message_plain`]): the same string goes to
    /// the screen, to window titles and to en-US logs, which must stay
    /// byte-identical with the English the code built before the catalog.
    /// A nested message argument is rendered first, with the same `i18n`.
    pub fn render<R: MessageRenderer + ?Sized>(&self, i18n: &R) -> String {
        match &self.0 {
            Inner::Catalog { id, args } => {
                let args: Vec<(&str, Arg)> = args
                    .iter()
                    .map(|(name, a)| (*name, a.rendered_with(i18n)))
                    .collect();
                i18n.message_plain(*id, &args)
            }
            Inner::Verbatim(text) => text.as_str().to_owned(),
            Inner::LegacyEnglish(text) => text.clone(),
        }
    }

    /// The id, or the verbatim text, for a nested message that reached the
    /// formatter without being rendered.
    pub(crate) fn unrendered(&self) -> &str {
        match &self.0 {
            Inner::Catalog { id, .. } => id.as_str(),
            Inner::Verbatim(text) => text.as_str(),
            Inner::LegacyEnglish(text) => text,
        }
    }
}

impl From<MessageId> for Translatable {
    fn from(id: MessageId) -> Self {
        Self::new(id)
    }
}

impl From<UserText> for Translatable {
    fn from(text: UserText) -> Self {
        Self::verbatim(text)
    }
}

#[cfg(test)]
#[path = "translatable_tests.rs"]
mod tests;
