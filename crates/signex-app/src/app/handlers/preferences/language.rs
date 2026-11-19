//! The one runtime entry that switches the UI language (#2055), and the
//! Language page's draft state in Preferences (#144a).

use std::sync::Arc;

use iced::Task;
use signex_i18n::{I18n, LanguagePref};

use super::super::super::*;

/// Say once that a saved or chosen language is not in this build.
pub(crate) fn log_dropped_language(i18n: &I18n) {
    if let Some(lost) = i18n.dropped_locale() {
        crate::diagnostics::log_warning(format!(
            "Language: \"{lost}\" is not available in this version, so Signex shows English. Your choice is kept."
        ));
    }
}

/// The name of the language "Follow the system" resolves to, in that
/// language's own name: what the Language page's first row shows.
pub(crate) fn system_language_name(i18n: &I18n) -> String {
    let tag = signex_i18n::resolved_locale(&LanguagePref::System);
    i18n.option_for(&LanguagePref::Locale(tag.to_owned())).label
}

impl Signex {
    /// Switch the language now and persist it. Menus read the new `I18n` from
    /// the next frame; the native menu rebuilds on its next sync because its
    /// inputs hold the instance. The palette, the menu and Preferences' Save
    /// call this; no restart. A no-op when the language is already in force.
    pub(crate) fn set_language(&mut self, pref: LanguagePref) -> Task<Message> {
        if pref == *self.ui_state.i18n.pref() {
            return Task::none();
        }
        let next = Arc::new(I18n::new(&pref));
        log_dropped_language(&next);
        // A Preferences draft that still stood for the language in force
        // follows the switch, so a palette or menu change made while the
        // dialog is open does not read as an unsaved Language change.
        let draft_followed =
            self.ui_state.preferences_pages.draft_language == *self.ui_state.i18n.pref();
        self.ui_state.i18n = next;
        if draft_followed {
            self.ui_state.preferences_pages.draft_language = pref.clone();
        }
        if self.ui_state.preferences_open {
            self.refresh_language_preview();
            self.recompute_preferences_dirty();
        }
        // The shortcuts-sheet cache holds labels rendered in the old language.
        self.ui_state.refresh_shortcut_sections();
        // So does the live keymap: menus, palette, hints and key caps read it.
        self.ui_state.active_keymap =
            Arc::new(self.ui_state.active_keymap.labelled_in(&self.ui_state.i18n));
        crate::fonts::background_pref_write("language", move || {
            crate::fonts::write_language_pref(&pref)
        })
    }

    /// Build the Language page's preview catalog for the drafted language:
    /// the live one while the draft is the language in force, else a new
    /// one. Built here, never in `view`.
    pub(crate) fn refresh_language_preview(&mut self) {
        let draft = &self.ui_state.preferences_pages.draft_language;
        let preview = if draft == self.ui_state.i18n.pref() {
            Arc::clone(&self.ui_state.i18n)
        } else {
            Arc::new(I18n::new(draft))
        };
        self.ui_state.preferences_pages.language_preview = Some(preview);
    }

    /// Seed the Language page from the language in force (dialog open and
    /// Discard): the draft, its preview, and the name of the language
    /// "Follow the system" resolves to.
    pub(crate) fn seed_language_draft(&mut self) {
        self.ui_state.preferences_pages.draft_language = self.ui_state.i18n.pref().clone();
        self.ui_state.preferences_pages.system_language_name =
            system_language_name(&self.ui_state.i18n);
        self.refresh_language_preview();
    }
}
