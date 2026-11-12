//! The one runtime entry that switches the UI language (#2055).

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

impl Signex {
    /// Switch the language now and persist it. Menus read the new `I18n` from
    /// the next frame; the native menu rebuilds on its next sync because its
    /// inputs hold the instance. The future Preferences picker will call
    /// this; no restart. A no-op when the language is already in force.
    pub(crate) fn set_language(&mut self, pref: LanguagePref) -> Task<Message> {
        if pref == *self.ui_state.i18n.pref() {
            return Task::none();
        }
        let next = Arc::new(I18n::new(&pref));
        log_dropped_language(&next);
        self.ui_state.i18n = next;
        // The shortcuts-sheet cache holds labels rendered in the old language.
        self.ui_state.refresh_shortcut_sections();
        crate::fonts::background_pref_write("language", move || {
            crate::fonts::write_language_pref(&pref)
        })
    }
}
