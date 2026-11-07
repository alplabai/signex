//! Committing the Language draft (#2055).

use std::sync::Arc;

use signex_i18n::I18n;

use super::super::super::*;

impl Signex {
    /// Put the drafted language in force. Menus and Preferences read the new
    /// `I18n` from the next frame; the native menu rebuilds on its next sync,
    /// because its inputs hold the instance.
    pub(super) fn commit_language_draft(&mut self) {
        if self.ui_state.preferences_draft_language == *self.ui_state.i18n.pref() {
            return;
        }
        let next = Arc::new(I18n::new(&self.ui_state.preferences_draft_language));
        self.ui_state.preferences_draft_language = next.pref().clone();
        self.ui_state.i18n = next;
    }
}
