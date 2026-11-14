//! Text kept in state and rendered at view time (epic #2053, decision 4).

use crate::i18n::I18n;
use crate::types::{Arg, Localized, MessageId, UserText};

/// A message held in state: a catalog id with owned arguments, or text that
/// is not ours (an OS error line, a path, user content) shown verbatim. The
/// view renders it with the active [`I18n`], so a language switch re-renders
/// it; output that leaves the screen (logs, agent replies) renders it with
/// [`I18n::en_us`] so it stays locale-invariant.
#[derive(Debug, Clone, PartialEq)]
pub struct Translatable(Inner);

#[derive(Debug, Clone, PartialEq)]
enum Inner {
    Catalog {
        id: MessageId,
        args: Vec<(&'static str, Arg)>,
    },
    Verbatim(UserText),
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
            verbatim @ Inner::Verbatim(_) => Self(verbatim),
        }
    }

    /// Text that is not a catalog message, shown as it is.
    pub fn verbatim(text: UserText) -> Self {
        Self(Inner::Verbatim(text))
    }

    /// The catalog id, `None` for verbatim text.
    pub fn id(&self) -> Option<MessageId> {
        match &self.0 {
            Inner::Catalog { id, .. } => Some(*id),
            Inner::Verbatim(_) => None,
        }
    }

    /// The text in `i18n`'s language. Plain text without bidi isolation
    /// marks ([`I18n::message_plain`]): the same string goes to the screen,
    /// to window titles and to en-US logs, which must stay byte-identical
    /// with the English the code built before the catalog.
    pub fn render(&self, i18n: &I18n) -> String {
        match &self.0 {
            Inner::Catalog { id, args } => {
                let args: Vec<(&str, Arg)> =
                    args.iter().map(|(name, a)| (*name, a.clone())).collect();
                i18n.message_plain(*id, &args)
            }
            Inner::Verbatim(text) => text.as_str().to_owned(),
        }
    }

    /// [`Self::render`] for a widget.
    pub fn localized(&self, i18n: &I18n) -> Localized {
        Localized::new(self.render(i18n))
    }

    /// [`Self::render`] in en-US, for output that leaves the screen.
    pub fn render_en_us(&self) -> String {
        self.render(&I18n::en_us())
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
mod tests {
    use super::*;
    use crate::{LanguagePref, ids};

    fn pseudo() -> I18n {
        I18n::new(&LanguagePref::Locale("en-XA".to_owned()))
    }

    #[test]
    fn render_fills_the_arguments_without_isolation_marks() {
        let text =
            Translatable::new(ids::NOTICE_LIBRARY_READ_ONLY_DETAIL).arg("library", "Basic Parts");
        assert_eq!(
            text.render_en_us(),
            "The library \"Basic Parts\" ships with Signex and is not editable."
        );
        assert_eq!(text.id(), Some(ids::NOTICE_LIBRARY_READ_ONLY_DETAIL));
    }

    #[test]
    fn render_follows_the_language_it_is_given() {
        let text = Translatable::new(ids::NOTICE_SAVE_FAILED);
        let shown = text.render(&pseudo());
        assert_ne!(shown, "Save Failed");
        assert_eq!(text.render_en_us(), "Save Failed");
    }

    #[test]
    fn verbatim_text_passes_through_in_every_language() {
        let line = "Permission denied (os error 13): C:\\Projekt\\ü.snxsch";
        let text = Translatable::from(UserText::new(line)).arg("ignored", "x");
        assert_eq!(text.render(&pseudo()), line);
        assert_eq!(text.render_en_us(), line);
        assert_eq!(text.id(), None);
    }
}
