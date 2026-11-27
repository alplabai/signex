//! The installed extensions' `.ftl` files in the app's message catalog
//! (#2495): every `I18n` the app builds for a language carries them, so an
//! extension's titles and labels follow a language switch.

use std::sync::Arc;

use signex_i18n::{ExtensionMessages, I18n, LanguagePref};

use super::ExtensionHost;
use crate::app::Signex;

impl ExtensionHost {
    /// Every message file the installed extensions ship, in install order.
    pub fn message_files(&self) -> Vec<ExtensionMessages> {
        self.points()
            .flat_map(|points| points.messages.files().iter().copied())
            .collect()
    }
}

impl Signex {
    /// The catalog for `pref`, with the installed extensions' messages.
    pub(crate) fn i18n_for(&self, pref: &LanguagePref) -> I18n {
        I18n::new(pref).with_extension_messages(&self.extensions.message_files())
    }

    /// Rebuild the catalog in force with the installed extensions' messages,
    /// and what was rendered from it at boot (the keymap's labels, the panel
    /// context). Called by `with_extensions` when an extension ships
    /// messages.
    pub(super) fn relocalize_for_extensions(&mut self) {
        let next = Arc::new(self.i18n_for(self.ui_state.i18n.pref()));
        self.ui_state.i18n = next;
        self.ui_state.active_keymap =
            Arc::new(self.ui_state.active_keymap.labelled_in(&self.ui_state.i18n));
        self.document_view.panel_ctx.i18n = Arc::clone(&self.ui_state.i18n);
        self.ui_state.refresh_shortcut_sections();
    }
}
