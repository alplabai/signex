//! Rendering a [`Translatable`] (epic #2053, decision 4): the ids crate holds
//! the value, this crate supplies the [`I18n`] that renders it.

use crate::i18n::I18n;
use crate::{Arg, Localized, MessageId, MessageRenderer, Translatable};

impl MessageRenderer for I18n {
    fn message_plain(&self, id: MessageId, args: &[(&str, Arg)]) -> String {
        I18n::message_plain(self, id, args)
    }
}

/// The renderings of a [`Translatable`] that need this crate: a widget's
/// [`Localized`], and en-US for output that leaves the screen. Import it
/// where those are called; [`Translatable::render`] needs no import.
pub trait RenderTranslatable {
    /// [`Translatable::render`] for a widget.
    fn localized(&self, i18n: &I18n) -> Localized;

    /// [`Translatable::render`] in en-US, for output that leaves the screen.
    fn render_en_us(&self) -> String;
}

impl RenderTranslatable for Translatable {
    fn localized(&self, i18n: &I18n) -> Localized {
        Localized::new(self.render(i18n))
    }

    fn render_en_us(&self) -> String {
        self.render(&I18n::en_us())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LanguagePref, UserText, ids};

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

    /// #2059: `t!`, `message` and `message_plain` render a nested message
    /// argument too, in their own language, never as its id.
    #[test]
    fn every_render_path_renders_a_nested_message() {
        let en = I18n::en_us();
        let project = || Translatable::new(crate::menu_ids::MENU_PROJECT);
        let shown = crate::t!(en, ids::NOTICE_MENU_PATH, "menu" => project(), "row" => "Run")
            .into_string()
            .replace(['\u{2068}', '\u{2069}'], "");
        assert_eq!(shown, "Project \u{25B8} Run");
        let args = [("menu", Arg::from(project())), ("row", Arg::from("Run"))];
        assert_eq!(
            en.message_plain(ids::NOTICE_MENU_PATH, &args),
            "Project \u{25B8} Run"
        );
        let xa = pseudo().message_plain(ids::NOTICE_MENU_PATH, &args);
        assert!(!xa.contains("menu-project"), "{xa}");
        assert!(
            xa.starts_with(&pseudo().message_plain(crate::menu_ids::MENU_PROJECT, &[])),
            "{xa}"
        );
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
